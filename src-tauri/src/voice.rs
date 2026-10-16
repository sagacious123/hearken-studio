//! Spoken commands for the live scripture: "next verse", "go back", "go to the first verse".
//!
//! Preachers say "next" and "go back" all the time, so a phrase only counts when it's clearly
//! a command: said on its own ("Next verse." / "Go back.") or as an instruction ("let's go to
//! the next verse", "take us back to the previous verse"). Narration ("in the next verse Paul
//! says…"), places ("let's go back to Genesis") and time ("next week") are left alone.
//!
//! French and Spanish follow the same rules with their own phrases ("Verset suivant.",
//! "Passons au verset 6.", "Volvamos atrás.", "Vamos al versículo siguiente.").

use std::sync::LazyLock;

use regex::Regex;

use crate::scripture::{lang::Lang, numbers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Next,
    Previous,
    /// Verse 1 of the live chapter.
    First,
    /// The chapter's final verse.
    Final,
    /// A verse (or verses) of the live chapter: "go to verse 6", "the tenth verse", "go to 24".
    Verse(u16, Option<u16>),
}

impl Command {
    pub fn label(self) -> String {
        match self {
            Command::Next => "Next verse".into(),
            Command::Previous => "Previous verse".into(),
            Command::First => "First verse".into(),
            Command::Final => "Last verse of the chapter".into(),
            Command::Verse(s, Some(e)) => format!("Verses {s}–{e}"),
            Command::Verse(s, None) => format!("Verse {s}"),
        }
    }

    /// Only a specific verse may be completed by the next sentence ("Go to verse." / "6.").
    fn is_verse(self) -> bool {
        matches!(self, Command::Verse(..))
    }
}

const ORDINALS: &[(&str, u16)] = &[
    ("first", 1), ("second", 2), ("third", 3), ("fourth", 4), ("fifth", 5), ("sixth", 6), ("seventh", 7), ("eighth", 8),
    ("ninth", 9), ("tenth", 10), ("eleventh", 11), ("twelfth", 12), ("thirteenth", 13), ("fourteenth", 14), ("fifteenth", 15),
    ("sixteenth", 16), ("seventeenth", 17), ("eighteenth", 18), ("nineteenth", 19), ("twentieth", 20), ("thirtieth", 30),
    ("fortieth", 40), ("fiftieth", 50), ("sixtieth", 60), ("seventieth", 70), ("eightieth", 80), ("ninetieth", 90), ("hundredth", 100),
];

/// Lowercase words; spoken numbers as digits ("twenty four" → "24"); ordinals as "<n>th"
/// ("tenth", "10th", "twenty first", "21st" → "10th", "21th"); mishearings of "verse" folded in.
fn normalize(text: &str) -> String {
    let words: Vec<String> = text
        .to_lowercase()
        .replace(['’', '\''], "")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| match w {
            "versse" | "vers" | "verses" | "vs" | "vous" => "verse".to_owned(),
            "nex" | "necks" | "nexts" => "next".to_owned(),
            "previews" | "previos" => "previous".to_owned(),
            w => match ORDINALS.iter().find(|(o, _)| *o == w) {
                Some((_, n)) => format!("{n}th"),
                // "10th", "21st", "2nd", "3rd"
                None if w.len() > 2 && w[..w.len() - 2].chars().all(|c| c.is_ascii_digit()) && ["th", "st", "nd", "rd"].contains(&&w[w.len() - 2..]) => {
                    format!("{}th", &w[..w.len() - 2])
                }
                None => w.to_owned(),
            },
        })
        .collect();
    let t = numbers::normalize(&words.join(" "));
    // "twenty first" → "20 1th" → "21th"
    let mut out: Vec<String> = Vec::new();
    for w in t.split_whitespace() {
        if let (Some(prev), Some(unit)) = (out.last(), w.strip_suffix("th").and_then(|n| n.parse::<u16>().ok())) {
            if let Ok(tens) = prev.parse::<u16>() {
                if tens % 10 == 0 && (20..=90).contains(&tens) && unit < 10 {
                    *out.last_mut().unwrap() = format!("{}th", tens + unit);
                    continue;
                }
            }
        }
        out.push(w.to_owned());
    }
    out.join(" ")
}

