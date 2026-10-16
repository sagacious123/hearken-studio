import { useEffect, useState } from "react";
import { useT } from "../../lib/i18n";
import { api, events, type LibraryItem } from "../../lib/ipc";
import { useDashboard } from "../store";
import { useDismiss } from "../useDismiss";

/**
 * More Bibles: Yoruba, Igbo, Hausa, Twi, Ewe, Swahili, French, Spanish, Portuguese and German,
 * downloaded once from eBible.org and used offline afterwards, for the screen, side by side,
 * and on congregation phones.
 */
export function BibleLibrary({
  open: openProp,
  onOpenChange,
  showTrigger = true,
}: {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  showTrigger?: boolean;
} = {}) {
  const t = useT();
  const [openState, setOpenState] = useState(false);
  const open = openProp ?? openState;
  const setOpen = (o: boolean) =>
    onOpenChange ? onOpenChange(o) : setOpenState(o);
  const [items, setItems] = useState<LibraryItem[]>([]);
  const [busy, setBusy] = useState<Record<string, number>>({});
  const [error, setError] = useState<string | null>(null);
  // Open towards the side with room: the toolbar wraps, so the button can be anywhere.
  const [alignRight, setAlignRight] = useState(!showTrigger);
  const ref = useDismiss<HTMLDivElement>(open, () => setOpen(false));

  useEffect(() => {
    if (!open) return;
    api
      .listLibrary()
      .then(setItems)
      .catch((e) => setError(String(e)));
  }, [open]);

  useEffect(() => {
    const off = events.libraryProgress((p) =>
      setBusy((b) => (p.code in b ? { ...b, [p.code]: p.fraction } : b)),
    );
    return () => void off.then((f) => f());
  }, []);

  const refresh = async (
    translations: Awaited<ReturnType<typeof api.listTranslations>>,
  ) => {
    useDashboard.setState({ translations, settings: await api.getSettings() });
    setItems(await api.listLibrary());
  };

  const run = async (
    code: string,
    action: (code: string) => ReturnType<typeof api.listTranslations>,
  ) => {
    setError(null);
    setBusy((b) => ({ ...b, [code]: 0 }));
    try {
      await refresh(await action(code));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(({ [code]: _, ...rest }) => rest);
    }
  };

  return (
    <div ref={ref} className="relative">
      {showTrigger && (
        <button
          onClick={(e) => {
            const r = e.currentTarget.getBoundingClientRect();
            setAlignRight(r.left + r.width / 2 > window.innerWidth / 2);
            setOpen(!open);
          }}
          className="h-9 rounded-lg px-2 text-sm font-medium hover:bg-white/[.06] min-[1300px]:px-3 text-zinc-300"
          title={t("Download Bibles in other languages")}
        >
          {t("Bibles")}
        </button>
      )}
      {open && (
        <div
          className={`absolute ${alignRight ? "right-0" : "left-0"} top-full z-20 mt-2 max-h-[calc(100dvh-7rem)] w-[26rem] max-w-[calc(100vw-2rem)] space-y-2 overflow-y-auto overscroll-contain rounded-xl bg-zinc-900 p-4 text-sm shadow-2xl ring-1 ring-zinc-700 backdrop-blur-xl`}
        >
          <div>
            <h3 className="font-semibold">{t("Bible library")}</h3>
            <p className="text-xs text-zinc-400">
              {t(
                "Download once (about 4–5 MB each, from eBible.org), then use offline: on screen, side by side, and on congregation phones.",
              )}
            </p>
          </div>
          {error && <p className="text-xs text-red-400">{error}</p>}
          <ul className="divide-y divide-zinc-800">
            {items.map((it) => {
              const progress = busy[it.code];
              return (
                <li key={it.code} className="flex items-start gap-3 py-2">
                  <div className="min-w-0 flex-1">
                    <div className="flex items-baseline gap-2">
                      <span className="font-medium">{t(it.language)}</span>
                      <span className="font-mono text-xs text-zinc-500">
                        {it.code}
                      </span>
                    </div>
                    <div
                      className="truncate text-xs text-zinc-400"
                      title={it.name}
                    >
                      {it.name}
                    </div>
                    <div className="text-[11px] leading-snug text-zinc-500">
                      {it.notice}
                    </div>
                  </div>
                  {progress !== undefined ? (
                    <span className="w-20 flex-none pt-1 text-right text-xs tabular-nums text-sky-300">
                      {it.installed
                        ? t("Removing…")
                        : progress >= 0.99
                          ? t("Installing…")
                          : t("{n}%", { n: Math.round(progress * 100) })}
                    </span>
                  ) : it.installed ? (
                    <button
                      onClick={() => void run(it.code, api.removeTranslation)}
                      className="flex-none rounded bg-zinc-800 px-2 py-1 text-xs text-zinc-300 hover:bg-zinc-700"
                    >
                      {t("Remove")}
                    </button>
                  ) : (
                    <button
                      onClick={() => void run(it.code, api.installTranslation)}
                      className="flex-none rounded bg-sky-700 px-2 py-1 text-xs font-medium hover:bg-sky-600"
                    >
                      {t("Download")}
                    </button>
                  )}
                </li>
              );
            })}
          </ul>
          <p className="text-[11px] text-zinc-500">
            {t(
              "Choose a downloaded Bible as the translation or side by side; phones can pick any of them. To listen in another language, choose it under Settings → Language.",
            )}
          </p>
        </div>
      )}
    </div>
  );
}
