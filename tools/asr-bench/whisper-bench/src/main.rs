//! Runs a Whisper model over the test clips the way Hearken's live pass does (greedy, the
//! app's English prompt, no context) and writes results/<name>.json for score.py.
//!   whisper-bench <model.bin> <name> <out.json> <threads> [--beam] [--retry] <clip.wav>...
//! --retry re-decodes unsure pieces at a higher temperature, as the app's final pass does.
use std::time::Instant;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const PROMPT: &str = "Sermon transcript with scripture references such as John 3:16, Romans 8:28, \
1 Corinthians 13:4-7, Philippians 4:13, Psalm 23:1, Habakkuk 2:3, Ecclesiastes 3:1, Deuteronomy 31:6, \
Exodus 14:14, Isaiah 40:31, Ezekiel 37:4, Galatians 5:22, Leviticus 19:18, Revelation 21:4.";

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (model, name, out, threads) = (&a[1], &a[2], &a[3], a[4].parse::<i32>().unwrap());
    let beam = a.iter().any(|x| x == "--beam");
    let retry = a.iter().any(|x| x == "--retry");
    let clips: Vec<&String> = a[5..].iter().filter(|x| !x.starts_with("--")).collect();
    let mut p = WhisperContextParameters::default();
    p.use_gpu(false);
    let ctx = WhisperContext::new_with_params(model, p).expect("load model");
    let mut state = ctx.create_state().unwrap();
    let mut results = serde_json::Map::new();
    let (mut secs, mut audio_secs) = (0f64, 0f64);
    for clip in clips {
        let mut r = hound::WavReader::open(clip).unwrap();
        let audio: Vec<f32> = r.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect();
        audio_secs += audio.len() as f64 / 16000.0;
        let mut params = FullParams::new(if beam {
            SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 }
        } else {
            SamplingStrategy::Greedy { best_of: 1 }
        });
        params.set_n_threads(threads);
        params.set_language(Some("en"));
        params.set_no_context(true);
        params.set_suppress_blank(true);
        params.set_temperature(0.0);
        params.set_temperature_inc(if retry { 0.2 } else { 0.0 });
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        params.set_initial_prompt(PROMPT);
        let t = Instant::now();
        state.full(params, &audio).unwrap();
        secs += t.elapsed().as_secs_f64();
        let mut text = String::new();
        for s in 0..state.full_n_segments().unwrap() {
            text.push_str(&state.full_get_segment_text(s).unwrap_or_default());
        }
        let key = std::path::Path::new(clip).file_stem().unwrap().to_string_lossy().to_string();
        eprintln!("{key}: {}", text.trim());
        results.insert(key, serde_json::Value::String(text.trim().to_string()));
    }
    let doc = serde_json::json!({ "model": name, "seconds": secs, "audio_seconds": audio_secs, "out": results });
    std::fs::write(out, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
}
