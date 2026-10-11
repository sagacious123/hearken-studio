import { StrictMode, useEffect, useMemo, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { createRoot } from "react-dom/client";
import { api, events, type OutputState } from "../lib/ipc";
import { parseTheme } from "../lib/theme";
import { SlideView } from "./SlideView";
import { Stage } from "./Stage";

/**
 * Projector window. Stateless by design: Rust owns OutputState, so this window can be
 * closed, reopened or reloaded mid-service and immediately shows what's live.
 */
function OutputApp() {
  const [state, setState] = useState<OutputState | null>(null);

  useEffect(() => {
    let revision = -1;
    const apply = (s: OutputState) => {
      if (s.revision <= revision) return; // drop out-of-order snapshots
      revision = s.revision;
      setState(s);
    };
    // Subscribe first, then fetch, so nothing emitted in between is missed.
    const unlisten = events.outputState(apply);
    api.getOutputState().then(apply).catch(console.error);
    return () => void unlisten.then((f) => f());
  }, []);

  // Escape hatch: a borderless window covering the only display would otherwise trap the operator.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void getCurrentWindow().close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Keyed on content: output events (e.g. follow-along highlights) carry a fresh theme object
  // each time, and a new theme would refit the text on every one.
  const themeJson = JSON.stringify(state?.theme ?? null);
  const theme = useMemo(() => parseTheme(JSON.parse(themeJson)), [themeJson]);

  return (
    <Stage background={theme.background.color}>
      <SlideView slide={state?.slide ?? null} theme={theme} blackout={state?.blackout ?? false} reading={state?.reading} qr={state?.qr} />
    </Stage>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <OutputApp />
  </StrictMode>,
);
