//! The Bible library: more translations, downloaded on request from eBible.org and kept in a
//! local database next to the settings, so they work offline afterwards. Only translations
//! whose licence allows sharing the unchanged text are listed (public domain or Creative
//! Commons); each keeps its copyright line, which is shown with its text.

use std::{
    io::{Cursor, Read},
    path::Path,
};

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use serde::Serialize;

/// Downloaded translations are numbered from here; bundled ones are below.
pub const FIRST_ID: i64 = 1000;

/// Fewer verses than this means a broken or partial download (full Bibles have ~31,100).
const MIN_VERSES: usize = 30_000;

#[derive(Debug, Clone, Copy)]
pub struct CatalogEntry {
    /// eBible.org translation id.
    pub source: &'static str,
    pub code: &'static str,
    pub name: &'static str,
    pub language: &'static str,
    /// Copyright and licence, as the publisher asks it to be shown.
    pub notice: &'static str,
    /// Approximate download size.
    pub mb: f32,
}

const BIBLICA: &str = "Used under the Creative Commons Attribution-ShareAlike 4.0 licence (CC BY-SA). Biblica® is a trademark of Biblica, Inc.";

pub static CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        source: "yor",
        code: "YOR",
        name: "Bíbélì Mímọ́ Yorùbá Òde-Òní (Biblica® Open Yoruba Contemporary Bible™)",
        language: "Yoruba",
        notice: "Biblica® Open Yoruba Contemporary Bible™. Copyright © 2009, 2017 by Biblica, Inc.",
        mb: 4.8,
    },
    CatalogEntry {
        source: "ibo",
        code: "IGBO",
        name: "Baịbụlụ Nsọ nʼIgbo Ndị Ugbu a (Biblica® Open Igbo Contemporary Bible™)",
        language: "Igbo",
        notice: "Biblica® Open Igbo Contemporary Bible™. Copyright © 1980, 1988, 2019, 2020 by Biblica, Inc.",
        mb: 4.8,
    },
    CatalogEntry {
        source: "hausa",
        code: "HAU",
        name: "Littafi Mai Tsarki, Sabon Rai Don Kowa (Biblica® Open Hausa Contemporary Bible™)",
        language: "Hausa",
        notice: "Biblica® Open Hausa Contemporary Bible™. Copyright © 2009, 2020 by Biblica, Inc.",
        mb: 4.2,
    },
    CatalogEntry {
        source: "twi",
        code: "TWI",
        name: "Nkwa Asɛm, Akuapem Twi (Biblica® Open Akuapem Twi Contemporary Bible™)",
        language: "Twi (Akuapem)",
        notice: "Biblica® Open Akuapem Twi Contemporary Bible™. Copyright © 1996, 2020 by Biblica, Inc.",
        mb: 4.2,
    },
    CatalogEntry {
        source: "twiasante",
        code: "ASANTE",
        name: "Nkwa Asɛm, Asante Twi (Biblica® Open Asante Twi Contemporary Bible™)",
        language: "Twi (Asante)",
        notice: "Biblica® Open Asante Twi Contemporary Bible™. Copyright © 1996, 2020 by Biblica, Inc.",
        mb: 4.2,
    },
    CatalogEntry {
        source: "ewe",
        code: "EWE",
        name: "Agbenya La (Biblica® Open Ewe Contemporary Scriptures™)",
        language: "Ewe",
        notice: "Biblica® Open Ewe Contemporary Scriptures™. Copyright © 1988, 2006, 2020 by Biblica, Inc.",
        mb: 4.4,
    },
    CatalogEntry {
        source: "swhonen",
        code: "SWA",
        name: "Neno: Bibilia Takatifu (Kiswahili Contemporary Version)",
        language: "Swahili",
        notice: "Kiswahili Contemporary Version. Copyright © 1984, 1989, 2009, 2015 Biblica, Inc. Used under CC BY-SA 4.0.",
        mb: 4.4,
    },
    CatalogEntry { source: "fraLSG", code: "LSG", name: "Louis Segond 1910", language: "French", notice: "Public domain", mb: 4.6 },
    CatalogEntry { source: "spaRV1909", code: "RV1909", name: "Reina-Valera 1909", language: "Spanish", notice: "Public domain", mb: 4.4 },
    CatalogEntry {
        source: "porbr2018",
        code: "BLIVRE",
        name: "Bíblia Livre",
        language: "Portuguese",
        notice: "Bíblia Livre. Copyright © 2018 Diego Santos, Mario Sérgio e Marco Teles. Used under CC BY 4.0 Brasil.",
        mb: 4.4,
    },
    CatalogEntry { source: "deu1912", code: "LUT1912", name: "Lutherbibel 1912", language: "German", notice: "Public domain", mb: 4.5 },
];

