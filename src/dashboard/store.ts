import { create } from "zustand";
import {
  api,
  events,
  type Detection,
  type HardwareProfile,
  type LicenceInfo,
  type OutputState,
  type PipelineInfo,
  type PipelineStatus,
  type Settings,
  type Slide,
  type TranscriptEvent,
  type VoiceCommand,
  type Translation,
  type LanguageOption,
} from "../lib/ipc";
import { setUiLanguage, t } from "../lib/i18n";
import type { Theme } from "../lib/theme";

const MAX_FINALS = 60;
const MAX_QUEUE = 40;

export interface TranscriptLine {
  utteranceId: number;
  text: string;
  confidence: number;
  inferenceMs: number;
  /** Replaced by the accuracy pass. */
  refined: boolean;
  /** When it was heard (ms since epoch). */
  at: number;
  /** Sung rather than spoken. */
  sung: boolean;
}

interface DashboardState {
  running: boolean;
  starting: boolean;
  info: PipelineInfo | null;
  status: PipelineStatus | null;
  error: string | null;
  benchProgress: string | null;

  finals: TranscriptLine[];
  partial: TranscriptEvent | null;
  queue: Detection[];
  /** Spoken commands, by the sentence that gave them. */
  voiceCommands: Record<number, VoiceCommand>;
  /** Since when the room has been singing (ms since epoch), or null. */
  singingSince: number | null;
  /** When a song from the library was last recognised (ms since epoch). */
  lastSongAt: number | null;
  lastLatencyMs: number | null;

  preview: Slide | null;
  followLatest: boolean;
  live: OutputState | null;
  /** Unapplied style edits: shown in Preview, not on the projector. */
  draftTheme: Theme | null;
  setDraftTheme: (theme: Theme | null) => void;
  /** The Style drawer (slides in from the left; Preview stays visible). */
  styleOpen: boolean;
  setStyleOpen: (open: boolean) => void;
  licence: LicenceInfo | null;
  /** The Licence panel is open. */
  licenceOpen: boolean;
  setLicenceOpen: (open: boolean) => void;
  /** The sign-in screen is open. */
  signInOpen: boolean;
  setSignInOpen: (open: boolean) => void;
  /** The Language dialog is open. */
  languageOpen: boolean;
  setLanguageOpen: (open: boolean) => void;
  /** The languages Hearken can listen for. */
  languages: LanguageOption[];
  /** Downloads for the preaching languages ("bible:LSG", "model:base"), 0–1. */
  setup: Record<string, { label: string; fraction: number }>;
  setupError: string | null;
  /** Save the preaching and menu languages and start downloading what they need. */
  applyLanguages: (languages: string[], uiLanguage: Settings["uiLanguage"]) => Promise<void>;

  settings: Settings | null;
  translations: Translation[];
  profile: HardwareProfile | null;

  init: () => Promise<() => void>;
  start: () => Promise<void>;
  stop: () => Promise<void>;
  saveSettings: (patch: Partial<Settings>) => Promise<void>;
  setPreview: (slide: Slide | null, pin?: boolean) => void;
  goLive: (slide?: Slide | null) => Promise<void>;
  dismiss: (id: string) => void;
  rebenchmark: () => Promise<void>;
}

/**
 * A scripture slide built for other translations than the ones chosen now (side by side turned
 * on or off, or the translation changed since it was detected or previewed).
 */
const outdated = (slide: Slide, s: Settings | null) => {
  if (!s || slide.song) return false;
  const want = s.secondaryTranslation && s.secondaryTranslation.toUpperCase() !== s.translation.toUpperCase() ? s.secondaryTranslation.toUpperCase() : null;
  return slide.translation.toUpperCase() !== s.translation.toUpperCase() || (slide.secondary?.translation.toUpperCase() ?? null) !== want;
};

/** The same passage in the translations chosen now (unchanged if it can't be rebuilt). */
const current = async (slide: Slide, s: Settings | null): Promise<Slide> => {
  if (!s || !outdated(slide, s)) return slide;
  try {
    return (await api.buildSlide(slide.reference, s.translation)) ?? slide;
  } catch {
    return slide;
  }
};

const sameJson = (a: unknown, b: unknown) => a === b || JSON.stringify(a) === JSON.stringify(b);

const sameChapter = (a: Detection, b: Detection) =>
  a.slide.reference.bookId === b.slide.reference.bookId && a.slide.reference.chapter === b.slide.reference.chapter;

/** Rebuild the Preview slide for `settings`, unless something else was previewed meanwhile. */
function refreshPreview(settings: Settings | null) {
  const before = useDashboard.getState().preview;
  if (!before || !outdated(before, settings)) return;
  void current(before, settings).then((fresh) => {
    if (fresh !== before && useDashboard.getState().preview === before) useDashboard.setState({ preview: fresh });
  });
}

