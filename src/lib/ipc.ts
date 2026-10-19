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

/** The free trial or the church's licence. Times are Unix seconds. */
export type LicenceStatus =
  | { state: "trial"; ends: number; daysLeft: number; registered: boolean }
  | { state: "trialOver"; ended: number }
  | { state: "licensed"; church: string; plan: string; expires: number; inGrace: boolean }
  | { state: "licenceExpired"; church: string; expired: number }
  | { state: "clockBehind"; now: number; seen: number };

export type AccountState =
  | { state: "signedIn"; email: string; church: string; until: number }
  | { state: "signedOut" }
  | { state: "expired"; email: string };

/** One of the church account's computers, as the website lists them. */
export interface Computer {
  id: string;
  name: string;
  /** Unix seconds. */
  last: number;
}

/** Signed in, or the account is on its most computers and one must be signed out first. */
export type SignIn =
  | { state: "signedIn"; info: LicenceInfo }
  | { state: "tooManyComputers"; limit: number; computers: Computer[]; message: string };

/** Creating the account: a code was emailed to confirm the address, or (an older website) signed in. */
export type SignUp = { state: "verify"; email: string } | { state: "signedIn"; info: LicenceInfo };

/** Sign-in, trial and licence together: listening needs a signed-in account and a trial or licence. */
export interface LicenceInfo {
  status: LicenceStatus;
  account: AccountState;
  canListen: boolean;
}

export interface TranscriptEvent {
  utteranceId: number;
  text: string;
  isFinal: boolean;
  confidence: number;
  inferenceMs: number;
  audioMs: number;
  /** From the accuracy pass: replaces the quick text of the same utterance. */
  refined: boolean;
  /** Sung rather than spoken (finished sentences only). */
  sung: boolean;
}

export type DetectionKind = "explicit" | "contextual" | "quote" | "song" | "meaning" | "command";

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
  /** Accuracy-pass model, if running. */
  refineModel: string | null;
}

/** An extra screen beside the main output. */
export interface ScreenConfig {
  look: "full" | "lower-third" | "stage";
  key: "black" | "green" | "theme";
  monitor: number | null;
}

/** A spoken command that moved (or tried to move) the live scripture. */
export interface VoiceCommand {
  utteranceId: number;
  label: string;
  /** False when there was nowhere to go (already at the first or last verse). */
  moved: boolean;
  /** What's on screen now. */
  now: string | null;
}

export type RecapFormat = "pdf" | "docx" | "doc" | "html";

export interface Settings {
  translation: string;
  autoProject: boolean;
  autoThreshold: number;
  inputDevice: string | null;
  modelOverride: string | null;
  gpuOverride: boolean | null;
  reducedEncoderCtx: boolean;
  quoteDetection: boolean;
  /** Suggest verses said in other words (needs the search-by-meaning model). */
  meaningDetection: boolean;
  /** Spoken commands for the live scripture: "next verse", "go back", "go to the first verse". */
  voiceCommands: boolean;
  /** Extra screens beside the main output (up to three). */
  screens: ScreenConfig[];
  secondaryTranslation: string | null;
  followReading: boolean;
  lyricsDetection: boolean;
  shareEnabled: boolean;
  /** Fallback port when 80 isn't available. */
  sharePort: number;
  /** Friendly network name: phones open http://<name>.local. */
  shareName: string;
  churchName: string;
  /** What phones can open from "Recap". */
  phoneRecap: "notes" | "scriptures" | "off";
  /** "auto" | "off" | a model id. */
  accuracyModel: string;
  /** Names and words to listen for, comma or line separated. */
  vocabulary: string;
  /** Languages preached, as codes ("en", "fr", "es"). */
  languages: string[];
  /** Language of Hearken's menus and buttons. */
  uiLanguage: "en" | "fr" | "es";
  /** The first-run screens have been seen. */
  onboarded: boolean;
}

