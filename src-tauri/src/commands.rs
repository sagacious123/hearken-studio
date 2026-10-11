//! IPC surface. Commands that touch audio, models or the GPU are `async` and offload to the
//! blocking pool — Tauri runs synchronous commands on the main thread.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    backgrounds::{self, UploadedBackground},
    asr::{
        hardware::{self, HardwareProfile},
        whisper::model_path,
    },
    audio::capture::{self, DeviceInfo},
    events,
    history::{self, ArchiveHit, DaySummary, Insights},
    pipeline::{self, PipelineConfig, PipelineInfo},
    presentation::{self, MonitorInfo, OutputState},
    scripture::{self, db, db::Translation, parser::ReferenceParser, ScriptureRef, Slide},
    share::ShareInfo,
    songs::{self, SongSummary},
    state::{write_json, AppState, Plan, Settings},
};

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

// ---------- settings & hardware ----------

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.lock().clone()
}

#[tauri::command]
pub fn update_settings(settings: Settings, app: AppHandle, state: State<'_, AppState>) -> CmdResult<Settings> {
    *state.settings.lock() = settings.clone();
    state.save_settings().map_err(err)?;
    // A sharing failure (port in use) is reported in the Share panel, not as a failed save.
    if let Err(e) = apply_sharing(&app, &state) {
        log::warn!("{e}");
    }
    Ok(settings)
}

/// Start or stop the local-network server to match the settings.
pub fn apply_sharing(app: &AppHandle, state: &AppState) -> CmdResult<()> {
    let (enabled, port) = {
        let s = state.settings.lock();
        (s.share_enabled, s.share_port)
    };
    if enabled {
        state.share.start(app.clone(), port, state.paths.bible_db.clone()).map_err(err)
    } else {
        state.share.stop();
        if state.presenter.snapshot().qr.is_some() {
            state.presenter.set_qr(app, None);
        }
        Ok(())
    }
}

fn secondary(state: &AppState) -> Option<String> {
    state.settings.lock().secondary_translation.clone()
}

#[tauri::command]
pub fn get_hardware_profile(state: State<'_, AppState>) -> Option<HardwareProfile> {
    state.hardware.lock().clone()
}

#[tauri::command]
pub async fn run_hardware_benchmark(app: AppHandle, state: State<'_, AppState>) -> CmdResult<HardwareProfile> {
    if state.pipeline.lock().is_some() {
        return Err("stop listening before re-running the benchmark".into());
    }
    benchmark(&app, &state).await
}

