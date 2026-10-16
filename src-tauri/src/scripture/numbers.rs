//! Normalises ASR text so a single regex can match references however Whisper writes them, in
//! any of the preaching languages: "John 3:16", "John 3.16", "john chapter three verse sixteen",
//! "First Corinthians 13, 4 to 7", "Jean chapitre trois, verset seize", "Primera de Corintios
//! trece, del cuatro al siete". The languages' words for "chapter", "verse", "to" and "and"
//! become the English ones the parser reads, and accents are dropped.

use super::lang::{fold, Connector, Lang};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NumWord {
    Unit(u32), // 0..=19
    Ten(u32),  // 20, 30, … 90
    Hundred,
    /// A whole number in one word ("veintiocho") or a hyphenated one worked out already.
    Whole(u32),
}
use NumWord::*;

/// Marks a hyphenated number already worked out ("dix-sept" → "#17").
const COMPOUND: char = '#';

fn num_word(w: &str, langs: &[Lang]) -> Option<NumWord> {
    if let Some(v) = w.strip_prefix(COMPOUND).and_then(|v| v.parse::<u32>().ok()) {
        return Some(if v < 20 { Unit(v) } else { Whole(v) });
    }
    langs.iter().find_map(|l| {
        let n = l.numbers();
        let find = |table: &[(&str, u32)]| table.iter().find(|(s, _)| *s == w).map(|&(_, v)| v);
        find(n.units)
            .map(Unit)
            .or_else(|| find(n.tens).map(Ten))
            .or_else(|| n.hundreds.contains(&w).then_some(Hundred))
            .or_else(|| find(n.wholes).map(Whole))
    })
}

/// "un", "una", or (with English listened for too) "once": a number only beside another.
fn is_article(w: &str, langs: &[Lang]) -> bool {
    let english = langs.contains(&Lang::En);
    langs.iter().any(|l| {
        let n = l.numbers();
        n.articles.contains(&w) || (english && *l != Lang::En && n.english_words.contains(&w))
    })
}

fn is_number(w: &str) -> bool {
    !w.is_empty() && w.chars().all(|c| c.is_ascii_digit() || c == ':')
}

fn is_keyword(w: &str) -> bool {
    matches!(w, "chapter" | "chapters" | "verse" | "verses")
}

/// "chapitre" → "chapter", "première" → "1".
fn word(w: &str, langs: &[Lang]) -> String {
    langs
        .iter()
        .find_map(|l| l.keywords().iter().chain(l.ordinals()).find(|(from, _)| *from == w).map(|(_, to)| (*to).to_owned()))
        .unwrap_or_else(|| w.to_owned())
}

/// A hyphenated number ("twenty-three", "dix-sept", "quatre-vingt-dix-neuf", "vingt-et-un"), or
/// None when the parts aren't one number ("three-sixteen" is John 3-16).
fn compound(parts: &[&str], langs: &[Lang]) -> Option<u32> {
    if parts.len() < 2 {
        return None;
    }
    let vigesimal = langs.iter().any(|l| l.numbers().vigesimal);
    let joiners: Vec<&str> = langs
        .iter()
        .filter_map(|l| match l.numbers().connector {
            Connector::AfterTen(c) => Some(c),
            Connector::AfterHundred(_) => None,
        })
        .collect();
    let mut nums = Vec::with_capacity(parts.len());
    for p in parts.iter().filter(|p| !joiners.contains(p)) {
        nums.push(num_word(p, langs)?);
    }
    if vigesimal {
        nums = merge(nums, |a, b| matches!((a, b), (Unit(4), Ten(20))).then_some(Ten(80)));
        nums = merge(nums, |a, b| match (a, b) {
            (Unit(10), Unit(u)) if (7..=9).contains(&u) => Some(Unit(10 + u)),
            _ => None,
        });
    }
    let (mut value, mut k) = match nums.as_slice() {
        [Unit(h), Hundred, ..] if (1..10).contains(h) => (h * 100, 2),
        [Hundred, ..] => (100, 1),
        _ => (0, 0),
    };
    match nums.get(k) {
        Some(Ten(t)) => {
            value += t;
            k += 1;
            if let Some(Unit(u)) = nums.get(k) {
                if *u >= 10 && !(vigesimal && (*t == 60 || *t == 80)) {
                    return None;
                }
                value += u;
                k += 1;
            }
        }
        Some(Unit(u) | Whole(u)) => {
            value += u;
            k += 1;
        }
        _ => {}
    }
    (k == nums.len()).then_some(value)
}

