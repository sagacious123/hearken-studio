//! Where the trial record is kept: the app's settings folder, a second folder, and the
//! system's secure storage (Keychain on Mac, the registry on Windows), which uninstalling the
//! app doesn't clear. Each copy is sealed so an edited one is ignored; the earliest start wins.

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::machine::hex;

/// What's remembered between runs. Times are Unix seconds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Record {
    /// When this computer first ran Hearken.
    pub first_seen: i64,
    /// The latest time the computer's clock has shown: a clock behind this was turned back.
    pub latest_seen: i64,
    /// History written at or before this time was checked against the website's clock.
    pub activity_checked: i64,
    /// The trial as registered on the website.
    pub trial_pass: Option<String>,
    /// The church's licence key.
    pub licence: Option<String>,
    /// Signed in: the session to renew with, and the signed pass that works offline.
    pub session: Option<String>,
    pub account_pass: Option<String>,
    /// Stands in for the computer's ID on systems that won't give one.
    pub install_id: Option<String>,
}

impl Record {
    /// Combines copies: the earliest start, the latest clock, and any passes found.
    pub fn merge(copies: impl IntoIterator<Item = Record>) -> Option<Record> {
        copies.into_iter().reduce(|a, b| Record {
            first_seen: match (a.first_seen, b.first_seen) {
                (0, x) | (x, 0) => x,
                (x, y) => x.min(y),
            },
            latest_seen: a.latest_seen.max(b.latest_seen),
            activity_checked: a.activity_checked.max(b.activity_checked),
            trial_pass: a.trial_pass.or(b.trial_pass),
            licence: a.licence.or(b.licence),
            session: a.session.or(b.session),
            account_pass: a.account_pass.or(b.account_pass),
            install_id: a.install_id.or(b.install_id),
        })
    }
}

/// Not secret (it ships in the app), but enough that an edited record doesn't pass as genuine.
const SEAL: &[u8] = b"hearken-studio/record/v1/3f9c1e7a52d84b06";

fn hmac(message: &[u8]) -> String {
    let mut key = [0u8; 64];
    key[..SEAL.len()].copy_from_slice(SEAL);
    let pad = |b: u8| key.iter().map(|k| k ^ b).collect::<Vec<u8>>();
    let inner = Sha256::new().chain_update(pad(0x36)).chain_update(message).finalize();
    hex(&Sha256::new().chain_update(pad(0x5c)).chain_update(inner).finalize())
}

pub fn seal(record: &Record) -> String {
    let json = serde_json::to_string(record).unwrap_or_default();
    let mac = hmac(json.as_bytes());
    format!("{json}\n{mac}")
}

pub fn unseal(text: &str) -> Option<Record> {
    let (json, mac) = text.trim().rsplit_once('\n')?;
    (hmac(json.as_bytes()) == mac.trim()).then(|| serde_json::from_str(json).ok()).flatten()
}

pub trait Store: Send + Sync {
    fn read(&self) -> Option<String>;
    fn write(&self, text: &str);
}

pub struct FileStore(pub PathBuf);

impl Store for FileStore {
    fn read(&self) -> Option<String> {
        fs::read_to_string(&self.0).ok()
    }
    fn write(&self, text: &str) {
        if let Some(dir) = self.0.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Err(e) = fs::write(&self.0, text) {
            log::debug!("couldn't save {}: {e}", self.0.display());
        }
    }
}

/// The login Keychain, through Apple's `security` tool: items it creates can be read back by it
/// without a password prompt, even after the (unsigned) app is updated.
#[cfg(target_os = "macos")]
pub struct SystemStore;

#[cfg(target_os = "macos")]
impl SystemStore {
    const ACCOUNT: &'static str = "hearken-studio";
    const SERVICE: &'static str = "Hearken Studio";
}

#[cfg(target_os = "macos")]
impl Store for SystemStore {
    fn read(&self) -> Option<String> {
        let out = std::process::Command::new("/usr/bin/security")
            .args(["find-generic-password", "-a", Self::ACCOUNT, "-s", Self::SERVICE, "-w"])
            .output()
            .ok()?;
        // Stored base64-encoded: the record has a line break, which `-w` wouldn't keep.
        use base64::{engine::general_purpose::STANDARD, Engine};
        let text = STANDARD.decode(String::from_utf8_lossy(&out.stdout).trim()).ok()?;
        out.status.success().then(|| String::from_utf8(text).ok()).flatten()
    }
    fn write(&self, text: &str) {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let _ = std::process::Command::new("/usr/bin/security")
            .args(["add-generic-password", "-U", "-a", Self::ACCOUNT, "-s", Self::SERVICE, "-w", &STANDARD.encode(text)])
            .output();
    }
}

/// The current user's registry, which the uninstaller leaves alone.
#[cfg(target_os = "windows")]
pub struct SystemStore;

#[cfg(target_os = "windows")]
impl SystemStore {
    const KEY: &'static str = r"HKCU\Software\HearkenStudioRecord";
    fn reg(args: &[&str]) -> Option<std::process::Output> {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("reg").args(args).creation_flags(CREATE_NO_WINDOW).output().ok()
    }
}

#[cfg(target_os = "windows")]
impl Store for SystemStore {
    fn read(&self) -> Option<String> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let out = Self::reg(&["query", Self::KEY, "/v", "Record"])?;
        let text = String::from_utf8_lossy(&out.stdout);
        let value = text.lines().find(|l| l.trim_start().starts_with("Record"))?.split_whitespace().last()?.to_owned();
        String::from_utf8(STANDARD.decode(value).ok()?).ok()
    }
    fn write(&self, text: &str) {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let _ = Self::reg(&["add", Self::KEY, "/v", "Record", "/t", "REG_SZ", "/d", &STANDARD.encode(text), "/f"]);
    }
}

/// Linux has no system store that's always there; the two folders are used.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub struct SystemStore;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
impl Store for SystemStore {
    fn read(&self) -> Option<String> {
        None
    }
    fn write(&self, _: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edited_record_is_ignored() {
        let r = Record { first_seen: 100, latest_seen: 200, ..Default::default() };
        let sealed = seal(&r);
        assert_eq!(unseal(&sealed), Some(r));
        // Moving the trial start forward by editing the file: rejected.
        assert_eq!(unseal(&sealed.replace("\"firstSeen\":100", "\"firstSeen\":900")), None);
        assert_eq!(unseal("garbage"), None);
    }

    #[test]
    fn copies_merge_to_the_earliest_start() {
        let a = Record { first_seen: 500, latest_seen: 900, ..Default::default() };
        let b = Record { first_seen: 100, latest_seen: 300, licence: Some("k".into()), ..Default::default() };
        let m = Record::merge([a, b, Record::default()]).unwrap();
        assert_eq!((m.first_seen, m.latest_seen, m.licence.as_deref()), (100, 900, Some("k")));
    }
}