const WHICH: &str = r"(?P<which>next|following|previous|prior|preceding|last|final|opening|1th)";
const ITEM: &str = r"(?:verse|scripture|one|1|slide|passage)";
/// Motion verbs are commands anywhere; "read", "look at" and "see" only when asked ("let's read",
/// "can we read", "read verse 6." at the start), so "we read verse 6 last week" is just talk.
const VERB: &str = r"(?:go|move|jump|skip|turn|take us|take me|bring us|bring up|put up|show us|give us|(?:^|\b(?:lets|let us|can we|could we|shall we|please|now|okay|ok|and|so)\s+)(?:read|look at|see|show))";
const MOVE: &str = r"(?:\s+(?:on|back|ahead|forward|over|down|up))?(?:\s+(?:to|into|unto))?";
const RANGE_SEP: &str = r"(?:to|through|thru|till|until|and|2|too)";
const FILLER: &str = r"(?:(?:okay|ok|alright|all right|so|and|now|then|yes|right|please|church|brethren|lets|let us|can we|could we|shall we|well)\s+)*";

/// The whole sentence is the command: "Next.", "Next verse please.", "The previous one."
static ALONE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"^{FILLER}(?:the\s+)?{WHICH}(?:\s+{ITEM})?(?:\s+please)?$")).unwrap());

/// An instruction anywhere in the sentence: "let's go to the next verse", "read the next verse",
/// "take us back to the previous verse", "move on to the following verse".
static INSTRUCTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"\b{VERB}{MOVE}(?:\s+(?:the|our))?\s+{WHICH}\s+{ITEM}\b"
    ))
    .unwrap()
});

/// "Go back." / "Go back one verse." / "Back one." — only at the end of the sentence, so
/// "let's go back to Genesis 3" stays a reference.
static BACK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?:^{FILLER}|\b(?:lets|let us)\s+)(?:go\s+|move\s+|step\s+)?back(?:\s+(?:1|a verse|1 verse|a bit))?(?:\s+please)?$")).unwrap()
});

/// "Go forward." / "Move on." on its own.
static FORWARD: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"^{FILLER}(?:go|move|step)\s+(?:forward|on|ahead)(?:\s+(?:1|a verse|1 verse))?(?:\s+please)?$")).unwrap());

/// "go to verse 6", "let's read verses 6 to 8", "jump to verse number 12" (anywhere in the sentence).
static VERSE_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"\b{VERB}{MOVE}(?:\s+the)?\s+verse(?:\s+number)?\s+(?P<s>\d{{1,3}})(?:\s+{RANGE_SEP}\s+(?:verse\s+)?(?P<e>\d{{1,3}}))?(?P<rest>.*)$")).unwrap()
});

/// "go to the tenth verse" (anywhere), "the 10th verse" (on its own).
static ORDINAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"\b{VERB}{MOVE}(?:\s+the)?\s+(?P<s>\d{{1,3}})th\s+verse\b")).unwrap());
static ORDINAL_ALONE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"^{FILLER}(?:the\s+)?(?P<s>\d{{1,3}})th\s+verse(?:\s+please)?$")).unwrap());

/// "go to 24" — only as the whole sentence, so "go to 1 Corinthians" stays a reference.
static BARE_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"^{FILLER}(?:go|move|jump|skip|turn){MOVE}\s+(?P<s>\d{{1,3}})(?:\s+{RANGE_SEP}\s+(?P<e>\d{{1,3}}))?(?:\s+please)?$")).unwrap()
});

fn which(word: &str) -> Command {
    match word {
        "next" | "following" => Command::Next,
        "previous" | "prior" | "preceding" | "last" => Command::Previous,
        "1th" | "opening" => Command::First,
        _ => Command::Final, // "final"
    }
}

