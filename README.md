# Hearken Studio

Offline, real-time scripture detection and projection. Hearken Studio listens to the sermon, transcribes it on-device with whisper.cpp, spots Bible references as they are spoken, and puts them on a second display. It needs no cloud services, accounts or running costs.

Built with Tauri 2, Rust, React 19, TypeScript and Tailwind 4.

## Architecture

```
┌──────────────────────────── Rust core (src-tauri) ─────────────────────────────┐
│                                                                                 │
│  cpal callback ─ring buffer─▶ DSP thread ───jobs───▶ ASR thread                 │
│  (mono f32,      (lock-free)  • resample → 16 kHz     • whisper.cpp             │
│   no allocs)                  • energy gate + VAD       (Metal / Vulkan / CPU)  │
│                               • utterance segmenter   • L1 regex parser         │
│                               • partial every N ms    • L2 FTS5 quote matcher   │
│                                                       • SQLite verse lookup     │
│                                                       • auto-project gate       │
│                                                              │                  │
│                     Presenter (single source of truth) ◀─────┘                  │
└──────────────┬──────────────────────────────────────────────┬───────────────────┘
               │ events: transcript, detection, status          │ output://state
      ┌────────▼─────────┐                            ┌─────────▼─────────┐
      │ Operator window  │  invoke("present", …) ───▶ │  Output window    │
      │ transcript,      │                            │  1920×1080 stage, │
      │ queue, preview,  │                            │  auto-fit text,   │
      │ search, themes   │                            │  JSON themes      │
      └──────────────────┘                            └───────────────────┘
```

### Latency budget

Latency is measured from the moment the last word of a reference is spoken to the moment the verse appears on screen.

| Stage | Typical | Notes |
|---|---|---|
| Capture + ring + resample | 10–25 ms | Device buffer plus one 1024-frame resampler chunk |
| VAD frame | 30 ms | WebRTC frame size |
| Wait for next partial step | 0–`step` (avg `step`/2) | `step` adapts: 200–300 ms on GPU tiers |
| Whisper partial decode (≤ 5 s window, reduced encoder ctx) | 40–150 ms | `tiny.en`/`base.en` q5_1 on Metal/Vulkan |
| Regex parse + SQLite lookup | < 1 ms | Measured in tests and in the `search_scripture` timer |
| IPC + React commit + fade start | 5–20 ms | Opacity-only animation, composited on the GPU |

On Apple Silicon or a discrete GPU, end-to-end latency is usually **≈ 150–300 ms**. On CPU-only "low" tier machines expect 400–900 ms. The pipeline slows its step to match measured decode time rather than building a backlog.

Each detection card shows its measured capture-to-detection latency, and the header shows the rolling ASR decode time.

**Measured: Intel MacBook (x86_64, 8 cores, 8 GB, 2026-10-09).** 5 s partial window, reduced encoder context, median of 3 runs:

| Model | Metal | CPU (4 threads) |
|---|---|---|
| base.en q5_1 | 1200 ms (first load 38 s compiling shaders) | **488 ms** ← selected (tier `mid`, step 609 ms) |
| tiny.en q5_1 | 1040 ms | 834 ms |

On Intel Macs Metal is slower than CPU, so the benchmark now skips it there. This class of machine lands at about 0.8 s end-to-end. Getting under 300 ms needs Apple Silicon or a discrete GPU.

Reproduce with `cd src-tauri && cargo test --release -- --ignored --nocapture`.

## Getting started

### Prerequisites