pub fn entry(code_or_source: &str) -> Option<&'static CatalogEntry> {
    CATALOG.iter().find(|e| e.code.eq_ignore_ascii_case(code_or_source) || e.source == code_or_source)
}

impl CatalogEntry {
    /// The licence line stored with the text: copyright plus, for Biblica texts, the terms.
    pub fn license(&self) -> String {
        if self.notice.starts_with("Biblica®") {
            format!("{} {BIBLICA}", self.notice)
        } else {
            self.notice.to_owned()
        }
    }

    fn url(&self) -> String {
        format!("https://ebible.org/Scriptures/{}_vpl.zip", self.source)
    }
}

/// eBible's verse-per-line book codes (with the USFM spellings too), in canon order.
const BOOK_CODES: [&[&str]; 66] = [
    &["GEN"], &["EXO"], &["LEV"], &["NUM"], &["DEU"], &["JOS"], &["JDG"], &["RUT"], &["1SA"], &["2SA"], &["1KI"], &["2KI"],
    &["1CH"], &["2CH"], &["EZR"], &["NEH"], &["EST"], &["JOB"], &["PSA"], &["PRO"], &["ECC"], &["SOL", "SNG"], &["ISA"],
    &["JER"], &["LAM"], &["EZE", "EZK"], &["DAN"], &["HOS"], &["JOE", "JOL"], &["AMO"], &["OBA"], &["JON"], &["MIC"],
    &["NAH", "NAM"], &["HAB"], &["ZEP"], &["HAG"], &["ZEC"], &["MAL"], &["MAT"], &["MAR", "MRK"], &["LUK"], &["JOH", "JHN"],
    &["ACT"], &["ROM"], &["1CO"], &["2CO"], &["GAL"], &["EPH"], &["PHI", "PHP"], &["COL"], &["1TH"], &["2TH"], &["1TI"],
    &["2TI"], &["TIT"], &["PHM"], &["HEB"], &["JAM", "JAS"], &["1PE"], &["2PE"], &["1JO", "1JN"], &["2JO", "2JN"],
    &["3JO", "3JN"], &["JUD"], &["REV"],
];

fn book_id(code: &str) -> Option<u8> {
    BOOK_CODES.iter().position(|codes| codes.contains(&code)).map(|i| i as u8 + 1)
}

/// Book names in each translation's language (from eBible's BookNames.xml), bundled so a
/// download needs only the text.
fn book_names(source: &str) -> Vec<String> {
    static JSON: &str = include_str!("../../assets/booknames.json");
    serde_json::from_str::<std::collections::HashMap<String, Vec<String>>>(JSON).ok().and_then(|mut m| m.remove(source)).unwrap_or_default()
}

pub struct Verse {
    pub book_id: u8,
    pub chapter: u16,
    pub verse: u16,
    pub text: String,
}

/// "GEN 1:1 Ní ìbẹ̀rẹ̀ …" lines. Lines that don't parse, books outside the 66 and empty
/// verses are skipped.
pub fn parse_vpl(text: &str) -> Vec<Verse> {
    text.lines()
        .filter_map(|line| {
            let (code, rest) = line.trim_start_matches('\u{feff}').split_once(' ')?;
            let (cv, text) = rest.split_once(' ')?;
            let (c, v) = cv.split_once(':')?;
            let text = text.trim();
            if text.is_empty() {
                return None;
            }
            Some(Verse { book_id: book_id(code)?, chapter: c.parse().ok()?, verse: v.parse().ok()?, text: text.to_owned() })
        })
        .collect()
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS translations (
  id       INTEGER PRIMARY KEY,
  code     TEXT NOT NULL UNIQUE COLLATE NOCASE,
  name     TEXT NOT NULL,
  license  TEXT,
  language TEXT,
  source   TEXT
);
CREATE TABLE IF NOT EXISTS verses (
  id             INTEGER PRIMARY KEY,
  translation_id INTEGER NOT NULL,
  book_id        INTEGER NOT NULL,
  chapter        INTEGER NOT NULL,
  verse          INTEGER NOT NULL,
  text           TEXT NOT NULL,
  UNIQUE (translation_id, book_id, chapter, verse)
);
CREATE TABLE IF NOT EXISTS book_names (
  translation_id INTEGER NOT NULL,
  book_id        INTEGER NOT NULL,
  name           TEXT NOT NULL,
  PRIMARY KEY (translation_id, book_id)
);";

fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path).with_context(|| format!("opening the Bible library at {}", path.display()))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

