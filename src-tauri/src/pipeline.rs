//! Real-time orchestration.
//!
//! ```text
//!  cpal callback ──ring──▶ DSP thread ──jobs──▶ ASR thread ──events──▶ dashboard / output
//!   (mono f32)          resample 16k          whisper.cpp           (Tauri IPC)
//!                       VAD + segmenter       parse → SQLite
//!                       partial cadence       auto-project
//! ```
//!
//! Latency control: while someone is speaking, the DSP thread sends a *partial* job every
//! `step_ms` containing the last ≤ 5 s of the utterance. The ASR thread always decodes the
//! newest partial and drops stale ones, so a slow decode never builds a backlog. The step
//! adapts to measured inference time.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering::Relaxed},
        Arc, OnceLock,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{anyhow, Context, Result};
use crossbeam_channel::{Receiver, Sender, TrySendError};
use parking_lot::Mutex;
use ringbuf::{
    traits::{Consumer, Split},
    HeapCons, HeapRb,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{
    asr::{
        accuracy::{self, AccuracyProfile, Calibration},
        hardware::{self, PARTIAL_WINDOW_MS},
        priority,
        engine::{self, SpeechEngine},
        whisper::{self, DecodeOptions},
    },
    audio::{
        capture::{self, CaptureHandle},
        ms_to_samples,
        resample::Resampler16k,
        segmenter::{SegEvent, Segmenter, SegmenterConfig},
        singing::{self, Classifier, Room},
        vad::SpeechDetector,
        LevelMeter, FRAME_SAMPLES, SAMPLE_RATE,
    },
    commands, events,
    follow::{self, Follower, LyricsIndex},
    scripture::{
        build_slide,
        lang::Lang,
        parser::{ParsedRef, RefKind, ReferenceParser},
        meaning::{self, Meaning},
        quote, BibleDb, Slide, Versification,
    },
    state::AppState,
    voice,
};

const MIN_PARTIAL_MS: u32 = 600;
const MIN_FINAL_MS: u32 = 300;
const MAX_STEP_MS: u32 = 1_500;
/// Don't re-queue the same reference within this window (preachers repeat references).
const DEDUPE_WINDOW: Duration = Duration::from_secs(20);
/// Quote matches are auto-projected only above this (explicit refs use the user threshold).
const QUOTE_AUTO_THRESHOLD: f32 = 0.92;
/// Real microphones always pick up some noise; this long of exact digital zeros means no audio
/// is arriving at all (on macOS: microphone permission blocked).
const SILENCE_WARNING_AFTER: Duration = Duration::from_secs(3);
#[cfg(target_os = "macos")]
const NO_AUDIO_MESSAGE: &str = "No sound is reaching Hearken Studio. If macOS blocked the microphone, allow it in \
System Settings → Privacy & Security → Microphone, then stop and start listening. Otherwise check the input isn't muted.";
#[cfg(not(target_os = "macos"))]
const NO_AUDIO_MESSAGE: &str = "No sound is reaching Hearken Studio. Check the input device isn't muted and that \
microphone access is allowed in the system privacy settings, then stop and start listening.";
/// Confidence added when a detection is in the pastor's sermon plan.
const PLAN_BOOST: f32 = 0.08;
/// Songs go up automatically only above this, and only onto a clear screen or over another song.
const SONG_AUTO_THRESHOLD: f32 = 0.85;

/// Finished sentences waiting for the accuracy pass; beyond this they keep the quick text,
/// so a slow computer never builds a backlog.
const REFINE_QUEUE: usize = 3;
/// A sentence that waited longer than this for the accuracy pass is skipped (it keeps the
/// quick text): on a computer where the model falls behind, the backlog can't grow.
const REFINE_MAX_WAIT: Duration = Duration::from_secs(6);
/// References the accuracy pass finds later than this after they were said aren't queued:
/// the moment has passed, and a card 20 s late is noise to the operator. (The better text
/// still goes to the transcript and archive.)
const REFINE_MAX_LATE: Duration = Duration::from_secs(8);

pub struct PipelineConfig {
    pub device: Option<String>,
    pub model_path: PathBuf,
    pub model_name: String,
    pub use_gpu: bool,
    pub threads: usize,
    pub step_ms: u32,
    pub reduced_ctx: bool,
    pub bible_path: PathBuf,
    /// Second, more accurate model that re-transcribes each finished sentence.
    pub refine: Option<RefineConfig>,
    /// YAMNet, to tell singing from speech; without it, only known lyrics count as singing.
    pub singing_model: Option<PathBuf>,
    /// The preaching languages (the models must be multilingual unless it's only English).
    pub languages: Vec<Lang>,
}

pub struct RefineConfig {
    pub model_path: PathBuf,
    pub model_name: String,
    pub use_gpu: bool,
    pub threads: usize,
    /// Saved speed test for this model on this computer; measured on first use when absent.
    pub calibration: Option<Calibration>,
    /// Chosen explicitly by the operator: run it even if it's slow (the bounded queue keeps
    /// live updates safe). Automatic choice drops a model that can't keep up.
    pub forced: bool,
    pub calibration_file: PathBuf,
    pub bench_wav: PathBuf,
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineInfo {
    pub device: String,
    pub sample_rate: u32,
    pub model: String,
    pub backend: String,
    pub step_ms: u32,
    /// Accuracy-pass model, if one is running.
    pub refine_model: Option<String>,
}

pub struct PipelineHandle {
    stop: Arc<AtomicBool>,
    capture: Option<CaptureHandle>,
    threads: Vec<JoinHandle<()>>,
    pub info: PipelineInfo,
}

impl PipelineHandle {
    /// Still listening: the audio thread ends only when stopped or when capture fails.
    pub fn is_alive(&self) -> bool {
        self.threads.first().is_some_and(|t| !t.is_finished())
    }

    pub fn stop(mut self) {
        self.stop.store(true, Relaxed);
        drop(self.capture.take());
        // DSP exits on the flag and drops the job sender, which unblocks and ends the ASR thread.
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

/// Reference keys already sent to the operator, shared by the live and accuracy passes:
/// key → (utterance, tentative, when).
#[derive(Clone, Default)]
struct Dedupe(Arc<Mutex<HashMap<String, (u64, bool, Instant)>>>);

impl Dedupe {
    /// Once per utterance (plus one tentative→confirmed upgrade), and not again across
    /// utterances within the dedupe window.
    fn should_emit(&self, key: &str, utterance: u64, tentative: bool) -> bool {
        let mut emitted = self.0.lock();
        let emit = match emitted.get(key) {
            None => true,
            Some(&(utt, was_tentative, _)) if utt == utterance => was_tentative && !tentative,
            Some(&(_, _, at)) => at.elapsed() > DEDUPE_WINDOW,
        };
        if emit {
            emitted.insert(key.to_owned(), (utterance, tentative, Instant::now()));
        }
        if emitted.len() > 512 {
            emitted.retain(|_, (_, _, at)| at.elapsed() < DEDUPE_WINDOW);
        }
        emit
    }
}

struct RefineJob {
    utterance: u64,
    audio: Vec<f32>,
    captured_at: Instant,
    /// The quick pass's text, archived instead if this sentence is skipped as too old.
    quick_text: Arc<OnceLock<String>>,
}

struct Job {
    utterance: u64,
    audio: Vec<f32>,
    is_final: bool,
    /// When the newest sample in `audio` was pulled off the capture ring.
    captured_at: Instant,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TranscriptEvent {
    utterance_id: u64,
    text: String,
    is_final: bool,
    confidence: f32,
    inference_ms: f32,
    audio_ms: u32,
    /// From the accuracy pass: replaces the quick text of the same utterance.
    refined: bool,
    /// Sung rather than spoken (finished sentences only).
    sung: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct VoiceCommandEvent {
    utterance_id: u64,
    label: String,
    /// False when there was nowhere to go (already at the first or last verse).
    moved: bool,
    /// What's on screen now.
    now: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct DetectionEvent {
    id: String,
    utterance_id: u64,
    kind: RefKind,
    confidence: f32,
    tentative: bool,
    matched: String,
    slide: Slide,
    auto_projected: bool,
    /// In the pastor's sermon plan.
    planned: bool,
    /// Audio capture → detection emitted.
    latency_ms: f32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StatusEvent {
    speaking: bool,
    level_db: f32,
    step_ms: u32,
    inference_ms: f32,
}

pub fn start(app: AppHandle, cfg: PipelineConfig, v11n: Arc<Versification>) -> Result<PipelineHandle> {
    let stop = Arc::new(AtomicBool::new(false));
    let step_ms = Arc::new(AtomicU32::new(cfg.step_ms));
    let infer_ewma = Arc::new(AtomicU32::new(0f32.to_bits()));
    let (job_tx, job_rx) = crossbeam_channel::unbounded::<Job>();
    let dedupe = Dedupe::default();
    let room = Arc::new(Room::default());

    // 0. Accuracy-pass thread. Loads in the background (a large model can take seconds), and
    //    never blocks or fails listening: until it's ready, or if it can't load, sentences just
    //    keep their quick transcription.
    let (refine_tx, refine_thread, refine_model) = match cfg.refine {
        Some(rc) => {
            let (tx, rx) = crossbeam_channel::bounded::<RefineJob>(REFINE_QUEUE);
            let (app, v11n, bible_path, dedupe, room) = (app.clone(), v11n.clone(), cfg.bible_path.clone(), dedupe.clone(), room.clone());
            let langs = cfg.languages.clone();
            let name = rc.model_name.clone();
            let handle = thread::Builder::new().name("asr-refine".into()).spawn(move || {
                priority::background();
                let engine = engine::load(&rc.model_path, rc.use_gpu, rc.threads).map(|mut e| {
                    e.set_languages(&langs);
                    e
                });
                let bible = BibleDb::open(&bible_path);
                let (mut engine, bible) = match (engine, bible) {
                    (Ok(e), Ok(b)) => (e, b),
                    (Err(e), _) | (_, Err(e)) => {
                        log::warn!("accuracy pass unavailable: {e:#}");
                        let _ = app.emit(events::WARNING, format!("The accuracy model ({}) couldn't load, so quick transcription is used: {e:#}", rc.model_name));
                        return;
                    }
                };
                let cal = match rc.calibration {
                    Some(c) => c,
                    None => {
                        // First use on this computer: time it on a full sentence and remember.
                        let prompt = whisper::accurate_prompt("", &[], &langs);
                        match hardware::load_wav(&rc.bench_wav).and_then(|a| accuracy::calibrate(engine.as_mut(), &rc.model_name, &a, &prompt)) {
                            Ok(c) => {
                                AccuracyProfile::record(&rc.calibration_file, &rc.app_version, c.clone());
                                c
                            }
                            Err(e) => {
                                log::warn!("accuracy calibration failed ({e:#}); using the standard setting");
                                Calibration { model: rc.model_name.clone(), backend: engine.backend().into(), usable: true, beam: false, full_ctx: false, decode_ms: 0.0, audio_ms: 1.0 }
                            }
                        }
                    }
                };
                if !cal.usable && !rc.forced {
                    let _ = app.emit(
                        events::WARNING,
                        format!("{} is too slow on this computer for the accuracy pass, so quick transcription is used. Choose a smaller model under Accuracy.", rc.model_name),
                    );
                    return;
                }
                log::info!("accuracy pass ready: {} on {} ({})", rc.model_name, engine.backend(), cal.describe());
                let parser = ReferenceParser::for_languages(&langs);
                RefineWorker { app, bible, v11n, parser, dedupe, opts: cal.options(), lyrics: LyricsIndex::default(), room, langs }.run(engine.as_mut(), rx);
                log::info!("accuracy pass stopped");
            })?;
            (Some(tx), Some(handle), Some(name))
        }
        None => (None, None, None),
    };

    // 1. ASR thread: load model first so startup errors surface to the caller.
    let (ready_tx, ready_rx) = crossbeam_channel::bounded::<Result<&'static str>>(1);
    let asr = {
        let app = app.clone();
        let room = room.clone();
        let step_ms = step_ms.clone();
        let infer_ewma = infer_ewma.clone();
        let (model_path, use_gpu, threads, reduced_ctx, bible_path, base_step) =
            (cfg.model_path.clone(), cfg.use_gpu, cfg.threads, cfg.reduced_ctx, cfg.bible_path.clone(), cfg.step_ms);
        let langs = cfg.languages.clone();
        thread::Builder::new().name("asr".into()).spawn(move || {
            priority::interactive();
            let engine = engine::load(&model_path, use_gpu, threads).map(|mut e| {
                e.set_languages(&langs);
                e
            });
            let bible = BibleDb::open(&bible_path);
            let (mut engine, bible) = match (engine, bible) {
                (Ok(e), Ok(b)) => {
                    let _ = ready_tx.send(Ok(e.backend()));
                    (e, b)
                }
                (Err(e), _) | (_, Err(e)) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let mut worker = AsrWorker {
                app,
                bible,
                v11n,
                parser: ReferenceParser::for_languages(&langs),
                langs,
                follower: Follower::default(),
                lyrics: LyricsIndex::default(),
                dedupe,
                refine: refine_tx,
                step_ms,
                infer_ewma,
                base_step,
                reduced_ctx,
                last_sentence: None,
                room,
            };
            worker.run(engine.as_mut(), job_rx);
            log::info!("asr thread stopped");
        })?
    };
    let backend = ready_rx.recv().map_err(|_| anyhow!("ASR thread exited during startup"))??;

    // 1b. Singing detector: is the room singing or speaking? Off the critical path: frames are
    //     dropped rather than queued if it falls behind, and listening works without it.
    let (sound_tx, singing_thread) = match cfg.singing_model.filter(|p| p.exists()) {
        Some(model) => {
            let (tx, rx) = crossbeam_channel::bounded::<(Instant, Vec<f32>)>(4);
            let (app, room) = (app.clone(), room.clone());
            let handle = thread::Builder::new().name("singing".into()).spawn(move || {
                priority::background();
                let classifier = match Classifier::load(&model) {
                    Ok(c) => c,
                    Err(e) => {
                        log::warn!("singing detection unavailable: {e:#}");
                        return;
                    }
                };
                let mut was = false;
                for (at, frame) in rx {
                    match classifier.classify(&frame) {
                        Ok(scores) => room.push(at, scores.sound()),
                        Err(e) => log::debug!("singing detection failed on a frame: {e:#}"),
                    }
                    let now = room.singing_now();
                    if now != was {
                        was = now;
                        let _ = app.emit(events::SINGING, now);
                    }
                }
                if was {
                    let _ = app.emit(events::SINGING, false);
                }
            })?;
            (Some(tx), Some(handle))
        }
        None => (None, None),
    };

    // 2. Capture → lock-free ring (≈ 2 s at 48 kHz).
    let (producer, consumer) = HeapRb::<f32>::new(96_000).split();
    let capture = match capture::start(cfg.device.as_deref(), producer) {
        Ok(c) => c,
        Err(e) => {
            drop(job_tx); // lets the ASR thread exit
            let _ = asr.join();
            return Err(e);
        }
    };

    // 3. DSP thread.
    let dsp = {
        let (app, stop, step_ms, infer_ewma) = (app.clone(), stop.clone(), step_ms.clone(), infer_ewma.clone());
        let rate = capture.sample_rate;
        thread::Builder::new().name("dsp".into()).spawn(move || {
            priority::interactive();
            if let Err(e) = dsp_loop(consumer, rate, job_tx, sound_tx, &stop, &step_ms, &infer_ewma, &app) {
                log::error!("dsp thread failed: {e:#}");
                let _ = app.emit(events::ERROR, format!("{e:#}"));
            }
        })?
    };

    let info = PipelineInfo {
        device: capture.device_name.clone(),
        sample_rate: capture.sample_rate,
        model: cfg.model_name,
        refine_model,
        backend: backend.into(),
        step_ms: cfg.step_ms,
    };
    log::info!("pipeline started: {info:?}");
    // Joined in order: DSP ends (dropping the singing detector's sender), then the live ASR
    // thread (dropping the accuracy-pass sender), then the accuracy pass drains and ends.
    let threads = [Some(dsp), singing_thread, Some(asr), refine_thread].into_iter().flatten().collect();
    Ok(PipelineHandle { stop, capture: Some(capture), threads, info })
}

fn dsp_loop(
    mut consumer: HeapCons<f32>,
    input_rate: u32,
    jobs: Sender<Job>,
    sounds: Option<Sender<(Instant, Vec<f32>)>>,
    stop: &AtomicBool,
    step_ms: &AtomicU32,
    infer_ewma: &AtomicU32,
    app: &AppHandle,
) -> Result<()> {
    let mut resampler = Resampler16k::new(input_rate)?;
    let mut vad = SpeechDetector::new(false);
    let mut seg = Segmenter::new(SegmenterConfig::default());
    let mut meter = LevelMeter::default();
    let mut raw = vec![0f32; 4096];
    let mut pcm: Vec<f32> = Vec::with_capacity(SAMPLE_RATE);
    let mut sound: Vec<f32> = Vec::with_capacity(singing::FRAME);
    let mut since_partial = 0usize;
    let mut last_status = Instant::now();
    let started = Instant::now();
    let mut heard_signal = false;
    let mut warned_silent = false;
    let send = |job: Job| jobs.send(job).context("ASR thread is gone");

    while !stop.load(Relaxed) {
        let n = consumer.pop_slice(&mut raw);
        if n == 0 {
            thread::sleep(Duration::from_millis(4));
            continue;
        }
        let captured_at = Instant::now();
        if !heard_signal && raw[..n].iter().any(|s| *s != 0.0) {
            heard_signal = true;
            if warned_silent {
                let _ = app.emit(events::WARNING, "");
            }
        }
        if !heard_signal && !warned_silent && started.elapsed() >= SILENCE_WARNING_AFTER {
            warned_silent = true;
            let _ = app.emit(events::WARNING, NO_AUDIO_MESSAGE);
        }
        resampler.process(&raw[..n], &mut pcm)?;

        let mut offset = 0;
        while pcm.len() - offset >= FRAME_SAMPLES {
            let frame = &pcm[offset..offset + FRAME_SAMPLES];
            offset += FRAME_SAMPLES;
            meter.push(frame);
            if let Some(tx) = &sounds {
                sound.extend_from_slice(frame);
                if sound.len() >= singing::FRAME {
                    let _ = tx.try_send((captured_at, std::mem::replace(&mut sound, Vec::with_capacity(singing::FRAME))));
                }
            }

            match seg.push(frame, vad.is_speech(frame)) {
                Some(SegEvent::Start(id)) => {
                    since_partial = 0;
                    log::debug!("utterance {id} started");
                }
                Some(SegEvent::End { id, audio, forced }) => {
                    since_partial = 0;
                    if audio.len() >= ms_to_samples(MIN_FINAL_MS) {
                        send(Job { utterance: id, audio, is_final: true, captured_at })?;
                    }
                    if forced {
                        log::debug!("utterance {id} force-cut at max length");
                    }
                }
                None => {}
            }

            if seg.in_speech() {
                since_partial += FRAME_SAMPLES;
                let step = ms_to_samples(step_ms.load(Relaxed));
                if since_partial >= step && seg.current_len() >= ms_to_samples(MIN_PARTIAL_MS) {
                    since_partial = 0;
                    let audio = seg.tail(ms_to_samples(PARTIAL_WINDOW_MS)).to_vec();
                    send(Job { utterance: seg.current_id(), audio, is_final: false, captured_at })?;
                }
            }
        }
        pcm.drain(..offset);

        if last_status.elapsed() >= Duration::from_millis(100) {
            last_status = Instant::now();
            let _ = app.emit(
                events::STATUS,
                StatusEvent {
                    speaking: seg.in_speech(),
                    level_db: meter.take_db(),
                    step_ms: step_ms.load(Relaxed),
                    inference_ms: f32::from_bits(infer_ewma.load(Relaxed)),
                },
            );
        }
    }
    Ok(())
}

struct AsrWorker {
    app: AppHandle,
    bible: BibleDb,
    v11n: Arc<Versification>,
    parser: ReferenceParser,
    langs: Vec<Lang>,
    follower: Follower,
    lyrics: LyricsIndex,
    dedupe: Dedupe,
    /// Finished sentences go here too, for the accuracy pass.
    refine: Option<Sender<RefineJob>>,
    step_ms: Arc<AtomicU32>,
    infer_ewma: Arc<AtomicU32>,
    base_step: u32,
    reduced_ctx: bool,
    /// The last finished sentence, for a command split by a pause ("Go to verse." / "6.").
    last_sentence: Option<(Instant, String)>,
    room: Arc<Room>,
}

impl AsrWorker {
    /// Move the live scripture as told, and show it in the detections like anything else.
    fn run_voice_command(&self, state: &AppState, cmd: voice::Command, said: &str, utterance: u64, captured_at: Instant) {
        let app = &self.app;
        let before = state.presenter.snapshot().slide.map(|s| s.id);
        let result = match cmd {
            voice::Command::Next => commands::step_live(app, state, 1),
            voice::Command::Previous => commands::step_live(app, state, -1),
            voice::Command::First => commands::live_chapter_verse(app, state, 1, None).map(|_| ()),
            voice::Command::Final => commands::live_chapter_verse(app, state, 0, None).map(|_| ()),
            voice::Command::Verse(s, e) => commands::live_chapter_verse(app, state, s, e).map(|_| ()),
        };
        if let Err(e) = result {
            log::warn!("voice command {cmd:?} failed: {e}");
        }
        let now = state.presenter.snapshot().slide;
        let moved = now.as_ref().is_some_and(|s| Some(&s.id) != before.as_ref());
        let _ = app.emit(events::VOICE_COMMAND, VoiceCommandEvent { utterance_id: utterance, label: cmd.label(), moved, now: now.as_ref().map(|s| s.label.clone()) });
        if let Some(slide) = now.filter(|_| moved) {
            state.history.detected(&slide, RefKind::Command.name(), 1.0, said);
            let _ = app.emit(
                events::DETECTION,
                DetectionEvent {
                    id: format!("{utterance}:voice:{}", slide.id),
                    utterance_id: utterance,
                    kind: RefKind::Command,
                    confidence: 1.0,
                    tentative: false,
                    matched: said.trim().to_owned(),
                    slide,
                    auto_projected: true,
                    planned: false,
                    latency_ms: captured_at.elapsed().as_secs_f32() * 1000.0,
                },
            );
        }
    }

    fn run(&mut self, engine: &mut dyn SpeechEngine, jobs: Receiver<Job>) {
        while let Ok(first) = jobs.recv() {
            // Coalesce the backlog: every final is kept (in order), but only the newest partial
            // — and only if it belongs to an utterance newer than any final in this batch.
            let mut batch = vec![first];
            batch.extend(jobs.try_iter());
            let newest_final = batch.iter().filter(|j| j.is_final).map(|j| j.utterance).max();
            let newest_partial = batch.iter().rposition(|j| !j.is_final);
            for (i, job) in batch.into_iter().enumerate() {
                let keep = job.is_final
                    || (Some(i) == newest_partial && newest_final.map_or(true, |f| job.utterance > f));
                if keep {
                    let utterance = job.utterance;
                    // A sentence that trips a bug must not end listening mid-service.
                    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.process(engine, job))).is_err() {
                        log::error!("utterance {utterance} couldn't be processed (the panic is logged above); listening continues");
                    }
                }
            }
        }
    }

    fn process(&mut self, engine: &mut dyn SpeechEngine, job: Job) {
        let settings = self.app.state::<AppState>().settings.lock().clone();
        // Hand a finished sentence to the accuracy pass first, so both decode in parallel.
        // If it's busy, this sentence keeps the quick text (logged below).
        let quick_text = Arc::new(OnceLock::new());
        let refining = job.is_final
            && self.refine.as_ref().is_some_and(|tx| {
                let refine_job = RefineJob { utterance: job.utterance, audio: job.audio.clone(), captured_at: job.captured_at, quick_text: quick_text.clone() };
                match tx.try_send(refine_job) {
                    Ok(()) => true,
                    Err(TrySendError::Full(_)) => {
                        log::debug!("accuracy pass busy; utterance {} keeps the quick text", job.utterance);
                        false
                    }
                    Err(TrySendError::Disconnected(_)) => false,
                }
            });

        let opts = DecodeOptions::live(!job.is_final, self.reduced_ctx);
        let t0 = Instant::now();
        let transcript = match engine.transcribe(&job.audio, opts, &whisper::live_prompt(&settings.vocabulary, &self.langs)) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("transcription failed: {e:#}");
                return;
            }
        };
        let inference_ms = t0.elapsed().as_secs_f32() * 1000.0;
        let sung = job.is_final && self.room.sung(heard_from(&job.audio, job.captured_at), job.captured_at);

        if !job.is_final {
            // Adapt cadence: keep the step ~25% above decode time so the ASR thread never saturates.
            let prev = f32::from_bits(self.infer_ewma.load(Relaxed));
            let ewma = if prev == 0.0 { inference_ms } else { 0.8 * prev + 0.2 * inference_ms };
            self.infer_ewma.store(ewma.to_bits(), Relaxed);
            let step = ((ewma * 1.25) as u32).clamp(self.base_step, MAX_STEP_MS);
            self.step_ms.store(step, Relaxed);
        }

        let _ = self.app.emit(
            events::TRANSCRIPT,
            TranscriptEvent {
                utterance_id: job.utterance,
                text: transcript.text.clone(),
                is_final: job.is_final,
                confidence: transcript.confidence,
                inference_ms,
                audio_ms: (job.audio.len() * 1000 / SAMPLE_RATE) as u32,
                refined: false,
                sung,
            },
        );
        if transcript.text.is_empty() {
            return;
        }
        let _ = quick_text.set(transcript.text.clone());

        // Cloned handle so `state` doesn't borrow `self` across the `&mut self` calls below.
        let app = self.app.clone();
        let state = app.state::<AppState>();
        let asr_factor = 0.7 + 0.3 * transcript.confidence.clamp(0.0, 1.0);
        if job.is_final && !refining {
            // With the accuracy pass on, the archive gets its (better) text instead.
            state.history.said(&transcript.text, sung);
        }

        // Spoken commands ("next verse", "go back", "go to verse 6"): finished sentences only, so
        // a command acts once, and only while a scripture is on screen. A sung "go back" is a lyric.
        if job.is_final && !sung {
            let previous = self.last_sentence.take().filter(|(at, _)| at.elapsed() < Duration::from_secs(8)).map(|(_, t)| t);
            let langs = &self.langs;
            let command = settings
                .voice_commands
                .then(|| voice::parse_in(&transcript.text, langs).or_else(|| previous.as_deref().and_then(|p| voice::parse_joined_in(p, &transcript.text, langs))))
                .flatten();
            let live = state.presenter.snapshot();
            match command {
                Some(cmd) if live.slide.as_ref().is_some_and(|s| !s.is_song()) => {
                    self.run_voice_command(&state, cmd, &transcript.text, job.utterance, job.captured_at);
                    return; // the sentence was an instruction, not something to detect or follow
                }
                // Kept so "Go to verse." / "6." can be read together.
                _ => self.last_sentence = Some((Instant::now(), transcript.text.clone())),
            }
        }

        // Follow-along: keep the live passage or song in step with the reader or singers.
        let library = state.songs.version();
        if self.lyrics.version() != library {
            self.lyrics.refresh(library, state.songs.all());
        }
        let live = state.presenter.snapshot();
        let ctx = follow::Context {
            bible: &self.bible,
            v11n: &self.v11n,
            songs: self.lyrics.songs(),
            follow_scripture: settings.follow_reading,
            follow_songs: settings.lyrics_detection,
        };
        match self.follower.update(&transcript.text, &live, &ctx) {
            follow::Action::Advance(slide) => {
                state.presenter.present(&app, slide);
            }
            follow::Action::Progress { slide_id, line, offset } => state.presenter.set_reading(&app, &slide_id, line, offset),
            follow::Action::None => {}
        }

        // A Bible phrase that's sung is a lyric, not a verse being quoted: when the room is heard
        // singing, or during worship (a song on screen) when the line matches an imported song.
        // In the sermon, a pastor quoting the same words is still a quote.
        let lyric = self.lyrics.detect(&transcript.text);
        let singing = sung || (lyric.is_some() && live.slide.as_ref().is_some_and(|s| s.is_song()));

        // Worship songs: which imported song is being sung (a new one, not the live one).
        if settings.lyrics_detection {
            let live_song = live.slide.as_ref().and_then(|s| s.song.as_ref()).map(|p| p.song_id.clone());
            let found = lyric.filter(|m| m.slide.song.as_ref().map(|p| &p.song_id) != live_song.as_ref());
            if let Some(m) = found {
                if self.dedupe.should_emit(&m.slide.id, job.utterance, false) {
                    let confidence = m.score * asr_factor;
                    let clear_or_song = !live.blackout && (live.slide.is_none() || live_song.is_some());
                    let auto = settings.auto_project && confidence >= SONG_AUTO_THRESHOLD && clear_or_song;
                    let auto_projected = auto && state.presenter.present(&app, m.slide.clone());
                    state.history.detected(&m.slide, RefKind::Song.name(), confidence, &m.phrase);
                    state.remember(&m.slide);
                    let _ = self.app.emit(
                        events::DETECTION,
                        DetectionEvent {
                            id: format!("{}:{}", job.utterance, m.slide.id),
                            utterance_id: job.utterance,
                            kind: RefKind::Song,
                            confidence,
                            tentative: false,
                            matched: m.phrase,
                            slide: m.slide,
                            auto_projected,
                            planned: false,
                            latency_ms: job.captured_at.elapsed().as_secs_f32() * 1000.0,
                        },
                    );
                }
            }
        }

        let meaning = settings.meaning_detection.then(|| state.meaning.lock().clone()).flatten();
        let heard = Heard { text: &transcript.text, is_final: job.is_final, languages: &self.langs, language: transcript.language, asr_factor };
        let refs = find_references(&mut self.parser, &self.bible, &self.v11n, heard, settings.quote_detection, meaning.as_deref());
        emit_references(&app, &state, &settings, &self.bible, &self.dedupe, refs, singing, job.utterance, job.captured_at);
    }
}

/// When the first sample of a sentence was heard.
fn heard_from(audio: &[f32], captured_at: Instant) -> Instant {
    let len = Duration::from_secs_f32(audio.len() as f32 / SAMPLE_RATE as f32);
    captured_at.checked_sub(len).unwrap_or(captured_at)
}

/// A transcript, as heard.
struct Heard<'a> {
    text: &'a str,
    is_final: bool,
    /// The preaching languages, and the one this was heard in (if the model said).
    languages: &'a [Lang],
    language: Option<Lang>,
    /// How sure the speech model was, as a confidence factor.
    asr_factor: f32,
}

/// References in a transcript: announced ("John 3:16") and, on finished sentences with nothing
/// announced, quoted verses (in the language it was heard in, or each preaching language).
fn find_references(
    parser: &mut ReferenceParser,
    bible: &BibleDb,
    v11n: &Versification,
    heard: Heard,
    quote_detection: bool,
    meaning: Option<&Meaning>,
) -> Vec<ParsedRef> {
    let Heard { text, is_final, asr_factor, .. } = heard;
    let mut refs = parser.parse(text, is_final, v11n);
    for r in &mut refs {
        r.confidence *= asr_factor;
    }
    let quote_langs: Vec<Lang> = match heard.language {
        Some(l) => vec![l],
        None => heard.languages.to_vec(),
    };
    if refs.is_empty() && is_final && quote_detection {
        let mut hits = Vec::new();
        for &lang in &quote_langs {
            match quote::find_quotes_in(bible, text, lang) {
                Ok(found) => hits.extend(found),
                Err(e) => log::warn!("quote search failed: {e:#}"),
            }
        }
        hits.sort_by(|a, b| b.score.total_cmp(&a.score));
        refs.extend(hits.into_iter().take(quote::MAX_HITS).map(|hit| ParsedRef {
            reference: hit.reference,
            confidence: hit.score * asr_factor,
            tentative: false,
            kind: RefKind::Quote,
            matched: hit.phrase,
        }));
    }
    // Nothing said or quoted word for word: a verse said in other words? (English only: the
    // meaning model is.)
    let meaning = meaning.filter(|_| quote_langs.contains(&Lang::En));
    if let Some(hit) = meaning.filter(|_| refs.is_empty() && is_final).and_then(|m| m.live_hit(text)) {
        refs.push(ParsedRef {
            reference: hit.reference,
            confidence: meaning::confidence(hit.score) * asr_factor,
            tentative: false,
            kind: RefKind::Meaning,
            matched: text.trim().to_owned(),
        });
    }
    refs
}

/// Queue new references for the operator, projecting them when confident enough.
#[allow(clippy::too_many_arguments)]
fn emit_references(
    app: &AppHandle,
    state: &AppState,
    settings: &crate::state::Settings,
    bible: &BibleDb,
    dedupe: &Dedupe,
    refs: Vec<ParsedRef>,
    singing: bool,
    utterance: u64,
    captured_at: Instant,
) {
    // During worship a scripture suggestion is only a card: it never replaces the song by itself.
    let song_live = state.presenter.snapshot().slide.as_ref().is_some_and(|s| s.is_song());
    for mut r in refs {
        if singing && matches!(r.kind, RefKind::Quote | RefKind::Meaning) {
            continue; // a lyric that borrows Bible words ("The Lord is my shepherd")
        }
        if !dedupe.should_emit(&r.reference.key(), utterance, r.tentative) {
            continue;
        }
        let planned = state.plan.lock().contains(&r.reference);
        if planned {
            r.confidence = (r.confidence + PLAN_BOOST).min(0.99);
        }
        let slide = match build_slide(bible, &settings.translation, settings.secondary_translation.as_deref(), &r.reference) {
            Ok(Some(s)) => s,
            Ok(None) => continue,
            Err(e) => {
                log::warn!("slide lookup failed: {e:#}");
                continue;
            }
        };
        let threshold = match r.kind {
            RefKind::Quote => QUOTE_AUTO_THRESHOLD,
            RefKind::Meaning => f32::INFINITY, // a suggestion for the operator, never projected by itself
            _ => settings.auto_threshold,
        };
        let auto = settings.auto_project && !r.tentative && r.confidence >= threshold && !song_live;
        let auto_projected = auto && state.presenter.present(app, slide.clone());
        if !r.tentative {
            state.history.detected(&slide, r.kind.name(), r.confidence, &r.matched);
            state.remember(&slide);
        }
        let _ = app.emit(
            events::DETECTION,
            DetectionEvent {
                id: format!("{}:{}", utterance, r.reference.key()),
                utterance_id: utterance,
                kind: r.kind,
                confidence: r.confidence,
                tentative: r.tentative,
                matched: r.matched,
                slide,
                auto_projected,
                planned,
                latency_ms: captured_at.elapsed().as_secs_f32() * 1000.0,
            },
        );
    }
}

/// Re-transcribes each finished sentence with the accuracy model (beam search, full context,
/// Bible-vocabulary prompt), replaces the quick text, archives it, and reports references the
/// quick pass missed. Live updates never wait for it.
struct RefineWorker {
    app: AppHandle,
    bible: BibleDb,
    v11n: Arc<Versification>,
    parser: ReferenceParser,
    dedupe: Dedupe,
    opts: DecodeOptions,
    /// To tell a sung lyric from a quoted verse, as the live pass does.
    lyrics: LyricsIndex,
    room: Arc<Room>,
    langs: Vec<Lang>,
}

impl RefineWorker {
    fn run(&mut self, engine: &mut dyn SpeechEngine, jobs: Receiver<RefineJob>) {
        while let Ok(job) = jobs.recv() {
            let utterance = job.utterance;
            // As in the live pass: one sentence that trips a bug mustn't end the accuracy pass.
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.refine(engine, job))).is_err() {
                log::error!("accuracy pass couldn't process utterance {utterance} (the panic is logged above); it continues");
            }
        }
    }

    fn refine(&mut self, engine: &mut dyn SpeechEngine, job: RefineJob) {
        let app = self.app.clone();
        let state = app.state::<AppState>();
        let settings = state.settings.lock().clone();
        let planned: Vec<String> = state.plan.lock().refs.iter().map(|r| r.label()).collect();
        let prompt = whisper::accurate_prompt(&settings.vocabulary, &planned, &self.langs);

        // Fallen behind: let this sentence keep its quick text rather than grow a backlog.
        let waited = job.captured_at.elapsed();
        let sung = self.room.sung(heard_from(&job.audio, job.captured_at), job.captured_at);
        if waited > REFINE_MAX_WAIT {
            log::info!("accuracy pass behind ({:.1} s); utterance {} keeps the quick text", waited.as_secs_f32(), job.utterance);
            if let Some(text) = job.quick_text.get() {
                state.history.said(text, sung);
            }
            return;
        }

        let t0 = Instant::now();
        let transcript = match engine.transcribe(&job.audio, self.opts, &prompt) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("accuracy pass failed: {e:#}");
                return;
            }
        };
        let inference_ms = t0.elapsed().as_secs_f32() * 1000.0;
        log::debug!("accuracy pass: {:.0} ms for {:.1} s", inference_ms, job.audio.len() as f32 / SAMPLE_RATE as f32);
        let _ = app.emit(
            events::TRANSCRIPT,
            TranscriptEvent {
                utterance_id: job.utterance,
                text: transcript.text.clone(),
                is_final: true,
                confidence: transcript.confidence,
                inference_ms,
                audio_ms: (job.audio.len() * 1000 / SAMPLE_RATE) as u32,
                refined: true,
                sung,
            },
        );
        if transcript.text.is_empty() {
            return;
        }
        state.history.said(&transcript.text, sung);
        let late = job.captured_at.elapsed();
        if late > REFINE_MAX_LATE {
            log::info!("accuracy pass took {:.1} s for utterance {}; its references are too late to queue", late.as_secs_f32(), job.utterance);
            return;
        }
        let asr_factor = 0.7 + 0.3 * transcript.confidence.clamp(0.0, 1.0);
        let meaning = settings.meaning_detection.then(|| state.meaning.lock().clone()).flatten();
        let heard = Heard { text: &transcript.text, is_final: true, languages: &self.langs, language: transcript.language, asr_factor };
        let refs = find_references(&mut self.parser, &self.bible, &self.v11n, heard, settings.quote_detection, meaning.as_deref());
        let library = state.songs.version();
        if self.lyrics.version() != library {
            self.lyrics.refresh(library, state.songs.all());
        }
        let singing = sung || (state.presenter.snapshot().slide.as_ref().is_some_and(|s| s.is_song()) && self.lyrics.detect(&transcript.text).is_some());
        emit_references(&app, &state, &settings, &self.bible, &self.dedupe, refs, singing, job.utterance, job.captured_at);
    }
}