async fn benchmark(app: &AppHandle, state: &AppState) -> CmdResult<HardwareProfile> {
    let models = state.paths.models_dir.clone();
    let wav = state.paths.bench_wav.clone();
    let reduced = state.settings.lock().reduced_encoder_ctx;
    let app2 = app.clone();
    let profile = tauri::async_runtime::spawn_blocking(move || {
        hardware::run_benchmark(&models, &wav, env!("CARGO_PKG_VERSION"), reduced, |msg| {
            let _ = app2.emit(events::HARDWARE_PROGRESS, msg);
        })
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    hardware::save(&state.paths.hardware(), &profile).map_err(err)?;
    *state.hardware.lock() = Some(profile.clone());
    Ok(profile)
}

// ---------- pipeline ----------

#[tauri::command]
pub fn list_audio_devices() -> Vec<DeviceInfo> {
    capture::list_devices()
}

#[tauri::command]
pub async fn start_pipeline(app: AppHandle, state: State<'_, AppState>) -> CmdResult<PipelineInfo> {
    if state.pipeline.lock().is_some() {
        return Err("already listening".into());
    }
    let cached = state.hardware.lock().clone();
    let profile = match cached {
        Some(p) => p,
        None => benchmark(&app, &state).await?,
    };
    let settings = state.settings.lock().clone();
    let model = settings.model_override.clone().unwrap_or(profile.model.clone());
    let model_file = model_path(&state.paths.models_dir, &model)
        .ok_or_else(|| format!("model '{model}' not found — run scripts/fetch_models.sh"))?;

    let cfg = PipelineConfig {
        device: settings.input_device.clone(),
        model_path: model_file,
        model_name: model,
        use_gpu: settings.gpu_override.unwrap_or(profile.use_gpu),
        threads: profile.threads,
        step_ms: profile.step_ms,
        reduced_ctx: settings.reduced_encoder_ctx,
        bible_path: state.paths.bible_db.clone(),
    };
    let v11n = state.versification.clone();
    let app2 = app.clone();
    let handle = tauri::async_runtime::spawn_blocking(move || pipeline::start(app2, cfg, v11n))
        .await
        .map_err(err)?
        .map_err(|e| format!("{e:#}"))?;
    let info = handle.info.clone();
    *state.pipeline.lock() = Some(handle);
    Ok(info)
}

#[tauri::command]
pub async fn stop_pipeline(state: State<'_, AppState>) -> CmdResult<()> {
    let handle = state.pipeline.lock().take();
    if let Some(h) = handle {
        tauri::async_runtime::spawn_blocking(move || h.stop()).await.map_err(err)?;
    }
    Ok(())
}

// ---------- scripture ----------

#[tauri::command]
pub fn list_translations(state: State<'_, AppState>) -> Vec<Translation> {
    state.bible.lock().translations().to_vec()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    /// Set when the query parses as a reference ("jn 3 16", "romans 8:28-30").
    reference: Option<Slide>,
    /// Full-text matches, best first.
    hits: Vec<Slide>,
    elapsed_ms: f32,
}

#[tauri::command]
pub fn search_scripture(query: String, translation: String, state: State<'_, AppState>) -> CmdResult<SearchResponse> {
    let t0 = std::time::Instant::now();
    let bible = state.bible.lock();
    let secondary = secondary(&state);
    let parsed = ReferenceParser::new().parse(&query, true, &state.versification).into_iter().next();
    let reference = match parsed {
        Some(p) => scripture::build_slide(&bible, &translation, secondary.as_deref(), &p.reference).map_err(err)?,
        None => None,
    };
    let hits = match db::user_query(&query) {
        Some(q) => bible
            .search(Some(&translation), &q, 25)
            .map_err(err)?
            .into_iter()
            .filter_map(|h| scripture::build_slide(&bible, &translation, secondary.as_deref(), &h.reference).ok().flatten())
            .collect(),
        None => vec![],
    };
    Ok(SearchResponse { reference, hits, elapsed_ms: t0.elapsed().as_secs_f32() * 1000.0 })
}

/// Re-resolve a reference (e.g. in another translation).
#[tauri::command]
pub fn build_slide(reference: ScriptureRef, translation: String, state: State<'_, AppState>) -> CmdResult<Option<Slide>> {
    scripture::build_slide(&state.bible.lock(), &translation, secondary(&state).as_deref(), &reference).map_err(err)
}

// ---------- output ----------

#[tauri::command]
pub fn get_output_state(state: State<'_, AppState>) -> OutputState {
    state.presenter.snapshot()
}

#[tauri::command]
pub fn present(slide: Slide, app: AppHandle, state: State<'_, AppState>) {
    state.presenter.present(&app, slide);
}

#[tauri::command]
pub fn clear_output(app: AppHandle, state: State<'_, AppState>) {
    state.presenter.clear(&app);
}

#[tauri::command]
pub fn set_blackout(on: bool, app: AppHandle, state: State<'_, AppState>) {
    state.presenter.set_blackout(&app, on);
}

#[tauri::command]
pub fn set_theme(theme: serde_json::Value, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    write_json(&state.paths.theme(), &theme).map_err(err)?;
    state.presenter.set_theme(&app, theme);
    Ok(())
}

/// Move the live slide forward/back by one verse (or song section).
#[tauri::command]
pub fn step_verse(delta: i32, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    step_live(&app, &state, delta)
}

pub fn step_live(app: &AppHandle, state: &AppState, delta: i32) -> CmdResult<()> {
    let Some(slide) = state.presenter.snapshot().slide else { return Ok(()) };
    if let Some(part) = &slide.song {
        let target = part.section as i64 + i64::from(delta.signum());
        let next = state.songs.get(&part.song_id).zip(usize::try_from(target).ok()).and_then(|(song, i)| songs::slide(&song, i));
        if let Some(s) = next {
            state.presenter.present(app, s);
        }
        return Ok(());
    }
    let r = &slide.reference;
    let start = r.verse_start.unwrap_or(1);
    let end = r.verse_end.unwrap_or(start);
    let max = state.versification.max_verse(r.book_id, r.chapter).unwrap_or(end);
    let next = match delta.signum() {
        1 if end < max => end + 1,
        -1 if start > 1 => start - 1,
        _ => return Ok(()),
    };
    let target = ScriptureRef { verse_start: Some(next), verse_end: None, ..r.clone() };
    let keep = slide.secondary.as_ref().map(|s| s.translation.as_str());
    let built = scripture::build_slide(&state.bible.lock(), &slide.translation, keep, &target).map_err(err)?;
    if let Some(s) = built {
        state.presenter.present(app, s);
    }
    Ok(())
}

#[tauri::command]
pub fn list_monitors(app: AppHandle) -> CmdResult<Vec<MonitorInfo>> {
    presentation::list_monitors(&app).map_err(err)
}

/// Async: creating a window from a synchronous command deadlocks on Windows.
#[tauri::command]
pub async fn open_output_window(monitor: Option<usize>, app: AppHandle) -> CmdResult<()> {
    presentation::open_output(&app, monitor).map_err(err)
}

#[tauri::command]
pub async fn close_output_window(app: AppHandle) -> CmdResult<()> {
    presentation::close_output(&app).map_err(err)
}

// ---------- sharing (phones, OBS overlay, remote) ----------

#[tauri::command]
pub fn get_share_info(state: State<'_, AppState>) -> ShareInfo {
    state.share.info(state.settings.lock().share_port)
}

/// Show the "follow on your phone" QR code on the projector.
#[tauri::command]
pub fn show_qr(on: bool, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    if on && !state.settings.lock().share_enabled {
        return Err("turn on sharing first".into());
    }
    let qr = on.then(|| state.share.qr_overlay(state.settings.lock().share_port));
    state.presenter.set_qr(&app, qr);
    Ok(())
}

// ---------- sermon plan ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanInfo {
    text: String,
    slides: Vec<Slide>,
}

pub fn plan_slides(state: &AppState) -> Vec<Slide> {
    let refs = state.plan.lock().refs.clone();
    let (translation, secondary) = {
        let s = state.settings.lock();
        (s.translation.clone(), s.secondary_translation.clone())
    };
    let bible = state.bible.lock();
    refs.iter().filter_map(|r| scripture::build_slide(&bible, &translation, secondary.as_deref(), r).ok().flatten()).collect()
}

#[tauri::command]
pub fn get_plan(state: State<'_, AppState>) -> PlanInfo {
    plan_info(&state)
}

fn plan_info(state: &AppState) -> PlanInfo {
    // Copy the text out first: a lock guard in the same expression as `plan_slides` would
    // still be held when it locks the plan again, and parking_lot mutexes aren't re-entrant.
    let text = state.plan.lock().text.clone();
    PlanInfo { text, slides: plan_slides(state) }
}

/// Paste the pastor's notes: every reference in them is lined up and boosted when spoken.
#[tauri::command]
pub fn set_plan(text: String, state: State<'_, AppState>) -> CmdResult<PlanInfo> {
    std::fs::write(state.paths.plan(), &text).map_err(err)?;
    *state.plan.lock() = Plan::parse(&text, &state.versification);
    Ok(PlanInfo { text, slides: plan_slides(&state) })
}

// ---------- worship songs ----------

#[tauri::command]
pub fn list_songs(state: State<'_, AppState>) -> Vec<SongSummary> {
    state.songs.summaries()
}

#[tauri::command]
pub fn get_song_text(id: String, state: State<'_, AppState>) -> CmdResult<String> {
    state.songs.get(&id).map(|s| songs::to_text(&s)).ok_or_else(|| "song not found".into())
}

/// Add (no `id`) or replace a song from pasted lyrics.
#[tauri::command]
pub fn save_song(text: String, id: Option<String>, state: State<'_, AppState>) -> CmdResult<Vec<SongSummary>> {
    state.songs.save(&text, id.as_deref()).map_err(err)?;
    Ok(state.songs.summaries())
}

#[tauri::command]
pub fn delete_song(id: String, state: State<'_, AppState>) -> CmdResult<Vec<SongSummary>> {
    state.songs.delete(&id).map_err(err)?;
    Ok(state.songs.summaries())
}

#[tauri::command]
pub fn song_slide(id: String, section: usize, state: State<'_, AppState>) -> Option<Slide> {
    state.songs.get(&id).and_then(|s| songs::slide(&s, section))
}

// ---------- history: recaps, archive, insights, CCLI ----------

#[tauri::command]
pub fn list_sessions(state: State<'_, AppState>) -> Vec<DaySummary> {
    state.history.summaries()
}

#[tauri::command]
pub fn search_archive(query: String, state: State<'_, AppState>) -> Vec<ArchiveHit> {
    let parsed = ReferenceParser::new().parse(&query, true, &state.versification).into_iter().next();
    state.history.search(&query, parsed.as_ref().map(|p| &p.reference), 100)
}

#[tauri::command]
pub fn get_insights(year: i32, state: State<'_, AppState>) -> Insights {
    state.history.insights(year)
}

/// Writes the recap page to Documents/Hearken Studio and opens it in the browser
/// (where it can be printed or saved as PDF). Returns the file path.
#[tauri::command]
pub fn export_recap(date: String, app: AppHandle, state: State<'_, AppState>) -> CmdResult<String> {
    let recap = state.history.recap(&date);
    if recap.scriptures.is_empty() && recap.songs.is_empty() && recap.mentioned.is_empty() {
        return Err("nothing was detected or shown on that day".into());
    }
    let church = state.settings.lock().church_name.clone();
    let path = export_dir(&app)?.join(format!("Recap {date}.html"));
    std::fs::write(&path, history::recap_html(&recap, &church)).map_err(err)?;
    open_path(&path);
    Ok(path.display().to_string())
}

/// Song usage for the last `days` days, as CSV for CCLI reporting.
#[tauri::command]
pub fn export_ccli_report(days: i64, app: AppHandle, state: State<'_, AppState>) -> CmdResult<String> {
    let path = export_dir(&app)?.join(format!("Song usage (last {days} days).csv"));
    std::fs::write(&path, state.history.ccli_csv(days)).map_err(err)?;
    open_path(&path);
    Ok(path.display().to_string())
}

fn export_dir(app: &AppHandle) -> CmdResult<PathBuf> {
    let dir = app.path().document_dir().map_err(err)?.join("Hearken Studio");
    std::fs::create_dir_all(&dir).map_err(err)?;
    Ok(dir)
}

/// Open a file with the system's default app (browser for .html, spreadsheet for .csv).
fn open_path(path: &Path) {
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(path).spawn();
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(path).spawn();
    if let Err(e) = r {
        log::warn!("couldn't open {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, sync::Arc, time::Duration};

    use parking_lot::Mutex;

    use super::*;
    use crate::{
        history::History,
        presentation::Presenter,
        scripture::{BibleDb, Versification},
        share::Share,
        songs::SongLibrary,
        state::Paths,
    };

    fn test_state() -> AppState {
        let dir = std::env::temp_dir().join(format!("sb-commands-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let v11n = Versification::from_map(HashMap::from([((43, 3), 36)]));
        let history = Arc::new(History::new(dir.join("history")));
        AppState {
            paths: Paths { bible_db: dir.join("bible.db"), models_dir: dir.clone(), bench_wav: dir.join("x.wav"), config_dir: dir.clone() },
            settings: Mutex::new(Settings::default()),
            bible: Mutex::new(BibleDb::open_in_memory_for_tests()),
            plan: Mutex::new(Plan::parse("Intro: John 3:16", &v11n)),
            versification: Arc::new(v11n),
            hardware: Mutex::new(None),
            pipeline: Mutex::new(None),
            presenter: Presenter::new(None, history.clone()),
            history,
            songs: SongLibrary::load(dir.join("songs.json")),
            recent: Mutex::new(Default::default()),
            share: Share::new(),
        }
    }

    /// Regression: opening the Plan tab froze the app (the plan was locked twice).
    #[test]
    fn reading_the_plan_does_not_deadlock() {
        let state = Arc::new(test_state());
        let (tx, rx) = std::sync::mpsc::channel();
        let s = state.clone();
        std::thread::spawn(move || {
            let info = plan_info(&s);
            let _ = tx.send(info.text);
        });
        let text = rx.recv_timeout(Duration::from_secs(5)).expect("plan_info deadlocked");
        assert_eq!(text, "Intro: John 3:16");
        assert_eq!(state.plan.lock().refs.len(), 1);
    }
}

// ---------- uploaded backgrounds ----------

#[tauri::command]
pub fn list_backgrounds(state: State<'_, AppState>) -> Vec<UploadedBackground> {
    backgrounds::list(&state.paths.backgrounds())
}

/// Raw image bytes as the request body; the original file name in the
/// `x-file-name` header (URI-encoded). Raw bytes avoid a slow JSON number array.
#[tauri::command]
pub fn save_background(request: tauri::ipc::Request<'_>, state: State<'_, AppState>) -> CmdResult<Vec<UploadedBackground>> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected the image bytes".into());
    };
    let name = request
        .headers()
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .map(crate::share::decode)
        .unwrap_or_else(|| "image".into());
    let dir = state.paths.backgrounds();
    backgrounds::save(&dir, &name, bytes).map_err(|e| format!("{e:#}"))?;
    Ok(backgrounds::list(&dir))
}

#[tauri::command]
pub fn delete_background(file: String, state: State<'_, AppState>) -> CmdResult<Vec<UploadedBackground>> {
    let dir = state.paths.backgrounds();
    backgrounds::delete(&dir, &file).map_err(|e| format!("{e:#}"))?;
    Ok(backgrounds::list(&dir))
}

/// Absolute path of an uploaded background, for the asset protocol (output window).
#[tauri::command]
pub fn background_path(file: String, state: State<'_, AppState>) -> Option<String> {
    backgrounds::path(&state.paths.backgrounds(), &file)
}
