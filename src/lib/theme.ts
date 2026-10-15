// Output themes are plain JSON so churches can share them as files. Anything unknown or
// mistyped falls back to the default, so a bad theme can never blank the projector.

export interface Theme {
  version: 1;
  name: string;
  background: {
    color: string;
    /** "" | "sample:<id>" (bundled) | "upload:<file>" (the church's own image). */
    picture: string;
    /** Darken the picture for legibility: 0 (none) to 0.9. */
    dim: number;
    /** Soften a busy photo, in stage px. */
    blurPx: number;
    /** Advanced: any CSS background-image value, e.g. a gradient. Used when there's no picture. */
    image: string;
    /** Advanced: extra CSS layer over the background, e.g. "rgba(0,0,0,0.45)". */
    overlay: string;
  };
  text: {
    fontFamily: string;
    color: string;
    weight: number;
    lineHeight: number;
    align: "left" | "center" | "right";
    /** Auto-fit search range, in px of the 1920×1080 stage. */
    minPx: number;
    maxPx: number;
    shadow: string;
    verseNumbers: boolean;
    /** Follow-along: colour of the words already read or sung ("" turns highlighting off). */
    readColor: string;
    /** Outline around the letters, for text over busy pictures or video (width 0 = none). */
    outline: { width: number; color: string };
  };
  reference: {
    show: boolean;
    position: "top" | "bottom";
    fontFamily: string;
    color: string;
    sizePx: number;
    uppercase: boolean;
    showTranslation: boolean;
  };
  layout: {
    /** Safe-area padding as % of stage width (projectors overscan). */
    paddingPct: number;
    verticalAlign: "top" | "center" | "bottom";
    /** Two translations: next to each other ("columns") or one above the other ("rows"). */
    sideBySide: "columns" | "rows";
  };
  transition: {
    type: "fade" | "none";
    durationMs: number;
  };
}

export const DEFAULT_THEME: Theme = {
  version: 1,
  name: "Classic Dark",
  background: { color: "#05070d", picture: "", dim: 0, blurPx: 0, image: "", overlay: "" },
  text: {
    fontFamily: 'Georgia, "Iowan Old Style", "Times New Roman", serif',
    color: "#f8fafc",
    weight: 500,
    lineHeight: 1.3,
    align: "center",
    minPx: 36,
    maxPx: 96,
    shadow: "0 2px 12px rgba(0,0,0,0.6)",
    verseNumbers: true,
    readColor: "#fde68a",
    outline: { width: 0, color: "#000000" },
  },
  reference: {
    show: true,
    position: "bottom",
    fontFamily: 'system-ui, -apple-system, "Segoe UI", sans-serif',
    color: "#fbbf24",
    sizePx: 44,
    uppercase: true,
    showTranslation: true,
  },
  layout: { paddingPct: 6, verticalAlign: "center", sideBySide: "columns" },
  transition: { type: "fade", durationMs: 350 },
};

const ENUMS: Record<string, readonly string[]> = {
  "text.align": ["left", "center", "right"],
  "reference.position": ["top", "bottom"],
  "layout.verticalAlign": ["top", "center", "bottom"],
  "layout.sideBySide": ["columns", "rows"],
  "transition.type": ["fade", "none"],
};

const RANGES: Record<string, [number, number]> = {
  "background.dim": [0, 0.9],
  "background.blurPx": [0, 30],
  "text.weight": [100, 900],
  "text.lineHeight": [0.9, 2.5],
  "text.minPx": [12, 200],
  "text.maxPx": [16, 320],
  "reference.sizePx": [12, 160],
  "text.outline.width": [0, 12],
  "layout.paddingPct": [0, 25],
  "transition.durationMs": [0, 3000],
};

function merge(def: unknown, input: unknown, path: string): unknown {
  if (def && typeof def === "object" && !Array.isArray(def)) {
    const src = input && typeof input === "object" ? (input as Record<string, unknown>) : {};
    return Object.fromEntries(
      Object.entries(def as Record<string, unknown>).map(([k, v]) => [k, merge(v, src[k], path ? `${path}.${k}` : k)]),
    );
  }
  if (typeof input !== typeof def) return def;
  if (typeof input === "string" && ENUMS[path] && !ENUMS[path].includes(input)) return def;
  if (typeof input === "number") {
    if (!Number.isFinite(input)) return def;
    const r = RANGES[path];
    return r ? Math.min(r[1], Math.max(r[0], input)) : input;
  }
  return input;
}

