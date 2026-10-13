//! Local-network sharing, no internet needed:
//!   /         congregation phones follow the live verse, in any translation and text size
//!   /recap    the service so far (scriptures, songs, sermon notes) to read or download as
//!             PDF or Word
//!   /overlay  transparent lower third for OBS / vMix browser sources (livestream)
//!   /remote   drive the output from a phone (needs the PIN shown in the dashboard)
//! Plain HTTP on the church network, served from a small blocking server thread, on port 80
//! when the system allows (so addresses need no ":7878") and under a friendly name announced
//! with mDNS: http://hearken.local. The IP address stays available as a fallback.

use std::{
    collections::HashMap,
    hash::{BuildHasher, Hasher},
    net::{IpAddr, Ipv4Addr, UdpSocket},
    path::PathBuf,
    sync::Arc,
    thread::JoinHandle,
};

use anyhow::{anyhow, Result};
use parking_lot::Mutex;
use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Manager};
use tiny_http::{Header, Method, Request, Response, Server};

mod announce;

pub use announce::clean_name;
use announce::{Announcement, NameStatus};

use crate::{
    commands,
    presentation::QrOverlay,
    recap::{self, Format},
    scripture::{BibleDb, Slide},
    state::AppState,
};

const PHONE_HTML: &str = include_str!("share/phone.html");
const OVERLAY_HTML: &str = include_str!("share/overlay.html");
const REMOTE_HTML: &str = include_str!("share/remote.html");

/// Tried first: browsers don't show the default port, so addresses read "http://hearken.local".
const FRIENDLY_PORT: u16 = 80;

struct Running {
    server: Arc<Server>,
    thread: Option<JoinHandle<()>>,
    /// Port actually serving, and the one asked for (the fallback when 80 is unavailable).
    port: u16,
    preferred_port: u16,
    name: String,
    ip: Ipv4Addr,
    /// Friendly-name announcement; dropped (and the name withdrawn) with the server.
    announce: Option<Announcement>,
    announce_error: Option<String>,
}

pub struct Share {
    running: Mutex<Option<Running>>,
    /// Why the server isn't running when it should be (e.g. port in use).
    error: Mutex<Option<String>>,
    pin: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareInfo {
    pub enabled: bool,
    pub port: u16,
    /// Friendly addresses: http://hearken.local, …/overlay, …/remote.
    pub phone_url: String,
    pub overlay_url: String,
    pub remote_url: String,
    /// The same pages by IP address, for devices that can't resolve ".local" names.
    pub ip_url: String,
    pub name: String,
    /// "checking" | "announced" | "conflict" | "failed"
    pub name_status: &'static str,
    pub pin: String,
    /// QR code for the phone page. Encodes the IP address, which every phone can open.
    pub qr_svg: String,
    pub error: Option<String>,
}

/// "http://hearken.local" or "http://hearken.local:7878" (and the same for an IP address).
fn base_url(host: &str, port: u16) -> String {
    if port == 80 {
        format!("http://{host}")
    } else {
        format!("http://{host}:{port}")
    }
}

impl Default for Share {
    fn default() -> Self {
        Self::new()
    }
}

impl Share {
    pub fn new() -> Self {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        Self { running: Mutex::new(None), error: Mutex::new(None), pin: format!("{:04}", h.finish() % 10_000) }
    }

    /// Start (or restart with a new name or port). Failures are kept for `info()` as well as
    /// returned.
    pub fn start(&self, app: AppHandle, preferred_port: u16, name: &str, bible_path: PathBuf) -> Result<()> {
        let result = self.try_start(app, preferred_port, &clean_name(name), bible_path);
        *self.error.lock() = result.as_ref().err().map(|e| format!("{e:#}"));
        result
    }

