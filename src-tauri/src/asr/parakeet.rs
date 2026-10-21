//! NVIDIA's Parakeet TDT 0.6B through sherpa-onnx: the accuracy pass on any computer. Replayed
//! through this pipeline on real sermons it caught every reference Whisper Small did, 3–5x
//! faster on a CPU, and transcribed Nigerian preachers more clearly (tools/asr-bench). v2 hears
//! English; v3 hears 25 European languages, French and Spanish among them. CC BY 4.0: credited
//! in its description in the Accuracy panel and in the README.

use std::path::Path;

use anyhow::{anyhow, ensure, Result};
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig};

use super::whisper::{DecodeOptions, Transcript};
use super::engine::SpeechEngine;
use crate::{audio::SAMPLE_RATE, scripture::lang::Lang};

/// The model folder's files (the int8 export from k2-fsa).
pub const FILES: [&str; 4] = ["encoder.int8.onnx", "decoder.int8.onnx", "joiner.int8.onnx", "tokens.txt"];

/// Parakeet reports no word probabilities, so every sentence counts as this sure. (Whisper's
/// mean token probability on clear speech is about the same.)
const CONFIDENCE: f32 = 0.85;

pub struct ParakeetEngine {
    recognizer: OfflineRecognizer,
    languages: Vec<Lang>,
}

impl ParakeetEngine {
    pub fn load(dir: &Path, threads: usize) -> Result<Self> {
        for f in FILES {
            ensure!(dir.join(f).is_file(), "{} is missing from {}", f, dir.display());
        }
        let file = |f: &str| Some(dir.join(f).to_string_lossy().into_owned());
        let mut config = OfflineRecognizerConfig::default();
        config.model_config.transducer.encoder = file(FILES[0]);
        config.model_config.transducer.decoder = file(FILES[1]);
        config.model_config.transducer.joiner = file(FILES[2]);
        config.model_config.tokens = file(FILES[3]);
        config.model_config.model_type = Some("nemo_transducer".into());
        config.model_config.provider = Some("cpu".into());
        config.model_config.num_threads = threads.max(1) as i32;
        config.decoding_method = Some("greedy_search".into());
        let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| anyhow!("couldn't load the Parakeet model in {}", dir.display()))?;
        Ok(Self { recognizer, languages: vec![Lang::En] })
    }
}

impl SpeechEngine for ParakeetEngine {
    fn transcribe(&mut self, audio: &[f32], _opts: DecodeOptions, _prompt: &str) -> Result<Transcript> {
        let stream = self.recognizer.create_stream();
        stream.accept_waveform(SAMPLE_RATE as i32, audio);
        self.recognizer.decode(&stream);
        let text = stream.get_result().map(|r| r.text.trim().to_owned()).unwrap_or_default();
        // v3 picks the language by itself; with several preaching languages, quotes are then
        // searched in each of them.
        let language = match self.languages.as_slice() {
            [only] => Some(*only),
            _ => None,
        };
        Ok(Transcript::new(text, CONFIDENCE, language))
    }

    fn backend(&self) -> &'static str {
        "cpu"
    }

    fn set_languages(&mut self, langs: &[Lang]) {
        if !langs.is_empty() {
            self.languages = langs.to_vec();
        }
    }

    fn tunable(&self) -> bool {
        false
    }
}
