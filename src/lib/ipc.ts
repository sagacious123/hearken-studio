// Typed bridge to the Rust backend. Keep in sync with src-tauri/src/{commands,pipeline,presentation}.rs.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface ScriptureRef {
  bookId: number;
  chapter: number;
  verseStart: number | null;
  verseEnd: number | null;
}

export interface VerseLine {
  verse: number;
  text: string;
}

export interface SongPart {
  songId: string;
  title: string;
  section: number;
  sections: number;
  ccli?: string;
}

export interface Slide {
  id: string;
  label: string;
  translation: string;
  /** Unused (bookId 0) on song slides. */
  reference: ScriptureRef;
  /** Verses, or lyric lines on song slides. */
  verses: VerseLine[];
  /** The same passage in a second translation, shown side by side. */
  secondary?: { translation: string; verses: VerseLine[] };
  song?: SongPart;
}

/** Follow-along: the live slide has been read/sung up to `offset` (UTF-16) in `verses[line]`. */
export interface Reading {
  slideId: string;
  line: number;
  offset: number;
}

export interface OutputState {
  slide: Slide | null;
  theme: unknown;
  blackout: boolean;
  reading: Reading | null;
  qr: { url: string; svg: string } | null;
  revision: number;
}

export interface TranscriptEvent {
  utteranceId: number;
  text: string;
  isFinal: boolean;
  confidence: number;
  inferenceMs: number;
  audioMs: number;
}

export type DetectionKind = "explicit" | "contextual" | "quote" | "song";

export interface Detection {
  id: string;
  utteranceId: number;
  kind: DetectionKind;
  confidence: number;
  tentative: boolean;
  matched: string;
  slide: Slide;
  autoProjected: boolean;
  /** In the pastor's sermon plan. */
  planned: boolean;
  latencyMs: number;
}

export interface PipelineStatus {
  speaking: boolean;
  levelDb: number;
  stepMs: number;
  inferenceMs: number;
}

export interface PipelineInfo {
  device: string;
  sampleRate: number;
  model: string;
  backend: string;
  stepMs: number;
}

export interface Settings {
  translation: string;
  autoProject: boolean;
  autoThreshold: number;
  inputDevice: string | null;
  modelOverride: string | null;
  gpuOverride: boolean | null;
  reducedEncoderCtx: boolean;
  quoteDetection: boolean;
  secondaryTranslation: string | null;
  followReading: boolean;
  lyricsDetection: boolean;
  shareEnabled: boolean;
  sharePort: number;
  churchName: string;
}

export interface BenchResult {
  model: string;
  backend: string;
  ok: boolean;
  loadMs: number;
  inferMs: number;
  audioMs: number;
  error: string | null;
}

export interface HardwareProfile {
  appVersion: string;
  specs: { os: string; arch: string; totalRamGb: number; logicalCores: number; appleSilicon: boolean };
  tier: "low" | "mid" | "high";
  model: string;
  useGpu: boolean;
  threads: number;
  stepMs: number;
  method: "benchmark" | "heuristic";
  benchmarks: BenchResult[];
}

export interface DeviceInfo {
  id: string;
  name: string;
  kind: "input" | "loopback";
  isDefault: boolean;
}

export interface MonitorInfo {
  index: number;
  name: string;
  width: number;
  height: number;
  x: number;
  y: number;
  scale: number;
  primary: boolean;
}

export interface Translation {
  id: number;
  code: string;
  name: string;
}

export interface SearchResponse {
  reference: Slide | null;
  hits: Slide[];
  elapsedMs: number;
}

export interface ShareInfo {
  enabled: boolean;
  port: number;
  phoneUrl: string;
  overlayUrl: string;
  remoteUrl: string;
  pin: string;
  qrSvg: string;
  /** Why sharing isn't running although it's switched on (e.g. port in use). */
  error: string | null;
}

export interface PlanInfo {
  text: string;
  slides: Slide[];
}

