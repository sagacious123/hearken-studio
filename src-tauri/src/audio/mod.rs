//! Audio front-end: device capture → mono → 16 kHz → VAD → utterance segmentation.
//!
//! Thread model:
//!   cpal callback (real-time)  --lock-free ring-->  DSP thread (resample, VAD, segment)
//! The callback never allocates or locks; it only downmixes and pushes samples.

pub mod capture;
pub mod resample;
pub mod segmenter;
pub mod singing;
pub mod vad;

/// Whisper's native sample rate.
pub const SAMPLE_RATE: usize = 16_000;
/// 30 ms frames — the largest frame size WebRTC VAD accepts at 16 kHz.
pub const FRAME_SAMPLES: usize = SAMPLE_RATE * 30 / 1000;

pub const fn ms_to_samples(ms: u32) -> usize {
    SAMPLE_RATE * ms as usize / 1000
}

/// Running RMS level in dBFS, used for the operator's input meter.
#[derive(Default)]
pub struct LevelMeter {
    sum_sq: f64,
    count: usize,
}

impl LevelMeter {
    pub fn push(&mut self, frame: &[f32]) {
        self.sum_sq += frame.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>();
        self.count += frame.len();
    }

    /// Returns the level since the last call and resets the accumulator.
    pub fn take_db(&mut self) -> f32 {
        let rms = if self.count == 0 { 0.0 } else { (self.sum_sq / self.count as f64).sqrt() };
        self.sum_sq = 0.0;
        self.count = 0;
        (20.0 * rms.max(1e-6).log10()) as f32
    }
}
