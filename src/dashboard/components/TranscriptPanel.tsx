import { useEffect, useMemo, useRef } from "react";
import { useDashboard } from "../store";

const clock = (ms: number) => new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

export function TranscriptPanel() {
  const finals = useDashboard((s) => s.finals);
  const partial = useDashboard((s) => s.partial);
  const queue = useDashboard((s) => s.queue);
  const voice = useDashboard((s) => s.voiceCommands);
  const endRef = useRef<HTMLDivElement>(null);

  // What each sentence led to, shown beside it ("→ John 3:16").
  const found = useMemo(() => {
    const m = new Map<number, string[]>();
    for (const d of queue) {
      if (d.tentative) continue;
      const list = m.get(d.utteranceId) ?? [];
      if (!list.includes(d.slide.label)) list.push(d.slide.label);
      m.set(d.utteranceId, list);
    }
    return m;
  }, [queue]);

  // Braces matter: newer engines return a Promise from scrollIntoView, and an effect that
  // returns a non-function makes React throw (and blank the dashboard) on the next update.
  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [finals, partial]);

  return (
    <div className="min-h-0 flex-1 space-y-2.5 overflow-y-auto px-4 pb-3 text-[15px] font-light leading-relaxed">
      {finals.length === 0 && !partial && <p className="text-zinc-500">Waiting for speech…</p>}
      {finals.map((l) => (
        <div
          key={l.utteranceId}
          className="grid grid-cols-[2.75rem_minmax(0,1fr)] gap-2"
          title={`${l.refined ? "Checked by the accuracy pass" : "Quick transcription"} · confidence ${(l.confidence * 100).toFixed(0)}% · ${(l.inferenceMs / 1000).toFixed(1)} s`}
        >
          <time className="pt-0.5 text-xs tabular-nums text-zinc-500">{clock(l.at)}</time>
          <p>
            <span className={l.sung ? "italic text-zinc-400" : l.confidence < 0.6 ? "text-zinc-400" : "text-zinc-200"}>{l.text}</span>
            {l.sung && (
              <span className="ml-2 inline-block rounded bg-zinc-800 px-1.5 align-middle text-xs text-zinc-400" title="Heard sung, not spoken: not treated as a quote or a command, and left out of sermon notes">
                ♪ sung
              </span>
            )}
            {l.refined && <span className="ml-1.5 align-middle text-[10px] text-zinc-500" aria-label="checked">✓</span>}
            {voice[l.utteranceId] && (
              <span
                className={`ml-2 inline-block rounded px-1.5 align-middle text-xs font-medium ${voice[l.utteranceId].moved ? "bg-amber-500/15 text-amber-200" : "bg-zinc-800 text-zinc-400"}`}
                title={voice[l.utteranceId].moved ? `Voice command: now showing ${voice[l.utteranceId].now}` : "Voice command: already at the end of the chapter"}
              >
                ↪ {voice[l.utteranceId].label}
                {voice[l.utteranceId].moved && voice[l.utteranceId].now ? ` · ${voice[l.utteranceId].now}` : ""}
              </span>
            )}
            {found.get(l.utteranceId)?.map((label) => (
              <span key={label} className="ml-2 inline-block rounded bg-amber-500/15 px-1.5 align-middle text-xs font-medium text-amber-200">
                → {label}
              </span>
            ))}
          </p>
        </div>
      ))}
      {partial && (
        <div className="grid grid-cols-[2.75rem_minmax(0,1fr)] gap-2">
          <span />
          <p className="text-zinc-100">
            {partial.text}
            <span className="ml-0.5 inline-block h-4 w-0.5 translate-y-0.5 bg-amber-500" />
          </p>
        </div>
      )}
      <div ref={endRef} />
    </div>
  );
}

/** Shown in the transcript header while the room is singing. */
export function SingingBadge() {
  const singing = useDashboard((s) => s.singingSince !== null);
  if (!singing) return <span className="font-ui text-[11px] font-light normal-case tracking-normal text-zinc-500">Checked lines marked ✓</span>;
  return (
    <span
      className="flex items-center gap-1.5 rounded-full bg-amber-500/10 px-2 py-0.5 font-ui text-[11px] font-medium normal-case tracking-normal text-amber-200"
      title="The room is singing: sung Bible phrases aren't suggested as quotes, and voice commands are ignored"
    >
      <span className="animate-pulse">♪</span> Singing
    </span>
  );
}

export function PanelHeader({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <div className="flex min-h-11 flex-none flex-wrap items-center justify-between gap-x-3 gap-y-1 px-4 py-2 font-display text-[13px] font-semibold uppercase tracking-[0.2em] text-amber-200">
      {title}
      {children}
    </div>
  );
}
