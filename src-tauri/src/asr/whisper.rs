use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

use crate::audio::SAMPLE_RATE;

pub const GPU_BACKEND: &str = if cfg!(target_os = "macos") { "metal" } else { "vulkan" };

/// Biases the decoder toward Bible vocabulary and toward writing references as
/// "Book C:V" digits, which is far easier (and faster) to parse than spelled-out numbers.
const DOMAIN_PROMPT: &str = "Sermon transcript with scripture references such as John 3:16, Romans 8:28, \
1 Corinthians 13:4-7, Philippians 4:13, Psalm 23:1, Habakkuk 2:3, Ecclesiastes 3:1, Deuteronomy 31:6.";

/// Common Whisper hallucinations on silence, music or applause.
const HALLUCINATIONS: &[&str] = &["thank you.", "thank you", "thanks for watching!", "thank you for watching.", "you", "bye.", "."];

/// Resolve a model id (`tiny.en`, `base.en`) to a file, preferring the q5_1 quantization.
pub fn model_path(models_dir: &Path, model: &str) -> Option<PathBuf> {
    [format!("ggml-{model}-q5_1.bin"), format!("ggml-{model}.bin")]
        .into_iter()
        .map(|f| models_dir.join(f))
        .find(|p| p.exists())
}

#[derive(Debug, Clone, Copy)]
pub struct DecodeOptions {
    /// Partial (in-progress utterance) decodes trade a little accuracy for bounded latency.
    pub partial: bool,
    /// Shrink the encoder context to the audio length instead of the fixed 30 s window.
    /// The single biggest speed-up for short windows on tiny/base (often 3–5x).
    pub reduced_ctx: bool,
}

#[derive(Debug, Clone)]
pub struct Transcript {
    pub text: String,
    /// Mean token probability, 0..1.
    pub confidence: f32,
}

pub struct WhisperEngine {
    _ctx: WhisperContext,
    state: WhisperState,
    threads: i32,
    backend: &'static str,
}

impl WhisperEngine {
    /// Strict load: fails if the requested backend can't initialise.
    pub fn load(path: &Path, gpu: bool, threads: usize) -> Result<Self> {
        let path_str = path.to_str().ok_or_else(|| anyhow!("model path is not UTF-8"))?;
        let mut params = WhisperContextParameters::default();
        params.use_gpu(gpu);
        let ctx = WhisperContext::new_with_params(path_str, params)
            .map_err(|e| anyhow!("loading {}: {e:?}", path.display()))?;
        let state = ctx.create_state().map_err(|e| anyhow!("creating whisper state: {e:?}"))?;
        Ok(Self { _ctx: ctx, state, threads: threads as i32, backend: if gpu { GPU_BACKEND } else { "cpu" } })
    }

    /// GPU first, CPU on failure (missing Vulkan driver, unsupported Metal family, …).
    pub fn load_with_fallback(path: &Path, prefer_gpu: bool, threads: usize) -> Result<Self> {
        if prefer_gpu {
            match Self::load(path, true, threads) {
                Ok(engine) => return Ok(engine),
                Err(e) => log::warn!("{GPU_BACKEND} init failed ({e}); falling back to CPU"),
            }
        }
        Self::load(path, false, threads)
    }

    pub fn backend(&self) -> &'static str {
        self.backend
    }

    pub fn transcribe(&mut self, audio: &[f32], opts: DecodeOptions) -> Result<Transcript> {
        // whisper.cpp rejects < 1 s of audio; pad with silence.
        let min_len = SAMPLE_RATE * 1100 / 1000;
        let padded;
        let audio = if audio.len() < min_len {
            let mut v = audio.to_vec();
            v.resize(min_len, 0.0);
            padded = v;
            &padded[..]
        } else {
            audio
        };

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(self.threads);
        params.set_language(Some("en"));
        params.set_translate(false);
        params.set_no_context(true);
        params.set_single_segment(opts.partial);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_temperature(0.0);
        // Temperature fallback re-decodes on low confidence — great for accuracy, terrible for
        // latency spikes. Only allow it on finals.
        params.set_temperature_inc(if opts.partial { 0.0 } else { 0.2 });
        params.set_initial_prompt(DOMAIN_PROMPT);
        if opts.partial {
            params.set_max_tokens(64);
        }
        if opts.reduced_ctx {
            // 1500 encoder positions == 30 s; scale to the actual audio plus a safety margin.
            let secs = audio.len() as f32 / SAMPLE_RATE as f32;
            let ctx = ((secs / 30.0) * 1500.0).ceil() as i32 + 64;
            params.set_audio_ctx(ctx.clamp(256, 1500));
        }

        self.state.full(params, audio).map_err(|e| anyhow!("whisper decode failed: {e:?}"))?;

        let n_segments = self.state.full_n_segments().map_err(|e| anyhow!("{e:?}"))?;
        let mut text = String::new();
        let mut prob_sum = 0f32;
        let mut prob_n = 0usize;
        for seg in 0..n_segments {
            if let Ok(t) = self.state.full_get_segment_text(seg) {
                text.push_str(&t);
            }
            let n_tokens = self.state.full_n_tokens(seg).unwrap_or(0);
            for tok in 0..n_tokens {
                let tok_text = self.state.full_get_token_text(seg, tok).unwrap_or_default();
                if tok_text.starts_with("[_") || tok_text.starts_with("<|") {
                    continue; // special / timestamp tokens
                }
                if let Ok(p) = self.state.full_get_token_prob(seg, tok) {
                    prob_sum += p;
                    prob_n += 1;
                }
            }
        }
        let confidence = if prob_n == 0 { 0.0 } else { prob_sum / prob_n as f32 };
        let mut text = strip_annotations(&text);
        if confidence < 0.6 && HALLUCINATIONS.contains(&text.to_lowercase().as_str()) {
            text.clear();
        }
        Ok(Transcript { text, confidence })
    }
}

/// Removes "[Music]", "(applause)" and similar non-speech annotations.
fn strip_annotations(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0u32;
    for c in text.chars() {
        match c {
            '[' | '(' => depth += 1,
            ']' | ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::strip_annotations;

    #[test]
    fn strips_non_speech_annotations() {
        assert_eq!(strip_annotations(" [Music] Turn to John 3:16 (applause) "), "Turn to John 3:16");
    }
}
