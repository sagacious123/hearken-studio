import type { Detection } from "../../lib/ipc";
import { useDashboard } from "../store";
import { PanelHeader } from "./TranscriptPanel";

const KIND_LABEL = { explicit: "Announced", contextual: "Follow-up", quote: "Quoted", song: "Song" } as const;

export function DetectionQueue() {
  const queue = useDashboard((s) => s.queue);
  const followLatest = useDashboard((s) => s.followLatest);

  return (
    <section className="flex min-h-0 flex-col bg-zinc-950">
      <PanelHeader title="Detections">
        <label className="flex items-center gap-1.5 font-normal normal-case tracking-normal">
          <input
            type="checkbox"
            checked={followLatest}
            onChange={(e) => useDashboard.setState({ followLatest: e.target.checked })}
            className="accent-amber-500"
          />
          Preview follows latest
        </label>
      </PanelHeader>
      <ul className="min-h-0 flex-1 space-y-2 overflow-y-auto p-3">
        {queue.length === 0 && <li className="px-1 text-sm text-zinc-500">References and songs will appear here as they're spoken or sung.</li>}
        {queue.map((d) => (
          <DetectionCard key={d.id} d={d} />
        ))}
      </ul>
    </section>
  );
}

function DetectionCard({ d }: { d: Detection }) {
  const setPreview = useDashboard((s) => s.setPreview);
  const goLive = useDashboard((s) => s.goLive);
  const dismiss = useDashboard((s) => s.dismiss);
  const isPreview = useDashboard((s) => s.preview?.id === d.slide.id);
  const isLive = useDashboard((s) => s.live?.slide?.id === d.slide.id);
  const pct = Math.round(d.confidence * 100);
  const color = d.confidence >= 0.9 ? "bg-emerald-500" : d.confidence >= 0.7 ? "bg-amber-400" : "bg-red-400";

  return (
    <li
      onClick={() => setPreview(d.slide)}
      onDoubleClick={() => goLive(d.slide)}
      className={`cursor-pointer rounded-lg border p-3 transition-colors ${
        isLive ? "border-red-500 bg-red-950/30" : isPreview ? "border-amber-500 bg-zinc-900" : "border-zinc-800 bg-zinc-900 hover:border-zinc-600"
      } ${d.tentative ? "animate-pulse opacity-70" : ""}`}
    >
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-lg font-semibold">{d.slide.label}</span>
        <span className="text-xs text-zinc-400">
          {d.planned && <span className="mr-1.5 rounded bg-sky-800 px-1 text-sky-100">Planned</span>}
          {d.kind === "quote" && d.confidence < 0.9 ? "Possible quote" : KIND_LABEL[d.kind]}
          {d.autoProjected && <span className="ml-1.5 rounded bg-red-600 px-1 text-white">AUTO</span>}
        </span>
      </div>
      <p className="mt-1 line-clamp-2 text-sm text-zinc-400">{d.slide.verses[0]?.text}</p>
      <div className="mt-2 flex items-center gap-2 text-xs text-zinc-500">
        <div className="h-1.5 w-20 overflow-hidden rounded bg-zinc-800">
          <div className={`h-full ${color}`} style={{ width: `${pct}%` }} />
        </div>
        <span className="font-mono">{pct}%</span>
        <span className="font-mono">{d.latencyMs.toFixed(0)} ms</span>
        <span className="truncate italic">“{d.matched}”</span>
        <div className="ml-auto flex gap-1">
          <button
            onClick={(e) => {
              e.stopPropagation();
              void goLive(d.slide);
            }}
            className="rounded bg-red-600 px-2 py-0.5 font-semibold text-white hover:bg-red-500"
          >
            Live
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              dismiss(d.id);
            }}
            className="rounded px-1.5 py-0.5 hover:bg-zinc-800"
            aria-label="Dismiss"
          >
            ✕
          </button>
        </div>
      </div>
    </li>
  );
}
