//! Layer 2: recognise scripture that is *quoted or read aloud* without being announced, even
//! when the ASR mishears a word or two.
//!
//! One FTS5 query ORs every three-word window of the latest final utterance (and every pair of
//! adjacent content words) as a phrase, so a single misheard word leaves other windows to find
//! the verse. Each candidate is then aligned against the transcript with a local (Smith-Waterman)
//! word alignment: exact words score, sound-alikes ("seizing"/"ceasing", "shepard"/"shepherd")
//! score a little less, and wrong, missing or extra words cost, so one bad word inside a real
//! quote is absorbed instead of splitting it. Words are stemmed, with KJV "-eth"/"-est" endings
//! folded so "believeth" meets "believes". Short phrases found in many verses ("and it came to
//! pass", "the word of God") are dropped as generic. Runs only on finals, off the partial path.

use std::collections::HashMap;
use std::collections::HashSet;

use anyhow::Result;
use rust_stemmers::{Algorithm, Stemmer};

use super::{fuzzy, BibleDb, ScriptureRef};

/// ≤ 64 so a verse's coverage of the transcript fits in a `u64` mask.
const TAIL_WORDS: usize = 40;
const WINDOW: usize = 3;
const CANDIDATES: usize = 200;

// Alignment scores. A mismatch costs more than a stopword earns, so a quote can't be stitched
// together from scattered pieces ("it is good … to see"), but a long run absorbs one bad word.
const EXACT_CONTENT: f32 = 3.0;
const EXACT_STOPWORD: f32 = 1.0;
const SIMILAR_CONTENT: f32 = 2.0;
const SIMILAR_STOPWORD: f32 = 1.0;
const MISMATCH: f32 = -2.0;
const GAP: f32 = -1.5;
/// How much a sound-alike counts towards phrase length and accuracy, relative to an exact word.
const SIMILAR_WEIGHT: f32 = 0.7;

/// A match needs two exact words, at most one error per three matched words, and either three
/// content words ("pray without ceasing") or two inside five matched words ("the Lord is my
/// shepherd"); the longer bar for two content words rejects everyday phrases ("turn to your
/// neighbour").
const MIN_EXACT: u32 = 2;
const MIN_MATCHED_TWO_CONTENT: u32 = 5;
/// A phrase with this few content words that occurs in more than `MAX_GENERIC_VERSES` verses is generic.
const GENERIC_CONTENT: u32 = 3;
const MAX_GENERIC_VERSES: usize = 3;
/// Verses covering this share of the best match's words are rivals for the same phrase.
const RIVAL_OVERLAP: f32 = 0.8;
const AMBIGUITY_PENALTY: f32 = 0.85;
/// Alternatives are offered only when they score close to the best match.
const ALTERNATIVE_RATIO: f32 = 0.85;
/// Below this, a match is padding around two common words ("the Lord has been so good to").
/// Real quotes, misheard ones included, score 0.65 and up.
const MIN_SCORE: f32 = 0.6;
pub const MAX_HITS: usize = 3;

const STOPWORDS: &[&str] = &[
    "the", "and", "of", "to", "a", "in", "that", "is", "for", "it", "as", "was", "with", "be", "his", "he", "i",
    "you", "on", "by", "not", "but", "are", "this", "they", "have", "from", "or", "at", "all", "we", "my", "me",
    "your", "so", "will", "unto", "thee", "thou", "thy", "shall", "him", "them", "which", "there", "what", "when",
    "who", "were", "their", "an", "if", "our", "us", "do", "no", "hath", "ye", "then", "said", "into", "up", "out",
    "now", "also", "been", "had", "has", "her", "she", "its", "these", "those", "would", "could", "let", "like",
    "just", "very", "okay", "amen", "can", "say", "says", "saying", "know", "going", "get", "got", "one",
];

#[derive(Debug, Clone)]
pub struct QuoteHit {
    pub reference: ScriptureRef,
    pub score: f32,
    /// The words of the transcript that matched, for the operator's detection card.
    pub phrase: String,
}

