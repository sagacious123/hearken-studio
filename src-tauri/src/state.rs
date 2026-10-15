use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use parking_lot::Mutex;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tauri::{path::BaseDirectory, AppHandle, Emitter, Manager};

use crate::{
    asr::hardware::{self, HardwareProfile},
    history::History,
    pipeline::PipelineHandle,
    presentation::Presenter,
    scripture::{self, parser, parser::ReferenceParser, BibleDb, ScriptureRef, Slide, Versification},
    share::Share,
    songs::SongLibrary,
};

/// Detections kept for the phone remote's list.
pub const RECENT_SLIDES: usize = 30;

/// An extra screen beside the main output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ScreenConfig {
    /// "full" (the main look), "lower-third" (livestream / video switcher) or "stage"
    /// (confidence monitor for the preacher: large text and a clock).
    pub look: String,
    /// Lower third background: "black", "green" (for chroma keying) or "theme".
    pub key: String,
    /// Display index, as listed by the Output menu; `None` picks one.
    pub monitor: Option<usize>,
}

impl Default for ScreenConfig {
    fn default() -> Self {
        Self { look: "stage".into(), key: "black".into(), monitor: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Translation code to display (e.g. "KJV").
    pub translation: String,
    /// Send high-confidence detections straight to the output window.
    pub auto_project: bool,
    pub auto_threshold: f32,
    /// `None` = system default input.
    pub input_device: Option<String>,
    /// Overrides the benchmarked model ("tiny.en" / "base.en").
    pub model_override: Option<String>,
    pub gpu_override: Option<bool>,
    /// Shrink whisper's encoder context to the window length (big speed-up, slight accuracy cost).
    pub reduced_encoder_ctx: bool,
    /// Layer-2 detection of verses read aloud without an announcement.
    pub quote_detection: bool,
    /// Suggest verses said in other words (needs the search-by-meaning model).
    pub meaning_detection: bool,
    /// Spoken commands for the live scripture: "next verse", "go back", "go to the first verse".
    pub voice_commands: bool,
    /// Second translation shown side by side ("WEB" under "KJV", or "FRJ" for a French service).
    pub secondary_translation: Option<String>,
    /// Highlight the live passage as it is read and move to the next verse automatically.
    pub follow_reading: bool,
    /// Recognise imported worship songs from the singing and follow them section by section.
    pub lyrics_detection: bool,
    /// Serve the phone view, OBS overlay and phone remote on the local network.
    pub share_enabled: bool,
    /// Fallback port when 80 isn't available.
    pub share_port: u16,
    /// Friendly network name: phones open http://<name>.local.
    pub share_name: String,
    /// Heading on sermon recaps.
    pub church_name: String,
    /// What congregation phones can open from "Recap": "notes" (scriptures, songs and sermon
    /// notes), "scriptures" (without the notes) or "off".
    pub phone_recap: String,
    /// Accuracy pass: "auto", "off", or a model id ("small.en", "large-v3-turbo").
    pub accuracy_model: String,
    /// Names and words to listen for (pastors, the church, places), comma or line separated.
    pub vocabulary: String,
    /// Extra screens beside the main output (up to three).
    pub screens: Vec<ScreenConfig>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            translation: "KJV".into(),
            auto_project: false,
            auto_threshold: 0.9,
            input_device: None,
            model_override: None,
            gpu_override: None,
            reduced_encoder_ctx: true,
            quote_detection: true,
            meaning_detection: true,
            voice_commands: true,
            secondary_translation: None,
            follow_reading: true,
            lyrics_detection: true,
            share_enabled: false,
            share_port: 7878,
            share_name: "hearken".into(),
            church_name: String::new(),
            phone_recap: "notes".into(),
            accuracy_model: "auto".into(),
            vocabulary: String::new(),
            screens: Vec::new(),
        }
    }
}

pub struct Paths {
    pub bible_db: PathBuf,
    pub models_dir: PathBuf,
    pub bench_wav: PathBuf,
    pub config_dir: PathBuf,
}

