//! Announce a friendly name for this computer on the church network ("hearken.local"), so phones
//! can open http://hearken.local instead of an IP address. iPhones, Macs and Windows PCs resolve
//! `.local` names (mDNS); some Android phones don't, so the IP address stays as a fallback.
//!
//! macOS: registered with the system's Bonjour service (as `dns-sd -P` does). It handles the
//! network, the computer's own lookups and name conflicts, and drops the name when the app
//! quits, even after a crash. Elsewhere: announced by the `mdns-sd` responder.

use std::net::Ipv4Addr;

use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameStatus {
    /// Still checking that no other device uses the name.
    Checking,
    Announced,
    /// Another device on the network already uses this name.
    Conflict,
    Failed,
}

/// "Grace Chapel!" → "grace-chapel": letters, digits and hyphens, as DNS labels allow.
pub fn clean_name(raw: &str) -> String {
    let s: String = raw.trim().to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    let s: String = s.chars().take(40).collect();
    let s = s.trim_end_matches('-').to_owned();
    if s.is_empty() { "hearken".into() } else { s }
}

pub struct Announcement {
    imp: imp::Imp,
}

impl Announcement {
    pub fn start(name: &str, ip: Ipv4Addr, port: u16) -> Result<Self> {
        Ok(Self { imp: imp::Imp::start(name, ip, port)? })
    }

