# Hearken Studio

Offline, real-time scripture detection and projection. Hearken Studio listens to the sermon, transcribes it on-device with whisper.cpp, spots Bible references as they are spoken, and puts them on a second display. Speech never leaves the computer and the service needs no internet: the church signs in once, then Hearken works offline for up to 30 days at a time.

For how to use it, see the [user manual](https://hearken-site.vercel.app/manual).

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
| ASR (accuracy pass) | `asr/parakeet.rs`, `asr/engine.rs` | NVIDIA Parakeet TDT 0.6B (int8) through sherpa-onnx, linked statically. Both engines sit behind the `SpeechEngine` trait; a model folder loads Parakeet, a `.bin` file Whisper. |
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
- **Show sure detections automatically** is ticked in Settings (off by default).
- The detection is not tentative.
- Its score is at or above the threshold (default 0.90).

Planned references get +0.08 (capped at 0.99). Quote matches need at least 0.92, which in practice means a near-complete recitation. Shorter phrases such as "the Lord is my shepherd" (about 0.70–0.85) appear in the queue as **Possible quote** for the operator to choose. **Same meaning** suggestions never project by themselves. Songs need 0.85 and go up only onto a clear screen or in place of another song, and a scripture never replaces a live song by itself. Everything else waits in the queue for the operator.

### Keyboard shortcuts (operator)

| Key | Action |
|---|---|
| Enter | Send preview to live |
| Esc | Clear the output |
| B | Toggle blackout |
| ← / → | Previous / next verse |
| Double-click a card | Send it straight to live |
| Enter in the search box | First result to preview; Enter again sends it live |
| ⌘/Ctrl + Enter in the passage picker | Send the passage straight to live |

## Service features

| Feature | Where | How it works |
|---|---|---|
| **Follow reading** | Settings → **Follow reading** (on by default) | While a passage is live and being read aloud, the words read so far are highlighted (Style → **Highlight words read**; theme `text.readColor`, `""` turns highlighting off). The next verse goes up as soon as the reader starts it, continuing into the next chapter. |
| **Worship lyrics** | **Songs** tab, Settings → **Follow worship songs** | Add songs by pasting lyrics (first line is the title; parts separated by blank lines, optionally headed "Verse 1", "Chorus"…) or importing .txt files. While listening, the song being sung shows up in the queue. With auto-project on, it goes up by itself on a clear screen or in place of another song. Sections then advance as they're sung; repeated choruses are recognised. Hearken Studio includes no lyrics: import only songs your church is licensed to project. |
| **Singing detection** | Automatic; **♪ Singing** in the Transcript header | Google's YAMNet (Apache 2.0, 16 MB, bundled) runs on every second of audio in pure Rust (tract) and tells singing from speech. A sung line is marked **♪ sung** in the transcript, is never suggested as a quote or same-meaning verse, is ignored as a voice command, and is left out of sermon notes. A sentence counts as sung when music or singing outweighs speech and voices sang in the last 20 s, so a preacher speaking over the keys is still speech. After 20 s of singing with no song recognised and none on screen, Detections suggests adding the song's lyrics. Tested on recordings of hymns, a choir, worship with a band, and sermons in English and Hausa, including a sermon with a band or organ mixed in under it. |
| **CCLI report** | Songs tab footer | CSV of each song used per service day over the last 90 days, with CCLI numbers, for licence reporting. |
| **Sermon plan** | **Plan** tab | Paste the pastor's notes. Every reference in them is lined up ready to show, and gets +0.08 confidence when spoken, so planned verses project faster. Detection cards are marked **in plan**. |
| **Bible library** | Settings → **Bibles** | Download Yoruba, Igbo, Hausa, Twi (Akuapem and Asante), Ewe, Swahili, French (LSG 1910), Spanish (RV 1909), Portuguese (Bíblia Livre) or German (Luther 1912) from eBible.org. Each is about 4–5 MB, takes a few seconds, then works offline: on screen, side by side and on phones. Only translations whose licences allow sharing the unchanged text are listed (public domain, CC BY-SA, CC BY). Book names appear in each language ("Johanu 3:16"), and copyright lines are shown on phones and in recaps. Downloads are kept in `library.db` in the config folder. |
| **Search by meaning** | Settings → **Transcription accuracy** (download, 31 MB) | Finds verses said in other words ("don't worry about tomorrow" → Matthew 6:34). It uses Model2Vec `potion-base-8M` (MIT): static word-piece vectors, with no neural network to run. The 31k WEB verses are indexed at load and searched in milliseconds. Live, a finished sentence with no announced or quoted verse that scores 0.8 or more becomes a **Same meaning** suggestion, which never projects by itself. In the search box, phrases of three or more words get a **By meaning** section. Measured on paraphrases of 20 well-known verses: 15 top result, 17 top five. |
| **More screens** | Top bar **Screens** | Up to three extra screens beside the main output, each on its own display and following what's live. **Stage display:** a confidence monitor with large text (read words dimmed), screen status and the time. **Lower third:** the passage in a band over black or green, for a video switcher's key or a capture card. **Same as main:** another projector or TV. |
| **Side by side** | Top bar **Side by side** | Shows a second translation next to the first, such as KJV with WEB, or English with French for a bilingual service (see `--extra` under Bible data). |
| **Sermon recap and notes** | **Archive** tab → Recap ▾ | Saves a service's recap as **PDF**, **Word (.docx)**, **Word 97–2003 (.doc)** (RTF, which every Word version opens) or a web page to `Documents/Hearken Studio`, and opens it. It holds sermon notes, every scripture shown (with times), other references mentioned, and songs. **Sermon notes** are made on the computer, with no AI service: the transcript's key sentences are ranked (TextRank), grouped under the passages on screen at the time, and listed with recurring themes. Small talk, verses read aloud, singing and whisper's non-speech marks are left out. A transcript too noisy to summarise (music, crowd, a bad microphone) gives no notes rather than nonsense. PDFs embed a subsetted Noto Serif, so Yoruba, Igbo and Hausa letters print. |
| **Recap on phones** | Congregation page → **Recap**; Share panel setting | The service so far, in the reader's own translation, with PDF / Word / .doc downloads. In the Share panel choose scriptures, songs and notes; scriptures and songs only; or off. |
| **Sermon archive** | Archive tab search | Search every past service by passage ("Romans 8") or by words said ("mustard seed"). |
| **Insights** | Archive tab | Per year: services, scriptures shown, Old/New Testament split, most-preached books and passages, and most-sung songs. |
| **Accuracy pass** | Settings → **Transcription accuracy** (an **Accuracy** button also appears in the top bar during a model download or after a speed warning) | A fast bundled model (`tiny.en`/`base.en`) keeps live updates quick. A larger model then re-transcribes each finished sentence with Bible-vocabulary hints, replacing the quick line (marked ✓) and feeding the archive, quotes and references with the better text. **Parakeet** (661 MB, NVIDIA, CC BY 4.0), **Small** (190 MB) or **Large v3 Turbo** (574 MB) are downloaded in the app (resumable, SHA-256 verified) and timed on the computer. **Automatic** prefers Parakeet, then the most accurate Whisper model that decodes a sentence in under half its length: on an Intel MacBook (i5-8279U, CPU), Small keeps up (4.5 s per 10 s of speech) and Turbo doesn't (29 s per 11 s); Turbo is for Apple Silicon or a PC with a graphics card. Parakeet runs on the CPU everywhere, about 7× faster than real time on that MacBook. Replayed through Hearken's own pipeline on four real sermon recordings, Base (live), Small and Parakeet each caught all 31 references; Parakeet was 5–9× faster than real time where Small (beam search) managed 0.9–2×, and transcribed Nigerian preachers more clearly (`tools/asr-bench`). If it falls behind, sentences keep the quick text, never a backlog. **Names and words to listen for** (pastors, church, places) are added to the prompts, along with book names and today's sermon plan. |
| **Style: fonts & backgrounds** | Settings → **Style** (slides in from the left, so the Preview stays in view; Esc closes it) | Start from a preset (Classic Dark, Sunrise, Night sky, Clean light, Bold, Calvary), then change any of it: font (8 bundled OFL families plus every font installed on the computer), weight, colour, size, line spacing, alignment, outline (colour and width), shadow (soft, strong, glow, block), reference style, margins and transition. Backgrounds: colour, 9 bundled samples (original artwork from `scripts/make_backgrounds.py`), or your own uploaded JPEG/PNG/WebP/GIF images (stored in the config folder's `backgrounds/`), with darken and soften sliders. Edits show in Preview until **Apply to screen**. |
| **Phones, livestream, remote** | Top bar **Share** | Serves three pages on the church network, with no internet needed (on port 80 when it's free, so addresses need no port; otherwise 7878): the congregation's phone view (any translation and text size, with a QR code that can be shown on the projector), `/overlay` (a transparent lower third for an OBS/vMix browser source, 1920×1080, `?pos=top`, `&opacity=0.8`, `&scale=1.2`), and `/remote` (previous/next, blackout, clear, and show any detected or planned item; protected by a 4-digit PIN shown in the Share panel). macOS and Windows ask once to allow incoming connections. |

Everything is stored in the app's config folder (`history/`, `songs.json`, `sermon-plan.txt`, `library.db`, `models/`) next to `settings.json`.

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

## Languages

Hearken listens for English, French and Spanish. The first-run screens ask which languages are preached and whether Hearken's own menus switch to one of them; both can be changed under Settings → Language. Choosing a language other than English downloads its Bible (Louis Segond 1910, Reina-Valera 1909) and a multilingual Whisper model of the size the benchmark picked (`tiny` or `base`, 31–60 MB); the accuracy pass uses Parakeet's 25-language version (v3), the multilingual `small` or Large v3 Turbo.

Everything that listens takes its words from `src-tauri/src/scripture/lang.rs`: book names, "chapitre"/"versículo", spoken numbers (including French counting in twenties), ordinals ("Première épître aux Corinthiens", "Primera de Corintios"), little words for quote matching, the Whisper prompt and its hallucinations. Text is compared without accents. Downloaded Bibles are word-indexed, so verses read aloud in French or Spanish are recognised against them. Voice commands have their own French and Spanish phrases in `voice.rs`. Search by meaning is English only.

With several languages, Whisper tells them apart on each finished sentence; partials keep the last sentence's language, because detection costs a second encoder pass. References said in any chosen language are understood whichever language a sentence is decoded in.

Interface text lives in `src/lib/i18n/` (one file per area, keyed by the English text, falling back to English).

To check real recordings: `HEARKEN_MODEL=…/ggml-base-q5_1.bin HEARKEN_LANGS=fr HEARKEN_WAV=a.wav cargo test transcribe_files -- --ignored --nocapture` (16 kHz mono WAV).

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
   1. Bump `version` in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` and `tauri.conf.json`.
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