impl Paths {
    pub fn settings(&self) -> PathBuf {
        self.config_dir.join("settings.json")
    }
    pub fn hardware(&self) -> PathBuf {
        self.config_dir.join("hardware-profile.json")
    }
    pub fn theme(&self) -> PathBuf {
        self.config_dir.join("theme.json")
    }
    /// Translations downloaded from the Bible library.
    pub fn library(&self) -> PathBuf {
        self.config_dir.join("library.db")
    }
    pub fn history(&self) -> PathBuf {
        self.config_dir.join("history")
    }
    pub fn songs(&self) -> PathBuf {
        self.config_dir.join("songs.json")
    }
    pub fn plan(&self) -> PathBuf {
        self.config_dir.join("sermon-plan.txt")
    }
    pub fn backgrounds(&self) -> PathBuf {
        self.config_dir.join("backgrounds")
    }
    /// Speed tests of accuracy-pass models on this computer.
    pub fn accuracy_profile(&self) -> PathBuf {
        self.config_dir.join("accuracy-profile.json")
    }
    /// Models downloaded in the app (the accuracy-pass ones).
    pub fn downloaded_models(&self) -> PathBuf {
        self.config_dir.join("models")
    }
}

/// The pastor's notes for this service and the references found in them.
#[derive(Default)]
pub struct Plan {
    pub text: String,
    pub refs: Vec<ScriptureRef>,
}

impl Plan {
    pub fn parse(text: &str, v11n: &Versification) -> Self {
        let mut refs: Vec<ScriptureRef> = Vec::new();
        for line in text.lines() {
            // Fresh parser per line: notes shouldn't chain "verse 18" across unrelated lines.
            for r in ReferenceParser::new().parse(line, true, v11n) {
                if !refs.contains(&r.reference) {
                    refs.push(r.reference);
                }
            }
        }
        Self { text: text.to_owned(), refs }
    }

    /// Whether a detected reference is one the pastor planned (same passage, overlapping verses).
    pub fn contains(&self, r: &ScriptureRef) -> bool {
        self.refs.iter().any(|p| {
            p.book_id == r.book_id
                && p.chapter == r.chapter
                && match (p.verse_start, r.verse_start) {
                    (Some(ps), Some(rs)) => ps <= r.verse_end.unwrap_or(rs) && rs <= p.verse_end.unwrap_or(ps),
                    _ => true,
                }
        })
    }
}

pub struct AppState {
    pub paths: Paths,
    pub settings: Mutex<Settings>,
    /// Connection for operator commands; the pipeline thread opens its own.
    pub bible: Mutex<BibleDb>,
    pub versification: Arc<Versification>,
    pub hardware: Mutex<Option<HardwareProfile>>,
    pub pipeline: Mutex<Option<PipelineHandle>>,
    pub presenter: Presenter,
    pub history: Arc<History>,
    pub songs: SongLibrary,
    pub plan: Mutex<Plan>,
    /// Latest confirmed detections, newest first (phone remote).
    pub recent: Mutex<VecDeque<Slide>>,
    pub share: Share,
    /// Cancel flags of model downloads in progress, by model id.
    pub downloads: Mutex<std::collections::HashMap<String, Arc<std::sync::atomic::AtomicBool>>>,
    /// Search by meaning, once its model is downloaded and loaded.
    pub meaning: Mutex<Option<Arc<scripture::meaning::Meaning>>>,
    /// The free trial or the church's licence.
    pub licence: crate::licence::Licensing,
}

impl AppState {
    pub fn initialize(app: &AppHandle) -> Result<Self> {
        let resource = |p: &str| app.path().resolve(p, BaseDirectory::Resource);
        let config_dir = app.path().app_config_dir()?;
        migrate_legacy_config(&config_dir);
        std::fs::create_dir_all(&config_dir)?;
        let paths = Paths {
            bible_db: resource("resources/bible.db")?,
            models_dir: resource("resources/models")?,
            bench_wav: resource("resources/bench/jfk.wav")?,
            config_dir,
        };

        // Before any Bible database opens, so every reader attaches the library.
        if let Err(e) = scripture::library::ensure(&paths.library()) {
            log::warn!("Bible library unavailable: {e:#}");
        }
        scripture::db::set_library_path(paths.library());
        let bible = BibleDb::open(&paths.bible_db)?;
        let versification = Arc::new(bible.versification()?);
        parser::warm_up();

        let mut settings: Settings = read_json(&paths.settings()).unwrap_or_default();
        if !bible.translations().iter().any(|t| t.code.eq_ignore_ascii_case(&settings.translation)) {
            settings.translation = bible.translations()[0].code.clone(); // a removed download
        }
        let hardware = hardware::load_cached(&paths.hardware(), env!("CARGO_PKG_VERSION"));
        if settings.secondary_translation.as_ref().is_some_and(|s| !bible.translations().iter().any(|t| t.code.eq_ignore_ascii_case(s))) {
            settings.secondary_translation = None;
        }
        let history = Arc::new(History::new(paths.history()));
        let presenter = Presenter::new(read_json(&paths.theme()), history.clone());
        let songs = SongLibrary::load(paths.songs());
        let plan = Plan::parse(&std::fs::read_to_string(paths.plan()).unwrap_or_default(), &versification);
        let licence = crate::licence::Licensing::load(&paths.config_dir, app.path().app_local_data_dir().ok().as_deref(), paths.history());

        Ok(Self {
            paths,
            settings: Mutex::new(settings),
            bible: Mutex::new(bible),
            versification,
            hardware: Mutex::new(hardware),
            pipeline: Mutex::new(None),
            presenter,
            history,
            songs,
            plan: Mutex::new(plan),
            recent: Mutex::new(VecDeque::new()),
            share: Share::new(),
            downloads: Mutex::new(Default::default()),
            meaning: Mutex::new(None),
            licence,
        })
    }

