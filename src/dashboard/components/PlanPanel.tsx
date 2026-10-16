import { useEffect, useState } from "react";
import { api, type Slide } from "../../lib/ipc";
import { useDashboard } from "../store";
import { SlideRow } from "./SlideRow";

/**
 * The pastor's notes for this service. Every reference in them is lined up ready to show,
 * and gets a confidence boost when it's spoken, so planned verses go up faster.
 */
export function PlanPanel() {
  const translation = useDashboard((s) => s.settings?.translation);
  const secondary = useDashboard((s) => s.settings?.secondaryTranslation);
  const [text, setText] = useState("");
  const [slides, setSlides] = useState<Slide[]>([]);
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Reload when the translation changes so the lined-up slides match the projector.
  useEffect(() => {
    api
      .getPlan()
      .then((p) => {
        setText(p.text);
        setSlides(p.slides);
      })
      .catch((e) => setError(String(e)));
  }, [translation, secondary]);

  const save = async () => {
    if (!dirty) return;
    try {
      const p = await api.setPlan(text);
      setSlides(p.slides);
      setDirty(false);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex-none space-y-2 border-b border-zinc-800 p-3">
        <p className="text-xs text-zinc-500 font-normal">
          Paste the pastor's notes or verse list. Every reference is lined up
          below, and gets a confidence boost when it's spoken.
        </p>
        <textarea
          value={text}
          onChange={(e) => {
            setText(e.target.value);
            setDirty(true);
          }}
          onBlur={save}
          rows={3}
          spellCheck={false}
          placeholder={
            "Intro: John 3:16\n1. Rom 8:28-30\n2. Psalm 23\nClose: Philippians 4:13"
          }
          className="w-full resize-y rounded-lg bg-white/[.02] p-2 text-sm font-medium outline-none ring-1 ring-zinc-800 focus:ring-amber-500"
        />
        <div className="flex items-center gap-2">
          <button
            onClick={save}
            disabled={!dirty}
            className="rounded bg-amber-600 px-3 py-1 text-sm font-medium hover:bg-amber-500 disabled:opacity-40"
          >
            Line up references
          </button>
          <span className="text-xs text-zinc-500">{slides.length} found</span>
          {text && (
            <button
              onClick={async () => {
                setText("");
                const p = await api.setPlan("");
                setSlides(p.slides);
                setDirty(false);
              }}
              className="ml-auto text-xs text-zinc-500 hover:text-zinc-300"
            >
              Clear plan
            </button>
          )}
        </div>
        {error && <p className="text-xs text-red-400">{error}</p>}
      </div>
      <ul className="min-h-0 flex-1 space-y-2 overflow-y-auto p-3">
        {slides.length === 0 && (
          <li className="text-sm text-zinc-500">No references yet.</li>
        )}
        {slides.map((s, i) => (
          <SlideRow key={s.id + i} slide={s} />
        ))}
      </ul>
    </div>
  );
}
