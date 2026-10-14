import { create } from "zustand";
import {
  api,
  events,
  type Detection,
  type HardwareProfile,
  type OutputState,
  type PipelineInfo,
  type PipelineStatus,
  type Settings,
  type Slide,
  type TranscriptEvent,
  type Translation,
} from "../lib/ipc";
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

const sameJson = (a: unknown, b: unknown) => a === b || JSON.stringify(a) === JSON.stringify(b);

const sameChapter = (a: Detection, b: Detection) =>
  a.slide.reference.bookId === b.slide.reference.bookId && a.slide.reference.chapter === b.slide.reference.chapter;

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
  lastLatencyMs: null,
  preview: null,
  followLatest: true,
  live: null,
  draftTheme: null,
  setDraftTheme: (draftTheme) => set({ draftTheme }),
  styleOpen: false,
  setStyleOpen: (styleOpen) => set({ styleOpen }),
  settings: null,
  translations: [],
  profile: null,

  init: async () => {
    const [settings, translations, profile, live] = await Promise.all([
      api.getSettings(),
      api.listTranslations(),
      api.getHardwareProfile(),
      api.getOutputState(),
    ]);
    set({ settings, translations, profile, live });

    const unlisteners = await Promise.all([
      events.transcript((t) => {
        if (!t.isFinal) {
          set({ partial: t.text ? t : null });
          return;
        }
        const line = { utteranceId: t.utteranceId, text: t.text, confidence: t.confidence, inferenceMs: t.inferenceMs, refined: t.refined, at: Date.now() };
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
          return { queue: next, preview, lastLatencyMs: d.latencyMs };
        });
      }),
      events.status((status) => set({ status })),
      events.pipelineError((error) => set({ error, running: false })),
      events.pipelineWarning((warning) => set({ error: warning || null })),
      // Keep the theme object identical while its content is: output events arrive with every
      // follow-along highlight, and a "new" theme would make every preview refit its text.
      events.outputState((live) =>
        set((s) => ({ live: s.live && sameJson(s.live.theme, live.theme) ? { ...live, theme: s.live.theme } : live })),
      ),
      events.hardwareProgress((benchProgress) => set({ benchProgress })),
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
      set({ error: String(e) });
    } finally {
      set({ starting: false, benchProgress: null });
    }
  },

  stop: async () => {
    await api.stopPipeline();
    set({ running: false, status: null, partial: null });
  },

  saveSettings: async (patch) => {
    const current = get().settings;
    if (!current) return;
    const settings = await api.updateSettings({ ...current, ...patch });
    set({ settings });
  },

  setPreview: (slide, pin = true) => set({ preview: slide, followLatest: !pin }),

  goLive: async (slide) => {
    const target = slide ?? get().preview;
    if (target) await api.present(target);
  },

  dismiss: (id) => set((s) => ({ queue: s.queue.filter((d) => d.id !== id) })),

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