    fn try_start(&self, app: AppHandle, preferred_port: u16, name: &str, bible_path: PathBuf) -> Result<()> {
        let mut running = self.running.lock();
        if running.as_ref().is_some_and(|r| r.preferred_port == preferred_port && r.name == name) {
            return Ok(());
        }
        if let Some(r) = running.take() {
            stop(r);
        }
        // Port 80 first (no ":port" in addresses). macOS and Windows allow it without admin
        // rights; Linux usually doesn't, and something else may be using it.
        let (server, port) = match Server::http(("0.0.0.0", FRIENDLY_PORT)) {
            Ok(s) => (s, FRIENDLY_PORT),
            Err(_) => (
                Server::http(("0.0.0.0", preferred_port))
                    .map_err(|e| anyhow!("couldn't start sharing on port {preferred_port} ({e}). Is another app using it?"))?,
                preferred_port,
            ),
        };
        let server = Arc::new(server);
        let ip = local_ipv4();
        let (announce, announce_error) = match Announcement::start(name, ip, port) {
            Ok(a) => (Some(a), None),
            Err(e) => {
                log::warn!("couldn't announce {name}.local: {e:#}");
                (None, Some(format!("{e:#}")))
            }
        };
        let pin = self.pin.clone();
        let srv = server.clone();
        let thread = std::thread::Builder::new().name("share".into()).spawn(move || {
            // Own read-only connection: phones never wait on the operator's.
            let bible = BibleDb::open(&bible_path).ok();
            for req in srv.incoming_requests() {
                handle(&app, bible.as_ref(), &pin, req);
            }
        })?;
        log::info!("sharing at {} ({})", base_url(&format!("{name}.local"), port), base_url(&ip.to_string(), port));
        *running = Some(Running { server, thread: Some(thread), port, preferred_port, name: name.to_owned(), ip, announce, announce_error });
        Ok(())
    }

    pub fn stop(&self) {
        *self.error.lock() = None;
        if let Some(r) = self.running.lock().take() {
            stop(r);
        }
    }

    /// Addresses for the Share panel. When not running, shows what they will be.
    pub fn info(&self, preferred_port: u16, name: &str) -> ShareInfo {
        let running = self.running.lock();
        let (enabled, port, name, ip, name_status) = match running.as_ref() {
            Some(r) => {
                let status = match &r.announce {
                    Some(a) => match a.status() {
                        NameStatus::Checking => "checking",
                        NameStatus::Announced => "announced",
                        NameStatus::Conflict => "conflict",
                        NameStatus::Failed => "failed",
                    },
                    None => "failed",
                };
                (true, r.port, r.name.clone(), r.ip, status)
            }
            None => (false, preferred_port, clean_name(name), local_ipv4(), "checking"),
        };
        let friendly = base_url(&format!("{name}.local"), port);
        let ip_url = base_url(&ip.to_string(), port);
        let error = self.error.lock().clone().or_else(|| running.as_ref().and_then(|r| r.announce_error.clone()));
        ShareInfo {
            enabled,
            port,
            qr_svg: qr_svg(&format!("{ip_url}/")),
            overlay_url: format!("{friendly}/overlay"),
            remote_url: format!("{friendly}/remote"),
            phone_url: friendly,
            ip_url,
            name,
            name_status,
            pin: self.pin.clone(),
            error,
        }
    }

    /// The projector's "follow on your phone" box: the QR opens the IP address (works on every
    /// phone); the text shows the friendly address for anyone typing it.
    pub fn qr_overlay(&self, preferred_port: u16, name: &str) -> QrOverlay {
        let info = self.info(preferred_port, name);
        let url = if matches!(info.name_status, "conflict" | "failed") { info.ip_url } else { info.phone_url };
        QrOverlay { svg: info.qr_svg, url }
    }
}

fn stop(mut r: Running) {
    r.server.unblock();
    if let Some(t) = r.thread.take() {
        let _ = t.join();
    }
}

/// The address phones on the same network can reach. Connecting a UDP socket sends nothing;
/// it only asks the OS which interface routes to the LAN.
fn local_ipv4() -> Ipv4Addr {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("10.255.255.255:1").map(|_| s))
        .and_then(|s| s.local_addr())
        .ok()
        .and_then(|a| match a.ip() {
            IpAddr::V4(v4) if !v4.is_unspecified() => Some(v4),
            _ => None,
        })
        .unwrap_or(Ipv4Addr::LOCALHOST)
}

fn qr_svg(text: &str) -> String {
    use qrcode::render::svg;
    qrcode::QrCode::new(text.as_bytes())
        .map(|code| {
            code.render::<svg::Color>()
                .min_dimensions(240, 240)
                .dark_color(svg::Color("#000000"))
                .light_color(svg::Color("#ffffff"))
                .build()
        })
        .unwrap_or_default()
}

fn query(url: &str) -> (String, HashMap<String, String>) {
    let (path, q) = url.split_once('?').unwrap_or((url, ""));
    let params = q
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (decode(k), decode(v)))
        .collect();
    (path.to_owned(), params)
}

