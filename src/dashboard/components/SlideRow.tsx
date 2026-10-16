import { useT } from "../../lib/i18n";
import type { Slide } from "../../lib/ipc";
import { useDashboard } from "../store";

/** A slide in a list: click to preview, double-click or "Live" to put it on screen. */
export function SlideRow({
  slide,
  title,
  badge,
}: {
  slide: Slide;
  title?: string;
  badge?: string;
}) {
  const setPreview = useDashboard((s) => s.setPreview);
  const goLive = useDashboard((s) => s.goLive);
  const isLive = useDashboard((s) => s.live?.slide?.id === slide.id);
  const isPreview = useDashboard((s) => s.preview?.id === slide.id);
  const t = useT();
  return (
    <li
      onClick={() => setPreview(slide)}
      onDoubleClick={() => goLive(slide)}
      className={`flex cursor-pointer items-start gap-2 rounded-lg border p-2.5 transition-colors ${
        isLive
          ? "border-red-500/70 bg-red-500/[.06]"
          : isPreview
            ? "border-amber-500/70 bg-amber-500/[.06]"
            : "border-zinc-800 hover:border-zinc-600"
      }`}
    >
      <div className="min-w-0 flex-1">
        <div className="flex items-baseline gap-2">
          <span className="font-scripture text-lg leading-tight text-zinc-50">
            {title ?? slide.label}
          </span>
          {badge && <span className="text-xs text-zinc-500">{badge}</span>}
        </div>
        <p className="mt-0.5 line-clamp-2 text-sm font-medium leading-snug text-zinc-300">
          {slide.verses.map((v) => v.text).join(slide.song ? " / " : " ")}
        </p>
      </div>
      <button
        onClick={(e) => {
          e.stopPropagation();
          void goLive(slide);
        }}
        className="flex-none rounded bg-amber-500 px-2.5 py-1 text-xs font-semibold hover:bg-amber-400"
      >
        {t("Live")}
      </button>
    </li>
  );
}
