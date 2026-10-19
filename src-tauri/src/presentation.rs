//! Output state lives in Rust, not in either webview, so the projector window can be
//! reloaded/reopened (or crash) without losing what's live, and both the auto-projector
//! (pipeline thread) and the operator (dashboard) go through one code path.

use std::sync::Arc;

use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{events, history::History, scripture::Slide};

/// Windows (WebView2): keep drawing screens that are covered by other windows, so a lower third
/// or stage display captured in OBS never freezes behind the operator's window. Every window
/// must use the same arguments (tauri.conf.json sets them on the main one), and they keep
/// wry's defaults. Other systems ignore them.
const BROWSER_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion --disable-backgrounding-occluded-windows --disable-renderer-backgrounding --disable-background-timer-throttling";

pub const OUTPUT_LABEL: &str = "output";

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputState {
    pub slide: Option<Slide>,
    pub theme: Option<serde_json::Value>,
    pub blackout: bool,
    /// Follow-along: how far the live slide has been read or sung.
    pub reading: Option<Reading>,
    /// "Follow on your phone" QR code shown over the slide.
    pub qr: Option<QrOverlay>,
    /// Monotonic; lets windows ignore out-of-order events.
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub slide_id: String,
    /// Index into the slide's verses/lines, and UTF-16 offset in that line's text.
    pub line: usize,
    pub offset: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QrOverlay {
    pub url: String,
    pub svg: String,
}

pub struct Presenter {
    state: Mutex<OutputState>,
    history: Arc<History>,
}

impl Presenter {
    pub fn new(theme: Option<serde_json::Value>, history: Arc<History>) -> Self {
        Self { state: Mutex::new(OutputState { theme, ..Default::default() }), history }
    }

    pub fn snapshot(&self) -> OutputState {
        self.state.lock().clone()
    }

    fn update(&self, app: &AppHandle, f: impl FnOnce(&mut OutputState)) {
        let snapshot = {
            let mut s = self.state.lock();
            f(&mut s);
            s.revision += 1;
            s.clone()
        };
        // Broadcast: the output window renders it, the dashboard mirrors "what's live".
        if let Err(e) = app.emit(events::OUTPUT_STATE, &snapshot) {
            log::warn!("failed to emit output state: {e}");
        }
    }

    /// Returns false if the slide was already live (no-op, no flicker).
    pub fn present(&self, app: &AppHandle, slide: Slide) -> bool {
        if self.state.lock().slide.as_ref().is_some_and(|s| s.id == slide.id) {
            return false;
        }
        self.history.shown(&slide);
        self.update(app, |s| {
            s.slide = Some(slide);
            s.blackout = false;
            s.reading = None;
        });
        true
    }

    pub fn clear(&self, app: &AppHandle) {
        self.history.cleared();
        self.update(app, |s| {
            s.slide = None;
            s.reading = None;
        });
    }

    /// Ignored if the slide has changed since the speech was transcribed.
    pub fn set_reading(&self, app: &AppHandle, slide_id: &str, line: usize, offset: usize) {
        if self.state.lock().slide.as_ref().map(|s| s.id.as_str()) != Some(slide_id) {
            return;
        }
        self.update(app, |s| s.reading = Some(Reading { slide_id: slide_id.to_owned(), line, offset }));
    }

    pub fn set_qr(&self, app: &AppHandle, qr: Option<QrOverlay>) {
        self.update(app, |s| s.qr = qr);
    }

    pub fn set_blackout(&self, app: &AppHandle, on: bool) {
        self.update(app, |s| s.blackout = on);
    }

    pub fn set_theme(&self, app: &AppHandle, theme: serde_json::Value) {
        self.update(app, |s| s.theme = Some(theme));
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub index: usize,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub primary: bool,
}

fn main_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    app.get_webview_window("main").ok_or(tauri::Error::WindowNotFound)
}

pub fn list_monitors(app: &AppHandle) -> tauri::Result<Vec<MonitorInfo>> {
    let main = main_window(app)?;
    let primary = main.primary_monitor()?.and_then(|m| m.name().cloned());
    Ok(main
        .available_monitors()?
        .into_iter()
        .enumerate()
        .map(|(index, m)| {
            let name = m.name().cloned().unwrap_or_else(|| format!("Display {}", index + 1));
            MonitorInfo {
                index,
                primary: primary.as_deref() == Some(name.as_str()),
                name,
                width: m.size().width,
                height: m.size().height,
                x: m.position().x,
                y: m.position().y,
                scale: m.scale_factor(),
            }
        })
        .collect())
}

/// Opens (or moves) the output window onto `monitor`, defaulting to the first non-primary
/// display. On a display of its own it is borderless and covers the screen. We size it to the
/// display instead of using native fullscreen, because macOS fullscreen spawns a separate
/// Space and animates, which is unwanted live. On the operator's display (a laptop with no
/// projector attached) it opens as an ordinary 16:9 window instead, so the dashboard stays usable.
pub fn open_output(app: &AppHandle, monitor: Option<usize>) -> tauri::Result<()> {
    open_window(app, OUTPUT_LABEL, "output.html", "Hearken Studio Output", monitor)
}

/// Extra screens beside the main output, each with its own look (see `ScreenConfig`).
pub const MAX_EXTRA_SCREENS: usize = 3;

pub fn screen_label(n: usize) -> String {
    format!("output-{}", n + 2)
}

/// Open (or move, or switch the look of) extra screen `n` (0-based; shown as "Screen n+2").
pub fn open_screen(app: &AppHandle, n: usize, look: &str, key: &str, monitor: Option<usize>) -> tauri::Result<()> {
    let look = if matches!(look, "full" | "lower-third" | "stage") { look } else { "full" };
    let key = if matches!(key, "black" | "green" | "theme") { key } else { "black" };
    let url = format!("output.html?look={look}&key={key}");
    open_window(app, &screen_label(n), &url, &format!("Hearken Studio Screen {}", n + 2), monitor)
}

pub fn close_screen(app: &AppHandle, n: usize) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(&screen_label(n)) {
        w.close()?;
    }
    Ok(())
}

