//! Startup hardware tiering: static spec detection plus a short real inference benchmark
//! that picks the best Whisper model / backend that fits the latency budget.

use std::{
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};

use super::whisper::{model_path, DecodeOptions, WhisperEngine, GPU_BACKEND};
use crate::audio::SAMPLE_RATE;

/// Longest audio window a partial decode sees. The benchmark times exactly this, so the
/// measured budget is the worst case the pipeline will hit.
pub const PARTIAL_WINDOW_MS: u32 = 5_000;
const BENCH_WINDOW_MS: u32 = PARTIAL_WINDOW_MS;
/// A partial decode must finish within this for a model to be "real-time" at a ~250 ms step.
const BUDGET_MS: f32 = 200.0;
/// Below this on `base.en` we call the machine "high" tier.
const HIGH_TIER_MS: f32 = 110.0;
const MIN_STEP_MS: u32 = 200;
const MAX_STEP_MS: u32 = 1_500;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Low,
    Mid,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Specs {
    pub os: String,
    pub arch: String,
    pub total_ram_gb: f32,
    pub logical_cores: usize,
    pub apple_silicon: bool,
}

impl Specs {
    pub fn detect() -> Self {
        let mut sys = sysinfo::System::new();
        sys.refresh_memory();
        Self {
            os: std::env::consts::OS.into(),
            arch: std::env::consts::ARCH.into(),
            total_ram_gb: sys.total_memory() as f32 / 1024f32.powi(3),
            logical_cores: std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4),
            apple_silicon: cfg!(all(target_os = "macos", target_arch = "aarch64")),
        }
    }

    /// whisper.cpp scales with *performance* cores; oversubscribing hurts latency.
    pub fn recommended_threads(&self) -> usize {
        if self.apple_silicon {
            self.logical_cores.saturating_sub(4).clamp(4, 8)
        } else {
            (self.logical_cores / 2).clamp(2, 8) // assume SMT on x86
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchResult {
    pub model: String,
    pub backend: String,
    pub ok: bool,
    pub load_ms: f32,
    pub infer_ms: f32,
    pub audio_ms: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareProfile {
    pub app_version: String,
    pub specs: Specs,
    pub tier: Tier,
    pub model: String,
    pub use_gpu: bool,
    pub threads: usize,
    /// Initial partial-decode cadence; the pipeline adapts it at runtime.
    pub step_ms: u32,
    /// `benchmark` or `heuristic` (no benchmark clip available).
    pub method: String,
    pub benchmarks: Vec<BenchResult>,
}

pub fn load_cached(path: &Path, app_version: &str) -> Option<HardwareProfile> {
    let profile: HardwareProfile = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    // Re-benchmark after upgrades: whisper.cpp performance changes between releases.
    (profile.app_version == app_version).then_some(profile)
}

pub fn save(path: &Path, profile: &HardwareProfile) -> Result<()> {
    std::fs::write(path, serde_json::to_vec_pretty(profile)?)?;
    Ok(())
}

pub fn run_benchmark(
    models_dir: &Path,
    bench_wav: &Path,
    app_version: &str,
    reduced_ctx: bool,
    progress: impl Fn(String),
) -> Result<HardwareProfile> {
    let specs = Specs::detect();
    let threads = specs.recommended_threads();
    progress(format!(
        "{} {} · {:.0} GB RAM · {} cores",
        specs.os, specs.arch, specs.total_ram_gb, specs.logical_cores
    ));

    let audio = match load_bench_audio(bench_wav) {
        Ok(a) => a,
        Err(e) => {
            log::warn!("benchmark clip unavailable ({e}); using heuristic tiering");
            return heuristic(specs, models_dir, app_version);
        }
    };

    // Low-RAM machines never try base.en (≈ 2x the working set of tiny).
    let candidates: &[&str] = if specs.total_ram_gb >= 6.0 { &["base.en", "tiny.en"] } else { &["tiny.en"] };
    let mut results = Vec::new();

    for &model in candidates {
        let Some(path) = model_path(models_dir, model) else { continue };
        // Intel Macs: Metal measured ~2.5x slower than CPU for whisper and its first load spends
        // ~40 s compiling shaders, so don't bother benchmarking it there.
        let gpu_options: &[bool] = if cfg!(target_os = "macos") && !specs.apple_silicon { &[false] } else { &[true, false] };
        for &gpu in gpu_options {
            let backend = if gpu { GPU_BACKEND } else { "cpu" };
            progress(format!("Benchmarking {model} on {backend}…"));
            let r = bench_one(&path, model, gpu, threads, &audio, reduced_ctx);
            log::info!("bench {model}/{backend}: {r:?}");
            let passed = r.ok && r.infer_ms <= BUDGET_MS;
            results.push(r);
            if passed {
                return Ok(build_profile(specs, model, gpu, threads, results, "benchmark", app_version));
            }
        }
    }

    // Nothing met the budget: take the fastest working config and slow the step to match.
    let Some(best) = results.iter().filter(|r| r.ok).min_by(|a, b| a.infer_ms.total_cmp(&b.infer_ms)).cloned()
    else {
        bail!("no Whisper model could be loaded — run scripts/fetch_models.sh");
    };
    Ok(build_profile(specs, &best.model, best.backend != "cpu", threads, results, "benchmark", app_version))
}

fn bench_one(path: &Path, model: &str, gpu: bool, threads: usize, audio: &[f32], reduced_ctx: bool) -> BenchResult {
    let backend = if gpu { GPU_BACKEND } else { "cpu" };
    let fail = |e: String| BenchResult {
        model: model.into(),
        backend: backend.into(),
        ok: false,
        load_ms: 0.0,
        infer_ms: f32::INFINITY,
        audio_ms: BENCH_WINDOW_MS,
        error: Some(e),
    };

    let t = Instant::now();
    let mut engine = match WhisperEngine::load(path, gpu, threads) {
        Ok(e) => e,
        Err(e) => return fail(e.to_string()),
    };
    let load_ms = ms(t.elapsed());
    let opts = DecodeOptions::live(true, reduced_ctx);

    // Warm-up: the first Metal/Vulkan run compiles pipelines and is not representative.
    if let Err(e) = engine.transcribe(audio, opts, "") {
        return fail(e.to_string());
    }
    let mut times = Vec::with_capacity(3);
    for _ in 0..3 {
        let t = Instant::now();
        if let Err(e) = engine.transcribe(audio, opts, "") {
            return fail(e.to_string());
        }
        times.push(ms(t.elapsed()));
    }
    times.sort_by(f32::total_cmp);

    BenchResult {
        model: model.into(),
        backend: backend.into(),
        ok: true,
        load_ms,
        infer_ms: times[1], // median
        audio_ms: BENCH_WINDOW_MS,
        error: None,
    }
}

fn build_profile(
    specs: Specs,
    model: &str,
    use_gpu: bool,
    threads: usize,
    benchmarks: Vec<BenchResult>,
    method: &str,
    app_version: &str,
) -> HardwareProfile {
    let chosen = benchmarks.iter().find(|b| b.ok && b.model == model && (b.backend != "cpu") == use_gpu);
    let infer = chosen.map(|b| b.infer_ms).unwrap_or(BUDGET_MS);
    let tier = match (model, infer) {
        ("base.en", t) if t <= HIGH_TIER_MS => Tier::High,
        ("base.en", _) => Tier::Mid,
        _ => Tier::Low,
    };
    HardwareProfile {
        app_version: app_version.into(),
        specs,
        tier,
        model: model.into(),
        use_gpu,
        threads,
        step_ms: ((infer * 1.25) as u32).clamp(MIN_STEP_MS, MAX_STEP_MS),
        method: method.into(),
        benchmarks,
    }
}

/// Spec-only fallback when the benchmark clip is missing.
fn heuristic(specs: Specs, models_dir: &Path, app_version: &str) -> Result<HardwareProfile> {
    let strong = specs.apple_silicon || (specs.total_ram_gb >= 16.0 && specs.logical_cores >= 8);
    let model = if strong && model_path(models_dir, "base.en").is_some() { "base.en" } else { "tiny.en" };
    ensure!(model_path(models_dir, model).is_some(), "model {model} missing — run scripts/fetch_models.sh");
    let threads = specs.recommended_threads();
    let gpu = !(cfg!(target_os = "macos") && !specs.apple_silicon);
    let mut p = build_profile(specs, model, gpu, threads, vec![], "heuristic", app_version);
    p.step_ms = if strong { 300 } else { 600 };
    Ok(p)
}

fn load_bench_audio(path: &Path) -> Result<Vec<f32>> {
    let samples = load_wav(path)?;
    let n = (SAMPLE_RATE * BENCH_WINDOW_MS as usize / 1000).min(samples.len());
    ensure!(n >= SAMPLE_RATE, "benchmark clip too short");
    Ok(samples[..n].to_vec())
}

/// The whole 16 kHz mono clip (the accuracy pass is calibrated on a full sentence).
pub fn load_wav(path: &Path) -> Result<Vec<f32>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    ensure!(spec.sample_rate as usize == SAMPLE_RATE && spec.channels == 1, "benchmark clip must be 16 kHz mono");
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => {
            reader.samples::<i16>().map(|s| s.map(|v| v as f32 / 32768.0)).collect::<Result<_, _>>()?
        }
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
    };
    Ok(samples)
}

fn ms(d: Duration) -> f32 {
    d.as_secs_f32() * 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `cargo test --release -- --ignored --nocapture` after `scripts/fetch_models.sh`.
    #[test]
    #[ignore = "needs models from scripts/fetch_models.sh"]
    fn benchmark_and_transcribe_on_this_machine() {
        let res = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
        let profile = run_benchmark(&res.join("models"), &res.join("bench/jfk.wav"), "test", true, |m| eprintln!("{m}"))
            .expect("benchmark");
        eprintln!("{}", serde_json::to_string_pretty(&profile).unwrap());

        let path = model_path(&res.join("models"), &profile.model).unwrap();
        let mut engine = WhisperEngine::load_with_fallback(&path, profile.use_gpu, profile.threads).unwrap();
        let audio = load_bench_audio(&res.join("bench/jfk.wav")).unwrap();
        let t = Instant::now();
        let out = engine.transcribe(&audio, DecodeOptions::live(false, true), "").unwrap();
        eprintln!("final decode {:.0} ms ({:.2} conf): {}", ms(t.elapsed()), out.confidence, out.text);
        assert!(out.text.to_lowercase().contains("ask not"), "unexpected transcript: {}", out.text);
    }

    /// Accuracy-pass cost per model and backend on a whole 11 s sentence (the clip), as used
    /// for finished sentences. Looks in the bundled models and the app's downloads folder.
    /// `cargo test --release -- --ignored --nocapture accuracy_pass_speed`
    #[test]
    #[ignore = "needs downloaded models"]
    fn accuracy_pass_speed() {
        let res = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources");
        let downloads = std::env::var("HOME").map(|h| Path::new(&h).join("Library/Application Support/com.hearkenstudio.app/models")).unwrap();
        let dirs = [res.join("models"), downloads];
        let dirs: Vec<&Path> = dirs.iter().map(|d| d.as_path()).collect();
        let mut reader = hound::WavReader::open(res.join("bench/jfk.wav")).unwrap();
        let audio: Vec<f32> = reader.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect();
        let threads = Specs::detect().recommended_threads();
        let prompt = super::super::whisper::accurate_prompt("", &[], &[crate::scripture::lang::Lang::En]);
        let variants = [("beam+full", true, true), ("beam+fit", true, false), ("greedy+full", false, true), ("greedy+fit", false, false)];
        for id in ["base.en", "small.en", "large-v3-turbo"] {
            let Some(path) = super::super::models::find(&dirs, id) else { eprintln!("{id}: not installed"); continue };
            let Ok(mut engine) = WhisperEngine::load(&path, false, threads) else { eprintln!("{id}: load failed"); continue };
            for (name, beam, full) in variants {
                let opts = DecodeOptions { beam, ..DecodeOptions::accurate(full) };
                let _ = engine.transcribe(&audio, opts, &prompt); // warm-up
                let t = Instant::now();
                let out = engine.transcribe(&audio, opts, &prompt).unwrap();
                eprintln!("{id:15} cpu {name:12} {:6.0} ms for {:.1} s  {}", ms(t.elapsed()), audio.len() as f32 / 16000.0, out.text);
            }
        }
    }
}
