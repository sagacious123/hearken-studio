//! An anonymous ID for this computer: a hash of the ID the operating system gives it (it
//! survives reinstalling the app), salted so it can't be matched with anything else.

use sha2::{Digest, Sha256};

/// 64 hex characters, or `None` if the system won't say (the trial then relies on local records).
pub fn fingerprint() -> Option<String> {
    let id = system_id()?;
    let id = id.trim();
    if id.len() < 8 {
        return None;
    }
    Some(hex(&Sha256::digest(format!("hearken-studio/machine/v1/{id}").as_bytes())))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(target_os = "macos")]
fn system_id() -> Option<String> {
    // The hardware UUID: `ioreg -rd1 -c IOPlatformExpertDevice` → "IOPlatformUUID" = "…"
    let out = std::process::Command::new("/usr/sbin/ioreg").args(["-rd1", "-c", "IOPlatformExpertDevice"]).output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().find(|l| l.contains("IOPlatformUUID"))?;
    line.rsplit('"').nth(1).map(str::to_owned)
}

#[cfg(target_os = "windows")]
fn system_id() -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // Set when Windows is installed: HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid
    let out = std::process::Command::new("reg")
        .args(["query", r"HKLM\SOFTWARE\Microsoft\Cryptography", "/v", "MachineGuid", "/reg:64"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines().find(|l| l.contains("MachineGuid"))?.split_whitespace().last().map(str::to_owned)
}

#[cfg(target_os = "linux")]
fn system_id() -> Option<String> {
    ["/etc/machine-id", "/var/lib/dbus/machine-id"].iter().find_map(|p| std::fs::read_to_string(p).ok())
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn system_id() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn this_computer_has_a_stable_id() {
        let a = super::fingerprint();
        assert_eq!(a, super::fingerprint());
        if let Some(id) = a {
            assert_eq!(id.len(), 64);
        }
    }
}
