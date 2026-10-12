//! IPC surface. Tauri runs synchronous commands on the main (UI) thread, so anything that
//! touches files, SQLite, audio devices or a shared lock is `async`; only trivial reads stay
//! sync. Slow work (models, device enumeration) also moves to the blocking pool.

use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    backgrounds::{self, UploadedBackground},
    asr::{
        accuracy::{self, AccuracyProfile, Calibration},
        hardware::{self, HardwareProfile},
        models::{self, ModelStatus},
        priority,
        whisper::{WhisperEngine, GPU_BACKEND},
    },
    audio::capture::{self, DeviceInfo},
    events,
    history::{self, ArchiveHit, DaySummary, Insights},
    pipeline::{self, PipelineConfig, PipelineInfo, RefineConfig},
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
pub async fn update_settings(settings: Settings, app: AppHandle, state: State<'_, AppState>) -> CmdResult<Settings> {
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
    let (enabled, port, name) = {
        let s = state.settings.lock();
        (s.share_enabled, s.share_port, s.share_name.clone())
    };
    if enabled {
        state.share.start(app.clone(), port, &name, state.paths.bible_db.clone()).map_err(err)
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
pub async fn list_audio_devices() -> CmdResult<Vec<DeviceInfo>> {
    // CoreAudio enumeration takes 200–500 ms: never on the UI thread.
    tauri::async_runtime::spawn_blocking(capture::list_devices).await.map_err(err)
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
    let (bundled, downloaded) = (state.paths.models_dir.clone(), state.paths.downloaded_models());
    let dirs = [bundled.as_path(), downloaded.as_path()];
    let model_file = models::find(&dirs, &model).ok_or_else(|| format!("model '{model}' not found — run scripts/fetch_models.sh"))?;
    let use_gpu = settings.gpu_override.unwrap_or(profile.use_gpu);
    let calibrations = AccuracyProfile::load(&state.paths.accuracy_profile(), env!("CARGO_PKG_VERSION"));
    let refine = choose_accuracy_model(&settings.accuracy_model, use_gpu, &dirs, &calibrations).map(|(id, path, calibration, forced)| RefineConfig {
        model_path: path,
        model_name: id.to_owned(),
        use_gpu,
        threads: priority::accuracy_threads(use_gpu, profile.threads),
        calibration,
        forced,
        calibration_file: state.paths.accuracy_profile(),
        bench_wav: state.paths.bench_wav.clone(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
    });

    let cfg = PipelineConfig {
        device: settings.input_device.clone(),
        model_path: model_file,
        model_name: model,
        use_gpu,
        threads: profile.threads,
        step_ms: profile.step_ms,
        reduced_ctx: settings.reduced_encoder_ctx,
        bible_path: state.paths.bible_db.clone(),
        refine,
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

/// Which model (if any) re-transcribes finished sentences, with its saved speed test.
/// "auto": the most accurate installed model proven to keep up here; failing that, an
/// untested one (it's timed on first use: GPU machines try Turbo first, CPU-only Small).
fn choose_accuracy_model(
    choice: &str,
    use_gpu: bool,
    dirs: &[&Path],
    profile: &AccuracyProfile,
) -> Option<(&'static str, PathBuf, Option<Calibration>, bool)> {
    let backend = if use_gpu { GPU_BACKEND } else { "cpu" };
    let installed = |id: &'static str| models::find(dirs, id).map(|p| (id, p));
    let cal = |id: &str| profile.get(id, backend).cloned();
    match choice {
        "off" => None,
        "auto" => {
            let best_known = ["large-v3-turbo", "small.en"].into_iter().filter(|id| cal(id).is_some_and(|c| c.usable)).find_map(installed);
            let untested = if use_gpu { ["large-v3-turbo", "small.en"] } else { ["small.en", "large-v3-turbo"] };
            let first_untested = || untested.into_iter().filter(|id| cal(id).is_none()).find_map(installed);
            best_known.or_else(first_untested).map(|(id, p)| (id, p, cal(id), false))
        }
        other => models::CATALOG.iter().find(|m| m.id == other && m.accuracy).and_then(|m| installed(m.id)).map(|(id, p)| (id, p, cal(id), true)),
    }
}

/// Time a downloaded model on this computer and save the result (blocking). Runs exactly as
/// the accuracy pass will: same thread count, low priority.
fn calibrate_now(state: &AppState, id: &str, use_gpu: bool, live_threads: usize) -> Result<Calibration, String> {
    let dirs = model_dirs(state);
    let path = models::find(&[dirs[0].as_path(), dirs[1].as_path()], id).ok_or("model not installed")?;
    let audio = hardware::load_wav(&state.paths.bench_wav).map_err(|e| format!("{e:#}"))?;
    let threads = priority::accuracy_threads(use_gpu, live_threads);
    let model = id.to_owned();
    let cal = std::thread::spawn(move || {
        priority::background();
        let mut engine = WhisperEngine::load_with_fallback(&path, use_gpu, threads).map_err(|e| format!("{e:#}"))?;
        let books: Vec<&str> = scripture::books::BOOKS.iter().map(|b| b.display).collect();
        let prompt = crate::asr::whisper::accurate_prompt("", &[], &books);
        accuracy::calibrate(&mut engine, &model, &audio, &prompt).map_err(|e| format!("{e:#}"))
    })
    .join()
    .map_err(|_| "speed test crashed".to_string())??;
    AccuracyProfile::record(&state.paths.accuracy_profile(), env!("CARGO_PKG_VERSION"), cal.clone());
    Ok(cal)
}

// ---------- speech models ----------

fn model_dirs(state: &AppState) -> [std::path::PathBuf; 2] {
    [state.paths.models_dir.clone(), state.paths.downloaded_models()]
}

/// A model plus how it runs on this computer (from its speed test, when it has one).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelView {
    #[serde(flatten)]
    status: ModelStatus,
    /// None until tested; false = too slow for the accuracy pass here.
    usable: Option<bool>,
    speed: Option<String>,
}

fn model_views(state: &AppState) -> Vec<ModelView> {
    let dirs = model_dirs(state);
    let use_gpu = {
        let s = state.settings.lock();
        s.gpu_override.or(state.hardware.lock().as_ref().map(|p| p.use_gpu)).unwrap_or(false)
    };
    let backend = if use_gpu { GPU_BACKEND } else { "cpu" };
    let profile = AccuracyProfile::load(&state.paths.accuracy_profile(), env!("CARGO_PKG_VERSION"));
    models::statuses(&[dirs[0].as_path(), dirs[1].as_path()])
        .into_iter()
        .map(|status| {
            let cal = profile.get(status.id, backend).filter(|_| status.installed);
            ModelView { usable: cal.map(|c| c.usable), speed: cal.map(|c| if c.usable { c.describe() } else { "too slow on this computer".into() }), status }
        })
        .collect()
}

#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> CmdResult<Vec<ModelView>> {
    Ok(model_views(&state))
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DownloadProgress {
    id: String,
    received: u64,
    total: u64,
    /// "download", then "testing" while the speed test runs.
    phase: &'static str,
}

/// Time an installed model on this computer again (e.g. after closing heavy apps).
#[tauri::command]
pub async fn test_model_speed(id: String, app: AppHandle) -> CmdResult<Vec<ModelView>> {
    if app.state::<AppState>().pipeline.lock().is_some() {
        return Err("stop listening first: the test needs the computer's full attention".into());
    }
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        let (use_gpu, threads) = gpu_and_threads(&state)?;
        calibrate_now(&state, &id, use_gpu, threads)
    })
    .await
    .map_err(err)??;
    Ok(model_views(&app.state::<AppState>()))
}

fn gpu_and_threads(state: &AppState) -> Result<(bool, usize), String> {
    let profile = state.hardware.lock().clone().ok_or("start listening once first, so the computer is benchmarked")?;
    let gpu = state.settings.lock().gpu_override.unwrap_or(profile.use_gpu);
    Ok((gpu, profile.threads))
}

/// Download a model for the accuracy pass (resumable, checksum-verified). Progress arrives as
/// `models://progress` events; resolves when the model is ready.
#[tauri::command]
pub async fn download_model(id: String, app: AppHandle, state: State<'_, AppState>) -> CmdResult<Vec<ModelView>> {
    let cancel = Arc::new(AtomicBool::new(false));
    if state.downloads.lock().insert(id.clone(), cancel.clone()).is_some() {
        return Err("that model is already downloading".into());
    }
    let dir = state.paths.downloaded_models();
    let (app2, id2) = (app.clone(), id.clone());
    let result = tauri::async_runtime::spawn_blocking(move || {
        models::download(&id2, &dir, |received, total| {
            let _ = app2.emit(events::MODEL_PROGRESS, DownloadProgress { id: id2.clone(), received, total, phase: "download" });
        }, &cancel)?;
        // Time it right away, so "Automatic" knows whether it keeps up on this computer.
        let state = app2.state::<AppState>();
        if state.pipeline.lock().is_none() {
            if let Ok((use_gpu, threads)) = gpu_and_threads(&state) {
                let _ = app2.emit(events::MODEL_PROGRESS, DownloadProgress { id: id2.clone(), received: 1, total: 1, phase: "testing" });
                if let Err(e) = calibrate_now(&state, &id2, use_gpu, threads) {
                    log::warn!("speed test of {id2} failed: {e}");
                }
            }
        }
        anyhow::Ok(())
    })
    .await
    .map_err(err)
    .and_then(|r| r.map_err(|e| format!("{e:#}")));
    state.downloads.lock().remove(&id);
    result?;
    Ok(model_views(&state))
}

#[tauri::command]
pub async fn cancel_model_download(id: String, state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(flag) = state.downloads.lock().get(&id) {
        flag.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_model(id: String, state: State<'_, AppState>) -> CmdResult<Vec<ModelView>> {
    if state.pipeline.lock().is_some() {
        return Err("stop listening before removing a model".into());
    }
    models::delete(&id, &state.paths.downloaded_models()).map_err(|e| format!("{e:#}"))?;
    Ok(model_views(&state))
}

// ---------- scripture ----------

#[tauri::command]
pub async fn list_translations(state: State<'_, AppState>) -> CmdResult<Vec<Translation>> {
    Ok(state.bible.lock().translations().to_vec())
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
pub async fn search_scripture(query: String, translation: String, state: State<'_, AppState>) -> CmdResult<SearchResponse> {
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookInfo {
    id: u8,
    name: &'static str,
    /// Lowercase names people type: "jn", "1 cor", "first corinthians", "psalm".
    aliases: Vec<String>,
    /// Verses in each chapter (index 0 = chapter 1), from the reference translation.
    verses: Vec<u16>,
}

/// The 66 books with their chapter and verse counts, for the passage picker.
#[tauri::command]
pub async fn list_books(state: State<'_, AppState>) -> CmdResult<Vec<BookInfo>> {
    let mut aliases: std::collections::HashMap<u8, Vec<String>> = Default::default();
    for (alias, id, _) in scripture::books::all_aliases() {
        aliases.entry(id).or_default().push(alias);
    }
    Ok(scripture::books::BOOKS
        .iter()
        .map(|b| BookInfo {
            id: b.id,
            name: b.display,
            aliases: aliases.remove(&b.id).unwrap_or_default(),
            verses: (1..=b.chapters).map(|c| state.versification.max_verse(b.id, c).unwrap_or(0)).collect(),
        })
        .collect())
}

/// Re-resolve a reference (e.g. in another translation).
#[tauri::command]
pub async fn build_slide(reference: ScriptureRef, translation: String, state: State<'_, AppState>) -> CmdResult<Option<Slide>> {
    scripture::build_slide(&state.bible.lock(), &translation, secondary(&state).as_deref(), &reference).map_err(err)
}

// ---------- output ----------

#[tauri::command]
pub fn get_output_state(state: State<'_, AppState>) -> OutputState {
    state.presenter.snapshot()
}

#[tauri::command]
pub async fn present(slide: Slide, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    state.presenter.present(&app, slide);
    Ok(())
}

#[tauri::command]
pub async fn clear_output(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    state.presenter.clear(&app);
    Ok(())
}

#[tauri::command]
pub async fn set_blackout(on: bool, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    state.presenter.set_blackout(&app, on);
    Ok(())
}

#[tauri::command]
pub async fn set_theme(theme: serde_json::Value, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    write_json(&state.paths.theme(), &theme).map_err(err)?;
    state.presenter.set_theme(&app, theme);
    Ok(())
}

/// Move the live slide forward/back by one verse (or song section).
#[tauri::command]
pub async fn step_verse(delta: i32, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
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
pub async fn get_share_info(state: State<'_, AppState>) -> CmdResult<ShareInfo> {
    let (port, name) = {
        let s = state.settings.lock();
        (s.share_port, s.share_name.clone())
    };
    Ok(state.share.info(port, &name))
}

/// Show the "follow on your phone" QR code on the projector.
#[tauri::command]
pub async fn show_qr(on: bool, app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    if on && !state.settings.lock().share_enabled {
        return Err("turn on sharing first".into());
    }
    let (port, name) = {
        let s = state.settings.lock();
        (s.share_port, s.share_name.clone())
    };
    let qr = on.then(|| state.share.qr_overlay(port, &name));
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
pub async fn get_plan(state: State<'_, AppState>) -> CmdResult<PlanInfo> {
    Ok(plan_info(&state))
}

fn plan_info(state: &AppState) -> PlanInfo {
    // Copy the text out first: a lock guard in the same expression as `plan_slides` would
    // still be held when it locks the plan again, and parking_lot mutexes aren't re-entrant.
    let text = state.plan.lock().text.clone();
    PlanInfo { text, slides: plan_slides(state) }
}

/// Paste the pastor's notes: every reference in them is lined up and boosted when spoken.
#[tauri::command]
pub async fn set_plan(text: String, state: State<'_, AppState>) -> CmdResult<PlanInfo> {
    std::fs::write(state.paths.plan(), &text).map_err(err)?;
    *state.plan.lock() = Plan::parse(&text, &state.versification);
    Ok(PlanInfo { text, slides: plan_slides(&state) })
}

// ---------- worship songs ----------

#[tauri::command]
pub async fn list_songs(state: State<'_, AppState>) -> CmdResult<Vec<SongSummary>> {
    Ok(state.songs.summaries())
}

#[tauri::command]
pub async fn get_song_text(id: String, state: State<'_, AppState>) -> CmdResult<String> {
    state.songs.get(&id).map(|s| songs::to_text(&s)).ok_or_else(|| "song not found".into())
}

/// Add (no `id`) or replace a song from pasted lyrics.
#[tauri::command]
pub async fn save_song(text: String, id: Option<String>, state: State<'_, AppState>) -> CmdResult<Vec<SongSummary>> {
    state.songs.save(&text, id.as_deref()).map_err(err)?;
    Ok(state.songs.summaries())
}

#[tauri::command]
pub async fn delete_song(id: String, state: State<'_, AppState>) -> CmdResult<Vec<SongSummary>> {
    state.songs.delete(&id).map_err(err)?;
    Ok(state.songs.summaries())
}

#[tauri::command]
pub async fn song_slide(id: String, section: usize, state: State<'_, AppState>) -> CmdResult<Option<Slide>> {
    Ok(state.songs.get(&id).and_then(|s| songs::slide(&s, section)))
}

// ---------- history: recaps, archive, insights, CCLI ----------

#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> CmdResult<Vec<DaySummary>> {
    Ok(state.history.summaries())
}

#[tauri::command]
pub async fn search_archive(query: String, state: State<'_, AppState>) -> CmdResult<Vec<ArchiveHit>> {
    let parsed = ReferenceParser::new().parse(&query, true, &state.versification).into_iter().next();
    Ok(state.history.search(&query, parsed.as_ref().map(|p| &p.reference), 100))
}

#[tauri::command]
pub async fn get_insights(year: i32, state: State<'_, AppState>) -> CmdResult<Insights> {
    Ok(state.history.insights(year))
}

/// Writes the recap page to Documents/Hearken Studio and opens it in the browser
/// (where it can be printed or saved as PDF). Returns the file path.
#[tauri::command]
pub async fn export_recap(date: String, app: AppHandle, state: State<'_, AppState>) -> CmdResult<String> {
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
pub async fn export_ccli_report(days: i64, app: AppHandle, state: State<'_, AppState>) -> CmdResult<String> {
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
            downloads: Mutex::new(Default::default()),
        }
    }

    #[test]
    fn automatic_accuracy_model_prefers_proven_then_untested() {
        let dir = std::env::temp_dir().join(format!("sb-choose-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for f in ["ggml-small.en-q5_1.bin", "ggml-large-v3-turbo-q5_0.bin"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        let dirs = [dir.as_path()];
        let cal = |model: &str, backend: &str, usable: bool| Calibration {
            model: model.into(), backend: backend.into(), usable, beam: true, full_ctx: false, decode_ms: 1.0, audio_ms: 10.0,
        };
        let id = |c: Option<(&'static str, PathBuf, Option<Calibration>, bool)>| c.map(|c| c.0);

        // Nothing tested yet: GPU machines try Turbo first, CPU-only machines Small.
        let empty = AccuracyProfile::default();
        assert_eq!(id(choose_accuracy_model("auto", true, &dirs, &empty)), Some("large-v3-turbo"));
        assert_eq!(id(choose_accuracy_model("auto", false, &dirs, &empty)), Some("small.en"));

        // CPU: Turbo proven too slow, Small proven fine → Small; both too slow → none.
        let mut p = AccuracyProfile { app_version: "x".into(), entries: vec![cal("large-v3-turbo", "cpu", false), cal("small.en", "cpu", true)] };
        assert_eq!(id(choose_accuracy_model("auto", false, &dirs, &p)), Some("small.en"));
        p.entries = vec![cal("large-v3-turbo", "cpu", false), cal("small.en", "cpu", false)];
        assert_eq!(id(choose_accuracy_model("auto", false, &dirs, &p)), None);

        // A proven model beats an untested more-accurate one only when the latter is unknown:
        // Turbo proven usable wins over everything.
        p.entries = vec![cal("large-v3-turbo", GPU_BACKEND, true)];
        assert_eq!(id(choose_accuracy_model("auto", true, &dirs, &p)), Some("large-v3-turbo"));

        // Explicit choice is honoured (and flagged as forced); off is off.
        let forced = choose_accuracy_model("large-v3-turbo", false, &dirs, &AccuracyProfile::default()).unwrap();
        assert_eq!((forced.0, forced.3), ("large-v3-turbo", true));
        assert!(choose_accuracy_model("off", true, &dirs, &empty).is_none());
        assert!(choose_accuracy_model("base.en", true, &dirs, &empty).is_none()); // not an accuracy model
        std::fs::remove_dir_all(&dir).ok();
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
pub async fn list_backgrounds(state: State<'_, AppState>) -> CmdResult<Vec<UploadedBackground>> {
    Ok(backgrounds::list(&state.paths.backgrounds()))
}

/// Raw image bytes as the request body; the original file name in the
/// `x-file-name` header (URI-encoded). Raw bytes avoid a slow JSON number array.
#[tauri::command]
pub async fn save_background(request: tauri::ipc::Request<'_>, state: State<'_, AppState>) -> CmdResult<Vec<UploadedBackground>> {
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
pub async fn delete_background(file: String, state: State<'_, AppState>) -> CmdResult<Vec<UploadedBackground>> {
    let dir = state.paths.backgrounds();
    backgrounds::delete(&dir, &file).map_err(|e| format!("{e:#}"))?;
    Ok(backgrounds::list(&dir))
}

/// Absolute path of an uploaded background, for the asset protocol (output window).
#[tauri::command]
pub async fn background_path(file: String, state: State<'_, AppState>) -> CmdResult<Option<String>> {
    Ok(backgrounds::path(&state.paths.backgrounds(), &file))
}
