import { useEffect, useRef } from "react";

/**
 * Close a popover or drawer on a click (or tap) outside it, or on Esc. Attach the returned ref
 * to the element that wraps both the toggle button and the panel, so the toggle keeps working.
 * `keepOpenWithin` lists selectors whose clicks don't count as outside (e.g. the Preview column
 * while styling). Esc is caught before the dashboard shortcuts, so it closes the panel instead
 * of clearing the screen.
 */
export function useDismiss<T extends HTMLElement>(open: boolean, close: () => void, keepOpenWithin: string[] = []) {
  const ref = useRef<T>(null);
  const closeRef = useRef(close);
  closeRef.current = close;
  const keep = keepOpenWithin.join(",");

  useEffect(() => {
    if (!open) return;
    const onPointerDown = (e: PointerEvent) => {
      const target = e.target as Element | null;
      if (!target || ref.current?.contains(target)) return;
      // A guided tour showing this panel: its own buttons aren't "outside".
      if (target.closest("[data-tour-ui]")) return;
      if (keep && target.closest(keep)) return;
      closeRef.current();
    };
    const onKeyDown = (e: KeyboardEvent) => {
      // During a guided tour, Esc ends the tour rather than closing the panel under it.
      if (e.key !== "Escape" || document.documentElement.dataset.tour) return;
      e.stopPropagation();
      e.preventDefault();
      closeRef.current();
    };
    document.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("keydown", onKeyDown, true);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("keydown", onKeyDown, true);
    };
  }, [open, keep]);

  return ref;
}
