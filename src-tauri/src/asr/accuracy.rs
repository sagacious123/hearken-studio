//! How well each accuracy-pass model runs on this computer. Speed depends on the machine and
//! its conditions (CPU vs GPU, cores, thermal throttling), so instead of guessing we time each
//! model once on a full spoken sentence and remember the best setting that keeps up.

use std::{
    path::Path,
    time::{Duration, Instant},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::whisper::{DecodeOptions, WhisperEngine};
use crate::{
    audio::SAMPLE_RATE,
    state::{read_json, write_json},
};

/// The accuracy pass must decode a sentence in at most this fraction of its length, leaving
/// room for the live decodes and for longer sentences.
pub const RTF_BUDGET: f32 = 0.5;

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

impl AccuracyProfile {
    /// Saved results, discarded after an app update (whisper.cpp speed changes between versions).
    pub fn load(path: &Path, app_version: &str) -> Self {
        read_json::<Self>(path).filter(|p| p.app_version == app_version).unwrap_or_else(|| Self {
            app_version: app_version.to_owned(),
            entries: vec![],
        })
    }

    pub fn get(&self, model: &str, backend: &str) -> Option<&Calibration> {
        self.entries.iter().find(|c| c.model == model && c.backend == backend)
    }

    /// Store one result (replacing any older one for the same model and backend).
    pub fn record(path: &Path, app_version: &str, cal: Calibration) {
        let mut p = Self::load(path, app_version);
        p.entries.retain(|c| !(c.model == cal.model && c.backend == cal.backend));
        p.entries.push(cal);
        if let Err(e) = write_json(path, &p) {
            log::warn!("couldn't save accuracy calibration: {e:#}");
        }
    }
}

fn time(engine: &mut WhisperEngine, audio: &[f32], opts: DecodeOptions, prompt: &str) -> Result<f32> {
    let t = Instant::now();
    engine.transcribe(audio, opts, prompt)?;
    Ok(Duration::as_secs_f32(&t.elapsed()) * 1000.0)
}

/// Time `engine` on `audio` (a full sentence) and pick the best setting within budget.
/// Cheapest first: if even that can't keep up, nothing else is tried.
pub fn calibrate(engine: &mut WhisperEngine, model: &str, audio: &[f32], prompt: &str) -> Result<Calibration> {
    let audio_ms = audio.len() as f32 * 1000.0 / SAMPLE_RATE as f32;
    let budget = audio_ms * RTF_BUDGET;
    let backend = engine.backend().to_owned();
    let cheapest = DecodeOptions { beam: false, ..DecodeOptions::accurate(false) };
    if backend != "cpu" {
        time(engine, audio, cheapest, prompt)?; // GPU warm-up: first run compiles kernels
    }
    let base_ms = time(engine, audio, cheapest, prompt)?;
    let result = |usable, beam, full_ctx, decode_ms| Calibration { model: model.to_owned(), backend: backend.clone(), usable, beam, full_ctx, decode_ms, audio_ms };
    if base_ms > budget {
        return Ok(result(false, false, false, base_ms));
    }
    // Full context costs roughly 3x on CPU and beam search ~1.5x; skip tries that can't fit.
    for (beam, full_ctx, est) in [(true, true, 3.0), (true, false, 1.3)] {
        if base_ms * est > budget * 1.2 {
            continue;
        }
        let ms = time(engine, audio, DecodeOptions { beam, ..DecodeOptions::accurate(full_ctx) }, prompt)?;
        if ms <= budget {
            return Ok(result(true, beam, full_ctx, ms));
        }
    }
    Ok(result(true, false, false, base_ms))
}

