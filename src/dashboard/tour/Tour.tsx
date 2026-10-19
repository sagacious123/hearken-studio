import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { create } from "zustand";
import { useT } from "../../lib/i18n";
import { Mark } from "../components/Mark";
import { useDashboard } from "../store";
import { Modal } from "../components/Licence";
import { FIRST_OPEN, TOURS, tourById, type TourId } from "./tours";

// ---------- what's been seen (this computer only) ----------

const SEEN = "hk.tours.seen";
/** Set when a church starts out with this copy: panels then show their tour on first opening. */
const GUIDE = "hk.tours.guide";

const read = (key: string) => {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
};
const write = (key: string, value: string) => {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* a tour shown twice is harmless */
  }
};
export const seenTours = (): TourId[] => JSON.parse(read(SEEN) ?? "[]");
const markSeen = (id: TourId) => write(SEEN, JSON.stringify([...new Set([...seenTours(), id])]));

// ---------- the running tour ----------

interface TourState {
  active: TourId | null;
  step: number;
  /** The Guided tours window (Settings → Guided tours). */
  listOpen: boolean;
  setListOpen: (open: boolean) => void;
  start: (id: TourId) => void;
  go: (step: number) => void;
  end: () => void;
}

export const useTour = create<TourState>((set, get) => ({
  active: null,
  step: 0,
  listOpen: false,
  setListOpen: (listOpen) => set({ listOpen }),
  start: (active) => set({ active, step: 0, listOpen: false }),
  go: (step) => {
    const { active } = get();
    if (!active) return;
    if (step >= tourById(active).steps.length) return get().end();
    set({ step: Math.max(0, step) });
  },
  end: () => {
    const { active } = get();
    if (active) markSeen(active);
    set({ active: null, step: 0 });
  },
}));

// ---------- placing the spotlight and the card ----------

interface Box {
  top: number;
  left: number;
  width: number;
  height: number;
}
const PAD = 8;
const CARD_W = 360;
const GAP = 16;

/** Where the light goes: the element with a little room, or nothing (the middle). */
function measure(name: string | undefined): Box | null {
  if (!name) return null;
  const node = document.querySelector<HTMLElement>(`[data-tour="${name}"]`);
  if (!node) return null;
  const r = node.getBoundingClientRect();
  if (!r.width && !r.height) return null;
  return { top: r.top - PAD, left: r.left - PAD, width: r.width + PAD * 2, height: r.height + PAD * 2 };
}

/** Whether an element is actually showing: some panels stay in the page, slid off-screen, when closed. */
function onScreen(name: string) {
  const node = document.querySelector(`[data-tour="${name}"]`);
  if (!node) return false;
  const r = node.getBoundingClientRect();
  return r.width > 0 && r.height > 0 && r.right > 0 && r.left < innerWidth && r.bottom > 0 && r.top < innerHeight;
}

/** The card goes below the light, else above, else beside it, kept on screen. */
function place(box: Box | null, cardH: number): { top: number; left: number } {
  const vw = innerWidth;
  const vh = innerHeight;
  const clampX = (x: number) => Math.min(Math.max(16, x), vw - CARD_W - 16);
  const clampY = (y: number) => Math.min(Math.max(16, y), vh - cardH - 16);
  if (!box) return { top: clampY((vh - cardH) / 2), left: clampX((vw - CARD_W) / 2) };
  const below = box.top + box.height + GAP;
  const above = box.top - GAP - cardH;
  const centreX = clampX(box.left + box.width / 2 - CARD_W / 2);
  if (below + cardH <= vh - 16) return { top: below, left: centreX };
  if (above >= 16) return { top: above, left: centreX };
  const right = box.left + box.width + GAP;
  if (right + CARD_W <= vw - 16) return { top: clampY(box.top), left: right };
  return { top: clampY(box.top), left: clampX(box.left - GAP - CARD_W) };
}

// ---------- the host: spotlight, card, keys, and starting tours ----------

