pub mod books;
pub mod db;
pub mod fuzzy;
pub mod numbers;
pub mod parser;
pub mod quote;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub use db::BibleDb;

/// Upper bound on verses per slide; operators step through longer passages.
pub const MAX_SLIDE_VERSES: u16 = 6;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptureRef {
    pub book_id: u8,
    pub chapter: u16,
    pub verse_start: Option<u16>,
    pub verse_end: Option<u16>,
}

impl ScriptureRef {
    pub fn key(&self) -> String {
        match (self.verse_start, self.verse_end) {
            (Some(s), Some(e)) => format!("{}.{}.{}-{}", self.book_id, self.chapter, s, e),
            (Some(s), None) => format!("{}.{}.{}", self.book_id, self.chapter, s),
            _ => format!("{}.{}", self.book_id, self.chapter),
        }
    }

    pub fn label(&self) -> String {
        let name = books::by_id(self.book_id).map(|b| b.display).unwrap_or("?");
        match (self.verse_start, self.verse_end) {
            (Some(s), Some(e)) => format!("{name} {}:{s}-{e}", self.chapter),
            (Some(s), None) => format!("{name} {}:{s}", self.chapter),
            _ => format!("{name} {}", self.chapter),
        }
    }
}

/// Verse counts per chapter, loaded once from the DB. Used to validate parsed references
/// (rejecting "John 3:99") and to disambiguate run-together digits ("John 316" → 3:16).
#[derive(Debug, Default)]
pub struct Versification {
    max_verse: HashMap<(u8, u16), u16>,
}

impl Versification {
    pub fn from_map(max_verse: HashMap<(u8, u16), u16>) -> Self {
        Self { max_verse }
    }

    pub fn max_verse(&self, book_id: u8, chapter: u16) -> Option<u16> {
        self.max_verse.get(&(book_id, chapter)).copied()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerseLine {
    pub verse: u16,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slide {
    pub id: String,
    pub label: String,
    pub translation: String,
    /// Unused (book 0) on song slides.
    pub reference: ScriptureRef,
    /// Verses, or lyric lines on song slides (numbered from 1).
    pub verses: Vec<VerseLine>,
    /// The same passage in a second translation, shown side by side.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<SecondaryText>,
    /// Set on worship-song slides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub song: Option<SongPart>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecondaryText {
    pub translation: String,
    pub verses: Vec<VerseLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongPart {
    pub song_id: String,
    pub title: String,
    /// Index into the song's sections, and how many there are.
    pub section: usize,
    pub sections: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ccli: Option<String>,
}

impl Slide {
    pub fn is_song(&self) -> bool {
        self.song.is_some()
    }
}

/// Resolve a reference to displayable text, with `secondary` (another translation code) side
/// by side when set. Chapter-only references open at verse 1.
pub fn build_slide(db: &BibleDb, translation: &str, secondary: Option<&str>, r: &ScriptureRef) -> anyhow::Result<Option<Slide>> {
    let start = r.verse_start.unwrap_or(1);
    let end = r.verse_end.unwrap_or(start).max(start).min(start + MAX_SLIDE_VERSES - 1);
    let verses = db.passage(translation, r.book_id, r.chapter, start, end)?;
    if verses.is_empty() {
        return Ok(None);
    }
    let shown = ScriptureRef {
        verse_start: Some(start),
        verse_end: (end > start).then_some(end),
        ..r.clone()
    };
    let secondary = match secondary.filter(|s| !s.eq_ignore_ascii_case(translation)) {
        Some(code) => {
            let verses = db.passage(code, r.book_id, r.chapter, start, end).unwrap_or_default();
            (!verses.is_empty()).then(|| SecondaryText { translation: code.to_owned(), verses })
        }
        None => None,
    };
    let id = match &secondary {
        Some(s) => format!("{translation}+{}:{}", s.translation, shown.key()),
        None => format!("{translation}:{}", shown.key()),
    };
    Ok(Some(Slide {
        id,
        label: shown.label(),
        translation: translation.to_owned(),
        reference: shown,
        verses,
        secondary,
        song: None,
    }))
}
