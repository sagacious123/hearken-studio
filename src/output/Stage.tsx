import { useLayoutEffect, useRef, useState, type ReactNode } from "react";

export const STAGE_W = 1920;
export const STAGE_H = 1080;

/**
 * Renders children on a fixed 1920×1080 logical stage, uniformly scaled (letterboxed) into
 * whatever box it's given. Themes are authored once in stage pixels and look identical on a
 * 720p projector, a 4K LED wall, and the dashboard's thumbnail preview.
 */
export function Stage({ children, background = "#000" }: { children: ReactNode; background?: string }) {
  const boxRef = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(0);

  useLayoutEffect(() => {
    const box = boxRef.current;
    if (!box) return;
    const update = () => setScale(Math.min(box.clientWidth / STAGE_W, box.clientHeight / STAGE_H));
    update();
    const ro = new ResizeObserver(update);
    ro.observe(box);
    return () => ro.disconnect();
  }, []);

  return (
    <div ref={boxRef} style={{ position: "relative", width: "100%", height: "100%", overflow: "hidden", background }}>
      {scale > 0 && (
        <div
          style={{
            position: "absolute",
            width: STAGE_W,
            height: STAGE_H,
            left: "50%",
            top: "50%",
            transform: `translate(-50%, -50%) scale(${scale})`,
            transformOrigin: "center",
          }}
        >
          {children}
        </div>
      )}
    </div>
  );
}
