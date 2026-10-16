import type { Update } from "@tauri-apps/plugin-updater";
import { useEffect, useState } from "react";
import { useT } from "../../lib/i18n";
import { checkForUpdate, installUpdate } from "../../lib/updater";
import { useDashboard } from "../store";

/**
 * Updates are checked in the background but *never* installed automatically: restarting the
 * app mid-sermon would drop the projector. The operator installs when listening is stopped.
 */
export function UpdateBanner() {
  const running = useDashboard((s) => s.running);
  const [update, setUpdate] = useState<Update | null>(null);
  const [progress, setProgress] = useState<number | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const t = useT();

  useEffect(() => {
    const id = setTimeout(() => checkForUpdate().then(setUpdate), 5_000);
    return () => clearTimeout(id);
  }, []);

  if (!update) return null;

  const installing = progress !== undefined;
  const [before, after] = t("Version {version} is available.").split("{version}");
  return (
    <div className="flex items-center gap-3 bg-sky-950 px-4 py-1.5 text-sm text-sky-100">
      <span>
        {before}
        <b>{update.version}</b>
        {after}
      </span>
      {error && <span className="text-red-300">{error}</span>}
      <button
        disabled={running || installing}
        title={running ? t("Stop listening before updating") : undefined}
        onClick={() =>
          installUpdate(update, setProgress).catch((e) => {
            setError(String(e));
            setProgress(undefined);
          })
        }
        className="ml-auto rounded bg-sky-600 px-3 py-0.5 font-medium hover:bg-sky-500 disabled:opacity-40"
      >
        {installing ? (progress === null ? t("Downloading…") : t("Installing {n}%", { n: Math.round((progress ?? 0) * 100) })) : t("Install & restart")}
      </button>
      {!installing && (
        <button onClick={() => setUpdate(null)} className="text-sky-300 hover:text-white">
          {t("Later")}
        </button>
      )}
    </div>
  );
}
