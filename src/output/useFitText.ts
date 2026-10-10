import { useCallback, useEffect, useLayoutEffect, useRef } from "react";

/**
 * Largest font size in [minPx, maxPx] at which the element's content fits its box.
 * Binary search (~8 synchronous layouts, well under a frame) writing style.fontSize
 * directly, so fitting never triggers a React re-render or a visible jump.
 */
export function useFitText<T extends HTMLElement>(minPx: number, maxPx: number, deps: readonly unknown[]) {
  const ref = useRef<T>(null);

  const fit = useCallback(() => {
    const el = ref.current;
    if (!el) return;
    let lo = minPx;
    let hi = maxPx;
    let best = minPx;
    while (hi - lo > 0.5) {
      const mid = (lo + hi) / 2;
      el.style.fontSize = `${mid}px`;
      if (el.scrollHeight <= el.clientHeight + 1 && el.scrollWidth <= el.clientWidth + 1) {
        best = mid;
        lo = mid;
      } else {
        hi = mid;
      }
    }
    el.style.fontSize = `${best}px`;
  }, [minPx, maxPx]);

  // eslint-disable-next-line react-hooks/exhaustive-deps
  useLayoutEffect(fit, [fit, ...deps]);

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(() => fit());
    ro.observe(el);
    // Web fonts change metrics once loaded.
    document.fonts?.ready.then(fit).catch(() => {});
    return () => ro.disconnect();
  }, [fit]);

  return ref;
}