export function parseTheme(input: unknown): Theme {
  const theme = merge(DEFAULT_THEME, input, "") as Theme;
  if (theme.text.minPx > theme.text.maxPx) theme.text.minPx = theme.text.maxPx;
  return theme;
}

/** Validates a JSON string from the theme editor. */
export function parseThemeJson(json: string): { theme: Theme } | { error: string } {
  try {
    return { theme: parseTheme(JSON.parse(json)) };
  } catch (e) {
    return { error: e instanceof Error ? e.message : String(e) };
  }
}

export interface FontChoice {
  label: string;
  stack: string;
  /** Shipped with the app (same look on every computer, offline). */
  bundled: boolean;
}

/** Text shadow choices: layered shadows for a stronger lift than one blur can give. */
export const SHADOWS: { label: string; css: string }[] = [
  { label: "None", css: "" },
  { label: "Soft", css: DEFAULT_THEME.text.shadow },
  { label: "Strong", css: "0 2px 4px rgba(0,0,0,0.9), 0 4px 18px rgba(0,0,0,0.75), 0 0 42px rgba(0,0,0,0.5)" },
  { label: "Glow", css: "0 0 10px rgba(255,255,255,0.55), 0 0 28px rgba(255,255,255,0.3)" },
  { label: "Block", css: "4px 4px 0 rgba(0,0,0,0.85), 8px 8px 0 rgba(0,0,0,0.35)" },
];

export const FONTS: FontChoice[] = [
  { label: "Georgia", stack: DEFAULT_THEME.text.fontFamily, bundled: false },
  { label: "Lora", stack: '"Lora", Georgia, serif', bundled: true },
  { label: "Merriweather", stack: '"Merriweather", Georgia, serif', bundled: true },
  { label: "Playfair Display", stack: '"Playfair Display", Georgia, serif', bundled: true },
  { label: "EB Garamond", stack: '"EB Garamond", Garamond, Georgia, serif', bundled: true },
  { label: "Inter", stack: '"Inter", system-ui, sans-serif', bundled: true },
  { label: "Montserrat", stack: '"Montserrat", system-ui, sans-serif', bundled: true },
  { label: "Nunito", stack: '"Nunito", system-ui, sans-serif', bundled: true },
  { label: "Oswald", stack: '"Oswald", "Arial Narrow", sans-serif', bundled: true },
  { label: "System sans-serif", stack: DEFAULT_THEME.reference.fontFamily, bundled: false },
  { label: "Helvetica / Arial", stack: '"Helvetica Neue", Helvetica, Arial, sans-serif', bundled: false },
  { label: "Times", stack: '"Times New Roman", Times, serif', bundled: false },
];

type DeepPartial<T> = { [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K] };

function preset(name: string, patch: DeepPartial<Theme>): Theme {
  const t = structuredClone(DEFAULT_THEME);
  for (const [k, v] of Object.entries(patch)) Object.assign((t as unknown as Record<string, object>)[k], v);
  return { ...t, name };
}

const font = (label: string) => FONTS.find((f) => f.label === label)!.stack;

/** One-click starting points; everything can be adjusted afterwards. */
export const PRESETS: Theme[] = [
  DEFAULT_THEME,
  preset("Sunrise", {
    background: { picture: "sample:dawn", dim: 0.35 },
    text: { fontFamily: font("Lora") },
    reference: { color: "#fde68a" },
  }),
  preset("Night sky", {
    background: { picture: "sample:night-sky", dim: 0.15 },
    text: { fontFamily: font("Inter"), weight: 400 },
    reference: { fontFamily: font("Inter"), color: "#93c5fd" },
  }),
  preset("Clean light", {
    background: { color: "#f5efe3", picture: "sample:linen" },
    text: { fontFamily: font("Merriweather"), color: "#1c1917", weight: 400, shadow: "", readColor: "#b45309" },
    reference: { color: "#b45309" },
  }),
  preset("Bold", {
    background: { color: "#0b0b0f" },
    text: { fontFamily: font("Montserrat"), weight: 700, lineHeight: 1.2 },
    reference: { fontFamily: font("Oswald"), color: "#22d3ee", sizePx: 52 },
  }),
  preset("Calvary", {
    background: { picture: "sample:calvary", dim: 0.3 },
    text: { fontFamily: font("Playfair Display"), align: "left" },
    reference: { color: "#fbbf24" },
    layout: { verticalAlign: "top" },
  }),
];