/** A setup download made progress. */
function progress(key: string, fraction: number) {
  const s = useDashboard.getState();
  const d = s.setup[key];
  if (d) useDashboard.setState({ setup: { ...s.setup, [key]: { ...d, fraction } } });
}

/**
 * Show scripture in a preaching language: when the translation on screen isn't in any of
 * `languages`, switch to the first language's Bible (once it's downloaded).
 */
async function showBibleFor(languages: string[]) {
  const { settings, translations, languages: options, saveSettings } = useDashboard.getState();
  if (!settings || languages.length === 0) return;
  const current = settings.translation.toUpperCase();
  const bundled = translations.filter((tr) => !tr.downloaded).map((tr) => tr.code.toUpperCase());
  const currentLang = bundled.includes(current) ? "en" : options.find((o) => o.bible.toUpperCase() === current)?.code;
  if (currentLang && languages.includes(currentLang)) return;
  const want = options.find((o) => o.code === languages[0])?.bible;
  const installed = translations.find((tr) => tr.code.toUpperCase() === want?.toUpperCase());
  if (installed) await saveSettings({ translation: installed.code });
}

export const useDashboard = create<DashboardState>((set, get) => ({
  running: false,
  starting: false,
  info: null,
  status: null,
  error: null,
  benchProgress: null,
  finals: [],
  partial: null,
  queue: [],
  voiceCommands: {},
  singingSince: null,
  lastSongAt: null,
  lastLatencyMs: null,
  preview: null,
  followLatest: true,
  live: null,
  draftTheme: null,
  setDraftTheme: (draftTheme) => set({ draftTheme }),
  styleOpen: false,
  setStyleOpen: (styleOpen) => set({ styleOpen }),
  licence: null,
  licenceOpen: false,
  setLicenceOpen: (licenceOpen) => set({ licenceOpen }),
  signInOpen: false,
  setSignInOpen: (signInOpen) => set({ signInOpen }),
  languageOpen: false,
  setLanguageOpen: (languageOpen) => set({ languageOpen }),
  languages: [],
  setup: {},
  setupError: null,
  settings: null,
  translations: [],
  profile: null,

  init: async () => {
    const [settings, translations, profile, live, info] = await Promise.all([
      api.getSettings(),
      api.listTranslations(),
      api.getHardwareProfile(),
      api.getOutputState(),
      // Listening carries on if the dashboard reloads: pick the session back up.
      api.pipelineInfo().catch(() => null),
    ]);
    setUiLanguage(settings.uiLanguage);
    set({ settings, translations, profile, live, ...(info ? { running: true, info } : {}) });
    void api.licenceStatus().then((licence) => set({ licence })).catch(console.error);
    void api.listLanguages().then((languages) => set({ languages })).catch(console.error);

    const unlisteners = await Promise.all([
      events.transcript((t) => {
        if (!t.isFinal) {
          set({ partial: t.text ? t : null });
          return;
        }
        const line = { utteranceId: t.utteranceId, text: t.text, confidence: t.confidence, inferenceMs: t.inferenceMs, refined: t.refined, at: Date.now(), sung: t.sung };
        if (t.refined) {
          // The accuracy pass replaces the quick text of the same sentence (or fills it in).
          if (!t.text) return;
          set((s) => {
            const i = s.finals.findIndex((f) => f.utteranceId === t.utteranceId);
            if (i >= 0) return { finals: s.finals.map((f, j) => (j === i ? { ...line, at: f.at } : f)) };
            return { finals: [...s.finals, line].sort((a, b) => a.utteranceId - b.utteranceId).slice(-MAX_FINALS) };
          });
          return;
        }
        set((s) => ({
          partial: s.partial?.utteranceId === t.utteranceId ? null : s.partial,
          finals: t.text ? [...s.finals, line].slice(-MAX_FINALS) : s.finals,
          // Tentative detections that the final transcript didn't confirm are discarded.
          // (Confirmed ones are re-emitted right after this event with tentative=false.)
          queue: s.queue.filter((d) => !(d.tentative && d.utteranceId === t.utteranceId)),
        }));
      }),
      events.detection((d) => {
        set((s) => {
          const queue = s.queue
            .filter((q) => q.id !== d.id)
            // "John 3" (tentative) is superseded by "John 3:16" from the same utterance.
            .filter((q) => !(q.tentative && !d.tentative && q.utteranceId === d.utteranceId && sameChapter(q, d)));
          const next = [d, ...queue].slice(0, MAX_QUEUE);
          const preview = s.followLatest && !d.tentative ? d.slide : s.preview;
          return { queue: next, preview, lastLatencyMs: d.latencyMs, lastSongAt: d.kind === "song" ? Date.now() : s.lastSongAt };
        });
      }),
      events.voiceCommand((c) => set((s) => ({ voiceCommands: { ...s.voiceCommands, [c.utteranceId]: c } }))),
      events.licence((licence) => set({ licence })),
      events.singing((on) => set((s) => ({ singingSince: on ? (s.singingSince ?? Date.now()) : null }))),
      events.status((status) => set({ status })),
      events.pipelineError((error) => set({ error, running: false })),
      events.pipelineWarning((warning) => set({ error: warning || null })),
      // Keep the theme object identical while its content is: output events arrive with every
      // follow-along highlight, and a "new" theme would make every preview refit its text.
      events.outputState((live) =>
        set((s) => ({ live: s.live && sameJson(s.live.theme, live.theme) ? { ...live, theme: s.live.theme } : live })),
      ),
      events.hardwareProgress((benchProgress) => set({ benchProgress })),
      events.libraryProgress((p) => progress(`bible:${p.code}`, p.fraction)),
      events.modelProgress((p) => p.phase === "download" && progress(`model:${p.id}`, p.received / Math.max(1, p.total))),
    ]);
    return () => unlisteners.forEach((u) => u());
  },

  start: async () => {
    set({ starting: true, error: null });
    try {
      const info = await api.startPipeline();
      const profile = await api.getHardwareProfile();
      set({ running: true, info, profile });
    } catch (e) {
      // Not signed in: the sign-in screen. The trial has ended, or the date looks wrong: the
      // Licence panel explains what to do.
      if (String(e).startsWith("SIGNIN:")) {
        set({ signInOpen: true, licence: await api.licenceStatus().catch(() => get().licence) });
      } else if (String(e).startsWith("LICENCE:")) {
        set({ licenceOpen: true, licence: await api.licenceStatus().catch(() => get().licence) });
      } else if (String(e).startsWith("LANGUAGE:")) {
        // The speech model for the preaching language isn't downloaded: the Language dialog has it.
        set({ languageOpen: true });
      } else {
        set({ error: String(e) });
      }
    } finally {
      set({ starting: false, benchProgress: null });
    }
  },

  stop: async () => {
    await api.stopPipeline();
    set({ running: false, status: null, partial: null, singingSince: null });
  },

  saveSettings: async (patch) => {
    const existing = get().settings;
    if (!existing) return;
    const settings = await api.updateSettings({ ...existing, ...patch });
    setUiLanguage(settings.uiLanguage);
    set({ settings });
    // Preview follows a change of translation or side by side straight away.
    refreshPreview(settings);
  },

  setPreview: (slide, pin = true) => {
    set({ preview: slide, followLatest: !pin });
    // A card detected before the translations changed is shown as they are now.
    if (slide && outdated(slide, get().settings)) refreshPreview(get().settings);
  },

  goLive: async (slide) => {
    const target = slide ?? get().preview;
    if (target) await api.present(await current(target, get().settings));
  },

  dismiss: (id) => set((s) => ({ queue: s.queue.filter((d) => d.id !== id) })),

  applyLanguages: async (languages, uiLanguage) => {
    await get().saveSettings({ languages, uiLanguage });
    set({ setupError: null });
    const need = await api.languageSetup(languages);
    const run = async (key: string, label: string, download: () => Promise<unknown>) => {
      if (get().setup[key]) return; // already downloading
      set((s) => ({ setup: { ...s.setup, [key]: { label, fraction: 0 } } }));
      try {
        await download();
      } catch (e) {
        set({ setupError: String(e) });
      } finally {
        set(({ setup: { [key]: _, ...rest } }) => ({ setup: rest }));
      }
    };
    const bibles = need.bibles
      .filter((b) => !b.installed)
      .map((b) =>
        run(`bible:${b.code}`, b.name, async () => {
          set({ translations: await api.installTranslation(b.code) });
          await showBibleFor(get().settings?.languages ?? languages);
        }),
      );
    const names = languages.filter((c) => c !== "en").map((c) => t(get().languages.find((l) => l.code === c)?.name ?? c));
    const speech = need.speechModel;
    const model = speech && !speech.installed
      ? run(`model:${speech.id}`, t("{language} speech model", { language: names.join(" + ") }), () => api.downloadModel(speech.id))
      : null;
    await showBibleFor(languages);
    // Returns once the settings are saved; the downloads carry on (`setup` shows them).
    void Promise.all([...bibles, model]);
  },

  rebenchmark: async () => {
    set({ benchProgress: "Starting benchmark…", error: null });
    try {
      set({ profile: await api.runHardwareBenchmark() });
    } catch (e) {
      set({ error: String(e) });
    } finally {
      set({ benchProgress: null });
    }
  },
}));
