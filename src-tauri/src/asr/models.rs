//! Speech model catalogue. Whisper's `tiny.en` and `base.en` ship in the installer (fast, live
//! updates); the larger, more accurate models used for the accuracy pass (Whisper Small and
//! Turbo, and Parakeet), and the multilingual ones for preaching in other languages, are
//! downloaded on demand into the app's data folder, so the installer stays small. Downloads
//! resume after interruption and every file is checked against its published SHA-256 before use.

use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering::Relaxed},
};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::scripture::lang::Lang;

const BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Whisper,
    Parakeet,
}

/// One file of a model kept as a folder of files.
pub struct Part {
    pub file: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

pub struct ModelInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub engine: Engine,
    /// Whisper: the model file. Parakeet: the folder its `parts` go in.
    pub file: &'static str,
    /// The whole download.
    pub bytes: u64,
    /// Whisper's one file ("" for bundled ones); folders check each part instead.
    pub sha256: &'static str,
    pub parts: &'static [Part],
    /// Where the files are downloaded from.
    pub source: &'static str,
    /// Shipped in the installer (never downloaded or deleted).
    pub bundled: bool,
    /// Usable for the accuracy pass.
    pub accuracy: bool,
    pub note: &'static str,
}

pub const CATALOG: &[ModelInfo] = &[
    ModelInfo {
        id: "tiny.en",
        label: "Tiny (English)",
        engine: Engine::Whisper,
        file: "ggml-tiny.en-q5_1.bin",
        bytes: 32_166_155,
        sha256: "",
        parts: &[],
        source: BASE_URL,
        bundled: true,
        accuracy: false,
        note: "Fastest; live updates on slow computers.",
    },
    ModelInfo {
        id: "base.en",
        label: "Base (English)",
        engine: Engine::Whisper,
        file: "ggml-base.en-q5_1.bin",
        bytes: 59_721_011,
        sha256: "",
        parts: &[],
        source: BASE_URL,
        bundled: true,
        accuracy: false,
        note: "Live updates on most computers.",
    },
    ModelInfo {
        id: "tiny",
        label: "Tiny (all languages)",
        engine: Engine::Whisper,
        file: "ggml-tiny-q5_1.bin",
        bytes: 32_152_673,
        sha256: "818710568da3ca15689e31a743197b520007872ff9576237bda97bd1b469c3d7",
        parts: &[],
        source: BASE_URL,
        bundled: false,
        accuracy: false,
        note: "Live updates in other languages on slow computers.",
    },
    ModelInfo {
        id: "base",
        label: "Base (all languages)",
        engine: Engine::Whisper,
        file: "ggml-base-q5_1.bin",
        bytes: 59_707_625,
        sha256: "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
        parts: &[],
        source: BASE_URL,
        bundled: false,
        accuracy: false,
        note: "Live updates in other languages, such as French and Spanish.",
    },
    ModelInfo {
        id: "small.en",
        label: "Small (English)",
        engine: Engine::Whisper,
        file: "ggml-small.en-q5_1.bin",
        bytes: 190_098_681,
        sha256: "bfdff4894dcb76bbf647d56263ea2a96645423f1669176f4844a1bf8e478ad30",
        parts: &[],
        source: BASE_URL,
        bundled: false,
        accuracy: true,
        note: "Clearly more accurate than Base. Fast enough on any recent computer.",
    },
    ModelInfo {
        id: "small",
        label: "Small (all languages)",
        engine: Engine::Whisper,
        file: "ggml-small-q5_1.bin",
        bytes: 190_085_487,
        sha256: "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
        parts: &[],
        source: BASE_URL,
        bundled: false,
        accuracy: true,
        note: "Small for sermons in other languages. Fast enough on any recent computer.",
    },
    ModelInfo {
        id: "large-v3-turbo",
        label: "Large v3 Turbo",
        engine: Engine::Whisper,
        file: "ggml-large-v3-turbo-q5_0.bin",
        bytes: 574_041_195,
        sha256: "394221709cd5ad1f40c46e6031ca61bce88931e6e088c188294c6d5a55ffa7e2",
        parts: &[],
        source: BASE_URL,
        bundled: false,
        accuracy: true,
        note: "Most accurate, best with accents. Needs Apple Silicon or a graphics card.",
    },
    ModelInfo {
        id: "parakeet.en",
        label: "Parakeet (English)",
        engine: Engine::Parakeet,
        file: "parakeet-tdt-0.6b-v2-int8",
        bytes: 661_190_513,
        sha256: "",
        parts: &[
            Part { file: "encoder.int8.onnx", bytes: 652_184_296, sha256: "a32b12d17bbbc309d0686fbbcc2987b5e9b8333a7da83fa6b089f0a2acd651ab" },
            Part { file: "decoder.int8.onnx", bytes: 7_257_753, sha256: "b6bb64963457237b900e496ee9994b59294526439fbcc1fecf705b31a15c6b4e" },
            Part { file: "joiner.int8.onnx", bytes: 1_739_080, sha256: "7946164367946e7f9f29a122407c3252b680dbae9a51343eb2488d057c3c43d2" },
            Part { file: "tokens.txt", bytes: 9_384, sha256: "ec182b70dd42113aff6c5372c75cac58c952443eb22322f57bbd7f53977d497d" },
        ],
        source: "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8/resolve/main",
        bundled: false,
        accuracy: true,
        note: "As accurate as Turbo on sermons, and fast on any computer: no graphics card needed. By NVIDIA (CC BY 4.0).",
    },
    ModelInfo {
        id: "parakeet",
        label: "Parakeet (25 languages)",
        engine: Engine::Parakeet,
        file: "parakeet-tdt-0.6b-v3-int8",
        bytes: 670_478_772,
        sha256: "",
        parts: &[
            Part { file: "encoder.int8.onnx", bytes: 652_184_281, sha256: "acfc2b4456377e15d04f0243af540b7fe7c992f8d898d751cf134c3a55fd2247" },
            Part { file: "decoder.int8.onnx", bytes: 11_845_275, sha256: "179e50c43d1a9de79c8a24149a2f9bac6eb5981823f2a2ed88d655b24248db4e" },
            Part { file: "joiner.int8.onnx", bytes: 6_355_277, sha256: "3164c13fc2821009440d20fcb5fdc78bff28b4db2f8d0f0b329101719c0948b3" },
            Part { file: "tokens.txt", bytes: 93_939, sha256: "d58544679ea4bc6ac563d1f545eb7d474bd6cfa467f0a6e2c1dc1c7d37e3c35d" },
        ],
        source: "https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8/resolve/main",
        bundled: false,
        accuracy: true,
        note: "Parakeet for sermons in French, Spanish and other European languages. Fast on any computer. By NVIDIA (CC BY 4.0).",
    },
];

