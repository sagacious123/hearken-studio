import { useEffect, useState } from "react";
import { api, type MonitorInfo, type ScreenConfig } from "../../lib/ipc";
import { useDashboard } from "../store";
import { useDismiss } from "../useDismiss";

const MAX_SCREENS = 3;

const LOOKS: { value: ScreenConfig["look"]; label: string; hint: string }[] = [
  { value: "stage", label: "Stage display", hint: "For the preacher or worship leader: large text, what's been read, screen status and the time." },
  { value: "lower-third", label: "Lower third", hint: "For a livestream or video switcher: the passage in a band at the bottom, over black or green to key out." },
  { value: "full", label: "Same as main", hint: "Another projector or TV showing the main look." },
];

/** Extra screens beside the main output: a stage display, a lower third for streaming, or a copy. */
export function ScreensButton({ monitors }: { monitors: MonitorInfo[] }) {
  const screens = useDashboard((s) => s.settings?.screens ?? []);
  const saveSettings = useDashboard((s) => s.saveSettings);
  const [open, setOpen] = useState(false);
  const [live, setLive] = useState<number[]>([]);
  const [error, setError] = useState<string | null>(null);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));

  useEffect(() => {
    if (open) api.listOpenScreens().then(setLive).catch(() => {});
  }, [open]);

  const update = (n: number, patch: Partial<ScreenConfig>) => void saveSettings({ screens: screens.map((s, i) => (i === n ? { ...s, ...patch } : s)) });
  const run = (p: Promise<number[]>) => {
    setError(null);
    p.then(setLive).catch((e) => setError(String(e)));
  };

  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen(!open)}
        className={`rounded px-3 py-1 font-medium ${live.length ? "bg-amber-700 hover:bg-amber-600" : "bg-zinc-700 hover:bg-zinc-600"}`}
        title="More screens: stage display, livestream lower third"
      >
        Screens{live.length ? " ●" : ""}
      </button>
      {open && (
        <div className="absolute right-0 top-full z-20 mt-2 max-h-[calc(100dvh-7rem)] w-[26rem] max-w-[calc(100vw-2rem)] space-y-3 overflow-y-auto overscroll-contain rounded-lg bg-zinc-900 p-4 text-sm shadow-2xl ring-1 ring-zinc-700">
          <div>
            <h3 className="font-semibold">More screens</h3>
            <p className="text-xs text-zinc-400">Each follows what's live, on its own display. The main output keeps its own button.</p>
          </div>
          {error && <p className="text-xs text-red-400">{error}</p>}
          {screens.map((sc, n) => {
            const isOpen = live.includes(n);
            return (
              <div key={n} className="space-y-2 rounded-md bg-zinc-950 p-3 ring-1 ring-zinc-800">
                <div className="flex items-center gap-2">
                  <span className="font-medium">Screen {n + 2}</span>
                  {isOpen && <span className="text-xs text-emerald-400">● open</span>}
                  <button
                    onClick={() => {
                      if (isOpen) void api.closeScreen(n);
                      void saveSettings({ screens: screens.filter((_, i) => i !== n) });
                    }}
                    className="ml-auto text-xs text-zinc-500 hover:text-zinc-200"
                    title="Remove this screen"
                  >
                    Remove
                  </button>
                </div>
                <div className="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-2 gap-y-1.5 text-xs">
                  <span className="text-zinc-400">Shows</span>
                  <select value={sc.look} onChange={(e) => update(n, { look: e.target.value as ScreenConfig["look"] })} className="min-w-0 rounded bg-zinc-800 px-2 py-1">
                    {LOOKS.map((l) => (
                      <option key={l.value} value={l.value}>
                        {l.label}
                      </option>
                    ))}
                  </select>
                  {sc.look === "lower-third" && (
                    <>
                      <span className="text-zinc-400">Behind it</span>
                      <select value={sc.key} onChange={(e) => update(n, { key: e.target.value as ScreenConfig["key"] })} className="min-w-0 rounded bg-zinc-800 px-2 py-1">
                        <option value="black">Black (luma key)</option>
                        <option value="green">Green (chroma key)</option>
                        <option value="theme">Theme colour</option>
                      </select>
                    </>
                  )}
                  <span className="text-zinc-400">Display</span>
                  <select
                    value={sc.monitor ?? ""}
                    onChange={(e) => update(n, { monitor: e.target.value === "" ? null : Number(e.target.value) })}
                    className="min-w-0 rounded bg-zinc-800 px-2 py-1"
                  >
                    <option value="">Auto</option>
                    {monitors.map((m) => (
                      <option key={m.index} value={m.index}>
                        {m.name} {m.width}×{m.height}
                        {m.primary ? " (primary)" : ""}
                      </option>
                    ))}
                  </select>
                </div>
                <p className="text-[11px] text-zinc-500">{LOOKS.find((l) => l.value === sc.look)?.hint}</p>
                <div className="flex gap-2">
                  <button onClick={() => run(api.openScreen(n))} className="rounded bg-amber-600 px-2.5 py-1 text-xs font-medium hover:bg-amber-500">
                    {isOpen ? "Move / refresh" : "Open"}
                  </button>
                  {isOpen && (
                    <button onClick={() => run(api.closeScreen(n))} className="rounded bg-zinc-700 px-2.5 py-1 text-xs hover:bg-zinc-600">
                      Close
                    </button>
                  )}
                </div>
              </div>
            );
          })}
          {screens.length < MAX_SCREENS && (
            <button
              onClick={() => void saveSettings({ screens: [...screens, { look: screens.length === 0 ? "stage" : "lower-third", key: "black", monitor: null }] })}
              className="w-full rounded-md border border-dashed border-zinc-700 py-2 text-xs text-zinc-300 hover:border-zinc-500 hover:text-zinc-100"
            >
              ＋ Add a screen
            </button>
          )}
          <p className="text-[11px] text-zinc-500">Press Esc on a screen to close it. One display can't show two screens at once; on a laptop alone, screens open as windows.</p>
        </div>
      )}
    </div>
  );
}