#[cfg(test)]
mod replay {
    //! Plays a recording through the listening pipeline offline, as if it were heard live: the
    //! same speech detector and sentence cutter, the live and accuracy passes with their prompts,
    //! and the same reference finder. Writes one results file per pass for
    //! `tools/asr-bench/score_real.py`, with the references the app itself found.
    //!
    //! ```text
    //! HEARKEN_REPLAY=service.wav            16 kHz mono
    //! HEARKEN_LIVE=…/ggml-base.en-q5_1.bin  live model (optional)
    //! HEARKEN_ACCURATE=…/ggml-small.en-q5_1.bin   accuracy pass (optional; a Parakeet folder works too)
    //! HEARKEN_OUT=results/real-service      written as <out>-live.json and <out>-accurate.json
    //! HEARKEN_LIVE_PROMPT, HEARKEN_ACCURATE_PROMPT   try another prompt (" " for none)
    //! cargo test --release replay_recording -- --ignored --nocapture
    //! ```
    //! Unlike a live service nothing is dropped for being slow: every sentence gets both passes.
    use std::path::Path;

    use super::*;
    use crate::asr::hardware::{load_wav, Specs};

    struct Pass {
        name: String,
        engine: Box<dyn SpeechEngine>,
        opts: DecodeOptions,
        prompt: String,
        seconds: f64,
        out: serde_json::Map<String, serde_json::Value>,
        refs: serde_json::Map<String, serde_json::Value>,
    }

