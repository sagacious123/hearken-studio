import { useEffect, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import {
  api,
  events,
  type MeaningStatus,
  type ModelStatus,
} from "../../lib/ipc";
import { useDashboard } from "../store";
import { useDismiss } from "../useDismiss";

type Progress = Record<
  string,
  { received: number; total: number; phase: "download" | "testing" }
>;

/**
 * Transcription accuracy: a fast model keeps live updates quick, and a larger model
 * re-checks every finished sentence (the "accuracy pass"). Larger models are downloaded here
 * rather than bundled. Also the vocabulary list of names and words to listen for.
 */
/** Opened from Settings (controlled); its own button shows only while there is news: a download or a warning. */
export function AccuracyButton({
  open: openProp,
  onOpenChange,
  showTrigger = true,
}: {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  showTrigger?: boolean;
} = {}) {
  const { settings, saveSettings, running, info, profile } = useDashboard(
    useShallow((s) => ({
      settings: s.settings,
      saveSettings: s.saveSettings,
      running: s.running,
      info: s.info,
      profile: s.profile,
    })),
  );
  const [openState, setOpenState] = useState(false);
  const open = openProp ?? openState;
  const setOpen = (o: boolean) =>
    onOpenChange ? onOpenChange(o) : setOpenState(o);
  const rootRef = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  const [models, setModels] = useState<ModelStatus[]>([]);
  const [progress, setProgress] = useState<Progress>({});
  const [error, setError] = useState<string | null>(null);
  const [vocab, setVocab] = useState(settings?.vocabulary ?? "");

  useEffect(() => setVocab(settings?.vocabulary ?? ""), [settings?.vocabulary]);
  useEffect(() => {
    api
      .listModels()
      .then(setModels)
      .catch((e) => setError(String(e)));
    // Mounted with the top bar, so downloads keep reporting while the panel is closed.
    const unlisten = events.modelProgress((p) =>
      setProgress((prev) => ({
        ...prev,
        [p.id]: { received: p.received, total: p.total, phase: p.phase },
      })),
    );
    return () => void unlisten.then((f) => f());
  }, []);

  if (!settings) return null;
  const choice = settings.accuracyModel;
  const installed = (id: string) =>
    models.some((m) => m.id === id && m.installed);
  const known = (id: string) => models.find((m) => m.id === id)?.usable;
  const gpu = settings.gpuOverride ?? profile?.useGpu ?? false;
  // Mirrors the Rust choice: the most accurate model proven to keep up here, else an untested
  // one (timed on first use; GPU machines try Turbo first, CPU-only Small).
  const autoPick =
    ["large-v3-turbo", "small.en"].find(
      (id) => installed(id) && known(id) === true,
    ) ??
    (gpu
      ? ["large-v3-turbo", "small.en"]
      : ["small.en", "large-v3-turbo"]
    ).find((id) => installed(id) && known(id) == null);
  const effective =
    choice === "off"
      ? null
      : choice === "auto"
        ? autoPick
        : installed(choice)
          ? choice
          : null;
  const label = (id?: string | null) =>
    models.find((m) => m.id === id)?.label ?? id ?? "";
  const downloading = Object.keys(progress).filter((id) => !installed(id));
  // A model pinned with "Always" that its speed test says can't keep up on this computer.
  const tooSlow =
    choice !== "auto" &&
    choice !== "off" &&
    installed(choice) &&
    known(choice) === false;

  const download = async (id: string) => {
    setError(null);
    setProgress((p) => ({
      ...p,
      [id]: { received: 0, total: 1, phase: "download" },
    }));
    try {
      setModels(await api.downloadModel(id));
    } catch (e) {
      setError(String(e));
    } finally {
      setProgress(({ [id]: _, ...rest }) => rest);
    }
  };

  return (
    <div ref={rootRef} className="relative">
      {(showTrigger || tooSlow || downloading.length > 0) && (
        <button
          onClick={() => setOpen(!open)}
          className={`h-9 rounded-lg px-2 text-sm font-medium hover:bg-white/[.06] min-[1300px]:px-3 ${open ? "bg-white/[.06] text-zinc-100" : "text-zinc-300"}`}
          title="Transcription accuracy and vocabulary"
        >
          Accuracy{downloading.length ? " ↓" : ""}
          {tooSlow && <span className="ml-1 text-amber-300">⚠</span>}
        </button>
      )}
      {open && (
        <div className="absolute right-0 top-full z-20 mt-2 max-h-[calc(100dvh-7rem)] w-[26rem] space-y-4 overflow-y-auto overscroll-contain rounded-xl bg-zinc-900 p-4 text-sm shadow-2xl ring-1 ring-zinc-700 backdrop-blur-xl">
          <div>
            <h3 className="font-semibold">Transcription accuracy</h3>
            <p className="mt-1 text-xs text-zinc-400">
              A fast model keeps live updates quick. A larger model then
              re-checks each finished sentence (lines marked
              <span className="text-emerald-500"> ✓</span>), giving better
              words, names and accents in the transcript, quotes and archive.
            </p>
            <p className="mt-2 text-xs text-zinc-300">
              Now: live <b>{info?.model ?? profile?.model ?? "base.en"}</b>
              {running ? (
                info?.refineModel ? (
                  <>
                    {" "}
                    · accuracy pass <b>{label(info.refineModel)}</b>
                  </>
                ) : (
                  " · no accuracy pass"
                )
              ) : effective ? (
                <>
                  {" "}
                  · accuracy pass <b>{label(effective)}</b>
                </>
              ) : (
                " · accuracy pass off"
              )}
            </p>
          </div>

          {!gpu && (
            <p className="rounded-md bg-zinc-950 p-2 text-xs text-zinc-400 ring-1 ring-zinc-800">
              This computer has no graphics chip the models can use, so the
              accuracy pass runs gently in the background (one low-priority
              thread) to keep live detection fast. It still adds roughly 0.1–0.2
              s to detections; choose <b>Off</b> for the fastest possible
              detection.
            </p>
          )}

          <fieldset className="space-y-1.5">
            <legend className="mb-1 text-xs font-semibold text-zinc-400">
              Accuracy pass
            </legend>
            <Choice
              value="auto"
              current={choice}
              onPick={(v) => saveSettings({ accuracyModel: v })}
            >
              Automatic{" "}
              {autoPick ? (
                <span className="text-zinc-500">
                  (uses {label(autoPick)} on this computer)
                </span>
              ) : models.some((m) => m.accuracy && m.installed) ? (
                <span className="text-amber-400">
                  (installed models are too slow here, so it's off)
                </span>
              ) : (
                <span className="text-amber-400">(download a model below)</span>
              )}
            </Choice>
            {models
              .filter((m) => m.accuracy)
              .map((m) => (
                <Choice
                  key={m.id}
                  value={m.id}
                  current={choice}
                  onPick={(v) => saveSettings({ accuracyModel: v })}
                  disabled={!m.installed}
                >
                  Always {m.label}
                </Choice>
              ))}
            <Choice
              value="off"
              current={choice}
              onPick={(v) => saveSettings({ accuracyModel: v })}
            >
              Off{" "}
              <span className="text-zinc-500">(quick transcription only)</span>
            </Choice>
          </fieldset>
          {tooSlow && (
            <div className="space-y-2 rounded-md bg-amber-950/60 p-3 text-xs text-amber-200 ring-1 ring-amber-800">
              <p>
                <b>{label(choice)}</b> is too slow on this computer while
                listening, so references it finds would come too late. Sentences
                it can't reach in time keep the quick text, and its late finds
                aren't queued.
              </p>
              <button
                onClick={() => void saveSettings({ accuracyModel: "auto" })}
                className="rounded bg-amber-600 px-2 py-1 font-medium text-white hover:bg-amber-500"
              >
                Use Automatic
              </button>
            </div>
          )}

          <div className="space-y-2">
            <h4 className="text-xs font-semibold text-zinc-400">Models</h4>
            {models
              .filter((m) => m.accuracy)
              .map((m) => {
                const p = progress[m.id];
                return (
                  <div
                    key={m.id}
                    className="rounded-md bg-zinc-950 p-2.5 ring-1 ring-zinc-800"
                  >
                    <div className="flex items-center gap-2">
                      <span className="font-medium">{m.label}</span>
                      <span className="text-xs text-zinc-500">
                        {m.sizeMb} MB
                      </span>
                      <span className="ml-auto">
                        {m.installed ? (
                          <button
                            disabled={running}
                            title={running ? "Stop listening first" : undefined}
                            onClick={() => {
                              if (
                                !window.confirm(
                                  `Remove ${m.label}? You can download it again later.`,
                                )
                              )
                                return;
                              api
                                .deleteModel(m.id)
                                .then(setModels)
                                .catch((e) => setError(String(e)));
                            }}
                            className="text-xs text-zinc-500 hover:text-red-400 disabled:opacity-40"
                          >
                            ✓ Installed · Remove
                          </button>
                        ) : p ? (
                          <button
                            onClick={() => void api.cancelModelDownload(m.id)}
                            className="text-xs text-zinc-400 hover:text-zinc-100"
                          >
                            Cancel
                          </button>
                        ) : (
                          <button
                            onClick={() => void download(m.id)}
                            className="rounded bg-emerald-700 px-2 py-0.5 text-xs font-medium hover:bg-emerald-600"
                          >
                            Download
                          </button>
                        )}
                      </span>
                    </div>
                    <p className="mt-1 text-xs text-zinc-500">{m.note}</p>
                    {m.installed && (
                      <p className="mt-1 flex items-center gap-2 text-xs">
                        <span
                          className={
                            m.usable === false
                              ? "text-amber-400"
                              : m.usable
                                ? "text-emerald-400"
                                : "text-zinc-500"
                          }
                        >
                          {m.speed
                            ? `On this computer: ${m.speed}`
                            : "Speed not tested yet (tested the first time it's used)"}
                        </span>
                        <button
                          disabled={running || !!p}
                          title={
                            running
                              ? "Stop listening first"
                              : "Close other heavy apps for a fair result"
                          }
                          onClick={() => {
                            setProgress((prev) => ({
                              ...prev,
                              [m.id]: {
                                received: 1,
                                total: 1,
                                phase: "testing",
                              },
                            }));
                            api
                              .testModelSpeed(m.id)
                              .then(setModels)
                              .catch((e) => setError(String(e)))
                              .finally(() =>
                                setProgress(({ [m.id]: _, ...rest }) => rest),
                              );
                          }}
                          className="ml-auto text-zinc-500 underline-offset-2 hover:text-zinc-200 hover:underline disabled:opacity-40"
                        >
                          {p?.phase === "testing" ? "Testing…" : "Test speed"}
                        </button>
                      </p>
                    )}
                    {p && p.phase === "testing" && !m.installed && (
                      <p className="mt-2 text-xs text-zinc-400">
                        Testing speed on this computer…
                      </p>
                    )}
                    {p && p.phase === "download" && !m.installed && (
                      <div className="mt-2 flex items-center gap-2">
                        <div className="h-1.5 flex-1 overflow-hidden rounded bg-zinc-800">
                          <div
                            className="h-full bg-emerald-500 transition-[width]"
                            style={{
                              width: `${Math.round((p.received / Math.max(p.total, 1)) * 100)}%`,
                            }}
                          />
                        </div>
                        <span className="w-20 text-right font-mono text-[11px] text-zinc-400">
                          {Math.round(p.received / 1e6)} /{" "}
                          {Math.round(p.total / 1e6)} MB
                        </span>
                      </div>
                    )}
                  </div>
                );
              })}
            {error && <p className="text-xs text-red-400">{error}</p>}
          </div>

          <MeaningSection />

          <label className="block space-y-1.5">
            <span className="text-xs font-semibold text-zinc-400">
              Names and words to listen for
            </span>
            <textarea
              value={vocab}
              onChange={(e) => setVocab(e.target.value)}
              onBlur={() =>
                vocab !== settings.vocabulary &&
                void saveSettings({ vocabulary: vocab })
              }
              rows={3}
              placeholder={
                "Pastor Johnson, Grace Chapel, Shiloh,\nHoly Ghost, Ebenezer"
              }
              className="w-full resize-y rounded-md bg-zinc-950 p-2 text-sm outline-none ring-1 ring-zinc-800 focus:ring-emerald-600"
            />
            <span className="block text-xs text-zinc-500">
              Pastors' and members' names, the church, places, and phrases that
              often come out wrong. Comma or one per line. Bible book names and
              today's sermon plan are added automatically.
            </span>
          </label>

          <p className="text-xs text-zinc-500">
            {running
              ? "Model changes apply the next time you start listening. Vocabulary applies right away."
              : "Changes apply when you start listening."}
          </p>
        </div>
      )}
    </div>
  );
}

/** Search by meaning: verses said in other words, live and in the search box. */
function MeaningSection() {
  const enabled = useDashboard((s) => s.settings?.meaningDetection ?? true);
  const saveSettings = useDashboard((s) => s.saveSettings);
  const [status, setStatus] = useState<MeaningStatus | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api
      .meaningStatus()
      .then(setStatus)
      .catch((e) => setError(String(e)));
    const off = events.meaningProgress((p) =>
      setProgress(p.received / Math.max(p.total, 1)),
    );
    return () => void off.then((f) => f());
  }, []);

  const download = async () => {
    setError(null);
    setProgress(0);
    try {
      setStatus(await api.downloadMeaningModel());
    } catch (e) {
      setError(String(e));
    } finally {
      setProgress(null);
    }
  };

  return (
    <div className="space-y-1.5 rounded-md bg-zinc-950 p-3 ring-1 ring-zinc-800">
      <div className="flex items-center gap-2">
        <span className="text-xs font-semibold text-zinc-300">
          Verses said in other words
        </span>
        {status?.installed && (
          <label className="ml-auto flex items-center gap-1.5 text-xs text-zinc-400">
            <input
              type="checkbox"
              checked={enabled}
              onChange={(e) =>
                void saveSettings({ meaningDetection: e.target.checked })
              }
              className="accent-emerald-500"
            />
            Suggest live
          </label>
        )}
      </div>
      <p className="text-xs text-zinc-500">
        Finds the verse when someone paraphrases it (“don't worry about
        tomorrow” → Matthew 6:34), live as a suggestion and under “By meaning”
        in the search box. Works offline once downloaded.
      </p>
      {error && <p className="text-xs text-red-400">{error}</p>}
      {progress !== null ? (
        <div className="flex items-center gap-2">
          <div className="h-1.5 flex-1 overflow-hidden rounded bg-zinc-800">
            <div
              className="h-full bg-emerald-500 transition-[width]"
              style={{ width: `${Math.round(progress * 100)}%` }}
            />
          </div>
          <span className="w-16 text-right font-mono text-[11px] text-zinc-400">
            {progress >= 1 ? "Loading…" : `${Math.round(progress * 100)}%`}
          </span>
        </div>
      ) : status && !status.installed ? (
        <button
          onClick={() => void download()}
          className="rounded bg-emerald-700 px-2 py-1 text-xs font-medium hover:bg-emerald-600"
        >
          Download ({Math.round(status.downloadMb)} MB)
        </button>
      ) : status?.installed ? (
        <p className="flex items-center gap-2 text-xs text-zinc-400">
          {status.ready ? "Ready." : "Loading…"}
          <button
            onClick={() =>
              void api
                .removeMeaningModel()
                .then(setStatus)
                .catch((e) => setError(String(e)))
            }
            className="ml-auto text-zinc-500 underline-offset-2 hover:text-zinc-200 hover:underline"
          >
            Remove
          </button>
        </p>
      ) : null}
    </div>
  );
}

function Choice({
  value,
  current,
  onPick,
  disabled,
  children,
}: {
  value: string;
  current: string;
  onPick: (v: string) => void;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  return (
    <label
      className={`flex items-center gap-2 ${disabled ? "opacity-40" : "cursor-pointer"}`}
    >
      <input
        type="radio"
        name="accuracy-model"
        checked={current === value}
        disabled={disabled}
        onChange={() => onPick(value)}
        className="accent-emerald-500"
      />
      <span>{children}</span>
    </label>
  );
}
