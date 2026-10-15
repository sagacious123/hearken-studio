//! Follow-along: while a passage or song is live, keep the output in step with the reader or the
//! singers. Highlights how far they have got, and moves to the next verse (or song section) as
//! soon as they start it. Also spots which imported song is being sung when none is live.

use std::time::{Duration, Instant};

use crate::{
    presentation::OutputState,
    scripture::{
        build_slide,
        quote::{self, Fit, Passage, Spoken},
        BibleDb, ScriptureRef, Slide, Versification,
    },
    songs::{self, Song},
};

/// Words of the transcript considered: enough for a phrase, short enough to be "now".
const TAIL_WORDS: usize = 16;
/// After moving on, wait this long so the tail of one sentence can't skip two verses.
const ADVANCE_COOLDOWN: Duration = Duration::from_millis(1500);
/// The next verse/section must be picked up from (near) its first word…
const MAX_START_WORD: usize = 3;
/// …with this many words matching exactly.
const MIN_EXACT_TO_ADVANCE: u32 = 3;
/// Song recognition from singing when no song is live.
const SONG_TAIL_WORDS: usize = 20;
const SONG_MIN_EXACT: u32 = 4;
const SONG_MIN_MATCHED: u32 = 5;

pub enum Action {
    None,
    /// The live slide has been read/sung up to here (line, UTF-16 offset).
    Progress { slide_id: String, line: usize, offset: usize },
    /// They have started the next verse or section.
    Advance(Slide),
}

pub struct Context<'a> {
    pub bible: &'a BibleDb,
    pub v11n: &'a Versification,
    pub songs: &'a [Song],
    pub follow_scripture: bool,
    pub follow_songs: bool,
}

struct Target {
    slide: Slide,
    passage: Passage,
    /// The natural next step (verse + 1, next section); wins ties with a repeated chorus.
    preferred: bool,
}

#[derive(Default)]
pub struct Follower {
    slide_id: Option<String>,
    current: Option<Passage>,
    next: Vec<Target>,
    progress: Option<usize>,
    last_advance: Option<Instant>,
}

fn passage(slide: &Slide) -> Passage {
    Passage::new(&slide.verses.iter().map(|v| v.text.as_str()).collect::<Vec<_>>())
}

impl Follower {
    fn load(&mut self, slide: &Slide, ctx: &Context) {
        self.slide_id = Some(slide.id.clone());
        self.current = Some(passage(slide));
        self.progress = None;
        self.next = match &slide.song {
            Some(part) => ctx
                .songs
                .iter()
                .find(|s| s.id == part.song_id)
                .map(|song| {
                    (0..song.sections.len())
                        .filter(|&i| i != part.section)
                        .filter_map(|i| songs::slide(song, i))
                        .map(|s| {
                            let preferred = s.song.as_ref().is_some_and(|p| p.section == part.section + 1);
                            Target { passage: passage(&s), slide: s, preferred }
                        })
                        .collect()
                })
                .unwrap_or_default(),
            None => next_verse(slide, ctx)
                .map(|s| vec![Target { passage: passage(&s), slide: s, preferred: true }])
                .unwrap_or_default(),
        };
    }