- **All platforms:** Rust (stable, 1.80 or newer), Node 20+, CMake and Python 3.
- **macOS:** Xcode Command Line Tools. Metal is used automatically.
- **Windows:** the [Vulkan SDK](https://vulkan.lunarg.com/) for building, and the WebView2 runtime, which ships with Windows 11.
- **Linux:** `libwebkit2gtk-4.1-dev libasound2-dev libvulkan-dev glslc patchelf`.

### Setup

```bash
npm install
npm run models          # downloads tiny.en + base.en (q5_1) and the benchmark clip (~90 MB)
npm run bible:fetch     # downloads KJV, ASV, BBE, WEB, YLT into data/translations (~23 MB)
npm run bible:build     # builds src-tauri/resources/bible.db (~39 MB, about 5 s)
npm run tauri dev
```

The first time you click **Start listening**, the app runs the hardware benchmark (about 5–15 s) and caches the result.

### Tests

```bash
cd src-tauri && cargo test     # parser, number normalisation, segmenter, FTS query sanitising
```

## Modules

| Module | File(s) | Responsibility |
|---|---|---|
| Capture | `audio/capture.rs` | Lists devices (plus WASAPI loopback on Windows) and runs a real-time-safe callback that downmixes into a lock-free ring buffer. |
| DSP | `audio/{resample,vad,segmenter}.rs` | Resamples to 16 kHz (rubato FFT), applies an adaptive noise-floor gate plus WebRTC VAD, then segments utterances with pre-roll, hangover, a 12 s force-cut and a 1.5 s overlap. |
| ASR | `asr/whisper.rs` | whisper-rs engine with GPU→CPU fallback, a domain prompt, reduced `audio_ctx`, no temperature fallback on partials, and a hallucination filter. |
| Tiering | `asr/hardware.rs` | Detects specs, then benchmarks `base.en` before `tiny.en` and GPU before CPU against a 200 ms budget for a 5 s window. Results are cached per app version. |
| Pipeline | `pipeline.rs` | Thread orchestration, partial/final job coalescing, adaptive step, dedupe, and the auto-project gate. |
| Parser (L1) | `scripture/{books,numbers,parser}.rs` | Turns spoken numbers into digits, matches a single alias regex, validates against versification, splits run-together digits ("John 316" → 3:16), and resolves follow-ups ("verse 18"). Misheard book names followed by chapter and verse ("Philippines 4:13", "Romance 8 28") are matched by sound (`scripture/fuzzy.rs`). Matches at the end of a partial are marked tentative. |
| Store | `scripture/db.rs` | Read-only, memory-mapped SQLite with prepared-statement cache, indexed lookups and FTS5/BM25 search. |
| Quotes (L2) | `scripture/quote.rs` | Recognises verses quoted or read aloud without an announcement. Every three-word window of speech is an FTS5 phrase query across all translations. Candidates are aligned word by word, allowing for an occasional misheard word, so "pray without seizing" still finds 1 Thessalonians 5:17. Matching is stemmed so "believeth" matches "believes", and words that sound alike count as near-matches. Up to three possible matches are offered, and phrases found in many verses are ignored. |
| Follow-along | `follow.rs` | Highlights how far the live passage or song has been read or sung, and moves to the next verse or section when it starts. Also recognises which imported song is being sung. |
| Songs | `songs.rs` | Song library parsed from pasted lyrics (SongSelect format or plain text). Ships no lyrics. |
| History | `history.rs` | Per-day log of what was shown, said and detected. Recaps, the sermon archive, insights and the CCLI report read it. |
| Sharing | `share.rs`, `share/*.html` | Local-network server for congregation phones, the OBS/vMix overlay and the phone remote. |
| Presenter | `presentation.rs` | Owns the output state and broadcasts it, and manages monitors and the borderless output window. |
| Operator UI | `src/dashboard/*` | Transcript, detection queue, preview/live mirrors, search, JSON theme editor and keyboard shortcuts. |
| Output UI | `src/output/*` | Separate Vite entry. Renders a fixed 1920×1080 stage scaled to any display, with binary-search font fitting and crossfades. |

### Confidence and auto-projection

Each detection gets a score:

- Base score: **0.95** for a chapter and verse however Whisper writes them ("John 3:16", "John 3 16", "John three sixteen", "John 3-16"), **0.70** for chapter only. Saying "chapter" and "verse" isn't needed.
- Penalties:
  - Ambiguous book names ("Mark", "Job", "Acts"…) with no "chapter" or "verse" keyword: −0.25 with chapter only, −0.10 with a verse number.
  - Run-together digits ("John 316"): −0.10.
  - A book name recognised only by sound ("Philippines"): −0.20, so these always wait for the operator.
- The result is multiplied by `0.7 + 0.3 × Whisper token confidence`.

A detection is auto-projected only when all of these hold:
- Auto-project is on.
- The detection is not tentative.
- Its score is at or above the threshold (default 0.90).

Quote matches need at least 0.92, which in practice means a near-complete recitation. Shorter phrases such as "the Lord is my shepherd" (about 0.70–0.85) appear in the queue as **Possible quote** for the operator to choose. Everything else waits in the queue for the operator.

### Keyboard shortcuts (operator)

| Key | Action |
|---|---|
| Enter | Send preview to live |
| Esc | Clear the output |
| B | Toggle blackout |
| ← / → | Previous / next verse |
| Double-click a card | Send it straight to live |

## Service features

| Feature | Where | How it works |
|---|---|---|
| **Follow reading** | Top bar switch (on by default) | While a passage is live and being read aloud, the words read so far are highlighted (theme `text.readColor`; set it to `""` to turn highlighting off). The next verse goes up as soon as the reader starts it, continuing into the next chapter. |
| **Worship lyrics** | **Songs** tab, top bar **Lyrics** switch | Add songs by pasting lyrics (first line is the title; parts separated by blank lines, optionally headed "Verse 1", "Chorus"…) or importing .txt files. While listening, the song being sung shows up in the queue. With auto-project on, it goes up by itself on a clear screen or in place of another song. Sections then advance as they're sung; repeated choruses are recognised. Hearken Studio includes no lyrics: import only songs your church is licensed to project. |
| **CCLI report** | Songs tab footer | CSV of each song used per service day over the last 90 days, with CCLI numbers, for licence reporting. |
| **Sermon plan** | **Plan** tab | Paste the pastor's notes. Every reference in them is lined up ready to show, and gets +0.08 confidence when spoken, so planned verses project faster. Detection cards show a **Planned** badge. |
| **Side by side** | Top bar **Side by side** | Shows a second translation next to the first, such as KJV with WEB, or English with French for a bilingual service (see `--extra` under Bible data). |
| **Sermon recap** | **Archive** tab → Recap | Writes a printable page of every scripture shown (with times), other references mentioned, and songs to `Documents/Hearken Studio`, and opens it. Print it to PDF or share it. |
| **Sermon archive** | Archive tab search | Search every past service by passage ("Romans 8") or by words said ("mustard seed"). |
| **Insights** | Archive tab | Per year: services, scriptures shown, Old/New Testament split, most-preached books and passages, and most-sung songs. |
| **Accuracy pass** | **Accuracy** button in the top bar | A fast bundled model (`tiny.en`/`base.en`) keeps live updates quick. A larger model then re-transcribes each finished sentence with Bible-vocabulary hints, replacing the quick line (marked ✓) and feeding the archive, quotes and references with the better text. **Small** (190 MB) or **Large v3 Turbo** (574 MB) are downloaded in the app (resumable, SHA-256 verified) and timed on the computer. **Automatic** uses the most accurate one that decodes a sentence in under half its length: on an Intel MacBook (i5-8279U, CPU), Small keeps up (4.5 s per 10 s of speech) and Turbo doesn't (29 s per 11 s); Turbo is for Apple Silicon or a PC with a graphics card. If it falls behind, sentences keep the quick text, never a backlog. **Names and words to listen for** (pastors, church, places) are added to the prompts, along with book names and today's sermon plan. |
| **Style: fonts & backgrounds** | **Style** button in the top bar (slides in from the left, so the Preview stays in view; Esc closes it) | Start from a preset (Classic Dark, Sunrise, Night sky, Clean light, Bold, Calvary), then change any of it: font (8 bundled OFL families plus system fonts), weight, colour, size, line spacing, alignment, shadow, reference style, margins and transition. Backgrounds: colour, 9 bundled samples (original artwork from `scripts/make_backgrounds.py`), or your own uploaded JPEG/PNG/WebP/GIF images (stored in the config folder's `backgrounds/`), with darken and soften sliders. Edits show in Preview until **Apply to screen**. |
| **Phones, livestream, remote** | Top bar **Share** | Serves three pages on the church network, with no internet needed (default port 7878): the congregation's phone view (any translation and text size, with a QR code that can be shown on the projector), `/overlay` (a transparent lower third for an OBS/vMix browser source, 1920×1080, `?pos=top`, `&opacity=0.8`, `&scale=1.2`), and `/remote` (previous/next, blackout, clear, and show any detected or planned item; protected by a 4-digit PIN shown in the Share panel). macOS and Windows ask once to allow incoming connections. |

Everything is stored in the app's config folder (`history/`, `songs.json`, `sermon-plan.txt`) next to `settings.json`.

## Bible data

```bash
npm run bible:fetch     # scripts/fetch_bibles.py → data/translations/<CODE>.csv + <CODE>.json
npm run bible:build     # scripts/build_bible_db.py → src-tauri/resources/bible.db
npm run bible:sample    # alternative: 8-verse KJV+WEB database for quick UI work
```

`fetch_bibles.py` takes KJV, ASV, BBE and YLT from [scrollmapper/bible_databases](https://github.com/scrollmapper/bible_databases) (pinned to a commit) and the WEB from [eBible.org](https://ebible.org/find/details.php?id=engwebp) (Protestant canon). It fixes display artefacts without changing any wording: YLT's backtick quotes, stray `[` before "Selah" in the ASV, and en dashes in KJV names. BBE's `***` marks text its translators left unrendered and is kept.

To add another translation, put a CSV in `data/translations/` with columns `book,chapter,verse,text`, plus an optional `<CODE>.json` with `{"name", "license"}`, then run `npm run bible:build`. Books can be numbers (1–66) or names.

**Licensing:** KJV, WEB, ASV, YLT and BBE are public domain. In the UK, the KJV is under Crown copyright (letters patent), although that is rarely enforced for software. NIV, ESV, NKJV, NLT and others need a licence from the publisher before you can redistribute them.

For a bilingual service, add another language, then rebuild:

```bash
python3 scripts/fetch_bibles.py --extra FreJND      # French (Darby 1885). Also: GerElb1905 (German 1905), SpaRV1865 (Spanish 1865)
npm run bible:build
```

Any other scrollmapper file name also works with `--extra`, but check its licence first.

The five translations come to 155,474 verses and 38.8 MB, including the FTS index. KJV loads first and defines versification. Verses that a translation omits return no slide rather than an error. These are Matthew 17:21, Acts 8:37 and others in ASV/BBE/WEB, plus the Romans doxology, which WEB places at 14:24–26.

## Auto-updates via GitHub Releases (free)

1. **Generate the updater signing key.** Do this once, and keep the key out of git.
   ```bash
   npm run tauri signer generate -- -w ~/.tauri/hearkenstudio.key
   ```
2. **Configure `src-tauri/tauri.conf.json`:**
   - Paste the contents of `~/.tauri/hearkenstudio.key.pub` into `plugins.updater.pubkey`.
   - Replace `OWNER/REPO` in the endpoint URL.
3. **Add repo secrets:** `TAURI_SIGNING_PRIVATE_KEY` (the file contents) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
4. **Release:**
   1. Bump `version` in `tauri.conf.json` and `Cargo.toml`.
   2. Run `git tag v0.2.0 && git push --tags`.
   3. CI builds every platform and uploads the installers, `.sig` files and `latest.json` to a **draft** release.
   4. Publish the draft. Installed apps see the update on their next check.

The app checks 5 s after launch. It never installs on its own: the operator clicks **Install & restart**, which is disabled while listening.

**What's free and what isn't:** GitHub Releases hosting and Tauri's minisign update signatures cost nothing. OS code signing is separate and paid:
- Apple Developer ID costs $99/yr and removes the Gatekeeper prompt.
- A Windows code-signing certificate removes SmartScreen warnings.

Unsigned builds work, but users must approve the first launch.

## Known limitations and next steps

- **Windows/Linux Vulkan loader:** the Vulkan build links against `vulkan-1.dll` / `libvulkan.so.1`. Every GPU driver install provides it, but a bare VM may not.
  - Linux packages declare it as a dependency.
  - On Windows, consider bundling the redistributable loader next to the exe. With no driver present, the app then falls back to CPU instead of failing to start.
- **System audio on macOS:** this needs a virtual device such as BlackHole. In practice, a USB feed from the sound desk gives the cleanest input.
- **VAD:** WebRTC VAD is light and fast but can trigger on worship music. `SpeechDetector` is a single-method seam if you want to swap in Silero (ONNX).
- **Versification:** validation uses the first translation's verse counts. Translations with different verse splits (for example Psalm titles) still resolve, but the edge verses may differ.
