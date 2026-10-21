#!/bin/bash
# Runs every model and scores them.
#   ./run.sh synthetic                      made-up clips (built on first run by gen.py)
#   ./run.sh real <recording> <refs.txt>    a real service; refs.txt lists what was said, and when
# Optional: MODELS="small.en turbo parakeet" (default all three), THREADS=4.
set -e
cd "$(dirname "$0")"
THREADS=${THREADS:-4}
MODELS=${MODELS:-"small.en turbo parakeet"}
case "$(uname)" in
  Darwin) APPDATA="$HOME/Library/Application Support/com.hearkenstudio.app/models" ;;
  *) APPDATA="${APPDATA:-$HOME/.local/share/com.hearkenstudio.app/models}" ;;
esac
WHISPER=whisper-bench/target/release/whisper-bench
PARAKEET=models/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8
mkdir -p results

if [ "$1" = real ]; then
  [ -f "$2" ] && [ -f "$3" ] || { echo "usage: ./run.sh real <recording> <refs.txt>"; exit 1; }
  name=$(basename "${2%.*}" | tr @ _)
  python3 cut_recording.py "$2"
  CLIPS=(real/$name/*.wav); TAG="real-$name"
else
  [ -f truth.json ] || python3 gen.py
  CLIPS=(clips/*.wav); TAG="synthetic"
fi

for m in $MODELS; do
  out=results/$TAG-$m.json
  case $m in
    small.en) $WHISPER "$APPDATA/ggml-small.en-q5_1.bin" "Whisper Small (English)" $out $THREADS "${CLIPS[@]}" 2> results/$TAG-$m.log ;;
    turbo)    $WHISPER "$APPDATA/ggml-large-v3-turbo-q5_0.bin" "Whisper Large v3 Turbo" $out $THREADS "${CLIPS[@]}" 2> results/$TAG-$m.log ;;
    parakeet) .venv/bin/python run_parakeet.py $PARAKEET "Parakeet TDT 0.6B v2" $out $THREADS "${CLIPS[@]}" 2> results/$TAG-$m.log ;;
    *) echo "unknown model $m"; exit 1 ;;
  esac
  echo "$m done"
done

if [ "$1" = real ]; then
  python3 score_real.py "$3" results/$TAG-*.json
else
  python3 score.py results/$TAG-*.json
fi