pub fn info(id: &str) -> Option<&'static ModelInfo> {
    CATALOG.iter().find(|m| m.id == id)
}

/// Whether `langs` need a multilingual model (English-only models are faster and better in English).
pub fn needs_multilingual(langs: &[Lang]) -> bool {
    langs.iter().any(|l| *l != Lang::En)
}

/// The model to use for `langs` in place of `id`: the multilingual one of the same size when a
/// language other than English is preached ("base.en" → "base").
pub fn for_languages(id: &str, langs: &[Lang]) -> &'static str {
    let wanted = if needs_multilingual(langs) { id.strip_suffix(".en").unwrap_or(id) } else { id };
    info(wanted).map(|m| m.id).unwrap_or(if needs_multilingual(langs) { "base" } else { "base.en" })
}

/// Live models that can listen for `langs`, the one sized for this computer (`benchmarked`) first.
pub fn live_choices(benchmarked: &str, langs: &[Lang]) -> Vec<&'static str> {
    let first = for_languages(benchmarked, langs);
    let others: &[&str] = if needs_multilingual(langs) { &["base", "tiny"] } else { &["base.en", "tiny.en"] };
    std::iter::once(first).chain(others.iter().copied().filter(|m| *m != first)).collect()
}

/// The file (or, for Parakeet, the complete folder) for `id` in any of `dirs` (bundled
/// resources first, then downloads).
pub fn find(dirs: &[&Path], id: &str) -> Option<PathBuf> {
    if let Some(m) = info(id).filter(|m| !m.parts.is_empty()) {
        return dirs.iter().map(|d| d.join(m.file)).find(|p| m.parts.iter().all(|part| p.join(part.file).is_file()));
    }
    let names: Vec<String> = match info(id) {
        Some(m) => vec![m.file.to_owned()],
        None => vec![format!("ggml-{id}-q5_1.bin"), format!("ggml-{id}.bin")],
    };
    dirs.iter().flat_map(|d| names.iter().map(move |n| d.join(n))).find(|p| p.is_file())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub id: &'static str,
    pub label: &'static str,
    pub size_mb: u64,
    pub bundled: bool,
    pub accuracy: bool,
    pub installed: bool,
    pub note: &'static str,
}

