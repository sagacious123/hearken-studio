# Listening model test

Compares speech models on what matters to Hearken: **how many scripture references they catch**,
how often they offer one that wasn't said, and how fast they run. Not part of the app.

Models compared: Whisper Small (English) and Whisper Large v3 Turbo (the files Hearken already
downloaded, run with the app's prompt and live settings), and NVIDIA Parakeet TDT 0.6B v2 through
sherpa-onnx. Everything runs on the CPU, so speeds compare the models with each other, not with
the app (which uses the GPU for Whisper).

## Setup (once)

Needs Python 3, ffmpeg, Rust, and Hearken installed with both Whisper models downloaded.

```bash
./setup.sh      # Python tools, the Parakeet model (~460 MB), the Whisper runner
```

## Made-up clips

```bash
./run.sh synthetic
```

36 sermon lines full of references (mostly hard books: Habakkuk, Philemon, Zephaniah…), read by
six of the Mac's voices and placed in three rooms: clean, a hall with echo and crowd noise, and the
hall with a keyboard chord underneath. Mac only (it uses `say`). Computer voices are much easier
than a real preacher, so treat the result as "worth testing for real", not "better in church".

First result (2026-10-20, Intel i5, CPU only):

| Model | References caught | Word errors | Speed |
|---|---|---|---|
| Whisper Small (English) | 93% | 1.8% | 0.6× real time |
| Whisper Large v3 Turbo | 96% | 0.9% | 0.1× real time |
| Parakeet TDT 0.6B v2 | 94% | 1.1% | 6× real time |

Then four real recordings (2026-10-20): two Nigerian preachers, a 16-minute Bible talk on prayer,
and a 12-minute morning prayer; 31 references in all, each checked against the verse read after it.
Cut into fixed 20-second pieces (`./run.sh real`), Whisper Small left 67 of 149 pieces empty and
both Whisper models sometimes wrote references nobody said. **That was this cutting, not the app:**
see the next section.

## Through the app itself

`replay_recording` (a test in `src-tauri/src/pipeline.rs`) plays a recording through Hearken's own
listening code: its speech detector and sentence cutter, the live and accuracy passes with their
prompts, and its reference finder. Its results files carry the references the app found, and
`score_real.py` scores those.

```bash
cd src-tauri
HEARKEN_REPLAY=../tools/asr-bench/real/service.wav \
HEARKEN_LIVE=resources/models/ggml-base.en-q5_1.bin \
HEARKEN_ACCURATE=".../ggml-small.en-q5_1.bin"      `# or a Parakeet model folder` \
HEARKEN_OUT=../tools/asr-bench/results/app-service \
cargo test --lib replay_recording -- --ignored --nocapture
python3 ../tools/asr-bench/score_real.py ../tools/asr-bench/real/service.refs.txt ../tools/asr-bench/results/app-service-*.json
```

The recording must be 16 kHz mono WAV (`ffmpeg -i in.mp4 -ar 16000 -ac 1 out.wav`).
`HEARKEN_LIVE_PROMPT` / `HEARKEN_ACCURATE_PROMPT` try other prompts.

Results on the same four recordings (Intel i5, CPU only):

| Pass | References caught | Misheard references | Empty sentences | Speed |
|---|---|---|---|---|
| Whisper Base (live) | 31/31 | 0 | 8 of 388 | 5–10× real time |
| Whisper Small (accuracy, beam search) | 31/31 | 1 | 3 of 388 | 0.9–2× |
| Parakeet TDT 0.6B v2 (accuracy) | 31/31 | 1 | 3 of 388 | 5–9× |

All three catch every reference. Parakeet's edge is speed on a CPU and clearer transcripts of
African accents ("Satan, I rebuke you" where Small wrote "Sitan" and Base "Sitanary, you're
cute"). The extra "quote" and "contextual" cards are the same whichever model listens: they come
from the reference finder.

## A real service

Record a service (from the sound desk, or a phone near a speaker) and list the references that
were said, with roughly when:

```text
# refs.txt: time (mm:ss or h:mm:ss), then the reference
02:15    John 3:16
14:40    1 Corinthians 13:4
1:02:30  Psalm 23:1
```

```bash
./run.sh real service.mp3 refs.txt
MODELS="parakeet small.en" ./run.sh real service.mp3 refs.txt   # skip the slow one
```

The recording is cut into overlapping 20-second pieces, as the app hears it. For each model it
prints references caught and missed (with what it heard instead), and **offered but not said**:
references the model wrote down that nobody said, which would put a wrong verse in front of the
operator. Times only need to be within about 20 seconds.

Only use recordings you have permission to use. Recordings and results stay in this folder and
are not committed.

## Files

- `gen.py`: makes the made-up clips and `truth.json`
- `cut_recording.py`: cuts a real recording into pieces
- `whisper-bench/`: runs a Whisper model the way the app's live pass does
- `run_parakeet.py`: runs Parakeet
- `score.py`, `score_real.py`: the scoring
