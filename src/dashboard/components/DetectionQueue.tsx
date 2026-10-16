import { useEffect, useState } from "react";
import type { Detection } from "../../lib/ipc";
import { useT } from "../../lib/i18n";
import { useDashboard } from "../store";
import { PanelHeader } from "./TranscriptPanel";

const KIND_LABEL = {
  explicit: "Announced",
  contextual: "Follow-up",
  quote: "Quoted",
  song: "Song",
  meaning: "Same meaning",
  command: "Voice command",
} as const;

/** The queue: everything Hearken heard, newest first. Click to preview, double-click or Live to show. */
export function DetectionQueue() {
  const queue = useDashboard((s) => s.queue);
  const followLatest = useDashboard((s) => s.followLatest);
  const t = useT();

  return (
    <section className="flex min-h-0 min-w-0 flex-col overflow-hidden rounded-xl border border-zinc-800 bg-white/[.02]">
      <PanelHeader title={t("Detections")}>
        <label
          className="flex items-center gap-1.5 font-ui text-xs font-semibold normal-case tracking-normal text-zinc-400"
          title={t("Each new detection goes straight into Preview")}
        >
          <input
            type="checkbox"
            checked={followLatest}
            onChange={(e) =>
              useDashboard.setState({ followLatest: e.target.checked })
            }
            className="accent-amber-500"
          />
          {t("Preview follows latest")}
        </label>
      </PanelHeader>
      <UnknownSong />
      <ul className="min-h-0 flex-1 overflow-y-auto">
        {queue.length === 0 && (
          <li className="border-t border-zinc-800 px-4 py-4 text-sm font-semibold text-zinc-500">
            {t("References and songs appear here as they’re spoken or sung.")}
          </li>
        )}
        {queue.map((d) => (
          <DetectionRow key={d.id} d={d} />
        ))}
      </ul>
      <footer className="flex flex-none flex-wrap gap-x-3 gap-y-1 border-t border-zinc-800 px-4 py-2.5 text-xs font-medium text-zinc-500">
        <Key k="⏎">{t("go live")}</Key>
        <Key k="← →">{t("verse")}</Key>
        <Key k="B">{t("black")}</Key>
        <Key k="Esc">{t("clear")}</Key>
      </footer>
    </section>
  );
}

/** How long the room sings, with no song recognised, before suggesting the song be added. */
const UNKNOWN_SONG_AFTER_MS = 20_000;

/** The room is singing, but nothing in the song library matches and no song is on screen. */
function UnknownSong() {
  const since = useDashboard((s) => s.singingSince);
  const lastSongAt = useDashboard((s) => s.lastSongAt);
  const songLive = useDashboard((s) => !!s.live?.slide?.song);
  const [, tick] = useState(0);
  const tr = useT();
  useEffect(() => {
    if (since === null) return;
    const wait = since + UNKNOWN_SONG_AFTER_MS - Date.now();
    if (wait <= 0) return;
    const t = setTimeout(() => tick((n) => n + 1), wait);
    return () => clearTimeout(t);
  }, [since]);
  const show =
    since !== null &&
    Date.now() - since >= UNKNOWN_SONG_AFTER_MS &&
    !songLive &&
    (lastSongAt === null || lastSongAt < since);
  if (!show) return null;
  // One sentence, its [opening] highlighted, so translations can move the highlight.
  const [before, lead, after] = tr(
    "[♪ A song is being sung] that isn’t in your song library. Add its lyrics under Songs and Hearken will find and follow it next time.",
  ).split(/\[|\]/);
  return (
    <p className="flex-none border-t border-zinc-800 bg-amber-500/[.06] px-4 py-2.5 text-xs font-medium leading-snug text-zinc-300">
      {before}
      <span className="font-medium text-amber-200">{lead}</span>
      {after}
    </p>
  );
}

function Key({ k, children }: { k: string; children: React.ReactNode }) {
  return (
    <span>
      <kbd className="mr-1 rounded border border-zinc-700 px-1.5 py-px font-ui text-[11px] font-medium text-zinc-300">
        {k}
      </kbd>
      {children}
    </span>
  );
}

function DetectionRow({ d }: { d: Detection }) {
  const setPreview = useDashboard((s) => s.setPreview);
  const goLive = useDashboard((s) => s.goLive);
  const dismiss = useDashboard((s) => s.dismiss);
  const isPreview = useDashboard((s) => s.preview?.id === d.slide.id);
  const isLive = useDashboard((s) => s.live?.slide?.id === d.slide.id);
  const pct = Math.round(d.confidence * 100);
  const t = useT();

  return (
    <li
      onClick={() => setPreview(d.slide)}
      onDoubleClick={() => goLive(d.slide)}
      className={`relative cursor-pointer border-t border-zinc-800 px-4 py-3 transition-colors ${
        isPreview
          ? "bg-amber-500/[.07] shadow-[inset_3px_0_0_var(--color-amber-500)]"
          : "hover:bg-white/[.03]"
      } ${d.tentative ? "animate-pulse opacity-70" : ""}`}
    >
      <div className="flex items-baseline justify-between gap-2">
        <span className="min-w-0 truncate font-scripture text-xl text-zinc-50">
          {d.slide.label}
        </span>
        <span
          className="flex flex-none items-center gap-1.5 text-xs tabular-nums text-zinc-400"
          title={t("{pct}% sure", { pct })}
        >
          <span className="h-[3px] w-8 overflow-hidden rounded bg-zinc-700">
            <span
              className={`block h-full ${d.confidence >= 0.7 ? "bg-amber-500" : "bg-amber-700"}`}
              style={{ width: `${pct}%` }}
            />
          </span>
          {pct}%
        </span>
      </div>
      <p className="mt-0.5 flex flex-wrap items-center gap-x-1.5 text-xs font-medium text-zinc-500">
        <span className="font-medium text-zinc-300">
          {d.kind === "quote" && d.confidence < 0.9
            ? t("Possible quote")
            : t(KIND_LABEL[d.kind])}
        </span>
        <span className="min-w-0 truncate">· “{d.matched}”</span>
        {d.planned && <span>· {t("in plan")}</span>}
        {d.autoProjected && (
          <span className="font-medium text-amber-300">
            · {d.kind === "command" ? t("shown") : t("shown automatically")}
          </span>
        )}
        {isLive && (
          <span className="font-semibold tracking-wider text-red-400 text-[10px]">
            · {t("ON SCREEN")}
          </span>
        )}
      </p>
      <p className="mt-1 line-clamp-2 text-sm font-normal leading-snug text-zinc-300">
        {d.slide.verses[0]?.text}
      </p>
      <div className="mt-2 flex items-center justify-end gap-1 text-xs">
        <span className="mr-auto font-medium tabular-nums text-zinc-600">
          {(d.latencyMs / 1000).toFixed(1)} s
        </span>
        <button
          onClick={(e) => {
            e.stopPropagation();
            dismiss(d.id);
          }}
          className="rounded px-2 py-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
          aria-label={t("Dismiss")}
        >
          {t("Dismiss")}
        </button>
        <button
          onClick={(e) => {
            e.stopPropagation();
            void goLive(d.slide);
          }}
          className="rounded bg-amber-500 px-2.5 py-1 font-semibold hover:bg-amber-400"
        >
          {t("Live")}
        </button>
      </div>
    </li>
  );
}
