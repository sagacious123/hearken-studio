//! The 66-book Protestant canon with spoken/ASR aliases. Names in other languages are in `lang`.
//!
//! Aliases are lowercase, punctuation-free, and match the output of `numbers::normalize`.
//! Numbered books list their *base* aliases; ordinal prefixes ("1", "first", "1st", "ii", …)
//! are generated. `ambiguous` books are also common English words ("Mark", "Job", "Acts"),
//! so references to them need stronger evidence before auto-projecting.

use super::lang::Lang;

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

/// How book names commonly come out of speech recognition, or are said in other accents
/// ("Exodos", "Deutronomy", "Isaya", "first Pita"). Recognised like the real name, chapter-only
/// references included, at slightly lower confidence (`parser::MISHEARD_PENALTY`). Numbered
/// books list base names; ordinals are added as usual. Everyday words and place names
/// ("Philippines", "Romance", "Easter") are left to the sound-alike matcher, which needs more
/// evidence. (book ids, spellings, weak) — weak spellings are also ordinary words.
pub static MISHEARD: &[(&[u8], &[&str], bool)] = &[
    (&[1], &["genises", "genisis", "genesys", "jenesis", "geneses"], false),
    (&[2], &["exodos", "exodous", "exedus", "exudus", "exodas", "exodes"], false),
    (&[3], &["leviticos", "levitikus", "levitcus", "leviticous", "levitikos"], false),
    (&[5], &["deutronomy", "deuteronomi", "deutoronomy", "deuteronomey", "dutronomy", "deuteronomies", "deutoronomi"], false),
    (&[6], &["joshuah", "jeshua", "joshwa"], false),
    (&[9, 10], &["samual", "semuel", "samwel", "samuels"], false),
    (&[11, 12], &["king"], true),
    (&[13, 14], &["chronicle", "cronicles", "chronicals", "cronicals", "kronicles"], false),
    (&[16], &["nehemia", "nehemaya", "nehemiya", "nehamiah"], false),
    (&[17], &["ester", "easter", "esta", "asta"], true),
    (&[17], &["estra", "estha", "esthar"], false),
    (&[19], &["salm", "salms", "psalmes", "psams"], false),
    (&[20], &["proverbes", "provabs", "provebs"], false),
    (&[21], &["ecclesiates", "eclesiastes", "ecclesiastis", "eclesiates", "ecclesiastes's"], false),
    (&[23], &["isaya", "isiah", "isaia", "isayah", "esaias", "isaiahs"], false),
    (&[24], &["jeremaya", "jeremia", "jerimiah", "jeremaiah", "jeremiahs"], false),
    (&[25], &["lamentation", "lamentaions"], false),
    (&[26], &["ezekial", "ezikiel", "ezechiel", "izikiel", "ezekiah", "ezekel"], false),
    (&[28], &["hoshea", "hozea"], false),
    (&[31], &["obadaya", "obadia", "obediah"], false),
    (&[33], &["maika", "micha", "mikah"], false),
    (&[34], &["nahom"], false),
    (&[35], &["habbakuk", "habakook", "habakkok", "habakok", "abakuk", "habacuc"], false),
    (&[36], &["zephania", "sefania", "zefaniah"], false),
    (&[37], &["hagai", "haggi", "hagay"], false),
    (&[38], &["zachariah", "zecharia", "zakaria", "zacharia", "zacharias", "zakariah"], false),
    (&[39], &["malachai", "malaki", "malakai", "malakia", "malachy"], false),
    (&[40], &["mathew", "matthews", "mattew", "mathews", "matiu"], false),
    (&[43, 62, 63, 64], &["jon", "johns"], true),
    (&[45], &["roman"], true),
    (&[46, 47], &["corinthias", "carinthians", "chorinthians", "corintians", "corinthans", "corinthiens", "korinthians", "corithians"], false),
    (&[48], &["galatian", "galations", "galeshians", "galatias", "galasians", "galatiens"], false),
    (&[49], &["ephesian", "efesians", "ephesions", "ephisians", "ephesus", "ephesiens"], false),
    (&[50], &["philippian", "phillippians", "philipian", "filipians", "philippiens", "phillipian"], false),
    (&[51], &["colossian", "collosians", "colosians", "colossions", "kolossians", "colosians"], false),
    (&[52, 53], &["thessalonian", "thesalonians", "tesalonians", "thessalonica", "thessalonions", "thesalonica"], false),
    (&[54, 55], &["timoty", "timothee", "timotheus", "timoti"], false),
    (&[57], &["philemone", "filemon", "filemone"], false),
    (&[58], &["hebrew"], true),
    (&[60, 61], &["pita", "peta", "petre", "peters"], false),
    (&[66], &["revalation", "revilation", "revelasion", "revalations", "revelation's"], false),
];

/// Is `alias` (as matched, ordinal included) one of the `MISHEARD` spellings?
pub fn is_misheard(alias: &str) -> bool {
    let base = alias.rsplit(' ').next().unwrap_or(alias);
    MISHEARD.iter().any(|(_, spellings, _)| spellings.contains(&base))
}

