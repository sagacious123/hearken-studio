//! The free trial and paid licences, checked on this computer with no internet needed.
//!
//! - **Trial:** 60 days from the first run. The start date is kept in several places,
//!   including the system's secure storage, so reinstalling doesn't restart it, and the
//!   earliest copy wins. Once online, the website registers this computer's anonymous ID and
//!   signs the trial dates, so a new install on the same computer gets the same dates.
//! - **Licence:** a key signed by the website, pasted in by the church. It carries its end
//!   date; there are 14 days' grace after it.
//! - **Clock:** the latest time ever seen is remembered. A clock more than two days behind it
//!   (or behind the service history) was turned back, and listening waits for one check
//!   against the website's clock.
//! - Only starting to listen is ever refused, never a service in progress. Search, the screen
//!   and phones keep working.

mod machine;
pub mod store;
pub mod token;

use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use parking_lot::Mutex;
use serde::Serialize;
use sha2::{Digest, Sha256};

use store::{FileStore, Record, Store, SystemStore};
use token::Pass;

pub const TRIAL_DAYS: i64 = 60;
pub const GRACE_DAYS: i64 = 14;
const DAY: i64 = 86_400;
/// Clocks drift and time zones change: only a clock this far behind counts as turned back.
const CLOCK_SLACK: i64 = 2 * DAY;
const SERVER: &str = "https://hearken-site.vercel.app/api";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Status {
    /// Times are Unix seconds.
    #[serde(rename_all = "camelCase")]
    Trial { ends: i64, days_left: i64, registered: bool },
    #[serde(rename_all = "camelCase")]
    TrialOver { ended: i64 },
    #[serde(rename_all = "camelCase")]
    Licensed { church: String, plan: String, expires: i64, in_grace: bool },
    #[serde(rename_all = "camelCase")]
    LicenceExpired { church: String, expired: i64 },
    /// The computer's clock reads earlier than it already has.
    #[serde(rename_all = "camelCase")]
    ClockBehind { now: i64, seen: i64 },
}

impl Status {
    pub fn can_listen(&self) -> bool {
        matches!(self, Status::Trial { .. } | Status::Licensed { .. })
    }

    /// Why listening can't start, for the operator.
    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Status::TrialOver { .. } => Some("Your free trial has ended. Enter a licence key to keep listening."),
            Status::LicenceExpired { .. } => Some("Your licence has ended. Enter your new licence key to keep listening."),
            Status::ClockBehind { .. } => Some("This computer's date looks wrong. Set the correct date and time, or connect to the internet and click Check now."),
            _ => None,
        }
    }
}

/// The rules, with everything they depend on passed in.
pub fn evaluate(record: &Record, now: i64, newest_activity: i64, machine: Option<&str>, verify: &dyn Fn(&str) -> Option<Pass>) -> Status {
    let activity = if newest_activity > record.activity_checked { newest_activity } else { 0 };
    let seen = record.latest_seen.max(activity);
    if now + CLOCK_SLACK < seen {
        return Status::ClockBehind { now, seen };
    }

    // The trial: the website's dates for this computer if registered, but never later than the
    // first run recorded here.
    let local_end = record.first_seen + TRIAL_DAYS * DAY;
    let registered = record.trial_pass.as_deref().and_then(verify).and_then(|p| match p {
        Pass::Trial { machine: m, expires, .. } if Some(m.as_str()) == machine => Some(expires),
        _ => None,
    });
    let trial_end = registered.map_or(local_end, |e| e.min(local_end));
    let trial = if now < trial_end {
        Status::Trial { ends: trial_end, days_left: (trial_end - now + DAY - 1) / DAY, registered: registered.is_some() }
    } else {
        Status::TrialOver { ended: trial_end }
    };

    match record.licence.as_deref().and_then(verify) {
        Some(Pass::Licence { church, plan, expires, .. }) => {
            if now <= expires + GRACE_DAYS * DAY {
                Status::Licensed { church, plan, expires, in_grace: now > expires }
            } else if trial.can_listen() {
                trial
            } else {
                Status::LicenceExpired { church, expired: expires }
            }
        }
        _ => trial,
    }
}