/// Create the library database if needed, so readers can attach it from the start.
pub fn ensure(path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    open(path).map(drop)
}

/// Codes of the translations already downloaded.
pub fn installed(path: &Path) -> Result<Vec<String>> {
    let conn = open(path)?;
    let mut stmt = conn.prepare("SELECT code FROM translations ORDER BY id")?;
    let codes = stmt.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(codes)
}

/// Store a parsed translation (replacing an earlier copy) in one transaction, so readers see
/// all of it or none.
pub fn import(path: &Path, e: &CatalogEntry, verses: &[Verse]) -> Result<()> {
    if verses.len() < MIN_VERSES {
        bail!("the {} download looks incomplete ({} verses)", e.language, verses.len());
    }
    let mut conn = open(path)?;
    let tx = conn.transaction()?;
    let old: Option<i64> = tx.query_row("SELECT id FROM translations WHERE code = ?1", [e.code], |r| r.get(0)).ok();
    if let Some(id) = old {
        tx.execute("DELETE FROM verses WHERE translation_id = ?1", [id])?;
        tx.execute("DELETE FROM book_names WHERE translation_id = ?1", [id])?;
        tx.execute("DELETE FROM translations WHERE id = ?1", [id])?;
    }
    let next: i64 = tx.query_row("SELECT COALESCE(MAX(id) + 1, ?1) FROM translations", [FIRST_ID], |r| r.get(0))?;
    let id = old.unwrap_or(next).max(FIRST_ID);
    tx.execute(
        "INSERT INTO translations (id, code, name, license, language, source) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, e.code, e.name, e.license(), e.language, e.source],
    )?;
    {
        let mut insert = tx.prepare("INSERT OR IGNORE INTO verses (translation_id, book_id, chapter, verse, text) VALUES (?1, ?2, ?3, ?4, ?5)")?;
        for v in verses {
            insert.execute(params![id, v.book_id, v.chapter, v.verse, v.text])?;
        }
        let mut name = tx.prepare("INSERT INTO book_names (translation_id, book_id, name) VALUES (?1, ?2, ?3)")?;
        for (i, n) in book_names(e.source).iter().enumerate().filter(|(_, n)| !n.is_empty()) {
            name.execute(params![id, i as u8 + 1, n])?;
        }
    }
    tx.commit()?;
    Ok(())
}

pub fn remove(path: &Path, code: &str) -> Result<()> {
    let mut conn = open(path)?;
    let tx = conn.transaction()?;
    let id: i64 = tx.query_row("SELECT id FROM translations WHERE code = ?1", [code], |r| r.get(0)).context("that translation isn't downloaded")?;
    tx.execute("DELETE FROM verses WHERE translation_id = ?1", [id])?;
    tx.execute("DELETE FROM book_names WHERE translation_id = ?1", [id])?;
    tx.execute("DELETE FROM translations WHERE id = ?1", [id])?;
    tx.commit()?;
    // Give the space back.
    conn.execute_batch("VACUUM")?;
    Ok(())
}

/// Download and import one translation. `progress` gets 0.0–1.0 while downloading.
pub fn install(path: &Path, e: &CatalogEntry, mut progress: impl FnMut(f32)) -> Result<()> {
    // eBible.org refuses requests without a browser-like User-Agent.
    let resp = ureq::get(&e.url())
        .set("User-Agent", "Mozilla/5.0 (compatible; HearkenStudio)")
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .with_context(|| format!("couldn't download the {} Bible. Is this computer online?", e.language))?;
    let total = resp.header("Content-Length").and_then(|l| l.parse::<usize>().ok()).unwrap_or((e.mb * 1_000_000.0) as usize);
    let mut body = Vec::with_capacity(total);
    let mut reader = resp.into_reader().take(64 * 1024 * 1024);
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
        progress((body.len() as f32 / total as f32).min(0.99));
    }
    let mut zip = zip::ZipArchive::new(Cursor::new(body)).context("the download wasn't a valid zip file")?;
    let name = format!("{}_vpl.txt", e.source);
    let mut text = String::new();
    zip.by_name(&name).with_context(|| format!("{name} is missing from the download"))?.read_to_string(&mut text)?;
    import(path, e, &parse_vpl(&text))?;
    progress(1.0);
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub code: &'static str,
    pub name: &'static str,
    pub language: &'static str,
    pub notice: &'static str,
    pub mb: f32,
    pub installed: bool,
}

