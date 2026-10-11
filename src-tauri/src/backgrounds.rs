//! Uploaded background images. Copied into `<config>/backgrounds/` and shown through Tauri's
//! asset protocol (scoped to that folder), so themes only carry a short "upload:<file>"
//! reference, not the image itself, which would otherwise ride along on every output update.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;

/// Projector backgrounds rarely need more; this also keeps a stray RAW photo out.
const MAX_BYTES: usize = 30 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadedBackground {
    /// File name inside the backgrounds folder; themes refer to it as "upload:<file>".
    pub file: String,
    /// Original name, for the gallery.
    pub name: String,
    /// Absolute path, for the asset protocol.
    pub path: String,
}

/// Image type from the file's first bytes, never from its name.
fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    match bytes {
        [0xFF, 0xD8, 0xFF, ..] => Some("jpg"),
        [0x89, b'P', b'N', b'G', ..] => Some("png"),
        [b'G', b'I', b'F', b'8', ..] => Some("gif"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some("webp"),
        _ => None,
    }
}

/// First 12 bytes, enough to recognise the image type without reading the whole file.
fn head(path: &Path) -> Option<[u8; 12]> {
    use std::io::Read;
    let mut buf = [0u8; 12];
    std::fs::File::open(path).ok()?.read_exact(&mut buf).ok()?;
    Some(buf)
}

/// "Easter Sunday (1).JPG" → "Easter-Sunday-1"
fn safe_stem(name: &str) -> String {
    let stem = Path::new(name).file_stem().and_then(|s| s.to_str()).unwrap_or("image");
    let cleaned: String = stem.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let joined = cleaned.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    let short: String = joined.chars().take(40).collect();
    if short.is_empty() { "image".into() } else { short }
}

/// A plain file name inside `dir` (no separators, no "..").
fn resolve(dir: &Path, file: &str) -> Option<PathBuf> {
    let ok = !file.is_empty() && !file.starts_with('.') && file.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c));
    ok.then(|| dir.join(file))
}

pub fn list(dir: &Path) -> Vec<UploadedBackground> {
    let mut out: Vec<UploadedBackground> = std::fs::read_dir(dir)
        .map(|it| {
            it.filter_map(|e| {
                let e = e.ok()?;
                let file = e.file_name().to_str()?.to_owned();
                let path = resolve(dir, &file)?;
                image_extension(&head(&path)?)?;
                // "Easter-Sunday-1-1760171000000.jpg" → "Easter Sunday 1"
                let stem = file.rsplit_once('.').map_or(file.as_str(), |(s, _)| s);
                let name = stem.rsplit_once('-').map_or(stem, |(n, _)| n).replace('-', " ");
                Some(UploadedBackground { name, path: path.display().to_string(), file })
            })
            .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.file.rsplit('-').next().cmp(&a.file.rsplit('-').next())); // newest first
    out
}

/// Store an uploaded image; returns its file name.
pub fn save(dir: &Path, original_name: &str, bytes: &[u8]) -> Result<String> {
    if bytes.len() > MAX_BYTES {
        bail!("that image is over {} MB; please use a smaller copy", MAX_BYTES / (1024 * 1024));
    }
    let Some(ext) = image_extension(bytes) else { bail!("only JPEG, PNG, WebP or GIF images can be used") };
    std::fs::create_dir_all(dir).context("creating the backgrounds folder")?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let file = format!("{}-{stamp}.{ext}", safe_stem(original_name));
    std::fs::write(dir.join(&file), bytes).context("saving the image")?;
    Ok(file)
}

pub fn delete(dir: &Path, file: &str) -> Result<()> {
    let Some(path) = resolve(dir, file) else { bail!("invalid background name") };
    std::fs::remove_file(path).context("deleting the image")
}

pub fn path(dir: &Path, file: &str) -> Option<String> {
    resolve(dir, file).filter(|p| p.is_file()).map(|p| p.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];

    #[test]
    fn saves_lists_and_deletes_images() {
        let dir = std::env::temp_dir().join(format!("sb-bg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let file = save(&dir, "Easter Sunday (1).PNG", PNG).unwrap();
        assert!(file.starts_with("Easter-Sunday-1-") && file.ends_with(".png"), "{file}");
        let listed = list(&dir);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Easter Sunday 1");
        assert!(path(&dir, &file).is_some());
        delete(&dir, &file).unwrap();
        assert!(list(&dir).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_non_images_and_path_tricks() {
        let dir = std::env::temp_dir().join(format!("sb-bg-bad-{}", std::process::id()));
        assert!(save(&dir, "notes.png", b"hello, not an image").is_err());
        assert!(delete(&dir, "../settings.json").is_err());
        assert!(path(&dir, "../../etc/passwd").is_none());
        assert!(path(&dir, ".hidden").is_none());
        assert_eq!(safe_stem("../../evil name!!.jpg"), "evil-name");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
