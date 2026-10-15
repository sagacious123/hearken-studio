import { useEffect, useState } from "react";
import { api, type LicenceStatus } from "../../lib/ipc";
import { useDashboard } from "../store";

const date = (unix: number) => new Date(unix * 1000).toLocaleDateString(undefined, { day: "numeric", month: "long", year: "numeric" });
const DAY = 86_400;
const GRACE_DAYS = 14;

/** Beside the brand: how long the trial has left, or that something needs attention. Hidden
 *  once licensed. */
export function LicenceChip() {
  const licence = useDashboard((s) => s.licence);
  const open = useDashboard((s) => s.setLicenceOpen);
  if (!licence) return null;
  const chip = ((): [string, "calm" | "warn" | "stop"] | null => {
    switch (licence.state) {
      case "trial":
        return [`Trial · ${licence.daysLeft} ${licence.daysLeft === 1 ? "day" : "days"} left`, licence.daysLeft <= 7 ? "warn" : "calm"];
      case "trialOver":
        return ["Trial ended", "stop"];
      case "licensed":
        return licence.inGrace ? ["Renew licence", "warn"] : null;
      case "licenceExpired":
        return ["Licence ended", "stop"];
      case "clockBehind":
        return ["Check the date", "stop"];
    }
  })();
  if (!chip) return null;
  const tone = {
    calm: "border-zinc-700 text-zinc-300 hover:border-zinc-500",
    warn: "border-amber-500/50 bg-amber-500/10 text-amber-200 hover:border-amber-400",
    stop: "border-red-500/50 bg-red-500/10 text-red-200 hover:border-red-400",
  }[chip[1]];
  return (
    <button onClick={() => open(true)} className={`h-7 flex-none rounded-full border px-2.5 text-xs font-medium ${tone}`} title="Free trial and licence">
      {chip[0]}
    </button>
  );
}

/** The trial or licence, with a place to enter a key and a check against the website. */
export function LicencePanel() {
  const isOpen = useDashboard((s) => s.licenceOpen);
  const setOpen = useDashboard((s) => s.setLicenceOpen);
  const licence = useDashboard((s) => s.licence);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState<"check" | "activate" | null>(null);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    if (!isOpen) return setMessage(null);
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    addEventListener("keydown", onKey);
    return () => removeEventListener("keydown", onKey);
  }, [isOpen, setOpen]);

  if (!isOpen || !licence) return null;

  const run = async (what: "check" | "activate") => {
    setBusy(what);
    setMessage(null);
    try {
      const next = what === "check" ? await api.checkLicenceOnline() : await api.activateLicence(key);
      useDashboard.setState({ licence: next });
      if (what === "activate") setKey("");
      setMessage({ ok: true, text: what === "check" ? "Checked with the Hearken website." : "Licence activated. Thank you!" });
    } catch (e) {
      setMessage({ ok: false, text: String(e) });
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/60 p-4 backdrop-blur-sm" onMouseDown={(e) => e.target === e.currentTarget && setOpen(false)}>
      <section role="dialog" aria-modal="true" aria-label="Licence" className="grid w-full max-w-md gap-5 rounded-2xl bg-zinc-900 p-6 text-sm shadow-2xl ring-1 ring-zinc-700">
        <header className="flex items-center justify-between">
          <h2 className="font-display text-[13px] font-semibold uppercase tracking-[0.2em] text-amber-200">Licence</h2>
          <button onClick={() => setOpen(false)} className="rounded px-2 py-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200" aria-label="Close">
            ✕
          </button>
        </header>

        <Summary licence={licence} />

        <div className="flex flex-wrap items-center gap-2">
          <button
            onClick={() => run("check")}
            disabled={busy !== null}
            className="rounded-lg border border-zinc-700 px-3 py-1.5 font-medium text-zinc-200 hover:border-zinc-500 disabled:opacity-50"
            title="Registers this computer's trial and checks its date with the Hearken website"
          >
            {busy === "check" ? "Checking…" : "Check now"}
          </button>
          <span className="text-xs font-light text-zinc-500">Needs the internet. Nothing about your services is sent.</span>
        </div>

        <form
          className="grid gap-2 border-t border-zinc-800 pt-4"
          onSubmit={(e) => {
            e.preventDefault();
            if (key.trim()) void run("activate");
          }}
        >
          <label htmlFor="licence-key" className="font-medium text-zinc-200">
            {licence.state === "licensed" ? "New licence key" : "Licence key"}
          </label>
          <textarea
            id="licence-key"
            value={key}
            onChange={(e) => setKey(e.target.value)}
            rows={3}
            spellCheck={false}
            placeholder="Paste the key from your email (it starts with HK1.)"
            className="resize-none rounded-lg border border-zinc-800 bg-black/20 px-3 py-2 font-mono text-xs text-zinc-100 outline-none placeholder:font-ui placeholder:text-zinc-600 focus:border-amber-500"
          />
          <button
            type="submit"
            disabled={!key.trim() || busy !== null}
            className="justify-self-start rounded-lg bg-amber-500 px-3.5 py-1.5 font-semibold hover:bg-amber-400 disabled:opacity-40"
          >
            {busy === "activate" ? "Activating…" : "Activate"}
          </button>
        </form>

        {message && (
          <p role="status" className={`text-sm ${message.ok ? "text-emerald-300" : "text-red-300"}`}>
            {message.text}
          </p>
        )}
        {licence.state === "trial" && <p className="text-xs font-light text-zinc-500">Paid plans will be announced before your trial ends.</p>}
      </section>
    </div>
  );
}

function Summary({ licence }: { licence: LicenceStatus }) {
  const [title, ...lines] = ((): string[] => {
    switch (licence.state) {
      case "trial":
        return [
          "Free trial",
          `Ends on ${date(licence.ends)}: ${licence.daysLeft} ${licence.daysLeft === 1 ? "day" : "days"} left. Every feature is included.`,
          licence.registered
            ? "Registered for this computer."
            : "Hearken registers this computer’s trial the next time it’s online, using an anonymous ID.",
        ];
      case "trialOver":
        return [
          "Your free trial has ended",
          `It ended on ${date(licence.ended)}. Search, the screen and phones still work. To listen again, enter a licence key.`,
        ];
      case "licensed":
        return licence.inGrace
          ? [
              `Licensed to ${licence.church}`,
              `Your licence ended on ${date(licence.expires)}. Listening keeps working until ${date(licence.expires + GRACE_DAYS * DAY)}: enter your new key before then.`,
            ]
          : [`Licensed to ${licence.church}`, `${capital(licence.plan)} plan, until ${date(licence.expires)}.`];
      case "licenceExpired":
        return [
          "Your licence has ended",
          `The licence for ${licence.church} ended on ${date(licence.expired)}. Search, the screen and phones still work. To listen again, enter your new key.`,
        ];
      case "clockBehind":
        return [
          "This computer’s date looks wrong",
          `It says ${date(licence.now)}, but Hearken was already used on ${date(licence.seen)}. Set the correct date and time, or connect to the internet and click Check now.`,
        ];
    }
  })();
  return (
    <div className="grid gap-1.5">
      <p className="font-scripture text-2xl text-zinc-50">{title}</p>
      {lines.map((l) => (
        <p key={l} className="font-light leading-relaxed text-zinc-300">
          {l}
        </p>
      ))}
    </div>
  );
}

const capital = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
