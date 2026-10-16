use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperState};

use crate::{audio::SAMPLE_RATE, scripture::lang::Lang};

pub const GPU_BACKEND: &str = if cfg!(target_os = "macos") { "metal" } else { "vulkan" };

/// Biases the decoder toward Bible vocabulary and toward writing references as "Book C:V"
/// digits, which is far easier (and faster) to parse than spelled-out numbers. One sentence per
/// preaching language, so none of them reads as a translation request.
fn domain_prompt(langs: &[Lang]) -> String {
    langs.iter().map(|l| l.prompt()).collect::<Vec<_>>().join(" ")
}

/// Whisper keeps only the last ~224 prompt tokens; this keeps us comfortably inside that.
const MAX_PROMPT_CHARS: usize = 820;
const MAX_VOCAB_CHARS: usize = 300;

/// Church-specific words ("Pastor Adeyemi, Ebenezer, Ikeja") from the operator, tidied.
fn vocabulary(words: &str) -> String {
    let terms: Vec<&str> = words.split([',', '\n', ';']).map(str::trim).filter(|t| !t.is_empty()).collect();
    let mut out = terms.join(", ");
    if out.len() > MAX_VOCAB_CHARS {
        out.truncate(out[..MAX_VOCAB_CHARS].rfind(", ").unwrap_or(MAX_VOCAB_CHARS));
    }
    out
}

/// Prompt for the live (fast) decodes: short, because every prompt token costs time on each
/// partial. The operator's own vocabulary goes last, where Whisper weighs it most.
pub fn live_prompt(vocab: &str, langs: &[Lang]) -> String {
    match vocabulary(vocab) {
        v if v.is_empty() => domain_prompt(langs),
        v => format!("{} {v}.", domain_prompt(langs)),
    }
}

/// Prompt for the accuracy pass: every book name in each language (so "Philippians" isn't
/// heard as "Philippines"), today's planned passages, then the operator's vocabulary.
pub fn accurate_prompt(vocab: &str, planned: &[String], langs: &[Lang]) -> String {
    let mut parts: Vec<String> = langs.iter().map(|l| format!("{} {}.", l.books_heading(), l.book_names().join(", "))).collect();
    if !planned.is_empty() {
        let heading = langs.first().unwrap_or(&Lang::En).readings_heading();
        parts.push(format!("{heading} {}.", planned.iter().take(12).cloned().collect::<Vec<_>>().join(", ")));
    }
    let v = vocabulary(vocab);
    if !v.is_empty() {
        parts.push(format!("{v}."));
    }
    let mut prompt = parts.join(" ");
    if prompt.len() > MAX_PROMPT_CHARS {
        // Drop from the front: the end (readings, vocabulary) matters most.
        let cut = prompt.len() - MAX_PROMPT_CHARS;
        let start = prompt[cut..].find(' ').map_or(cut, |i| cut + i + 1);
        prompt = prompt[start..].to_owned();
    }
    prompt
}

/// Resolve a model id (`tiny.en`, `base.en`, …) to a file in `models_dir`.
pub fn model_path(models_dir: &Path, model: &str) -> Option<PathBuf> {
    super::models::find(&[models_dir], model)
}

#[derive(Debug, Clone, Copy)]
pub struct DecodeOptions {
    /// Partial (in-progress utterance) decodes trade a little accuracy for bounded latency.
    pub partial: bool,
    /// Shrink the encoder context to the audio length instead of the fixed 30 s window.
    /// The single biggest speed-up for short windows on tiny/base (often 3–5x).
    pub reduced_ctx: bool,
    /// Beam search (5 candidates) instead of greedy decoding. Accuracy pass only.
    pub beam: bool,
}

impl DecodeOptions {
    pub const fn live(partial: bool, reduced_ctx: bool) -> Self {
        Self { partial, reduced_ctx, beam: false }
    }

    /// The accuracy pass over a finished sentence. `full_ctx` gives the encoder the whole 30 s
    /// window (best quality, costly on CPU); otherwise it's sized to the sentence.
    pub const fn accurate(full_ctx: bool) -> Self {
        Self { partial: false, reduced_ctx: !full_ctx, beam: true }
    }
}

#[derive(Debug, Clone)]
pub struct Transcript {
    pub text: String,
    /// Mean token probability, 0..1.
    pub confidence: f32,
    /// The language it was heard in, when listening for more than one.
    pub language: Option<Lang>,
    /// Heard as a language that isn't being listened for.
    stray: bool,
}

pub struct WhisperEngine {
    _ctx: WhisperContext,
    state: WhisperState,
    threads: i32,
    backend: &'static str,
    /// The preaching languages. One is decoded as that language; several are told apart by
    /// the model, sentence by sentence.
    languages: Vec<Lang>,
    /// The language of the last finished sentence. Partials are decoded in it: telling
    /// languages apart costs a second encoder pass, too slow for every partial.
    last_heard: Option<Lang>,
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
        Ok(Self { _ctx: ctx, state, threads: threads as i32, backend: if gpu { GPU_BACKEND } else { "cpu" }, languages: vec![Lang::En], last_heard: None })
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

    /// Listen for `langs` (needs a multilingual model unless it's only English).
    pub fn set_languages(&mut self, langs: &[Lang]) {
        if !langs.is_empty() {
            self.languages = langs.to_vec();
        }
    }

