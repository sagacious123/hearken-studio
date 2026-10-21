"""Cuts a service recording into overlapping 20-second pieces, as the app hears it in pieces.

  python cut_recording.py <recording (any format ffmpeg reads)>   ->  real/<name>/<name>@<start s>.wav
"""
import os, subprocess, sys, wave

HERE = os.path.dirname(os.path.abspath(__file__))
LEN, STEP, RATE = 20, 15, 16000

src = sys.argv[1]
name = os.path.splitext(os.path.basename(src))[0].replace("@", "_")
out = os.path.join(HERE, "real", name)
os.makedirs(out, exist_ok=True)
whole = os.path.join(out, "_whole.wav")
subprocess.run(["ffmpeg", "-y", "-i", src, "-ar", str(RATE), "-ac", "1", "-sample_fmt", "s16", whole],
               check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
with wave.open(whole) as w:
    frames = w.readframes(w.getnframes())
os.remove(whole)
n = 0
for start in range(0, max(1, len(frames) // 2 // RATE - 5), STEP):
    piece = frames[start * RATE * 2:(start + LEN) * RATE * 2]
    with wave.open(os.path.join(out, f"{name}@{start}.wav"), "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(RATE); w.writeframes(piece)
    n += 1
print(f"{n} pieces of {LEN} s in {out}")