    pub fn status(&self) -> NameStatus {
        self.imp.status()
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use std::{
        ffi::{c_void, CString},
        net::Ipv4Addr,
        sync::{
            atomic::{AtomicBool, AtomicI32, Ordering::Relaxed},
            Arc,
        },
        thread::JoinHandle,
    };

    use anyhow::{bail, Result};

    use super::NameStatus;

    type DNSServiceRef = *mut c_void;
    type DNSRecordRef = *mut c_void;
    type RegisterRecordReply = extern "C" fn(DNSServiceRef, DNSRecordRef, u32, i32, *mut c_void);

    // libSystem's Bonjour client API (dns_sd.h).
    extern "C" {
        fn DNSServiceCreateConnection(sd: *mut DNSServiceRef) -> i32;
        #[allow(clippy::too_many_arguments)]
        fn DNSServiceRegisterRecord(
            sd: DNSServiceRef,
            record: *mut DNSRecordRef,
            flags: u32,
            interface_index: u32,
            fullname: *const std::ffi::c_char,
            rrtype: u16,
            rrclass: u16,
            rdlen: u16,
            rdata: *const c_void,
            ttl: u32,
            callback: RegisterRecordReply,
            context: *mut c_void,
        ) -> i32;
        fn DNSServiceProcessResult(sd: DNSServiceRef) -> i32;
        fn DNSServiceRefSockFD(sd: DNSServiceRef) -> i32;
        fn DNSServiceRefDeallocate(sd: DNSServiceRef);
    }

    const FLAGS_UNIQUE: u32 = 0x20; // probe first; report a conflict if someone has the name
    const TYPE_A: u16 = 1;
    const CLASS_IN: u16 = 1;
    const ERR_NAME_CONFLICT: i32 = -65548;
    const PENDING: i32 = 1;

    /// Result code from Bonjour: PENDING until probing finishes, then 0 or an error.
    extern "C" fn on_registered(_: DNSServiceRef, _: DNSRecordRef, _: u32, error: i32, context: *mut c_void) {
        // SAFETY: `context` is the AtomicI32 inside the Arc kept alive by `Imp` until after the
        // connection (and so every callback) is gone.
        let status = unsafe { &*(context as *const AtomicI32) };
        status.store(error, Relaxed);
    }

    /// Sendable wrapper: the connection is only ever used by the results thread, then
    /// deallocated after that thread has been joined.
    struct Conn(DNSServiceRef);
    // SAFETY: see above; there is never concurrent use.
    unsafe impl Send for Conn {}

    pub struct Imp {
        status: Arc<AtomicI32>,
        stop: Arc<AtomicBool>,
        thread: Option<JoinHandle<Conn>>,
    }

    impl Imp {
        pub fn start(name: &str, ip: Ipv4Addr, _port: u16) -> Result<Self> {
            let mut sd: DNSServiceRef = std::ptr::null_mut();
            // SAFETY: out-pointer to a local; checked below.
            let err = unsafe { DNSServiceCreateConnection(&mut sd) };
            if err != 0 || sd.is_null() {
                bail!("Bonjour isn't available (error {err})");
            }
            let status = Arc::new(AtomicI32::new(PENDING));
            let fullname = CString::new(format!("{name}.local."))?;
            let rdata = ip.octets();
            let mut record: DNSRecordRef = std::ptr::null_mut();
            // SAFETY: valid connection, NUL-terminated name, 4-byte A record; the context
            // pointer outlives the connection (see `on_registered`).
            let err = unsafe {
                DNSServiceRegisterRecord(
                    sd,
                    &mut record,
                    FLAGS_UNIQUE,
                    0,
                    fullname.as_ptr(),
                    TYPE_A,
                    CLASS_IN,
                    4,
                    rdata.as_ptr().cast(),
                    120,
                    on_registered,
                    Arc::as_ptr(&status) as *mut c_void,
                )
            };
            if err != 0 {
                // SAFETY: created above, not used elsewhere.
                unsafe { DNSServiceRefDeallocate(sd) };
                bail!("couldn't announce {name}.local (error {err})");
            }

            // Bonjour reports probing results on the connection's socket.
            let stop = Arc::new(AtomicBool::new(false));
            let (stop2, conn) = (stop.clone(), Conn(sd));
            let thread = std::thread::Builder::new().name("bonjour".into()).spawn(move || {
                let conn = conn;
                // SAFETY: valid connection owned by this thread.
                let fd = unsafe { DNSServiceRefSockFD(conn.0) };
                while !stop2.load(Relaxed) {
                    let mut pfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
                    // SAFETY: one valid pollfd.
                    let ready = unsafe { libc::poll(&mut pfd, 1, 300) };
                    if ready > 0 {
                        // SAFETY: readable connection owned by this thread.
                        if unsafe { DNSServiceProcessResult(conn.0) } != 0 {
                            break;
                        }
                    }
                }
                conn
            })?;
            Ok(Self { status, stop, thread: Some(thread) })
        }

        pub fn status(&self) -> NameStatus {
            match self.status.load(Relaxed) {
                PENDING => NameStatus::Checking,
                0 => NameStatus::Announced,
                ERR_NAME_CONFLICT => NameStatus::Conflict,
                _ => NameStatus::Failed,
            }
        }
    }

    impl Drop for Imp {
        fn drop(&mut self) {
            self.stop.store(true, Relaxed);
            if let Some(conn) = self.thread.take().and_then(|t| t.join().ok()) {
                // SAFETY: the results thread has finished; this frees the connection and
                // withdraws the name. No callback can run afterwards, so `status` may drop.
                unsafe { DNSServiceRefDeallocate(conn.0) };
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use std::net::Ipv4Addr;

    use anyhow::Result;
    use mdns_sd::{ServiceDaemon, ServiceInfo};

    use super::NameStatus;

    pub struct Imp {
        daemon: ServiceDaemon,
    }

    impl Imp {
        pub fn start(name: &str, ip: Ipv4Addr, port: u16) -> Result<Self> {
            let daemon = ServiceDaemon::new()?;
            let info = ServiceInfo::new(
                "_http._tcp.local.",
                "Hearken Studio",
                &format!("{name}.local."),
                ip.to_string().as_str(),
                port,
                None::<std::collections::HashMap<String, String>>,
            )?;
            daemon.register(info)?;
            Ok(Self { daemon })
        }

        pub fn status(&self) -> NameStatus {
            NameStatus::Announced // this responder doesn't probe for conflicts
        }
    }

    impl Drop for Imp {
        fn drop(&mut self) {
            let _ = self.daemon.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_cleaned_to_dns_labels() {
        assert_eq!(clean_name("Grace Chapel!"), "grace-chapel");
        assert_eq!(clean_name("  RCCG -- Victory Parish  "), "rccg-victory-parish");
        assert_eq!(clean_name("…"), "hearken");
        assert_eq!(clean_name(&"a".repeat(60)).len(), 40);
    }

    /// Registers a name with Bonjour and resolves it the way a browser would. macOS only.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "touches the local network"]
    fn announced_name_resolves() {
        let ann = Announcement::start("hearken-selftest", Ipv4Addr::new(192, 0, 2, 7), 80).unwrap();
        for _ in 0..30 {
            if ann.status() != NameStatus::Checking {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        assert_eq!(ann.status(), NameStatus::Announced);
        let out = std::process::Command::new("dscacheutil").args(["-q", "host", "-a", "name", "hearken-selftest.local"]).output().unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).contains("192.0.2.7"), "{}", String::from_utf8_lossy(&out.stdout));
        drop(ann);
        std::thread::sleep(std::time::Duration::from_millis(500));
        let out = std::process::Command::new("dscacheutil").args(["-q", "host", "-a", "name", "hearken-selftest.local"]).output().unwrap();
        assert!(!String::from_utf8_lossy(&out.stdout).contains("192.0.2.7"), "name should be withdrawn");
    }
}