export function TourHost() {
  const t = useT();
  const { active, step, go, end, start } = useTour();
  const onboarded = useDashboard((s) => s.settings?.onboarded);
  const [box, setBox] = useState<Box | null>(null);
  const [cardH, setCardH] = useState(240);
  const card = useRef<HTMLDivElement>(null);
  const tour = active ? tourById(active) : null;
  const current = tour?.steps[step];
  const last = !!tour && step === tour.steps.length - 1;

  // A church finishing the welcome screens gets the welcome tour, and first-open tours after it.
  const wasOnboarded = useRef(onboarded);
  useEffect(() => {
    if (wasOnboarded.current === false && onboarded) {
      write(GUIDE, "1");
      setTimeout(() => useTour.getState().start("basics"), 900);
    }
    wasOnboarded.current = onboarded;
  }, [onboarded]);

  // First-open tours: a panel appearing for the first time shows its tour.
  useEffect(() => {
    if (read(GUIDE) !== "1") return;
    const check = () => {
      if (useTour.getState().active) return;
      const seen = seenTours();
      if (!seen.includes("basics")) return;
      const due = FIRST_OPEN.find(([id, anchor]) => !seen.includes(id) && onScreen(anchor));
      if (due) setTimeout(() => !useTour.getState().active && start(due[0]), 350);
    };
    const watcher = new MutationObserver(check);
    watcher.observe(document.body, { childList: true, subtree: true });
    return () => watcher.disconnect();
  }, [start, active]);

  // Open what the step needs, then follow its element (menus open, panels move).
  useEffect(() => {
    if (!current) return;
    current.prepare?.();
    const follow = () => {
      const next = measure(current.target);
      setBox((prev) => (JSON.stringify(prev) === JSON.stringify(next) ? prev : next));
    };
    const first = setTimeout(() => {
      document.querySelector(`[data-tour="${current.target}"]`)?.scrollIntoView({ block: "nearest", behavior: "smooth" });
      follow();
    }, 60);
    const id = setInterval(follow, 200);
    addEventListener("resize", follow);
    return () => {
      clearTimeout(first);
      clearInterval(id);
      removeEventListener("resize", follow);
    };
  }, [current]);

  useLayoutEffect(() => {
    if (card.current) setCardH(card.current.offsetHeight);
  }, [step, active, t]);

  // While a tour is open, keys move the tour and never reach the dashboard (Enter = Go live).
  useEffect(() => {
    if (!active) return;
    document.documentElement.dataset.tour = "on";
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Tab") return;
      e.stopImmediatePropagation();
      if (e.key === "ArrowRight" || e.key === "Enter") go(step + 1);
      else if (e.key === "ArrowLeft") go(step - 1);
      else if (e.key === "Escape") end();
      else return;
      e.preventDefault();
    };
    addEventListener("keydown", onKey, true);
    return () => {
      removeEventListener("keydown", onKey, true);
      delete document.documentElement.dataset.tour;
    };
  }, [active, step, go, end]);

  if (!tour || !current) return null;
  const spot = box ?? { top: innerHeight / 2, left: innerWidth / 2, width: 0, height: 0 };
  const at = place(box, cardH);
  const welcome = active === "basics" && step === 0;

  return createPortal(
    <div data-tour-ui className="tour fixed inset-0 z-[100]" role="dialog" aria-modal="true" aria-labelledby="tour-title">
      {/* Blocks clicks on the dashboard; the light itself only darkens around the element. */}
      <div className="absolute inset-0" />
      <div className={`tour-spot ${box ? "" : "tour-spot-none"}`} style={{ top: spot.top, left: spot.left, width: spot.width, height: spot.height }} />
      <div ref={card} className="tour-card" style={{ top: at.top, left: at.left, width: CARD_W }}>
        <div key={`${active}-${step}`} className="tour-card-in grid gap-3">
          {welcome ? (
            <div className="flex justify-center pb-1 pt-2">
              <Mark className="h-14 w-14 drop-shadow-[0_0_24px_rgba(233,178,76,.45)]" listening />
            </div>
          ) : (
            <div className="flex items-center gap-3">
              <span className="text-[11px] font-semibold uppercase tracking-[0.18em] text-amber-200/80">
                {t("Step {n} of {total}", { n: step + 1, total: tour.steps.length })}
              </span>
              <span className="h-1 flex-1 overflow-hidden rounded-full bg-white/10">
                <span className="tour-progress block h-full rounded-full bg-amber-500" style={{ width: `${((step + 1) / tour.steps.length) * 100}%` }} />
              </span>
            </div>
          )}
          <h2 id="tour-title" className={`font-scripture text-zinc-50 ${welcome ? "text-center text-3xl" : "text-2xl"}`}>
            {t(current.title)}
          </h2>
          <p className={`font-light leading-relaxed text-zinc-300 ${welcome ? "text-center" : ""}`}>{t(current.body)}</p>
          <div className={`flex items-center gap-2 pt-1 ${welcome ? "justify-center" : ""}`}>
            {welcome ? (
              <>
                <button onClick={end} className="rounded-lg px-3 py-2 text-sm font-medium text-zinc-400 hover:text-zinc-100">
                  {t("Not now")}
                </button>
                <button onClick={() => go(1)} autoFocus className="rounded-lg bg-amber-500 px-4 py-2 text-sm font-bold text-[#1d1200] hover:bg-amber-400">
                  {t("Show me around")}
                </button>
              </>
            ) : (
              <>
                {!last && (
                  <button onClick={end} className="mr-auto text-sm font-medium text-zinc-500 hover:text-zinc-200">
                    {t("Skip tour")}
                  </button>
                )}
                {step > 0 && (
                  <button onClick={() => go(step - 1)} className={`rounded-lg border border-zinc-700 px-3 py-2 text-sm font-semibold text-zinc-200 hover:border-amber-500 ${last ? "mr-auto" : ""}`}>
                    {t("Back")}
                  </button>
                )}
                <button onClick={() => go(step + 1)} autoFocus className="rounded-lg bg-amber-500 px-4 py-2 text-sm font-bold text-[#1d1200] hover:bg-amber-400">
                  {last ? t("Finish") : t("Next")}
                </button>
              </>
            )}
          </div>
        </div>
        {last && <Sparkles />}
      </div>
    </div>,
    document.body,
  );
}