pub struct Licensing {
    record: Mutex<Record>,
    stores: Vec<Box<dyn Store>>,
    machine: Option<String>,
    history: PathBuf,
}

impl Licensing {
    /// Reads every copy of the record (starting the trial on a first run) and brings them all
    /// up to date.
    pub fn load(config_dir: &Path, second_dir: Option<&Path>, history: PathBuf) -> Self {
        let mut stores: Vec<Box<dyn Store>> = vec![Box::new(FileStore(config_dir.join("licence-record")))];
        if let Some(dir) = second_dir.filter(|d| *d != config_dir) {
            stores.push(Box::new(FileStore(dir.join("licence-record"))));
        }
        stores.push(Box::new(SystemStore));
        Self::with_stores(stores, machine::fingerprint(), history)
    }

    /// A fresh trial kept in a test folder, away from the system's secure storage.
    #[cfg(test)]
    pub fn for_tests(dir: &Path) -> Self {
        Self::with_stores(vec![Box::new(FileStore(dir.join("licence-record")))], None, dir.join("history"))
    }

    fn with_stores(stores: Vec<Box<dyn Store>>, machine: Option<String>, history: PathBuf) -> Self {
        let now = unix_now();
        let found = Record::merge(stores.iter().filter_map(|s| s.read()).filter_map(|t| store::unseal(&t)));
        let record = found.unwrap_or(Record { first_seen: now, latest_seen: now, ..Default::default() });
        let this = Self { record: Mutex::new(record), stores, machine, history };
        this.touch();
        this
    }

    pub fn status(&self) -> Status {
        evaluate(&self.record.lock(), unix_now(), newest_activity(&self.history), self.machine.as_deref(), &token::verify)
    }

    /// Remembers the latest time seen, and saves every copy.
    pub fn touch(&self) {
        let mut r = self.record.lock();
        r.latest_seen = r.latest_seen.max(unix_now());
        self.save(&r);
    }

    fn save(&self, record: &Record) {
        let sealed = store::seal(record);
        for s in &self.stores {
            s.write(&sealed);
        }
    }

    /// Saves a licence key if it's genuine.
    pub fn activate(&self, key: &str) -> Result<Status, String> {
        match token::verify(key) {
            Some(Pass::Licence { .. }) => {
                let key: String = key.chars().filter(|c| !c.is_whitespace()).collect();
                let mut r = self.record.lock();
                r.licence = Some(key);
                self.save(&r);
                drop(r);
                Ok(self.status())
            }
            _ => Err("That isn't a valid licence key. Copy the whole key from your email and paste it again.".into()),
        }
    }

    /// Registers the trial with the website if it isn't yet, and checks this computer's clock
    /// against the website's. Needs the internet; nothing else does.
    pub fn check_online(&self) -> Result<Status, String> {
        let nonce = nonce(self.machine.as_deref());
        let licensed = matches!(self.status(), Status::Licensed { .. });
        let unregistered = !licensed && !matches!(self.status(), Status::Trial { registered: true, .. } | Status::TrialOver { .. });
        let issued = match (&self.machine, unregistered) {
            (Some(machine), true) => {
                let first_seen = self.record.lock().first_seen;
                let body = serde_json::json!({ "machine": machine, "firstSeen": first_seen, "nonce": nonce, "version": env!("CARGO_PKG_VERSION") });
                let pass = fetch_pass(agent().post(&format!("{SERVER}/trial")).set("Content-Type", "application/json").send_string(&body.to_string()))?;
                match token::verify(&pass) {
                    Some(Pass::Trial { machine: m, nonce: n, issued, .. }) if &m == machine && n == nonce => {
                        self.record.lock().trial_pass = Some(pass);
                        issued
                    }
                    _ => return Err("The website's answer couldn't be verified.".into()),
                }
            }
            _ => {
                let pass = fetch_pass(agent().get(&format!("{SERVER}/time")).query("nonce", &nonce).call())?;
                match token::verify(&pass) {
                    Some(Pass::Time { issued, nonce: n }) if n == nonce => issued,
                    _ => return Err("The website's answer couldn't be verified.".into()),
                }
            }
        };

        let local = unix_now();
        let mut r = self.record.lock();
        if local + CLOCK_SLACK < issued {
            self.save(&r);
            return Err(format!(
                "This computer's clock is behind: it says {}, but it's {}. Set the correct date and time, then click Check now.",
                day(local),
                day(issued)
            ));
        }
        // The website vouches for the time: anything seen before now was a wrong clock, not tampering.
        r.latest_seen = local.max(issued);
        r.activity_checked = newest_activity(&self.history).max(r.activity_checked);
        self.save(&r);
        drop(r);
        Ok(self.status())
    }
}