pub fn statuses(dirs: &[&Path]) -> Vec<ModelStatus> {
    CATALOG
        .iter()
        .map(|m| ModelStatus {
            id: m.id,
            label: m.label,
            size_mb: m.bytes / 1_000_000,
            bundled: m.bundled,
            accuracy: m.accuracy,
            installed: find(dirs, m.id).is_some(),
            note: m.note,
        })
        .collect()
}

/// Download `id` into `dir`, resuming a partial download. Calls `progress(received, total)`
/// as bytes arrive; stops with an error when `cancel` is set.
pub fn download(id: &str, dir: &Path, progress: impl Fn(u64, u64), cancel: &AtomicBool) -> Result<PathBuf> {
    let Some(m) = info(id).filter(|m| !m.bundled) else { bail!("{id} can't be downloaded") };
    fs::create_dir_all(dir).context("creating the models folder")?;
    let dest = dir.join(m.file);
    if m.parts.is_empty() {
        fetch(&format!("{}/{}", m.source, m.file), &dest, m.bytes, m.sha256, m.label, |n| progress(n, m.bytes), cancel)?;
    } else {
        // A folder: each file in turn, so an interrupted download resumes where it stopped.
        fs::create_dir_all(&dest).context("creating the model's folder")?;
        let mut done = 0;
        for part in m.parts {
            fetch(&format!("{}/{}", m.source, part.file), &dest.join(part.file), part.bytes, part.sha256, m.label, |n| progress(done + n, m.bytes), cancel)?;
            done += part.bytes;
        }
    }
    progress(m.bytes, m.bytes);
    Ok(dest)
}

/// One file from `url` to `dest`, through `dest.part` (resumed if it's there), checked against
/// `sha256` before it takes its place.
fn fetch(url: &str, dest: &Path, bytes: u64, sha256: &str, label: &str, progress: impl Fn(u64), cancel: &AtomicBool) -> Result<()> {
    if dest.is_file() {
        return Ok(());
    }
    let part = dest.with_file_name(format!("{}.part", dest.file_name().unwrap_or_default().to_string_lossy()));
    let mut have = fs::metadata(&part).map(|md| md.len()).unwrap_or(0);
    if have > bytes {
        fs::remove_file(&part).ok();
        have = 0;
    }

    if have < bytes {
        let mut req = ureq::get(url).set("User-Agent", "HearkenStudio");
        if have > 0 {
            req = req.set("Range", &format!("bytes={have}-"));
        }
        let resp = req.call().with_context(|| format!("downloading {label}"))?;
        if have > 0 && resp.status() != 206 {
            have = 0; // server ignored the range: start over
        }
        let mut out = fs::OpenOptions::new().create(true).write(true).append(have > 0).truncate(have == 0).open(&part)?;
        let mut reader = resp.into_reader();
        let mut buf = vec![0u8; 1 << 16];
        let mut last_report = 0u64;
        loop {
            if cancel.load(Relaxed) {
                bail!("download cancelled");
            }
            let n = reader.read(&mut buf).context("download interrupted; it will resume next time")?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])?;
            have += n as u64;
            if have - last_report >= 1 << 20 || have == bytes {
                last_report = have;
                progress(have);
            }
        }
        out.flush()?;
    }

    if have != bytes {
        bail!("download incomplete ({have} of {bytes} bytes); try again to resume");
    }
    if sha256_file(&part)? != sha256 {
        fs::remove_file(&part).ok();
        bail!("the downloaded file didn't match its checksum and was removed; please try again");
    }
    fs::rename(&part, dest)?;
    Ok(())
}

