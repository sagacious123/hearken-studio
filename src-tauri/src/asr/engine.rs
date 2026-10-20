//! A speech-to-text engine. Whisper (whisper.cpp) runs the live pass and can run the accuracy
//! pass; Parakeet (NVIDIA's model, through sherpa-onnx) runs the accuracy pass. The pipeline
//! only sees this trait.

use std::path::Path;

use anyhow::Result;

use super::{
    parakeet::ParakeetEngine,
    whisper::{DecodeOptions, Transcript, WhisperEngine},
};
use crate::scripture::lang::Lang;

pub trait SpeechEngine {
    /// Decode a sentence (or, live, the newest few seconds of one). `prompt` steers Whisper
    /// toward Bible vocabulary; engines without prompts ignore it.
    fn transcribe(&mut self, audio: &[f32], opts: DecodeOptions, prompt: &str) -> Result<Transcript>;
    /// "metal", "vulkan" or "cpu".
    fn backend(&self) -> &'static str;
    /// The preaching languages.
    fn set_languages(&mut self, langs: &[Lang]);
    /// Whether beam search and the full encoder window change anything (the accuracy speed
    /// test only tries them where they do).
    fn tunable(&self) -> bool {
        true
    }
}

impl SpeechEngine for WhisperEngine {
    fn transcribe(&mut self, audio: &[f32], opts: DecodeOptions, prompt: &str) -> Result<Transcript> {
        WhisperEngine::transcribe(self, audio, opts, prompt)
    }
    fn backend(&self) -> &'static str {
        WhisperEngine::backend(self)
    }
    fn set_languages(&mut self, langs: &[Lang]) {
        WhisperEngine::set_languages(self, langs)
    }
}

/// Load the model at `path`: a Parakeet folder or a Whisper file. Parakeet always runs on the
/// CPU; Whisper tries the GPU first when `prefer_gpu`.
pub fn load(path: &Path, prefer_gpu: bool, threads: usize) -> Result<Box<dyn SpeechEngine>> {
    Ok(if path.is_dir() {
        Box::new(ParakeetEngine::load(path, threads)?)
    } else {
        Box::new(WhisperEngine::load_with_fallback(path, prefer_gpu, threads)?)
    })
}
