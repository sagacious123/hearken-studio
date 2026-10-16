import { Fragment, useEffect, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import {
  api,
  events,
  type MeaningStatus,
  type ModelStatus,
} from "../../lib/ipc";
import { locale, useT, type T } from "../../lib/i18n";
import { useDashboard } from "../store";
import { useDismiss } from "../useDismiss";

/** A translated sentence with elements (bold, colour) where its {placeholders} stand. */
function rich(text: string, slots: Record<string, React.ReactNode>) {
  return text
    .split(/\{(\w+)\}/)
    .map((part, i) => (i % 2 ? <Fragment key={i}>{slots[part]}</Fragment> : part));
}

/** The backend's speed note ("high quality, 3.1 s per 10 s of speech") in the UI language. */
function speedText(t: T, speed: string) {
  const m = /^(.+), ([\d.]+) s per 10 s of speech$/.exec(speed);
  if (!m) return t(speed);
  const seconds = Number(m[2]).toLocaleString(locale(), { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  return t("{quality}, {seconds} s per 10 s of speech", { quality: t(m[1]), seconds });
}

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
  const t = useT();
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
  // Sermons in other languages need the multilingual Small; English ones the English Small.
  const multilingual = settings.languages.some((c) => c !== "en");
  const small = multilingual ? "small" : "small.en";
  const forLanguages = (m: ModelStatus) => m.accuracy && m.id !== (multilingual ? "small.en" : "small");
  // Mirrors the Rust choice: the most accurate model proven to keep up here, else an untested
  // one (timed on first use; GPU machines try Turbo first, CPU-only Small).
  const autoPick =
    ["large-v3-turbo", small].find(
      (id) => installed(id) && known(id) === true,
    ) ??
    (gpu
      ? ["large-v3-turbo", small]
      : [small, "large-v3-turbo"]
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
    t(models.find((m) => m.id === id)?.label ?? id ?? "");
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
          title={t("Transcription accuracy and vocabulary")}
        >
          {t("Accuracy")}{downloading.length ? " ↓" : ""}
          {tooSlow && <span className="ml-1 text-amber-300">⚠</span>}
        </button>
      )}
      {open && (
        <div className="absolute right-0 top-full z-20 mt-2 max-h-[calc(100dvh-7rem)] w-[26rem] space-y-4 overflow-y-auto overscroll-contain rounded-xl bg-zinc-900 p-4 text-sm shadow-2xl ring-1 ring-zinc-700 backdrop-blur-xl">
          <div>
            <h3 className="font-semibold">{t("Transcription accuracy")}</h3>
            <p className="mt-1 text-xs text-zinc-400">
              {rich(
                t(
                  "A fast model keeps live updates quick. A larger model then re-checks each finished sentence (lines marked {check}), giving better words, names and accents in the transcript, quotes and archive.",
                ),
                { check: <span className="text-emerald-500">✓</span> },
              )}
            </p>
            <p className="mt-2 text-xs text-zinc-300">
              {t("Now: live")} <b>{info?.model ?? profile?.model ?? "base.en"}</b>
              {running ? (
                info?.refineModel ? (
                  <>
                    {" "}
                    · {t("accuracy pass")} <b>{label(info.refineModel)}</b>
                  </>
                ) : (
                  ` · ${t("no accuracy pass")}`
                )
              ) : effective ? (
                <>
                  {" "}
                  · {t("accuracy pass")} <b>{label(effective)}</b>
                </>
              ) : (
                ` · ${t("accuracy pass off")}`
              )}
            </p>
          </div>

          {!gpu && (
            <p className="rounded-md bg-zinc-950 p-2 text-xs text-zinc-400 ring-1 ring-zinc-800">
              {t(
                "This computer has no graphics chip the models can use, so the accuracy pass runs gently in the background (one low-priority thread) to keep live detection fast.",
              )}{" "}
              {rich(
                t(
                  "It still adds roughly 0.1–0.2 s to detections; choose {off} for the fastest possible detection.",
                ),
                { off: <b>{t("Off")}</b> },
              )}
            </p>
          )}

          <fieldset className="space-y-1.5">
            <legend className="mb-1 text-xs font-semibold text-zinc-400">
              {t("Accuracy pass")}
            </legend>
            <Choice
              value="auto"
              current={choice}
              onPick={(v) => saveSettings({ accuracyModel: v })}
            >
              {t("Automatic")}{" "}
              {autoPick ? (
                <span className="text-zinc-500">
                  {t("(uses {model} on this computer)", { model: label(autoPick) })}
                </span>
              ) : models.some((m) => forLanguages(m) && m.installed) ? (
                <span className="text-amber-400">
                  {t("(installed models are too slow here, so it's off)")}
                </span>
              ) : (
                <span className="text-amber-400">{t("(download a model below)")}</span>
              )}
            </Choice>
            {models
              .filter(forLanguages)
              .map((m) => (
                <Choice
                  key={m.id}
                  value={m.id}
                  current={choice}
                  onPick={(v) => saveSettings({ accuracyModel: v })}
                  disabled={!m.installed}
                >
                  {t("Always {model}", { model: t(m.label) })}
                </Choice>
              ))}
            <Choice
              value="off"
              current={choice}
              onPick={(v) => saveSettings({ accuracyModel: v })}
            >
              {t("Off")}{" "}
              <span className="text-zinc-500">{t("(quick transcription only)")}</span>
            </Choice>
          </fieldset>
          {tooSlow && (
            <div className="space-y-2 rounded-md bg-amber-950/60 p-3 text-xs text-amber-200 ring-1 ring-amber-800">
              <p>
                {rich(
                  t(
                    "{model} is too slow on this computer while listening, so references it finds would come too late. Sentences it can't reach in time keep the quick text, and its late finds aren't queued.",
                  ),
                  { model: <b>{label(choice)}</b> },
                )}
              </p>
              <button
                onClick={() => void saveSettings({ accuracyModel: "auto" })}
                className="rounded bg-amber-600 px-2 py-1 font-medium text-white hover:bg-amber-500"
              >
                {t("Use Automatic")}
              </button>
            </div>
          )}

          <div className="space-y-2">
            <h4 className="text-xs font-semibold text-zinc-400">{t("Models")}</h4>
            {models
              .filter(forLanguages)
              .map((m) => {
                const p = progress[m.id];
                return (
                  <div
                    key={m.id}
                    className="rounded-md bg-zinc-950 p-2.5 ring-1 ring-zinc-800"
                  >
                    <div className="flex items-center gap-2">
                      <span className="font-medium">{t(m.label)}</span>
                      <span className="text-xs text-zinc-500">
                        {t("{mb} MB", { mb: m.sizeMb })}
                      </span>
                      <span className="ml-auto">
                        {m.installed ? (
                          <button
                            disabled={running}
                            title={running ? t("Stop listening first") : undefined}
                            onClick={() => {
                              if (
                                !window.confirm(
                                  t("Remove {model}? You can download it again later.", { model: t(m.label) }),
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
                            {t("✓ Installed · Remove")}
                          </button>
                        ) : p ? (
                          <button
                            onClick={() => void api.cancelModelDownload(m.id)}
                            className="text-xs text-zinc-400 hover:text-zinc-100"
                          >
                            {t("Cancel")}
                          </button>
                        ) : (
                          <button
                            onClick={() => void download(m.id)}
                            className="rounded bg-emerald-700 px-2 py-0.5 text-xs font-medium hover:bg-emerald-600"
                          >
                            {t("Download")}
                          </button>
                        )}
                      </span>
                    </div>
                    <p className="mt-1 text-xs text-zinc-500">{t(m.note)}</p>
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
                            ? t("On this computer: {speed}", { speed: speedText(t, m.speed) })
                            : t("Speed not tested yet (tested the first time it's used)")}
                        </span>
                        <button
                          disabled={running || !!p}
                          title={
                            running
                              ? t("Stop listening first")
                              : t("Close other heavy apps for a fair result")
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
                          {p?.phase === "testing" ? t("Testing…") : t("Test speed")}
                        </button>
                      </p>
                    )}
                    {p && p.phase === "testing" && !m.installed && (
                      <p className="mt-2 text-xs text-zinc-400">
                        {t("Testing speed on this computer…")}
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
                          {t("{received} / {total} MB", {
                            received: Math.round(p.received / 1e6),
                            total: Math.round(p.total / 1e6),
                          })}
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
              {t("Names and words to listen for")}
            </span>
            <textarea
              value={vocab}
              onChange={(e) => setVocab(e.target.value)}
              onBlur={() =>
                vocab !== settings.vocabulary &&
                void saveSettings({ vocabulary: vocab })
              }
              rows={3}
              placeholder={t(
                "Pastor Johnson, Grace Chapel, Shiloh,\nHoly Ghost, Ebenezer",
              )}
              className="w-full resize-y rounded-md bg-zinc-950 p-2 text-sm outline-none ring-1 ring-zinc-800 focus:ring-emerald-600"
            />
            <span className="block text-xs text-zinc-500">
              {t(
                "Pastors' and members' names, the church, places, and phrases that often come out wrong. Comma or one per line. Bible book names and today's sermon plan are added automatically.",
              )}
            </span>
          </label>

          <p className="text-xs text-zinc-500">
            {running
              ? t("Model changes apply the next time you start listening. Vocabulary applies right away.")
              : t("Changes apply when you start listening.")}
          </p>
        </div>
      )}
    </div>
  );
}

/** Search by meaning: verses said in other words, live and in the search box. */
function MeaningSection() {
  const t = useT();
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
          {t("Verses said in other words")}
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
            {t("Suggest live")}
          </label>
        )}
      </div>
      <p className="text-xs text-zinc-500">
        {t(
          "Finds the verse when someone paraphrases it (“don't worry about tomorrow” → Matthew 6:34), live as a suggestion and under “By meaning” in the search box. Works offline once downloaded.",
        )}
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
            {progress >= 1 ? t("Loading…") : t("{n}%", { n: Math.round(progress * 100) })}
          </span>
        </div>
      ) : status && !status.installed ? (
        <button
          onClick={() => void download()}
          className="rounded bg-emerald-700 px-2 py-1 text-xs font-medium hover:bg-emerald-600"
        >
          {t("Download ({mb} MB)", { mb: Math.round(status.downloadMb) })}
        </button>
      ) : status?.installed ? (
        <p className="flex items-center gap-2 text-xs text-zinc-400">
          {status.ready ? t("Ready.") : t("Loading…")}
          <button
            onClick={() =>
              void api
                .removeMeaningModel()
                .then(setStatus)
                .catch((e) => setError(String(e)))
            }
            className="ml-auto text-zinc-500 underline-offset-2 hover:text-zinc-200 hover:underline"
          >
            {t("Remove")}
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
