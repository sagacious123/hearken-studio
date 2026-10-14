//! Layer 1: deterministic reference detection for explicit announcements.
//!
//! Runs on every partial and final transcript (~20–50 µs per call), so it has to be cheap:
//! one precompiled regex over normalised text, then validation against the versification.

use std::{
    collections::HashMap,
    sync::LazyLock,
    time::{Duration, Instant},
};

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};

use super::{books, numbers, ScriptureRef, Versification};

/// How long "verse 18" / "chapter 5" may refer back to the last explicit reference.
const CONTEXT_TTL: Duration = Duration::from_secs(180);
/// "…from verse 10" this soon after the chapter was named is as good as an announcement.
const FRESH_CONTEXT: Duration = Duration::from_secs(30);
/// Speech recognition splits sentences at short pauses ("Let's begin from verse." / "10.").
/// A sentence that ends mid-reference is read together with the next one if it follows this soon.
const STITCH_WINDOW: Duration = Duration::from_secs(8);
/// Words a reference can't end on: after these, the rest is still to come.
const DANGLING: &[&str] = &["chapter", "chapters", "verse", "verses", "vs", "v", "from", "to", "through", "thru", "till", "until", "and", "2", "too"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefKind {
    /// "John 3:16"
    Explicit,
    /// "verse 18" resolved against the previous reference
    Contextual,
    /// Verse text recognised via full-text search (layer 2)
    Quote,
    /// An imported worship song recognised from the singing
    Song,
    /// A verse said in other words, found by meaning
    Meaning,
    /// A spoken command ("next verse", "go to verse 6") that moved the live scripture
    Command,
}

impl RefKind {
    pub fn name(self) -> &'static str {
        match self {
            RefKind::Explicit => "explicit",
            RefKind::Contextual => "contextual",
            RefKind::Quote => "quote",
            RefKind::Song => "song",
            RefKind::Meaning => "meaning",
            RefKind::Command => "command",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedRef {
    pub reference: ScriptureRef,
    pub confidence: f32,
    /// The match touches the end of a partial transcript, so more words may still change it
    /// ("John 3" may be about to become "John 3:16"). Never auto-projected.
    pub tentative: bool,
    pub kind: RefKind,
    pub matched: String,
}

struct Patterns {
    full: Regex,
    /// "<word> 4:13" where the word isn't a known book name; it may be a misheard one.
    fuzzy: Regex,
    verse_ctx: Regex,
    chapter_ctx: Regex,
    aliases: HashMap<String, (u8, bool)>,
}

// "to" is often heard as "2" or "too" ("verse 10 2 12"); as a separator it needs spaces either side.
const RANGE: &str = r"(?:(?:\s*(?P<rsep>-|to|through|thru|till|until|and)\s*|\s+(?:2|too)\s+)(?P<ve>\d{1,3}))?";
// "-" between chapter and verse is Whisper writing "John 3-16" / "three-sixteen" for a spoken "John 3 16".
const SEP: &str = r"(?P<sep>\s*:\s*|\s*-\s*|\s+(?:and\s+)?(?:verses?|vs|v)\s+|\s+)";
/// Followed by numbers these are keywords or everyday speech ("hymn number 4 12"), never
/// misheard book names. The real "Numbers 4:12" still matches exactly.
const NOT_BOOKS: &[&str] = &["chapter", "chapters", "verse", "verses", "number"];
/// Confidence cost of a book name recognised only by sound; keeps such hits out of auto-projection.
const FUZZY_BOOK_PENALTY: f32 = 0.2;
/// Confidence cost of a known mishearing ("Exodos"): nearly as sure as the real name.
pub const MISHEARD_PENALTY: f32 = 0.05;

static PATTERNS: LazyLock<Patterns> = LazyLock::new(|| {
    let mut aliases: HashMap<String, (u8, bool)> = HashMap::new();
    for (alias, id, weak) in books::all_aliases() {
        aliases.entry(alias).or_insert((id, weak));
    }
    let mut list: Vec<&String> = aliases.keys().collect();
    // Longest first so "1 john" wins over "john" and "song of songs" over "song".
    list.sort_by(|a, b| b.len().cmp(&a.len()).then(a.cmp(b)));
    let alt = list.iter().map(|a| regex::escape(a).replace(' ', r"\s+")).collect::<Vec<_>>().join("|");

    // Chapter allows 4 digits so "john 316" can be split into 3:16 during validation.
    let full = format!(r"\b(?P<book>{alt})\s+(?P<chkw>chapters?\s+)?(?P<ch>\d{{1,4}})(?:{SEP}(?P<vs>\d{{1,3}}){RANGE})?\b");
    // A verse number (or run-together digits) is required: "<word> <number>" alone is everyday speech.
    let fuzzy = format!(
        r"\b(?:(?P<ord>1|2|3|first|second|third|1st|2nd|3rd|ii|iii)\s+)?(?P<word>[a-z]{{4,}})\s+(?P<chkw>chapters?\s+)?(?P<ch>\d{{1,4}})(?:{SEP}(?P<vs>\d{{1,3}}){RANGE})?\b"
    );
    let verse_ctx = format!(r"\b(?:verses?|vs)\s+(?P<vs>\d{{1,3}}){RANGE}\b");
    let chapter_ctx = format!(
        r"\bchapters?\s+(?P<ch>\d{{1,3}})(?:(?P<sep>\s*:\s*|\s+(?:and\s+)?(?:verses?|vs|v)\s+)(?P<vs>\d{{1,3}}){RANGE})?\b"
    );
    Patterns {
        full: Regex::new(&full).expect("full reference regex"),
        fuzzy: Regex::new(&fuzzy).expect("fuzzy reference regex"),
        verse_ctx: Regex::new(&verse_ctx).expect("verse regex"),
        chapter_ctx: Regex::new(&chapter_ctx).expect("chapter regex"),
        aliases,
    }
});

/// Forces regex compilation (≈10 ms) at startup instead of on the first transcript.
pub fn warm_up() {
    LazyLock::force(&PATTERNS);
}

#[derive(Clone, Copy)]
struct Context {
    book_id: u8,
    chapter: u16,
    at: Instant,
}

enum Hit<'t> {
    Full(Captures<'t>),
    /// A word that sounds like a book name, with its distance from the nearest spelling.
    Fuzzy(Captures<'t>, &'static books::Book, usize),
    Verse(Captures<'t>),
    Chapter(Captures<'t>),
}

/// Stateful only for "verse 18"-style follow-ups; one instance per transcript stream.
#[derive(Default)]
pub struct ReferenceParser {
    context: Option<Context>,
    /// Recent finished sentences, normalised, for references split across them.
    recent: Vec<(Instant, String)>,
}

impl ReferenceParser {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn parse(&mut self, raw: &str, is_final: bool, v11n: &Versification) -> Vec<ParsedRef> {
        let p = &*PATTERNS;
        let current = numbers::normalize(raw);
        // A reference split by a pause: read the end of the previous sentence with this one, and
        // keep only what reaches into this one (the rest was handled last time).
        let prefix = self.dangling_prefix(&p.aliases);
        let (text, offset) = if prefix.is_empty() { (current.clone(), 0) } else { (format!("{prefix} {current}"), prefix.len() + 1) };
        if is_final {
            self.recent.retain(|(at, _)| at.elapsed() < STITCH_WINDOW);
            self.recent.push((Instant::now(), current.clone()));
            if self.recent.len() > 3 {
                self.recent.remove(0);
            }
        }
        let text_end = text.len();

        let mut hits: Vec<(usize, Hit)> = Vec::new();
        let mut spans = Vec::new();
        for c in p.full.captures_iter(&text) {
            let m = c.get(0).unwrap();
            spans.push(m.range());
            hits.push((m.start(), Hit::Full(c)));
        }
        let mut fuzzy = Vec::new();
        for c in p.fuzzy.captures_iter(&text) {
            let m = c.get(0).unwrap();
            let word = &c["word"];
            if spans.iter().any(|r| r.contains(&m.start())) || p.aliases.contains_key(word) || NOT_BOOKS.contains(&word) {
                continue;
            }
            let ordinal = c.name("ord").and_then(|o| books::ordinal_value(o.as_str()));
            if let Some((book, edits)) = books::fuzzy_match(word, ordinal) {
                fuzzy.push((m.range(), c, book, edits));
            }
        }
        for (range, c, book, edits) in fuzzy {
            hits.push((range.start, Hit::Fuzzy(c, book, edits)));
            spans.push(range);
        }
        let overlaps = |s: usize| spans.iter().any(|r| r.contains(&s));
        for c in p.chapter_ctx.captures_iter(&text) {
            let s = c.get(0).unwrap().start();
            if !overlaps(s) {
                hits.push((s, Hit::Chapter(c)));
            }
        }
        for c in p.verse_ctx.captures_iter(&text) {
            let s = c.get(0).unwrap().start();
            let in_chapter_hit = hits.iter().any(|(_, h)| matches!(h, Hit::Chapter(cc) if cc.get(0).unwrap().range().contains(&s)));
            if !overlaps(s) && !in_chapter_hit {
                hits.push((s, Hit::Verse(c)));
            }
        }
        hits.sort_by_key(|(s, _)| *s);

        let mut out = Vec::new();
        for (_, hit) in hits {
            let (caps, kind) = match &hit {
                Hit::Full(c) | Hit::Fuzzy(c, _, _) => (c, RefKind::Explicit),
                Hit::Chapter(c) | Hit::Verse(c) => (c, RefKind::Contextual),
            };
            let m = caps.get(0).unwrap();
            if m.end() <= offset {
                continue; // entirely in the previous sentence
            }
            let tentative = !is_final && m.end() >= text_end;
            let vs = num(caps, "vs");
            let ve = num(caps, "ve");
            let and_range = caps.name("rsep").is_some_and(|r| r.as_str() == "and");
            let explicit_sep = caps.name("sep").is_some_and(|s| !s.as_str().trim().is_empty());

            let resolved = match &hit {
                Hit::Full(c) => {
                    let alias = c["book"].split_whitespace().collect::<Vec<_>>().join(" ");
                    let Some(&(book_id, weak)) = p.aliases.get(&alias) else { continue };
                    let Some(book) = books::by_id(book_id) else { continue };
                    let raw_ch: u32 = c["ch"].parse().unwrap_or(0);
                    let Some((r, split)) = resolve(book, raw_ch, vs, ve, and_range, v11n) else { continue };
                    // "John 3 16" is as explicit as "John 3:16": the speaker doesn't pronounce the
                    // colon, Whisper just may or may not write one.
                    let mut conf: f32 = if r.verse_start.is_some() { 0.95 } else { 0.7 };
                    if books::is_misheard(&alias) {
                        conf -= MISHEARD_PENALTY;
                    }
                    if split {
                        conf -= 0.1;
                    }
                    if weak && c.name("chkw").is_none() && !explicit_sep {
                        // "mark 2 things" vs "Mark 2 17": a verse number is much stronger evidence.
                        conf -= if r.verse_start.is_some() { 0.1 } else { 0.25 };
                    }
                    Some((r, conf))
                }
                Hit::Fuzzy(c, book, edits) => {
                    let raw_ch: u32 = c["ch"].parse().unwrap_or(0);
                    let Some((r, split)) = resolve(book, raw_ch, vs, ve, and_range, v11n) else { continue };
                    // A chapter alone needs more: "chapter" said ("Exodos chapter 20"), or a word
                    // one letter off a book name ("Exodis 20"). "romance 8" stays everyday speech.
                    let near = *edits <= 1 && c["word"].len() >= 5 && !book.ambiguous;
                    if r.verse_start.is_none() && c.name("chkw").is_none() && !near {
                        continue;
                    }
                    let mut conf = if r.verse_start.is_some() { 0.95 } else { 0.7 } - FUZZY_BOOK_PENALTY;
                    if split {
                        conf -= 0.1;
                    }
                    if book.ambiguous {
                        conf -= 0.1;
                    }
                    Some((r, conf))
                }
                Hit::Chapter(c) => {
                    let ctx = self.live_context();
                    ctx.and_then(|ctx| {
                        let ch: u16 = c["ch"].parse().ok()?;
                        build(ctx.book_id, ch, vs, ve, and_range, v11n).map(|r| (r, 0.7))
                    })
                }
                Hit::Verse(_) => {
                    let ctx = self.live_context();
                    ctx.and_then(|ctx| {
                        let conf = if ctx.at.elapsed() < FRESH_CONTEXT { 0.85 } else { 0.75 };
                        build(ctx.book_id, ctx.chapter, vs, ve, and_range, v11n).filter(|r| r.verse_start.is_some()).map(|r| (r, conf))
                    })
                }
            };
            let Some((reference, confidence)) = resolved else { continue };

            if !tentative {
                self.context = Some(Context { book_id: reference.book_id, chapter: reference.chapter, at: Instant::now() });
            }
            out.push(ParsedRef { reference, confidence: confidence.clamp(0.0, 1.0), tentative, kind, matched: m.as_str().to_owned() });
        }
        out
    }

    /// The last few words of the previous sentence(s), if they stop mid-reference ("…from
    /// verse", "…Esther chapter 3", "…verse 10 to") and were said within the stitch window.
    fn dangling_prefix(&self, aliases: &HashMap<String, (u8, bool)>) -> String {
        let Some((last_at, _)) = self.recent.last() else { return String::new() };
        if last_at.elapsed() >= STITCH_WINDOW {
            return String::new();
        }
        let words: Vec<&str> = self.recent.iter().filter(|(at, _)| at.elapsed() < STITCH_WINDOW).flat_map(|(_, t)| t.split_whitespace()).collect();
        let Some(&last) = words.last() else { return String::new() };
        let dangling = DANGLING.contains(&last) || last.chars().all(|c| c.is_ascii_digit() || c == ':') || aliases.contains_key(last);
        if !dangling {
            return String::new();
        }
        words[words.len().saturating_sub(10)..].join(" ")
    }

    fn live_context(&self) -> Option<Context> {
        self.context.filter(|c| c.at.elapsed() < CONTEXT_TTL)
    }
}

fn num(caps: &Captures, name: &str) -> Option<u16> {
    caps.name(name).and_then(|m| m.as_str().parse().ok())
}

/// Validates against the versification; splits run-together digits ("316" → 3:16) when the
/// literal chapter can't exist.
fn resolve(
    book: &books::Book,
    raw_ch: u32,
    vs: Option<u16>,
    ve: Option<u16>,
    and_range: bool,
    v11n: &Versification,
) -> Option<(ScriptureRef, bool)> {
    if (1..=book.chapters as u32).contains(&raw_ch) {
        return build(book.id, raw_ch as u16, vs, ve, and_range, v11n).map(|r| (r, false));
    }
    if vs.is_some() || raw_ch < 10 {
        return None;
    }
    let digits = raw_ch.to_string();
    (1..digits.len()).find_map(|i| {
        let (a, b) = digits.split_at(i);
        if b.starts_with('0') {
            return None;
        }
        let ch: u16 = a.parse().ok()?;
        let v: u16 = b.parse().ok()?;
        if ch == 0 || ch > book.chapters {
            return None;
        }
        build(book.id, ch, Some(v), ve, and_range, v11n).map(|r| (r, true))
    })
}

fn build(book_id: u8, chapter: u16, vs: Option<u16>, ve: Option<u16>, and_range: bool, v11n: &Versification) -> Option<ScriptureRef> {
    let max = v11n.max_verse(book_id, chapter)?;
    let verse_start = match vs {
        None => None,
        Some(v) if (1..=max).contains(&v) => Some(v),
        Some(_) => return None,
    };
    // "verses 4 and 5" is a range; "verse 4 and 9" is not.
    let verse_end = verse_start.and_then(|s| ve.filter(|&e| e > s && e <= max && (!and_range || e == s + 1)));
    Some(ScriptureRef { book_id, chapter, verse_start, verse_end })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v11n() -> Versification {
        let mut m = HashMap::new();
        for (book, ch, max) in [
            (2u8, 20u16, 26u16), // Exodus 20
            (2, 3, 22),
            (5, 6, 25),   // Deuteronomy 6
            (60, 5, 14),  // 1 Peter 5
            (17, 3, 15),  // Esther 3
            (66, 21, 27), // Revelation 21
            (35, 2, 20),  // Habakkuk 2
            (26, 37, 28), // Ezekiel 37
            (48, 5, 26),  // Galatians 5
            (40, 5, 48),  // Matthew 5
            (43, 3, 36), // John 3
            (43, 1, 51),
            (45, 8, 39),  // Romans 8
            (19, 23, 6),  // Psalm 23
            (19, 119, 176),
            (46, 13, 13), // 1 Corinthians 13
            (62, 4, 21),  // 1 John 4
            (41, 2, 28),  // Mark 2
            (50, 4, 23),  // Philippians 4
            (23, 53, 12), // Isaiah 53
        ] {
            m.insert((book, ch), max);
        }
        Versification::from_map(m)
    }

    fn parse_one(text: &str) -> ParsedRef {
        let refs = ReferenceParser::new().parse(text, true, &v11n());
        assert!(!refs.is_empty(), "no reference found in {text:?}");
        refs.into_iter().next().unwrap()
    }

    fn sr(book_id: u8, chapter: u16, s: Option<u16>, e: Option<u16>) -> ScriptureRef {
        ScriptureRef { book_id, chapter, verse_start: s, verse_end: e }
    }

    #[test]
    fn explicit_forms() {
        assert_eq!(parse_one("Open your Bibles to John 3:16").reference, sr(43, 3, Some(16), None));
        assert_eq!(parse_one("john chapter three verse sixteen").reference, sr(43, 3, Some(16), None));
        assert_eq!(parse_one("Romans 8, 28").reference, sr(45, 8, Some(28), None));
        assert_eq!(parse_one("First Corinthians 13 verses 4 through 7").reference, sr(46, 13, Some(4), Some(7)));
        assert_eq!(parse_one("1 Cor 13:4-7").reference, sr(46, 13, Some(4), Some(7)));
        assert_eq!(parse_one("Psalm one hundred and nineteen verse 105").reference, sr(19, 119, Some(105), None));
        assert_eq!(parse_one("first john four eight").reference, sr(62, 4, Some(8), None));
        assert_eq!(parse_one("Isaiah 53 verses 4 and 5").reference, sr(23, 53, Some(4), Some(5)));
    }

    #[test]
    fn chapter_only_and_confidence() {
        let r = parse_one("The Lord is my shepherd, Psalm 23.");
        assert_eq!(r.reference, sr(19, 23, None, None));
        assert!(r.confidence < 0.8);
        // No "chapter"/"verse" needed, and no colon: Whisper may or may not write one.
        for spoken in ["John 3:16", "John 3 16", "John three sixteen", "John 3, 16", "John 3-16", "John three-sixteen"] {
            let r = parse_one(spoken);
            assert_eq!(r.reference, sr(43, 3, Some(16), None), "{spoken}");
            assert!(r.confidence >= 0.95, "{spoken}: {}", r.confidence);
        }
        assert!(parse_one("John 316").confidence < parse_one("John 3 16").confidence);
    }

    #[test]
    fn ambiguous_book_names_are_penalised() {
        let r = parse_one("I want to mark 2 things");
        assert!(r.confidence < 0.7, "got {}", r.confidence);
        assert!(parse_one("Mark 2:17").confidence >= 0.9);
    }

    #[test]
    fn run_together_digits() {
        let r = parse_one("John 316");
        assert_eq!(r.reference, sr(43, 3, Some(16), None));
    }

    #[test]
    fn misheard_book_names() {
        for (heard, want) in [
            ("open to Philippines 4:13", sr(50, 4, Some(13), None)),
            ("Romance 8 28", sr(45, 8, Some(28), None)),
            ("Fillipians four thirteen", sr(50, 4, Some(13), None)),
            ("first Corinthean 13 4", sr(46, 13, Some(4), None)),
            ("Romance 828", sr(45, 8, Some(28), None)),
        ] {
            let r = parse_one(heard);
            assert_eq!(r.reference, want, "{heard}");
            assert_eq!(r.kind, RefKind::Explicit);
            assert!(r.confidence < 0.9, "sound-alike book names never auto-project: {heard} {}", r.confidence);
        }
        let v = v11n();
        for talk in ["Romance 8", "about 3 16 year olds", "chapter 3 16", "we have 3 4 more songs", "times 3 4", "hymn number 4 12"] {
            assert!(ReferenceParser::new().parse(talk, true, &v).is_empty(), "{talk}");
        }
        // A real book name is never second-guessed.
        assert_eq!(parse_one("John 3:16").confidence, 0.95);
    }

    #[test]
    fn common_mishearings_and_accents() {
        for (heard, want) in [
            ("Exodos 20", sr(2, 20, None, None)),
            ("Exodos chapter 20 verse 3", sr(2, 20, Some(3), None)),
            ("turn to Exodos 3:14", sr(2, 3, Some(14), None)),
            ("Deutronomy 6 verse 5", sr(5, 6, Some(5), None)),
            ("Isaya 53 5", sr(23, 53, Some(5), None)),
            ("first Pita 5:7", sr(60, 5, Some(7), None)),
            ("Ezekial 37", sr(26, 37, None, None)),
            ("Habakok 2 verse 2", sr(35, 2, Some(2), None)),
            ("Habbakuk 2:2", sr(35, 2, Some(2), None)),
            ("Galations 5 22", sr(48, 5, Some(22), None)),
            ("Revalation 21 4", sr(66, 21, Some(4), None)),
            ("Mathew 5", sr(40, 5, None, None)),
        ] {
            let r = parse_one(heard);
            assert_eq!(r.reference, want, "{heard}");
            let full = if want.verse_start.is_some() { 0.95 } else { 0.7 };
            assert!((r.confidence - (full - MISHEARD_PENALTY)).abs() < 1e-6, "{heard}: {}", r.confidence);
        }
        // Not in the list but one letter off, or with "chapter" said: understood, cautiously.
        for (heard, want) in [("Exodis 20", sr(2, 20, None, None)), ("Exoduss chapter 3", sr(2, 3, None, None))] {
            let r = parse_one(heard);
            assert_eq!(r.reference, want, "{heard}");
            assert!(r.confidence < 0.6, "{heard}: {}", r.confidence);
        }
        let v = v11n();
        for talk in ["a roman 3 soldiers", "the king 3 times", "Romance 8", "jon 2 days ago", "hebrew 2 words"] {
            let refs = ReferenceParser::new().parse(talk, true, &v);
            assert!(refs.iter().all(|r| r.confidence < 0.7), "{talk}: {refs:?}");
        }
        // Mishearings show up when typing in the passage picker too.
        assert!(books::all_aliases().iter().any(|(a, id, _)| a == "exodos" && *id == 2));
    }

    #[test]
    fn references_split_by_pauses() {
        let mut m = HashMap::new();
        m.insert((17u8, 3u16), 15u16); // Esther 3
        let v = Versification::from_map(m);
        let last = |p: &mut ReferenceParser, parts: &[&str]| {
            let mut out = Vec::new();
            for part in parts {
                out = p.parse(part, true, &v);
            }
            out
        };
        // What speech recognition actually produced for these, sentence by sentence.
        let r = last(&mut ReferenceParser::new(), &["Esther chapter 3.", "Let's begin from verse.", "10."]);
        assert_eq!(r.first().map(|r| r.reference.clone()), Some(sr(17, 3, Some(10), None)));
        assert!(r[0].confidence >= 0.85, "chapter just named: {}", r[0].confidence);
        let r = last(&mut ReferenceParser::new(), &["Esther chapter 3.", "From verse 10.", "2.", "12."]);
        assert_eq!(r.first().map(|r| r.reference.clone()), Some(sr(17, 3, Some(10), Some(12))), "{r:?}");
        let r = last(&mut ReferenceParser::new(), &["Turn with me to Esther.", "Chapter 3, verse 10."]);
        assert_eq!(r.first().map(|r| r.reference.clone()), Some(sr(17, 3, Some(10), None)));
        // Words in between, in one sentence.
        assert_eq!(parse_one("Esther chapter 3, let's begin from verse 10").reference, sr(17, 3, None, None));
        let r = ReferenceParser::new().parse("Esther chapter 3, let's begin from verse 10", true, &v);
        assert_eq!(r.last().map(|r| r.reference.clone()), Some(sr(17, 3, Some(10), None)));
        // "to" heard as "2" or "too".
        let r = ReferenceParser::new().parse("Esther 3 verse 10 2 12", true, &v);
        assert_eq!(r[0].reference, sr(17, 3, Some(10), Some(12)));
        // Esther misheard.
        assert_eq!(ReferenceParser::new().parse("Asta 3-10", true, &v)[0].reference, sr(17, 3, Some(10), None));
        // An ordinary sentence ending in a number isn't glued to the next one's verse.
        let r = last(&mut ReferenceParser::new(), &["We were there for 3 years.", "God is good."]);
        assert!(r.is_empty());
        // Nothing is detected twice: the previous sentence's reference isn't reported again.
        let mut p = ReferenceParser::new();
        p.parse("Read Esther 3 10 and", true, &v);
        assert!(p.parse("let us pray", true, &v).is_empty());
    }

    #[test]
    fn rejects_impossible_references() {
        assert!(ReferenceParser::new().parse("John 3:99", true, &v11n()).is_empty());
        assert!(ReferenceParser::new().parse("John 45", true, &v11n()).is_empty());
    }

    #[test]
    fn contextual_follow_ups() {
        let v = v11n();
        let mut p = ReferenceParser::new();
        p.parse("Turn with me to Philippians 4:6", true, &v);
        let r = p.parse("and look at verse 13", true, &v);
        assert_eq!(r[0].reference, sr(50, 4, Some(13), None));
        assert_eq!(r[0].kind, RefKind::Contextual);
    }

    #[test]
    fn partial_at_end_is_tentative() {
        let v = v11n();
        let r = ReferenceParser::new().parse("turn to John 3", false, &v);
        assert!(r[0].tentative);
        let r = ReferenceParser::new().parse("turn to John 3:16 for God so loved", false, &v);
        assert!(!r[0].tentative);
    }
}