    pub fn update(&mut self, text: &str, live: &OutputState, ctx: &Context) -> Action {
        let Some(slide) = live.slide.as_ref().filter(|_| !live.blackout) else {
            self.slide_id = None;
            return Action::None;
        };
        if !(if slide.is_song() { ctx.follow_songs } else { ctx.follow_scripture }) {
            return Action::None;
        }
        if self.slide_id.as_deref() != Some(slide.id.as_str()) {
            self.load(slide, ctx);
        }
        let spoken = Spoken::new(text, TAIL_WORDS);
        if spoken.len() < 3 {
            return Action::None;
        }
        let cur = self.current.as_ref().and_then(|p| quote::fit(&spoken, p));

        if self.last_advance.map_or(true, |t| t.elapsed() > ADVANCE_COOLDOWN) {
            let started = |f: &Fit| {
                f.exact >= MIN_EXACT_TO_ADVANCE
                    && f.content >= 1
                    && f.target_first <= MAX_START_WORD
                    && f.spoken_last + 2 >= spoken.len()
                    && f.errors * 3 <= f.matched()
                    // Speech has moved past the current verse, not just echoed a phrase of it.
                    && cur.map_or(true, |c| f.spoken_last > c.spoken_last)
            };
            let best = self
                .next
                .iter()
                .filter_map(|t| quote::fit(&spoken, &t.passage).filter(started).map(|f| (f.matched() * 2 + u32::from(t.preferred), t)))
                .max_by_key(|(rank, _)| *rank);
            if let Some((_, t)) = best {
                self.last_advance = Some(Instant::now());
                return Action::Advance(t.slide.clone());
            }
        }

        // Highlight only forward, and only from words just spoken.
        if let Some(f) = cur.filter(|f| f.exact >= 2 && f.spoken_last + 4 >= spoken.len()) {
            if self.progress.map_or(true, |p| f.target_last > p) {
                if let Some((line, offset)) = self.current.as_ref().and_then(|p| p.position(f.target_last)) {
                    self.progress = Some(f.target_last);
                    return Action::Progress { slide_id: slide.id.clone(), line, offset };
                }
            }
        }
        Action::None
    }
}

/// The verse after the slide's last one, continuing into the next chapter.
fn next_verse(slide: &Slide, ctx: &Context) -> Option<Slide> {
    let r = &slide.reference;
    let last = r.verse_end.or(r.verse_start)?;
    let max = ctx.v11n.max_verse(r.book_id, r.chapter)?;
    let next = if last < max {
        ScriptureRef { verse_start: Some(last + 1), verse_end: None, ..r.clone() }
    } else {
        ctx.v11n.max_verse(r.book_id, r.chapter + 1)?;
        ScriptureRef { chapter: r.chapter + 1, verse_start: Some(1), verse_end: None, ..r.clone() }
    };
    let secondary = slide.secondary.as_ref().map(|s| s.translation.as_str());
    build_slide(ctx.bible, &slide.translation, secondary, &next).ok().flatten()
}

/// Every section of every imported song, tokenised once per library change.
#[derive(Default)]
pub struct LyricsIndex {
    version: u64,
    songs: Vec<Song>,
    sections: Vec<(usize, usize, Passage)>,
}

pub struct SongMatch {
    pub slide: Slide,
    pub score: f32,
    pub phrase: String,
}

impl LyricsIndex {
    pub fn version(&self) -> u64 {
        self.version
    }

    pub fn refresh(&mut self, version: u64, songs: Vec<Song>) {
        self.sections = songs
            .iter()
            .enumerate()
            .flat_map(|(si, song)| song.sections.iter().enumerate().map(move |(xi, sec)| (si, xi, Passage::new(&sec.lines))))
            .collect();
        self.songs = songs;
        self.version = version;
    }

    pub fn songs(&self) -> &[Song] {
        &self.songs
    }

