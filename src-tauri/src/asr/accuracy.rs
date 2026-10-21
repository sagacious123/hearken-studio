//! How well each accuracy-pass model runs on this computer. Speed depends on the machine and
//! its conditions (CPU vs GPU, cores, thermal throttling), so instead of guessing we time each
//! model once on a full spoken sentence and remember the best setting that keeps up.

use std::{
    path::Path,
    time::{Duration, Instant},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::{engine::SpeechEngine, whisper::DecodeOptions};
use crate::{
    audio::SAMPLE_RATE,
    state::{read_json, write_json},
};

/// How fast the accuracy pass must decode a sentence, as a fraction of its length.
/// On a GPU it shares the chip with live decodes, so it must leave half the time free.
pub const RTF_BUDGET_GPU: f32 = 0.5;
/// On CPU it runs on one low-priority thread beside the live decodes and only needs to keep
/// up on average (sermons pause; anything it can't reach keeps the quick text).
pub const RTF_BUDGET_CPU: f32 = 0.85;
/// The speed test runs on a quiet computer; during a service the live decodes share the CPU,
/// which slowed a one-thread accuracy pass by about this much on a 4-core laptop.
const CPU_CONTENTION: f32 = 1.5;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Calibration {
    pub model: String,
    pub backend: String,
    /// Keeps up with speech on this computer.
    pub usable: bool,
    pub beam: bool,
    pub full_ctx: bool,
    /// Time for the chosen setting (or the cheapest, when unusable), and the clip length.
    pub decode_ms: f32,
    pub audio_ms: f32,
}

impl Calibration {
    pub fn options(&self) -> DecodeOptions {
        DecodeOptions { beam: self.beam, ..DecodeOptions::accurate(self.full_ctx) }
    }

    /// "Beam search, full context" etc., for the settings panel.
    pub fn describe(&self) -> String {
        let quality = match (self.beam, self.full_ctx) {
            (true, true) => "highest quality",
            (true, false) => "high quality",
            _ => "standard quality",
        };
        format!("{quality}, {:.1} s per 10 s of speech", self.decode_ms / self.audio_ms * 10.0)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccuracyProfile {
    pub app_version: String,
    pub entries: Vec<Calibration>,
}

/// Bumped when the way the accuracy pass runs changes (threads, priority), so saved speed
/// tests made under the old conditions are redone.
const METHOD: &str = "1t-background";

impl AccuracyProfile {
    /// Saved results, discarded after an app update (whisper.cpp speed changes between
    /// versions) or a change in how the pass runs.
    pub fn load(path: &Path, app_version: &str) -> Self {
        let version = format!("{app_version}+{METHOD}");
        read_json::<Self>(path).filter(|p| p.app_version == version).unwrap_or(Self { app_version: version, entries: vec![] })
    }

    pub fn get(&self, model: &str, backend: &str) -> Option<&Calibration> {
        self.entries.iter().find(|c| c.model == model && c.backend == backend)
    }

    /// Store one result (replacing any older one for the same model and backend).
    pub fn record(path: &Path, app_version: &str, cal: Calibration) {
        let mut p = Self::load(path, app_version); // carries the method-qualified version
        p.entries.retain(|c| !(c.model == cal.model && c.backend == cal.backend));
        p.entries.push(cal);
        if let Err(e) = write_json(path, &p) {
            log::warn!("couldn't save accuracy calibration: {e:#}");
        }
    }
}

fn time(engine: &mut dyn SpeechEngine, audio: &[f32], opts: DecodeOptions, prompt: &str) -> Result<f32> {
    let t = Instant::now();
    engine.transcribe(audio, opts, prompt)?;
    Ok(Duration::as_secs_f32(&t.elapsed()) * 1000.0)
}

/// Time `engine` on `audio` (a full sentence) and pick the best setting within budget.
/// Cheapest first: if even that can't keep up, nothing else is tried.
pub fn calibrate(engine: &mut dyn SpeechEngine, model: &str, audio: &[f32], prompt: &str) -> Result<Calibration> {
    let audio_ms = audio.len() as f32 * 1000.0 / SAMPLE_RATE as f32;
    let backend = engine.backend().to_owned();
    let (budget, contention) = if backend == "cpu" { (audio_ms * RTF_BUDGET_CPU, CPU_CONTENTION) } else { (audio_ms * RTF_BUDGET_GPU, 1.0) };
    let fits = |ms: f32| ms * contention <= budget;
    let cheapest = DecodeOptions { beam: false, ..DecodeOptions::accurate(false) };
    if backend != "cpu" {
        time(engine, audio, cheapest, prompt)?; // GPU warm-up: first run compiles kernels
    }
    let base_ms = time(engine, audio, cheapest, prompt)?;
    let result = |usable, beam, full_ctx, decode_ms| Calibration { model: model.to_owned(), backend: backend.clone(), usable, beam, full_ctx, decode_ms, audio_ms };
    if !fits(base_ms) || !engine.tunable() {
        return Ok(result(fits(base_ms), false, false, base_ms));
    }
    // Full context costs roughly 3x on CPU and beam search ~1.5x; skip tries that can't fit.
    for (beam, full_ctx, est) in [(true, true, 3.0), (true, false, 1.3)] {
        if !fits(base_ms * est / 1.2) {
            continue;
        }
        let ms = time(engine, audio, DecodeOptions { beam, ..DecodeOptions::accurate(full_ctx) }, prompt)?;
        if fits(ms) {
            return Ok(result(true, beam, full_ctx, ms));
        }
    }
    Ok(result(true, false, false, base_ms))
}