    impl Pass {
        fn new(model: &str, opts: DecodeOptions, prompt: String, langs: &[Lang]) -> Self {
            let threads = Specs::detect().recommended_threads();
            let mut engine = engine::load(Path::new(model), false, threads).expect("model");
            engine.set_languages(langs);
            let path = Path::new(model);
            let name = if path.is_dir() { path.file_name() } else { path.file_stem() }.unwrap().to_string_lossy().into_owned();
            Self { name, engine, opts, prompt, seconds: 0.0, out: Default::default(), refs: Default::default() }
        }

        fn hear(&mut self, key: &str, audio: &[f32], parser: &mut ReferenceParser, bible: &BibleDb, v11n: &Versification, langs: &[Lang]) -> String {
            let t0 = Instant::now();
            let t = self.engine.transcribe(audio, self.opts, &self.prompt).expect("decode");
            self.seconds += t0.elapsed().as_secs_f64();
            let heard = Heard { text: &t.text, is_final: true, languages: langs, language: t.language, asr_factor: 0.7 + 0.3 * t.confidence.clamp(0.0, 1.0) };
            let found: Vec<String> = if t.text.is_empty() {
                vec![]
            } else {
                find_references(parser, bible, v11n, heard, true, None)
                    .into_iter()
                    .map(|r| format!("{} ({}, {:.0}%)", r.reference.label(), r.kind.name(), r.confidence * 100.0))
                    .collect()
            };
            self.out.insert(key.into(), t.text.clone().into());
            self.refs.insert(key.into(), found.clone().into());
            format!("{} {}{}", self.name, t.text, if found.is_empty() { String::new() } else { format!("  → {}", found.join(", ")) })
        }

