//! Worship-song library. Hearken Studio ships no lyrics: churches import the songs they are
//! licensed to project (CCLI or similar), and the pipeline follows the singing through them.
//!
//! Import format is plain text, as copied from SongSelect or typed by hand:
//!
//! ```text
//! Amazing Grace
//! CCLI Song # 22025
//!
//! Verse 1
//! Amazing grace how sweet the sound
//! That saved a wretch like me
//!
//! Chorus
//! …
//! ```
//!
//! The first line is the title; blocks are separated by blank lines; a block may start with a
//! header ("Verse 1", "[Chorus]", "Bridge:"). Copyright/licence footer lines are dropped.

use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering::Relaxed},
        LazyLock,
    },
};

use anyhow::{bail, Result};
use parking_lot::Mutex;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{
    scripture::{ScriptureRef, Slide, SongPart, VerseLine},
    state::{read_json, write_json},
};

/// Longer blocks are split so lyrics stay readable from the back row.
const MAX_LINES_PER_SLIDE: usize = 6;
const LINES_PER_SPLIT: usize = 4;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub ccli: Option<String>,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub label: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SongSummary {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub ccli: Option<String>,
    pub sections: Vec<String>,
}

static HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\[?\s*((?:verse|chorus|refrain|pre-?chorus|bridge|tag|intro|outro|ending|interlude|vamp|coda)\b[^\]:]*?)\s*\]?\s*:?\s*$")
        .expect("header regex")
});
static CCLI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)ccli\s*(?:song)?\s*(?:#|no\.?|number)?\s*:?\s*(\d{3,8})").expect("ccli regex"));
static FOOTER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(ccli\s+licen[cs]e|©|\(c\)|copyright|for use solely|all rights reserved|songselect)").expect("footer regex"));

fn slug(title: &str) -> String {
    let s: String = title.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect();
    let s = s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    if s.is_empty() { "song".into() } else { s }
}

/// Parse pasted lyrics. Errors when there is no title or no lyric lines.
pub fn parse(text: &str) -> Result<Song> {
    let mut ccli = None;
    let mut author = None;
    let mut title = None;
    let mut blocks: Vec<Vec<String>> = vec![vec![]];
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(c) = CCLI.captures(line) {
            ccli.get_or_insert_with(|| c[1].to_owned());
            if title.is_some() {
                continue;
            }
        }
        if FOOTER.is_match(line) {
            continue;
        }
        if title.is_none() {
            if !line.is_empty() {
                title = Some(line.trim_start_matches("Title:").trim().to_owned());
            }
            continue;
        }
        if blocks.iter().all(|b| b.is_empty()) {
            let lower = line.to_lowercase();
            let credit = ["author:", "words and music by", "words:", "written by", "by "]
                .iter()
                .find_map(|p| lower.starts_with(p).then(|| line[p.len()..].trim()));
            if let Some(a) = credit {
                author = Some(a.to_owned());
                continue;
            }
        }
        if line.is_empty() {
            if !blocks.last().unwrap().is_empty() {
                blocks.push(vec![]);
            }
        } else {
            blocks.last_mut().unwrap().push(line.to_owned());
        }
    }
    let Some(title) = title.filter(|t| !t.is_empty()) else { bail!("the first line should be the song title") };

    let mut sections = Vec::new();
    for block in blocks.into_iter().filter(|b| !b.is_empty()) {
        let (label, lines) = match HEADER.captures(&block[0]) {
            Some(c) => (title_case(&c[1]), block[1..].to_vec()),
            None => (format!("Part {}", sections.len() + 1), block),
        };
        if lines.is_empty() {
            continue;
        }
        if lines.len() <= MAX_LINES_PER_SLIDE {
            sections.push(Section { label, lines });
        } else {
            for (i, chunk) in lines.chunks(LINES_PER_SPLIT).enumerate() {
                let label = if i == 0 { label.clone() } else { format!("{label} ({})", i + 1) };
                sections.push(Section { label, lines: chunk.to_vec() });
            }
        }
    }
    if sections.is_empty() {
        bail!("no lyrics found under the title");
    }
    Ok(Song { id: slug(&title), title, author, ccli, sections })
}

