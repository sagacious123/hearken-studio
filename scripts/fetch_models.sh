#!/usr/bin/env bash
# Downloads quantized Whisper models (bundled into the installer for offline use) and the
# benchmark clip used by hardware tiering. Safe to re-run; existing files are skipped.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MODELS="$ROOT/src-tauri/resources/models"
BENCH="$ROOT/src-tauri/resources/bench"
HF="https://huggingface.co/ggerganov/whisper.cpp/resolve/main"
mkdir -p "$MODELS" "$BENCH"

fetch() {
  local url="$1" dest="$2" min_bytes="$3"
  if [[ -f "$dest" && $(wc -c <"$dest") -ge $min_bytes ]]; then
    echo "✓ $(basename "$dest")"
    return
  fi
  echo "↓ $(basename "$dest")"
  curl -fL --retry 3 --progress-bar -o "$dest.part" "$url"
  [[ $(wc -c <"$dest.part") -ge $min_bytes ]] || { echo "download too small: $url" >&2; exit 1; }
  mv "$dest.part" "$dest"
}

# q5_1 keeps accuracy within ~1% WER of f16 at ~40% of the size: tiny ≈ 31 MB, base ≈ 57 MB.
fetch "$HF/ggml-tiny.en-q5_1.bin" "$MODELS/ggml-tiny.en-q5_1.bin" 20000000
fetch "$HF/ggml-base.en-q5_1.bin" "$MODELS/ggml-base.en-q5_1.bin" 40000000

# 11 s, 16 kHz mono public-domain speech clip shipped with whisper.cpp.
fetch "https://raw.githubusercontent.com/ggml-org/whisper.cpp/master/samples/jfk.wav" "$BENCH/jfk.wav" 100000
