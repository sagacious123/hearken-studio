import { useEffect, useState } from "react";
import type { OutputState, Slide } from "../lib/ipc";
import { lowerThirdBox, withAlpha, type Theme } from "../lib/theme";
import { useFitText } from "./useFitText";

/** Lower third background: black or green for a video switcher's key, or the theme colour. */
export const KEY_COLORS = { black: "#000000", green: "#00b140" } as const;

const lines = (slide: Slide) => (slide.song ? slide.verses.map((v) => v.text).join("\n") : slide.verses.map((v) => v.text).join(" "));

/**
 * Lower third for a livestream or video switcher: the passage in a band across the bottom of
 * the 1920×1080 stage, over a plain colour that a switcher can key out.
 */
export function LowerThird({ state, theme, keyColor }: { state: OutputState | null; theme: Theme; keyColor: string }) {
  const slide = state?.blackout ? null : (state?.slide ?? null);
  return (
    <div style={{ position: "absolute", inset: 0, background: keyColor }}>
      <LivestreamBand slide={slide} theme={theme} />
    </div>
  );
}

/**
 * The livestream band itself, as the Style panel sets it (Style → Livestream). Drawn on the
 * 1920×1080 stage; the OBS overlay (share/overlay.html) draws the same band from the same numbers.
 */
export function LivestreamBand({ slide, theme }: { slide: Slide | null; theme: Theme }) {
  const ls = theme.livestream;
  const box = lowerThirdBox(ls);
  const text = slide ? lines(slide) : "";
  const fitRef = useFitText<HTMLDivElement>(box.textMinPx, box.textMaxPx, [slide?.id, text, box.textMaxPx, ls.fontFamily]);
  if (!slide) return null;
  return (
    <div
      key={slide.id}
      className="sb-enter"
      style={{
        position: "absolute",
        left: box.left,
        right: box.right,
        top: box.top,
        bottom: box.bottom,
        height: box.height,
        boxSizing: "border-box",
        padding: box.padding,
        borderRadius: ls.radius,
        background: withAlpha(ls.background, ls.opacity),
        boxShadow: ls.stripe ? `inset ${Math.round(8 * ls.size)}px 0 0 ${ls.accent}` : undefined,
        display: "flex",
        flexDirection: "column",
        gap: Math.round(10 * ls.size),
        textAlign: ls.align,
      }}
    >
      {ls.showReference && (
        <div
          style={{
            flex: "none",
            fontFamily: ls.referenceFont || theme.reference.fontFamily,
            color: ls.accent,
            fontSize: box.referencePx,
            fontWeight: 700,
            letterSpacing: "0.06em",
            textTransform: "uppercase",
          }}
        >
          {slide.label}
          {!slide.song && ls.showTranslation && <span style={{ opacity: 0.7, marginLeft: "0.6em" }}>{slide.translation}</span>}
        </div>
      )}
      <div
        ref={fitRef}
        style={{
          flex: "1 1 auto",
          minHeight: 0,
          overflow: "hidden",
          fontFamily: ls.fontFamily || theme.text.fontFamily,
          color: ls.textColor,
          fontWeight: theme.text.weight,
          lineHeight: 1.3,
          whiteSpace: slide.song ? "pre-line" : undefined,
          textWrap: "pretty",
        }}
      >
        {text}
      </div>
    </div>
  );
}

function useClock() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(id);
  }, []);
  return now;
}

/**
 * Confidence monitor for the preacher or worship leader: what's on screen in large, plain
 * type, how far it has been read, what the congregation sees (blackout, clear), and the time.
 */
export function StageDisplay({ state }: { state: OutputState | null }) {
  const now = useClock();
  const slide = state?.slide ?? null;
  const reading = slide && state?.reading?.slideId === slide.id ? state.reading : null;
  const fitRef = useFitText<HTMLDivElement>(40, 120, [slide?.id]);
  const status = state?.blackout ? { text: "SCREEN BLACK", color: "#ef4444" } : !slide ? { text: "SCREEN CLEAR", color: "#71717a" } : { text: "LIVE", color: "#22c55e" };

  return (
    <div style={{ position: "absolute", inset: 0, background: "#000", color: "#fafafa", display: "flex", flexDirection: "column", padding: "40px 64px", boxSizing: "border-box", fontFamily: 'system-ui, -apple-system, "Segoe UI", sans-serif' }}>
      <div style={{ display: "flex", alignItems: "baseline", gap: 32, flex: "none" }}>
        <span style={{ fontSize: 30, fontWeight: 800, letterSpacing: "0.1em", color: status.color }}>● {status.text}</span>
        <span style={{ fontSize: 56, fontWeight: 700, color: "#fbbf24", flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {slide?.label ?? ""}
          {slide && !slide.song && <span style={{ color: "#a1a1aa", fontSize: 36, marginLeft: 20 }}>{slide.translation}</span>}
        </span>
        <span style={{ fontSize: 72, fontWeight: 700, fontVariantNumeric: "tabular-nums" }}>{now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
      </div>
      <div ref={fitRef} style={{ flex: "1 1 auto", minHeight: 0, overflow: "hidden", marginTop: 28, lineHeight: 1.25, fontWeight: 500, whiteSpace: slide?.song ? "pre-line" : undefined }}>
        {slide?.verses.map((v, i) => {
          const upTo = reading === null ? 0 : i < reading.line ? v.text.length : i === reading.line ? reading.offset : 0;
          return (
            <span key={v.verse}>
              {!slide.song && slide.verses.length > 1 && <sup style={{ fontSize: "0.45em", color: "#71717a", marginRight: "0.2em" }}>{v.verse}</sup>}
              <span style={{ color: "#71717a" }}>{v.text.slice(0, upTo)}</span>
              {v.text.slice(upTo)}
              {slide.song ? "\n" : " "}
            </span>
          );
        })}
      </div>
    </div>
  );
}
