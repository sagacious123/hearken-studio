import { useEffect, useMemo, useState } from "react";
import { api, type SearchResponse, type Slide } from "../../lib/ipc";
import { DEFAULT_THEME, parseTheme, parseThemeJson } from "../../lib/theme";
import { SlideView } from "../../output/SlideView";
import { Stage } from "../../output/Stage";
import { useDashboard } from "../store";
import { PanelHeader } from "./TranscriptPanel";

export function PreviewPane() {
  const { preview, live, goLive } = useDashboard();
  const theme = useMemo(() => parseTheme(live?.theme), [live?.theme]);

  return (
    <section className="flex min-h-0 flex-col overflow-y-auto bg-zinc-950">
      <SearchBox />

      <PanelHeader title="Preview">
        <button
          disabled={!preview}
          onClick={() => goLive()}
          className="rounded bg-red-600 px-3 py-1 font-semibold normal-case tracking-normal text-white hover:bg-red-500 disabled:opacity-40"
        >
          Go live ⏎
        </button>
      </PanelHeader>
      <div className="aspect-video flex-none p-3">
        <div className="h-full overflow-hidden rounded-md ring-1 ring-zinc-800">
          <Stage background={theme.background.color}>
            <SlideView slide={preview} theme={theme} blackout={false} />
          </Stage>
        </div>
      </div>

      <PanelHeader title={live?.blackout ? "Live — BLACKOUT" : "Live"}>
        <div className="flex gap-1 normal-case tracking-normal">
          <Btn onClick={() => api.stepVerse(-1)}>◀ Verse</Btn>
          <Btn onClick={() => api.stepVerse(1)}>Verse ▶</Btn>
          <Btn onClick={() => api.setBlackout(!live?.blackout)}>{live?.blackout ? "Unblack" : "Black (B)"}</Btn>
          <Btn onClick={() => api.clearOutput()}>Clear (Esc)</Btn>
        </div>
      </PanelHeader>
      <div className="aspect-video flex-none p-3">
        <div className={`h-full overflow-hidden rounded-md ring-2 ${live?.slide ? "ring-red-600" : "ring-zinc-800"}`}>
          <Stage background={theme.background.color}>
            <SlideView slide={live?.slide ?? null} theme={theme} blackout={live?.blackout ?? false} reading={live?.reading} qr={live?.qr} />
          </Stage>
        </div>
      </div>

      <ThemeEditor current={live?.theme} />
    </section>
  );
}

function Btn({ onClick, children }: { onClick: () => void; children: React.ReactNode }) {
  return (
    <button onClick={onClick} className="rounded bg-zinc-800 px-2 py-1 text-zinc-200 hover:bg-zinc-700">
      {children}
    </button>
  );
}

function SearchBox() {
  const translation = useDashboard((s) => s.settings?.translation ?? "KJV");
  const setPreview = useDashboard((s) => s.setPreview);
  const [query, setQuery] = useState("");
  const [result, setResult] = useState<SearchResponse | null>(null);

  useEffect(() => {
    if (!query.trim()) {
      setResult(null);
      return;
    }
    const id = setTimeout(() => {
      api.searchScripture(query, translation).then(setResult).catch(console.error);
    }, 80);
    return () => clearTimeout(id);
  }, [query, translation]);

  const items: Slide[] = result ? [...(result.reference ? [result.reference] : []), ...result.hits] : [];

  return (
    <div className="relative flex-none border-b border-zinc-800 p-3">
      <input
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && items[0]) {
            setPreview(items[0]);
            setQuery("");
          }
          if (e.key === "Escape") setQuery("");
        }}
        placeholder="Search: “jn 3 16”, “rom 8:28-30” or words “love is patient”"
        className="w-full rounded-md bg-zinc-900 px-3 py-2 text-sm outline-none ring-1 ring-zinc-800 focus:ring-amber-500"
      />
      {result && (
        <ul className="absolute inset-x-3 top-full z-10 mt-1 max-h-80 overflow-y-auto rounded-md bg-zinc-900 shadow-xl ring-1 ring-zinc-700">
          {items.length === 0 && <li className="px-3 py-2 text-sm text-zinc-500">No matches</li>}
          {items.map((s, i) => (
            <li
              key={s.id + i}
              onClick={() => {
                setPreview(s);
                setQuery("");
              }}
              className="cursor-pointer px-3 py-2 text-sm hover:bg-zinc-800"
            >
              <span className="font-semibold text-amber-400">{s.label}</span>{" "}
              <span className="text-zinc-400">{s.verses[0]?.text}</span>
            </li>
          ))}
          <li className="px-3 py-1 text-right font-mono text-[10px] text-zinc-600">{result.elapsedMs.toFixed(2)} ms</li>
        </ul>
      )}
    </div>
  );
}

function ThemeEditor({ current }: { current: unknown }) {
  const [open, setOpen] = useState(false);
  const [json, setJson] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) setJson(JSON.stringify(parseTheme(current ?? DEFAULT_THEME), null, 2));
  }, [open, current]);

  const apply = () => {
    const r = parseThemeJson(json);
    if ("error" in r) return setError(r.error);
    setError(null);
    void api.setTheme(r.theme);
  };

  return (
    <div className="flex-none border-t border-zinc-800">
      <button onClick={() => setOpen(!open)} className="w-full px-4 py-2 text-left text-xs font-semibold uppercase tracking-wider text-zinc-400">
        {open ? "▾" : "▸"} Theme (JSON)
      </button>
      {open && (
        <div className="space-y-2 px-3 pb-3">
          <textarea
            value={json}
            onChange={(e) => setJson(e.target.value)}
            spellCheck={false}
            className="h-64 w-full rounded-md bg-zinc-900 p-2 font-mono text-xs ring-1 ring-zinc-800"
          />
          {error && <p className="text-xs text-red-400">{error}</p>}
          <div className="flex gap-2">
            <button onClick={apply} className="rounded bg-amber-600 px-3 py-1 text-sm font-medium hover:bg-amber-500">
              Apply to output
            </button>
            <button onClick={() => setJson(JSON.stringify(DEFAULT_THEME, null, 2))} className="rounded bg-zinc-800 px-3 py-1 text-sm">
              Reset to default
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