    /// The song section being sung, if the latest words match one well.
    pub fn detect(&self, text: &str) -> Option<SongMatch> {
        let spoken = Spoken::new(text, SONG_TAIL_WORDS);
        let mut best: Option<(f32, usize, usize, Fit)> = None;
        for (si, xi, p) in &self.sections {
            let Some(f) = quote::fit(&spoken, p) else { continue };
            if f.exact < SONG_MIN_EXACT || f.matched() < SONG_MIN_MATCHED || f.content < 2 || f.errors * 3 > f.matched() {
                continue;
            }
            let accuracy = f.exact as f32 / (f.matched() + f.errors) as f32;
            let score = 0.5 + 0.3 * (f.matched() as f32 / 10.0).min(1.0) + 0.2 * accuracy;
            if best.as_ref().map_or(true, |b| score > b.0) {
                best = Some((score, *si, *xi, f));
            }
        }
        let (score, si, xi, f) = best?;
        Some(SongMatch { slide: songs::slide(&self.songs[si], xi)?, score, phrase: spoken.phrase(f.spoken_first, f.spoken_last) })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::scripture::VerseLine;

    fn scripture_slide(verse: u16, text: &str) -> Slide {
        Slide {
            id: format!("KJV:19.23.{verse}"),
            label: format!("Psalm 23:{verse}"),
            translation: "KJV".into(),
            reference: ScriptureRef { book_id: 19, chapter: 23, verse_start: Some(verse), verse_end: None },
            verses: vec![VerseLine { verse, text: text.into() }],
            secondary: None,
            song: None,
        }
    }

    fn live(slide: Slide) -> OutputState {
        OutputState { slide: Some(slide), ..Default::default() }
    }

    #[test]
    fn highlights_then_moves_to_the_next_section() {
        let song = songs::parse(
            "Amazing Grace\n\nVerse 1\nAmazing grace how sweet the sound\nThat saved a wretch like me\n\nVerse 2\nTwas grace that taught my heart to fear\nAnd grace my fears relieved",
        )
        .unwrap();
        let bible = BibleDb::open_in_memory_for_tests();
        let v11n = Versification::from_map(HashMap::new());
        let songs = vec![song.clone()];
        let ctx = Context { bible: &bible, v11n: &v11n, songs: &songs, follow_scripture: true, follow_songs: true };
        let state = live(songs::slide(&song, 0).unwrap());
        let mut f = Follower::default();

        match f.update("amazing grace how sweet the sound", &state, &ctx) {
            Action::Progress { line, offset, .. } => assert_eq!((line, offset), (0, 33)), // "…the sound"
            _ => panic!("expected progress"),
        }
        assert!(matches!(f.update("amazing grace how sweet", &state, &ctx), Action::None)); // never backwards
        match f.update("that saved a wretch like me twas grace that taught my heart", &state, &ctx) {
            Action::Advance(s) => assert_eq!(s.song.unwrap().section, 1),
            _ => panic!("expected advance"),
        }
    }

    #[test]
    fn scripture_follow_respects_the_switch() {
        let bible = BibleDb::open_in_memory_for_tests();
        let v11n = Versification::from_map(HashMap::new());
        let ctx = Context { bible: &bible, v11n: &v11n, songs: &[], follow_scripture: false, follow_songs: true };
        let state = live(scripture_slide(1, "The LORD is my shepherd; I shall not want."));
        assert!(matches!(Follower::default().update("the lord is my shepherd", &state, &ctx), Action::None));
    }

    #[test]
    fn recognises_a_song_being_sung() {
        let mut idx = LyricsIndex::default();
        let songs = vec![songs::parse("Amazing Grace\n\nVerse 1\nAmazing grace how sweet the sound\nThat saved a wretch like me").unwrap()];
        idx.refresh(1, songs);
        let m = idx.detect("everybody sing amazing grace how sweet the sound").unwrap();
        assert_eq!(m.slide.label, "Amazing Grace · Verse 1");
        assert_eq!(m.phrase, "amazing grace how sweet the sound");
        assert!(idx.detect("good morning church").is_none());
    }

    /// A worship song built on a psalm: the sung line is recognised as the song, and the same
    /// words also match the verse, which is why scripture found in singing is held back while a
    /// song is on screen (see `pipeline::emit_references`).
    #[test]
    fn a_scripture_song_is_recognised_as_singing() {
        let mut idx = LyricsIndex::default();
        idx.refresh(1, vec![songs::parse("The Lord's My Shepherd\n\nVerse 1\nThe Lord is my shepherd I shall not want\nHe makes me down to lie").unwrap()]);
        let sung = "the lord is my shepherd i shall not want";
        assert!(idx.detect(sung).is_some(), "recognised as the song");
        let bible = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/bible.db");
        if let Ok(db) = crate::scripture::BibleDb::open(&bible) {
            let quotes = crate::scripture::quote::find_quotes(&db, sung).unwrap();
            assert!(quotes.iter().any(|q| q.reference.book_id == 19 && q.reference.chapter == 23), "and also as Psalm 23: {quotes:?}");
        }
    }
}
