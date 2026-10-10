import type { Slide } from "../../lib/ipc";
import { useDashboard } from "../store";

/** A slide in a list: click to preview, double-click or "Live" to put it on screen. */
export function SlideRow({ slide, title, badge }: { slide: Slide; title?: string; badge?: string }) {
  const { setPreview, goLive, preview, live } = useDashboard();
  const isLive = live?.slide?.id === slide.id;
  const isPreview = preview?.id === slide.id;
  return (
    <li
      onClick={() => setPreview(slide)}
      onDoubleClick={() => goLive(slide)}
      className={`flex cursor-pointer items-start gap-2 rounded-lg border p-2.5 transition-colors ${
        isLive ? "border-red-500 bg-red-950/30" : isPreview ? "border-amber-500 bg-zinc-900" : "border-zinc-800 bg-zinc-900 hover:border-zinc-600"
      }`}
    >
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="font-semibold">{title ?? slide.label}</span>
          {badge && <span className="text-xs text-zinc-500">{badge}</span>}
        </div>
        <p className="mt-0.5 line-clamp-2 text-sm text-zinc-400">{slide.verses.map((v) => v.text).join(slide.song ? " / " : " ")}</p>
      </div>
      <button
        onClick={(e) => {
          e.stopPropagation();
          void goLive(slide);
        }}
        className="flex-none rounded bg-red-600 px-2 py-0.5 text-xs font-semibold text-white hover:bg-red-500"
      >
        Live
      </button>
    </li>
  );
}
