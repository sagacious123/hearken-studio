import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { forgetUpload, SAMPLES } from "../../lib/backgrounds";
import { useT } from "../../lib/i18n";
import { api, type Slide, type UploadedBackground } from "../../lib/ipc";
import { DEFAULT_THEME, FONTS, parseTheme, parseThemeJson, PRESETS, SHADOWS, type Theme } from "../../lib/theme";
import { useDashboard } from "../store";
import { useDismiss } from "../useDismiss";
import { LivestreamBand } from "../../output/ScreenLooks";
import { Stage } from "../../output/Stage";

const WEIGHTS: [number, string][] = [
  [300, "Light"],
  [400, "Regular"],
  [500, "Medium"],
  [600, "Semibold"],
  [700, "Bold"],
  [800, "Extra bold"],
];
const DEFAULT_READ = DEFAULT_THEME.text.readColor;

/**
 * Visual theme editor in a drawer that slides in from the left, over the detections (and as
 * wide as their column), so Preview and Live stay in view while styling. Edits go to a draft shown in
 * Preview; nothing reaches the projector until "Apply to screen".
 */
export function StyleDrawer() {
  const open = useDashboard((s) => s.styleOpen);
  const setOpen = useDashboard((s) => s.setStyleOpen);
  const liveTheme = useDashboard((s) => s.live?.theme);
  const draft = useDashboard((s) => s.draftTheme);
  const setDraft = useDashboard((s) => s.setDraftTheme);
  const theme = draft ?? parseTheme(liveTheme);
  const drawerRef = useDismiss<HTMLElement>(open, () => setOpen(false), ["[data-preview-column]", "[data-style-toggle]"]);
  const t = useT();

  /** Copy, change, validate; the draft is always a well-formed theme. */
  const edit = (change: (t: Theme) => void) => {
    const next = structuredClone(theme);
    change(next);
    setDraft(parseTheme(next));
  };

  return (
    <aside
      ref={drawerRef}
      aria-label={t("Style")}
      inert={!open}
      className={`absolute inset-y-0 left-0 z-30 flex w-[clamp(300px,calc(24vw+12px),392px)] flex-col bg-zinc-950 shadow-2xl ring-1 ring-zinc-800 transition-transform duration-200 ease-out motion-reduce:transition-none ${
        open ? "translate-x-0" : "-translate-x-full"
      }`}
    >
      <header className="flex h-10 flex-none items-center justify-between border-b border-zinc-800 px-4 text-xs font-semibold uppercase tracking-wider text-zinc-400">
        {t("Style: fonts & background")}
        <button onClick={() => setOpen(false)} className="rounded px-1.5 text-base leading-none text-zinc-400 hover:bg-zinc-800 hover:text-zinc-100" aria-label={t("Close style")}>
          ×
        </button>
      </header>
      <div className="min-h-0 flex-1 space-y-4 overflow-y-auto p-3 text-sm">
        <Presets current={theme} onPick={(t) => setDraft(parseTheme(structuredClone(t)))} />
        <BackgroundSection theme={theme} edit={edit} />
        <TextSection theme={theme} edit={edit} />
        <ReferenceSection theme={theme} edit={edit} />
        <Section title="Layout">
          <Slider label="Margins" value={theme.layout.paddingPct} min={0} max={15} step={0.5} format={(v) => `${v}%`} onChange={(v) => edit((t) => void (t.layout.paddingPct = v))} />
          <Row label="Two translations">
            <Segmented
              value={theme.layout.sideBySide}
              options={[
                ["columns", "Side by side"],
                ["rows", "Stacked"],
              ]}
              onChange={(v) => edit((t) => void (t.layout.sideBySide = v as Theme["layout"]["sideBySide"]))}
            />
          </Row>
          <Row label="Transition">
            <Segmented
              value={theme.transition.type}
              options={[
                ["fade", "Fade"],
                ["none", "Cut"],
              ]}
              onChange={(v) => edit((t) => void (t.transition.type = v as Theme["transition"]["type"]))}
            />
          </Row>
          {theme.transition.type === "fade" && (
            <Slider label="Fade length" value={theme.transition.durationMs} min={100} max={1500} step={50} format={(v) => `${v} ms`} onChange={(v) => edit((t) => void (t.transition.durationMs = v))} />
          )}
        </Section>
        <LivestreamSection theme={theme} edit={edit} />
        <AdvancedJson theme={theme} onUse={(t) => setDraft(t)} />
      </div>
      <DraftBar />
    </aside>
  );
}