fn title_case(s: &str) -> String {
    let mut c = s.trim().chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// Back to the editable text format.
pub fn to_text(song: &Song) -> String {
    let mut out = song.title.clone();
    if let Some(a) = &song.author {
        out.push_str(&format!("\nAuthor: {a}"));
    }
    if let Some(c) = &song.ccli {
        out.push_str(&format!("\nCCLI Song # {c}"));
    }
    for s in &song.sections {
        out.push_str(&format!("\n\n{}\n{}", s.label, s.lines.join("\n")));
    }
    out
}

pub fn slide(song: &Song, section: usize) -> Option<Slide> {
    let s = song.sections.get(section)?;
    Some(Slide {
        id: format!("song:{}:{section}", song.id),
        label: format!("{} · {}", song.title, s.label),
        translation: String::new(),
        reference: ScriptureRef { book_id: 0, chapter: 0, verse_start: None, verse_end: None },
        verses: s.lines.iter().enumerate().map(|(i, l)| VerseLine { verse: i as u16 + 1, text: l.clone() }).collect(),
        secondary: None,
        song: Some(SongPart {
            song_id: song.id.clone(),
            title: song.title.clone(),
            section,
            sections: song.sections.len(),
            ccli: song.ccli.clone(),
        }),
    })
}

pub struct SongLibrary {
    path: PathBuf,
    songs: Mutex<Vec<Song>>,
    /// Bumped on every change so the pipeline knows to re-index.
    version: AtomicU64,
}

impl SongLibrary {
    pub fn load(path: PathBuf) -> Self {
        let songs: Vec<Song> = read_json(&path).unwrap_or_default();
        Self { path, songs: Mutex::new(songs), version: AtomicU64::new(1) }
    }

    pub fn version(&self) -> u64 {
        self.version.load(Relaxed)
    }

    pub fn all(&self) -> Vec<Song> {
        self.songs.lock().clone()
    }

    pub fn get(&self, id: &str) -> Option<Song> {
        self.songs.lock().iter().find(|s| s.id == id).cloned()
    }

    pub fn summaries(&self) -> Vec<SongSummary> {
        let mut v: Vec<SongSummary> = self
            .songs
            .lock()
            .iter()
            .map(|s| SongSummary {
                id: s.id.clone(),
                title: s.title.clone(),
                author: s.author.clone(),
                ccli: s.ccli.clone(),
                sections: s.sections.iter().map(|x| x.label.clone()).collect(),
            })
            .collect();
        v.sort_by_key(|s| s.title.to_lowercase());
        v
    }

    /// Add or replace (`id` = the song being edited). A new song whose title slug is taken
    /// gets a numbered id rather than overwriting.
    pub fn save(&self, text: &str, id: Option<&str>) -> Result<Song> {
        let mut song = parse(text)?;
        let mut songs = self.songs.lock();
        match id.and_then(|id| songs.iter().position(|s| s.id == id)) {
            Some(i) => {
                song.id = songs[i].id.clone();
                songs[i] = song.clone();
            }
            None => {
                let base = song.id.clone();
                let mut n = 2;
                while songs.iter().any(|s| s.id == song.id) {
                    song.id = format!("{base}-{n}");
                    n += 1;
                }
                songs.push(song.clone());
            }
        }
        write_json(&self.path, &*songs)?;
        self.version.fetch_add(1, Relaxed);
        Ok(song)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let mut songs = self.songs.lock();
        songs.retain(|s| s.id != id);
        write_json(&self.path, &*songs)?;
        self.version.fetch_add(1, Relaxed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "Amazing Grace\nWords: John Newton\nCCLI Song # 22025\n\nVerse 1\nAmazing grace how sweet the sound\nThat saved a wretch like me\nI once was lost but now am found\nWas blind but now I see\n\n[Chorus]\nMy chains are gone\nI've been set free\n\nThe Lord has promised good to me\nHis word my hope secures\n\nCCLI License # 1234567\n© 1779 Public Domain";

    #[test]
    fn parses_pasted_lyrics() {
        let s = parse(SAMPLE).unwrap();
        assert_eq!((s.id.as_str(), s.title.as_str(), s.ccli.as_deref()), ("amazing-grace", "Amazing Grace", Some("22025")));
        let labels: Vec<_> = s.sections.iter().map(|x| x.label.as_str()).collect();
        assert_eq!(labels, ["Verse 1", "Chorus", "Part 3"]);
        assert_eq!(s.sections[0].lines.len(), 4);
        assert!(s.sections.iter().all(|x| x.lines.iter().all(|l| !l.contains('©') && !l.contains("License"))));
        // round-trips through the editor
        let again = parse(&to_text(&s)).unwrap();
        assert_eq!(again.sections.len(), 3);
        assert_eq!(again.ccli.as_deref(), Some("22025"));
    }

    #[test]
    fn long_blocks_are_split_and_bad_input_rejected() {
        let long = format!("Song\n\n{}", (1..=9).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n"));
        let s = parse(&long).unwrap();
        assert_eq!(s.sections.iter().map(|x| x.lines.len()).collect::<Vec<_>>(), [4, 4, 1]);
        assert!(parse("").is_err());
        assert!(parse("Only a title").is_err());
    }

    #[test]
    fn slides_carry_song_details() {
        let s = parse(SAMPLE).unwrap();
        let sl = slide(&s, 1).unwrap();
        assert_eq!(sl.label, "Amazing Grace · Chorus");
        assert_eq!(sl.verses.len(), 2);
        assert_eq!(sl.song.as_ref().unwrap().sections, 3);
        assert!(slide(&s, 9).is_none());
    }
}