/** A small burst of gold at the end of a tour. */
function Sparkles() {
  return (
    <div aria-hidden className="pointer-events-none absolute inset-0">
      {Array.from({ length: 14 }, (_, i) => (
        <i key={i} className="tour-spark" style={{ ["--a" as string]: `${(i / 14) * 360}deg`, ["--d" as string]: `${70 + (i % 3) * 26}px`, animationDelay: `${(i % 4) * 40}ms` }} />
      ))}
    </div>
  );
}

/** Settings → Guided tours: every tour, to take again or for the first time. */
export function ToursDialog() {
  const t = useT();
  const { listOpen, setListOpen, start } = useTour();
  if (!listOpen) return null;
  const seen = seenTours();
  return (
    <Modal label={t("Guided tours")} onClose={() => setListOpen(false)}>
      <p className="font-light leading-relaxed text-zinc-300">{t("A short tour of each part of Hearken. Take any of them again whenever you like.")}</p>
      <ul className="grid gap-2">
        {TOURS.map((tour) => (
          <li key={tour.id} className="flex items-center gap-3 rounded-lg border border-zinc-800 bg-black/20 px-3 py-2.5">
            <span className="grid min-w-0 flex-1">
              <span className="font-medium text-zinc-100">
                {t(tour.name)}
                {seen.includes(tour.id) && <span className="ml-2 text-xs font-normal text-amber-300/80">✓ {t("Taken")}</span>}
              </span>
              <span className="text-xs font-light text-zinc-500">{t(tour.summary)}</span>
            </span>
            <button
              onClick={() => {
                setListOpen(false);
                setTimeout(() => start(tour.id), 150);
              }}
              className="flex-none rounded-lg border border-zinc-700 px-3 py-1.5 text-sm font-semibold text-zinc-100 hover:border-amber-500"
            >
              {seen.includes(tour.id) ? t("Again") : t("Start")}
            </button>
          </li>
        ))}
      </ul>
    </Modal>
  );
}