/** A language Hearken can listen for. */
export interface LanguageOption {
  code: string;
  /** In English ("French") and in the language itself ("Français"). */
  name: string;
  native: string;
  /** The Bible shown by default when it's preached. */
  bible: string;
}

/** What listening in some languages needs downloaded. */
export interface LanguageSetup {
  /** Their default Bibles (bundled English ones aren't listed). */
  bibles: LibraryItem[];
  /** The speech model they need on this computer, unless it ships with the app. */
  speechModel: Omit<ModelStatus, "usable" | "speed"> | null;
}

export interface ModelStatus {
  id: string;
  label: string;
  sizeMb: number;
  bundled: boolean;
  /** Usable for the accuracy pass. */
  accuracy: boolean;
  installed: boolean;
  note: string;
  /** From the speed test on this computer: null = not tested yet, false = too slow. */
  usable: boolean | null;
  speed: string | null;
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
  /** Copyright or licence line. */
  license: string | null;
  /** From the Bible library (can be removed). */
  downloaded: boolean;
}

/** A translation that can be downloaded into the Bible library. */
export interface LibraryItem {
  code: string;
  name: string;
  language: string;
  notice: string;
  mb: number;
  installed: boolean;
}

export interface BookInfo {
  id: number;
  name: string;
  /** The name in other languages, by code ("fr": "Jean"). */
  names: Record<string, string>;
  /** Lowercase names people type: "jn", "1 cor", "psalm". */
  aliases: string[];
  /** Verses per chapter (index 0 = chapter 1). */
  verses: number[];
}

export interface SearchResponse {
  reference: Slide | null;
  hits: Slide[];
  /** Verses saying the same thing in other words (search by meaning). */
  meaning: Slide[];
  elapsedMs: number;
}

export interface MeaningStatus {
  installed: boolean;
  ready: boolean;
  downloadMb: number;
}

export interface UploadedBackground {
  /** File in the backgrounds folder; themes refer to it as "upload:<file>". */
  file: string;
  name: string;
  path: string;
}

