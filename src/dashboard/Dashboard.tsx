import { useEffect } from "react";
import { api } from "../lib/ipc";
import { ControlBar } from "./components/ControlBar";
import { DetectionQueue } from "./components/DetectionQueue";
import { Finder, Monitors } from "./components/PreviewPane";
import { Sidebar } from "./components/Sidebar";
import { PanelHeader, SingingBadge, TranscriptPanel } from "./components/TranscriptPanel";
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
    <div className="relative flex h-full flex-col">
      {/* A faint light from above, as on the website. */}
      <div
        aria-hidden
        className="pointer-events-none absolute left-1/2 top-[-300px] h-[660px] w-[1200px] max-w-full -translate-x-1/2 bg-[radial-gradient(closest-side,rgba(255,248,228,.12),rgba(248,224,164,.045)_45%,transparent_75%)]"
      />
      <UpdateBanner />
      <ControlBar />
      {error && (
        <div className="flex items-start gap-3 bg-red-950 px-4 py-2 text-sm text-red-200">
          <span className="flex-1">{error}</span>
          <button onClick={() => useDashboard.setState({ error: null })} className="text-red-300 hover:text-white" aria-label="Dismiss">
            ×
          </button>
        </div>
      )}
      <div className="relative flex min-h-0 flex-1 overflow-hidden">
        {/* Switcher layout: the queue on the left; Preview and Live side by side in the centre;
            what's being said and the service plan underneath. */}
        <main className="grid min-h-0 flex-1 grid-cols-[clamp(280px,24vw,380px)_minmax(0,1fr)] gap-3 p-3 pt-2">
          <DetectionQueue />
          <section className="grid min-h-0 min-w-0 grid-rows-[auto_auto_minmax(0,1fr)] gap-3">
            <Monitors />
            <Finder />
            <div className="grid min-h-0 grid-cols-[minmax(0,1.5fr)_minmax(0,1fr)] gap-3">
              <section className="flex min-h-0 min-w-0 flex-col overflow-hidden rounded-xl border border-zinc-800 bg-white/[.02]">
                <PanelHeader title="Transcript">
                  <SingingBadge />
                </PanelHeader>
                <TranscriptPanel />
              </section>
              <Sidebar />
            </div>
          </section>
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
      const { goLive, live } = useDashboard.getState();
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
