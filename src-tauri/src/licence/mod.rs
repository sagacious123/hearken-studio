//! Signing in, the free trial and paid licences. Everything is checked on this computer, so
//! services need no internet; the website is only needed to sign in, and once every 30 days.
//!
//! - **Account:** listening needs a signed-in account. Signing in gives a pass signed by the
//!   website for this computer, valid for 30 days and renewed whenever the app is online.
//! - **Trial:** 60 days from the first run. The start date is kept in several places,
//!   including the system's secure storage, so reinstalling doesn't restart it, and the
//!   earliest copy wins. Once online, the website registers this computer's anonymous ID and
//!   signs the trial dates, so a new install on the same computer gets the same dates.
//! - **Licence:** a key signed by the website, attached to the account (fetched on sign-in) or
//!   pasted in. It carries its end date; there are 14 days' grace after it.
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
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use store::{FileStore, Record, Store, SystemStore};
use token::Pass;

pub const TRIAL_DAYS: i64 = 60;
pub const GRACE_DAYS: i64 = 14;
const DAY: i64 = 86_400;
/// Clocks drift and time zones change: only a clock this far behind counts as turned back.
const CLOCK_SLACK: i64 = 2 * DAY;
/// Renew the sign-in pass when online if it's older than this.
const RENEW_AFTER: i64 = DAY;
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

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Account {
    /// `until`: when the app must next reach the website (Unix seconds).
    #[serde(rename_all = "camelCase")]
    SignedIn { email: String, church: String, until: i64 },
    SignedOut,
    /// Offline for more than 30 days: renews by itself once online.
    #[serde(rename_all = "camelCase")]
    Expired { email: String },
}

/// One of the account's computers, as the website lists them.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Computer {
    pub id: String,
    pub name: String,
    pub last: i64,
}

/// What signing in came to: signed in, or the account is on its most computers and one of them
/// must be signed out first (sign in again with its id as `replace`).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum SignIn {
    SignedIn { info: Info },
    #[serde(rename_all = "camelCase")]
    TooManyComputers { limit: u32, computers: Vec<Computer>, message: String },
}

/// What creating the account came to: a code was emailed to confirm the address (then
/// `confirm_sign_up`), or, from a website that doesn't confirm emails, signed in straight away.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum SignUp {
    Verify { email: String },
    SignedIn { info: Info },
}

/// Everything the dashboard shows about the account, trial and licence.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    pub status: Status,
    pub account: Account,
    pub can_listen: bool,
}