/** Shown above the Preview while there are unapplied style changes. */
export function DraftBar() {
  const draft = useDashboard((s) => s.draftTheme);
  const setDraft = useDashboard((s) => s.setDraftTheme);
  const [error, setError] = useState<string | null>(null);
  const t = useT();
  if (!draft) return null;
  return (
    <div className="flex flex-none items-center gap-2 bg-amber-950/60 px-3 py-1.5 text-xs text-amber-100">
      <span className="flex-1">{t("Style changes are shown in Preview only.")}{error && <span className="ml-1 text-red-300">{error}</span>}</span>
      <button
        onClick={() =>
          void api
            .setTheme(draft)
            .then(() => {
              setDraft(null);
              setError(null);
            })
            .catch((e) => setError(String(e)))
        }
        className="rounded bg-amber-600 px-2 py-0.5 font-semibold text-white hover:bg-amber-500"
      >
        {t("Apply to screen")}
      </button>
      <button onClick={() => setDraft(null)} className="rounded px-2 py-0.5 hover:bg-amber-900">
        {t("Discard")}
      </button>
    </div>
  );
}

type Edit = (change: (t: Theme) => void) => void;

function Presets({ current, onPick }: { current: Theme; onPick: (t: Theme) => void }) {
  const t = useT();
  return (
    <Section title="Start from">
      <div className="flex flex-wrap gap-1.5">
        {PRESETS.map((p) => (
          <button
            key={p.name}
            onClick={() => onPick(p)}
            className={`rounded-full px-2.5 py-1 text-xs ring-1 ${current.name === p.name ? "bg-amber-600 ring-amber-500" : "bg-zinc-900 ring-zinc-700 hover:ring-zinc-500"}`}
          >
            {t(p.name)}
          </button>
        ))}
      </div>
    </Section>
  );
}

