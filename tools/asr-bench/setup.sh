#!/bin/bash
# One-time setup: Python tools, the Parakeet model (~460 MB download), and the Whisper runner.
# Whisper models are the ones Hearken already downloaded (Settings → Listening).
set -e
cd "$(dirname "$0")"
[ -d .venv ] || python3 -m venv .venv
.venv/bin/python -m pip install -q --timeout 120 --retries 10 sherpa-onnx numpy soundfile
M=sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8
if [ ! -f models/$M/tokens.txt ]; then
  mkdir -p models
  # Resumes after drops and stalls: a slow connection gets there in the end.
  for i in $(seq 1 50); do
    curl -fsSL -C - --speed-limit 2000 --speed-time 45 -o models/$M.tar.bz2 \
      https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/$M.tar.bz2 && break
    echo "download interrupted, resuming ($i)"; sleep 5
  done
  tar xjf models/$M.tar.bz2 -C models && rm models/$M.tar.bz2
fi
(cd whisper-bench && cargo build --release -q)
echo "ready"