pub(crate) fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < b.len() => {
                match u8::from_str_radix(std::str::from_utf8(&b[i + 1..i + 3]).unwrap_or("x"), 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 2;
                    }
                    Err(_) => out.push(b'%'),
                }
            }
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn respond(req: Request, status: u16, content_type: &str, body: String) {
    let resp = Response::from_string(body)
        .with_status_code(status)
        .with_header(Header::from_bytes("Content-Type", content_type).expect("header"))
        .with_header(Header::from_bytes("Cache-Control", "no-store").expect("header"));
    let _ = req.respond(resp);
}

fn respond_file(req: Request, content_type: &str, file_name: &str, body: Vec<u8>) {
    // File names are ASCII ("Sermon recap 2026-10-11.pdf"); quotes are the only risk.
    let disposition = format!("attachment; filename=\"{}\"", file_name.replace('"', ""));
    let resp = Response::from_data(body)
        .with_header(Header::from_bytes("Content-Type", content_type).expect("header"))
        .with_header(Header::from_bytes("Content-Disposition", disposition.as_bytes()).expect("header"))
        .with_header(Header::from_bytes("Cache-Control", "no-store").expect("header"));
    let _ = req.respond(resp);
}

/// The latest service's recap for phones, as the operator allows: `None` when switched off.
fn phone_recap(state: &AppState, bible: Option<&BibleDb>, translation: Option<&str>) -> Option<(String, recap::Document)> {
    let (mode, church) = {
        let s = state.settings.lock();
        (s.phone_recap.clone(), s.church_name.clone())
    };
    if mode == "off" {
        return None;
    }
    let date = state.history.days().into_iter().next().unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
    let (mut r, notes) = state.history.recap_with_notes(&date);
    if let (Some(t), Some(db)) = (translation.filter(|t| !t.is_empty()), bible) {
        recap::retranslate(&mut r, db, t);
    }
    let notes = (mode == "notes").then_some(&notes);
    Some((date, recap::document(&r, notes, &church)))
}

fn recap_toolbar(translation: Option<&str>) -> String {
    let q = translation.filter(|t| !t.is_empty()).map(|t| format!("?t={}", t.chars().filter(char::is_ascii_alphanumeric).collect::<String>())).unwrap_or_default();
    format!(
        r#"<nav class="bar"><a class="back" href="/">← Live</a><span class="dl">Download</span><a href="/recap.pdf{q}" download>PDF</a><a href="/recap.docx{q}" download>Word</a><a href="/recap.doc{q}" download>.doc</a></nav>"#
    )
}

