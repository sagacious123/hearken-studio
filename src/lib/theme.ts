// Output themes are plain JSON so churches can share them as files. Anything unknown or
// mistyped falls back to the default, so a bad theme can never blank the projector.

export interface Theme {
  version: 1;
  name: string;
  background: {
    color: string;
    /** Any CSS background-image value, e.g. a gradient or url(data:…). */
    image: string;
    /** Darkening layer over the image for legibility, e.g. "rgba(0,0,0,0.45)". */
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
  };
  transition: {
    type: "fade" | "none";
    durationMs: number;
  };
}

export const DEFAULT_THEME: Theme = {
  version: 1,
  name: "Classic Dark",
  background: { color: "#05070d", image: "", overlay: "" },
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
  layout: { paddingPct: 6, verticalAlign: "center" },
  transition: { type: "fade", durationMs: 350 },
};

const ENUMS: Record<string, readonly string[]> = {
  "text.align": ["left", "center", "right"],
  "reference.position": ["top", "bottom"],
  "layout.verticalAlign": ["top", "center", "bottom"],
  "transition.type": ["fade", "none"],
};

const RANGES: Record<string, [number, number]> = {
  "text.weight": [100, 900],
  "text.lineHeight": [0.9, 2.5],
  "text.minPx": [12, 200],
  "text.maxPx": [16, 320],
  "reference.sizePx": [12, 160],
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