fn merge(nums: Vec<NumWord>, join: impl Fn(NumWord, NumWord) -> Option<NumWord>) -> Vec<NumWord> {
    let mut out: Vec<NumWord> = Vec::with_capacity(nums.len());
    for n in nums {
        match out.last().and_then(|&prev| join(prev, n)) {
            Some(joined) => *out.last_mut().unwrap() = joined,
            None => out.push(n),
        }
    }
    out
}

/// English only: lowercase, strip punctuation (keeping chapter/verse separators), and convert
/// spoken numbers to digits. Adjacent numbers stay separate: "three sixteen" → "3 16".
pub fn normalize(input: &str) -> String {
    normalize_in(input, &[Lang::En])
}

/// `normalize` for text in any of `langs`.
pub fn normalize_in(input: &str, langs: &[Lang]) -> String {
    let folded = fold(input);
    let chars: Vec<char> = folded.chars().collect();
    let mut s = String::with_capacity(folded.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| chars[j]);
        let next = chars.get(i + 1).copied();
        let prev_digit = prev.is_some_and(|c| c.is_ascii_digit());
        let next_digit = next.is_some_and(|c| c.is_ascii_digit());
        match c {
            c if c.is_alphanumeric() => s.push(c),
            ':' if prev_digit || next_digit => s.push(':'),
            '.' if prev_digit && next_digit => s.push(':'), // "3.16"
            // "dix-sept", "twenty-three": maybe one number, worked out below.
            '-' if prev.is_some_and(char::is_alphabetic) && next.is_some_and(char::is_alphabetic) => s.push('-'),
            '-' | '–' | '—' => s.push_str(" - "),
            '\'' | '’' => {}
            _ => s.push(' '),
        }
    }

    let mut tokens: Vec<String> = Vec::new();
    for t in s.split_whitespace() {
        if !t.contains('-') || t == "-" {
            tokens.push(word(t, langs));
            continue;
        }
        let parts: Vec<&str> = t.split('-').filter(|p| !p.is_empty()).collect();
        match compound(&parts, langs) {
            Some(v) => tokens.push(format!("{COMPOUND}{v}")),
            None => {
                for (k, p) in parts.iter().enumerate() {
                    if k > 0 {
                        tokens.push("-".into());
                    }
                    tokens.push(word(p, langs));
                }
            }
        }
    }

    let vigesimal = langs.iter().any(|l| l.numbers().vigesimal);
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i].as_str();
        // "a hundred and nineteen"
        if t == "a" && tokens.get(i + 1).is_some_and(|n| n == "hundred") && langs.contains(&Lang::En) {
            i += 1;
            continue;
        }
        if num_word(t, langs).is_none() {
            out.push(t.to_owned());
            i += 1;
            continue;
        }
        // "un jour" is "a day"; "chapitre un" and "un seize" are numbers.
        if is_article(t, langs) {
            let beside = |w: Option<&str>| w.is_some_and(|w| is_number(w) || is_keyword(w) || matches!(w, "to" | "and"));
            let next_number = tokens.get(i + 1).is_some_and(|n| is_number(n) || is_keyword(n) || num_word(n, langs).is_some());
            if !beside(out.last().map(String::as_str)) && !next_number {
                out.push(t.to_owned());
                i += 1;
                continue;
            }
        }

        let mut value = 0u32;
        let mut last: Option<NumWord> = None;
        let mut j = i;
        while j < tokens.len() {
            match num_word(&tokens[j], langs) {
                Some(Unit(u)) => {
                    // "three sixteen" is two numbers; "twenty three" is one; "twenty twelve" is two;
                    // "soixante douze" is 72.
                    match last {
                        Some(Unit(_) | Whole(_)) => break,
                        Some(Ten(t)) if u >= 10 && !(vigesimal && (t == 60 || t == 80)) => break,
                        _ => {}
                    }
                    value += u;
                    last = Some(Unit(u));
                }
                Some(Ten(t)) => {
                    if matches!(last, Some(Unit(_) | Ten(_) | Whole(_))) {
                        break;
                    }
                    value += t;
                    last = Some(Ten(t));
                }
                Some(Hundred) => {
                    if matches!(last, Some(Hundred | Ten(_) | Whole(_))) {
                        break;
                    }
                    value = value.max(1) * 100;
                    last = Some(Hundred);
                }
                Some(Whole(v)) => {
                    if !matches!(last, None | Some(Hundred)) {
                        break;
                    }
                    value += v;
                    last = Some(Whole(v));
                }
                None => {
                    // "one hundred and nineteen", "vingt et un", "treinta y ocho"
                    let w = tokens[j].as_str();
                    let next = tokens.get(j + 1).and_then(|n| num_word(n, langs));
                    let continues = langs.iter().any(|l| match l.numbers().connector {
                        Connector::AfterHundred(c) => w == c && last == Some(Hundred) && matches!(next, Some(Unit(_) | Ten(_) | Whole(_))),
                        Connector::AfterTen(c) => w == c && matches!(last, Some(Ten(_))) && matches!(next, Some(Unit(u)) if (1..10).contains(&u) || u == 11),
                    });
                    if !continues {
                        break;
                    }
                }
            }
            j += 1;
        }
        out.push(value.to_string());
        i = j;
    }

    // "versets 4 à 7" → "verses 4 to 7"; "du verset 16 au 18" → "verse 16 to 18".
    let range: Vec<&str> = langs.iter().flat_map(|l| l.range_words()).copied().collect();
    let and: Vec<&str> = langs.iter().flat_map(|l| l.and_words()).copied().collect();
    let det: Vec<&str> = langs.iter().flat_map(|l| l.determiners()).copied().collect();
    if range.is_empty() && and.is_empty() && det.is_empty() {
        return out.join(" ");
    }
    // The first word after `k` that isn't a determiner ("desde el versículo" → "versículo").
    let past_det = |k: usize| out[k + 1..].iter().map(String::as_str).find(|w| !det.contains(w));
    let mut result: Vec<String> = Vec::with_capacity(out.len());
    for k in 0..out.len() {
        let w = out[k].as_str();
        let prev = result.last().map(String::as_str);
        let after = past_det(k);
        if range.contains(&w) && prev.is_some_and(is_number) && after.is_some_and(|n| is_number(n) || is_keyword(n)) {
            result.push("to".into());
        } else if and.contains(&w) {
            result.push("and".into());
        } else if det.contains(&w)
            && (after.is_some_and(is_keyword) || (prev.is_some_and(|p| is_number(p) || p == "to" || p == "and") && after.is_some_and(is_number)))
        {
            // dropped: "du verset", "13 del 4", "hasta el 18"
        } else {
            result.push(w.to_owned());
        }
    }
    result.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spoken_numbers() {
        assert_eq!(normalize("John chapter three verse sixteen"), "john chapter 3 verse 16");
        assert_eq!(normalize("John three sixteen"), "john 3 16");
        assert_eq!(normalize("Psalm one hundred and nineteen"), "psalm 119");
        assert_eq!(normalize("Psalm a hundred and twenty one"), "psalm 121");
        assert_eq!(normalize("Proverbs twenty three, verse seven"), "proverbs 23 verse 7");
        assert_eq!(normalize("Psalm twenty-three"), "psalm 23");
        assert_eq!(normalize("John three-sixteen"), "john 3 - 16");
    }

    #[test]
    fn punctuation_and_separators() {
        assert_eq!(normalize("Turn to John 3:16-18."), "turn to john 3:16 - 18");
        assert_eq!(normalize("Romans 8.28"), "romans 8:28");
        assert_eq!(normalize("God's love, in John 3, 16"), "gods love in john 3 16");
        assert_eq!(normalize("self-control"), "self - control");
    }

    #[test]
    fn french() {
        let fr = |s: &str| normalize_in(s, &[Lang::Fr]);
        assert_eq!(fr("Jean chapitre trois, verset seize"), "jean chapter 3 verse 16");
        assert_eq!(fr("Romains huit, vingt-huit"), "romains 8 28");
        assert_eq!(fr("Psaume cent dix-neuf"), "psaume 119");
        assert_eq!(fr("Psaume quatre-vingt-dix"), "psaume 90");
        assert_eq!(fr("Psaume soixante-et-onze"), "psaume 71");
        assert_eq!(fr("Psaume vingt et un"), "psaume 21");
        assert_eq!(fr("Psaume soixante douze"), "psaume 72");
        assert_eq!(fr("Jean trois seize"), "jean 3 16");
        assert_eq!(fr("Jean trois-seize"), "jean 3 - 16");
        assert_eq!(fr("Première épître aux Corinthiens 13, versets 4 à 7"), "1 epitre aux corinthiens 13 verses 4 to 7");
        assert_eq!(fr("du verset 4 au verset 7"), "verse 4 to verse 7");
        assert_eq!(fr("Ésaïe 53, versets 4 et 5"), "esaie 53 verses 4 and 5");
        assert_eq!(fr("jusqu'au verset 12"), "jusquau verse 12");
        assert_eq!(fr("versets 10 jusqu'à 12"), "verses 10 to 12");
        // "un" is a number only beside one.
        assert_eq!(fr("Jean un jour a dit"), "jean un jour a dit");
        assert_eq!(fr("Genèse chapitre un"), "genese chapter 1");
        assert_eq!(fr("Jean un, seize"), "jean 1 16");
    }

    #[test]
    fn spanish() {
        let es = |s: &str| normalize_in(s, &[Lang::Es]);
        assert_eq!(es("Juan capítulo tres, versículo dieciséis"), "juan chapter 3 verse 16");
        assert_eq!(es("Romanos ocho veintiocho"), "romanos 8 28");
        assert_eq!(es("Salmo ciento diecinueve"), "salmo 119");
        assert_eq!(es("Salmo treinta y siete"), "salmo 37");
        assert_eq!(es("Salmo ciento cincuenta"), "salmo 150");
        assert_eq!(es("Primera de Corintios trece, del cuatro al siete"), "1 de corintios 13 4 to 7");
        assert_eq!(es("Juan 3 desde el versículo 16 hasta el 18"), "juan 3 verse 16 to 18");
        assert_eq!(es("Isaías 53 versículos 4 y 5"), "isaias 53 verses 4 and 5");
        assert_eq!(es("Juan una vez dijo"), "juan una vez dijo");
        assert_eq!(es("Juan uno uno"), "juan 1 1");
    }

    #[test]
    fn languages_together() {
        let both = |s: &str| normalize_in(s, &[Lang::En, Lang::Es]);
        // "once" is English too.
        assert_eq!(both("John once said"), "john once said");
        assert_eq!(both("Juan capítulo once"), "juan chapter 11");
        assert_eq!(both("John chapter three verse sixteen"), "john chapter 3 verse 16");
        assert_eq!(normalize_in("Juan once", &[Lang::Es]), "juan 11");
    }
}
