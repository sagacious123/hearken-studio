use webrtc_vad::{SampleRate, Vad, VadMode};

use super::FRAME_SAMPLES;

/// Two-stage voice detector:
///  1. An adaptive energy gate that rejects frames near the room's noise floor (HVAC, hum).
///  2. WebRTC's GMM VAD for the speech/non-speech decision.
///
/// `webrtc_vad::Vad` is `!Send`, so construct this on the thread that uses it.
/// To upgrade to Silero VAD (better with worship music under speech), implement the same
/// `is_speech` contract with an ONNX model; nothing else in the pipeline changes.
pub struct SpeechDetector {
    vad: Vad,
    scratch: Vec<i16>,
    noise_floor: f32,
}

impl SpeechDetector {
    pub fn new(aggressive: bool) -> Self {
        let mode = if aggressive { VadMode::VeryAggressive } else { VadMode::Aggressive };
        Self {
            vad: Vad::new_with_rate_and_mode(SampleRate::Rate16kHz, mode),
            scratch: vec![0; FRAME_SAMPLES],
            noise_floor: 0.005,
        }
    }

    pub fn is_speech(&mut self, frame: &[f32]) -> bool {
        debug_assert_eq!(frame.len(), FRAME_SAMPLES);
        let rms = (frame.iter().map(|s| s * s).sum::<f32>() / frame.len() as f32).sqrt();

        // Track the floor: fall quickly, rise slowly, so speech doesn't drag it upward.
        self.noise_floor = if rms < self.noise_floor {
            0.9 * self.noise_floor + 0.1 * rms
        } else {
            0.999 * self.noise_floor + 0.001 * rms
        };
        if rms < (self.noise_floor * 2.0).max(0.002) {
            return false; // < ~6 dB above floor
        }

        for (dst, src) in self.scratch.iter_mut().zip(frame) {
            *dst = (src.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        }
        self.vad.is_voice_segment(&self.scratch).unwrap_or(false)
    }
}
