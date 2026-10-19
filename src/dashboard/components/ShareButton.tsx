import { Fragment, useEffect, useState } from "react";
import { useT, type T } from "../../lib/i18n";
import { api, type ShareInfo } from "../../lib/ipc";
import { useDashboard } from "../store";
import { useDismiss } from "../useDismiss";

/**
 * Local-network sharing: congregation phones (QR code), a livestream overlay for OBS/vMix,
 * and a phone remote for the operator. Works without internet on the church Wi-Fi.
 */
/** Same rules as the Rust side: letters, digits and hyphens ("Grace Chapel" → "grace-chapel"). */
const cleanName = (raw: string) =>
  raw
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40)
    .replace(/-+$/, "") || "hearken";

const NAME_STATUS = {
  checking: {
    tone: "text-zinc-400",
    text: (t: T) => t("Checking the name on the network…"),
  },
  announced: {
    tone: "text-emerald-400",
    text: (t: T, url: string) =>
      t("✓ Phones on this Wi-Fi can open {address}", { address: url.replace("http://", "") }),
  },
  conflict: {
    tone: "text-amber-400",
    text: (t: T, url: string) =>
      t("Another device on this network already uses {address}. Choose a different name.", {
        address: url.replace("http://", ""),
      }),
  },
  failed: {
    tone: "text-amber-400",
    text: (t: T) =>
      t("Couldn't announce the name; phones can use the number address below."),
  },
} as const;

/** A translated sentence with elements (bold, colour) where its {placeholders} stand. */
function rich(text: string, slots: Record<string, React.ReactNode>) {
  return text
    .split(/\{(\w+)\}/)
    .map((part, i) => (i % 2 ? <Fragment key={i}>{slots[part]}</Fragment> : part));
}

export function ShareButton() {
  const t = useT();
  const enabled = useDashboard((s) => s.settings?.shareEnabled ?? false);
  const savedName = useDashboard((s) => s.settings?.shareName ?? "hearken");
  const qrLive = useDashboard((s) => !!s.live?.qr);
  const saveSettings = useDashboard((s) => s.saveSettings);
  const [open, setOpen] = useState(false);
  const rootRef = useDismiss<HTMLDivElement>(open, () => setOpen(false));
  const [info, setInfo] = useState<ShareInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [name, setName] = useState(savedName);
  useEffect(() => setName(savedName), [savedName]);

  useEffect(() => {
    if (open)
      api
        .getShareInfo()
        .then(setInfo)
        .catch((e) => setError(String(e)));
  }, [open, enabled, savedName]);
  // The name is checked on the network for a second or two after sharing starts.
  useEffect(() => {
    if (!open || info?.nameStatus !== "checking" || !info.enabled) return;
    const id = setTimeout(
      () =>
        void api
          .getShareInfo()
          .then(setInfo)
          .catch(() => {}),
      700,
    );
    return () => clearTimeout(id);
  }, [open, info]);

  const saveName = () => {
    const clean = cleanName(name);
    setName(clean);
    if (clean !== savedName)
      void saveSettings({ shareName: clean }).catch((e) => setError(String(e)));
  };

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
    <div ref={rootRef} className="relative">
      <button
        data-tour="share"
        onClick={() => setOpen(!open)}
        className={`h-9 rounded-lg px-2 text-xs font-bold hover:bg-white/[.06] min-[1300px]:px-3 ${open ? "bg-white/[.06] text-zinc-100" : "text-zinc-300"}`}
      >
        {t("Share")}
        {enabled && (
          <span className="ml-1.5 inline-block h-1.5 w-1.5 -translate-y-px rounded-full bg-amber-500" />
        )}
      </button>
      {open && (
        <div className="absolute right-0 top-full z-20 mt-2 max-h-[calc(100dvh-7rem)] w-[22rem] space-y-3 overflow-y-auto overscroll-contain rounded-xl bg-zinc-900 p-4 text-sm shadow-2xl ring-1 ring-zinc-700 backdrop-blur-xl">
          <label data-tour="share-toggle" className="flex items-center justify-between gap-3">
            <span>
              <span className="font-semibold">{t("Share on the local network")}</span>
              <span className="block text-xs text-zinc-400">
                {t("Phones, livestream overlay and remote. No internet needed.")}
              </span>
            </span>
            <input
              type="checkbox"
              checked={enabled}
              onChange={toggle}
              className="h-4 w-4 accent-sky-500"
            />
          </label>
          {(error || info?.error) && (
            <p className="text-xs text-red-400">{error ?? info?.error}</p>
          )}

          <label className="block space-y-1">
            <span className="text-xs text-zinc-400">{t("Network name")}</span>
            <span className="flex items-center rounded-md bg-zinc-950 ring-1 ring-zinc-800 focus-within:ring-sky-600">
              <span className="pl-2 text-zinc-500">http://</span>
              <input
                value={name}
                onChange={(e) => setName(e.target.value)}
                onBlur={saveName}
                onKeyDown={(e) =>
                  e.key === "Enter" &&
                  (e.currentTarget as HTMLInputElement).blur()
                }
                spellCheck={false}
                aria-label={t("Network name")}
                className="min-w-0 flex-1 bg-transparent px-0.5 py-1.5 font-mono text-sm text-zinc-100 outline-none"
              />
              <span className="pr-2 font-mono text-zinc-500">.local</span>
            </span>
            {info?.enabled && (
              <span
                className={`block text-xs ${NAME_STATUS[info.nameStatus].tone}`}
              >
                {NAME_STATUS[info.nameStatus].text(t, info.phoneUrl)}
              </span>
            )}
          </label>

          {info?.enabled && (
            <>
              <div data-tour="share-qr" className="flex gap-3">
                <div
                  className="h-28 w-28 flex-none overflow-hidden rounded bg-white p-1 [&_svg]:h-full [&_svg]:w-full"
                  dangerouslySetInnerHTML={{ __html: info.qrSvg }}
                />
                <div className="min-w-0 space-y-1.5">
                  <p className="text-xs text-zinc-400">
                    {t(
                      "Congregation: scan to follow the verses on their phone, in any translation and text size.",
                    )}
                  </p>
                  <button
                    onClick={() =>
                      void api.showQr(!qrLive).catch((e) => setError(String(e)))
                    }
                    className={`rounded px-2 py-1 text-xs font-medium ${qrLive ? "bg-amber-600 hover:bg-amber-500" : "bg-zinc-800 hover:bg-zinc-700"}`}
                  >
                    {qrLive ? t("Hide QR on screen") : t("Show QR on screen")}
                  </button>
                </div>
              </div>
              <Link label={t("Phones")} url={info.phoneUrl} />
              <PhoneRecap />
              <Link
                label={t("Livestream overlay")}
                url={info.overlayHereUrl}
                hint={t("For OBS or vMix on this computer: add a Browser source, 1920×1080. Style it in Style → Livestream.")}
              />
              <Link
                label={t("Streaming PC")}
                url={info.overlayUrl}
                hint={t("The overlay for OBS or vMix on another computer on the same network.")}
              />
              <Link
                label={t("Remote")}
                url={info.remoteUrl}
                hint={t("Open on your phone and enter PIN {pin}.", { pin: info.pin })}
              />
              <div className="rounded-md bg-zinc-950 p-2 text-xs text-zinc-400 ring-1 ring-zinc-800">
                {rich(
                  t(
                    "Some Android phones can't open {local} addresses. They can use {address} instead; the QR code always works.",
                  ),
                  {
                    local: <span className="font-mono">.local</span>,
                    address: (
                      <span className="select-all font-mono text-sky-300">
                        {info.ipUrl}
                      </span>
                    ),
                  },
                )}
              </div>
              <p className="text-xs text-zinc-500">
                {t(
                  "Devices must be on the same Wi-Fi. If they can't connect, allow Hearken Studio in the computer's firewall.",
                )}
              </p>
            </>
          )}
        </div>
      )}
    </div>
  );
}