export interface SongSummary {
  id: string;
  title: string;
  author: string | null;
  ccli: string | null;
  sections: string[];
}

export interface DaySummary {
  date: string;
  firstAt: string | null;
  lastAt: string | null;
  scriptures: number;
  songs: number;
  words: number;
}

export interface ArchiveHit {
  date: string;
  time: string;
  kind: "said" | "shown" | "mentioned";
  label: string | null;
  text: string;
}

export interface Insights {
  year: number;
  services: number;
  scripturesShown: number;
  oldTestament: number;
  newTestament: number;
  books: [string, number][];
  verses: [string, number][];
  songs: [string, number][];
}

export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  getHardwareProfile: () => invoke<HardwareProfile | null>("get_hardware_profile"),
  runHardwareBenchmark: () => invoke<HardwareProfile>("run_hardware_benchmark"),
  listAudioDevices: () => invoke<DeviceInfo[]>("list_audio_devices"),
  startPipeline: () => invoke<PipelineInfo>("start_pipeline"),
  stopPipeline: () => invoke<void>("stop_pipeline"),
  listTranslations: () => invoke<Translation[]>("list_translations"),
  searchScripture: (query: string, translation: string) =>
    invoke<SearchResponse>("search_scripture", { query, translation }),
  buildSlide: (reference: ScriptureRef, translation: string) =>
    invoke<Slide | null>("build_slide", { reference, translation }),
  getOutputState: () => invoke<OutputState>("get_output_state"),
  present: (slide: Slide) => invoke<void>("present", { slide }),
  clearOutput: () => invoke<void>("clear_output"),
  setBlackout: (on: boolean) => invoke<void>("set_blackout", { on }),
  setTheme: (theme: unknown) => invoke<void>("set_theme", { theme }),
  stepVerse: (delta: number) => invoke<void>("step_verse", { delta }),
  listMonitors: () => invoke<MonitorInfo[]>("list_monitors"),
  openOutputWindow: (monitor: number | null) => invoke<void>("open_output_window", { monitor }),
  closeOutputWindow: () => invoke<void>("close_output_window"),
  getShareInfo: () => invoke<ShareInfo>("get_share_info"),
  showQr: (on: boolean) => invoke<void>("show_qr", { on }),
  getPlan: () => invoke<PlanInfo>("get_plan"),
  setPlan: (text: string) => invoke<PlanInfo>("set_plan", { text }),
  listSongs: () => invoke<SongSummary[]>("list_songs"),
  getSongText: (id: string) => invoke<string>("get_song_text", { id }),
  saveSong: (text: string, id: string | null) => invoke<SongSummary[]>("save_song", { text, id }),
  deleteSong: (id: string) => invoke<SongSummary[]>("delete_song", { id }),
  songSlide: (id: string, section: number) => invoke<Slide | null>("song_slide", { id, section }),
  listSessions: () => invoke<DaySummary[]>("list_sessions"),
  searchArchive: (query: string) => invoke<ArchiveHit[]>("search_archive", { query }),
  getInsights: (year: number) => invoke<Insights>("get_insights", { year }),
  exportRecap: (date: string) => invoke<string>("export_recap", { date }),
  exportCcliReport: (days: number) => invoke<string>("export_ccli_report", { days }),
};

const on =
  <T,>(name: string) =>
  (cb: (payload: T) => void): Promise<UnlistenFn> =>
    listen<T>(name, (e) => cb(e.payload));

export const events = {
  transcript: on<TranscriptEvent>("pipeline://transcript"),
  detection: on<Detection>("pipeline://detection"),
  status: on<PipelineStatus>("pipeline://status"),
  pipelineError: on<string>("pipeline://error"),
  /** Listening continues; "" clears the warning. */
  pipelineWarning: on<string>("pipeline://warning"),
  outputState: on<OutputState>("output://state"),
  hardwareProgress: on<string>("hardware://progress"),
};
