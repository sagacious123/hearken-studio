//! Per-day service log: what was shown, said and detected. JSON lines in
//! `<config>/history/YYYY-MM-DD.jsonl`, appended as the service runs so nothing is lost if the
//! app closes mid-service. Recaps, the sermon archive, insights and the CCLI report all read it.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    io::Write,
    path::PathBuf,
};

use chrono::{Datelike, Local, NaiveDate, TimeZone};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::scripture::{books, ScriptureRef, Slide};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Entry {
    /// Went to the output.
    #[serde(rename_all = "camelCase")]
    Shown {
        at: i64,
        label: String,
        translation: String,
        text: String,
        #[serde(default)]
        reference: Option<ScriptureRef>,
        #[serde(default)]
        song: Option<SongShown>,
    },
    /// A final transcript line.
    #[serde(rename_all = "camelCase")]
    Said { at: i64, text: String },
    /// Queued for the operator (shown or not).
    #[serde(rename_all = "camelCase")]
    Detected {
        at: i64,
        label: String,
        kind: String,
        confidence: f32,
        matched: String,
        #[serde(default)]
        reference: Option<ScriptureRef>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongShown {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub ccli: Option<String>,
}

pub struct History {
    dir: PathBuf,
    /// Consecutive duplicates (re-sent slides, follow-along highlights) aren't logged twice.
    last_shown: Mutex<Option<String>>,
}

fn now_ms() -> i64 {
    Local::now().timestamp_millis()
}

fn time_of(ms: i64) -> String {
    Local.timestamp_millis_opt(ms).single().map(|t| t.format("%H:%M").to_string()).unwrap_or_default()
}

fn slide_text(slide: &Slide) -> String {
    let sep = if slide.is_song() { " / " } else { " " };
    slide.verses.iter().map(|v| v.text.as_str()).collect::<Vec<_>>().join(sep)
}

impl History {
    pub fn new(dir: PathBuf) -> Self {
        if let Err(e) = fs::create_dir_all(&dir) {
            log::warn!("cannot create history dir {}: {e}", dir.display());
        }
        Self { dir, last_shown: Mutex::new(None) }
    }

    fn append(&self, entry: &Entry) {
        let path = self.dir.join(format!("{}.jsonl", Local::now().format("%Y-%m-%d")));
        let line = match serde_json::to_string(entry) {
            Ok(l) => l,
            Err(e) => return log::warn!("history serialise failed: {e}"),
        };
        let res = fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f| writeln!(f, "{line}"));
        if let Err(e) = res {
            log::warn!("history write failed ({}): {e}", path.display());
        }
    }

    pub fn shown(&self, slide: &Slide) {
        {
            let mut last = self.last_shown.lock();
            if last.as_deref() == Some(slide.id.as_str()) {
                return;
            }
            *last = Some(slide.id.clone());
        }
        self.append(&Entry::Shown {
            at: now_ms(),
            label: slide.label.clone(),
            translation: slide.translation.clone(),
            text: slide_text(slide),
            reference: (!slide.is_song()).then(|| slide.reference.clone()),
            song: slide.song.as_ref().map(|s| SongShown { id: s.song_id.clone(), title: s.title.clone(), ccli: s.ccli.clone() }),
        });
    }

    pub fn cleared(&self) {
        *self.last_shown.lock() = None;
    }

    pub fn said(&self, text: &str) {
        self.append(&Entry::Said { at: now_ms(), text: text.to_owned() });
    }

    pub fn detected(&self, slide: &Slide, kind: &str, confidence: f32, matched: &str) {
        self.append(&Entry::Detected {
            at: now_ms(),
            label: slide.label.clone(),
            kind: kind.to_owned(),
            confidence,
            matched: matched.to_owned(),
            reference: (!slide.is_song()).then(|| slide.reference.clone()),
        });
    }

    /// Logged days, newest first ("2026-10-11").
    pub fn days(&self) -> Vec<String> {
        let mut days: Vec<String> = fs::read_dir(&self.dir)
            .map(|it| {
                it.filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".jsonl").map(str::to_owned))
                    .filter(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok())
                    .collect()
            })
            .unwrap_or_default();
        days.sort_unstable_by(|a, b| b.cmp(a));
        days
    }

    pub fn load(&self, day: &str) -> Vec<Entry> {
        if NaiveDate::parse_from_str(day, "%Y-%m-%d").is_err() {
            return Vec::new(); // never turn user input into an arbitrary path
        }
        let Ok(text) = fs::read_to_string(self.dir.join(format!("{day}.jsonl"))) else { return Vec::new() };
        text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
    }

    pub fn summaries(&self) -> Vec<DaySummary> {
        self.days()
            .into_iter()
            .map(|date| {
                let entries = self.load(&date);
                let r = recap_from(&date, &entries);
                DaySummary {
                    scriptures: r.scriptures.len(),
                    songs: r.songs.len(),
                    words: r.words,
                    first_at: r.first_at.map(time_of),
                    last_at: r.last_at.map(time_of),
                    date,
                }
            })
            .collect()
    }

    pub fn recap(&self, day: &str) -> Recap {
        recap_from(day, &self.load(day))
    }

    /// Transcript lines and references matching `query`, newest first. A query that parses as
    /// a reference (`reference`) matches by passage; otherwise every word must appear.
    pub fn search(&self, query: &str, reference: Option<&ScriptureRef>, limit: usize) -> Vec<ArchiveHit> {
        let words: Vec<String> = query.to_lowercase().split_whitespace().filter(|w| w.len() > 1).map(str::to_owned).collect();
        let text_match = |t: &str| {
            let t = t.to_lowercase();
            !words.is_empty() && words.iter().all(|w| t.contains(w.as_str()))
        };
        let ref_match = |r: &Option<ScriptureRef>| match (reference, r) {
            (Some(q), Some(r)) => {
                q.book_id == r.book_id
                    && q.chapter == r.chapter
                    && match (q.verse_start, r.verse_start) {
                        (Some(qs), Some(rs)) => {
                            let (qe, re) = (q.verse_end.unwrap_or(qs), r.verse_end.unwrap_or(rs));
                            qs <= re && rs <= qe
                        }
                        _ => true,
                    }
            }
            _ => false,
        };

        let mut out = Vec::new();
        for date in self.days() {
            let mut seen = HashSet::new();
            for e in self.load(&date).into_iter().rev() {
                let hit = match e {
                    Entry::Said { at, text } if reference.is_none() && text_match(&text) => {
                        Some(ArchiveHit { date: date.clone(), time: time_of(at), kind: "said", label: None, text })
                    }
                    Entry::Shown { at, label, text, reference: r, .. } if ref_match(&r) || (reference.is_none() && text_match(&label)) => {
                        seen.insert(label.clone()).then(|| ArchiveHit { date: date.clone(), time: time_of(at), kind: "shown", label: Some(label), text })
                    }
                    Entry::Detected { at, label, matched, reference: r, .. } if ref_match(&r) && !seen.contains(&label) => {
                        seen.insert(label.clone());
                        Some(ArchiveHit { date: date.clone(), time: time_of(at), kind: "mentioned", label: Some(label), text: matched })
                    }
                    _ => None,
                };
                if let Some(h) = hit {
                    out.push(h);
                    if out.len() >= limit {
                        return out;
                    }
                }
            }
        }
        out
    }

    pub fn insights(&self, year: i32) -> Insights {
        let mut services = 0;
        let mut shown = 0;
        let mut books: HashMap<u8, usize> = HashMap::new();
        let mut verses: HashMap<String, usize> = HashMap::new();
        let mut songs: HashMap<String, usize> = HashMap::new();
        let (mut ot, mut nt) = (0, 0);
        for date in self.days() {
            let Ok(d) = NaiveDate::parse_from_str(&date, "%Y-%m-%d") else { continue };
            if d.year() != year {
                continue;
            }
            services += 1;
            let r = self.recap(&date);
            for s in &r.scriptures {
                shown += 1;
                *verses.entry(s.label.clone()).or_default() += 1;
                if let Some(book) = s.book_id {
                    *books.entry(book).or_default() += 1;
                    if book <= 39 {
                        ot += 1;
                    } else {
                        nt += 1;
                    }
                }
            }
            for s in &r.songs {
                *songs.entry(s.title.clone()).or_default() += 1;
            }
        }
        let top = |m: HashMap<String, usize>, n: usize| {
            let mut v: Vec<(String, usize)> = m.into_iter().collect();
            v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
            v.truncate(n);
            v
        };
        let books = top(books.into_iter().map(|(id, n)| (books::by_id(id).map_or("?", |b| b.name).to_owned(), n)).collect(), 12);
        Insights { year, services, scriptures_shown: shown, old_testament: ot, new_testament: nt, books, verses: top(verses, 10), songs: top(songs, 10) }
    }

    /// CCLI-style usage report: each song once per service day, for the last `days` days.
    pub fn ccli_csv(&self, days: i64) -> String {
        let since = Local::now().date_naive() - chrono::Duration::days(days);
        let mut uses: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for date in self.days() {
            let Ok(d) = NaiveDate::parse_from_str(&date, "%Y-%m-%d") else { continue };
            if d < since {
                continue;
            }
            for s in self.recap(&date).songs {
                uses.entry((s.title, s.ccli.unwrap_or_default())).or_default().push(date.clone());
            }
        }
        let mut csv = String::from("Song title,CCLI song number,Times used,Dates\n");
        for ((title, ccli), mut dates) in uses {
            dates.sort();
            csv.push_str(&format!("{},{},{},{}\n", csv_field(&title), csv_field(&ccli), dates.len(), csv_field(&dates.join(" "))));
        }
        csv
    }
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DaySummary {
    pub date: String,
    pub first_at: Option<String>,
    pub last_at: Option<String>,
    pub scriptures: usize,
    pub songs: usize,
    pub words: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveHit {
    pub date: String,
    pub time: String,
    pub kind: &'static str,
    pub label: Option<String>,
    pub text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Insights {
    pub year: i32,
    pub services: usize,
    pub scriptures_shown: usize,
    pub old_testament: usize,
    pub new_testament: usize,
    pub books: Vec<(String, usize)>,
    pub verses: Vec<(String, usize)>,
    pub songs: Vec<(String, usize)>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecapItem {
    pub time: String,
    pub label: String,
    pub translation: String,
    pub text: String,
    #[serde(skip)]
    pub book_id: Option<u8>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecapSong {
    pub time: String,
    pub title: String,
    pub ccli: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recap {
    pub date: String,
    /// Scriptures that went on screen, in order, each passage once.
    pub scriptures: Vec<RecapItem>,
    /// Detected but never shown.
    pub mentioned: Vec<RecapItem>,
    pub songs: Vec<RecapSong>,
    pub words: usize,
    #[serde(skip)]
    pub first_at: Option<i64>,
    #[serde(skip)]
    pub last_at: Option<i64>,
}

fn recap_from(date: &str, entries: &[Entry]) -> Recap {
    let mut r = Recap { date: date.to_owned(), scriptures: vec![], mentioned: vec![], songs: vec![], words: 0, first_at: None, last_at: None };
    let mut shown_labels = HashSet::new();
    let mut song_ids = HashSet::new();
    let mut mentioned: Vec<(i64, String)> = vec![];
    for e in entries {
        let at = match e {
            Entry::Shown { at, .. } | Entry::Said { at, .. } | Entry::Detected { at, .. } => *at,
        };
        r.first_at = Some(r.first_at.map_or(at, |f| f.min(at)));
        r.last_at = Some(r.last_at.map_or(at, |l| l.max(at)));
        match e {
            Entry::Shown { at, song: Some(s), .. } => {
                if song_ids.insert(s.id.clone()) {
                    r.songs.push(RecapSong { time: time_of(*at), title: s.title.clone(), ccli: s.ccli.clone() });
                }
            }
            Entry::Shown { at, label, translation, text, reference, song: None } => {
                if shown_labels.insert(label.clone()) {
                    r.scriptures.push(RecapItem {
                        time: time_of(*at),
                        label: label.clone(),
                        translation: translation.clone(),
                        text: text.clone(),
                        book_id: reference.as_ref().map(|r| r.book_id),
                    });
                }
            }
            Entry::Said { text, .. } => r.words += text.split_whitespace().count(),
            Entry::Detected { at, label, kind, .. } if kind != "song" => mentioned.push((*at, label.clone())),
            Entry::Detected { .. } => {}
        }
    }
    let mut seen = HashSet::new();
    for (at, label) in mentioned {
        if !shown_labels.contains(&label) && seen.insert(label.clone()) {
            r.mentioned.push(RecapItem { time: time_of(at), label, translation: String::new(), text: String::new(), book_id: None });
        }
    }
    r
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A self-contained, printable page (Save as PDF from the browser's print dialog).
pub fn recap_html(r: &Recap, church: &str) -> String {
    let date = NaiveDate::parse_from_str(&r.date, "%Y-%m-%d").map(|d| d.format("%A %-d %B %Y").to_string()).unwrap_or_else(|_| r.date.clone());
    let mut body = String::new();
    if r.scriptures.is_empty() {
        body.push_str("<p class=\"empty\">No scriptures were shown in this service.</p>");
    }
    for s in &r.scriptures {
        body.push_str(&format!(
            "<article><header><h2>{}</h2><span>{} · {}</span></header><p>{}</p></article>",
            esc(&s.label),
            esc(&s.translation),
            esc(&s.time),
            esc(&s.text)
        ));
    }
    if !r.mentioned.is_empty() {
        let items: Vec<String> = r.mentioned.iter().map(|m| esc(&m.label)).collect();
        body.push_str(&format!("<section><h3>Also mentioned</h3><p>{}</p></section>", items.join(" · ")));
    }
    if !r.songs.is_empty() {
        let items: Vec<String> = r.songs.iter().map(|s| esc(&s.title)).collect();
        body.push_str(&format!("<section><h3>Songs</h3><p>{}</p></section>", items.join(" · ")));
    }
    let heading = if church.trim().is_empty() { "Sermon scriptures".to_owned() } else { esc(church.trim()) };
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{heading} — {date}</title>
<style>
:root{{--ink:#1c1917;--muted:#78716c;--line:#e7e5e4;--accent:#b45309;--bg:#fafaf9}}
@media (prefers-color-scheme:dark){{:root{{--ink:#f5f5f4;--muted:#a8a29e;--line:#292524;--accent:#fbbf24;--bg:#0c0a09}}}}
body{{margin:0;background:var(--bg);color:var(--ink);font:17px/1.6 Georgia,"Iowan Old Style",serif}}
main{{max-width:720px;margin:0 auto;padding:48px 20px}}
h1{{font:600 30px/1.2 system-ui,sans-serif;margin:0}} .date{{color:var(--muted);font-family:system-ui,sans-serif;margin:6px 0 36px}}
article{{border-top:1px solid var(--line);padding:18px 0}} article header{{display:flex;justify-content:space-between;align-items:baseline;gap:12px}}
h2{{font:600 19px system-ui,sans-serif;color:var(--accent);margin:0}} article header span{{color:var(--muted);font:13px system-ui,sans-serif;white-space:nowrap}}
article p{{margin:8px 0 0}} section{{border-top:1px solid var(--line);padding:18px 0}} h3{{font:600 14px system-ui,sans-serif;text-transform:uppercase;letter-spacing:.06em;color:var(--muted);margin:0 0 6px}}
footer{{margin-top:36px;color:var(--muted);font:12px system-ui,sans-serif}} .empty{{color:var(--muted)}}
@media print{{body{{background:#fff;color:#000}} main{{padding:0}} article{{break-inside:avoid}}}}
</style></head><body><main><h1>{heading}</h1><p class="date">{date}</p>{body}<footer>Recap made with Hearken Studio</footer></main></body></html>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scripture::{SongPart, VerseLine};

    fn slide(id: &str, label: &str, book: u8) -> Slide {
        Slide {
            id: id.into(),
            label: label.into(),
            translation: "KJV".into(),
            reference: ScriptureRef { book_id: book, chapter: 3, verse_start: Some(16), verse_end: None },
            verses: vec![VerseLine { verse: 16, text: "For God so loved the world".into() }],
            secondary: None,
            song: None,
        }
    }

    #[test]
    fn logs_recaps_searches_and_reports() {
        let dir = std::env::temp_dir().join(format!("sb-history-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let h = History::new(dir.clone());
        let john = slide("KJV:43.3.16", "John 3:16", 43);
        h.said("turn with me to John 3 16");
        h.detected(&john, "explicit", 0.95, "john 3 16");
        h.shown(&john);
        h.shown(&john); // re-sent: logged once
        h.detected(&slide("KJV:45.8.28", "Romans 8:28", 45), "explicit", 0.9, "romans 8 28");
        let mut song = slide("song:amazing:0", "Amazing Grace · Verse 1", 0);
        song.song = Some(SongPart { song_id: "amazing".into(), title: "Amazing Grace".into(), section: 0, sections: 4, ccli: Some("22025".into()) });
        h.shown(&song);

        let today = h.days().remove(0);
        let r = h.recap(&today);
        assert_eq!(r.scriptures.len(), 1);
        assert_eq!(r.mentioned.iter().map(|m| m.label.as_str()).collect::<Vec<_>>(), ["Romans 8:28"]);
        assert_eq!(r.songs.len(), 1);
        assert!(recap_html(&r, "Grace Chapel").contains("John 3:16"));

        let kinds: Vec<_> = h.search("john 3", None, 10).iter().map(|x| x.kind).collect();
        assert_eq!(kinds, ["shown", "said"]); // newest first: the slide, then what was said
        let q = ScriptureRef { book_id: 43, chapter: 3, verse_start: None, verse_end: None };
        assert_eq!(h.search("", Some(&q), 10)[0].kind, "shown");
        assert!(h.ccli_csv(30).contains("Amazing Grace,22025,1,"));
        assert_eq!(h.insights(Local::now().year()).books[0].0, "John");
        assert!(h.load("../../etc/passwd").is_empty());
        let _ = fs::remove_dir_all(&dir);
    }
}
