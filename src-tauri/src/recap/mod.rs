//! Shareable sermon recaps: the scriptures shown, sermon notes, other passages mentioned and
//! the songs, as one document rendered to a web page, PDF, Word (.docx) or .doc (RTF, which
//! every version of Word opens). The dashboard saves them; congregation phones download them.

mod docx;
mod html;
pub mod notes;
mod pdf;
mod rtf;

use anyhow::Result;
use chrono::NaiveDate;

pub use notes::SermonNotes;

use crate::{history::Recap, scripture::BibleDb};

pub const FOOTER: &str = "Recap made with Hearken Studio";

/// What a recap is made of, in reading order. Each renderer styles these its own way.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    /// "Sermon notes", "Scriptures", "Songs".
    Section(String),
    /// A passage or note heading, with a right-aligned detail ("KJV · 10:42").
    Heading { text: String, meta: Option<String> },
    Text(String),
    Bullet(String),
    /// Small, muted text ("Themes: faith · grace").
    Note(String),
}

#[derive(Debug, Clone)]
pub struct Document {
    pub title: String,
    pub subtitle: String,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Html,
    Pdf,
    Docx,
    /// Rich Text, saved as .doc: opens in every Word version, Pages, LibreOffice and WordPad.
    Doc,
}

impl Format {
    pub fn from_ext(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "html" => Some(Self::Html),
            "pdf" => Some(Self::Pdf),
            "docx" => Some(Self::Docx),
            "doc" => Some(Self::Doc),
            _ => None,
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            Self::Html => "html",
            Self::Pdf => "pdf",
            Self::Docx => "docx",
            Self::Doc => "doc",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Self::Html => "text/html; charset=utf-8",
            Self::Pdf => "application/pdf",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Doc => "application/msword",
        }
    }
}

pub fn file_name(date: &str, format: Format) -> String {
    format!("Sermon recap {date}.{}", format.ext())
}

pub fn render(doc: &Document, format: Format) -> Result<Vec<u8>> {
    Ok(match format {
        Format::Html => html::page(doc, None).into_bytes(),
        Format::Pdf => pdf::render(doc)?,
        Format::Docx => docx::render(doc)?,
        Format::Doc => rtf::render(doc).into_bytes(),
    })
}

/// The recap as a phone page, with `toolbar` (back link, download buttons) above it.
pub fn phone_page(doc: &Document, toolbar: &str) -> String {
    html::page(doc, Some(toolbar))
}

/// "Sunday 12 October 2026"
pub fn long_date(date: &str) -> String {
    NaiveDate::parse_from_str(date, "%Y-%m-%d").map(|d| d.format("%A %-d %B %Y").to_string()).unwrap_or_else(|_| date.to_owned())
}

pub fn document(r: &Recap, notes: Option<&SermonNotes>, church: &str) -> Document {
    let mut blocks = Vec::new();
    if let Some(n) = notes.filter(|n| !n.is_empty()) {
        blocks.push(Block::Section("Sermon notes".into()));
        if !n.themes.is_empty() {
            blocks.push(Block::Note(format!("Themes: {}", n.themes.join(" · "))));
        }
        for s in n.sections.iter().filter(|s| !s.points.is_empty()) {
            blocks.push(Block::Heading { text: s.heading.clone().unwrap_or_else(|| "Opening".into()), meta: Some(s.time.clone()) });
            blocks.extend(s.points.iter().cloned().map(Block::Bullet));
        }
    }
    blocks.push(Block::Section("Scriptures".into()));
    if r.scriptures.is_empty() {
        blocks.push(Block::Note("No scriptures were shown in this service.".into()));
    }
    for s in &r.scriptures {
        blocks.push(Block::Heading { text: s.label.clone(), meta: Some(format!("{} · {}", s.translation, s.time)) });
        blocks.push(Block::Text(s.text.clone()));
    }
    if !r.mentioned.is_empty() {
        blocks.push(Block::Section("Also mentioned".into()));
        blocks.push(Block::Text(r.mentioned.iter().map(|m| m.label.as_str()).collect::<Vec<_>>().join(" · ")));
    }
    if !r.songs.is_empty() {
        blocks.push(Block::Section("Songs".into()));
        blocks.push(Block::Text(r.songs.iter().map(|s| s.title.as_str()).collect::<Vec<_>>().join(" · ")));
    }
    let title = if church.trim().is_empty() { "Sermon recap".to_owned() } else { church.trim().to_owned() };
    let mut subtitle = long_date(&r.date);
    if let (Some(a), Some(b)) = (r.first_at, r.last_at) {
        subtitle.push_str(&format!(" · {}–{}", crate::history::time_of(a), crate::history::time_of(b)));
    }
    Document { title, subtitle, blocks }
}