const RECAP_OFF: &str = r#"<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Recap</title>
<style>body{font:17px/1.5 system-ui,sans-serif;margin:0;padding:30vh 24px;text-align:center;color:#78716c}a{color:inherit}</style>
<p>The recap isn't shared on this network.</p><p><a href="/">← Back to the live verse</a></p>"#;

fn handle(app: &AppHandle, bible: Option<&BibleDb>, pin: &str, req: Request) {
    let (path, q) = query(req.url());
    let state = app.state::<AppState>();
    let html = "text/html; charset=utf-8";
    let js = "application/json";
    match (req.method(), path.as_str()) {
        (Method::Get, "/") => respond(req, 200, html, PHONE_HTML.into()),
        (Method::Get, "/overlay") => respond(req, 200, html, OVERLAY_HTML.into()),
        (Method::Get, "/remote") => respond(req, 200, html, REMOTE_HTML.into()),
        (Method::Get, "/recap") => match phone_recap(&state, bible, q.get("t").map(String::as_str)) {
            Some((_, doc)) => respond(req, 200, html, recap::phone_page(&doc, &recap_toolbar(q.get("t").map(String::as_str)))),
            None => respond(req, 403, html, RECAP_OFF.into()),
        },
        (Method::Get, p) if p.starts_with("/recap.") => {
            let Some(format) = Format::from_ext(&p["/recap.".len()..]).filter(|f| *f != Format::Html) else {
                return respond(req, 404, "text/plain", "Not found".into());
            };
            match phone_recap(&state, bible, q.get("t").map(String::as_str)) {
                Some((date, doc)) => match recap::render(&doc, format) {
                    Ok(bytes) => respond_file(req, format.mime(), &recap::file_name(&date, format), bytes),
                    Err(e) => respond(req, 500, "text/plain", format!("Couldn't make the recap: {e:#}")),
                },
                None => respond(req, 403, html, RECAP_OFF.into()),
            }
        }
        (Method::Get, "/api/state") => {
            let body = live_json(&state, bible, q.get("t").map(String::as_str));
            respond(req, 200, js, body.to_string())
        }
        (Method::Get, "/api/translations") => {
            let list: Vec<_> = bible.map(|b| b.translations().iter().map(|t| json!({ "code": t.code, "name": t.name })).collect()).unwrap_or_default();
            respond(req, 200, js, json!(list).to_string())
        }
        (_, p) if p.starts_with("/api/remote/") => {
            if q.get("pin").map(String::as_str) != Some(pin) {
                return respond(req, 403, js, json!({ "error": "wrong PIN" }).to_string());
            }
            let result = remote(app, &state, &p["/api/remote/".len()..], &q);
            match result {
                Ok(body) => respond(req, 200, js, body.to_string()),
                Err(e) => respond(req, 400, js, json!({ "error": e }).to_string()),
            }
        }
        _ => respond(req, 404, "text/plain", "Not found".into()),
    }
}

/// The live slide for phones and the overlay, optionally re-rendered in another translation.
fn live_json(state: &AppState, bible: Option<&BibleDb>, translation: Option<&str>) -> serde_json::Value {
    let live = state.presenter.snapshot();
    let Some(slide) = live.slide.filter(|_| !live.blackout) else {
        return json!({ "revision": live.revision, "slide": null, "recap": recap_shared(state) });
    };
    let (code, verses) = match (translation, bible) {
        (Some(t), Some(db)) if !slide.is_song() && !t.eq_ignore_ascii_case(&slide.translation) => {
            let r = &slide.reference;
            let s = r.verse_start.unwrap_or(1);
            match db.passage(t, r.book_id, r.chapter, s, r.verse_end.unwrap_or(s)) {
                Ok(v) if !v.is_empty() => (t.to_uppercase(), v),
                _ => (slide.translation.clone(), slide.verses.clone()),
            }
        }
        _ => (slide.translation.clone(), slide.verses.clone()),
    };
    json!({
        "revision": live.revision,
        "recap": recap_shared(state),
        "slide": {
            "id": slide.id,
            "label": slide.label,
            "translation": code,
            "song": slide.is_song(),
            "verses": verses,
        }
    })
}

fn recap_shared(state: &AppState) -> bool {
    state.settings.lock().phone_recap != "off"
}

fn item(slide: &Slide, kind: &str) -> serde_json::Value {
    json!({ "id": slide.id, "label": slide.label, "kind": kind, "text": slide.verses.first().map(|v| v.text.as_str()).unwrap_or("") })
}

fn remote(app: &AppHandle, state: &AppState, action: &str, q: &HashMap<String, String>) -> Result<serde_json::Value, String> {
    match action {
        "state" => {
            let live = state.presenter.snapshot();
            let mut items: Vec<_> = state.recent.lock().iter().map(|s| item(s, "detected")).collect();
            items.extend(commands::plan_slides(state).iter().map(|s| item(s, "planned")));
            Ok(json!({ "live": live.slide.map(|s| s.label), "blackout": live.blackout, "items": items }))
        }
        "next" => commands::step_live(app, state, 1).map(|_| json!({})),
        "prev" => commands::step_live(app, state, -1).map(|_| json!({})),
        "blackout" => {
            state.presenter.set_blackout(app, !state.presenter.snapshot().blackout);
            Ok(json!({}))
        }
        "clear" => {
            state.presenter.clear(app);
            Ok(json!({}))
        }
        "present" => {
            let id = q.get("id").ok_or("missing id")?;
            // Release `recent` before `plan_slides` takes its own locks.
            let detected = state.recent.lock().iter().find(|s| &s.id == id).cloned();
            let slide = detected
                .or_else(|| commands::plan_slides(state).into_iter().find(|s| &s.id == id))
                .ok_or("that item is no longer in the list")?;
            state.presenter.present(app, slide);
            Ok(json!({}))
        }
        _ => Err("unknown action".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_strings_are_decoded() {
        let (path, q) = query("/api/remote/present?pin=0420&id=KJV%3A43.3.16&x=a+b");
        assert_eq!(path, "/api/remote/present");
        assert_eq!(q["pin"], "0420");
        assert_eq!(q["id"], "KJV:43.3.16");
        assert_eq!(q["x"], "a b");
        assert_eq!(decode("100%"), "100%");
    }

    #[test]
    fn pin_and_qr() {
        let s = Share::new();
        assert_eq!(s.pin.len(), 4);
        assert!(qr_svg("http://192.168.1.20:7878/").starts_with("<?xml"));
    }
}


