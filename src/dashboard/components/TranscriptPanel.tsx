import { useEffect, useRef } from "react";
import { useDashboard } from "../store";

export function TranscriptPanel() {
  const finals = useDashboard((s) => s.finals);
  const partial = useDashboard((s) => s.partial);
  const endRef = useRef<HTMLDivElement>(null);

  useEffect(() => endRef.current?.scrollIntoView({ block: "end" }), [finals, partial]);

  return (
    <div className="min-h-0 flex-1 space-y-2 overflow-y-auto px-4 py-3 text-[15px] leading-relaxed">
      {finals.length === 0 && !partial && <p className="text-zinc-500">Waiting for speech…</p>}
      {finals.map((l) => (
        <p key={l.utteranceId} title={`ASR confidence ${(l.confidence * 100).toFixed(0)}% · ${l.inferenceMs.toFixed(0)} ms`}>
          <span className={l.confidence < 0.6 ? "text-zinc-400" : "text-zinc-100"}>{l.text}</span>
        </p>
      ))}
      {partial && <p className="italic text-zinc-500">{partial.text}</p>}
      <div ref={endRef} />
    </div>
  );
}

export function PanelHeader({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <div className="flex h-10 flex-none items-center justify-between border-b border-zinc-800 px-4 text-xs font-semibold uppercase tracking-wider text-zinc-400">
      {title}
      {children}
    </div>
  );
}