fn verse(s: &str, e: Option<&str>) -> Option<Command> {
    let start: u16 = s.parse().ok().filter(|&n| n > 0)?;
    let end = e.and_then(|e| e.parse::<u16>().ok()).filter(|&e| e > start);
    Some(if start == 1 && end.is_none() { Command::First } else { Command::Verse(start, end) })
}

/// The command in a finished sentence, if it contains one.
pub fn parse(text: &str) -> Option<Command> {
    let t = normalize(text);
    if t.is_empty() || t.split(' ').count() > 24 {
        return None;
    }
    if let Some(c) = VERSE_NUMBER.captures(&t) {
        // "read verse 6 of John 3" names another chapter: that's a reference, not a move.
        let rest = c.name("rest").map_or("", |m| m.as_str()).trim_start();
        if !(rest.starts_with("of ") || rest.starts_with("in ")) {
            return verse(&c["s"], c.name("e").map(|m| m.as_str()));
        }
    }
    if let Some(c) = ORDINAL.captures(&t).or_else(|| ORDINAL_ALONE.captures(&t)) {
        return verse(&c["s"], None);
    }
    if let Some(c) = BARE_NUMBER.captures(&t) {
        return verse(&c["s"], c.name("e").map(|m| m.as_str()));
    }
    if let Some(c) = ALONE.captures(&t) {
        return Some(which(&c["which"]));
    }
    if let Some(c) = INSTRUCTION.captures(&t) {
        return Some(which(&c["which"]));
    }
    if BACK.is_match(&t) {
        return Some(Command::Previous);
    }
    if FORWARD.is_match(&t) {
        return Some(Command::Next);
    }
    None
}

/// A command split by a pause ("Go to verse." / "6."): the previous sentence and this one read
/// together. Only a specific verse can be completed this way.
#[cfg(test)]
pub fn parse_joined(previous: &str, current: &str) -> Option<Command> {
    parse(&format!("{previous} {current}")).filter(|c| c.is_verse())
}

/// The command in a finished sentence said in any of `langs`.
pub fn parse_in(text: &str, langs: &[Lang]) -> Option<Command> {
    langs.iter().find_map(|&l| match l {
        Lang::En => parse(text),
        _ => Phrases::of(l).and_then(|p| p.parse(text, l)),
    })
}

/// `parse_joined` for any of `langs`.
pub fn parse_joined_in(previous: &str, current: &str, langs: &[Lang]) -> Option<Command> {
    parse_in(&format!("{previous} {current}"), langs).filter(|c| c.is_verse())
}

/// Commands in another language, matched on text normalised for it: "verset" is "verse",
/// numbers are digits, "à" between numbers is "to", and "au"/"le"/"del" before "verse" are gone.
struct Phrases {
    /// The whole sentence: "Verset suivant." / "Siguiente."
    alone: Regex,
    /// An instruction anywhere: "passons au verset suivant", "vamos al versículo anterior".
    instruction: Regex,
    /// "En arrière." / "Volvamos atrás."
    back: Regex,
    /// "Allons au verset 6", "Leamos los versículos 6 al 8".
    verse_number: Regex,
    /// "Allons au 24." on its own.
    bare_number: Regex,
    /// After a verse number these name another chapter: "lisons le verset 6 de Jean 3".
    elsewhere: &'static [&'static str],
}

