import { useEffect, useRef, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api, type DeviceInfo, type MonitorInfo } from "../../lib/ipc";
import { useDashboard } from "../store";
import { useDismiss } from "../useDismiss";
import { AccuracyButton } from "./AccuracyButton";
import { BibleLibrary } from "./BibleLibrary";
import { ScreensButton } from "./ScreensButton";
import { ShareButton } from "./ShareButton";

/** Device enumeration takes ~200–500 ms (off the UI thread), so don't repeat it within this. */
const DEVICE_REFRESH_MS = 3000;

export function ControlBar() {
  // Only what the bar shows: the level meter and engine badge subscribe to their own
  // fast-changing state, so this doesn't re-render ten times a second.
  const s = useDashboard(
    useShallow((st) => ({
      running: st.running,
      starting: st.starting,
      benchProgress: st.benchProgress,
      settings: st.settings,
      translations: st.translations,
      start: st.start,
      stop: st.stop,
      saveSettings: st.saveSettings,
      styleOpen: st.styleOpen,
      setStyleOpen: st.setStyleOpen,
    })),
  );
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [monitor, setMonitor] = useState<number | null>(null);
  const lastDeviceRefresh = useRef(0);

  // Refreshed as the pointer reaches the menu (before it opens), so a mic plugged in later shows up.
  const refreshDevices = () => {
    if (Date.now() - lastDeviceRefresh.current < DEVICE_REFRESH_MS) return;
    lastDeviceRefresh.current = Date.now();
    void api.listAudioDevices().then(setDevices).catch(console.error);
  };
  useEffect(() => {
    refreshDevices();
    api.listMonitors().then(setMonitors).catch(console.error);
  }, []);

  const [panel, setPanel] = useState<"accuracy" | "bibles" | null>(null);
  const settings = s.settings;
  if (!settings) return <header className="mx-3 mt-3 h-[60px] rounded-2xl border border-zinc-800" />;

  const select = "h-9 rounded-lg border border-zinc-800 bg-transparent px-2.5 text-sm font-light text-zinc-100 outline-none hover:border-zinc-600 focus:border-amber-500";
  const box = "flex h-9 min-w-0 items-center rounded-lg border border-zinc-800 hover:border-zinc-600 focus-within:border-amber-500";

  return (
    // A floating glass bar, as on the website, all on one line. Only what's used during the
    // service is here; everything else is under Settings.
    <header className="relative z-20 mx-3 mt-3 flex flex-nowrap items-center gap-2 rounded-2xl border border-white/10 bg-[rgba(20,16,30,.42)] py-2.5 pl-4 pr-2.5 text-sm shadow-[0_10px_40px_-12px_rgba(0,0,0,.7),inset_0_1px_0_rgba(255,255,255,.06)] backdrop-blur-xl backdrop-saturate-150">
      <div className="mr-2 flex flex-none items-center gap-2.5 font-display text-base font-semibold tracking-[0.14em]">
        <svg viewBox="0 0 32 32" className="h-5 w-5" aria-hidden>
          <path d="M16 3c3 4.5 5 7.2 5 10.2A5 5 0 0 1 16 18a5 5 0 0 1-5-4.8C11 10.2 13 7.5 16 3z" fill="#e9b24c" />
          <path d="M6 22c3.3-1.6 6.7-1.6 10 0 3.3-1.6 6.7-1.6 10 0v6c-3.3-1.6-6.7-1.6-10 0-3.3-1.6-6.7-1.6-10 0z" fill="#f4ede1" />
        </svg>
        <span className="hidden min-[1250px]:inline">HEARKEN</span>
      </div>

      <ListenButton running={s.running} starting={s.starting} benchProgress={s.benchProgress} onToggle={() => (s.running ? s.stop() : s.start())} />

      <div className={`${box} relative gap-2.5 pl-3 pr-2.5 ${s.running ? "" : "cursor-pointer"}`} title={s.running ? "Stop listening to change the input" : "Sound input"}>
        <Dropdown
          disabled={s.running}
          onMouseEnter={refreshDevices}
          onFocus={refreshDevices}
          value={settings.inputDevice ?? ""}
          onChange={(v) => s.saveSettings({ inputDevice: v || null })}
          label="Sound input"
          display={devices.find((d) => d.id === settings.inputDevice)?.name ?? (settings.inputDevice ? "Saved input" : "System default input")}
          className="max-w-44 font-semibold min-[1400px]:max-w-56"
          caret={false}
          fill
        >
          <option value="">System default input</option>
          {devices.map((d) => (
            <option key={d.id} value={d.id}>
              {d.kind === "loopback" ? "🔁 " : ""}
              {d.name}
            </option>
          ))}
        </Dropdown>
        <LevelMeter />
        <span aria-hidden className={`text-[10px] text-zinc-500 ${s.running ? "opacity-40" : ""}`}>
          ▼
        </span>
      </div>

      <div className={`${box} gap-1.5 pl-3 pr-2.5`}>
        <Dropdown value={settings.translation} onChange={(v) => s.saveSettings({ translation: v })} label="Translation" display={settings.translation} className="font-semibold">
          {s.translations.map((t) => (
            <option key={t.code} value={t.code} title={t.name}>
              {t.code}
            </option>
          ))}
        </Dropdown>
        <span className="text-zinc-500">+</span>
        <Dropdown
          value={settings.secondaryTranslation ?? ""}
          onChange={(v) => s.saveSettings({ secondaryTranslation: v || null })}
          label="Side by side"
          title="Show a second translation (or language) next to the first"
          display={settings.secondaryTranslation ?? "Side by side"}
          className={settings.secondaryTranslation ? "font-semibold" : "font-light !text-zinc-500"}
        >
          <option value="">Side by side</option>
          {s.translations
            .filter((t) => t.code !== settings.translation)
            .map((t) => (
              <option key={t.code} value={t.code} title={t.name}>
                {t.code}
              </option>
            ))}
        </Dropdown>
      </div>

      <div className="ml-auto flex flex-none items-center gap-0.5">
        {/* Opened from Settings; each shows its own button only while there's news (a download, a warning). */}
        <BibleLibrary open={panel === "bibles"} onOpenChange={(o) => setPanel(o ? "bibles" : null)} showTrigger={false} />
        <AccuracyButton open={panel === "accuracy"} onOpenChange={(o) => setPanel(o ? "accuracy" : null)} showTrigger={false} />
        <ShareButton />
        <ScreensButton monitors={monitors} />
        <SettingsMenu
          monitors={monitors}
          monitor={monitor}
          setMonitor={setMonitor}
          select={select}
          onStyle={() => s.setStyleOpen(true)}
          onPanel={(p) => setPanel(p)}
        />
        <button
          onClick={() => api.openOutputWindow(monitor)}
          className="ml-1.5 h-9 whitespace-nowrap rounded-lg bg-amber-500 px-3.5 text-sm font-bold hover:bg-amber-400"
          title="Open the projector output on the chosen display (Settings → Output display). Press Esc on it to close it."
        >
          Open output
        </button>
      </div>
    </header>
  );
}

