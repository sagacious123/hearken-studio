import { useEffect, useRef } from "react";
import { useDashboard } from "../store";

export function TranscriptPanel() {
  const finals = useDashboard((s) => s.finals);
  const partial = useDashboard((s) => s.partial);
  const endRef = useRef<HTMLDivElement>(null);

  // Braces matter: newer engines return a Promise from scrollIntoView, and an effect that
  // returns a non-function makes React throw (and blank the dashboard) on the next update.
  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [finals, partial]);

  return (
    <div className="min-h-0 flex-1 space-y-2 overflow-y-auto px-4 py-3 text-[15px] leading-relaxed">
      {finals.length === 0 && !partial && <p className="text-zinc-500">Waiting for speech…</p>}
      {finals.map((l) => (
        <p
          key={l.utteranceId}
          title={`${l.refined ? "Checked by the accuracy pass" : "Quick transcription"} · confidence ${(l.confidence * 100).toFixed(0)}% · ${(l.inferenceMs / 1000).toFixed(1)} s`}
        >
          <span className={l.confidence < 0.6 ? "text-zinc-400" : "text-zinc-100"}>{l.text}</span>
          {l.refined && <span className="ml-1.5 align-middle text-[10px] text-emerald-500" aria-label="checked">✓</span>}
        </p>
      ))}
      {partial && <p className="italic text-zinc-500">{partial.text}</p>}
      <div ref={endRef} />
    </div>
  );
}

export function PanelHeader({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <div className="flex min-h-10 flex-none flex-wrap items-center justify-between gap-x-3 gap-y-1 border-b border-zinc-800 px-4 py-1.5 text-xs font-semibold uppercase tracking-wider text-zinc-400">
      {title}
      {children}
    </div>
  );
}