    /// Decode `audio`. With several languages the model picks one for each finished sentence
    /// (one it hears as some other language, which an accent can do, is decoded again as the
    /// first); partials keep the language of the sentence before.
    pub fn transcribe(&mut self, audio: &[f32], opts: DecodeOptions, prompt: &str) -> Result<Transcript> {
        let langs = self.languages.clone();
        let forced = match langs.as_slice() {
            [only] => Some(*only),
            _ if opts.partial => Some(self.last_heard.unwrap_or(langs[0])),
            _ => None,
        };
        let mut t = self.decode(audio, opts, prompt, forced)?;
        if t.stray && !opts.partial {
            t = self.decode(audio, opts, prompt, Some(langs[0]))?;
        }
        if !opts.partial && t.language.is_some() {
            self.last_heard = t.language;
        }
        Ok(t)
    }

    fn decode(&mut self, audio: &[f32], opts: DecodeOptions, prompt: &str, language: Option<Lang>) -> Result<Transcript> {
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

        let strategy = if opts.beam {
            SamplingStrategy::BeamSearch { beam_size: 5, patience: -1.0 }
        } else {
            SamplingStrategy::Greedy { best_of: 1 }
        };
        let mut params = FullParams::new(strategy);
        params.set_n_threads(self.threads);
        params.set_language(Some(language.map_or("auto", Lang::code)));
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
        let default_prompt;
        params.set_initial_prompt(if prompt.is_empty() {
            default_prompt = domain_prompt(&self.languages);
            &default_prompt
        } else {
            prompt
        });
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
        let lower = text.to_lowercase();
        if confidence < 0.6 && self.languages.iter().any(|l| l.hallucinations().contains(&lower.as_str())) {
            text.clear();
        }
        let heard = match language {
            Some(l) => Ok(l),
            None => match self.state.full_lang_id_from_state().ok().and_then(whisper_rs::get_lang_str) {
                Some(code) => Lang::from_code(code).filter(|l| self.languages.contains(l)).ok_or(code),
                None => Err("?"),
            },
        };
        Ok(Transcript { text, confidence, language: heard.ok(), stray: heard.is_err_and(|code| code != "?") })
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
    use super::*;

    #[test]
    fn prompts_carry_vocabulary_and_stay_short() {
        let en = [Lang::En];
        assert_eq!(live_prompt("  ", &en), Lang::En.prompt());
        assert!(live_prompt("Pastor Adeyemi,\nEbenezer ; Ikeja", &en).ends_with("Pastor Adeyemi, Ebenezer, Ikeja."));
        let p = accurate_prompt("Pastor Adeyemi", &["Romans 8:28".into()], &en);
        assert!(p.len() <= MAX_PROMPT_CHARS, "{}", p.len());
        assert!(p.ends_with("Today's readings: Romans 8:28. Pastor Adeyemi."), "{p}");
        assert!(p.contains("Philippians"));
        // In French: the books' French names, and the end (readings, vocabulary) kept when cut.
        let p = accurate_prompt("Pasteur Kouassi", &["Jean 3:16".into()], &[Lang::Fr]);
        assert!(p.starts_with("Prédication. Livres de la Bible : Genèse, Exode"), "{p}");
        assert!(p.ends_with("Lectures du jour : Jean 3:16. Pasteur Kouassi."), "{p}");
        let p = accurate_prompt("", &[], &[Lang::En, Lang::Es]);
        assert!(p.len() <= MAX_PROMPT_CHARS && p.contains("Apocalipsis"), "{p}");
        assert!(live_prompt("", &[Lang::En, Lang::Fr]).contains("Romains 8:28"));
        let long = "word, ".repeat(200);
        assert!(vocabulary(&long).len() <= MAX_VOCAB_CHARS);
    }

    /// Transcribe 16 kHz mono WAVs in some languages and show what the detector makes of them:
    /// `HEARKEN_MODEL=…/ggml-base-q5_1.bin HEARKEN_LANGS=fr HEARKEN_WAV=a.wav,b.wav cargo test transcribe_files -- --ignored --nocapture`
    #[test]
    #[ignore = "needs a model and recordings"]
    fn transcribe_files() {
        use crate::scripture::{parser::ReferenceParser, BibleDb};
        let model = std::env::var("HEARKEN_MODEL").expect("HEARKEN_MODEL");
        let codes: Vec<String> = std::env::var("HEARKEN_LANGS").unwrap_or_else(|_| "en".into()).split(',').map(String::from).collect();
        let langs = Lang::list(&codes);
        let threads = super::super::hardware::Specs::detect().recommended_threads();
        let mut engine = WhisperEngine::load(Path::new(&model), false, threads).unwrap();
        engine.set_languages(&langs);
        let v11n = BibleDb::open(&Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/bible.db")).unwrap().versification().unwrap();
        let mut parser = ReferenceParser::for_languages(&langs);
        for wav in std::env::var("HEARKEN_WAV").expect("HEARKEN_WAV").split(',') {
            let audio = super::super::hardware::load_wav(Path::new(wav)).unwrap();
            let t0 = std::time::Instant::now();
            let out = engine.transcribe(&audio, DecodeOptions::live(false, true), &live_prompt("", &langs)).unwrap();
            eprintln!("{wav}: {:.0} ms, heard as {:?} ({:.2}): {}", t0.elapsed().as_secs_f32() * 1000.0, out.language, out.confidence, out.text);
            let refs: Vec<_> = parser.parse(&out.text, true, &v11n).into_iter().map(|r| (r.reference.label(), (r.confidence * 100.0).round())).collect();
            eprintln!("  references: {refs:?}");
            eprintln!("  command: {:?}", crate::voice::parse_in(&out.text, &langs));
        }
    }

    #[test]
    fn strips_non_speech_annotations() {
        assert_eq!(strip_annotations(" [Music] Turn to John 3:16 (applause) "), "Turn to John 3:16");
    }
}