export interface ShareInfo {
  enabled: boolean;
  port: number;
  /** Friendly addresses (http://hearken.local …). */
  phoneUrl: string;
  overlayUrl: string;
  /** The overlay by 127.0.0.1, for OBS on this computer. */
  overlayHereUrl: string;
  remoteUrl: string;
  /** Same pages by IP address, for devices that can't open ".local" names. */
  ipUrl: string;
  name: string;
  nameStatus: "checking" | "announced" | "conflict" | "failed";
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
  /** The website's changelog at `version`, in the browser. */
  openChangelog: (version: string) => invoke<void>("open_changelog", { version }),
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  getHardwareProfile: () => invoke<HardwareProfile | null>("get_hardware_profile"),
  runHardwareBenchmark: () => invoke<HardwareProfile>("run_hardware_benchmark"),
  listAudioDevices: () => invoke<DeviceInfo[]>("list_audio_devices"),
  startPipeline: () => invoke<PipelineInfo>("start_pipeline"),
  stopPipeline: () => invoke<void>("stop_pipeline"),
  licenceStatus: () => invoke<LicenceInfo>("licence_status"),
  activateLicence: (key: string) => invoke<LicenceInfo>("activate_licence", { key }),
  /** Registers the trial, renews the sign-in and checks the clock against the website's. */
  checkLicenceOnline: () => invoke<LicenceInfo>("check_licence_online"),
  /** `replace` signs out one of the account's other computers to make room for this one. */
  signIn: (email: string, password: string, replace: string | null = null) => invoke<SignIn>("sign_in", { email, password, replace }),
  /** Starts creating the account: a code is emailed to confirm the address. */
  signUp: (church: string, email: string, password: string) => invoke<SignUp>("sign_up", { church, email, password }),
  confirmSignUp: (email: string, code: string) => invoke<LicenceInfo>("confirm_sign_up", { email, code }),
  signOut: () => invoke<LicenceInfo>("sign_out"),
  forgotPassword: (email: string) => invoke<void>("forgot_password", { email }),
  /** The running session, if listening. */
  pipelineInfo: () => invoke<PipelineInfo | null>("pipeline_info"),
  listTranslations: () => invoke<Translation[]>("list_translations"),
  listLanguages: () => invoke<LanguageOption[]>("list_languages"),
  /** What listening in `languages` needs downloaded: their Bibles and the speech model. */
  languageSetup: (languages: string[]) => invoke<LanguageSetup>("language_setup", { languages }),
  listSystemFonts: () => invoke<string[]>("list_system_fonts"),
  openScreen: (n: number) => invoke<number[]>("open_screen", { n }),
  closeScreen: (n: number) => invoke<number[]>("close_screen", { n }),
  listOpenScreens: () => invoke<number[]>("list_open_screens"),
  meaningStatus: () => invoke<MeaningStatus>("meaning_status"),
  downloadMeaningModel: () => invoke<MeaningStatus>("download_meaning_model"),
  removeMeaningModel: () => invoke<MeaningStatus>("remove_meaning_model"),
  listLibrary: () => invoke<LibraryItem[]>("list_library"),
  installTranslation: (code: string) => invoke<Translation[]>("install_translation", { code }),
  removeTranslation: (code: string) => invoke<Translation[]>("remove_translation", { code }),
  searchScripture: (query: string, translation: string) =>
    invoke<SearchResponse>("search_scripture", { query, translation }),
  listBooks: () => invoke<BookInfo[]>("list_books"),
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
  exportRecap: (date: string, format: RecapFormat) => invoke<string>("export_recap", { date, format }),
  exportCcliReport: (days: number) => invoke<string>("export_ccli_report", { days }),
  listBackgrounds: () => invoke<UploadedBackground[]>("list_backgrounds"),
  /** Sends the raw bytes (fast) with the original name in a header. */
  saveBackground: async (file: File) =>
    invoke<UploadedBackground[]>("save_background", new Uint8Array(await file.arrayBuffer()), {
      headers: { "x-file-name": encodeURIComponent(file.name) },
    }),
  deleteBackground: (file: string) => invoke<UploadedBackground[]>("delete_background", { file }),
  backgroundPath: (file: string) => invoke<string | null>("background_path", { file }),
  listModels: () => invoke<ModelStatus[]>("list_models"),
  downloadModel: (id: string) => invoke<ModelStatus[]>("download_model", { id }),
  cancelModelDownload: (id: string) => invoke<void>("cancel_model_download", { id }),
  deleteModel: (id: string) => invoke<ModelStatus[]>("delete_model", { id }),
  testModelSpeed: (id: string) => invoke<ModelStatus[]>("test_model_speed", { id }),
};

const on =
  <T,>(name: string) =>
  (cb: (payload: T) => void): Promise<UnlistenFn> =>
    listen<T>(name, (e) => cb(e.payload));

export const events = {
  transcript: on<TranscriptEvent>("pipeline://transcript"),
  detection: on<Detection>("pipeline://detection"),
  voiceCommand: on<VoiceCommand>("pipeline://voice-command"),
  licence: on<LicenceInfo>("licence://status"),
  /** The room started (true) or stopped (false) singing. */
  singing: on<boolean>("pipeline://singing"),
  status: on<PipelineStatus>("pipeline://status"),
  pipelineError: on<string>("pipeline://error"),
  /** Listening continues; "" clears the warning. */
  pipelineWarning: on<string>("pipeline://warning"),
  outputState: on<OutputState>("output://state"),
  hardwareProgress: on<string>("hardware://progress"),
  meaningProgress: on<{ received: number; total: number }>("meaning://progress"),
  libraryProgress: on<{ code: string; fraction: number }>("library://progress"),
  modelProgress: on<{ id: string; received: number; total: number; phase: "download" | "testing" }>("models://progress"),
};
