//! The 66-book Protestant canon with spoken/ASR aliases.
//!
//! Aliases are lowercase, punctuation-free, and match the output of `numbers::normalize`.
//! Numbered books list their *base* aliases; ordinal prefixes ("1", "first", "1st", "ii", …)
//! are generated. `ambiguous` books are also common English words ("Mark", "Job", "Acts"),
//! so references to them need stronger evidence before auto-projecting.

pub struct Book {
    pub id: u8,
    /// Canonical name, matching the `books` table built by scripts/build_bible_db.py.
    #[allow(dead_code)]
    pub name: &'static str,
    /// Name used in labels ("Psalm 23:1", not "Psalms 23:1").
    pub display: &'static str,
    pub chapters: u16,
    pub ordinal: Option<u8>,
    pub ambiguous: bool,
    pub aliases: &'static [&'static str],
}

macro_rules! book {
    ($id:expr, $name:expr, $display:expr, $ch:expr, $ord:expr, $amb:expr, [$($a:expr),* $(,)?]) => {
        Book { id: $id, name: $name, display: $display, chapters: $ch, ordinal: $ord, ambiguous: $amb, aliases: &[$($a),*] }
    };
}

pub static BOOKS: [Book; 66] = [
    book!(1, "Genesis", "Genesis", 50, None, false, ["genesis", "gen"]),
    book!(2, "Exodus", "Exodus", 40, None, false, ["exodus", "exod"]),
    book!(3, "Leviticus", "Leviticus", 27, None, false, ["leviticus", "lev"]),
    book!(4, "Numbers", "Numbers", 36, None, true, ["numbers", "num"]),
    book!(5, "Deuteronomy", "Deuteronomy", 34, None, false, ["deuteronomy", "deut"]),
    book!(6, "Joshua", "Joshua", 24, None, false, ["joshua", "josh"]),
    book!(7, "Judges", "Judges", 21, None, true, ["judges", "judg"]),
    book!(8, "Ruth", "Ruth", 4, None, true, ["ruth"]),
    book!(9, "1 Samuel", "1 Samuel", 31, Some(1), false, ["samuel", "sam"]),
    book!(10, "2 Samuel", "2 Samuel", 24, Some(2), false, ["samuel", "sam"]),
    book!(11, "1 Kings", "1 Kings", 22, Some(1), false, ["kings", "kgs"]),
    book!(12, "2 Kings", "2 Kings", 25, Some(2), false, ["kings", "kgs"]),
    book!(13, "1 Chronicles", "1 Chronicles", 29, Some(1), false, ["chronicles", "chron", "chr"]),
    book!(14, "2 Chronicles", "2 Chronicles", 36, Some(2), false, ["chronicles", "chron", "chr"]),
    book!(15, "Ezra", "Ezra", 10, None, false, ["ezra"]),
    book!(16, "Nehemiah", "Nehemiah", 13, None, false, ["nehemiah", "neh"]),
    book!(17, "Esther", "Esther", 10, None, false, ["esther", "esth"]),
    book!(18, "Job", "Job", 42, None, true, ["job"]),
    book!(19, "Psalms", "Psalm", 150, None, false, ["psalms", "psalm", "psa", "ps"]),
    book!(20, "Proverbs", "Proverbs", 31, None, false, ["proverbs", "proverb", "prov"]),
    book!(21, "Ecclesiastes", "Ecclesiastes", 12, None, false, ["ecclesiastes", "ecclesiastics", "eccles", "eccl"]),
    book!(22, "Song of Solomon", "Song of Solomon", 8, None, false,
        ["song of solomon", "songs of solomon", "song of songs", "song of sol", "canticles"]),
    book!(23, "Isaiah", "Isaiah", 66, None, false, ["isaiah", "isa"]),
    book!(24, "Jeremiah", "Jeremiah", 52, None, false, ["jeremiah", "jer"]),
    book!(25, "Lamentations", "Lamentations", 5, None, false, ["lamentations", "lam"]),
    book!(26, "Ezekiel", "Ezekiel", 48, None, false, ["ezekiel", "ezek"]),
    book!(27, "Daniel", "Daniel", 12, None, true, ["daniel", "dan"]),
    book!(28, "Hosea", "Hosea", 14, None, false, ["hosea", "hos"]),
    book!(29, "Joel", "Joel", 3, None, true, ["joel"]),
    book!(30, "Amos", "Amos", 9, None, true, ["amos"]),
    book!(31, "Obadiah", "Obadiah", 1, None, false, ["obadiah", "obad"]),
    book!(32, "Jonah", "Jonah", 4, None, true, ["jonah"]),
    book!(33, "Micah", "Micah", 7, None, false, ["micah", "mic"]),
    book!(34, "Nahum", "Nahum", 3, None, false, ["nahum"]),
    book!(35, "Habakkuk", "Habakkuk", 3, None, false, ["habakkuk", "habakuk", "hab"]),
    book!(36, "Zephaniah", "Zephaniah", 3, None, false, ["zephaniah", "zeph"]),
    book!(37, "Haggai", "Haggai", 2, None, false, ["haggai", "hag"]),
    book!(38, "Zechariah", "Zechariah", 14, None, false, ["zechariah", "zech"]),
    book!(39, "Malachi", "Malachi", 4, None, false, ["malachi", "mal"]),
    book!(40, "Matthew", "Matthew", 28, None, false, ["matthew", "matt"]),
    book!(41, "Mark", "Mark", 16, None, true, ["mark"]),
    book!(42, "Luke", "Luke", 24, None, false, ["luke"]),
    book!(43, "John", "John", 21, None, false, ["john", "jn"]),
    book!(44, "Acts", "Acts", 28, None, true, ["acts of the apostles", "acts"]),
    book!(45, "Romans", "Romans", 16, None, false, ["romans", "rom"]),
    book!(46, "1 Corinthians", "1 Corinthians", 16, Some(1), false, ["corinthians", "corinthian", "cor"]),
    book!(47, "2 Corinthians", "2 Corinthians", 13, Some(2), false, ["corinthians", "corinthian", "cor"]),
    book!(48, "Galatians", "Galatians", 6, None, false, ["galatians", "gal"]),
    book!(49, "Ephesians", "Ephesians", 6, None, false, ["ephesians", "eph"]),
    book!(50, "Philippians", "Philippians", 4, None, false, ["philippians", "phillipians", "philipians", "phil"]),
    book!(51, "Colossians", "Colossians", 4, None, false, ["colossians", "col"]),
    book!(52, "1 Thessalonians", "1 Thessalonians", 5, Some(1), false, ["thessalonians", "thess", "thes"]),
    book!(53, "2 Thessalonians", "2 Thessalonians", 3, Some(2), false, ["thessalonians", "thess", "thes"]),
    book!(54, "1 Timothy", "1 Timothy", 6, Some(1), false, ["timothy", "tim"]),
    book!(55, "2 Timothy", "2 Timothy", 4, Some(2), false, ["timothy", "tim"]),
    book!(56, "Titus", "Titus", 3, None, false, ["titus"]),
    book!(57, "Philemon", "Philemon", 1, None, false, ["philemon", "philem"]),
    book!(58, "Hebrews", "Hebrews", 13, None, false, ["hebrews", "heb"]),
    book!(59, "James", "James", 5, None, true, ["james", "jas"]),
    book!(60, "1 Peter", "1 Peter", 5, Some(1), false, ["peter", "pet"]),
    book!(61, "2 Peter", "2 Peter", 3, Some(2), false, ["peter", "pet"]),
    book!(62, "1 John", "1 John", 5, Some(1), false, ["john", "jn"]),
    book!(63, "2 John", "2 John", 1, Some(2), false, ["john", "jn"]),
    book!(64, "3 John", "3 John", 1, Some(3), false, ["john", "jn"]),
    book!(65, "Jude", "Jude", 1, None, true, ["jude"]),
    book!(66, "Revelation", "Revelation", 22, None, false, ["revelation", "revelations", "rev"]),
];