impl Info {
    /// Why listening can't start: "SIGNIN" or "LICENCE", and what to tell the operator.
    pub fn blocked(&self) -> Option<(&'static str, &'static str)> {
        if let Status::ClockBehind { .. } = self.status {
            return self.status.reason().map(|r| ("LICENCE", r));
        }
        match self.account {
            Account::SignedOut => return Some(("SIGNIN", "Sign in to start listening.")),
            Account::Expired { .. } => return Some(("SIGNIN", "Connect to the internet so Hearken can renew your sign-in, or sign in again.")),
            Account::SignedIn { .. } => {}
        }
        self.status.reason().map(|r| ("LICENCE", r))
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

/// Signed in on this computer, from the website's signed pass.
pub fn account(record: &Record, now: i64, machine: &str, verify: &dyn Fn(&str) -> Option<Pass>) -> Account {
    match record.account_pass.as_deref().and_then(verify) {
        Some(Pass::Account { email, church, machine: m, expires, .. }) if m == machine => {
            if now <= expires {
                Account::SignedIn { email, church, until: expires }
            } else {
                Account::Expired { email }
            }
        }
        _ => Account::SignedOut,
    }
}

pub struct Licensing {
    record: Mutex<Record>,
    stores: Vec<Box<dyn Store>>,
    /// The computer's anonymous ID (or this install's, where the system won't give one).
    machine: String,
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
        let mut record = found.unwrap_or(Record { first_seen: now, latest_seen: now, ..Default::default() });
        let machine = machine.unwrap_or_else(|| record.install_id.get_or_insert_with(|| random_hex(32)).clone());
        let this = Self { record: Mutex::new(record), stores, machine, history };
        this.touch();
        this
    }

    pub fn status(&self) -> Status {
        evaluate(&self.record.lock(), unix_now(), newest_activity(&self.history), Some(&self.machine), &token::verify)
    }

    pub fn info(&self) -> Info {
        let status = self.status();
        let account = account(&self.record.lock(), unix_now(), &self.machine, &token::verify);
        let can_listen = status.can_listen() && matches!(account, Account::SignedIn { .. });
        Info { status, account, can_listen }
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
    pub fn activate(&self, key: &str) -> Result<Info, String> {
        match token::verify(key) {
            Some(Pass::Licence { .. }) => {
                let key: String = key.chars().filter(|c| !c.is_whitespace()).collect();
                let mut r = self.record.lock();
                r.licence = Some(key);
                self.save(&r);
                drop(r);
                Ok(self.info())
            }
            _ => Err("That isn't a valid licence key. Copy the whole key from your email and paste it again.".into()),
        }
    }

    /// Signs this computer in. `replace` signs out another of the account's computers to make
    /// room, after the website said the account is on its most computers.
    pub fn sign_in(&self, email: &str, password: &str, replace: Option<&str>) -> Result<SignIn, String> {
        let body = json!({ "email": email, "password": password, "machine": self.machine, "name": computer_name(), "replace": replace });
        match post("auth/login", body) {
            Ok(reply) => self.accept(reply).map(|info| SignIn::SignedIn { info }),
            Err(Failure { status: Some(409), message, body }) if body["code"] == "computer_limit" => Ok(SignIn::TooManyComputers {
                limit: body["limit"].as_u64().unwrap_or(3) as u32,
                computers: serde_json::from_value(body["computers"].clone()).unwrap_or_default(),
                message,
            }),
            Err(f) => Err(f.message),
        }
    }

    /// Starts creating the church's account: the website emails a code to the address.
    pub fn sign_up(&self, church: &str, email: &str, password: &str) -> Result<SignUp, String> {
        let body = json!({ "church": church, "email": email, "password": password, "verify": true, "machine": self.machine, "name": computer_name() });
        let reply = post("auth/signup", body).map_err(|f| f.message)?;
        if reply["verify"] == true {
            return Ok(SignUp::Verify { email: reply["email"].as_str().unwrap_or(email).to_owned() });
        }
        self.accept(reply).map(|info| SignUp::SignedIn { info })
    }

    /// Finishes creating the account with the emailed code, and signs this computer in.
    pub fn confirm_sign_up(&self, email: &str, code: &str) -> Result<Info, String> {
        let reply = post("auth/signup", json!({ "email": email, "code": code, "machine": self.machine, "name": computer_name() })).map_err(|f| f.message)?;
        self.accept(reply)
    }

    /// Asks for a password-reset email.
    pub fn forgot(&self, email: &str) -> Result<(), String> {
        post("auth/forgot", json!({ "email": email })).map(|_| ()).map_err(|f| f.message)
    }

    /// Signs out of this computer (and ends the session on the website when online).
    pub fn sign_out(&self) -> Info {
        let session = self.record.lock().session.take();
        if let Some(session) = session {
            let _ = post("auth/logout", json!({ "session": session }));
        }
        let mut r = self.record.lock();
        r.account_pass = None;
        self.save(&r);
        drop(r);
        self.info()
    }

    /// Signed in, with a pass more than a day old (or past its 30 days).
    pub fn needs_renewal(&self) -> bool {
        let r = self.record.lock();
        let issued = r.account_pass.as_deref().and_then(token::verify).and_then(|p| match p {
            Pass::Account { issued, .. } => Some(issued),
            _ => None,
        });
        r.session.is_some() && issued.map_or(true, |i| unix_now() - i > RENEW_AFTER)
    }

    /// Renews the sign-in pass and fetches the account's licence. A session the website has
    /// ended (password reset, signed out elsewhere) signs this computer out.
    pub fn renew(&self) -> Result<Info, String> {
        let Some(session) = self.record.lock().session.clone() else { return Ok(self.info()) };
        match post("auth/refresh", json!({ "session": session, "machine": self.machine, "name": computer_name() })) {
            Ok(reply) => self.accept(reply),
            Err(Failure { status: Some(401), message, .. }) => {
                let mut r = self.record.lock();
                r.session = None;
                r.account_pass = None;
                self.save(&r);
                Err(message)
            }
            Err(f) => Err(f.message),
        }
    }

    /// Stores what the website sent on sign-in or renewal.
    fn accept(&self, reply: Value) -> Result<Info, String> {
        let pass = reply["pass"].as_str().unwrap_or_default().to_owned();
        let issued = match token::verify(&pass) {
            Some(Pass::Account { machine, issued, .. }) if machine == self.machine => issued,
            _ => return Err("The website's answer couldn't be verified.".into()),
        };
        let mut r = self.record.lock();
        if let Some(session) = reply["session"].as_str() {
            r.session = Some(session.to_owned());
        }
        r.account_pass = Some(pass);
        if let Some(licence) = reply["licence"].as_str().filter(|k| matches!(token::verify(k), Some(Pass::Licence { .. }))) {
            r.licence = Some(licence.to_owned());
        }
        drop(r);
        self.trust_clock(issued)?;
        Ok(self.info())
    }

    /// Registers the trial with the website if it isn't yet, renews the sign-in, and checks this
    /// computer's clock against the website's.
    pub fn check_online(&self) -> Result<Info, String> {
        let nonce = random_hex(16);
        let status = self.status();
        let unregistered = !matches!(status, Status::Licensed { .. } | Status::Trial { registered: true, .. } | Status::TrialOver { .. });
        let issued = if unregistered {
            let first_seen = self.record.lock().first_seen;
            let body = json!({ "machine": self.machine, "firstSeen": first_seen, "nonce": nonce, "version": env!("CARGO_PKG_VERSION") });
            let pass = post("trial", body).map_err(|f| f.message)?["pass"].as_str().unwrap_or_default().to_owned();
            match token::verify(&pass) {
                Some(Pass::Trial { machine: m, nonce: n, issued, .. }) if m == self.machine && n == nonce => {
                    self.record.lock().trial_pass = Some(pass);
                    issued
                }
                _ => return Err("The website's answer couldn't be verified.".into()),
            }
        } else {
            let reply = call(agent().get(&format!("{}/time", server())).query("nonce", &nonce).call()).map_err(|f| f.message)?;
            match token::verify(reply["pass"].as_str().unwrap_or_default()) {
                Some(Pass::Time { issued, nonce: n }) if n == nonce => issued,
                _ => return Err("The website's answer couldn't be verified.".into()),
            }
        };
        self.trust_clock(issued)?;
        if self.record.lock().session.is_some() {
            self.renew()?;
        }
        Ok(self.info())
    }

    /// The website vouched for the time `issued`: a clock that agrees clears any doubt about
    /// the date, and one far behind is reported.
    fn trust_clock(&self, issued: i64) -> Result<(), String> {
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
        r.latest_seen = local.max(issued);
        r.activity_checked = newest_activity(&self.history).max(r.activity_checked);
        self.save(&r);
        Ok(())
    }
}

/// The website's API: the live site, or another (a local test server) named by HEARKEN_API.
fn server() -> String {
    std::env::var("HEARKEN_API").unwrap_or_else(|_| SERVER.into())
}

struct Failure {
    status: Option<u16>,
    message: String,
    /// The website's whole answer, for errors that carry more than a message.
    body: Value,
}

fn post(path: &str, body: Value) -> Result<Value, Failure> {
    call(agent().post(&format!("{}/{path}", server())).set("Content-Type", "application/json").send_string(&body.to_string()))
}

/// The JSON answer, or the website's own explanation of what went wrong.
fn call(response: Result<ureq::Response, ureq::Error>) -> Result<Value, Failure> {
    let unreadable = || Failure { status: None, message: "The website's answer couldn't be read.".into(), body: Value::Null };
    match response {
        Ok(r) => serde_json::from_str(&r.into_string().map_err(|_| unreadable())?).map_err(|_| unreadable()),
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).unwrap_or(Value::Null);
            let message = body["error"].as_str().map(str::to_owned).unwrap_or_else(|| format!("The Hearken website answered {code}. Try again later."));
            Err(Failure { status: Some(code), message, body })
        }
        Err(ureq::Error::Transport(_)) => Err(Failure { status: None, message: "Couldn't reach the internet. Connect and try again.".into(), body: Value::Null }),
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(15)).build()
}