/**
 * A select drawn like the rest of the bar: the chosen value in our type, as wide as that value
 * (a native select is as wide as its longest option), with the native menu on top, invisible.
 */
function Dropdown({
  value,
  display,
  onChange,
  caret = true,
  fill = false,
  label,
  title,
  className = "",
  disabled,
  onMouseEnter,
  onFocus,
  children,
}: {
  value: string;
  display: string;
  onChange: (v: string) => void;
  /** Show ▼ (inside the clickable area). */
  caret?: boolean;
  /** The native menu covers the nearest positioned box, so all of it opens the menu. */
  fill?: boolean;
  label: string;
  title?: string;
  className?: string;
  disabled?: boolean;
  onMouseEnter?: () => void;
  onFocus?: () => void;
  children: React.ReactNode;
}) {
  return (
    <span className={`${fill ? "" : "relative"} inline-flex h-8 min-w-0 items-center gap-1.5`} title={title}>
      <span aria-hidden className={`truncate text-sm text-zinc-100 ${className}`}>
        {display}
      </span>
      {caret && (
        <span aria-hidden className="text-[10px] text-zinc-500">
          ▼
        </span>
      )}
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        aria-label={label}
        disabled={disabled}
        onMouseEnter={onMouseEnter}
        onFocus={onFocus}
        className="absolute inset-0 z-10 h-full w-full cursor-pointer appearance-none opacity-0 disabled:cursor-default"
      >
        {children}
      </select>
    </span>
  );
}

