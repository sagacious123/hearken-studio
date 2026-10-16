// Hearken's menus and buttons in the operator's language. The English text is the key, so a
// string without a translation yet simply shows in English. Placeholders are {name}:
// t("{n} days left", { n: 3 }). Each area of the app keeps its translations in its own file
// under ./i18n.
import { useCallback } from "react";
import { create } from "zustand";
import { es, fr } from "./i18n/index";

export type UiLanguage = "en" | "fr" | "es";
export type Vars = Record<string, string | number>;
export type T = (text: string, vars?: Vars) => string;

const DICTIONARIES: Record<string, Record<string, string>> = { fr, es };
const LOCALES: Record<UiLanguage, string> = { en: "en-GB", fr: "fr-FR", es: "es-ES" };

const useLanguage = create<{ lang: UiLanguage }>(() => ({ lang: "en" }));

/** Follow the settings (called whenever they load or change). */
export function setUiLanguage(lang: string | undefined) {
  const next: UiLanguage = lang === "fr" || lang === "es" ? lang : "en";
  if (useLanguage.getState().lang !== next) {
    useLanguage.setState({ lang: next });
    document.documentElement.lang = next;
  }
}

/** `text` in `lang`, with its placeholders filled in. */
export function translate(lang: string, text: string, vars?: Vars): string {
  const out = DICTIONARIES[lang]?.[text] ?? text;
  return vars ? out.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m)) : out;
}

/** The translator for the current language; components re-render when it changes. */
export function useT(): T {
  const lang = useLanguage((s) => s.lang);
  return useCallback((text: string, vars?: Vars) => translate(lang, text, vars), [lang]);
}

/** Outside components (the store, event handlers). */
export const t: T = (text, vars) => translate(useLanguage.getState().lang, text, vars);

export const useUiLanguage = () => useLanguage((s) => s.lang);

/** For dates and numbers: toLocaleDateString(locale()). */
export const locale = (lang: UiLanguage = useLanguage.getState().lang) => LOCALES[lang];
