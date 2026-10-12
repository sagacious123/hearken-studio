import { useEffect, useMemo, useState } from "react";
import { api, type SearchResponse, type Slide } from "../../lib/ipc";
import { parseTheme } from "../../lib/theme";
import { SlideView } from "../../output/SlideView";
import { Stage } from "../../output/Stage";
import { useDashboard } from "../store";
import { PassagePicker } from "./PassagePicker";
import { DraftBar } from "./StylePanel";
import { PanelHeader } from "./TranscriptPanel";

export function PreviewPane() {
  const preview = useDashboard((s) => s.preview);
  const live = useDashboard((s) => s.live);
  const goLive = useDashboard((s) => s.goLive);
  const draftTheme = useDashboard((s) => s.draftTheme);
  const theme = useMemo(() => parseTheme(live?.theme), [live?.theme]);
  // Preview shows unapplied style edits; the Live mirror always shows what's on screen.
  const previewTheme = draftTheme ?? theme;

  return (
    <section className="flex min-h-0 flex-col overflow-y-auto bg-zinc-950">
      <SearchBox />
      <PassagePicker />

      <PanelHeader title="Preview">
        <button
          disabled={!preview}
          onClick={() => goLive()}
          className="rounded bg-red-600 px-3 py-1 font-semibold normal-case tracking-normal text-white hover:bg-red-500 disabled:opacity-40"
        >
          Go live ⏎
        </button>
      </PanelHeader>
      <DraftBar />
      <div className="aspect-video flex-none p-3">
        <div className={`h-full overflow-hidden rounded-md ring-1 ${draftTheme ? "ring-amber-500" : "ring-zinc-800"}`}>
          <Stage background={previewTheme.background.color}>
            <SlideView slide={preview ?? (draftTheme ? SAMPLE_SLIDE : null)} theme={previewTheme} blackout={false} />
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
    <div className="relative flex-none p-3 pb-2">
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

/** Something to style against when nothing is in Preview yet. */
const SAMPLE_SLIDE: Slide = {
  id: "style-sample",
  label: "Psalm 23:1",
  translation: "KJV",
  reference: { bookId: 19, chapter: 23, verseStart: 1, verseEnd: null },
  verses: [{ verse: 1, text: "The LORD is my shepherd; I shall not want." }],
};