pub fn by_id(id: u8) -> Option<&'static Book> {
    BOOKS.get((id as usize).checked_sub(1)?)
}

/// The book whose name `word` sounds like ("philippines" → Philippians, "romance" → Romans),
/// among books with the given ordinal ("first corinthean"). None if no book is close or two
/// different books are equally close.
pub fn fuzzy_match(word: &str, ordinal: Option<u8>) -> Option<&'static Book> {
    let word_sound = super::fuzzy::sound(word);
    let mut best: Option<(usize, &'static Book)> = None;
    let mut tied = false;
    for b in BOOKS.iter().filter(|b| b.ordinal == ordinal) {
        for &alias in b.aliases.iter().filter(|a| a.len() >= 4 && !a.contains(' ')) {
            if !super::fuzzy::alike(word, &word_sound, alias, &super::fuzzy::sound(alias)) {
                continue;
            }
            let d = super::fuzzy::edits(word, alias);
            match best {
                Some((bd, bb)) if d == bd && bb.id != b.id => tied = true,
                Some((bd, _)) if d >= bd => {}
                _ => {
                    best = Some((d, b));
                    tied = false;
                }
            }
        }
    }
    best.filter(|_| !tied).map(|(_, b)| b)
}

/// "1", "first", "1st", "ii", … → 1, 2 or 3.
pub fn ordinal_value(prefix: &str) -> Option<u8> {
    (1..=3).find(|&n| ordinal_prefixes(n).contains(&prefix))
}

fn ordinal_prefixes(n: u8) -> &'static [&'static str] {
    match n {
        1 => &["1", "first", "1st"],
        2 => &["2", "second", "2nd", "ii"],
        3 => &["3", "third", "3rd", "iii"],
        _ => &[],
    }
}

/// (alias, book id, weak). An alias is weak if its book is ambiguous or it is a short abbreviation.
pub fn all_aliases() -> Vec<(String, u8, bool)> {
    let mut out = Vec::new();
    for b in &BOOKS {
        for &a in b.aliases {
            let weak = b.ambiguous || a.len() <= 3;
            match b.ordinal {
                None => out.push((a.to_string(), b.id, weak)),
                Some(n) => {
                    for p in ordinal_prefixes(n) {
                        out.push((format!("{p} {a}"), b.id, weak));
                    }
                }
            }
        }
    }
    out
}