pub fn list(path: &Path) -> Vec<LibraryItem> {
    let installed = installed(path).unwrap_or_default();
    CATALOG
        .iter()
        .map(|e| LibraryItem {
            code: e.code,
            name: e.name,
            language: e.language,
            notice: e.notice,
            mb: e.mb,
            installed: installed.iter().any(|c| c.eq_ignore_ascii_case(e.code)),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripture::db::{set_library_path, BibleDb};

    fn sample_vpl() -> String {
        let mut s = String::from("\u{feff}GEN 1:1 Ní ìbẹ̀rẹ̀ ohun gbogbo Ọlọ́run dá àwọn ọ̀run àti ayé.\nJOH 3:16 Nítorí Ọlọ́run fẹ́ aráyé.\nNUM 12:16 \nXYZ 1:1 not a book\nbroken line\n");
        for i in 0..MIN_VERSES {
            s.push_str(&format!("PSA {}:{} filler\n", 200 + i / 200, i % 200 + 1));
        }
        s
    }

    #[test]
    fn parses_vpl_lines_and_codes() {
        let v = parse_vpl(&sample_vpl());
        assert_eq!((v[0].book_id, v[0].chapter, v[0].verse), (1, 1, 1));
        assert_eq!((v[1].book_id, v[1].chapter, v[1].verse), (43, 3, 16), "JOH is John");
        assert!(v.iter().all(|v| !v.text.is_empty()), "empty verses skipped");
        assert_eq!(v.len(), MIN_VERSES + 2);
        assert_eq!(book_id("SOL"), Some(22));
        assert_eq!(book_id("JHN"), Some(43));
        assert_eq!(book_id("REV"), Some(66));
        assert_eq!(book_names("yor")[42], "Johanu");
        assert!(CATALOG.iter().all(|e| book_names(e.source).len() == 66), "every catalogue entry has book names");
    }

    #[test]
    fn imported_translations_are_read_with_the_bundled_ones() {
        let dir = std::env::temp_dir().join(format!("hk-library-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let lib = dir.join("library.db");
        ensure(&lib).unwrap();
        let yor = entry("YOR").unwrap();
        import(&lib, yor, &parse_vpl(&sample_vpl())).unwrap();
        import(&lib, yor, &parse_vpl(&sample_vpl())).unwrap(); // re-download replaces
        assert!(import(&lib, yor, &parse_vpl("GEN 1:1 too short")).is_err());
        assert_eq!(installed(&lib).unwrap(), ["YOR"]);

        set_library_path(lib.clone());
        let bundled = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/bible.db");
        if bundled.exists() {
            let db = BibleDb::open(&bundled).unwrap();
            let t = db.translations();
            assert_eq!(t.last().map(|t| (t.code.as_str(), t.downloaded)), Some(("YOR", true)));
            assert!(t.last().unwrap().license.as_deref().unwrap().contains("CC BY-SA"));
            assert_eq!(db.passage("yor", 43, 3, 16, 16).unwrap()[0].text, "Nítorí Ọlọ́run fẹ́ aráyé.");
            assert_eq!(db.passage("KJV", 43, 3, 16, 16).unwrap().len(), 1);
            assert_eq!(db.book_name("YOR", 43).as_deref(), Some("Johanu"));
            assert_eq!(db.book_name("KJV", 43), None);
            // Searching a downloaded translation falls back to the bundled ones.
            assert!(!db.search(Some("YOR"), "\"loved\" \"world\"", 3).unwrap().is_empty());

            remove(&lib, "YOR").unwrap();
            assert!(db.passage("YOR", 43, 3, 16, 16).is_err(), "removed translations disappear at once");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Downloads real translations: `LIBRARY_DOWNLOAD=YOR,HAU cargo test real_download -- --ignored --nocapture`
    #[test]
    #[ignore = "downloads from eBible.org"]
    fn real_download() {
        let codes = std::env::var("LIBRARY_DOWNLOAD").unwrap_or_else(|_| "YOR".into());
        let lib = std::env::temp_dir().join(format!("hk-real-{}.db", std::process::id()));
        for code in codes.split(',') {
            let e = entry(code).unwrap();
            let t = std::time::Instant::now();
            install(&lib, e, |_| {}).unwrap();
            let conn = Connection::open(&lib).unwrap();
            let (n, john): (i64, String) = conn
                .query_row(
                    "SELECT (SELECT COUNT(*) FROM verses v JOIN translations t ON t.id = v.translation_id WHERE t.code = ?1),
                            (SELECT text FROM verses v JOIN translations t ON t.id = v.translation_id WHERE t.code = ?1 AND book_id = 43 AND chapter = 3 AND verse = 16)",
                    [e.code],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            println!("{code}: {n} verses in {:.1}s; John 3:16 = {john}", t.elapsed().as_secs_f32());
            assert!(n > 31_000);
        }
        let _ = std::fs::remove_file(&lib);
    }
}