/// Remove a downloaded model (bundled ones are never touched).
pub fn delete(id: &str, dir: &Path) -> Result<()> {
    let Some(m) = info(id).filter(|m| !m.bundled) else { bail!("{id} is built in") };
    if !m.parts.is_empty() {
        let folder = dir.join(m.file);
        if folder.exists() {
            fs::remove_dir_all(&folder).with_context(|| format!("removing {}", folder.display()))?;
        }
        return Ok(());
    }
    for f in [dir.join(m.file), dir.join(format!("{}.part", m.file))] {
        if f.exists() {
            fs::remove_file(&f).with_context(|| format!("removing {}", f.display()))?;
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut f = fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_models_across_folders_and_by_catalog_name() {
        let root = std::env::temp_dir().join(format!("sb-models-{}", std::process::id()));
        let (bundled, downloads) = (root.join("bundled"), root.join("downloads"));
        fs::create_dir_all(&bundled).unwrap();
        fs::create_dir_all(&downloads).unwrap();
        fs::write(bundled.join("ggml-base.en-q5_1.bin"), b"x").unwrap();
        fs::write(downloads.join("ggml-large-v3-turbo-q5_0.bin"), b"x").unwrap();
        let dirs = [bundled.as_path(), downloads.as_path()];
        assert!(find(&dirs, "base.en").unwrap().starts_with(&bundled));
        assert!(find(&dirs, "large-v3-turbo").unwrap().starts_with(&downloads)); // q5_0, not q5_1
        assert!(find(&dirs, "small.en").is_none());
        let s = statuses(&dirs);
        assert!(s.iter().find(|m| m.id == "large-v3-turbo").unwrap().installed);
        assert!(download("base.en", &downloads, |_, _| {}, &AtomicBool::new(false)).is_err()); // bundled
        assert!(delete("tiny.en", &downloads).is_err());
        delete("large-v3-turbo", &downloads).unwrap();
        assert!(find(&dirs, "large-v3-turbo").is_none());
        // Parakeet is a folder, found only when every file is in it, and removed whole.
        let folder = downloads.join("parakeet-tdt-0.6b-v2-int8");
        fs::create_dir_all(&folder).unwrap();
        for p in &info("parakeet.en").unwrap().parts[1..] {
            fs::write(folder.join(p.file), b"x").unwrap();
        }
        assert!(find(&dirs, "parakeet.en").is_none());
        fs::write(folder.join("encoder.int8.onnx"), b"x").unwrap();
        assert_eq!(find(&dirs, "parakeet.en"), Some(folder.clone()));
        delete("parakeet.en", &downloads).unwrap();
        assert!(!folder.exists());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn other_languages_use_multilingual_models() {
        let (en, fr) = ([Lang::En], [Lang::En, Lang::Fr]);
        assert_eq!(for_languages("base.en", &en), "base.en");
        assert_eq!(for_languages("base.en", &fr), "base");
        assert_eq!(for_languages("tiny.en", &[Lang::Es]), "tiny");
        assert_eq!(for_languages("small.en", &fr), "small");
        assert_eq!(for_languages("large-v3-turbo", &fr), "large-v3-turbo");
        assert_eq!(live_choices("tiny.en", &fr), ["tiny", "base"]);
        assert_eq!(live_choices("base.en", &en), ["base.en", "tiny.en"]);
        let checked = |m: &ModelInfo| if m.parts.is_empty() { m.sha256.len() == 64 } else { m.parts.iter().all(|p| p.sha256.len() == 64) };
        assert!(CATALOG.iter().filter(|m| !m.bundled).all(checked), "downloads are checked");
        let whole = |m: &ModelInfo| m.parts.is_empty() || m.parts.iter().map(|p| p.bytes).sum::<u64>() == m.bytes;
        assert!(CATALOG.iter().all(whole), "a folder's size is its files'");
        assert_eq!(for_languages("parakeet.en", &fr), "parakeet");
        assert_eq!(for_languages("parakeet.en", &en), "parakeet.en");
    }

    #[test]
    fn checksum_matches_known_value() {
        let p = std::env::temp_dir().join(format!("sb-sha-{}", std::process::id()));
        fs::write(&p, b"abc").unwrap();
        assert_eq!(sha256_file(&p).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        fs::remove_file(&p).ok();
    }
}
