import { useEffect } from "react";
import { api } from "../lib/ipc";
import { ControlBar } from "./components/ControlBar";
import { DetectionQueue } from "./components/DetectionQueue";
import { PreviewPane } from "./components/PreviewPane";
import { Sidebar } from "./components/Sidebar";
import { StyleDrawer } from "./components/StylePanel";
import { UpdateBanner } from "./components/UpdateBanner";
import { useDashboard } from "./store";

export function Dashboard() {
  const init = useDashboard((s) => s.init);
  const error = useDashboard((s) => s.error);

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    let cancelled = false;
    init().then((c) => (cancelled ? c() : (cleanup = c)));
    return () => {
      cancelled = true;
      cleanup?.();
    };
  }, [init]);

  useKeyboardShortcuts();

  return (
    <div className="flex h-full flex-col">
      <UpdateBanner />
      <ControlBar />
      {error && <div className="bg-red-950 px-4 py-2 text-sm text-red-200">{error}</div>}
      <div className="relative flex min-h-0 flex-1 overflow-hidden">
        <main className="grid min-h-0 flex-1 grid-cols-[minmax(280px,1fr)_minmax(300px,1fr)_minmax(420px,1.4fr)] gap-px bg-zinc-800">
          <Sidebar />
          <DetectionQueue />
          <PreviewPane />
        </main>
        <StyleDrawer />
      </div>
    </div>
  );
}

/**
 * Operator shortcuts (ignored while typing in an input):
 *   Enter → preview to live · Esc → clear · B → blackout · ←/→ → previous/next verse
 */
function useKeyboardShortcuts() {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (t.closest("input, textarea, select, [contenteditable]")) return;
      const { goLive, live, styleOpen, setStyleOpen } = useDashboard.getState();
      // With the Style drawer open, Esc closes it rather than clearing the screen.
      if (styleOpen && e.key === "Escape") {
        setStyleOpen(false);
        e.preventDefault();
        return;
      }
      switch (e.key) {
        case "Enter":
          void goLive();
          break;
        case "Escape":
          void api.clearOutput();
          break;
        case "b":
        case "B":
          void api.setBlackout(!live?.blackout);
          break;
        case "ArrowRight":
          void api.stepVerse(1);
          break;
        case "ArrowLeft":
          void api.stepVerse(-1);
          break;
        default:
          return;
      }
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
