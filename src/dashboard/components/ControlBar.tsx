import { useEffect, useState } from "react";
import { api, type DeviceInfo, type MonitorInfo } from "../../lib/ipc";
import { useDashboard } from "../store";
import { ShareButton } from "./ShareButton";

export function ControlBar() {
  const s = useDashboard();
  const [devices, setDevices] = useState<DeviceInfo[]>([]);
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [monitor, setMonitor] = useState<number | null>(null);

  // Re-listed whenever the menu is opened, so a mic or interface plugged in later shows up.
  const refreshDevices = () => void api.listAudioDevices().then(setDevices).catch(console.error);
  useEffect(() => {
    refreshDevices();
    api.listMonitors().then(setMonitors).catch(console.error);
  }, []);

  const settings = s.settings;
  if (!settings) return <header className="h-14 border-b border-zinc-800" />;

  return (
    <header className="flex flex-wrap items-center gap-x-5 gap-y-2 border-b border-zinc-800 bg-zinc-900 px-4 py-2 text-sm">
      <button
        disabled={s.starting}
        onClick={() => (s.running ? s.stop() : s.start())}
        className={`rounded-md px-4 py-1.5 font-semibold ${
          s.running ? "bg-red-600 hover:bg-red-500" : "bg-emerald-600 hover:bg-emerald-500"
        } disabled:opacity-50`}
      >
        {s.starting ? (s.benchProgress ?? "Starting…") : s.running ? "Stop listening" : "Start listening"}
      </button>

      <Field label="Input">
        <select
          disabled={s.running}
          onFocus={refreshDevices}
          onMouseDown={refreshDevices}
          value={settings.inputDevice ?? ""}
          onChange={(e) => s.saveSettings({ inputDevice: e.target.value || null })}
          className="max-w-56 rounded bg-zinc-800 px-2 py-1"
        >
          <option value="">System default</option>
          {devices.map((d) => (
            <option key={d.id} value={d.id}>
              {d.kind === "loopback" ? "🔁 " : ""}
              {d.name}
            </option>
          ))}
        </select>
      </Field>

      <LevelMeter db={s.status?.levelDb ?? -90} speaking={s.status?.speaking ?? false} />

      <EngineBadge />

      <Field label="Translation">
        <select
          value={settings.translation}
          onChange={(e) => s.saveSettings({ translation: e.target.value })}
          className="rounded bg-zinc-800 px-2 py-1"
        >
          {s.translations.map((t) => (
            <option key={t.code} value={t.code} title={t.name}>
              {t.code}
            </option>
          ))}
        </select>
      </Field>

      <Field label="Side by side">
        <select
          value={settings.secondaryTranslation ?? ""}
          onChange={(e) => s.saveSettings({ secondaryTranslation: e.target.value || null })}
          className="rounded bg-zinc-800 px-2 py-1"
          title="Show a second translation (or language) next to the first"
        >
          <option value="">Off</option>
          {s.translations
            .filter((t) => t.code !== settings.translation)
            .map((t) => (
              <option key={t.code} value={t.code} title={t.name}>
                {t.code}
              </option>
            ))}
        </select>
      </Field>

      <Toggle
        checked={settings.followReading}
        onChange={(v) => s.saveSettings({ followReading: v })}
        label="Follow reading"
        title="Highlight the live passage as it's read and move to the next verse automatically"
      />
      <Toggle
        checked={settings.lyricsDetection}
        onChange={(v) => s.saveSettings({ lyricsDetection: v })}
        label="Lyrics"
        title="Recognise songs from your library as they're sung and follow them section by section"
      />

      <label className="flex items-center gap-2">
        <input
          type="checkbox"
          checked={settings.autoProject}
          onChange={(e) => s.saveSettings({ autoProject: e.target.checked })}
          className="accent-amber-500"
        />
        Auto-project ≥
        <input
          type="number"
          min={0.5}
          max={1}
          step={0.01}
          value={settings.autoThreshold}
          onChange={(e) => s.saveSettings({ autoThreshold: Number(e.target.value) })}
          className="w-16 rounded bg-zinc-800 px-1 py-0.5"
        />
      </label>

      <div className="ml-auto flex items-center gap-2">
        <ShareButton />
        <select
          value={monitor ?? ""}
          onChange={(e) => setMonitor(e.target.value === "" ? null : Number(e.target.value))}
          className="rounded bg-zinc-800 px-2 py-1"
        >
          <option value="">Auto (secondary display)</option>
          {monitors.map((m) => (
            <option key={m.index} value={m.index}>
              {m.name} {m.width}×{m.height}
              {m.primary ? " (primary)" : ""}
            </option>
          ))}
        </select>
        <button onClick={() => api.openOutputWindow(monitor)} className="rounded bg-amber-600 px-3 py-1 font-medium hover:bg-amber-500">
          Open output
        </button>
        <button onClick={() => api.closeOutputWindow()} className="rounded bg-zinc-700 px-3 py-1 hover:bg-zinc-600">
          Close
        </button>
      </div>
    </header>
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

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex items-center gap-2 text-zinc-400">
      {label}
      <span className="text-zinc-100">{children}</span>
    </label>
  );
}

function LevelMeter({ db, speaking }: { db: number; speaking: boolean }) {
  const pct = Math.max(0, Math.min(100, ((db + 60) / 60) * 100));
  return (
    <div className="flex items-center gap-2" title={`${db.toFixed(0)} dBFS`}>
      <span className={`h-2.5 w-2.5 rounded-full ${speaking ? "bg-emerald-400" : "bg-zinc-600"}`} />
      <div className="h-2 w-28 overflow-hidden rounded bg-zinc-800">
        <div
          className={`h-full ${db > -6 ? "bg-red-500" : db > -18 ? "bg-amber-400" : "bg-emerald-500"}`}
          style={{ width: `${pct}%`, transition: "width 90ms linear" }}
        />
      </div>
    </div>
  );
}

function EngineBadge() {
  const { info, profile, status, lastLatencyMs, rebenchmark, running, benchProgress } = useDashboard();
  const model = info?.model ?? profile?.model ?? "—";
  const backend = info?.backend ?? (profile ? (profile.useGpu ? "gpu" : "cpu") : "—");
  const title = profile
    ? `${profile.specs.os}/${profile.specs.arch} · ${profile.specs.totalRamGb.toFixed(0)} GB · ${profile.specs.logicalCores} cores\n` +
      profile.benchmarks.map((b) => `${b.model}/${b.backend}: ${b.ok ? `${b.inferMs.toFixed(0)} ms` : b.error}`).join("\n")
    : "Not benchmarked yet — runs automatically on first start";
  return (
    <div className="flex items-center gap-2 text-xs text-zinc-400" title={title}>
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