function BackgroundSection({ theme, edit }: { theme: Theme; edit: Edit }) {
  const [uploads, setUploads] = useState<UploadedBackground[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const fileRef = useRef<HTMLInputElement>(null);
  const t = useT();
  const bg = theme.background;
  const pick = (picture: string) => edit((t) => void (t.background.picture = picture));

  useEffect(() => {
    api.listBackgrounds().then(setUploads).catch((e) => setError(String(e)));
  }, []);

  const upload = async (files: FileList) => {
    setBusy(true);
    setError(null);
    let list = uploads;
    const failed: string[] = [];
    for (const f of Array.from(files)) {
      try {
        list = await api.saveBackground(f);
      } catch (e) {
        failed.push(`${f.name}: ${String(e)}`);
      }
    }
    setUploads(list);
    setBusy(false);
    if (failed.length) setError(failed.join("; "));
    else if (list[0]) pick(`upload:${list[0].file}`); // newest first: show what was just added
  };

  const remove = async (u: UploadedBackground) => {
    if (!window.confirm(t("Delete “{name}” from your backgrounds?", { name: u.name }))) return;
    try {
      setUploads(await api.deleteBackground(u.file));
      forgetUpload(u.file);
      if (bg.picture === `upload:${u.file}`) pick("");
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <Section title="Background">
      <div className="grid grid-cols-4 gap-1.5">
        <Tile selected={!bg.picture} onClick={() => pick("")} label={t("Colour only")}>
          <div className="h-full w-full" style={{ background: bg.color }} />
        </Tile>
        {SAMPLES.map((s) => (
          <Tile key={s.id} selected={bg.picture === `sample:${s.id}`} onClick={() => pick(`sample:${s.id}`)} label={t(s.name)}>
            <img src={s.url} alt="" className="h-full w-full object-cover" />
          </Tile>
        ))}
        {uploads.map((u) => (
          <Tile key={u.file} selected={bg.picture === `upload:${u.file}`} onClick={() => pick(`upload:${u.file}`)} label={u.name} onDelete={() => void remove(u)}>
            <img src={convertFileSrc(u.path)} alt="" className="h-full w-full object-cover" />
          </Tile>
        ))}
        <button
          onClick={() => fileRef.current?.click()}
          disabled={busy}
          className="flex aspect-video flex-col items-center justify-center rounded border border-dashed border-zinc-600 text-xs text-zinc-400 hover:border-amber-500 hover:text-amber-300 disabled:opacity-50"
        >
          <span className="text-lg leading-none">+</span>
          {busy ? t("Adding…") : t("Upload")}
        </button>
        <input
          ref={fileRef}
          type="file"
          multiple
          accept="image/jpeg,image/png,image/webp,image/gif"
          className="hidden"
          onChange={(e) => {
            if (e.target.files?.length) void upload(e.target.files);
            e.target.value = "";
          }}
        />
      </div>
      {error && <p className="text-xs text-red-400">{error}</p>}
      <Row label="Colour">
        <Color value={bg.color} onChange={(v) => edit((t) => void (t.background.color = v))} />
        <span className="text-xs text-zinc-500">{bg.picture ? t("shows at the edges while the picture loads") : ""}</span>
      </Row>
      {bg.picture && (
        <>
          <Slider label="Darken" value={bg.dim} min={0} max={0.9} step={0.05} format={(v) => `${Math.round(v * 100)}%`} onChange={(v) => edit((t) => void (t.background.dim = v))} />
          <Slider label="Soften" value={bg.blurPx} min={0} max={20} step={1} format={(v) => (v ? `${v}px` : t("off"))} onChange={(v) => edit((t) => void (t.background.blurPx = v))} />
        </>
      )}
    </Section>
  );
}

function TextSection({ theme, edit }: { theme: Theme; edit: Edit }) {
  const text = theme.text;
  const t = useT();
  return (
    <Section title="Verse text">
      <Row label="Font">
        <FontSelect value={text.fontFamily} onChange={(v) => edit((t) => void (t.text.fontFamily = v))} />
        <select value={text.weight} onChange={(e) => edit((t) => void (t.text.weight = Number(e.target.value)))} className="rounded bg-zinc-800 px-2 py-1">
          {WEIGHTS.map(([w, l]) => (
            <option key={w} value={w}>
              {t(l)}
            </option>
          ))}
        </select>
      </Row>
      <Row label="Colour">
        <Color value={text.color} onChange={(v) => edit((t) => void (t.text.color = v))} />
      </Row>
      <Slider
        label="Size (largest)"
        value={text.maxPx}
        min={40}
        max={200}
        step={2}
        format={(v) => `${v}px`}
        onChange={(v) =>
          edit((t) => {
            t.text.maxPx = v;
            t.text.minPx = Math.min(t.text.minPx, v);
          })
        }
      />
      <Slider label="Line spacing" value={text.lineHeight} min={1} max={2} step={0.05} format={(v) => v.toFixed(2)} onChange={(v) => edit((t) => void (t.text.lineHeight = v))} />
      <Row label="Align">
        <Segmented
          value={text.align}
          options={[
            ["left", "Left"],
            ["center", "Centre"],
            ["right", "Right"],
          ]}
          onChange={(v) => edit((t) => void (t.text.align = v as Theme["text"]["align"]))}
        />
        <Segmented
          value={theme.layout.verticalAlign}
          options={[
            ["top", "Top"],
            ["center", "Middle"],
            ["bottom", "Bottom"],
          ]}
          onChange={(v) => edit((t) => void (t.layout.verticalAlign = v as Theme["layout"]["verticalAlign"]))}
        />
      </Row>
      <Row label="Shadow">
        <select value={SHADOWS.some((sh) => sh.css === text.shadow) ? text.shadow : "custom"} onChange={(e) => edit((t) => void (t.text.shadow = e.target.value))} className="min-w-0 flex-1 rounded bg-zinc-800 px-2 py-1">
          {!SHADOWS.some((sh) => sh.css === text.shadow) && <option value="custom">{t("Custom")}</option>}
          {SHADOWS.map((sh) => (
            <option key={sh.label} value={sh.css}>
              {t(sh.label)}
            </option>
          ))}
        </select>
      </Row>
      <Row label="Outline">
        <Color value={text.outline.color} onChange={(v) => edit((t) => void (t.text.outline.color = v))} />
        <input
          type="range"
          min={0}
          max={12}
          step={0.5}
          value={text.outline.width}
          onChange={(e) => edit((t) => void (t.text.outline.width = Number(e.target.value)))}
          className="min-w-0 flex-1 accent-amber-500"
          aria-label={t("Outline width")}
        />
        <span className="w-10 text-right font-mono text-xs text-zinc-400">{text.outline.width ? `${text.outline.width}px` : t("off")}</span>
      </Row>
      <div className="flex flex-wrap gap-x-4 gap-y-1.5">
        <Check label="Verse numbers" checked={text.verseNumbers} onChange={(on) => edit((t) => void (t.text.verseNumbers = on))} />
        <Check label="Highlight words read" checked={!!text.readColor} onChange={(on) => edit((t) => void (t.text.readColor = on ? DEFAULT_READ : ""))} />
        {text.readColor && <Color value={text.readColor} onChange={(v) => edit((t) => void (t.text.readColor = v))} />}
      </div>
    </Section>
  );
}

function ReferenceSection({ theme, edit }: { theme: Theme; edit: Edit }) {
  const ref = theme.reference;
  return (
    <Section title="Reference (e.g. JOHN 3:16)">
      <div className="flex flex-wrap items-center gap-x-4 gap-y-1.5">
        <Check label="Show" checked={ref.show} onChange={(on) => edit((t) => void (t.reference.show = on))} />
        {ref.show && (
          <Segmented
            value={ref.position}
            options={[
              ["top", "Above"],
              ["bottom", "Below"],
            ]}
            onChange={(v) => edit((t) => void (t.reference.position = v as Theme["reference"]["position"]))}
          />
        )}
      </div>
      {ref.show && (
        <>
          <Row label="Font">
            <FontSelect value={ref.fontFamily} onChange={(v) => edit((t) => void (t.reference.fontFamily = v))} />
            <Color value={ref.color} onChange={(v) => edit((t) => void (t.reference.color = v))} />
          </Row>
          <Slider label="Size" value={ref.sizePx} min={20} max={120} step={2} format={(v) => `${v}px`} onChange={(v) => edit((t) => void (t.reference.sizePx = v))} />
          <div className="flex flex-wrap gap-x-4">
            <Check label="Capitals" checked={ref.uppercase} onChange={(on) => edit((t) => void (t.reference.uppercase = on))} />
            <Check label="Show translation" checked={ref.showTranslation} onChange={(on) => edit((t) => void (t.reference.showTranslation = on))} />
          </div>
        </>
      )}
    </Section>
  );
}

/** A verse to show in the livestream preview when nothing is in Preview or live. */
const SAMPLE: Slide = {
  id: "sample",
  label: "John 3:16",
  translation: "KJV",
  reference: { bookId: 43, chapter: 3, verseStart: 16, verseEnd: 16 },
  verses: [{ verse: 16, text: "For God so loved the world, that he gave his only begotten Son, that whosoever believeth in him should not perish, but have everlasting life." }],
};

/**
 * The band on the livestream: the OBS overlay and the "Lower third" screen follow these, once
 * applied to screen. The preview shows it over a stand-in for the camera picture.
 */
function LivestreamSection({ theme, edit }: { theme: Theme; edit: Edit }) {
  const t = useT();
  const ls = theme.livestream;
  const slide = useDashboard((s) => s.preview ?? s.live?.slide ?? null);
  const set = (fn: (l: Theme["livestream"]) => void) => edit((th) => fn(th.livestream));
  return (
    <Section title="Livestream">
      <p className="text-xs leading-relaxed text-zinc-500">{t("The band on your livestream: the OBS or vMix overlay (Share → Livestream overlay) and the Lower third screen. Changes reach them when you apply them to the screen.")}</p>
      <div className="overflow-hidden rounded-lg ring-1 ring-zinc-800" aria-label={t("Livestream preview")}>
        <div className="aspect-video">
          <Stage background="linear-gradient(160deg,#3b4a5c 0%,#1d2733 45%,#0d1117 100%)">
            <div aria-hidden style={{ position: "absolute", inset: 0, background: "radial-gradient(40% 55% at 50% 42%, rgba(255,226,190,.28), transparent 70%)" }} />
            <LivestreamBand slide={slide && !slide.song ? slide : (slide ?? SAMPLE)} theme={theme} />
          </Stage>
        </div>
      </div>
      <Row label="Position">
        <Segmented value={ls.position} options={[["bottom", "Bottom"], ["top", "Top"]]} onChange={(v) => set((l) => void (l.position = v as Theme["livestream"]["position"]))} />
        <Segmented value={ls.align} options={[["left", "Left"], ["center", "Centre"]]} onChange={(v) => set((l) => void (l.align = v as Theme["livestream"]["align"]))} />
      </Row>
      <Slider label="Size" value={ls.size} min={0.6} max={1.6} step={0.05} format={(v) => `${Math.round(v * 100)}%`} onChange={(v) => set((l) => void (l.size = v))} />
      <Slider label="Space at the sides" value={ls.insetPct} min={0} max={25} step={0.5} format={(v) => `${v}%`} onChange={(v) => set((l) => void (l.insetPct = v))} />
      <Slider label="Space from the edge" value={ls.offsetPct} min={0} max={20} step={0.5} format={(v) => `${v}%`} onChange={(v) => set((l) => void (l.offsetPct = v))} />
      <Row label="Band">
        <Color value={ls.background} onChange={(v) => set((l) => void (l.background = v))} />
        <input
          type="range"
          min={0}
          max={1}
          step={0.02}
          value={ls.opacity}
          onChange={(e) => set((l) => void (l.opacity = Number(e.target.value)))}
          className="min-w-0 flex-1 accent-amber-500"
          aria-label={t("Band opacity")}
        />
        <span className="w-10 text-right font-mono text-xs text-zinc-400">{Math.round(ls.opacity * 100)}%</span>
      </Row>
      <Slider label="Corners" value={ls.radius} min={0} max={48} step={1} format={(v) => `${v}px`} onChange={(v) => set((l) => void (l.radius = v))} />
      <Row label="Accent">
        <Color value={ls.accent} onChange={(v) => set((l) => void (l.accent = v))} />
        <Check label="Stripe at the side" checked={ls.stripe} onChange={(on) => set((l) => void (l.stripe = on))} />
      </Row>
      <Row label="Text">
        <Color value={ls.textColor} onChange={(v) => set((l) => void (l.textColor = v))} />
        <FontSelect value={ls.fontFamily || theme.text.fontFamily} onChange={(v) => set((l) => void (l.fontFamily = v === theme.text.fontFamily ? "" : v))} />
      </Row>
      <Row label="Reference">
        <FontSelect value={ls.referenceFont || theme.reference.fontFamily} onChange={(v) => set((l) => void (l.referenceFont = v === theme.reference.fontFamily ? "" : v))} />
      </Row>
      <div className="flex flex-wrap gap-x-4 gap-y-1.5">
        <Check label="Show the reference" checked={ls.showReference} onChange={(on) => set((l) => void (l.showReference = on))} />
        <Check label="Show the translation" checked={ls.showTranslation} onChange={(on) => set((l) => void (l.showTranslation = on))} />
      </div>
      <button onClick={() => edit((th) => void (th.livestream = structuredClone(DEFAULT_THEME.livestream)))} className="text-xs text-zinc-500 hover:text-zinc-200">
        {t("Reset the livestream look")}
      </button>
    </Section>
  );
}

function AdvancedJson({ theme, onUse }: { theme: Theme; onUse: (t: Theme) => void }) {
  const [open, setOpen] = useState(false);
  const [json, setJson] = useState("");
  const [error, setError] = useState<string | null>(null);
  const t = useT();
  useEffect(() => {
    if (open) setJson(JSON.stringify(theme, null, 2));
  }, [open, theme]);
  return (
    <div>
      <button onClick={() => setOpen(!open)} className="text-xs text-zinc-500 hover:text-zinc-300">
        {open ? "▾" : "▸"} {t("Advanced: edit as JSON (share themes as files)")}
      </button>
      {open && (
        <div className="mt-2 space-y-2">
          <textarea value={json} onChange={(e) => setJson(e.target.value)} spellCheck={false} className="h-56 w-full rounded-md bg-zinc-900 p-2 font-mono text-xs ring-1 ring-zinc-800" />
          {error && <p className="text-xs text-red-400">{error}</p>}
          <button
            onClick={() => {
              const r = parseThemeJson(json);
              if ("error" in r) return setError(r.error);
              setError(null);
              onUse(r.theme);
            }}
            className="rounded bg-zinc-800 px-3 py-1 text-xs hover:bg-zinc-700"
          >
            {t("Use this JSON")}
          </button>
        </div>
      )}
    </div>
  );
}

// ---------- small controls ----------

function Section({ title, children }: { title: string; children: ReactNode }) {
  const t = useT();
  return (
    <section data-tour={`style-${title.split(" ")[0].toLowerCase()}`} className="space-y-2">
      <h4 className="text-xs font-semibold text-zinc-400">{t(title)}</h4>
      {children}
    </section>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  const t = useT();
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="w-24 flex-none text-xs text-zinc-500">{t(label)}</span>
      {children}
    </div>
  );
}

function Slider({ label, value, min, max, step, format, onChange }: { label: string; value: number; min: number; max: number; step: number; format: (v: number) => string; onChange: (v: number) => void }) {
  return (
    <Row label={label}>
      <input type="range" min={min} max={max} step={step} value={value} onChange={(e) => onChange(Number(e.target.value))} className="min-w-0 flex-1 accent-amber-500" />
      <span className="w-14 text-right font-mono text-xs text-zinc-400">{format(value)}</span>
    </Row>
  );
}

function Color({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  // <input type=color> only takes #rrggbb; anything else (from JSON) shows as white until changed.
  const hex = /^#[0-9a-f]{6}$/i.test(value) ? value : "#ffffff";
  return <input type="color" value={hex} onChange={(e) => onChange(e.target.value)} className="h-7 w-10 cursor-pointer rounded border border-zinc-700 bg-transparent" />;
}

function Check({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  const t = useT();
  return (
    <label className="flex items-center gap-1.5 text-xs text-zinc-300">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} className="accent-amber-500" />
      {t(label)}
    </label>
  );
}

function Segmented({ value, options, onChange }: { value: string; options: [string, string][]; onChange: (v: string) => void }) {
  const t = useT();
  return (
    <div className="flex overflow-hidden rounded ring-1 ring-zinc-700">
      {options.map(([v, l]) => (
        <button key={v} onClick={() => onChange(v)} className={`px-2 py-0.5 text-xs ${v === value ? "bg-amber-600 text-white" : "bg-zinc-900 text-zinc-300 hover:bg-zinc-800"}`}>
          {t(l)}
        </button>
      ))}
    </div>
  );
}

/** Installed fonts, fetched once per session (the scan takes a moment). */
let systemFonts: Promise<string[]> | null = null;
const fontStack = (family: string) => `"${family.replace(/"/g, "")}", system-ui, sans-serif`;

function useSystemFonts() {
  const [fonts, setFonts] = useState<string[]>([]);
  useEffect(() => {
    // Anything but a list (an older backend, a failed scan) means no extra fonts, never a crash.
    systemFonts ??= api
      .listSystemFonts()
      .then((f) => (Array.isArray(f) ? f : []))
      .catch(() => []);
    void systemFonts.then(setFonts);
  }, []);
  return fonts;
}

function FontSelect({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const installed = useSystemFonts();
  const t = useT();
  const known = FONTS.some((f) => f.stack === value) || installed.some((f) => fontStack(f) === value);
  return (
    <select value={value} onChange={(e) => onChange(e.target.value)} className="min-w-0 flex-1 rounded bg-zinc-800 px-2 py-1" style={{ fontFamily: value }}>
      {!known && <option value={value}>{t("Custom font")}</option>}
      <optgroup label={t("Included with Hearken Studio")}>
        {FONTS.filter((f) => f.bundled).map((f) => (
          <option key={f.label} value={f.stack}>
            {t(f.label)}
          </option>
        ))}
      </optgroup>
      <optgroup label={t("Common")}>
        {FONTS.filter((f) => !f.bundled).map((f) => (
          <option key={f.label} value={f.stack}>
            {t(f.label)}
          </option>
        ))}
      </optgroup>
      {installed.length > 0 && (
        <optgroup label={t("Installed on this computer ({n})", { n: installed.length })}>
          {installed.map((f) => (
            <option key={f} value={fontStack(f)}>
              {f}
            </option>
          ))}
        </optgroup>
      )}
    </select>
  );
}

function Tile({ selected, onClick, label, onDelete, children }: { selected: boolean; onClick: () => void; label: string; onDelete?: () => void; children: ReactNode }) {
  const t = useT();
  return (
    <div className="group relative">
      <button
        onClick={onClick}
        title={label}
        className={`block aspect-video w-full overflow-hidden rounded ring-2 ${selected ? "ring-amber-500" : "ring-transparent hover:ring-zinc-500"}`}
      >
        {children}
      </button>
      <span className="pointer-events-none absolute inset-x-0 bottom-0 truncate rounded-b bg-black/55 px-1 text-[10px] text-zinc-200">{label}</span>
      {onDelete && (
        <button
          onClick={onDelete}
          aria-label={t("Delete {name}", { name: label })}
          className="absolute right-0.5 top-0.5 hidden h-5 w-5 rounded-full bg-black/70 text-xs leading-5 text-zinc-200 hover:bg-red-600 group-hover:block"
        >
          ×
        </button>
      )}
    </div>
  );
}