/// What the account page calls this computer: its network name, such as "Media-Desk-MacBook".
fn computer_name() -> String {
    sysinfo::System::host_name().unwrap_or_default().trim_end_matches(".local").chars().take(60).collect()
}

/// Unpredictable enough for a nonce or an install ID.
fn random_hex(bytes: usize) -> String {
    let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let seed = format!("{t}/{}/{:p}", std::process::id(), &t);
    machine::hex(&Sha256::digest(seed.as_bytes())[..bytes.min(32)])
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

    fn signed_in(machine: &str, issued: i64) -> Record {
        let pass = sign(&Pass::Account { user: "u1".into(), email: "media@grace.org".into(), church: "Grace Chapel".into(), machine: machine.into(), issued, expires: issued + 30 * DAY }, &test_key());
        Record { account_pass: Some(pass), session: Some("s".into()), ..fresh() }
    }

    fn info(r: &Record, now: i64) -> Info {
        let status = status(r, now, 0);
        let account = account(r, now, M, &verify_test);
        let can_listen = status.can_listen() && matches!(account, Account::SignedIn { .. });
        Info { status, account, can_listen }
    }

    #[test]
    fn listening_needs_a_signed_in_account() {
        // Signed out: the trial is running, but listening waits for sign-in.
        let out = info(&fresh(), T0);
        assert_eq!(out.account, Account::SignedOut);
        assert_eq!(out.blocked().map(|b| b.0), Some("SIGNIN"));
        // Signed in: allowed for 30 days offline.
        let r = signed_in(M, T0);
        assert!(info(&r, T0 + 29 * DAY).can_listen);
        assert!(info(&r, T0 + 29 * DAY).blocked().is_none());
        // Past 30 days offline: renew first.
        let late = info(&r, T0 + 31 * DAY);
        assert_eq!(late.account, Account::Expired { email: "media@grace.org".into() });
        assert_eq!(late.blocked().map(|b| b.0), Some("SIGNIN"));
        // A pass copied from another computer doesn't sign this one in.
        assert_eq!(info(&signed_in("machine-b", T0), T0).account, Account::SignedOut);
        // Signed in but the trial is over: it's the licence that's needed.
        assert_eq!(info(&signed_in(M, T0 + 61 * DAY), T0 + 61 * DAY).blocked().map(|b| b.0), Some("LICENCE"));
    }

    /// Against the website's API run locally (`node scripts/local-api.mjs` in the site repo):
    /// `HEARKEN_API=http://localhost:4400/api cargo test signs_up -- --ignored`
    #[test]
    #[ignore = "needs the site's local API and HEARKEN_API"]
    fn signs_up_signs_in_renews_and_signs_out() {
        assert!(std::env::var("HEARKEN_API").is_ok(), "set HEARKEN_API");
        let dir = std::env::temp_dir().join(format!("sb-account-{}", random_hex(4)));
        let l = Licensing::for_tests(&dir);
        assert_eq!(l.info().account, Account::SignedOut);
        assert_eq!(l.info().blocked().map(|b| b.0), Some("SIGNIN"));

        let email = format!("e2e-{}@example.com", random_hex(4));
        let SignUp::Verify { email: to } = l.sign_up("Test Church", &email, "a good password").unwrap() else { panic!("expected a code") };
        assert_eq!(to, email);
        assert!(l.confirm_sign_up(&email, "000000x").is_err(), "a wrong code");
        let info = l.confirm_sign_up(&email, &emailed_code(&email)).unwrap();
        assert!(info.can_listen, "{info:?}");
        assert!(matches!(&info.account, Account::SignedIn { church, .. } if church == "Test Church"));
        assert!(l.sign_up("Test Church", &email, "a good password").unwrap_err().contains("already"));

        assert_eq!(l.sign_out().account, Account::SignedOut);
        assert!(l.sign_in(&email, "not the password", None).unwrap_err().contains("Wrong email or password"));
        assert!(matches!(l.sign_in(&email.to_uppercase(), "a good password", None).unwrap(), SignIn::SignedIn { info } if info.can_listen));
        assert!(matches!(l.renew().unwrap().account, Account::SignedIn { .. }));

        // Online, the trial is registered too.
        assert!(matches!(l.check_online().unwrap().status, Status::Trial { registered: true, .. }));

        // Signed out on the website (here: the session forgotten), this computer is signed out at renewal.
        let session = l.record.lock().session.clone().unwrap();
        post("auth/logout", json!({ "session": session })).ok();
        assert!(l.renew().is_err());
        assert_eq!(l.info().account, Account::SignedOut);
    }

    /// The code the local API would have emailed (it keeps the last email to each address).
    fn emailed_code(to: &str) -> String {
        let mail: Value = serde_json::from_str(&agent().get(&format!("{}/dev/mail?to={to}", server())).call().unwrap().into_string().unwrap()).unwrap();
        mail["data"]["code"].as_str().expect("a code was emailed").to_owned()
    }

    /// Against the local API too: a fourth computer is refused, and can sign one of the others out.
    #[test]
    #[ignore = "needs the site's local API and HEARKEN_API"]
    fn a_fourth_computer_signs_one_out() {
        assert!(std::env::var("HEARKEN_API").is_ok(), "set HEARKEN_API");
        let computer = |n: u32| Licensing::for_tests(&std::env::temp_dir().join(format!("sb-limit-{}-{n}", random_hex(4))));
        let email = format!("e2e-{}@example.com", random_hex(4));
        let first = computer(1);
        first.sign_up("Test Church", &email, "a good password").unwrap();
        first.confirm_sign_up(&email, &emailed_code(&email)).unwrap();
        for n in 2..=3 {
            assert!(matches!(computer(n).sign_in(&email, "a good password", None).unwrap(), SignIn::SignedIn { .. }));
        }
        let fourth = computer(4);
        let SignIn::TooManyComputers { limit, computers, .. } = fourth.sign_in(&email, "a good password", None).unwrap() else { panic!("expected the limit") };
        assert_eq!((limit, computers.len()), (3, 3));
        let oldest = computers.iter().min_by_key(|c| c.last).unwrap().id.clone();
        assert!(matches!(fourth.sign_in(&email, "a good password", Some(&oldest)).unwrap(), SignIn::SignedIn { .. }));
        // The computer signed out finds out at its next renewal.
        assert!(first.renew().is_err());
        assert_eq!(first.info().account, Account::SignedOut);
    }

    /// The real website: its signed clock reads back in the app and settles the date.
    #[test]
    #[ignore = "contacts the Hearken website"]
    fn checks_the_clock_with_the_website() {
        let dir = std::env::temp_dir().join(format!("sb-licence-live-{}", std::process::id()));
        let l = Licensing::for_tests(&dir);
        let info = l.check_online().map_err(|e| e.to_string());
        assert!(matches!(info, Ok(Info { status: Status::Trial { .. }, .. })), "{info:?}");
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
