//! Local-network sharing, no internet needed:
//!   /         congregation phones follow the live verse, in any translation and text size
//!   /overlay  transparent lower third for OBS / vMix browser sources (livestream)
//!   /remote   drive the output from a phone (needs the PIN shown in the dashboard)
//! Plain HTTP on the church network, served from a small blocking server thread.

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

use crate::{
    commands,
    presentation::QrOverlay,
    scripture::{BibleDb, Slide},
    state::AppState,
};

const PHONE_HTML: &str = include_str!("share/phone.html");
const OVERLAY_HTML: &str = include_str!("share/overlay.html");
const REMOTE_HTML: &str = include_str!("share/remote.html");

struct Running {
    server: Arc<Server>,
    thread: Option<JoinHandle<()>>,
    port: u16,
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
    pub phone_url: String,
    pub overlay_url: String,
    pub remote_url: String,
    pub pin: String,
    /// QR code for `phone_url`.
    pub qr_svg: String,
    pub error: Option<String>,
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

    /// Start (or move to `port`). Failures are kept for `info()` as well as returned.
    pub fn start(&self, app: AppHandle, port: u16, bible_path: PathBuf) -> Result<()> {
        let result = self.try_start(app, port, bible_path);
        *self.error.lock() = result.as_ref().err().map(|e| format!("{e:#}"));
        result
    }

    fn try_start(&self, app: AppHandle, port: u16, bible_path: PathBuf) -> Result<()> {
        let mut running = self.running.lock();
        if running.as_ref().is_some_and(|r| r.port == port) {
            return Ok(());
        }
        if let Some(r) = running.take() {
            stop(r);
        }
        let server = Arc::new(
            Server::http(("0.0.0.0", port)).map_err(|e| anyhow!("couldn't start sharing on port {port} ({e}). Is another app using it?"))?,
        );
        let pin = self.pin.clone();
        let srv = server.clone();
        let thread = std::thread::Builder::new().name("share".into()).spawn(move || {
            // Own read-only connection: phones never wait on the operator's.
            let bible = BibleDb::open(&bible_path).ok();
            for req in srv.incoming_requests() {
                handle(&app, bible.as_ref(), &pin, req);
            }
        })?;
        *running = Some(Running { server, thread: Some(thread), port });
        log::info!("sharing on port {port}");
        Ok(())
    }

    pub fn stop(&self) {
        *self.error.lock() = None;
        if let Some(r) = self.running.lock().take() {
            stop(r);
        }
    }

    pub fn info(&self, port: u16) -> ShareInfo {
        let enabled = self.running.lock().is_some();
        let base = format!("http://{}:{port}", local_ip());
        let phone_url = format!("{base}/");
        ShareInfo {
            enabled,
            port,
            qr_svg: qr_svg(&phone_url),
            overlay_url: format!("{base}/overlay"),
            remote_url: format!("{base}/remote"),
            phone_url,
            pin: self.pin.clone(),
            error: self.error.lock().clone(),
        }
    }

    pub fn qr_overlay(&self, port: u16) -> QrOverlay {
        let info = self.info(port);
        QrOverlay { svg: info.qr_svg, url: info.phone_url }
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
fn local_ip() -> IpAddr {
    UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| s.connect("10.255.255.255:1").map(|_| s))
        .and_then(|s| s.local_addr())
        .map(|a| a.ip())
        .ok()
        .filter(|ip| !ip.is_unspecified())
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
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

fn handle(app: &AppHandle, bible: Option<&BibleDb>, pin: &str, req: Request) {
    let (path, q) = query(req.url());
    let state = app.state::<AppState>();
    let html = "text/html; charset=utf-8";
    let js = "application/json";
    match (req.method(), path.as_str()) {
        (Method::Get, "/") => respond(req, 200, html, PHONE_HTML.into()),
        (Method::Get, "/overlay") => respond(req, 200, html, OVERLAY_HTML.into()),
        (Method::Get, "/remote") => respond(req, 200, html, REMOTE_HTML.into()),
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
        return json!({ "revision": live.revision, "slide": null });
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
        "slide": {
            "id": slide.id,
            "label": slide.label,
            "translation": code,
            "song": slide.is_song(),
            "verses": verses,
        }
    })
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
