use std::collections::VecDeque;

use super::{ms_to_samples, FRAME_SAMPLES};

#[derive(Debug, Clone, Copy)]
pub struct SegmenterConfig {
    /// Audio kept from before speech onset so the first syllable isn't clipped.
    pub pre_roll_ms: u32,
    /// Consecutive voiced audio required to open an utterance (debounces clicks/coughs).
    pub onset_ms: u32,
    /// Silence required to close an utterance.
    pub hangover_ms: u32,
    /// Force-cut long monologues so finals keep flowing and Whisper windows stay short.
    pub max_utterance_ms: u32,
    /// Audio carried into the next utterance after a forced cut, so "John 3 | 16" isn't split.
    pub overlap_ms: u32,
}

impl Default for SegmenterConfig {
    fn default() -> Self {
        Self { pre_roll_ms: 300, onset_ms: 90, hangover_ms: 500, max_utterance_ms: 12_000, overlap_ms: 1_500 }
    }
}

#[derive(Debug)]
pub enum SegEvent {
    Start(u64),
    End { id: u64, audio: Vec<f32>, forced: bool },
}

pub struct Segmenter {
    cfg: SegmenterConfig,
    pre_roll: VecDeque<f32>,
    current: Vec<f32>,
    in_speech: bool,
    voiced_run: usize,
    silent_run: usize,
    current_id: u64,
    next_id: u64,
}

impl Segmenter {
    pub fn new(cfg: SegmenterConfig) -> Self {
        Self {
            cfg,
            pre_roll: VecDeque::with_capacity(ms_to_samples(cfg.pre_roll_ms) + FRAME_SAMPLES),
            current: Vec::with_capacity(ms_to_samples(cfg.max_utterance_ms)),
            in_speech: false,
            voiced_run: 0,
            silent_run: 0,
            current_id: 0,
            next_id: 1,
        }
    }

    pub fn in_speech(&self) -> bool {
        self.in_speech
    }

    pub fn current_id(&self) -> u64 {
        self.current_id
    }

    pub fn current_len(&self) -> usize {
        self.current.len()
    }

    /// The most recent `max_samples` of the open utterance (the partial-decode window).
    pub fn tail(&self, max_samples: usize) -> &[f32] {
        &self.current[self.current.len().saturating_sub(max_samples)..]
    }

    pub fn push(&mut self, frame: &[f32], voiced: bool) -> Option<SegEvent> {
        if !self.in_speech {
            self.pre_roll.extend(frame.iter().copied());
            let cap = ms_to_samples(self.cfg.pre_roll_ms);
            while self.pre_roll.len() > cap {
                self.pre_roll.pop_front();
            }
            self.voiced_run = if voiced { self.voiced_run + frame.len() } else { 0 };
            if self.voiced_run >= ms_to_samples(self.cfg.onset_ms) {
                self.in_speech = true;
                self.silent_run = 0;
                self.current.clear();
                self.current.extend(self.pre_roll.drain(..));
                self.current_id = self.next_id;
                self.next_id += 1;
                return Some(SegEvent::Start(self.current_id));
            }
            return None;
        }

        self.current.extend_from_slice(frame);
        self.silent_run = if voiced { 0 } else { self.silent_run + frame.len() };

        if self.silent_run >= ms_to_samples(self.cfg.hangover_ms) {
            self.in_speech = false;
            self.voiced_run = 0;
            let mut audio = std::mem::take(&mut self.current);
            // Trim trailing silence (keep 200 ms) — less audio means faster inference.
            let trim = self.silent_run.saturating_sub(ms_to_samples(200));
            audio.truncate(audio.len().saturating_sub(trim));
            return Some(SegEvent::End { id: self.current_id, audio, forced: false });
        }

        if self.current.len() >= ms_to_samples(self.cfg.max_utterance_ms) {
            let keep = ms_to_samples(self.cfg.overlap_ms).min(self.current.len());
            let overlap = self.current[self.current.len() - keep..].to_vec();
            let audio = std::mem::replace(&mut self.current, overlap);
            let id = self.current_id;
            self.current_id = self.next_id;
            self.next_id += 1;
            return Some(SegEvent::End { id, audio, forced: true });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_and_closes_utterance() {
        let mut seg = Segmenter::new(SegmenterConfig::default());
        let frame = vec![0.1f32; FRAME_SAMPLES];
        let mut started = None;
        for _ in 0..5 {
            if let Some(SegEvent::Start(id)) = seg.push(&frame, true) {
                started = Some(id);
            }
        }
        assert_eq!(started, Some(1));
        let mut ended = false;
        for _ in 0..40 {
            if let Some(SegEvent::End { id, forced, .. }) = seg.push(&frame, false) {
                assert_eq!(id, 1);
                assert!(!forced);
                ended = true;
                break;
            }
        }
        assert!(ended);
    }
}