fn misheard_for(id: u8) -> impl Iterator<Item = (&'static str, bool)> {
    MISHEARD.iter().filter(move |(ids, _, _)| ids.contains(&id)).flat_map(|(_, spellings, weak)| spellings.iter().map(move |s| (*s, *weak)))
}

pub fn by_id(id: u8) -> Option<&'static Book> {
    BOOKS.get((id as usize).checked_sub(1)?)
}

/// A book's name as it's listened for, before any ordinal ("corinthians", "jean").
#[derive(Debug, Clone)]
pub struct Spelling {
    pub id: u8,
    pub text: &'static str,
    pub lang: Lang,
    /// An ambiguous book, an everyday word, or a short abbreviation: needs stronger evidence.
    pub weak: bool,
    /// One of the `MISHEARD` spellings.
    pub misheard: bool,
}

/// Every name of every book in `langs`.
pub fn spellings(langs: &[Lang]) -> Vec<Spelling> {
    let mut out = Vec::new();
    for &lang in langs {
        if lang == Lang::En {
            for b in &BOOKS {
                for &a in b.aliases {
                    out.push(Spelling { id: b.id, text: a, lang, weak: b.ambiguous || a.len() <= 3, misheard: false });
                }
                for (a, weak) in misheard_for(b.id) {
                    out.push(Spelling { id: b.id, text: a, lang, weak: b.ambiguous || weak || a.len() <= 3, misheard: true });
                }
            }
            continue;
        }
        for (ids, names, weak) in lang.books() {
            for &id in *ids {
                for &a in *names {
                    out.push(Spelling { id, text: a, lang, weak: *weak || a.len() <= 3, misheard: false });
                }
            }
        }
    }
    out
}

/// The book whose name `word` sounds like ("philippines" → Philippians, "romance" → Romans),
/// among `spellings` of books with the given ordinal ("first corinthean"). None if no book is
/// close or two different books are equally close.
pub fn fuzzy_match(word: &str, ordinal: Option<u8>, spellings: &[Spelling]) -> Option<(&'static Book, usize)> {
    let word_sound = super::fuzzy::sound(word);
    let mut best: Option<(usize, &'static Book)> = None;
    let mut tied = false;
    let candidates = spellings.iter().filter(|s| !(s.misheard && s.weak) && s.text.len() >= 4 && !s.text.contains(' '));
    for s in candidates {
        let Some(b) = by_id(s.id).filter(|b| b.ordinal == ordinal) else { continue };
        if !super::fuzzy::alike(word, &word_sound, s.text, &super::fuzzy::sound(s.text)) {
            continue;
        }
        let d = super::fuzzy::edits(word, s.text);
        match best {
            Some((bd, bb)) if d == bd && bb.id != b.id => tied = true,
            Some((bd, _)) if d >= bd => {}
            _ => {
                best = Some((d, b));
                tied = false;
            }
        }
    }
    best.filter(|_| !tied).map(|(d, b)| (b, d))
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

/// (alias, book id, weak) for every name in `langs`, ordinals included ("1 john", "first john",
/// "1 epitre de jean", "1 de juan"). Other languages' ordinals are digits by the time the parser
/// sees them (see `numbers::normalize_in`).
pub fn aliases_for(langs: &[Lang]) -> Vec<(String, u8, bool)> {
    let mut out = Vec::new();
    for s in spellings(langs) {
        let Some(book) = by_id(s.id) else { continue };
        match book.ordinal {
            None => out.push((s.text.to_string(), s.id, s.weak)),
            Some(n) if s.lang == Lang::En => {
                for p in ordinal_prefixes(n) {
                    out.push((format!("{p} {}", s.text), s.id, s.weak));
                }
            }
            Some(n) => {
                for link in s.lang.ordinal_links() {
                    out.push((format!("{n} {link}{}", s.text), s.id, s.weak));
                }
            }
        }
    }
    out
}

/// English names: (alias, book id, weak). An alias is weak if its book is ambiguous or it is a
/// short abbreviation.
#[cfg(test)]
pub fn all_aliases() -> Vec<(String, u8, bool)> {
    aliases_for(&[Lang::En])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aliases_in_every_language() {
        let all = aliases_for(&Lang::ALL);
        let find = |a: &str| all.iter().find(|(x, _, _)| x == a).map(|(_, id, weak)| (*id, *weak));
        assert_eq!(find("1 john"), Some((62, false)));
        assert_eq!(find("1 jean"), Some((62, false)));
        assert_eq!(find("jean"), Some((43, false)));
        assert_eq!(find("1 epitre aux corinthiens"), Some((46, false)));
        assert_eq!(find("2 de timoteo"), Some((55, false)));
        assert_eq!(find("san juan"), Some((43, false)));
        assert_eq!(find("pierre"), None, "only with an ordinal");
        assert_eq!(find("1 pierre"), Some((60, true)));
        assert_eq!(find("marc"), Some((41, true)));
        assert!(all_aliases().iter().all(|(a, _, _)| a != "jean"), "English only");
    }
}
