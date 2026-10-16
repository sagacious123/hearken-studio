//! Read-only, memory-mapped SQLite store. Direct lookups hit the
//! (translation_id, book_id, chapter, verse) unique index (~10–50 µs); FTS5 queries with
//! BM25 ranking typically run in 1–4 ms over 5 translations (~155k verses).
//!
//! Schema is created by `scripts/build_bible_db.py`. Translations downloaded from the Bible
//! library live in a second, writable database (see `library`), attached here as `lib`; their
//! ids start at `library::FIRST_ID`, which is how a lookup knows where to go.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;

use super::{ScriptureRef, VerseLine, Versification};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Translation {
    pub id: i64,
    pub code: String,
    pub name: String,
    /// Copyright or licence line; shown with the text when it isn't public domain.
    pub license: Option<String>,
    /// Downloaded from the Bible library (can be removed).
    pub downloaded: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerseHit {
    pub translation: String,
    pub reference: ScriptureRef,
    pub text: String,
    pub rank: f64,
}

pub struct BibleDb {
    conn: Connection,
    /// The Bible library database is attached (it exists once the app has started).
    has_library: bool,
}

/// Where downloaded translations are kept; set once at startup, before any database opens.
static LIBRARY_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn set_library_path(path: PathBuf) {
    let _ = LIBRARY_PATH.set(path);
}

impl BibleDb {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_URI)
            .with_context(|| format!("opening bible database at {}", path.display()))?;
        // A download may be writing to the library; wait for it rather than fail a lookup.
        conn.busy_timeout(std::time::Duration::from_secs(3))?;
        let has_library = match LIBRARY_PATH.get().filter(|p| p.exists()) {
            Some(lib) => {
                let uri = format!("file:{}?mode=ro", lib.display().to_string().replace('?', "%3f").replace('#', "%23"));
                match conn.execute("ATTACH DATABASE ?1 AS lib", [uri]) {
                    Ok(_) => true,
                    Err(e) => {
                        log::warn!("couldn't open the Bible library ({}): {e}", lib.display());
                        false
                    }
                }
            }
            None => false,
        };
        conn.execute_batch(
            "PRAGMA query_only = ON;
             PRAGMA mmap_size = 268435456;
             PRAGMA cache_size = -16000;
             PRAGMA temp_store = MEMORY;",
        )?;
        let db = Self { conn, has_library };
        anyhow::ensure!(!db.translations().is_empty(), "bible database has no translations");
        Ok(db)
    }

    /// Empty KJV-only database with the real schema, for unit tests.
    #[cfg(test)]
    pub fn open_in_memory_for_tests() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE translations (id INTEGER PRIMARY KEY, code TEXT NOT NULL, name TEXT NOT NULL, license TEXT);
             INSERT INTO translations (id, code, name) VALUES (1, 'KJV', 'King James Version');
             CREATE TABLE verses (id INTEGER PRIMARY KEY, translation_id INTEGER, book_id INTEGER, chapter INTEGER, verse INTEGER, text TEXT);
             CREATE VIRTUAL TABLE verses_fts USING fts5(text, content = 'verses', content_rowid = 'id');",
        )
        .unwrap();
        Self { conn, has_library: false }
    }

    /// Bundled translations first, then downloaded ones. Read fresh each time (a few µs), so
    /// a translation downloaded a moment ago is already here.
    pub fn translations(&self) -> Vec<Translation> {
        let sql = if self.has_library {
            "SELECT id, code, name, license, 0 FROM main.translations
             UNION ALL SELECT id, code, name, license, 1 FROM lib.translations
             ORDER BY 5, 1"
        } else {
            "SELECT id, code, name, license, 0 FROM main.translations ORDER BY id"
        };
        let result = self.conn.prepare_cached(sql).and_then(|mut stmt| {
            stmt.query_map([], |r| Ok(Translation { id: r.get(0)?, code: r.get(1)?, name: r.get(2)?, license: r.get(3)?, downloaded: r.get(4)? }))?
                .collect::<rusqlite::Result<Vec<_>>>()
        });
        result.unwrap_or_else(|e| {
            log::warn!("reading translations failed: {e}");
            Vec::new()
        })
    }

    fn translation_id(&self, code: &str) -> Result<i64> {
        let sql = if self.has_library {
            "SELECT id FROM main.translations WHERE code = ?1 COLLATE NOCASE
             UNION ALL SELECT id FROM lib.translations WHERE code = ?1 COLLATE NOCASE"
        } else {
            "SELECT id FROM main.translations WHERE code = ?1 COLLATE NOCASE"
        };
        let mut stmt = self.conn.prepare_cached(sql)?;
        let id: Option<i64> = stmt.query_row([code], |r| r.get(0)).ok();
        id.with_context(|| format!("unknown translation '{code}'"))
    }

    /// Which database holds a translation's verses.
    fn schema(id: i64) -> &'static str {
        if id >= super::library::FIRST_ID {
            "lib"
        } else {
            "main"
        }
    }

    /// The book's name in a downloaded translation's own language ("Johanu" for John in
    /// Yoruba); `None` for bundled (English) translations.
    pub fn book_name(&self, translation: &str, book_id: u8) -> Option<String> {
        let tid = self.translation_id(translation).ok()?;
        if Self::schema(tid) != "lib" {
            return None;
        }
        let mut stmt = self.conn.prepare_cached("SELECT name FROM lib.book_names WHERE translation_id = ?1 AND book_id = ?2").ok()?;
        stmt.query_row(params![tid, book_id], |r| r.get(0)).ok()
    }

    /// Chapter verse counts from the first (reference) translation.
    pub fn versification(&self) -> Result<Versification> {
        let mut stmt = self.conn.prepare(
            "SELECT book_id, chapter, MAX(verse) FROM main.verses WHERE translation_id = ?1 GROUP BY book_id, chapter",
        )?;
        let first = self.translations().first().map(|t| t.id).context("no translations")?;
        let map = stmt
            .query_map([first], |r| Ok(((r.get::<_, u8>(0)?, r.get::<_, u16>(1)?), r.get::<_, u16>(2)?)))?
            .collect::<rusqlite::Result<HashMap<_, _>>>()?;
        Ok(Versification::from_map(map))
    }

    pub fn passage(&self, translation: &str, book_id: u8, chapter: u16, start: u16, end: u16) -> Result<Vec<VerseLine>> {
        let tid = self.translation_id(translation)?;
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT verse, text FROM {}.verses
             WHERE translation_id = ?1 AND book_id = ?2 AND chapter = ?3 AND verse BETWEEN ?4 AND ?5
             ORDER BY verse",
            Self::schema(tid)
        ))?;
        let rows = stmt.query_map(params![tid, book_id, chapter, start, end], |r| {
            Ok(VerseLine { verse: r.get(0)?, text: r.get(1)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Every verse of a translation, in canon order (for building the search-by-meaning index).
    pub fn all_verses(&self, translation: &str) -> Result<Vec<(ScriptureRef, String)>> {
        let tid = self.translation_id(translation)?;
        let mut stmt = self.conn.prepare(&format!(
            "SELECT book_id, chapter, verse, text FROM {}.verses WHERE translation_id = ?1 ORDER BY book_id, chapter, verse",
            Self::schema(tid)
        ))?;
        let rows = stmt.query_map([tid], |r| {
            Ok((ScriptureRef { book_id: r.get(0)?, chapter: r.get(1)?, verse_start: Some(r.get(2)?), verse_end: None }, r.get(3)?))
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// FTS5 search. `fts_query` must already be a safe FTS5 expression (see `user_query`).
    /// `translation = None` searches every bundled translation — used by quote matching so that
    /// modern paraphrase can hit WEB while archaic phrasing hits KJV. A downloaded translation
    /// searches its own words, then the bundled translations if none of them are there.
    pub fn search(&self, translation: Option<&str>, fts_query: &str, limit: usize) -> Result<Vec<VerseHit>> {
        let tid = translation.map(|t| self.translation_id(t)).transpose()?;
        if let Some(id) = tid.filter(|&id| Self::schema(id) == "lib") {
            let hits = self.search_library("v.translation_id = ?2", id, fts_query, limit)?;
            if !hits.is_empty() {
                return Ok(hits);
            }
        }
        let tid = tid.filter(|&id| Self::schema(id) == "main");
        let mut stmt = self.conn.prepare_cached(
            "SELECT t.code, v.book_id, v.chapter, v.verse, v.text, bm25(verses_fts) AS rank
             FROM main.verses_fts
             JOIN main.verses v ON v.id = verses_fts.rowid
             JOIN main.translations t ON t.id = v.translation_id
             WHERE verses_fts MATCH ?1 AND (?2 IS NULL OR v.translation_id = ?2)
             ORDER BY rank
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![fts_query, tid, limit as i64], hit)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// FTS5 search over the downloaded translations in `language` ("French"), for verses quoted
    /// aloud in that language.
    pub fn search_language(&self, language: &str, fts_query: &str, limit: usize) -> Result<Vec<VerseHit>> {
        self.search_library("t.language = ?2", language, fts_query, limit)
    }

    fn search_library(&self, filter: &str, value: impl rusqlite::ToSql, fts_query: &str, limit: usize) -> Result<Vec<VerseHit>> {
        if !self.has_library {
            return Ok(Vec::new());
        }
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT t.code, v.book_id, v.chapter, v.verse, v.text, bm25(verses_fts) AS rank
             FROM lib.verses_fts
             JOIN lib.verses v ON v.id = verses_fts.rowid
             JOIN lib.translations t ON t.id = v.translation_id
             WHERE verses_fts MATCH ?1 AND {filter}
             ORDER BY rank
             LIMIT ?3"
        ))?;
        let rows = stmt.query_map(params![fts_query, value, limit as i64], hit)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

fn hit(r: &rusqlite::Row) -> rusqlite::Result<VerseHit> {
    Ok(VerseHit {
        translation: r.get(0)?,
        reference: ScriptureRef { book_id: r.get(1)?, chapter: r.get(2)?, verse_start: Some(r.get(3)?), verse_end: None },
        text: r.get(4)?,
        rank: r.get(5)?,
    })
}

/// Lowercase alphanumeric words; apostrophes dropped so "God's" → "gods".
pub fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '’'))
        .map(|w| w.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Turns operator search-box input into a safe FTS5 query: every token quoted (so FTS
/// operators in user text can't break the query), AND-ed, with prefix matching on the last
/// token for search-as-you-type.
pub fn user_query(input: &str) -> Option<String> {
    let tokens = words(input);
    let (last, rest) = tokens.split_last()?;
    let mut q: Vec<String> = rest.iter().map(|t| format!("\"{t}\"")).collect();
    q.push(format!("\"{last}\"*"));
    Some(q.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_query_is_quoted_and_prefixed() {
        assert_eq!(user_query("for God so lo").as_deref(), Some(r#""for" "god" "so" "lo"*"#));
        assert_eq!(user_query("NEAR( OR \"x"), Some(r#""near" "or" "x"*"#.into()));
        assert_eq!(user_query("  ::  "), None);
    }
}