    /// Registers the trial, or settles a clock that looks wrong, if the internet is there.
    /// Quietly does nothing otherwise: the trial runs offline and this is tried next start.
    pub fn check_licence(app: &AppHandle) {
        let app = app.clone();
        std::thread::spawn(move || {
            use crate::licence::Status;
            let state = app.state::<AppState>();
            if matches!(state.licence.status(), Status::Trial { registered: false, .. } | Status::ClockBehind { .. }) {
                match state.licence.check_online() {
                    Ok(status) => {
                        let _ = app.emit(crate::events::LICENCE, status);
                    }
                    Err(e) => log::info!("licence check skipped: {e}"),
                }
            }
        });
    }

    /// Load search by meaning in the background when its model is downloaded (well under a
    /// second in release builds).
    pub fn load_meaning(app: &AppHandle) {
        let app = app.clone();
        std::thread::spawn(move || {
            let state = app.state::<AppState>();
            let dir = state.paths.downloaded_models();
            if !scripture::meaning::installed(&dir) {
                return;
            }
            let loaded = BibleDb::open(&state.paths.bible_db).and_then(|db| scripture::meaning::Meaning::load(&dir, &db));
            match loaded {
                Ok(m) => *state.meaning.lock() = Some(Arc::new(m)),
                Err(e) => log::warn!("search by meaning unavailable: {e:#}"),
            }
        });
    }

    /// Remember a confirmed detection for the phone remote (newest first, no duplicates).
    pub fn remember(&self, slide: &Slide) {
        let mut recent = self.recent.lock();
        recent.retain(|s| s.id != slide.id);
        recent.push_front(slide.clone());
        recent.truncate(RECENT_SLIDES);
    }

    pub fn save_settings(&self) -> Result<()> {
        write_json(&self.paths.settings(), &*self.settings.lock())
    }
}

/// The app was called ScriptureBeam before. Its data folder is copied (not moved, so it stays
/// as a backup) the first time the new one is created, keeping settings, songs, the sermon plan
/// and service history across the rename.
const LEGACY_IDENTIFIER: &str = "com.scripturebeam.app";

fn migrate_legacy_config(config_dir: &Path) {
    let Some(legacy) = config_dir.parent().map(|p| p.join(LEGACY_IDENTIFIER)) else { return };
    if config_dir.exists() || !legacy.is_dir() {
        return;
    }
    match copy_dir(&legacy, config_dir) {
        Ok(()) => log::info!("copied data from {}", legacy.display()),
        Err(e) => log::warn!("couldn't copy data from {}: {e}", legacy.display()),
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    // Write-then-rename so a crash mid-write never leaves corrupt settings.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_from_the_old_name_is_copied_once() {
        let root = std::env::temp_dir().join(format!("sb-migrate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let legacy = root.join(LEGACY_IDENTIFIER);
        std::fs::create_dir_all(legacy.join("history")).unwrap();
        std::fs::write(legacy.join("settings.json"), "{}").unwrap();
        std::fs::write(legacy.join("history/2026-10-11.jsonl"), "x").unwrap();

        let new = root.join("com.hearkenstudio.app");
        migrate_legacy_config(&new);
        assert!(new.join("settings.json").exists());
        assert!(new.join("history/2026-10-11.jsonl").exists());
        assert!(legacy.join("settings.json").exists(), "old data is kept as a backup");

        // Never overwrites data the new app has already written.
        std::fs::write(new.join("settings.json"), "new").unwrap();
        migrate_legacy_config(&new);
        assert_eq!(std::fs::read_to_string(new.join("settings.json")).unwrap(), "new");
        let _ = std::fs::remove_dir_all(&root);
    }
}