/** Start/stop, with how long it has been listening. */
function ListenButton({ running, starting, benchProgress, onToggle }: { running: boolean; starting: boolean; benchProgress: string | null; onToggle: () => void }) {
  const [since, setSince] = useState<number | null>(null);
  const [, tick] = useState(0);
  useEffect(() => {
    if (!running) {
      setSince(null);
      return;
    }
    setSince(Date.now());
    const id = setInterval(() => tick((n) => n + 1), 1000);
    return () => clearInterval(id);
  }, [running]);
  const secs = since ? Math.floor((Date.now() - since) / 1000) : 0;
  const elapsed = `${Math.floor(secs / 3600)}:${String(Math.floor(secs / 60) % 60).padStart(2, "0")}:${String(secs % 60).padStart(2, "0")}`;

  return (
    <button
      disabled={starting}
      onClick={onToggle}
      title={running ? "Stop listening" : "Start listening"}
      className={`flex h-9 flex-none items-center gap-2.5 whitespace-nowrap rounded-lg px-3.5 text-sm font-semibold disabled:opacity-60 ${
        running ? "border border-amber-500/45 bg-amber-500/[.14] text-amber-200 hover:bg-amber-500/25" : "bg-amber-500 hover:bg-amber-400"
      }`}
    >
      {running && <span className="h-2 w-2 rounded-full bg-amber-500 shadow-[0_0_0_3px_rgba(233,178,76,.25)]" />}
      {starting ? (benchProgress ?? "Starting…") : running ? "Listening" : "Start listening"}
      {running && <span className="font-normal tabular-nums opacity-80">{elapsed}</span>}
    </button>
  );
}

/** Settings usually chosen before the service starts. */
function SettingsMenu({
  monitors,
  monitor,
  setMonitor,
  select,
  onStyle,
  onPanel,
}: {
  monitors: MonitorInfo[];
  monitor: number | null;
  setMonitor: (m: number | null) => void;
  select: string;
  onStyle: () => void;
  onPanel: (p: "accuracy" | "bibles") => void;
}) {
  const settings = useDashboard((st) => st.settings);
  const saveSettings = useDashboard((st) => st.saveSettings);
  const [open, setOpen] = useState(false);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  if (!settings) return null;
  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen(!open)}
        className={`flex h-9 items-center gap-1.5 rounded-lg px-2.5 text-sm font-light hover:bg-white/[.06] ${open ? "bg-white/[.06] text-zinc-100" : "text-zinc-300"}`}
        aria-expanded={open}
        aria-label="Settings"
        title="Settings"
      >
        <svg viewBox="0 0 24 24" className="h-4 w-4" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden>
          <circle cx="12" cy="12" r="3" />
          <path d="M12 2v3m0 14v3M2 12h3m14 0h3M4.9 4.9l2.1 2.1m10 10 2.1 2.1M4.9 19.1 7 17m10-10 2.1-2.1" />
        </svg>
        <span className="hidden min-[1250px]:inline">Settings</span>
      </button>
      {open && (
        <div className="absolute right-0 top-full z-30 mt-2 w-[22rem] max-w-[calc(100vw-2rem)] space-y-4 overflow-hidden rounded-xl bg-zinc-900 p-4 text-sm shadow-2xl ring-1 ring-zinc-700 backdrop-blur-xl">
          <div className="grid gap-0.5">
            <MenuItem onClick={() => { setOpen(false); onStyle(); }} hint="Fonts, colours, backgrounds">Style</MenuItem>
            <MenuItem onClick={() => { setOpen(false); onPanel("accuracy"); }} hint="Models, names to listen for, search by meaning">Transcription accuracy</MenuItem>
            <MenuItem onClick={() => { setOpen(false); onPanel("bibles"); }} hint="Yoruba, Igbo, Hausa and more">Bibles</MenuItem>
          </div>
          <div className="space-y-2.5 border-t border-zinc-800 pt-3">
            <h3 className="font-display text-xs font-semibold uppercase tracking-[0.2em] text-amber-200">During the service</h3>
            <Toggle checked={settings.followReading} onChange={(v) => saveSettings({ followReading: v })} label="Follow reading" title="Highlight the live passage as it's read and move to the next verse automatically" />
            <Toggle checked={settings.lyricsDetection} onChange={(v) => saveSettings({ lyricsDetection: v })} label="Follow worship songs" title="Recognise songs from your library as they're sung and follow them section by section" />
            <Toggle
              checked={settings.voiceCommands}
              onChange={(v) => saveSettings({ voiceCommands: v })}
              label="Voice commands (“next verse”, “go back”)"
              title="Move the live scripture when the preacher says “next verse”, “previous verse”, “go back” or “go to the first verse”"
            />
            <label className="flex items-center gap-2 text-zinc-300">
              <input type="checkbox" checked={settings.autoProject} onChange={(e) => saveSettings({ autoProject: e.target.checked })} className="accent-amber-500" />
              Show sure detections automatically, at
              <input
                type="number"
                min={0.5}
                max={1}
                step={0.01}
                value={settings.autoThreshold}
                onChange={(e) => saveSettings({ autoThreshold: Number(e.target.value) })}
                className="w-16 rounded-md border border-zinc-700 bg-transparent px-1.5 py-0.5 tabular-nums"
                aria-label="Automatic threshold"
              />
            </label>
          </div>
          <label className="block space-y-1.5">
            <span className="font-display text-xs font-semibold uppercase tracking-[0.2em] text-amber-200">Output display</span>
            <select value={monitor ?? ""} onChange={(e) => setMonitor(e.target.value === "" ? null : Number(e.target.value))} className={`${select} w-full`}>
              <option value="">Automatic (second display)</option>
              {monitors.map((m) => (
                <option key={m.index} value={m.index}>
                  {m.name} {m.width}×{m.height}
                  {m.primary ? " (primary)" : ""}
                </option>
              ))}
            </select>
            <button onClick={() => void api.closeOutputWindow()} className="text-xs font-light text-zinc-400 underline-offset-2 hover:text-zinc-100 hover:underline">
              Close the output window
            </button>
          </label>
          <div className="space-y-1.5 border-t border-zinc-800 pt-3">
            <span className="font-display text-xs font-semibold uppercase tracking-[0.2em] text-amber-200">Speech engine</span>
            <EngineBadge />
          </div>
        </div>
      )}
    </div>
  );
}

