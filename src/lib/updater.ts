import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

/**
 * Polls `latest.json` from GitHub Releases. Fails quietly: venues are often offline and an
 * update check must never interrupt a service.
 */
export async function checkForUpdate(): Promise<Update | null> {
  // `tauri dev` has no installed build to replace, and the endpoint may still be a placeholder.
  if (import.meta.env.DEV) return null;
  try {
    return await check({ timeout: 10_000 });
  } catch (e) {
    console.warn("update check failed", e);
    return null;
  }
}

/** Downloads, verifies the minisign signature, installs and relaunches. */
export async function installUpdate(update: Update, onProgress: (fraction: number | null) => void) {
  let total = 0;
  let received = 0;
  await update.downloadAndInstall((event) => {
    switch (event.event) {
      case "Started":
        total = event.data.contentLength ?? 0;
        onProgress(total ? 0 : null);
        break;
      case "Progress":
        received += event.data.chunkLength;
        onProgress(total ? received / total : null);
        break;
      case "Finished":
        onProgress(1);
        break;
    }
  });
  await relaunch();
}
