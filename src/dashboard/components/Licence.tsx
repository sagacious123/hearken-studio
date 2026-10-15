import { useEffect, useState } from "react";
import { api, type LicenceInfo, type LicenceStatus } from "../../lib/ipc";
import { useDashboard } from "../store";

const date = (unix: number) => new Date(unix * 1000).toLocaleDateString(undefined, { day: "numeric", month: "long", year: "numeric" });
const DAY = 86_400;
const GRACE_DAYS = 14;

/** Beside the brand: Sign in when signed out, then how long the trial has left or that something
 *  needs attention. Nothing once signed in and licensed. */
export function LicenceChip() {
  const licence = useDashboard((s) => s.licence);
  const openLicence = useDashboard((s) => s.setLicenceOpen);
  const openSignIn = useDashboard((s) => s.setSignInOpen);
  if (!licence) return null;
  const { status, account } = licence;
  const chip = ((): [string, Tone] | null => {
    switch (status.state) {
      case "trial":
        return [`Trial · ${status.daysLeft} ${status.daysLeft === 1 ? "day" : "days"} left`, status.daysLeft <= 7 ? "warn" : "calm"];
      case "trialOver":
        return ["Trial ended", "stop"];
      case "licensed":
        return status.inGrace ? ["Renew licence", "warn"] : null;
      case "licenceExpired":
        return ["Licence ended", "stop"];
      case "clockBehind":
        return ["Check the date", "stop"];
    }
  })();
  return (
    <>
      {account.state !== "signedIn" && (
        <button onClick={() => openSignIn(true)} className={`${CHIP} ${TONE.warn}`} title="Sign in to start listening">
          Sign in
        </button>
      )}
      {chip && (
        <button onClick={() => openLicence(true)} className={`${CHIP} ${TONE[chip[1]]}`} title="Account, free trial and licence">
          {chip[0]}
        </button>
      )}
    </>
  );
}

type Tone = "calm" | "warn" | "stop";
const CHIP = "h-7 flex-none rounded-full border px-2.5 text-xs font-medium";
const TONE: Record<Tone, string> = {
  calm: "border-zinc-700 text-zinc-300 hover:border-zinc-500",
  warn: "border-amber-500/50 bg-amber-500/10 text-amber-200 hover:border-amber-400",
  stop: "border-red-500/50 bg-red-500/10 text-red-200 hover:border-red-400",
};
const FIELD =
  "rounded-lg border border-zinc-800 bg-black/20 px-3 py-2 text-sm text-zinc-100 outline-none placeholder:text-zinc-600 focus:border-amber-500";
const PRIMARY = "justify-self-start rounded-lg bg-amber-500 px-3.5 py-1.5 font-semibold hover:bg-amber-400 disabled:opacity-40";
const LINK = "justify-self-start text-sm font-medium text-amber-200 hover:text-amber-100";

/** A centred dialog over the dashboard, closed with ✕, Escape or a click outside. */
function Modal({ label, onClose, children }: { label: string; onClose: () => void; children: React.ReactNode }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    addEventListener("keydown", onKey);
    return () => removeEventListener("keydown", onKey);
  }, [onClose]);
  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/60 p-4 backdrop-blur-sm" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <section role="dialog" aria-modal="true" aria-label={label} className="grid w-full max-w-md gap-5 rounded-2xl bg-zinc-900 p-6 text-sm shadow-2xl ring-1 ring-zinc-700">
        <header className="flex items-center justify-between">
          <h2 className="font-display text-[13px] font-semibold uppercase tracking-[0.2em] text-amber-200">{label}</h2>
          <button onClick={onClose} className="rounded px-2 py-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200" aria-label="Close">
            ✕
          </button>
        </header>
        {children}
      </section>
    </div>
  );
}

function Message({ message }: { message: { ok: boolean; text: string } | null }) {
  if (!message) return null;
  return (
    <p role="status" className={`text-sm ${message.ok ? "text-emerald-300" : "text-red-300"}`}>
      {message.text.replace(/^Error: /, "")}
    </p>
  );
}

