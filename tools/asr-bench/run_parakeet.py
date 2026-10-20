"""Runs Parakeet (through sherpa-onnx) over wav files and writes a results file for the scorers.

  .venv/bin/python run_parakeet.py <model dir> <name> <out.json> <threads> <clip.wav>...
"""
import glob, json, os, sys, time
import sherpa_onnx, soundfile as sf

d, name, out, threads, wavs = sys.argv[1], sys.argv[2], sys.argv[3], int(sys.argv[4]), sys.argv[5:]
pick = lambda stem: sorted(glob.glob(os.path.join(d, f"{stem}*.onnx")))[0]
rec = sherpa_onnx.OfflineRecognizer.from_transducer(
    encoder=pick("encoder"), decoder=pick("decoder"), joiner=pick("joiner"),
    tokens=os.path.join(d, "tokens.txt"), num_threads=threads, model_type="nemo_transducer")
res, secs, audio_secs = {}, 0.0, 0.0
for wav in wavs:
    audio, sr = sf.read(wav, dtype="float32")
    audio_secs += len(audio) / sr
    t = time.time()
    s = rec.create_stream()
    s.accept_waveform(sr, audio)
    rec.decode_stream(s)
    secs += time.time() - t
    key = os.path.basename(wav)[:-4]
    res[key] = s.result.text.strip()
    print(f"{key}: {res[key]}", file=sys.stderr)
json.dump({"model": name, "seconds": secs, "audio_seconds": audio_secs, "out": res}, open(out, "w"), indent=1)
