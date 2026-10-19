import { useEffect, useMemo, useState } from "react";
import { useT } from "../../lib/i18n";
import { api, type SearchResponse, type Slide } from "../../lib/ipc";
import { parseTheme } from "../../lib/theme";
import { SlideView } from "../../output/SlideView";
import { Stage } from "../../output/Stage";
import { useDashboard } from "../store";
import { PassagePicker } from "./PassagePicker";
import { DraftBar } from "./StylePanel";

/**
 * Preview and Live side by side, like a video switcher, with Go live between them. Preview
 * shows unapplied style edits; Live always mirrors what's on the projector.
 */
export function Monitors() {
  const preview = useDashboard((s) => s.preview);
  const live = useDashboard((s) => s.live);
  const goLive = useDashboard((s) => s.goLive);
  const draftTheme = useDashboard((s) => s.draftTheme);
  const t = useT();
  const theme = useMemo(() => parseTheme(live?.theme), [live?.theme]);
  const previewTheme = draftTheme ?? theme;
  const onAir = !!live?.slide && !live.blackout;
  const status = live?.blackout
    ? t("BLACKED OUT")
    : live?.slide
      ? t("ON SCREEN")
      : t("CLEAR");

  return (
    // Capped so the monitors leave room for the transcript and plan on very wide screens.
    <div
      data-preview-column
      data-tour="monitors"
      className="mx-auto grid w-full min-w-0 max-w-[calc(149vh+130px)] grid-cols-[minmax(0,1fr)_auto_minmax(0,1fr)] items-center gap-3"
    >
      <div className="min-w-0 space-y-2">
        <MonitorLabel title={t("Preview")}>
          <span className="truncate text-xs font-semibold text-zinc-500">
            {preview
              ? `${preview.label} · ${preview.translation}`
              : t("Nothing selected")}
          </span>
        </MonitorLabel>
        <DraftBar />
        <StageBox
          ring={`ring-1 ${draftTheme ? "ring-amber-500" : "ring-zinc-800"}`}
        >
          <Stage background={previewTheme.background.color}>
            <SlideView
              slide={preview ?? (draftTheme ? SAMPLE_SLIDE : null)}
              theme={previewTheme}
              blackout={false}
            />
          </Stage>
        </StageBox>
      </div>

      <div className="flex flex-col items-center gap-1.5 pt-9">
        <button
          data-tour="golive"
          disabled={!preview}
          onClick={() => goLive()}
          className="flex h-[84px] w-[84px] flex-col items-center justify-center gap-1 rounded-xl bg-amber-500 text-sm font-extrabold hover:bg-amber-400 disabled:opacity-35"
          title={t("Put Preview on screen (Enter)")}
        >
          {t("Go live")}
          <span className="text-[11px] font-semibold opacity-70">{t("⏎ Enter")}</span>
        </button>
      </div>

      <div className="min-w-0 space-y-2">
        <MonitorLabel title={t("Live")}>
          <span
            className={`mr-auto flex items-center gap-1.5 text-[11px] font-bold tracking-[0.12em] ${onAir ? "text-red-400" : "text-zinc-500"}`}
          >
            {onAir && (
              <span className="h-1.5 w-1.5 rounded-full bg-red-500 shadow-[0_0_8px_#ef4444]" />
            )}
            <span className="hidden whitespace-nowrap min-[1300px]:inline">
              {status}
            </span>
          </span>
          <span className="flex flex-none gap-1">
            <Btn onClick={() => api.stepVerse(-1)} title={t("Previous verse (←)")}>
              ◀<span className="hidden min-[1300px]:inline"> {t("Verse")}</span>
            </Btn>
            <Btn onClick={() => api.stepVerse(1)} title={t("Next verse (→)")}>
              <span className="hidden min-[1300px]:inline">{t("Verse")} </span>▶
            </Btn>
            <Btn
              onClick={() => api.setBlackout(!live?.blackout)}
              title={t("Black out the screen (B)")}
              danger
            >
              {live?.blackout ? t("Unblack") : t("Black")}
            </Btn>
            <Btn
              onClick={() => api.clearOutput()}
              title={t("Remove the verse from the screen (Esc)")}
            >
              {t("Clear")}
            </Btn>
          </span>
        </MonitorLabel>
        <StageBox
          ring={
            onAir
              ? "ring-2 ring-red-600 shadow-[0_0_40px_-14px_rgba(229,72,77,.6)]"
              : "ring-1 ring-zinc-800"
          }
        >
          <Stage background={theme.background.color}>
            <SlideView
              slide={live?.slide ?? null}
              theme={theme}
              blackout={live?.blackout ?? false}
              reading={live?.reading}
              qr={live?.qr}
            />
          </Stage>
        </StageBox>
      </div>
    </div>
  );
}

function MonitorLabel({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex h-7 items-center justify-between gap-3">
      <h3 className="flex-none text-[12px] font-bold uppercase tracking-[0.14em] text-amber-200">
        {title}
      </h3>
      {children}
    </div>
  );
}

