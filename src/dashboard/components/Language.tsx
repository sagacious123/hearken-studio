import { useEffect, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { api, type LanguageSetup } from "../../lib/ipc";
import { translate, useT, type T, type UiLanguage } from "../../lib/i18n";
import { useDashboard } from "../store";
import { Modal, SignInForm } from "./Licence";

interface Choice {
  languages: string[];
  uiLanguage: UiLanguage;
}

/** How each language names itself as the language of menus. */
const IN_OWN_LANGUAGE: Record<string, string> = { fr: "En français", es: "En español" };

/**
 * Which languages are preached (listening understands each of them), and whether Hearken's own
 * menus and buttons change to one of them or stay in English.
 */
function LanguageChooser({ value, onChange, t }: { value: Choice; onChange: (c: Choice) => void; t: T }) {
  const options = useDashboard((s) => s.languages);
  const toggle = (code: string) => {
    const languages = value.languages.includes(code) ? value.languages.filter((c) => c !== code) : [...value.languages, code];
    if (languages.length === 0) return;
    onChange({ languages, uiLanguage: languages.includes(value.uiLanguage) ? value.uiLanguage : "en" });
  };
  const others = options.filter((o) => o.code !== "en" && value.languages.includes(o.code));
  const names = others.map((o) => o.native).join(" + ");
  return (
    <div className="grid gap-5">
      <fieldset className="grid gap-2">
        <legend className="mb-2 font-medium text-zinc-200">{t("Which languages are preached at your church?")}</legend>
        {options.map((o) => {
          const on = value.languages.includes(o.code);
          return (
            <label
              key={o.code}
              className={`flex cursor-pointer items-center gap-3 rounded-lg border px-3 py-2.5 ${on ? "border-amber-500/60 bg-amber-500/[.07]" : "border-zinc-800 hover:border-zinc-600"}`}
            >
              <input type="checkbox" checked={on} onChange={() => toggle(o.code)} className="accent-amber-500" />
              <span className="font-medium text-zinc-100">{o.native}</span>
              {o.native !== t(o.name) && <span className="text-xs text-zinc-500">{t(o.name)}</span>}
            </label>
          );
        })}
        <p className="text-xs font-light text-zinc-500">{t("Choose more than one if your services mix languages. Yoruba, Hausa and more are on the way.")}</p>
      </fieldset>

      {others.length > 0 && (
        <fieldset className="grid gap-2">
          <legend className="mb-2 font-medium text-zinc-200">{t("Hearken’s menus and buttons")}</legend>
          <MenuChoice
            checked={value.uiLanguage === "en"}
            onChoose={() => onChange({ ...value, uiLanguage: "en" })}
            title={t("Stay in English")}
            hint={t("{language} is used for listening only.", { language: names })}
          />
          {others.map((o) => (
            <MenuChoice
              key={o.code}
              checked={value.uiLanguage === o.code}
              onChoose={() => onChange({ ...value, uiLanguage: o.code as UiLanguage })}
              title={IN_OWN_LANGUAGE[o.code] ?? o.native}
              hint={t("Menus and buttons change to {language} too.", { language: o.native })}
            />
          ))}
        </fieldset>
      )}
    </div>
  );
}

function MenuChoice({ checked, onChoose, title, hint }: { checked: boolean; onChoose: () => void; title: string; hint: string }) {
  return (
    <label className="flex cursor-pointer items-start gap-3 rounded-lg px-1 py-1">
      <input type="radio" name="ui-language" checked={checked} onChange={onChoose} className="mt-1 accent-amber-500" />
      <span className="grid gap-0.5">
        <span className="font-medium text-zinc-100">{title}</span>
        <span className="text-xs font-light text-zinc-400">{hint}</span>
      </span>
    </label>
  );
}

/** What the languages need downloaded (their Bibles and the speech model), and how far along it is. */
function Needs({ languages, t, intro }: { languages: string[]; t: T; intro?: string }) {
  const setup = useDashboard((s) => s.setup);
  const [need, setNeed] = useState<LanguageSetup | null>(null);
  const key = languages.join(",");
  const downloading = Object.keys(setup).length;
  useEffect(() => {
    let cancelled = false;
    api
      .languageSetup(languages)
      .then((n) => !cancelled && setNeed(n))
      .catch(console.error);
    return () => {
      cancelled = true;
    };
    // Checked again whenever a download finishes.
  }, [key, downloading]);
  if (!need) return null;
  const items = [
    ...need.bibles.map((b) => ({ key: `bible:${b.code}`, label: b.name, mb: b.mb, installed: b.installed })),
    ...(need.speechModel
      ? [{ key: `model:${need.speechModel.id}`, label: t("Speech model · {model}", { model: t(need.speechModel.label) }), mb: need.speechModel.sizeMb, installed: need.speechModel.installed }]
      : []),
  ];
  if (items.length === 0) return null;
  return (
    <div className="grid gap-2 rounded-lg bg-white/[.03] px-3 py-2.5">
      {intro && <p className="text-xs font-light text-zinc-400">{intro}</p>}
      <ul className="grid gap-1.5">
        {items.map((it) => {
          const progress = setup[it.key]?.fraction;
          return (
            <li key={it.key} className="flex items-baseline gap-3 text-xs">
              <span className="min-w-0 flex-1 truncate text-zinc-200" title={it.label}>
                {it.label}
              </span>
              <span className="flex-none tabular-nums text-zinc-500">{t("{mb} MB", { mb: Math.round(it.mb) })}</span>
              <span className={`w-24 flex-none text-right tabular-nums ${it.installed ? "text-emerald-300" : "text-amber-200"}`}>
                {it.installed ? t("Ready") : progress !== undefined ? `${Math.round(progress * 100)}%` : t("Not downloaded")}
              </span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

/** Settings → Language. Also opened when listening needs a speech model that isn't downloaded. */
export function LanguagePanel() {
  const t = useT();
  const { open, setOpen, settings, applyLanguages, setup, setupError, running } = useDashboard(
    useShallow((s) => ({
      open: s.languageOpen,
      setOpen: s.setLanguageOpen,
      settings: s.settings,
      applyLanguages: s.applyLanguages,
      setup: s.setup,
      setupError: s.setupError,
      running: s.running,
    })),
  );
  const [choice, setChoice] = useState<Choice | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (open && settings) setChoice({ languages: settings.languages, uiLanguage: settings.uiLanguage });
    // Only when it opens: edits aren't overwritten by unrelated settings changes.
  }, [open]);
  if (!open || !settings || !choice) return null;

  const changed = choice.uiLanguage !== settings.uiLanguage || choice.languages.join(",") !== settings.languages.join(",");
  const downloading = Object.keys(setup).length > 0;
  const save = async () => {
    setBusy(true);
    try {
      await applyLanguages(choice.languages, choice.uiLanguage);
    } finally {
      setBusy(false);
    }
  };
  return (
    <Modal label={t("Language")} onClose={() => setOpen(false)}>
      <LanguageChooser value={choice} onChange={setChoice} t={t} />
      <Needs languages={choice.languages} t={t} intro={t("To listen offline, Hearken needs:")} />
      {setupError && <p className="text-xs text-red-300">{setupError.replace(/^Error: /, "")}</p>}
      {running && changed && <p className="text-xs font-light text-zinc-400">{t("Listening changes the next time you start listening.")}</p>}
      <div className="flex items-center gap-3">
        <button
          onClick={() => void save()}
          disabled={busy}
          className="rounded-lg bg-amber-500 px-3.5 py-1.5 font-semibold hover:bg-amber-400 disabled:opacity-40"
        >
          {changed ? t("Save") : t("Download what’s missing")}
        </button>
        {downloading && <span className="text-xs text-zinc-400">{t("Downloads carry on if you close this.")}</span>}
      </div>
    </Modal>
  );
}

/** In the top bar while language downloads run (or one failed). */
export function LanguageChip() {
  const t = useT();
  const setup = useDashboard((s) => s.setup);
  const failed = useDashboard((s) => s.setupError);
  const open = useDashboard((s) => s.setLanguageOpen);
  const items = Object.values(setup);
  if (items.length === 0 && !failed) return null;
  const fraction = items.length ? items.reduce((sum, d) => sum + d.fraction, 0) / items.length : 0;
  return (
    <button
      onClick={() => open(true)}
      className="h-9 whitespace-nowrap rounded-lg px-2.5 text-xs font-bold text-zinc-300 hover:bg-white/[.06]"
      title={[t("Language"), ...items.map((d) => d.label), failed ?? ""].filter(Boolean).join(" · ")}
      aria-label={t("Language")}
    >
      {items.length ? `↓ ${Math.round(fraction * 100)}%` : <span className="text-amber-300">⚠</span>}
    </button>
  );
}

/**
 * The first-run screens: the preaching language (and whether the menus change to it), then
 * signing in. Everything else (sound, the screen, Bibles) is in the top bar and Settings.
 */
export function Welcome() {
  const { settings, licence, applyLanguages, saveSettings } = useDashboard(
    useShallow((s) => ({ settings: s.settings, licence: s.licence, applyLanguages: s.applyLanguages, saveSettings: s.saveSettings })),
  );
  const [step, setStep] = useState<"language" | "account">("language");
  const [choice, setChoice] = useState<Choice | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (settings && !choice) setChoice({ languages: settings.languages, uiLanguage: settings.uiLanguage });
  }, [settings, choice]);
  if (!settings || settings.onboarded || !choice) return null;

  // These screens speak the menu language as soon as it's chosen.
  const t: T = (text, vars) => translate(choice.uiLanguage, text, vars);
  const signedIn = licence?.account.state === "signedIn";
  const finish = () => void saveSettings({ onboarded: true });
  const next = async () => {
    setBusy(true);
    setError(null);
    try {
      await applyLanguages(choice.languages, choice.uiLanguage);
      if (signedIn) finish();
      else setStep("account");
    } catch (e) {
      setError(String(e).replace(/^Error: /, ""));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 grid place-items-center overflow-y-auto bg-zinc-950/95 p-4 backdrop-blur-sm">
      <div
        aria-hidden
        className="pointer-events-none absolute left-1/2 top-[-300px] h-[660px] w-[1200px] max-w-full -translate-x-1/2 bg-[radial-gradient(closest-side,rgba(255,248,228,.12),rgba(248,224,164,.045)_45%,transparent_75%)]"
      />
      <section
        role="dialog"
        aria-modal="true"
        aria-label={t("Welcome to Hearken")}
        className="relative grid w-full max-w-lg gap-6 rounded-2xl bg-zinc-900 p-7 text-sm shadow-2xl ring-1 ring-zinc-700"
      >
        <header className="flex items-center justify-between">
          <span className="flex items-center gap-2.5 font-display text-base font-semibold tracking-[0.14em] text-zinc-100">
            <svg viewBox="0 0 32 32" className="h-5 w-5" aria-hidden>
              <path d="M16 3c3 4.5 5 7.2 5 10.2A5 5 0 0 1 16 18a5 5 0 0 1-5-4.8C11 10.2 13 7.5 16 3z" fill="#e9b24c" />
              <path d="M6 22c3.3-1.6 6.7-1.6 10 0 3.3-1.6 6.7-1.6 10 0v6c-3.3-1.6-6.7-1.6-10 0-3.3-1.6-6.7-1.6-10 0z" fill="#f4ede1" />
            </svg>
            HEARKEN
          </span>
          {!signedIn && <span className="text-xs font-medium text-zinc-500">{t("Step {n} of {total}", { n: step === "language" ? 1 : 2, total: 2 })}</span>}
        </header>

        {step === "language" ? (
          <>
            <div className="grid gap-1.5">
              <p className="font-scripture text-3xl text-zinc-50">{t("Welcome to Hearken")}</p>
              <p className="font-light leading-relaxed text-zinc-300">
                {t("Hearken listens to the sermon and puts each scripture on the screen as it’s read. First, the language.")}
              </p>
            </div>
            <LanguageChooser value={choice} onChange={setChoice} t={t} />
            <Needs languages={choice.languages} t={t} intro={t("To listen offline, Hearken will download:")} />
            {error && <p className="text-sm text-red-300">{error}</p>}
            <div className="flex items-center justify-between gap-3">
              <span className="text-xs font-light text-zinc-500">{t("You can change this later in Settings.")}</span>
              <button onClick={() => void next()} disabled={busy} className="rounded-lg bg-amber-500 px-4 py-2 font-semibold hover:bg-amber-400 disabled:opacity-40">
                {busy ? t("One moment…") : t("Continue")}
              </button>
            </div>
          </>
        ) : (
          <>
            <SignInForm onDone={finish} />
            <button onClick={finish} className="justify-self-start text-sm font-medium text-zinc-400 hover:text-zinc-100">
              {t("Not now: I’ll sign in when I start listening")}
            </button>
          </>
        )}
      </section>
    </div>
  );
}