/** Signing in, creating the church's account, or asking for a password-reset email. */
export function SignInPanel() {
  const isOpen = useDashboard((s) => s.signInOpen);
  const setOpen = useDashboard((s) => s.setSignInOpen);
  const [mode, setMode] = useState<"signIn" | "create" | "forgot">("signIn");
  const [church, setChurch] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    if (isOpen) return;
    setPassword("");
    setMessage(null);
    setMode("signIn");
  }, [isOpen]);

  if (!isOpen) return null;

  const go = (m: typeof mode) => {
    setMode(m);
    setMessage(null);
  };

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setMessage(null);
    try {
      if (mode === "forgot") {
        await api.forgotPassword(email);
        setMessage({ ok: true, text: "If there’s an account for that email, a link to choose a new password is on its way. Check your spam folder too." });
      } else {
        const licence = mode === "create" ? await api.signUp(church, email, password) : await api.signIn(email, password);
        useDashboard.setState({ licence, signInOpen: false });
      }
    } catch (err) {
      setMessage({ ok: false, text: String(err) });
    } finally {
      setBusy(false);
    }
  };

  const title = { signIn: "Sign in", create: "Create your account", forgot: "Reset your password" }[mode];
  return (
    <Modal label="Account" onClose={() => setOpen(false)}>
      <div className="grid gap-1.5">
        <p className="font-scripture text-2xl text-zinc-50">{title}</p>
        <p className="font-light leading-relaxed text-zinc-300">
          {mode === "forgot"
            ? "We’ll email you a link to choose a new password."
            : "Signing in needs the internet. After that, Hearken works offline for up to 30 days at a time."}
        </p>
      </div>

      <form className="grid gap-2" onSubmit={submit}>
        {mode === "create" && (
          <>
            <label htmlFor="acct-church" className="font-medium text-zinc-200">
              Church name
            </label>
            <input id="acct-church" value={church} onChange={(e) => setChurch(e.target.value)} required maxLength={120} autoComplete="organization" className={FIELD} />
          </>
        )}
        <label htmlFor="acct-email" className="font-medium text-zinc-200">
          Email
        </label>
        <input id="acct-email" type="email" value={email} onChange={(e) => setEmail(e.target.value)} required autoComplete="email" autoFocus className={FIELD} />
        {mode !== "forgot" && (
          <>
            <label htmlFor="acct-password" className="font-medium text-zinc-200">
              Password
            </label>
            <input
              id="acct-password"
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              required
              minLength={mode === "create" ? 8 : undefined}
              autoComplete={mode === "create" ? "new-password" : "current-password"}
              className={FIELD}
            />
            {mode === "create" && <p className="text-xs font-light text-zinc-500">At least 8 characters.</p>}
          </>
        )}
        <button type="submit" disabled={busy} className={`${PRIMARY} mt-2`}>
          {busy ? "One moment…" : { signIn: "Sign in", create: "Create account", forgot: "Email me a link" }[mode]}
        </button>
      </form>

      <Message message={message} />

      <div className="grid gap-1.5 border-t border-zinc-800 pt-4">
        {mode === "signIn" && (
          <>
            <button onClick={() => go("forgot")} className={LINK}>
              Forgot your password?
            </button>
            <button onClick={() => go("create")} className={LINK}>
              New to Hearken? Create an account
            </button>
          </>
        )}
        {mode !== "signIn" && (
          <button onClick={() => go("signIn")} className={LINK}>
            {mode === "create" ? "Already have an account? Sign in" : "Back to sign in"}
          </button>
        )}
      </div>
    </Modal>
  );
}

/** The account, trial or licence, with a place to enter a key and a check against the website. */
export function LicencePanel() {
  const isOpen = useDashboard((s) => s.licenceOpen);
  const setOpen = useDashboard((s) => s.setLicenceOpen);
  const licence = useDashboard((s) => s.licence);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState<"check" | "activate" | "signOut" | null>(null);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    if (!isOpen) setMessage(null);
  }, [isOpen]);

  if (!isOpen || !licence) return null;

  const run = async (what: "check" | "activate" | "signOut") => {
    setBusy(what);
    setMessage(null);
    try {
      const next: LicenceInfo =
        what === "check" ? await api.checkLicenceOnline() : what === "activate" ? await api.activateLicence(key) : await api.signOut();
      useDashboard.setState({ licence: next });
      if (what === "activate") setKey("");
      if (what !== "signOut") setMessage({ ok: true, text: what === "check" ? "Checked with the Hearken website." : "Licence activated. Thank you!" });
    } catch (e) {
      setMessage({ ok: false, text: String(e) });
    } finally {
      setBusy(null);
    }
  };

  const { account, status } = licence;
  return (
    <Modal label="Licence" onClose={() => setOpen(false)}>
      <Summary licence={status} />

      <div className="flex flex-wrap items-center justify-between gap-2 rounded-lg bg-white/[.03] px-3 py-2.5">
        {account.state === "signedIn" ? (
          <>
            <span className="min-w-0 text-zinc-300">
              Signed in as <span className="font-medium text-zinc-100">{account.email}</span>
              <span className="block text-xs font-light text-zinc-500">{account.church}</span>
            </span>
            <button onClick={() => run("signOut")} disabled={busy !== null} className="rounded-lg border border-zinc-700 px-3 py-1.5 font-medium text-zinc-200 hover:border-zinc-500 disabled:opacity-50">
              Sign out
            </button>
          </>
        ) : (
          <>
            <span className="text-zinc-300">{account.state === "expired" ? `Sign-in for ${account.email} needs renewing.` : "Not signed in."}</span>
            <button
              onClick={() => {
                setOpen(false);
                useDashboard.getState().setSignInOpen(true);
              }}
              className={PRIMARY}
            >
              Sign in
            </button>
          </>
        )}
      </div>

      <div className="flex flex-wrap items-center gap-2">
        <button
          onClick={() => run("check")}
          disabled={busy !== null}
          className="rounded-lg border border-zinc-700 px-3 py-1.5 font-medium text-zinc-200 hover:border-zinc-500 disabled:opacity-50"
          title="Renews your sign-in, registers this computer's trial and checks its date with the Hearken website"
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
          {status.state === "licensed" ? "New licence key" : "Licence key"}
        </label>
        <textarea
          id="licence-key"
          value={key}
          onChange={(e) => setKey(e.target.value)}
          rows={3}
          spellCheck={false}
          placeholder="Paste the key from your email (it starts with HK1.)"
          className={`${FIELD} resize-none font-mono text-xs placeholder:font-ui`}
        />
        <button type="submit" disabled={!key.trim() || busy !== null} className={PRIMARY}>
          {busy === "activate" ? "Activating…" : "Activate"}
        </button>
      </form>

      <Message message={message} />
      {status.state === "trial" && <p className="text-xs font-light text-zinc-500">Paid plans will be announced before your trial ends.</p>}
    </Modal>
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
