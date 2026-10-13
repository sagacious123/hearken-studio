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
        Arc,
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
        whisper::{self, DecodeOptions, WhisperEngine},
    },
    audio::{
        capture::{self, CaptureHandle},
        ms_to_samples,
        resample::Resampler16k,
        segmenter::{SegEvent, Segmenter, SegmenterConfig},
        vad::SpeechDetector,
        LevelMeter, FRAME_SAMPLES, SAMPLE_RATE,
    },
    events,
    follow::{self, Follower, LyricsIndex},
    scripture::{
        build_slide,
        parser::{ParsedRef, RefKind, ReferenceParser},
        books,
        meaning::{self, Meaning},
        quote, BibleDb, Slide, Versification,
    },
    state::AppState,
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

    // 0. Accuracy-pass thread. Loads in the background (a large model can take seconds), and
    //    never blocks or fails listening: until it's ready, or if it can't load, sentences just
    //    keep their quick transcription.
    let (refine_tx, refine_thread, refine_model) = match cfg.refine {
        Some(rc) => {
            let (tx, rx) = crossbeam_channel::bounded::<RefineJob>(REFINE_QUEUE);
            let (app, v11n, bible_path, dedupe) = (app.clone(), v11n.clone(), cfg.bible_path.clone(), dedupe.clone());
            let name = rc.model_name.clone();
            let handle = thread::Builder::new().name("asr-refine".into()).spawn(move || {
                priority::background();
                let engine = WhisperEngine::load_with_fallback(&rc.model_path, rc.use_gpu, rc.threads);
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
                        let books: Vec<&str> = books::BOOKS.iter().map(|b| b.display).collect();
                        let prompt = whisper::accurate_prompt("", &[], &books);
                        match hardware::load_wav(&rc.bench_wav).and_then(|a| accuracy::calibrate(&mut engine, &rc.model_name, &a, &prompt)) {
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
                RefineWorker { app, bible, v11n, parser: ReferenceParser::new(), dedupe, opts: cal.options() }.run(&mut engine, rx);
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
        let step_ms = step_ms.clone();
        let infer_ewma = infer_ewma.clone();
        let (model_path, use_gpu, threads, reduced_ctx, bible_path, base_step) =
            (cfg.model_path.clone(), cfg.use_gpu, cfg.threads, cfg.reduced_ctx, cfg.bible_path.clone(), cfg.step_ms);
        thread::Builder::new().name("asr".into()).spawn(move || {
            priority::interactive();
            let engine = WhisperEngine::load_with_fallback(&model_path, use_gpu, threads);
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
                parser: ReferenceParser::new(),
                follower: Follower::default(),
                lyrics: LyricsIndex::default(),
                dedupe,
                refine: refine_tx,
                step_ms,
                infer_ewma,
                base_step,
                reduced_ctx,
            };
            worker.run(&mut engine, job_rx);
            log::info!("asr thread stopped");
        })?
    };
    let backend = ready_rx.recv().map_err(|_| anyhow!("ASR thread exited during startup"))??;

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
            if let Err(e) = dsp_loop(consumer, rate, job_tx, &stop, &step_ms, &infer_ewma, &app) {
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
    // Joined in order: DSP ends, then the live ASR thread (dropping the accuracy-pass sender),
    // then the accuracy pass drains and ends.
    let threads = [Some(dsp), Some(asr), refine_thread].into_iter().flatten().collect();
    Ok(PipelineHandle { stop, capture: Some(capture), threads, info })
}

fn dsp_loop(
    mut consumer: HeapCons<f32>,
    input_rate: u32,
    jobs: Sender<Job>,
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
    follower: Follower,
    lyrics: LyricsIndex,
    dedupe: Dedupe,
    /// Finished sentences go here too, for the accuracy pass.
    refine: Option<Sender<RefineJob>>,
    step_ms: Arc<AtomicU32>,
    infer_ewma: Arc<AtomicU32>,
    base_step: u32,
    reduced_ctx: bool,
}

impl AsrWorker {
    fn run(&mut self, engine: &mut WhisperEngine, jobs: Receiver<Job>) {
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
                    self.process(engine, job);
                }
            }
        }
    }

    fn process(&mut self, engine: &mut WhisperEngine, job: Job) {
        let settings = self.app.state::<AppState>().settings.lock().clone();
        // Hand a finished sentence to the accuracy pass first, so both decode in parallel.
        // If it's busy, this sentence keeps the quick text (logged below).
        let refining = job.is_final
            && self.refine.as_ref().is_some_and(|tx| {
                let refine_job = RefineJob { utterance: job.utterance, audio: job.audio.clone(), captured_at: job.captured_at };
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
        let transcript = match engine.transcribe(&job.audio, opts, &whisper::live_prompt(&settings.vocabulary)) {
            Ok(t) => t,
            Err(e) => {
                log::warn!("transcription failed: {e:#}");
                return;
            }
        };
        let inference_ms = t0.elapsed().as_secs_f32() * 1000.0;

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
            },
        );
        if transcript.text.is_empty() {
            return;
        }

        // Cloned handle so `state` doesn't borrow `self` across the `&mut self` calls below.
        let app = self.app.clone();
        let state = app.state::<AppState>();
        let asr_factor = 0.7 + 0.3 * transcript.confidence.clamp(0.0, 1.0);
        if job.is_final && !refining {
            // With the accuracy pass on, the archive gets its (better) text instead.
            state.history.said(&transcript.text);
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

        // Worship songs: which imported song is being sung (a new one, not the live one).
        if settings.lyrics_detection {
            let live_song = live.slide.as_ref().and_then(|s| s.song.as_ref()).map(|p| p.song_id.clone());
            let found = self.lyrics.detect(&transcript.text).filter(|m| m.slide.song.as_ref().map(|p| &p.song_id) != live_song.as_ref());
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
        let refs = find_references(&mut self.parser, &self.bible, &self.v11n, &transcript.text, job.is_final, settings.quote_detection, meaning.as_deref(), asr_factor);
        emit_references(&app, &state, &settings, &self.bible, &self.dedupe, refs, job.utterance, job.captured_at);
    }
}

/// References in a transcript: announced ("John 3:16") and, on finished sentences with nothing
/// announced, quoted verses.
#[allow(clippy::too_many_arguments)]
fn find_references(
    parser: &mut ReferenceParser,
    bible: &BibleDb,
    v11n: &Versification,
    text: &str,
    is_final: bool,
    quote_detection: bool,
    meaning: Option<&Meaning>,
    asr_factor: f32,
) -> Vec<ParsedRef> {
    let mut refs = parser.parse(text, is_final, v11n);
    for r in &mut refs {
        r.confidence *= asr_factor;
    }
    if refs.is_empty() && is_final && quote_detection {
        match quote::find_quotes(bible, text) {
            Ok(hits) => refs.extend(hits.into_iter().map(|hit| ParsedRef {
                reference: hit.reference,
                confidence: hit.score * asr_factor,
                tentative: false,
                kind: RefKind::Quote,
                matched: hit.phrase,
            })),
            Err(e) => log::warn!("quote search failed: {e:#}"),
        }
    }
    // Nothing said or quoted word for word: a verse said in other words?
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
    utterance: u64,
    captured_at: Instant,
) {
    for mut r in refs {
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
        let auto = settings.auto_project && !r.tentative && r.confidence >= threshold;
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
}

impl RefineWorker {
    fn run(&mut self, engine: &mut WhisperEngine, jobs: Receiver<RefineJob>) {
        let books: Vec<&str> = books::BOOKS.iter().map(|b| b.display).collect();
        while let Ok(job) = jobs.recv() {
            let app = self.app.clone();
            let state = app.state::<AppState>();
            let settings = state.settings.lock().clone();
            let planned: Vec<String> = state.plan.lock().refs.iter().map(|r| r.label()).collect();
            let prompt = whisper::accurate_prompt(&settings.vocabulary, &planned, &books);

            let t0 = Instant::now();
            let transcript = match engine.transcribe(&job.audio, self.opts, &prompt) {
                Ok(t) => t,
                Err(e) => {
                    log::warn!("accuracy pass failed: {e:#}");
                    continue;
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
                },
            );
            if transcript.text.is_empty() {
                continue;
            }
            state.history.said(&transcript.text);
            let asr_factor = 0.7 + 0.3 * transcript.confidence.clamp(0.0, 1.0);
            let meaning = settings.meaning_detection.then(|| state.meaning.lock().clone()).flatten();
            let refs = find_references(&mut self.parser, &self.bible, &self.v11n, &transcript.text, true, settings.quote_detection, meaning.as_deref(), asr_factor);
            emit_references(&app, &state, &settings, &self.bible, &self.dedupe, refs, job.utterance, job.captured_at);
        }
    }
}
