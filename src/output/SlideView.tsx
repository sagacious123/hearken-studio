import { memo, useEffect, useRef, useState, type CSSProperties } from "react";
import "../assets/fonts/fonts.css";
import { useBackgroundUrl } from "../lib/backgrounds";
import type { OutputState, Reading, Slide, VerseLine } from "../lib/ipc";
import type { Theme } from "../lib/theme";
import { useFitText } from "./useFitText";
import "./output.css";

interface Props {
  slide: Slide | null;
  theme: Theme;
  blackout: boolean;
  /** Follow-along progress through the live slide. */
  reading?: Reading | null;
  /** "Follow on your phone" code in the corner. */
  qr?: OutputState["qr"];
}

interface Layer {
  key: number;
  slide: Slide | null;
}

/**
 * Full-stage slide renderer with GPU-composited crossfades (opacity-only animations).
 * Memoised: the dashboard's Preview stage needn't re-render when only the live slide changes.
 */
export const SlideView = memo(function SlideView({ slide, theme, blackout, reading = null, qr = null }: Props) {
  const keyRef = useRef(1);
  const [layers, setLayers] = useState<Layer[]>([{ key: 0, slide }]);

  useEffect(() => {
    setLayers((prev) => {
      const top = prev[prev.length - 1];
      if (top.slide?.id === slide?.id) {
        // Same passage (e.g. re-sent): update in place without a transition.
        return [...prev.slice(0, -1), { ...top, slide }];
      }
      if (theme.transition.type === "none") return [{ key: keyRef.current++, slide }];
      // Keep only the outgoing layer; rapid changes collapse instead of stacking.
      return [top, { key: keyRef.current++, slide }];
    });
  }, [slide, theme.transition.type]);

  const { background: bg } = theme;
  const picture = useBackgroundUrl(bg.picture);
  const style = { "--sb-fade": `${theme.transition.durationMs}ms`, background: bg.color } as CSSProperties;
  const image = picture ? `url("${picture}")` : bg.image;

  return (
    <div className="sb-stage" style={style}>
      {image && (
        // Own layer so blur softens only the picture; scaled up so blurred edges stay off-screen.
        <div
          className="sb-fill sb-bg"
          style={{
            backgroundImage: image,
            filter: bg.blurPx ? `blur(${bg.blurPx}px)` : undefined,
            transform: bg.blurPx ? "scale(1.06)" : undefined,
          }}
        />
      )}
      {image && bg.dim > 0 && <div className="sb-fill" style={{ background: `rgba(0,0,0,${bg.dim})` }} />}
      {bg.overlay && <div className="sb-fill" style={{ background: bg.overlay }} />}
      {layers.map((layer, i) => {
        const outgoing = i < layers.length - 1;
        return (
          <div
            key={layer.key}
            className={`sb-fill ${outgoing ? "sb-exit" : layers.length > 1 ? "sb-enter" : ""}`}
            onAnimationEnd={outgoing ? () => setLayers((ls) => ls.filter((l) => l.key !== layer.key)) : undefined}
          >
            {layer.slide && (
              <SlideContent slide={layer.slide} theme={theme} reading={reading?.slideId === layer.slide.id ? reading : null} />
            )}
          </div>
        );
      })}
      {qr && (
        <div className="sb-qr">
          <div dangerouslySetInnerHTML={{ __html: qr.svg }} />
          <div>
            Follow along on your phone
            <small>{qr.url}</small>
          </div>
        </div>
      )}
      <div className="sb-fill sb-blackout" style={{ opacity: blackout ? 1 : 0 }} />
    </div>
  );
});

function SlideContent({ slide, theme, reading }: { slide: Slide; theme: Theme; reading: Reading | null }) {
  const { text, reference, layout } = theme;
  const pad = `${(layout.paddingPct / 100) * 1920}px`;
  const justify = { top: "flex-start", center: "center", bottom: "flex-end" }[layout.verticalAlign];
  const song = !!slide.song;
  const translations = slide.secondary ? `${slide.translation} · ${slide.secondary.translation}` : slide.translation;

  const refLabel = reference.show && (
    <div
      style={{
        fontFamily: reference.fontFamily,
        color: reference.color,
        fontSize: reference.sizePx,
        textTransform: reference.uppercase ? "uppercase" : "none",
        letterSpacing: reference.uppercase ? "0.08em" : undefined,
        textAlign: text.align,
        fontWeight: 600,
        flex: "none",
      }}
    >
      {slide.label}
      {reference.showTranslation && !song && <span style={{ opacity: 0.7, marginLeft: "0.6em" }}>{translations}</span>}
    </div>
  );

  const column = (verses: VerseLine[], progress: Reading | null) => (
    <FitColumn key={verses === slide.verses ? "primary" : "secondary"} slideId={slide.id} theme={theme} justify={justify}>
      <p style={{ margin: 0 }}>
        {verses.map((v, i) => (
          <span key={v.verse} className={song ? "sb-line" : undefined}>
            {text.verseNumbers && !song && verses.length > 1 && <sup className="sb-vnum">{v.verse}</sup>}
            <ReadText text={v.text} upTo={progress === null || !text.readColor ? 0 : i < progress.line ? v.text.length : i === progress.line ? progress.offset : 0} color={text.readColor} />
            {song ? null : " "}
          </span>
        ))}
      </p>
    </FitColumn>
  );

  return (
    <div className="sb-content" style={{ padding: pad, gap: reference.sizePx * 0.8 }}>
      {reference.position === "top" && refLabel}
      {slide.secondary ? (
        <div className="sb-columns">
          {column(slide.verses, reading)}
          {column(slide.secondary.verses, null)}
        </div>
      ) : (
        column(slide.verses, reading)
      )}
      {reference.position === "bottom" && refLabel}
    </div>
  );
}

/** One auto-fitted text box. Re-fits only when the slide or theme changes, never on highlight. */
function FitColumn({ slideId, theme, justify, children }: { slideId: string; theme: Theme; justify: string; children: React.ReactNode }) {
  const { text } = theme;
  const ref = useFitText<HTMLDivElement>(text.minPx, text.maxPx, [slideId, theme]);
  return (
    <div
      ref={ref}
      className="sb-text"
      style={{
        justifyContent: justify,
        fontFamily: text.fontFamily,
        color: text.color,
        fontWeight: text.weight,
        lineHeight: text.lineHeight,
        textAlign: text.align,
        textShadow: text.shadow || undefined,
      }}
    >
      {children}
    </div>
  );
}

/** `text` with its first `upTo` characters in the follow-along colour. */
function ReadText({ text, upTo, color }: { text: string; upTo: number; color: string }) {
  if (upTo <= 0) return <>{text}</>;
  return (
    <>
      <span className="sb-read" style={{ color }}>
        {text.slice(0, upTo)}
      </span>
      {text.slice(upTo)}
    </>
  );
}