pub fn open_screens(app: &AppHandle) -> Vec<usize> {
    (0..MAX_EXTRA_SCREENS).filter(|&n| app.get_webview_window(&screen_label(n)).is_some()).collect()
}

fn open_window(app: &AppHandle, label: &str, url: &str, title: &str, monitor: Option<usize>) -> tauri::Result<()> {
    let window = match app.get_webview_window(label) {
        Some(w) => {
            // A different look: load it in place (the window keeps its display).
            let query = url.split_once('?').map(|(_, q)| q);
            let mut current = w.url()?;
            if current.query() != query {
                current.set_query(query);
                w.navigate(current)?;
            }
            w
        }
        None => WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
            .additional_browser_args(BROWSER_ARGS)
            .title(title)
            .decorations(false)
            .resizable(false)
            .skip_taskbar(true)
            .visible(false)
            .build()?,
    };

    let monitors = window.available_monitors()?;
    let primary = window.primary_monitor()?.and_then(|m| m.name().cloned());
    let target = monitor
        .and_then(|i| monitors.get(i))
        .or_else(|| monitors.iter().find(|m| m.name().cloned() != primary))
        .or_else(|| monitors.first());
    let operator = app.get_webview_window("main").and_then(|w| w.current_monitor().ok().flatten());

    if let Some(m) = target {
        let shared = operator.is_some_and(|o| o.name() == m.name() && o.position() == m.position());
        let (pos, size) = (*m.position(), *m.size());
        if shared {
            let w = size.width * 2 / 5;
            let h = w * 9 / 16;
            window.set_decorations(true)?;
            window.set_resizable(true)?;
            window.set_always_on_top(false)?;
            window.set_size(PhysicalSize::new(w, h))?;
            window.set_position(PhysicalPosition::new(
                pos.x + (size.width - w) as i32 / 2,
                pos.y + (size.height - h) as i32 / 2,
            ))?;
        } else {
            window.set_decorations(false)?;
            window.set_resizable(false)?;
            window.set_position(pos)?;
            window.set_size(size)?;
            window.set_always_on_top(true)?;
        }
    }
    window.show()?;
    Ok(())
}

pub fn close_output(app: &AppHandle) -> tauri::Result<()> {
    if let Some(w) = app.get_webview_window(OUTPUT_LABEL) {
        w.close()?;
    }
    Ok(())
}