struct Token {
    word: String,
    stem: String,
    /// Rough phonetic key, so sound-alikes can be recognised.
    sound: String,
    content: bool,
}

/// Lowercase alphanumeric runs, split exactly where FTS5's unicode61 tokenizer splits
/// ("God's" → "god", "s"), so phrase queries line up with the index.
fn split(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_lowercase).collect()
}

fn token(stemmer: &Stemmer, word: &str) -> Token {
    // "believeth"/"lovest" → "believe"/"love", which then stem like "believes"/"loved".
    // Applied to transcript and verse alike, so a wrong fold ("priest") can't cause a mismatch.
    let folded;
    let w = if word.len() >= 6 && (word.ends_with("eth") || word.ends_with("est")) {
        folded = format!("{}e", &word[..word.len() - 3]);
        folded.as_str()
    } else {
        word
    };
    Token {
        word: word.to_string(),
        stem: stemmer.stem(w).into_owned(),
        sound: fuzzy::sound(word),
        content: word.len() > 2 && !STOPWORDS.contains(&word),
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Pair {
    Exact,
    Similar,
    Different,
}

fn pair(spoken: &Token, verse: &Token) -> Pair {
    if spoken.stem == verse.stem {
        Pair::Exact
    } else if spoken.content && fuzzy::alike(&spoken.word, &spoken.sound, &verse.word, &verse.sound) {
        // Only meaningful spoken words get the benefit of the doubt.
        Pair::Similar
    } else {
        Pair::Different
    }
}

fn fts_query(words: &[String], tokens: &[Token]) -> Option<String> {
    let mut seen = HashSet::new();
    let triples = (0..(words.len() + 1).saturating_sub(WINDOW))
        .filter(|&i| tokens[i..i + WINDOW].iter().any(|t| t.content))
        .map(|i| &words[i..i + WINDOW]);
    // Adjacent content-word pairs still find "pray without [ceasing]" when the third word is misheard.
    let pairs = (0..words.len().saturating_sub(1))
        .filter(|&i| tokens[i].content && tokens[i + 1].content)
        .map(|i| &words[i..i + 2]);
    let phrases: Vec<String> = triples
        .chain(pairs)
        .map(|w| format!("\"{}\"", w.join(" ")))
        .filter(|p| seen.insert(p.clone()))
        .collect();
    (!phrases.is_empty()).then(|| phrases.join(" OR "))
}

#[derive(Default)]
struct Alignment {
    /// Transcript words matched exactly or as sound-alikes.
    mask: u64,
    exact: u32,
    similar: u32,
    /// Wrong, missing or extra words inside the aligned region.
    errors: u32,
    content: u32,
    content_weight: f32,
    verse_len: usize,
    /// First and last matched word, as (transcript index, verse index).
    first: Option<(usize, usize)>,
    last: Option<(usize, usize)>,
}

impl Alignment {
    fn matched(&self) -> u32 {
        self.exact + self.similar
    }

    /// Long, accurate and meaningful enough to stand as a quote.
    fn strong(&self) -> bool {
        let matched = self.matched();
        self.exact >= MIN_EXACT
            && self.errors * 3 <= matched
            && (self.content >= 3 || (self.content >= 2 && matched >= MIN_MATCHED_TWO_CONTENT))
    }
}

#[derive(Clone, Copy)]
enum Step {
    Stop,
    Diag(Pair),
    SkipSpoken,
    SkipVerse,
}

/// Best-scoring local alignment of the transcript against one verse (Smith-Waterman).
fn align(spoken: &[Token], verse: &[Token]) -> Alignment {
    let (n, m) = (spoken.len(), verse.len());
    let w = m + 1;
    let mut h = vec![0f32; (n + 1) * w];
    let mut from = vec![Step::Stop; (n + 1) * w];
    let mut best = (0f32, 0, 0);
    for i in 1..=n {
        for j in 1..=m {
            let (s, v) = (&spoken[i - 1], &verse[j - 1]);
            let p = pair(s, v);
            let gain = match p {
                Pair::Exact if s.content => EXACT_CONTENT,
                Pair::Exact => EXACT_STOPWORD,
                Pair::Similar if v.content => SIMILAR_CONTENT,
                Pair::Similar => SIMILAR_STOPWORD,
                Pair::Different => MISMATCH,
            };
            let (val, step) = [
                (h[(i - 1) * w + j - 1] + gain, Step::Diag(p)),
                (h[(i - 1) * w + j] + GAP, Step::SkipSpoken),
                (h[i * w + j - 1] + GAP, Step::SkipVerse),
            ]
            .into_iter()
            .fold((0f32, Step::Stop), |acc, o| if o.0 > acc.0 { o } else { acc });
            h[i * w + j] = val;
            from[i * w + j] = step;
            if val > best.0 {
                best = (val, i, j);
            }
        }
    }

    let mut a = Alignment { verse_len: m, ..Default::default() };
    let (mut i, mut j) = (best.1, best.2);
    loop {
        match from[i * w + j] {
            Step::Stop => break,
            Step::Diag(p) => {
                let (s, v) = (&spoken[i - 1], &verse[j - 1]);
                if p != Pair::Different {
                    // Traceback runs backwards: the first match seen is the last in reading order.
                    a.last.get_or_insert((i - 1, j - 1));
                    a.first = Some((i - 1, j - 1));
                }
                match p {
                    Pair::Exact => {
                        a.exact += 1;
                        a.mask |= 1 << (i - 1);
                        if s.content {
                            a.content += 1;
                            a.content_weight += 1.0;
                        }
                    }
                    Pair::Similar => {
                        a.similar += 1;
                        a.mask |= 1 << (i - 1);
                        if v.content {
                            a.content += 1;
                            a.content_weight += SIMILAR_WEIGHT;
                        }
                    }
                    Pair::Different => a.errors += 1,
                }
                i -= 1;
                j -= 1;
            }
            Step::SkipSpoken => {
                a.errors += 1;
                i -= 1;
            }
            Step::SkipVerse => {
                a.errors += 1;
                j -= 1;
            }
        }
    }
    a
}

/// 0.55 for a minimal two-content-word phrase, rising with phrase length and verse coverage to
/// 1.0 for a word-perfect full recitation (which alone clears the pipeline's auto-project bar).
/// Misheard and sound-alike words pull it down.
fn score(a: &Alignment) -> f32 {
    let matched = a.matched() as f32;
    let length = ((a.content_weight - 1.0) / 4.0).clamp(0.0, 1.0);
    let coverage = (matched / a.verse_len.max(1) as f32).min(1.0);
    let accuracy = (a.exact as f32 + SIMILAR_WEIGHT * a.similar as f32) / (matched + a.errors as f32);
    (0.55 + 0.25 * length + 0.2 * coverage) * (0.7 + 0.3 * accuracy)
}

/// Up to `MAX_HITS` verses the transcript appears to quote, best first.
pub fn find_quotes(db: &BibleDb, transcript: &str) -> Result<Vec<QuoteHit>> {
    let stemmer = Stemmer::create(Algorithm::English);
    let all = split(transcript);
    let words = &all[all.len().saturating_sub(TAIL_WORDS)..];
    let spoken: Vec<Token> = words.iter().map(|w| token(&stemmer, w)).collect();
    let Some(query) = fts_query(words, &spoken) else {
        return Ok(Vec::new());
    };

    // Best alignment per verse across all translations; `order` keeps BM25 rank as tie-break.
    let mut best: HashMap<String, (usize, ScriptureRef, Alignment, f32)> = HashMap::new();
    for (order, c) in db.search(None, &query, CANDIDATES)?.into_iter().enumerate() {
        let verse: Vec<Token> = split(&c.text).iter().map(|w| token(&stemmer, w)).collect();
        let a = align(&spoken, &verse);
        if !a.strong() {
            continue;
        }
        let s = score(&a);
        let key = c.reference.key();
        if best.get(&key).map_or(true, |b| s > b.3) {
            let order = best.get(&key).map_or(order, |b| b.0);
            best.insert(key, (order, c.reference, a, s));
        }
    }
    let mut ranked: Vec<_> = best.into_values().collect();
    ranked.sort_by(|x, y| y.3.total_cmp(&x.3).then(x.0.cmp(&y.0)));
    let Some(top) = ranked.first() else {
        return Ok(Vec::new());
    };

    let top_words = top.2.mask.count_ones() as f32;
    let rivals = ranked
        .iter()
        .filter(|r| (r.2.mask & top.2.mask).count_ones() as f32 >= RIVAL_OVERLAP * top_words)
        .count();
    if top.2.content <= GENERIC_CONTENT && rivals > MAX_GENERIC_VERSES {
        return Ok(Vec::new());
    }
    let penalty = if rivals > 1 { AMBIGUITY_PENALTY } else { 1.0 };
    let floor = top.3 * ALTERNATIVE_RATIO;

    Ok(ranked
        .into_iter()
        .take_while(|r| r.3 >= floor && r.3 * penalty >= MIN_SCORE)
        .take(MAX_HITS)
        .map(|(_, reference, a, s)| {
            let first = a.mask.trailing_zeros() as usize;
            let last = 64 - a.mask.leading_zeros() as usize;
            QuoteHit { reference, score: s * penalty, phrase: words[first..last].join(" ") }
        })
        .collect())
}

/// Text someone is reading or singing through (the live passage or song section), tokenised
/// once with each word's position so progress can be highlighted on the output.
pub struct Passage {
    tokens: Vec<Token>,
    /// (line index, UTF-16 offset just past the word): JavaScript string offsets.
    ends: Vec<(usize, usize)>,
}

impl Passage {
    pub fn new<S: AsRef<str>>(lines: &[S]) -> Self {
        let stemmer = Stemmer::create(Algorithm::English);
        let (mut tokens, mut ends) = (Vec::new(), Vec::new());
        for (li, line) in lines.iter().enumerate() {
            let line = line.as_ref();
            let mut start = None;
            // Same word boundaries as `split`, keeping byte positions.
            for (bi, c) in line.char_indices().chain(std::iter::once((line.len(), ' '))) {
                if c.is_alphanumeric() {
                    start.get_or_insert(bi);
                } else if let Some(s) = start.take() {
                    tokens.push(token(&stemmer, &line[s..bi].to_lowercase()));
                    ends.push((li, line[..bi].encode_utf16().count()));
                }
            }
        }
        Self { tokens, ends }
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Where word `i` ends: (line, UTF-16 offset).
    pub fn position(&self, i: usize) -> Option<(usize, usize)> {
        self.ends.get(i).copied()
    }
}

/// The most recent words of a transcript, ready to align.
pub struct Spoken {
    tokens: Vec<Token>,
}

impl Spoken {
    /// The last `tail` (≤ 64) words of `text`.
    pub fn new(text: &str, tail: usize) -> Self {
        let stemmer = Stemmer::create(Algorithm::English);
        let all = split(text);
        let words = &all[all.len().saturating_sub(tail.min(64))..];
        Self { tokens: words.iter().map(|w| token(&stemmer, w)).collect() }
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Words `first..=last`, for showing what matched.
    pub fn phrase(&self, first: usize, last: usize) -> String {
        let end = (last + 1).min(self.tokens.len());
        self.tokens[first.min(end)..end].iter().map(|t| t.word.as_str()).collect::<Vec<_>>().join(" ")
    }
}

/// How the latest speech lines up with a passage.
#[derive(Debug, Clone, Copy, Default)]
pub struct Fit {
    pub exact: u32,
    pub similar: u32,
    pub errors: u32,
    pub content: u32,
    /// Matched span in the transcript tail (inclusive).
    pub spoken_first: usize,
    pub spoken_last: usize,
    /// Matched span in the passage (inclusive word indices).
    pub target_first: usize,
    pub target_last: usize,
}

impl Fit {
    pub fn matched(&self) -> u32 {
        self.exact + self.similar
    }
}

/// Best local alignment of `spoken` against `passage`; None if no word lines up.
pub fn fit(spoken: &Spoken, passage: &Passage) -> Option<Fit> {
    if spoken.tokens.is_empty() || passage.tokens.is_empty() {
        return None;
    }
    let a = align(&spoken.tokens, &passage.tokens);
    let (first, last) = (a.first?, a.last?);
    Some(Fit {
        exact: a.exact,
        similar: a.similar,
        errors: a.errors,
        content: a.content,
        spoken_first: first.0,
        spoken_last: last.0,
        target_first: first.1,
        target_last: last.1,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::Instant;

    fn stems(text: &str) -> Vec<String> {
        let s = Stemmer::create(Algorithm::English);
        split(text).iter().map(|w| token(&s, w).stem).collect()
    }

    #[test]
    fn archaic_and_modern_forms_share_a_stem() {
        assert_eq!(stems("believeth believes believe"), vec!["believ"; 3]);
        assert_eq!(stems("loveth loved loves"), vec!["love"; 3]);
        assert_eq!(stems("God's"), vec!["god", "s"]);
    }

    #[test]
    fn query_skips_stopword_only_windows() {
        let s = Stemmer::create(Algorithm::English);
        let words = split("and it was the lord is my shepherd");
        let tokens: Vec<Token> = words.iter().map(|w| token(&s, w)).collect();
        let q = fts_query(&words, &tokens).unwrap();
        assert!(!q.contains("\"and it was\""));
        assert!(q.contains("\"is my shepherd\""));
        assert_eq!(fts_query(&words[..2], &tokens[..2]), None);
    }

    #[test]
    fn alignment_tolerates_a_changed_word() {
        let s = Stemmer::create(Algorithm::English);
        let t = |x: &str| split(x).iter().map(|w| token(&s, w)).collect::<Vec<_>>();
        let a = align(
            &t("he said whoever believes on him should not perish"),
            &t("that whosoever believeth in him should not perish, but have everlasting life"),
        );
        // whoever≈whosoever, believes=believeth, on≠in, him should not perish
        assert_eq!((a.exact, a.similar, a.errors), (5, 1, 1));
        assert!(a.strong());
    }

    #[test]
    fn sound_alikes_are_similar_and_unrelated_words_are_not() {
        let s = Stemmer::create(Algorithm::English);
        let p = |a: &str, b: &str| pair(&token(&s, a), &token(&s, b));
        for (heard, said) in [("seizing", "ceasing"), ("parish", "perish"), ("shepard", "shepherd"),
                              ("forgotten", "begotten"), ("whoever", "whosoever")] {
            assert_eq!(p(heard, said), Pair::Similar, "{heard} / {said}");
        }
        for (heard, said) in [("separate", "shepherd"), ("crisis", "christ"), ("table", "temple"), ("good", "god")] {
            assert_eq!(p(heard, said), Pair::Different, "{heard} / {said}");
        }
    }

    #[test]
    fn fit_reports_where_the_reader_is() {
        let passage = Passage::new(&["The LORD is my shepherd; I shall not want.", "He maketh me to lie down in green pastures:"]);
        assert_eq!(passage.len(), 9 + 9);
        let f = fit(&Spoken::new("and David said the Lord is my shepherd I shall", 16), &passage).unwrap();
        assert_eq!((f.target_first, f.target_last), (0, 6)); // "the" … "shall"
        assert_eq!(passage.position(f.target_last), Some((0, 32))); // "The LORD is my shepherd; I shall"
        let f = fit(&Spoken::new("not want he maketh me to lie down", 16), &passage).unwrap();
        assert_eq!(passage.position(f.target_last), Some((1, 24))); // "He maketh me to lie down"
        assert!(fit(&Spoken::new("", 16), &passage).is_none());
    }

    /// Runs against the full bundled database (`npm run bible:fetch && npm run bible:build`);
    /// skipped when only the sample or no database is present.
    #[test]
    fn finds_spoken_phrases_in_full_bible() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/bible.db");
        let Ok(db) = BibleDb::open(&path) else { return };
        if db.translations().len() < 5 {
            eprintln!("skipping: full bible.db not built");
            return;
        }
        let keys = |text: &str| {
            let t = Instant::now();
            let hits = find_quotes(&db, text).unwrap();
            eprintln!("{:>5.1} ms  {text:?} → {:?}", t.elapsed().as_secs_f64() * 1e3,
                hits.iter().map(|h| (h.reference.key(), (h.score * 100.0).round(), h.phrase.as_str())).collect::<Vec<_>>());
            hits.into_iter().map(|h| h.reference.key()).collect::<Vec<_>>()
        };
        let top = |text: &str| keys(text).into_iter().next();
        let some = |k: &str| Some(k.to_string());

        assert_eq!(top("as David wrote the Lord is my shepherd I shall not want"), some("19.23.1"));
        assert_eq!(top("you know for God so loved the world"), some("43.3.16"));
        // John 3:15 and 3:16 both contain this (WEB), so both are offered.
        assert!(keys("whoever believes in him should not perish").contains(&"43.3.16".to_string()));
        assert_eq!(top("I can do all things through Christ"), some("50.4.13"));
        assert_eq!(top("Jesus said I am the way the truth and the life"), some("43.14.6"));
        assert_eq!(top("we know all things work together for good"), some("45.8.28"));
        assert_eq!(top("be still and know that I am God"), some("19.46.10"));
        assert_eq!(top("the Bible says pray without ceasing"), some("52.5.17"));

        // One misheard or unrelated word inside the quote.
        assert_eq!(top("the Lord is my separate I shall not want"), some("19.23.1"));
        assert_eq!(top("the Bible says pray without seizing"), some("52.5.17"));
        assert_eq!(top("he gave his only forgotten son"), some("43.3.16"));
        assert_eq!(top("I can do all things through crisis which strengtheneth me"), some("50.4.13"));
        assert_eq!(top("Jesus said I am the way the truth and the light"), some("43.14.6"));
        assert_eq!(top("all things work together for food to them that love God"), some("45.8.28"));
        // Also in John 3:15 (KJV), so both are offered.
        assert!(keys("that whosoever believeth in him should not parish").contains(&"43.3.16".to_string()));

        // Generic phrases and ordinary sermon talk stay quiet.
        assert_eq!(top("and it came to pass"), None);
        assert_eq!(top("we preach the word of God"), None);
        assert_eq!(top("let us pray in the name of Jesus"), None);
        for talk in [
            "good morning church it is good to see all of you here today",
            "please turn to your neighbour and say you are welcome",
            "if you have your bibles open them with me as we read",
            "last week we talked about how to live a life that pleases God",
            "I want to tell you a story about a man who lost his job",
            "let us bow our heads and close our eyes",
            "the youth group will meet on Thursday at seven in the evening",
            "some of you have been going through a hard time this week",
            "give the Lord a hand clap of praise",
            "I believe God is going to do something great in this house",
            "we thank God for everyone who came out this morning",
            "the offering baskets will now come around",
            "Jesus loves you more than you can imagine",
            "the church is not a building it is the people",
            "if you are visiting with us today please fill out a card",
            "there was a man in our church who prayed for his son",
            "the Lord has been so good to this family",
            "I want everybody to lift up your hands and praise him",
            "our God is a good God and he is faithful",
            "the pastor will be preaching on forgiveness next Sunday",
        ] {
            assert_eq!(top(talk), None, "{talk}");
        }

        // A full recitation is confident enough to auto-project.
        let full = find_quotes(&db, "For God so loved the world, that he gave his only begotten Son, that whosoever believeth in him should not perish, but have everlasting life.").unwrap();
        assert!(full[0].score >= 0.95, "{}", full[0].score);
    }
}
