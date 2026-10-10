//! Read-only, memory-mapped SQLite store. Direct lookups hit the
//! (translation_id, book_id, chapter, verse) unique index (~10–50 µs); FTS5 queries with
//! BM25 ranking typically run in 1–4 ms over 5 translations (~155k verses).
//!
//! Schema is created by `scripts/build_bible_db.py`.

use std::{collections::HashMap, path::Path};

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
    translations: Vec<Translation>,
    by_code: HashMap<String, i64>,
}

impl BibleDb {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
            .with_context(|| format!("opening bible database at {}", path.display()))?;
        conn.execute_batch(
            "PRAGMA query_only = ON;
             PRAGMA mmap_size = 268435456;
             PRAGMA cache_size = -16000;
             PRAGMA temp_store = MEMORY;",
        )?;
        let translations = {
            let mut stmt = conn.prepare("SELECT id, code, name FROM translations ORDER BY id")?;
            let rows = stmt.query_map([], |r| Ok(Translation { id: r.get(0)?, code: r.get(1)?, name: r.get(2)? }))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        anyhow::ensure!(!translations.is_empty(), "bible database has no translations");
        let by_code = translations.iter().map(|t| (t.code.to_uppercase(), t.id)).collect();
        Ok(Self { conn, translations, by_code })
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
        let translations = vec![Translation { id: 1, code: "KJV".into(), name: "King James Version".into() }];
        let by_code = [("KJV".to_string(), 1)].into_iter().collect();
        Self { conn, translations, by_code }
    }

    pub fn translations(&self) -> &[Translation] {
        &self.translations
    }

    fn translation_id(&self, code: &str) -> Result<i64> {
        self.by_code.get(&code.to_uppercase()).copied().with_context(|| format!("unknown translation '{code}'"))
    }

    /// Chapter verse counts from the first (reference) translation.
    pub fn versification(&self) -> Result<Versification> {
        let mut stmt = self.conn.prepare(
            "SELECT book_id, chapter, MAX(verse) FROM verses WHERE translation_id = ?1 GROUP BY book_id, chapter",
        )?;
        let map = stmt
            .query_map([self.translations[0].id], |r| Ok(((r.get::<_, u8>(0)?, r.get::<_, u16>(1)?), r.get::<_, u16>(2)?)))?
            .collect::<rusqlite::Result<HashMap<_, _>>>()?;
        Ok(Versification::from_map(map))
    }

    pub fn passage(&self, translation: &str, book_id: u8, chapter: u16, start: u16, end: u16) -> Result<Vec<VerseLine>> {
        let tid = self.translation_id(translation)?;
        let mut stmt = self.conn.prepare_cached(
            "SELECT verse, text FROM verses
             WHERE translation_id = ?1 AND book_id = ?2 AND chapter = ?3 AND verse BETWEEN ?4 AND ?5
             ORDER BY verse",
        )?;
        let rows = stmt.query_map(params![tid, book_id, chapter, start, end], |r| {
            Ok(VerseLine { verse: r.get(0)?, text: r.get(1)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// FTS5 search. `fts_query` must already be a safe FTS5 expression (see `user_query`).
    /// `translation = None` searches every translation — used by quote matching so that
    /// modern paraphrase can hit WEB while archaic phrasing hits KJV.
    pub fn search(&self, translation: Option<&str>, fts_query: &str, limit: usize) -> Result<Vec<VerseHit>> {
        let tid = translation.map(|t| self.translation_id(t)).transpose()?;
        let mut stmt = self.conn.prepare_cached(
            "SELECT t.code, v.book_id, v.chapter, v.verse, v.text, bm25(verses_fts) AS rank
             FROM verses_fts
             JOIN verses v ON v.id = verses_fts.rowid
             JOIN translations t ON t.id = v.translation_id
             WHERE verses_fts MATCH ?1 AND (?2 IS NULL OR v.translation_id = ?2)
             ORDER BY rank
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![fts_query, tid, limit as i64], |r| {
            Ok(VerseHit {
                translation: r.get(0)?,
                reference: ScriptureRef {
                    book_id: r.get(1)?,
                    chapter: r.get(2)?,
                    verse_start: Some(r.get(3)?),
                    verse_end: None,
                },
                text: r.get(4)?,
                rank: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
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
