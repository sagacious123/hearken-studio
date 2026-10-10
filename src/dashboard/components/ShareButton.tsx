import { useEffect, useState } from "react";
import { api, type ShareInfo } from "../../lib/ipc";
import { useDashboard } from "../store";

/**
 * Local-network sharing: congregation phones (QR code), a livestream overlay for OBS/vMix,
 * and a phone remote for the operator. Works without internet on the church Wi-Fi.
 */
export function ShareButton() {
  const enabled = useDashboard((s) => s.settings?.shareEnabled ?? false);
  const qrLive = useDashboard((s) => !!s.live?.qr);
  const saveSettings = useDashboard((s) => s.saveSettings);
  const [open, setOpen] = useState(false);
  const [info, setInfo] = useState<ShareInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (open) api.getShareInfo().then(setInfo).catch((e) => setError(String(e)));
  }, [open, enabled]);

  const toggle = async () => {
    setError(null);
    try {
      await saveSettings({ shareEnabled: !enabled });
    } catch (e) {
      setError(String(e));
    }
    setInfo(await api.getShareInfo());
  };

  return (
    <div className="relative">
      <button
        onClick={() => setOpen(!open)}
        className={`rounded px-3 py-1 font-medium ${info?.enabled || enabled ? "bg-sky-700 hover:bg-sky-600" : "bg-zinc-700 hover:bg-zinc-600"}`}
      >
        Share{enabled ? " ●" : ""}
      </button>
      {open && (
        <div className="absolute right-0 top-full z-20 mt-2 w-[22rem] space-y-3 rounded-lg bg-zinc-900 p-4 text-sm shadow-2xl ring-1 ring-zinc-700">
          <label className="flex items-center justify-between gap-3">
            <span>
              <span className="font-semibold">Share on the local network</span>
              <span className="block text-xs text-zinc-400">Phones, livestream overlay and remote. No internet needed.</span>
            </span>
            <input type="checkbox" checked={enabled} onChange={toggle} className="h-4 w-4 accent-sky-500" />
          </label>
          {(error || info?.error) && <p className="text-xs text-red-400">{error ?? info?.error}</p>}

          {info?.enabled && (
            <>
              <div className="flex gap-3">
                <div className="h-28 w-28 flex-none overflow-hidden rounded bg-white p-1 [&_svg]:h-full [&_svg]:w-full" dangerouslySetInnerHTML={{ __html: info.qrSvg }} />
                <div className="min-w-0 space-y-1.5">
                  <p className="text-xs text-zinc-400">Congregation: scan to follow the verses on their phone, in any translation and text size.</p>
                  <button
                    onClick={() => void api.showQr(!qrLive).catch((e) => setError(String(e)))}
                    className={`rounded px-2 py-1 text-xs font-medium ${qrLive ? "bg-amber-600 hover:bg-amber-500" : "bg-zinc-800 hover:bg-zinc-700"}`}
                  >
                    {qrLive ? "Hide QR on screen" : "Show QR on screen"}
                  </button>
                </div>
              </div>
              <Link label="Phones" url={info.phoneUrl} />
              <Link label="Livestream overlay" url={info.overlayUrl} hint="OBS/vMix: add a Browser source, 1920×1080. Add ?pos=top to move it up." />
              <Link label="Remote" url={info.remoteUrl} hint={`Open on your phone and enter PIN ${info.pin}.`} />
              <p className="text-xs text-zinc-500">Devices must be on the same Wi-Fi. If they can't connect, allow Hearken Studio in the computer's firewall.</p>
            </>
          )}
        </div>
      )}
    </div>
  );
}

function Link({ label, url, hint }: { label: string; url: string; hint?: string }) {
  const [copied, setCopied] = useState(false);
  return (
    <div>
      <div className="flex items-center gap-2">
        <span className="w-20 flex-none text-xs text-zinc-400">{label}</span>
        <code className="min-w-0 flex-1 truncate rounded bg-zinc-950 px-1.5 py-0.5 text-xs text-sky-300">{url}</code>
        <button
          onClick={() => {
            void navigator.clipboard.writeText(url).then(() => {
              setCopied(true);
              setTimeout(() => setCopied(false), 1200);
            });
          }}
          className="flex-none rounded bg-zinc-800 px-1.5 py-0.5 text-xs hover:bg-zinc-700"
        >
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      {hint && <p className="ml-[5.5rem] mt-0.5 text-[11px] text-zinc-500">{hint}</p>}
    </div>
  );
}
