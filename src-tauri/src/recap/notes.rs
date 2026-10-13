//! Sermon notes from the service transcript, made on this computer with no AI service: the
//! sentences that carry the sermon's main ideas (ranked TextRank-style by how much they share
//! with the rest of the sermon), grouped under the scriptures that were on screen at the time,
//! plus the sermon's recurring themes. Small talk ("you may be seated") and verses read aloud
//! (already in the recap's scripture list) are left out.

use std::collections::{HashMap, HashSet};

use rust_stemmers::{Algorithm, Stemmer};
use serde::Serialize;

use crate::{
    history::{time_of, Entry},
    scripture::{books, ScriptureRef},
};

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SermonNotes {
    /// Recurring ideas, most frequent first ("faith", "forgiveness").
    pub themes: Vec<String>,
    pub sections: Vec<NoteSection>,
}

impl SermonNotes {
    pub fn is_empty(&self) -> bool {
        self.sections.iter().all(|s| s.points.is_empty())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSection {
    /// The passage on screen ("John 3:16–18"); `None` for what was said before the first one.
    pub heading: Option<String>,
    pub time: String,
    pub points: Vec<String>,
}

/// Words that say nothing about a sermon's content: grammar, fillers and pulpit small talk.
const STOPWORDS: &[&str] = &[
    "the", "and", "of", "to", "a", "in", "that", "is", "for", "it", "as", "was", "with", "be", "his", "he", "i",
    "you", "on", "by", "not", "but", "are", "this", "they", "have", "from", "or", "at", "all", "we", "my", "me",
    "your", "so", "will", "unto", "thee", "thou", "thy", "shall", "him", "them", "which", "there", "what", "when",
    "who", "were", "their", "an", "if", "our", "us", "do", "no", "hath", "ye", "then", "said", "into", "up", "out",
    "now", "also", "been", "had", "has", "her", "she", "its", "these", "those", "would", "could", "let", "like",
    "just", "very", "okay", "ok", "amen", "can", "say", "says", "saying", "know", "going", "get", "got", "one",
    "about", "because", "how", "why", "where", "here", "some", "any", "more", "most", "much", "many", "than",
    "too", "only", "own", "same", "other", "such", "even", "ever", "still", "yet", "again", "every", "each",
    "did", "does", "doing", "done", "am", "being", "can't", "don't", "won't", "it's", "that's", "i'm", "you're",
    "we're", "they're", "gonna", "wanna", "yeah", "yes", "oh", "uh", "um", "hmm", "well", "right", "really",
    "actually", "basically", "never", "always", "name", "start", "among", "beginning", "thing", "things", "something", "anything", "everything", "nothing", "somebody",
    "someone", "everybody", "everyone", "anybody", "come", "came", "go", "goes", "went", "gone", "see", "saw",
    "seen", "look", "make", "made", "take", "took", "tell", "told", "want", "wants", "way", "time", "times",
    "today", "day", "week", "lot", "kind", "maybe", "should", "must", "may", "might", "need", "needs", "put",
    "give", "gave", "keep", "think", "thought", "mean", "means", "two", "three", "first", "last", "next", "back",
    "over", "through", "after", "before", "down", "off", "while", "into", "upon", "yourself", "ourselves",
    "hallelujah", "praise", "church", "brother", "sister", "pastor", "please", "thank", "thanks", "morning",
    "evening", "neighbour", "neighbor", "turn", "verse", "chapter", "bible", "read", "reading", "open",
];

/// Common in every sermon, so never a theme on their own.
const NOT_THEMES: &[&str] = &["god", "lord", "jesus", "christ", "father", "people", "man", "men", "life", "word", "good", "great", "little", "big", "new", "old", "place", "hand", "heart"];

/// Phrases that announce a point worth writing down.
const CUES: &[&str] = &[
    "the point", "my point", "remember", "i want you to", "the truth is", "the lesson", "the key", "number one",
    "number two", "number three", "firstly", "secondly", "thirdly", "finally", "in conclusion", "the bible says",
    "god wants", "god is saying", "the word says", "you must", "we must", "never forget", "the secret",
    "the message", "this means", "what this means", "the reason", "understand that", "notice that",
];

/// Pulpit housekeeping, never a sermon point.
const SMALL_TALK: &[&str] = &[
    "good morning", "good evening", "be seated", "turn to your neighbo", "tell your neighbo", "can i get an amen",
    "somebody say", "say amen", "give the lord", "clap", "offering", "announcement", "let us pray", "let's pray",
    "bow your head", "close your eyes", "stand up", "on your feet", "welcome", "microphone", "testing",
];

/// Whisper's notes for non-speech ("*Singing*", "[MUSIC]", "(applause)") aren't words said.
fn speech(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut skip_until: Option<char> = None;
    for c in text.chars() {
        match (skip_until, c) {
            (Some(end), c) if c == end => skip_until = None,
            (Some(_), _) => {}
            (None, '*') => skip_until = Some('*'),
            (None, '[') => skip_until = Some(']'),
            (None, '(') => skip_until = Some(')'),
            (None, c) => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "No, no, no, no, no": a stuck or sung line, not a point.
fn repetitive(text: &str) -> bool {
    let all: Vec<String> = words(text).collect();
    let unique: HashSet<&String> = all.iter().collect();
    all.len() >= 6 && unique.len() * 2 < all.len()
}

/// Bible book names say where a point came from, not what it's about.
fn is_book_name(word: &str) -> bool {
    books::BOOKS.iter().any(|b| b.aliases.contains(&word) || b.name.eq_ignore_ascii_case(word))
}

/// Average TextRank score below which the transcript is treated as noise (see `notes_from`).
const MIN_COHERENCE: f32 = 0.6;
/// …or when fewer than this share of sentences could be a sermon point.
const MIN_USABLE_PERCENT: usize = 8;

struct Sentence {
    at: i64,
    text: String,
    words: usize,
    stems: HashSet<String>,
}

/// Lowercase words, apostrophes kept inside them ("don't"), everything else a separator.
fn words(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '’'))
        .map(|w| w.trim_matches(|c| c == '\'' || c == '’').replace('’', "'").to_lowercase())
        .filter(|w| !w.is_empty())
}

fn content_stems(stemmer: &Stemmer, text: &str) -> Vec<(String, String)> {
    words(text)
        .filter(|w| w.len() > 2 && !STOPWORDS.contains(&w.as_str()) && !w.chars().all(|c| c.is_ascii_digit()))
        .map(|w| (stemmer.stem(&w).into_owned(), w))
        .collect()
}

/// Transcript lines into sentences. Lines often end mid-sentence, so text is carried over until
/// a full stop, a long pause or a runaway length ends it.
fn sentences(stemmer: &Stemmer, said: &[(i64, &str)]) -> Vec<Sentence> {
    const PAUSE_MS: i64 = 8_000;
    const MAX_WORDS: usize = 60;
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut start = 0;
    let mut last_at = 0;
    let flush = |buf: &mut String, start: i64, out: &mut Vec<Sentence>| {
        let text = buf.trim().to_owned();
        buf.clear();
        if text.is_empty() {
            return;
        }
        let stems = content_stems(stemmer, &text).into_iter().map(|(s, _)| s).collect();
        out.push(Sentence { at: start, words: text.split_whitespace().count(), text, stems });
    };
    for &(at, line) in said {
        if !buf.is_empty() && at - last_at > PAUSE_MS {
            flush(&mut buf, start, &mut out);
        }
        last_at = at;
        let mut rest = line.trim();
        while !rest.is_empty() {
            if buf.is_empty() {
                start = at;
            }
            // Up to and including the next sentence end (and any closing quote after it).
            let end = rest.char_indices().find(|&(_, c)| matches!(c, '.' | '?' | '!')).map(|(i, c)| {
                let after = &rest[i + c.len_utf8()..];
                i + c.len_utf8() + after.len() - after.trim_start_matches(['"', '\'', '’', '”', ')']).len()
            });
            let (piece, ended) = match end {
                Some(e) => (&rest[..e], true),
                None => (rest, false),
            };
            if !buf.is_empty() {
                buf.push(' ');
            }
            buf.push_str(piece.trim());
            rest = rest[piece.len()..].trim_start();
            if ended || buf.split_whitespace().count() >= MAX_WORDS {
                flush(&mut buf, start, &mut out);
            }
        }
    }
    flush(&mut buf, start, &mut out);
    out
}

/// TextRank: a sentence matters when it shares words with other sentences that matter.
fn rank(sentences: &[Sentence]) -> Vec<f32> {
    let n = sentences.len();
    let mut edges: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (&sentences[i].stems, &sentences[j].stems);
            if a.len() < 2 || b.len() < 2 {
                continue;
            }
            let shared = a.intersection(b).count();
            if shared == 0 {
                continue;
            }
            let w = shared as f32 / ((a.len() as f32).ln() + (b.len() as f32).ln());
            edges[i].push((j, w));
            edges[j].push((i, w));
        }
    }
    let out_weight: Vec<f32> = edges.iter().map(|e| e.iter().map(|(_, w)| w).sum()).collect();
    let mut score = vec![1.0f32; n];
    for _ in 0..30 {
        let next: Vec<f32> = (0..n)
            .map(|i| 0.15 + 0.85 * edges[i].iter().map(|&(j, w)| if out_weight[j] > 0.0 { w / out_weight[j] * score[j] } else { 0.0 }).sum::<f32>())
            .collect();
        score = next;
    }
    score
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f32 {
    let union = a.union(b).count();
    if union == 0 {
        0.0
    } else {
        a.intersection(b).count() as f32 / union as f32
    }
}

/// A sentence as it should read in the notes: capitalised, ending in punctuation.
fn tidy(text: &str) -> String {
    let t = text.trim().trim_start_matches(|c: char| c == ',' || c == '-' || c == '…' || c.is_whitespace());
    let mut chars = t.chars();
    let mut s = match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect::<String>(),
        None => return String::new(),
    };
    if !s.ends_with(['.', '?', '!', '"', '”', '’', '\'']) {
        s.push('.');
    }
    s
}

struct Shown {
    at: i64,
    reference: ScriptureRef,
    stems: HashSet<String>,
}

/// "John 3:16", or "John 3:16–18" for a run of verses in one chapter.
fn heading(run: &[&Shown]) -> String {
    let first = &run[0].reference;
    let book = books::by_id(first.book_id).map_or("?", |b| b.name);
    let start = run.iter().filter_map(|s| s.reference.verse_start).min();
    let end = run.iter().filter_map(|s| s.reference.verse_end.or(s.reference.verse_start)).max();
    match (start, end) {
        (Some(s), Some(e)) if e > s => format!("{book} {}:{s}–{e}", first.chapter),
        (Some(s), _) => format!("{book} {}:{s}", first.chapter),
        _ => format!("{book} {}", first.chapter),
    }
}

pub fn notes_from(entries: &[Entry]) -> SermonNotes {
    let stemmer = Stemmer::create(Algorithm::English);
    // While a song is on screen the transcript is singing; it runs until the next slide (at
    // most ten minutes).
    let mut singing: Vec<(i64, i64)> = Vec::new();
    for e in entries {
        if let Entry::Shown { at, song, .. } = e {
            if let Some(last) = singing.last_mut().filter(|(_, end)| *end == i64::MAX) {
                last.1 = (*at).min(last.0 + 10 * 60_000);
            }
            if song.is_some() {
                singing.push((*at, i64::MAX));
            }
        }
    }
    if let Some(last) = singing.last_mut().filter(|(_, end)| *end == i64::MAX) {
        last.1 = last.0 + 10 * 60_000;
    }
    let cleaned: Vec<(i64, String)> = entries
        .iter()
        .filter_map(|e| match e {
            Entry::Said { at, text } if !singing.iter().any(|(a, b)| (*a..*b).contains(at)) => Some((*at, speech(text))),
            _ => None,
        })
        .filter(|(_, t)| !t.is_empty())
        .collect();
    let said: Vec<(i64, &str)> = cleaned.iter().map(|(at, t)| (*at, t.as_str())).collect();
    let shown: Vec<Shown> = entries
        .iter()
        .filter_map(|e| match e {
            Entry::Shown { at, reference: Some(r), song: None, text, .. } => {
                Some(Shown { at: *at, reference: r.clone(), stems: content_stems(&stemmer, text).into_iter().map(|(s, _)| s).collect() })
            }
            _ => None,
        })
        .collect();

    let sentences = sentences(&stemmer, &said);
    if sentences.len() < 3 {
        return SermonNotes::default();
    }

    // Themes: the content words the preacher keeps coming back to.
    let mut counts: HashMap<String, (usize, HashMap<String, usize>)> = HashMap::new();
    for (_, line) in &said {
        for (stem, word) in content_stems(&stemmer, line) {
            if NOT_THEMES.contains(&word.as_str()) || word.len() < 4 || word.contains('\'') || is_book_name(&word) {
                continue;
            }
            let e = counts.entry(stem).or_default();
            e.0 += 1;
            *e.1.entry(word).or_default() += 1;
        }
    }
    let mut themes: Vec<(usize, String)> = counts
        .into_values()
        .filter(|(n, _)| *n >= 3)
        .map(|(n, forms)| (n, forms.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.len().cmp(&a.0.len()))).map(|f| f.0).unwrap_or_default()))
        .collect();
    themes.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let themes: Vec<String> = themes.into_iter().take(6).map(|(_, w)| w).collect();
    let theme_stems: HashSet<String> = themes.iter().map(|w| stemmer.stem(w).into_owned()).collect();

    // Candidates: sermon-length sentences that aren't small talk or a verse being read out.
    let scores = rank(&sentences);
    let reading = |s: &Sentence| {
        shown.iter().any(|v| {
            (s.at - v.at).abs() < 3 * 60_000 && !s.stems.is_empty() && s.stems.iter().filter(|w| v.stems.contains(*w)).count() * 10 >= s.stems.len() * 6
        })
    };
    let mut candidates: Vec<(f32, usize)> = sentences
        .iter()
        .enumerate()
        .filter(|(_, s)| {
            let lower = s.text.to_lowercase();
            (9..=45).contains(&s.words)
                && s.stems.len() >= 4
                && (theme_stems.is_empty() || s.stems.iter().any(|w| theme_stems.contains(w)))
                && !repetitive(&s.text)
                && !SMALL_TALK.iter().any(|p| lower.contains(p))
                && !reading(s)
        })
        .map(|(i, s)| {
            let lower = s.text.to_lowercase();
            let cue = if CUES.iter().any(|c| lower.contains(c)) { 1.35 } else { 1.0 };
            let themed = 1.0 + 0.08 * s.stems.intersection(&theme_stems).count().min(3) as f32;
            (scores[i] * cue * themed, i)
        })
        .collect();
    candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
    // A sermon hangs together: most sentences share ideas with others, and many are usable.
    // A transcript of music, crowd noise or a bad microphone doesn't (measured: sermons average
    // 0.8+, noise 0.3–0.5 with almost nothing usable). Better no notes than nonsense.
    let coherence = scores.iter().sum::<f32>() / scores.len() as f32;
    if coherence < MIN_COHERENCE || candidates.len() * 100 < sentences.len() * MIN_USABLE_PERCENT {
        log::info!("no sermon notes: transcript too noisy (coherence {coherence:.2}, {} of {} sentences usable)", candidates.len(), sentences.len());
        return SermonNotes::default();
    }

    // The best few, skipping near-repeats of a point already taken.
    let want = (sentences.len() / 8).clamp(3, 10);
    let mut picked: Vec<usize> = Vec::new();
    for (_, i) in candidates {
        if picked.len() >= want {
            break;
        }
        if picked.iter().all(|&p| jaccard(&sentences[p].stems, &sentences[i].stems) < 0.5) {
            picked.push(i);
        }
    }
    picked.sort_unstable();

    // Sections: one per run of passages from the same chapter, in the order they were shown.
    let mut runs: Vec<Vec<&Shown>> = Vec::new();
    for s in &shown {
        match runs.last_mut() {
            Some(run) if run[0].reference.book_id == s.reference.book_id && run[0].reference.chapter == s.reference.chapter => run.push(s),
            _ => runs.push(vec![s]),
        }
    }
    // (start time, section); an opening section holds points made before the first passage.
    let mut sections: Vec<(i64, NoteSection)> = Vec::new();
    if picked.first().is_some_and(|&i| runs.first().is_none_or(|r| sentences[i].at < r[0].at)) {
        sections.push((i64::MIN, NoteSection { heading: None, time: time_of(sentences[0].at), points: Vec::new() }));
    }
    for run in &runs {
        sections.push((run[0].at, NoteSection { heading: Some(heading(run)), time: time_of(run[0].at), points: Vec::new() }));
    }
    for &i in &picked {
        let at = sentences[i].at;
        let idx = sections.iter().rposition(|(start, _)| *start <= at).unwrap_or(0);
        sections[idx].1.points.push(tidy(&sentences[i].text));
    }
    SermonNotes { themes, sections: sections.into_iter().map(|(_, s)| s).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(at_s: i64, text: &str) -> Entry {
        Entry::Said { at: at_s * 1000, text: text.into() }
    }

    #[test]
    fn sentences_join_lines_and_split_on_stops() {
        let stemmer = Stemmer::create(Algorithm::English);
        let s = sentences(&stemmer, &[(0, "The grace of God is"), (2000, "enough for you. Do you believe"), (4000, "it?")]);
        let texts: Vec<_> = s.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, ["The grace of God is enough for you.", "Do you believe it?"]);
        assert_eq!(s[1].at, 2000);
    }

    #[test]
    fn picks_points_and_groups_them_under_passages() {
        let john = ScriptureRef { book_id: 43, chapter: 3, verse_start: Some(16), verse_end: None };
        let mut entries = vec![
            said(0, "Good morning church, you may be seated."),
            said(5, "Today we are talking about the love of God and how his love changes everything about your life."),
            Entry::Shown { at: 20_000, label: "John 3:16".into(), translation: "KJV".into(), text: "For God so loved the world, that he gave his only begotten Son".into(), reference: Some(john), song: None },
            said(22, "For God so loved the world that he gave his only begotten son."),
            said(30, "Remember that the love of God is not earned, his love is a gift of grace you receive by faith."),
            said(40, "Grace means God gives his love freely, and faith is how you receive that grace every single day."),
            said(50, "So the point is this: receive his grace by faith and let his love change the way you love others."),
            said(60, "Turn to your neighbour and say neighbour, God loves you."),
            said(70, "Faith grows when you hear the word, and grace grows when you walk in his love with other believers."),
        ];
        entries.push(Entry::Shown {
            at: 80_000,
            label: "John 3:17".into(),
            translation: "KJV".into(),
            text: "For God sent not his Son into the world to condemn the world".into(),
            reference: Some(ScriptureRef { book_id: 43, chapter: 3, verse_start: Some(17), verse_end: None }),
            song: None,
        });
        let notes = notes_from(&entries);
        assert!(notes.themes.contains(&"love".to_owned()) && notes.themes.contains(&"grace".to_owned()), "{:?}", notes.themes);
        let all: Vec<&str> = notes.sections.iter().flat_map(|s| s.points.iter().map(String::as_str)).collect();
        assert!(all.iter().any(|p| p.starts_with("So the point is this")), "{all:?}");
        assert!(!all.iter().any(|p| p.contains("neighbour") || p.contains("Good morning")), "small talk: {all:?}");
        assert!(!all.iter().any(|p| p.contains("begotten")), "verse read aloud: {all:?}");
        let john = notes.sections.iter().find(|s| s.heading.as_deref() == Some("John 3:16–17")).expect("merged heading");
        assert!(!john.points.is_empty());
    }

    #[test]
    fn non_speech_and_repetition_are_ignored() {
        assert_eq!(speech("*Singing* Praise him [MUSIC] (applause) now"), "Praise him now");
        assert!(repetitive("There is no, no, no, no, no, no, no, no."));
        assert!(!repetitive("Grace means God gives his love freely to everyone."));
        assert!(is_book_name("corinthians") && !is_book_name("grace"));
    }

    #[test]
    fn noise_makes_no_notes() {
        let noise = ["There is not a friend like the Anglo-Langism, no, no, no.", "Why this method?", "The news was for now, but Rutu had it.",
            "And I was warned, the gavitt of my mind, I said, Mom, if we are going together.", "That beautiful place have never been before, this week the dog will text you there.",
            "They're rockin' news to now, man!", "She has never been told better before.", "Just never be the other one, ye are too big."];
        let entries: Vec<Entry> = noise.iter().enumerate().map(|(i, t)| said(i as i64 * 5, t)).collect();
        assert!(notes_from(&entries).is_empty());
    }

    #[test]
    fn too_little_speech_makes_no_notes() {
        assert!(notes_from(&[said(0, "Hello.")]).is_empty());
    }

    /// Prints the notes for a real service log: `NOTES_FILE=…/2026-10-11.jsonl cargo test notes_for_file -- --ignored --nocapture`
    #[test]
    #[ignore = "reads a service log named by NOTES_FILE"]
    fn notes_for_file() {
        let path = std::env::var("NOTES_FILE").expect("NOTES_FILE");
        let entries: Vec<Entry> = std::fs::read_to_string(path).unwrap().lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
        let notes = notes_from(&entries);
        println!("themes: {:?}", notes.themes);
        for s in &notes.sections {
            println!("\n## {} ({})", s.heading.as_deref().unwrap_or("Opening"), s.time);
            for p in &s.points {
                println!("  • {p}");
            }
        }
    }

    /// A sermon transcript fixture: one line every 12 s; "#SHOW book chapter from to text" puts a
    /// passage on screen.
    pub(crate) fn fixture_entries() -> Vec<Entry> {
        let text = include_str!("../../tests/fixtures/sermon-love-never-fails.txt");
        text.lines()
            .enumerate()
            .map(|(i, line)| {
                let at = i as i64 * 12_000;
                match line.strip_prefix("#SHOW ") {
                    Some(rest) => {
                        let mut p = rest.splitn(5, ' ');
                        let n = |p: &mut std::str::SplitN<'_, char>| p.next().unwrap().parse::<u16>().unwrap();
                        let (book, chapter, from, to) = (n(&mut p) as u8, n(&mut p), n(&mut p), n(&mut p));
                        let reference = ScriptureRef { book_id: book, chapter, verse_start: Some(from), verse_end: (to > from).then_some(to) };
                        Entry::Shown { at, label: reference.label(), translation: "KJV".into(), text: p.next().unwrap().into(), reference: Some(reference), song: None }
                    }
                    None => Entry::Said { at, text: line.into() },
                }
            })
            .collect()
    }

    #[test]
    fn a_real_sermon_gives_its_main_points() {
        let notes = notes_from(&fixture_entries());
        let all: Vec<&str> = notes.sections.iter().flat_map(|s| s.points.iter().map(String::as_str)).collect();
        println!("{:?}\n{all:#?}", notes.themes);
        assert!(notes.themes.iter().any(|t| t == "love"), "{:?}", notes.themes);
        assert!(all.len() >= 3, "{all:?}");
        for junk in ["Good morning", "neighbour", "Somebody say", "Let us pray", "while we were still sinners"] {
            assert!(!all.iter().any(|p| p.contains(junk)), "{junk}: {all:?}");
        }
        let headings: Vec<_> = notes.sections.iter().map(|s| s.heading.as_deref().unwrap_or("Opening")).collect();
        // Passages from one chapter shown one after another make one heading.
        assert_eq!(headings, ["1 Corinthians 13:4–8", "Romans 5:8"]);
        assert!(all.len() >= 4, "{all:?}");
    }
}