/** Whether congregation phones get a Recap button, and whether it includes the sermon notes. */
function PhoneRecap() {
  const t = useT();
  const mode = useDashboard((s) => s.settings?.phoneRecap ?? "notes");
  const saveSettings = useDashboard((s) => s.saveSettings);
  return (
    <label className="block space-y-1">
      <span className="text-xs text-zinc-400">
        {t("Recap on phones (read or download as PDF or Word)")}
      </span>
      <select
        value={mode}
        onChange={(e) =>
          void saveSettings({ phoneRecap: e.target.value as typeof mode })
        }
        className="w-full rounded-md bg-zinc-950 px-2 py-1.5 text-sm ring-1 ring-zinc-800 focus:ring-sky-600"
      >
        <option value="notes">{t("Scriptures, songs and sermon notes")}</option>
        <option value="scriptures">{t("Scriptures and songs only")}</option>
        <option value="off">{t("Off")}</option>
      </select>
    </label>
  );
}

function Link({
  label,
  url,
  hint,
}: {
  label: string;
  url: string;
  hint?: string;
}) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  return (
    <div>
      <div className="flex items-center gap-2">
        <span className="w-20 flex-none text-xs text-zinc-400">{label}</span>
        <code
          className="min-w-0 flex-1 truncate rounded bg-zinc-950 px-1.5 py-0.5 text-xs text-sky-300"
          title={url}
        >
          {url.replace(/^https?:\/\//, "")}
        </code>
        <button
          onClick={() => {
            void navigator.clipboard.writeText(url).then(() => {
              setCopied(true);
              setTimeout(() => setCopied(false), 1200);
            });
          }}
          className="flex-none rounded bg-zinc-800 px-1.5 py-0.5 text-xs hover:bg-zinc-700"
        >
          {copied ? t("Copied") : t("Copy")}
        </button>
      </div>
      {hint && (
        <p className="ml-[5.5rem] mt-0.5 text-[11px] text-zinc-500">{hint}</p>
      )}
    </div>
  );
}