static FRENCH: LazyLock<Phrases> = LazyLock::new(|| {
    let filler = r"(?:(?:alors|donc|et|maintenant|bien|bon|ok|okay|daccord|oui|eglise|freres|soeurs|sil vous plait)\s+)*";
    let verb = r"(?:allons|allez|passons|passez|lisons|lisez|retournons|revenons|revenez|retournez|voyons|regardons|montrez nous|montre nous|affichez|affiche|va|aller|passer|lire)";
    let which = r"(?:verse\s+(?P<w1>suivant|precedent|prochain)|(?P<w2>suivant|prochain|precedent|1|dernier)\s+verse)";
    let please = r"(?:\s+(?:sil vous plait|svp))?";
    Phrases {
        alone: Regex::new(&format!(r"^{filler}(?:(?:le|au)\s+)?(?:{which}|(?P<w3>suivant|precedent)){please}$")).unwrap(),
        instruction: Regex::new(&format!(r"\b{verb}(?:\s+(?:au|a|le|aux|les))?\s+{which}\b")).unwrap(),
        back: Regex::new(&format!(r"^{filler}(?:(?:retour|revenons|retournons|reculons|revenez|on revient|on retourne)\s+)?en arriere(?:\s+dun verse)?{please}$")).unwrap(),
        verse_number: Regex::new(&format!(r"\b{verb}(?:\s+(?:au|aux|a|le|les))?\s+verses?(?:\s+numero)?\s+(?P<s>\d{{1,3}})(?:\s+(?:to|and)\s+(?:verses?\s+)?(?P<e>\d{{1,3}}))?(?P<rest>.*)$")).unwrap(),
        bare_number: Regex::new(&format!(r"^{filler}(?:allons|allez|passons|va)\s+(?:au|a)\s+(?P<s>\d{{1,3}})(?:\s+to\s+(?P<e>\d{{1,3}}))?{please}$")).unwrap(),
        elsewhere: &["de ", "du ", "dans ", "en "],
    }
});

static SPANISH: LazyLock<Phrases> = LazyLock::new(|| {
    let filler = r"(?:(?:bueno|bien|entonces|ahora|y|ok|okay|vale|si|hermanos|iglesia|pues|por favor)\s+)*";
    let verb = r"(?:vamos|vayamos|pasemos|pasamos|pasa|pase|leamos|lean|lee|regresemos|regresamos|volvamos|volvemos|veamos|miremos|muestranos|ir|pasar|leer)";
    let which = r"(?:verse\s+(?P<w1>siguiente|anterior|proximo)|(?P<w2>siguiente|proximo|anterior|1|ultimo)\s+verse)";
    let please = r"(?:\s+por favor)?";
    Phrases {
        alone: Regex::new(&format!(r"^{filler}(?:(?:el|al)\s+)?(?:{which}|(?P<w3>siguiente|anterior)){please}$")).unwrap(),
        instruction: Regex::new(&format!(r"\b{verb}(?:\s+(?:al|a|el|los))?\s+{which}\b")).unwrap(),
        back: Regex::new(&format!(r"^{filler}(?:(?:volvamos|regresemos|vamos|regresa|vuelve)\s+)?(?:hacia\s+)?atras(?:\s+un verse)?{please}$|^{filler}regresa{please}$")).unwrap(),
        verse_number: Regex::new(&format!(r"\b{verb}(?:\s+(?:al|a|el|los))?\s+verses?(?:\s+numero)?\s+(?P<s>\d{{1,3}})(?:\s+(?:to|and)\s+(?:verses?\s+)?(?P<e>\d{{1,3}}))?(?P<rest>.*)$")).unwrap(),
        bare_number: Regex::new(&format!(r"^{filler}(?:vamos|pasemos|ve)\s+(?:al|a)\s+(?P<s>\d{{1,3}})(?:\s+to\s+(?P<e>\d{{1,3}}))?{please}$")).unwrap(),
        elsewhere: &["de ", "del ", "en "],
    }
});

