import { useEffect, useMemo, useRef } from "react";
import { locale, useT, type T } from "../../lib/i18n";
import { useDashboard } from "../store";

const clock = (ms: number) =>
  new Date(ms).toLocaleTimeString(locale(), { hour: "2-digit", minute: "2-digit" });

/** Voice-command labels arrive in English ("Next verse", "Verses 6–8"); shown translated. */
function voiceLabel(t: T, label: string): string {
  const range = /^Verses (\d+)–(\d+)$/.exec(label);
  if (range) return t("Verses {s}–{e}", { s: range[1], e: range[2] });
  const one = /^Verse (\d+)$/.exec(label);
  if (one) return t("Verse {n}", { n: one[1] });
  return t(label);
}

export function TranscriptPanel() {
  const finals = useDashboard((s) => s.finals);
  const partial = useDashboard((s) => s.partial);
  const queue = useDashboard((s) => s.queue);
  const voice = useDashboard((s) => s.voiceCommands);
  const endRef = useRef<HTMLDivElement>(null);
  const t = useT();

  // What each sentence led to, shown beside it ("→ John 3:16").
  const found = useMemo(() => {
    const m = new Map<number, string[]>();
    for (const d of queue) {
      if (d.tentative) continue;
      const list = m.get(d.utteranceId) ?? [];
      if (!list.includes(d.slide.label)) list.push(d.slide.label);
      m.set(d.utteranceId, list);
    }
    return m;
  }, [queue]);

  // Braces matter: newer engines return a Promise from scrollIntoView, and an effect that
  // returns a non-function makes React throw (and blank the dashboard) on the next update.
  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [finals, partial]);

  return (
    <div className="min-h-0 flex-1 select-text cursor-auto space-y-2.5 overflow-y-auto px-4 pb-3 text-[14px] font-medium leading-relaxed">
      {finals.length === 0 && !partial && (
        <p className="text-zinc-500">{t("Waiting for speech…")}</p>
      )}
      {finals.map((l) => (
        <div
          key={l.utteranceId}
          className="grid grid-cols-[2.75rem_minmax(0,1fr)] gap-2"
          title={`${l.refined ? t("Checked by the accuracy pass") : t("Quick transcription")} · ${t("confidence {n}%", { n: (l.confidence * 100).toFixed(0) })} · ${(l.inferenceMs / 1000).toFixed(1)} s`}
        >
          <time className="pt-0.5 text-xs tabular-nums text-zinc-500">
            {clock(l.at)}
          </time>
          <p>
            <span
              className={
                l.sung
                  ? "italic text-zinc-400"
                  : l.confidence < 0.6
                    ? "text-zinc-400"
                    : "text-zinc-200"
              }
            >
              {l.text}
            </span>
            {l.sung && (
              <span
                className="ml-2 inline-block rounded bg-zinc-800 px-1.5 align-middle text-xs text-zinc-400"
                title={t("Heard sung, not spoken: not treated as a quote or a command, and left out of sermon notes")}
              >
                {t("♪ sung")}
              </span>
            )}
            {l.refined && (
              <span
                className="ml-1.5 align-middle text-[10px] text-zinc-500"
                aria-label={t("checked")}
              >
                ✓
              </span>
            )}
            {voice[l.utteranceId] && (
              <span
                className={`ml-2 inline-block rounded px-1.5 align-middle text-xs font-medium ${voice[l.utteranceId].moved ? "bg-amber-500/15 text-amber-200" : "bg-zinc-800 text-zinc-400"}`}
                title={
                  voice[l.utteranceId].moved
                    ? t("Voice command: now showing {now}", { now: voice[l.utteranceId].now ?? "" })
                    : t("Voice command: already at the end of the chapter")
                }
              >
                ↪ {voiceLabel(t, voice[l.utteranceId].label)}
                {voice[l.utteranceId].moved && voice[l.utteranceId].now
                  ? ` · ${voice[l.utteranceId].now}`
                  : ""}
              </span>
            )}
            {found.get(l.utteranceId)?.map((label) => (
              <span
                key={label}
                className="ml-2 inline-block rounded bg-amber-500/15 px-1.5 align-middle text-xs font-medium text-amber-200"
              >
                → {label}
              </span>
            ))}
          </p>
        </div>
      ))}
      {partial && (
        <div className="grid grid-cols-[2.75rem_minmax(0,1fr)] gap-2">
          <span />
          <p className="text-zinc-100">
            {partial.text}
            <span className="ml-0.5 inline-block h-4 w-0.5 translate-y-0.5 bg-amber-500" />
          </p>
        </div>
      )}
      <div ref={endRef} />
    </div>
  );
}

/** Shown in the transcript header while the room is singing. */
export function SingingBadge() {
  const singing = useDashboard((s) => s.singingSince !== null);
  const t = useT();
  if (!singing)
    return (
      <span className="font-ui text-[11px] font-medium normal-case tracking-normal text-zinc-500">
        {t("Checked lines marked ✓")}
      </span>
    );
  return (
    <span
      className="flex items-center gap-1.5 rounded-full bg-amber-500/10 px-2 py-0.5 font-ui text-[11px] font-medium normal-case tracking-normal text-amber-200"
      title={t("The room is singing: sung Bible phrases aren't suggested as quotes, and voice commands are ignored")}
    >
      <span className="animate-pulse">♪</span> {t("Singing")}
    </span>
  );
}

export function PanelHeader({
  title,
  children,
}: {
  title: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="flex min-h-11 flex-none flex-wrap items-center justify-between gap-x-3 gap-y-1 px-4 py-2 text-[12px] font-bold uppercase tracking-[0.14em] text-amber-200">
      {title}
      {children}
    </div>
  );
}
