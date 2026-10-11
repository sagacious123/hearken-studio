// Background pictures: bundled samples (original artwork in src/assets/backgrounds) and images
// the church uploads (stored by the Rust side, shown through the asset protocol).
import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import aurora from "../assets/backgrounds/aurora.svg";
import bokeh from "../assets/backgrounds/bokeh.svg";
import calvary from "../assets/backgrounds/calvary.svg";
import dawn from "../assets/backgrounds/dawn.svg";
import lightRays from "../assets/backgrounds/light-rays.svg";
import linen from "../assets/backgrounds/linen.svg";
import mountains from "../assets/backgrounds/mountains.svg";
import nightSky from "../assets/backgrounds/night-sky.svg";
import waves from "../assets/backgrounds/waves.svg";
import { api } from "./ipc";

export const SAMPLES: { id: string; name: string; url: string }[] = [
  { id: "dawn", name: "Dawn", url: dawn },
  { id: "night-sky", name: "Night sky", url: nightSky },
  { id: "calvary", name: "Calvary", url: calvary },
  { id: "mountains", name: "Mountains", url: mountains },
  { id: "aurora", name: "Aurora", url: aurora },
  { id: "waves", name: "Waves", url: waves },
  { id: "light-rays", name: "Light rays", url: lightRays },
  { id: "bokeh", name: "Bokeh", url: bokeh },
  { id: "linen", name: "Linen", url: linen },
];

const uploadUrls = new Map<string, Promise<string | null>>();

function uploadUrl(file: string): Promise<string | null> {
  let p = uploadUrls.get(file);
  if (!p) {
    p = api
      .backgroundPath(file)
      .then((path) => (path ? convertFileSrc(path) : null))
      .catch(() => null);
    uploadUrls.set(file, p);
  }
  return p;
}

/** Forget a deleted upload so it isn't served from the cache. */
export function forgetUpload(file: string) {
  uploadUrls.delete(file);
}

/** CSS-ready URL for a theme's `background.picture`, or null for none / missing. */
export function useBackgroundUrl(picture: string): string | null {
  const sample = picture.startsWith("sample:") ? SAMPLES.find((s) => s.id === picture.slice(7))?.url ?? null : null;
  const [upload, setUpload] = useState<string | null>(null);
  useEffect(() => {
    if (!picture.startsWith("upload:")) {
      setUpload(null);
      return;
    }
    let live = true;
    void uploadUrl(picture.slice(7)).then((u) => live && setUpload(u));
    return () => {
      live = false;
    };
  }, [picture]);
  return sample ?? upload;
}