        fn save(self, path: String, audio_seconds: f64) {
            let doc = serde_json::json!({ "model": format!("Hearken: {}", self.name), "seconds": self.seconds, "audio_seconds": audio_seconds, "out": self.out, "refs": self.refs });
            std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
            eprintln!("wrote {path}");
        }
    }

    #[test]
    #[ignore = "needs a recording and models"]
    fn replay_recording() {
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let wav = env("HEARKEN_REPLAY").expect("HEARKEN_REPLAY");
        let out = env("HEARKEN_OUT").expect("HEARKEN_OUT");
        let langs = [Lang::En];
        let audio = load_wav(Path::new(&wav)).expect("16 kHz mono wav");
        let name = Path::new(&wav).file_stem().unwrap().to_string_lossy().replace('@', "_");
        let bible = BibleDb::open(&Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/bible.db")).unwrap();
        let v11n = bible.versification().unwrap();

        let live_prompt = std::env::var("HEARKEN_LIVE_PROMPT").unwrap_or_else(|_| whisper::live_prompt("", &langs));
        let accurate_prompt = std::env::var("HEARKEN_ACCURATE_PROMPT").unwrap_or_else(|_| whisper::accurate_prompt("", &[], &langs));
        let mut live = env("HEARKEN_LIVE").map(|m| Pass::new(&m, DecodeOptions::live(false, true), live_prompt, &langs));
        let mut accurate = env("HEARKEN_ACCURATE").map(|m| Pass::new(&m, DecodeOptions::accurate(false), accurate_prompt, &langs));
        // Each pass keeps its own parser, as the live and accuracy threads do ("verse 18" is
        // read against the reference before it).
        let mut parsers = (ReferenceParser::for_languages(&langs), ReferenceParser::for_languages(&langs));

        let mut vad = SpeechDetector::new(false);
        let mut seg = Segmenter::new(SegmenterConfig::default());
        let (mut sentences, mut spoken) = (0usize, 0f64);
        let (mut empty_live, mut empty_accurate) = (0usize, 0usize);
        for (i, frame) in audio.chunks_exact(FRAME_SAMPLES).enumerate() {
            let Some(SegEvent::End { audio: sentence, .. }) = seg.push(frame, vad.is_speech(frame)) else { continue };
            if sentence.len() < ms_to_samples(MIN_FINAL_MS) {
                continue;
            }
            let end = (i + 1) * FRAME_SAMPLES;
            let start = end.saturating_sub(sentence.len()) / SAMPLE_RATE;
            let key = format!("{name}@{start}");
            sentences += 1;
            spoken += sentence.len() as f64 / SAMPLE_RATE as f64;
            eprintln!("{}:{:02} ({:.1} s)", start / 60, start % 60, sentence.len() as f32 / SAMPLE_RATE as f32);
            if let Some(live) = live.as_mut() {
                let line = live.hear(&key, &sentence, &mut parsers.0, &bible, &v11n, &langs);
                empty_live += live.out[&key].as_str().unwrap().trim().is_empty() as usize;
                eprintln!("  {line}");
            }
            if let Some(acc) = accurate.as_mut() {
                let line = acc.hear(&key, &sentence, &mut parsers.1, &bible, &v11n, &langs);
                empty_accurate += acc.out[&key].as_str().unwrap().trim().is_empty() as usize;
                eprintln!("  {line}");
            }
        }
        eprintln!("{sentences} sentences, {spoken:.0} s of speech; empty: live {empty_live}, accuracy pass {empty_accurate}");
        if let Some(live) = live {
            live.save(format!("{out}-live.json"), spoken);
        }
        if let Some(acc) = accurate {
            acc.save(format!("{out}-accurate.json"), spoken);
        }
    }
}