impl Phrases {
    fn of(lang: Lang) -> Option<&'static Phrases> {
        match lang {
            Lang::Fr => Some(&FRENCH),
            Lang::Es => Some(&SPANISH),
            Lang::En => None,
        }
    }

    fn parse(&self, text: &str, lang: Lang) -> Option<Command> {
        let t = numbers::normalize_in(text, &[lang]);
        if t.is_empty() || t.split(' ').count() > 24 {
            return None;
        }
        if let Some(c) = self.verse_number.captures(&t) {
            let rest = c.name("rest").map_or("", |m| m.as_str()).trim_start();
            if !self.elsewhere.iter().any(|w| rest.starts_with(w)) {
                return verse(&c["s"], c.name("e").map(|m| m.as_str()));
            }
        }
        if let Some(c) = self.bare_number.captures(&t) {
            return verse(&c["s"], c.name("e").map(|m| m.as_str()));
        }
        let which = |c: regex::Captures| {
            let w = ["w1", "w2", "w3"].iter().find_map(|n| c.name(n)).map(|m| m.as_str()).unwrap_or("");
            match w {
                "suivant" | "prochain" | "siguiente" | "proximo" => Command::Next,
                "precedent" | "anterior" => Command::Previous,
                "1" => Command::First,
                _ => Command::Final, // "dernier", "ultimo"
            }
        };
        if let Some(c) = self.alone.captures(&t).or_else(|| self.instruction.captures(&t)) {
            return Some(which(c));
        }
        self.back.is_match(&t).then_some(Command::Previous)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_said_as_commands() {
        for (said, want) in [
            ("Next.", Command::Next),
            ("Next verse.", Command::Next),
            ("Next versse", Command::Next),
            ("Next verse, please.", Command::Next),
            ("Okay, next one.", Command::Next),
            ("Go to the next verse.", Command::Next),
            ("Let's go to the next verse.", Command::Next),
            ("Let us move on to the following verse.", Command::Next),
            ("Can we read the next verse?", Command::Next),
            ("Read verse 6.", Command::Verse(6, None)),
            ("Go forward.", Command::Next),
            ("Previous verse.", Command::Previous),
            ("Last verse.", Command::Previous),
            ("Go to the previous verse.", Command::Previous),
            ("Take us back to the previous verse.", Command::Previous),
            ("Go back.", Command::Previous),
            ("Go back one verse.", Command::Previous),
            ("Let's go back.", Command::Previous),
            ("Go to the first verse.", Command::First),
            ("First verse.", Command::First),
            ("Let's go to the final verse.", Command::Final),
            ("Go to verse 6.", Command::Verse(6, None)),
            ("Go to verse six.", Command::Verse(6, None)),
            ("Let's go to verse number twelve.", Command::Verse(12, None)),
            ("Let's read verses 6 to 8.", Command::Verse(6, Some(8))),
            ("Jump to verse 10 2 12.", Command::Verse(10, Some(12))),
            ("Go to the 10th verse.", Command::Verse(10, None)),
            ("Go to the tenth verse.", Command::Verse(10, None)),
            ("Let's go to the twenty first verse.", Command::Verse(21, None)),
            ("The 21st verse.", Command::Verse(21, None)),
            ("The tenth verse.", Command::Verse(10, None)),
            ("Go to 24.", Command::Verse(24, None)),
            ("Okay, go to twenty four, please.", Command::Verse(24, None)),
            ("Go to verse 1.", Command::First),
        ] {
            assert_eq!(parse(said), Some(want), "{said}");
        }
    }

    #[test]
    fn everyday_speech_is_not_a_command() {
        for said in [
            "In the next verse, Paul says we are more than conquerors.",
            "Let's go back to Genesis chapter 3.",
            "We'll be back next week.",
            "Next Sunday is our harvest service.",
            "The first time I heard this verse I wept.",
            "He went back to his father's house.",
            "What is next for you this year?",
            "The last verse of this chapter is my favourite.",
            "I want to go back to what I said earlier.",
            "Go to 1 Corinthians 13.",
            "Read verse 6 of John 3.",
            "We read verse 6 last week.",
            "I went to 24 churches this year.",
            "The tenth time he asked, she said yes.",
        ] {
            assert_eq!(parse(said), None, "{said}");
        }
    }

    #[test]
    fn commands_split_by_a_pause() {
        assert_eq!(parse("Go to verse."), None);
        assert_eq!(parse_joined("Go to verse.", "6."), Some(Command::Verse(6, None)));
        assert_eq!(parse_joined("Let's go to the", "tenth verse."), Some(Command::Verse(10, None)));
        assert_eq!(parse_joined("God is good.", "Next."), None, "only verse numbers complete across sentences");
    }

    #[test]
    fn french_commands() {
        let fr = |s: &str| parse_in(s, &[Lang::Fr]);
        for (said, want) in [
            ("Verset suivant.", Command::Next),
            ("Suivant.", Command::Next),
            ("Le verset suivant, s'il vous plaît.", Command::Next),
            ("Passons au verset suivant.", Command::Next),
            ("Lisons le verset suivant.", Command::Next),
            ("Allons au prochain verset.", Command::Next),
            ("Verset précédent.", Command::Previous),
            ("Revenons au verset précédent.", Command::Previous),
            ("En arrière.", Command::Previous),
            ("Retour en arrière.", Command::Previous),
            ("Revenons en arrière, s'il vous plaît.", Command::Previous),
            ("Allons au premier verset.", Command::First),
            ("Lisons le dernier verset.", Command::Final),
            ("Allons au verset 6.", Command::Verse(6, None)),
            ("Allez au verset six.", Command::Verse(6, None)),
            ("Lisons les versets 6 à 8.", Command::Verse(6, Some(8))),
            ("Passons au verset vingt-quatre.", Command::Verse(24, None)),
            ("Allons au 24.", Command::Verse(24, None)),
        ] {
            assert_eq!(fr(said), Some(want), "{said}");
        }
        for said in [
            "Dans le verset suivant, Paul dit que nous sommes plus que vainqueurs.",
            "Revenons à Genèse chapitre 3.",
            "La semaine prochaine, nous serons de retour.",
            "Lisons le verset 6 de Jean 3.",
            "Au verset 6, Paul parle de la foi.",
            "Le dernier verset de ce chapitre est mon préféré.",
            "Il est revenu en arrière dans sa vie.",
        ] {
            assert_eq!(fr(said), None, "{said}");
        }
        assert_eq!(parse_joined_in("Allons au verset.", "6.", &[Lang::Fr]), Some(Command::Verse(6, None)));
    }

    #[test]
    fn spanish_commands() {
        let es = |s: &str| parse_in(s, &[Lang::Es]);
        for (said, want) in [
            ("Versículo siguiente.", Command::Next),
            ("Siguiente versículo.", Command::Next),
            ("Siguiente.", Command::Next),
            ("Vamos al versículo siguiente.", Command::Next),
            ("Pasemos al siguiente versículo.", Command::Next),
            ("Versículo anterior.", Command::Previous),
            ("Regresemos al versículo anterior.", Command::Previous),
            ("Volvamos atrás.", Command::Previous),
            ("Atrás, por favor.", Command::Previous),
            ("Vamos al primer versículo.", Command::First),
            ("Leamos el último versículo.", Command::Final),
            ("Vamos al versículo 6.", Command::Verse(6, None)),
            ("Vamos al versículo seis.", Command::Verse(6, None)),
            ("Leamos los versículos 6 al 8.", Command::Verse(6, Some(8))),
            ("Vamos al 24.", Command::Verse(24, None)),
        ] {
            assert_eq!(es(said), Some(want), "{said}");
        }
        for said in [
            "En el siguiente versículo Pablo dice que somos más que vencedores.",
            "Volvamos a Génesis capítulo 3.",
            "La próxima semana tenemos culto de jóvenes.",
            "Leamos el versículo 6 de Juan 3.",
            "El último versículo de este capítulo es mi favorito.",
            "Él miró atrás y vio a su padre.",
        ] {
            assert_eq!(es(said), None, "{said}");
        }
    }

    #[test]
    fn languages_together() {
        let both = [Lang::En, Lang::Fr];
        assert_eq!(parse_in("Next verse.", &both), Some(Command::Next));
        assert_eq!(parse_in("Verset suivant.", &both), Some(Command::Next));
        assert_eq!(parse_in("Verset suivant.", &[Lang::En]), None);
    }
}