function MenuItem({ onClick, hint, children }: { onClick: () => void; hint: string; children: React.ReactNode }) {
  return (
    <button onClick={onClick} className="-mx-2 grid min-w-0 gap-0.5 rounded-lg px-2 py-1.5 text-left hover:bg-white/[.06]">
      <span className="font-medium text-zinc-100">{children}</span>
      <span className="text-xs font-light text-zinc-500">{hint}</span>
    </button>
  );
}

function Toggle({ checked, onChange, label, title }: { checked: boolean; onChange: (v: boolean) => void; label: string; title: string }) {
  return (
    <label className="flex items-center gap-1.5 text-zinc-300" title={title}>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} className="accent-amber-500" />
      {label}
    </label>
  );
}

function LevelMeter() {
  const db = useDashboard((s) => s.status?.levelDb ?? -90);
  const speaking = useDashboard((s) => s.status?.speaking ?? false);
  const pct = Math.max(0, Math.min(100, ((db + 60) / 60) * 100));
  return (
    <div className="flex items-center gap-2" title={`${db.toFixed(0)} dBFS`}>
      <span className={`h-1.5 w-1.5 rounded-full ${speaking ? "bg-amber-400" : "bg-zinc-600"}`} />
      <div className="h-1 w-14 overflow-hidden rounded bg-zinc-800">
        <div
          className={`h-full ${db > -6 ? "bg-red-500" : "bg-amber-500"}`}
          style={{ width: `${pct}%`, transition: "width 90ms linear" }}
        />
      </div>
    </div>
  );
}

function EngineBadge() {
  const { info, profile, status, lastLatencyMs, rebenchmark, running, benchProgress } = useDashboard(
    useShallow((s) => ({
      info: s.info,
      profile: s.profile,
      status: s.status,
      lastLatencyMs: s.lastLatencyMs,
      rebenchmark: s.rebenchmark,
      running: s.running,
      benchProgress: s.benchProgress,
    })),
  );
  const model = info?.model ?? profile?.model ?? "—";
  const backend = info?.backend ?? (profile ? (profile.useGpu ? "gpu" : "cpu") : "—");
  const title = profile
    ? `${profile.specs.os}/${profile.specs.arch} · ${profile.specs.totalRamGb.toFixed(0)} GB · ${profile.specs.logicalCores} cores\n` +
      profile.benchmarks.map((b) => `${b.model}/${b.backend}: ${b.ok ? `${b.inferMs.toFixed(0)} ms` : b.error}`).join("\n")
    : "Not benchmarked yet — runs automatically on first start";
  return (
    <div className="flex flex-wrap items-center gap-2 text-xs text-zinc-400" title={title}>
      <span className="rounded bg-zinc-800 px-2 py-1 font-mono text-zinc-200">
        {model} · {backend}
        {profile && <span className="ml-1 uppercase text-amber-400">{profile.tier}</span>}
      </span>
      {status && <span className="font-mono">ASR {status.inferenceMs.toFixed(0)}ms · step {status.stepMs}ms</span>}
      {lastLatencyMs !== null && <span className="font-mono">detect {lastLatencyMs.toFixed(0)}ms</span>}
      <button
        disabled={running || !!benchProgress}
        onClick={rebenchmark}
        className="rounded px-1.5 py-0.5 hover:bg-zinc-800 disabled:opacity-40"
      >
        {benchProgress && !running ? benchProgress : "Re-benchmark"}
      </button>
    </div>
  );
}
