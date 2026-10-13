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
    // min-w-0 + overflow-x-hidden: this column scrolls vertically only, even where WebKit's
    // always-visible scrollbar would leave a few pixels of sideways overflow.
    <section data-preview-column className="flex min-h-0 min-w-0 flex-col overflow-y-auto overflow-x-hidden bg-zinc-950 [scrollbar-gutter:stable]">
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
      <StageBox ring={`ring-1 ${draftTheme ? "ring-amber-500" : "ring-zinc-800"}`}>
        <Stage background={previewTheme.background.color}>
          <SlideView slide={preview ?? (draftTheme ? SAMPLE_SLIDE : null)} theme={previewTheme} blackout={false} />
        </Stage>
      </StageBox>

      <PanelHeader title={live?.blackout ? "Live — BLACKOUT" : "Live"}>
        <div className="flex gap-1 normal-case tracking-normal">
          <Btn onClick={() => api.stepVerse(-1)}>◀ Verse</Btn>
          <Btn onClick={() => api.stepVerse(1)}>Verse ▶</Btn>
          <Btn onClick={() => api.setBlackout(!live?.blackout)}>{live?.blackout ? "Unblack" : "Black (B)"}</Btn>
          <Btn onClick={() => api.clearOutput()}>Clear (Esc)</Btn>
        </div>
      </PanelHeader>
      <StageBox ring={`ring-2 ${live?.slide ? "ring-red-600" : "ring-zinc-800"}`}>
        <Stage background={theme.background.color}>
          <SlideView slide={live?.slide ?? null} theme={theme} blackout={live?.blackout ?? false} reading={live?.reading} qr={live?.qr} />
        </Stage>
      </StageBox>

    </section>
  );
}

/**
 * A 16:9 preview box as wide as the column. The width comes first (block layout) and the
 * height follows from it: with aspect-ratio on a flex item, WebKit sized these boxes from their
 * height instead and made them 47 px wider than the column.
 */
function StageBox({ ring, children }: { ring: string; children: React.ReactNode }) {
  return (
    <div className="flex-none p-3">
      <div className={`relative aspect-video w-full overflow-hidden rounded-md ${ring}`}>
        <div className="absolute inset-0">{children}</div>
      </div>
    </div>
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
  const byMeaning = result?.meaning ?? [];
  const pick = (s: Slide) => {
    setPreview(s);
    setQuery("");
  };

  return (
    <div className="relative flex-none p-3 pb-2">
      <input
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={(e) => {
          const first = items[0] ?? byMeaning[0];
          if (e.key === "Enter" && first) pick(first);
          if (e.key === "Escape") setQuery("");
        }}
        placeholder="Search: “jn 3 16”, “love is patient” or “don't worry about tomorrow”"
        className="w-full rounded-md bg-zinc-900 px-3 py-2 text-sm outline-none ring-1 ring-zinc-800 focus:ring-amber-500"
      />
      {result && (
        <ul className="absolute inset-x-3 top-full z-10 mt-1 max-h-80 overflow-y-auto rounded-md bg-zinc-900 shadow-xl ring-1 ring-zinc-700">
          {items.length === 0 && byMeaning.length === 0 && <li className="px-3 py-2 text-sm text-zinc-500">No matches</li>}
          {items.map((s, i) => (
            <SearchItem key={s.id + i} slide={s} onPick={pick} />
          ))}
          {byMeaning.length > 0 && (
            <>
              <li className="border-t border-zinc-800 px-3 pb-1 pt-2 text-[11px] font-semibold uppercase tracking-wider text-sky-400" title="Verses that say the same thing in other words">
                By meaning
              </li>
              {byMeaning.map((s, i) => (
                <SearchItem key={"m" + s.id + i} slide={s} onPick={pick} />
              ))}
            </>
          )}
          <li className="px-3 py-1 text-right font-mono text-[10px] text-zinc-600">{result.elapsedMs.toFixed(2)} ms</li>
        </ul>
      )}
    </div>
  );
}

function SearchItem({ slide, onPick }: { slide: Slide; onPick: (s: Slide) => void }) {
  return (
    <li onClick={() => onPick(slide)} className="cursor-pointer px-3 py-2 text-sm hover:bg-zinc-800">
      <span className="font-semibold text-amber-400">{slide.label}</span> <span className="text-zinc-400">{slide.verses[0]?.text}</span>
    </li>
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
