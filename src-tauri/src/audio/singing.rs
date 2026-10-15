//! Is the room singing or is someone speaking? Google's YAMNet (Apache 2.0) scores each 0.96 s
//! of audio against 521 everyday sounds, including speech, singing, choir and music. It runs
//! here in pure Rust (tract), about 10–40 ms per second of audio on an ordinary laptop.
//!
//! Measured on church recordings: a sermon reads as speech even with a band playing under it
//! at the same volume; an organ under preaching reads as music but never as singing; hymns and
//! worship with a band read as music with sung frames every few seconds.

use std::{
    collections::VecDeque,
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use parking_lot::Mutex;
use tract_onnx::prelude::*;

/// 0.96 s at 16 kHz: one YAMNet frame.
pub const FRAME: usize = 15_360;

type Plan = Arc<TypedRunnableModel>;

pub struct Classifier {
    plan: Plan,
}

/// Scores for one frame, 0–1 each.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Scores {
    pub speech: f32,
    pub singing: f32,
    pub music: f32,
}

impl Scores {
    pub fn sound(&self) -> Sound {
        if self.singing >= 0.15 && self.singing >= self.speech {
            Sound::Singing
        } else if self.speech >= 0.3 {
            Sound::Speech
        } else if self.music >= 0.3 {
            Sound::Music
        } else {
            Sound::Quiet
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sound {
    Quiet,
    Speech,
    Music,
    Singing,
}

/// A worship band can play for a while between sung frames, so a sentence counts as sung only
/// if voices were heard singing this recently.
const SUNG_RECENTLY: Duration = Duration::from_secs(20);
/// Frames arrive up to a second after the words they hold; a sentence's window is widened by
/// this much each side so a short line still has frames to judge by.
const SLACK: Duration = Duration::from_millis(1000);
const KEEP: Duration = Duration::from_secs(60);

/// What the room has sounded like over the last minute, one verdict per 0.96 s. Written by the
/// classifier thread, read by the transcription threads.
#[derive(Default)]
pub struct Room(Mutex<VecDeque<(Instant, Sound)>>);

impl Room {
    pub fn push(&self, at: Instant, sound: Sound) {
        let mut frames = self.0.lock();
        frames.push_back((at, sound));
        while frames.front().is_some_and(|(t, _)| at.duration_since(*t) > KEEP) {
            frames.pop_front();
        }
    }

    /// Was the sentence heard between `from` and `to` sung rather than spoken?
    pub fn sung(&self, from: Instant, to: Instant) -> bool {
        sung(self.0.lock().make_contiguous(), from, to)
    }

    /// Is the room singing right now (the last few seconds)?
    pub fn singing_now(&self) -> bool {
        let now = Instant::now();
        self.sung(now.checked_sub(Duration::from_secs(5)).unwrap_or(now), now)
    }
}

/// Sung when music or singing outweighs speech in the window, and voices sang recently: a
/// preacher over the keys is still speech, and a band between sung lines is still worship.
fn sung(frames: &[(Instant, Sound)], from: Instant, to: Instant) -> bool {
    let from = from.checked_sub(SLACK).unwrap_or(from);
    let to = to + SLACK;
    let (mut musical, mut speech) = (0, 0);
    for (_, s) in frames.iter().filter(|(t, _)| (from..=to).contains(t)) {
        match s {
            Sound::Singing | Sound::Music => musical += 1,
            Sound::Speech => speech += 1,
            Sound::Quiet => {}
        }
    }
    let recent = from.checked_sub(SUNG_RECENTLY).unwrap_or(from);
    musical > 0 && musical >= 2 * speech && frames.iter().any(|(t, s)| *s == Sound::Singing && (recent..=to).contains(t))
}

// Class indices in YAMNet's class map (yamnet_class_map.csv).
const SPEECH: &[usize] = &[0, 1, 2, 3]; // speech, child speech, conversation, narration
const SINGING: &[usize] = &[24, 25, 26, 27, 28, 29, 249, 250, 254, 261]; // singing, choir, yodeling, chant, mantra, child singing, vocal music, a capella, gospel music, song
const MUSIC: &[usize] = &[132, 133, 253]; // music, musical instrument, christian music

impl Classifier {
    pub fn load(model: &Path) -> Result<Self> {
        let plan = tract_onnx::onnx()
            .model_for_path(model)
            .with_context(|| format!("reading {}", model.display()))?
            .with_input_fact(0, f32::fact([FRAME]).into())?
            .into_optimized()?
            .into_runnable()?;
        Ok(Self { plan })
    }

    /// Scores for 0.96 s of 16 kHz mono audio (shorter input is padded with silence).
    pub fn classify(&self, audio: &[f32]) -> Result<Scores> {
        let mut frame = vec![0f32; FRAME];
        let n = audio.len().min(FRAME);
        frame[..n].copy_from_slice(&audio[audio.len() - n..]);
        let input: Tensor = tract_ndarray::Array1::from_vec(frame).into();
        let out = self.plan.run(tvec!(input.into()))?;
        let scores = out[0].try_as_plain_ram()?;
        let scores = scores.as_slice::<f32>()?;
        let row = |i: usize| scores.iter().skip(i).step_by(521).copied().fold(0f32, f32::max);
        let best = |ids: &[usize]| ids.iter().map(|&i| row(i)).fold(0f32, f32::max);
        Ok(Scores { speech: best(SPEECH), singing: best(SINGING), music: best(MUSIC) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_wav(path: &Path) -> Vec<f32> {
        let mut r = hound::WavReader::open(path).unwrap();
        r.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect()
    }

    /// Scores real recordings second by second:
    /// `SINGING_MODEL=…/yamnet.onnx SINGING_CLIPS=…/clips cargo test singing_on_clips -- --ignored --nocapture`
    fn frames(pattern: &str) -> (Instant, Vec<(Instant, Sound)>) {
        let t0 = Instant::now();
        let f = pattern
            .chars()
            .enumerate()
            .map(|(i, c)| {
                let s = match c {
                    '♪' => Sound::Singing,
                    's' => Sound::Speech,
                    'm' => Sound::Music,
                    _ => Sound::Quiet,
                };
                (t0 + Duration::from_millis(960 * i as u64), s)
            })
            .collect();
        (t0, f)
    }

    fn sung_at(pattern: &str, from: usize, to: usize) -> bool {
        let (t0, f) = frames(pattern);
        let at = |i: usize| t0 + Duration::from_millis(960 * i as u64);
        sung(&f, at(from), at(to))
    }

    #[test]
    fn worship_with_a_band_is_singing_between_sung_frames() {
        // Patterns as measured on real recordings.
        assert!(sung_at("mmmmmmmmmmmmmmmmmmm♪mmmmmmmmmmm♪mmmmmmm", 30, 34));
        assert!(sung_at("♪♪♪♪♪♪♪♪.♪♪♪♪♪m♪♪♪♪♪♪♪♪.♪♪.♪", 10, 14));
    }

    #[test]
    fn preaching_is_speech_even_over_music() {
        assert!(!sung_at("..ms.sssssss.ss.....ssss......ss.s.....", 4, 9));
        // An organ under preaching reads as music, but no one has sung.
        assert!(!sung_at("mmmmm..mmmm.mmmmm....mmmmmmm.mmmmmmmm.", 10, 14));
        // Speech after the singing stops: the window itself is speech.
        assert!(!sung_at("♪♪♪♪♪♪sssssssssss", 10, 14));
    }

    #[test]
    fn singing_long_ago_doesnt_count() {
        assert!(sung_at("♪mmmmmmmmmmmmmmmmm", 12, 16));
        assert!(!sung_at("♪mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm", 28, 32));
    }

    #[test]
    #[ignore = "needs the model and test recordings"]
    fn singing_on_clips() {
        let model = std::env::var("SINGING_MODEL").unwrap();
        let t = std::time::Instant::now();
        let c = Classifier::load(Path::new(&model)).unwrap();
        println!("loaded in {:?}", t.elapsed());
        let dir = std::env::var("SINGING_CLIPS").unwrap();
        let mut paths: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "wav")).collect();
        paths.sort();
        for p in paths {
            let audio = read_wav(&p);
            let t = std::time::Instant::now();
            let frames: Vec<Scores> = audio.chunks(FRAME).filter(|f| f.len() == FRAME).map(|f| c.classify(f).unwrap()).collect();
            let per = t.elapsed() / frames.len().max(1) as u32;
            let line: String = frames.iter().map(|s| if s.singing > s.speech.max(0.15) { '♪' } else if s.speech > 0.3 { 's' } else if s.music > 0.3 { 'm' } else { '.' }).collect();
            let avg = |f: fn(&Scores) -> f32| frames.iter().map(f).sum::<f32>() / frames.len().max(1) as f32;
            println!("{:<22} {:>3} frames, {:>5.1?}/frame  speech {:.2} singing {:.2} music {:.2}  {line}", p.file_stem().unwrap().to_string_lossy(), frames.len(), per, avg(|s| s.speech), avg(|s| s.singing), avg(|s| s.music));
            // As the pipeline sees it: 4 s sentences, each judged by the frames heard so far.
            let t0 = Instant::now();
            let at = |i: usize| t0 + Duration::from_millis(960 * (i as u64 + 1));
            let timeline: Vec<(Instant, Sound)> = frames.iter().enumerate().map(|(i, s)| (at(i), s.sound())).collect();
            let verdicts: String = (4..frames.len())
                .step_by(4)
                .map(|end| if sung(&timeline[..end], at(end - 4), at(end - 1)) { '♪' } else { 's' })
                .collect();
            println!("{:<22} sentences: {verdicts}", "");
        }
    }
}