/// Show the recap's scriptures in another translation (a phone reader's choice). Passages the
/// translation lacks keep the text that was projected.
pub fn retranslate(r: &mut Recap, db: &BibleDb, code: &str) {
    for s in &mut r.scriptures {
        let Some(rf) = &s.reference else { continue };
        if s.translation.eq_ignore_ascii_case(code) {
            continue;
        }
        let start = rf.verse_start.unwrap_or(1);
        if let Ok(v) = db.passage(code, rf.book_id, rf.chapter, start, rf.verse_end.unwrap_or(start)) {
            if !v.is_empty() {
                s.text = v.iter().map(|v| v.text.as_str()).collect::<Vec<_>>().join(" ");
                s.translation = code.to_uppercase();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{RecapItem, RecapSong};

    pub(super) fn sample() -> Document {
        let recap = Recap {
            date: "2026-10-11".into(),
            scriptures: vec![RecapItem {
                time: "10:42".into(),
                label: "John 3:16".into(),
                translation: "KJV".into(),
                text: "For God so loved the world, that he gave his only begotten Son, that whosoever believeth in him should not perish, but have everlasting life.".into(),
                book_id: Some(43),
                reference: None,
            }],
            mentioned: vec![RecapItem { time: "10:50".into(), label: "Romans 8:28".into(), translation: String::new(), text: String::new(), book_id: None, reference: None }],
            songs: vec![RecapSong { time: "10:05".into(), title: "Amazing Grace".into(), ccli: None }],
            words: 1200,
            first_at: None,
            last_at: None,
        };
        let notes = SermonNotes {
            themes: vec!["grace".into(), "faith".into()],
            sections: vec![notes::NoteSection { heading: Some("John 3:16".into()), time: "10:42".into(), points: vec!["Ìfẹ́ Ọlọ́run kò lópin — God's love <has> no end & no limit.".into()] }],
        };
        document(&recap, Some(&notes), "Grace Chapel")
    }

    #[test]
    fn document_lists_notes_then_scriptures() {
        let doc = sample();
        assert_eq!(doc.title, "Grace Chapel");
        assert_eq!(doc.subtitle, "Sunday 11 October 2026");
        let sections: Vec<_> = doc.blocks.iter().filter_map(|b| if let Block::Section(s) = b { Some(s.as_str()) } else { None }).collect();
        assert_eq!(sections, ["Sermon notes", "Scriptures", "Also mentioned", "Songs"]);
    }

    #[test]
    fn every_format_renders() {
        let doc = sample();
        for f in [Format::Html, Format::Pdf, Format::Docx, Format::Doc] {
            let bytes = render(&doc, f).unwrap();
            assert!(bytes.len() > 300, "{f:?} is suspiciously small");
            if let Ok(dir) = std::env::var("RECAP_SAMPLE_DIR") {
                std::fs::write(format!("{dir}/{}", file_name("sample", f)), &bytes).unwrap();
            }
        }
        assert_eq!(Format::from_ext("DOCX"), Some(Format::Docx));
        assert_eq!(file_name("2026-10-11", Format::Pdf), "Sermon recap 2026-10-11.pdf");
    }
}