/** Finding a passage by hand, and stepping, blacking out or clearing what's live. */
export function Finder() {
  return (
    <div
      data-preview-column
      data-tour="finder"
      className="flex min-w-0 flex-wrap items-start gap-2"
    >
      <div className="min-w-[16rem] flex-[1_1_18rem]">
        <SearchBox />
      </div>
      <div className="min-w-0 flex-[1_1_26rem]">
        <PassagePicker />
      </div>
    </div>
  );
}

/**
 * A 16:9 monitor as wide as its column. The width comes first (block layout) and the height
 * follows from it: with aspect-ratio on a flex item, WebKit sized these boxes from their height
 * instead and made them wider than the column.
 */
function StageBox({
  ring,
  children,
}: {
  ring: string;
  children: React.ReactNode;
}) {
  return (
    <div
      className={`relative aspect-video w-full overflow-hidden rounded-lg ${ring}`}
    >
      <div className="absolute inset-0">{children}</div>
    </div>
  );
}

function Btn({
  onClick,
  children,
  title,
  danger,
}: {
  onClick: () => void;
  children: React.ReactNode;
  title?: string;
  danger?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      title={title}
      className={`h-7 rounded-md border px-2.5 text-xs font-semibold ${danger ? "border-red-500/50 text-red-300 hover:bg-red-500/10" : "border-zinc-800 text-zinc-300 hover:border-zinc-600 hover:text-zinc-100"}`}
    >
      {children}
    </button>
  );
}

function SearchBox() {
  const translation = useDashboard((s) => s.settings?.translation ?? "KJV");
  const setPreview = useDashboard((s) => s.setPreview);
  const goLive = useDashboard((s) => s.goLive);
  const previewId = useDashboard((s) => s.preview?.id);
  const [query, setQuery] = useState("");
  /** The passage the last Enter put in Preview: Enter again (with nothing new typed) takes it live. */
  const [armed, setArmed] = useState<string | null>(null);
  const ready = armed !== null && armed === previewId;
  const [result, setResult] = useState<SearchResponse | null>(null);
  const t = useT();

  useEffect(() => {
    if (!query.trim()) {
      setResult(null);
      return;
    }
    const id = setTimeout(() => {
      api
        .searchScripture(query, translation)
        .then(setResult)
        .catch(console.error);
    }, 80);
    return () => clearTimeout(id);
  }, [query, translation]);

  const items: Slide[] = result
    ? [...(result.reference ? [result.reference] : []), ...result.hits]
    : [];
  const byMeaning = result?.meaning ?? [];
  const pick = (s: Slide) => {
    setPreview(s);
    setQuery("");
  };
  const enter = () => {
    const first = items[0] ?? byMeaning[0];
    if (query.trim() && first) {
      pick(first);
      setArmed(first.id);
    } else if (!query.trim() && ready) {
      void goLive();
      setArmed(null);
    }
  };

  return (
    <div className="relative">
      <input
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setArmed(null);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            enter();
          }
          if (e.key === "Escape") {
            setQuery("");
            setArmed(null);
          }
        }}
        onBlur={() => setArmed(null)}
        aria-label={t("Search scripture")}
        placeholder={
          ready
            ? t("Press Enter again to go live")
            : t("Search: “jn 3 16”, “love is patient” or “don't worry about tomorrow”")
        }
        className={`h-10 w-full rounded-lg bg-white/[.02] px-3.5 text-sm font-medium outline-none ring-1 ring-zinc-800 focus:ring-amber-500 ${ready ? "placeholder:text-amber-300" : "placeholder:text-zinc-500"}`}
      />
      {result && (
        <ul className="absolute inset-x-3 top-full z-10 mt-1 max-h-80 overflow-y-auto rounded-md bg-zinc-900 shadow-xl ring-1 ring-zinc-700">
          {items.length === 0 && byMeaning.length === 0 && (
            <li className="px-3 py-2 text-sm text-zinc-500">{t("No matches")}</li>
          )}
          {items.map((s, i) => (
            <SearchItem key={s.id + i} slide={s} onPick={pick} />
          ))}
          {byMeaning.length > 0 && (
            <>
              <li
                className="border-t border-zinc-800 px-3 pb-1 pt-2 text-[11px] font-semibold uppercase tracking-wider text-sky-400"
                title={t("Verses that say the same thing in other words")}
              >
                {t("By meaning")}
              </li>
              {byMeaning.map((s, i) => (
                <SearchItem key={"m" + s.id + i} slide={s} onPick={pick} />
              ))}
            </>
          )}
          <li className="px-3 py-1 text-right font-mono text-[10px] text-zinc-600">
            {result.elapsedMs.toFixed(2)} ms
          </li>
        </ul>
      )}
    </div>
  );
}

function SearchItem({
  slide,
  onPick,
}: {
  slide: Slide;
  onPick: (s: Slide) => void;
}) {
  return (
    <li
      onClick={() => onPick(slide)}
      className="cursor-pointer px-3 py-2 text-sm hover:bg-zinc-800"
    >
      <span className="font-semibold text-amber-400">{slide.label}</span>{" "}
      <span className="text-zinc-400">{slide.verses[0]?.text}</span>
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