fn fetch_pass(response: Result<ureq::Response, ureq::Error>) -> Result<String, String> {
    let response = response.map_err(|e| match e {
        ureq::Error::Status(code, _) => format!("The licence service answered {code}. Try again later."),
        ureq::Error::Transport(_) => "Couldn't reach the internet. Connect and try again.".into(),
    })?;
    let text = response.into_string().map_err(|_| "The website's answer couldn't be read.".to_string())?;
    let body: serde_json::Value = serde_json::from_str(&text).map_err(|_| "The website's answer couldn't be read.".to_string())?;
    body["pass"].as_str().map(str::to_owned).ok_or_else(|| "The website's answer couldn't be read.".into())
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).build()
}

/// Changes on each request so an old answer from the website can't be replayed.
fn nonce(machine: Option<&str>) -> String {
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    machine::hex(&Sha256::digest(format!("{t}/{}/{}", std::process::id(), machine.unwrap_or("")).as_bytes())[..16])
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// When the service history was last written: a clock behind that was turned back.
fn newest_activity(history: &Path) -> i64 {
    fs::read_dir(history)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok()?.metadata().ok()?.modified().ok())
        .filter_map(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .max()
        .unwrap_or(0)
}

fn day(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0).map(|d| d.format("%-d %B %Y").to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use token::tests::{sign, test_key, verify_test};

    const T0: i64 = 1_790_000_000;
    const M: &str = "machine-a";

    fn status(r: &Record, now: i64, activity: i64) -> Status {
        evaluate(r, now, activity, Some(M), &verify_test)
    }

    fn fresh() -> Record {
        Record { first_seen: T0, latest_seen: T0, ..Default::default() }
    }

    fn licence(expires: i64) -> String {
        sign(&Pass::Licence { id: "L1".into(), church: "Grace Chapel".into(), plan: "standard".into(), issued: T0, expires }, &test_key())
    }

    #[test]
    fn the_trial_lasts_sixty_days() {
        let r = fresh();
        assert_eq!(status(&r, T0, 0), Status::Trial { ends: T0 + 60 * DAY, days_left: 60, registered: false });
        assert!(matches!(status(&r, T0 + 59 * DAY + 3600, 0), Status::Trial { days_left: 1, .. }));
        assert_eq!(status(&r, T0 + 60 * DAY, 0), Status::TrialOver { ended: T0 + 60 * DAY });
        assert!(!status(&r, T0 + 60 * DAY, 0).can_listen());
    }

    #[test]
    fn the_websites_dates_count_but_never_extend_the_trial() {
        let mut r = fresh();
        let pass = |starts: i64| sign(&Pass::Trial { machine: M.into(), starts, expires: starts + 60 * DAY, issued: starts, nonce: "n".into() }, &test_key());
        // Reinstalled: this copy says day 0 is today, but the website registered it 30 days ago.
        r.first_seen = T0 + 30 * DAY;
        r.trial_pass = Some(pass(T0));
        assert!(matches!(status(&r, T0 + 40 * DAY, 0), Status::Trial { days_left: 20, registered: true, .. }));
        // A pass for another computer doesn't count.
        let other = Record { trial_pass: Some(sign(&Pass::Trial { machine: "machine-b".into(), starts: T0 + 50 * DAY, expires: T0 + 110 * DAY, issued: 0, nonce: "n".into() }, &test_key())), ..fresh() };
        assert!(matches!(status(&other, T0, 0), Status::Trial { registered: false, .. }));
        // A later registration doesn't push back the local start.
        let late = Record { trial_pass: Some(pass(T0 + 20 * DAY)), ..fresh() };
        assert!(matches!(status(&late, T0 + 61 * DAY, 0), Status::TrialOver { .. }));
    }

    #[test]
    fn turning_the_clock_back_is_caught() {
        let r = Record { latest_seen: T0 + 70 * DAY, ..fresh() };
        // Set back to day 10 to get the trial back: refused.
        assert_eq!(status(&r, T0 + 10 * DAY, 0), Status::ClockBehind { now: T0 + 10 * DAY, seen: T0 + 70 * DAY });
        // A day behind (time zones, a drifting clock): fine.
        assert!(matches!(status(&r, T0 + 69 * DAY, 0), Status::TrialOver { .. }));
        // History written "in the future" gives it away too, until checked online.
        let r = fresh();
        assert!(matches!(status(&r, T0 + 10 * DAY, T0 + 65 * DAY), Status::ClockBehind { .. }));
        let checked = Record { activity_checked: T0 + 65 * DAY, ..fresh() };
        assert!(matches!(status(&checked, T0 + 10 * DAY, T0 + 65 * DAY), Status::Trial { .. }));
    }

    #[test]
    fn a_licence_has_two_weeks_grace_then_ends() {
        let r = Record { licence: Some(licence(T0 + 365 * DAY)), ..fresh() };
        assert!(matches!(status(&r, T0 + 100 * DAY, 0), Status::Licensed { in_grace: false, .. }));
        assert!(matches!(status(&r, T0 + 370 * DAY, 0), Status::Licensed { in_grace: true, .. }));
        assert_eq!(status(&r, T0 + 380 * DAY, 0), Status::LicenceExpired { church: "Grace Chapel".into(), expired: T0 + 365 * DAY });
        // A short licence that ends during the trial leaves the trial running.
        let short = Record { licence: Some(licence(T0 + DAY)), ..fresh() };
        assert!(matches!(status(&short, T0 + 30 * DAY, 0), Status::Trial { .. }));
        // A forged licence is ignored.
        let forged = Record { licence: Some(licence(T0 + 999 * DAY).replace("HK1.", "HK1.x")), ..fresh() };
        assert!(matches!(status(&forged, T0 + 61 * DAY, 0), Status::TrialOver { .. }));
    }

    /// The real website: its signed clock reads back in the app and settles the date.
    #[test]
    #[ignore = "contacts the Hearken website"]
    fn checks_the_clock_with_the_website() {
        let dir = std::env::temp_dir().join(format!("sb-licence-live-{}", std::process::id()));
        let l = Licensing::for_tests(&dir);
        assert!(matches!(l.check_online(), Ok(Status::Trial { .. })), "{:?}", l.check_online());
        assert!((l.record.lock().latest_seen - unix_now()).abs() < 60);
    }

    struct Memory(Mutex<Option<String>>);
    impl Store for Memory {
        fn read(&self) -> Option<String> {
            self.0.lock().clone()
        }
        fn write(&self, t: &str) {
            *self.0.lock() = Some(t.into());
        }
    }
    impl Store for std::sync::Arc<Memory> {
        fn read(&self) -> Option<String> {
            self.as_ref().read()
        }
        fn write(&self, t: &str) {
            self.as_ref().write(t)
        }
    }

    #[test]
    fn reinstalling_keeps_the_original_start() {
        use std::sync::Arc;
        let (settings, system) = (Arc::new(Memory(Mutex::new(None))), Arc::new(Memory(Mutex::new(None))));
        let original = Record { first_seen: T0, latest_seen: T0, ..Default::default() };
        system.write(&store::seal(&original));
        // Reinstalled: the settings folder is gone, the system store isn't.
        let l = Licensing::with_stores(vec![Box::new(settings.clone()), Box::new(system.clone())], Some(M.into()), PathBuf::from("/nonexistent"));
        assert_eq!(l.record.lock().first_seen, T0);
        // And the settings folder gets the original back.
        assert_eq!(store::unseal(&settings.read().unwrap()).unwrap().first_seen, T0);
        // An edited copy claiming a later start is ignored.
        settings.write(&store::seal(&original).replace(&T0.to_string(), &(T0 + 50 * DAY).to_string()));
        let l = Licensing::with_stores(vec![Box::new(settings), Box::new(system)], Some(M.into()), PathBuf::from("/nonexistent"));
        assert_eq!(l.record.lock().first_seen, T0);
    }
}
