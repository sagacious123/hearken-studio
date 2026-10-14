//! Spoken commands for the live scripture: "next verse", "go back", "go to the first verse".
//!
//! Preachers say "next" and "go back" all the time, so a phrase only counts when it's clearly
//! a command: said on its own ("Next verse." / "Go back.") or as an instruction ("let's go to
//! the next verse", "take us back to the previous verse"). Narration ("in the next verse Paul
//! says…"), places ("let's go back to Genesis") and time ("next week") are left alone.

use std::sync::LazyLock;

use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Next,
    Previous,
    /// Verse 1 of the live chapter.
    First,
    /// The chapter's final verse.
    Final,
}

impl Command {
    pub fn label(self) -> &'static str {
        match self {
            Command::Next => "Next verse",
            Command::Previous => "Previous verse",
            Command::First => "First verse",
            Command::Final => "Last verse of the chapter",
        }
    }
}

/// Lowercase words only; common mishearings of "verse" folded in.
fn normalize(text: &str) -> String {
    let words: Vec<String> = text
        .to_lowercase()
        .replace(['’', '\''], "")
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| match w {
            "versse" | "vers" | "verses" | "vs" | "vous" | "vers3" => "verse".to_owned(),
            "nex" | "necks" | "nexts" => "next".to_owned(),
            "previews" | "previos" => "previous".to_owned(),
            "1st" => "first".to_owned(),
            w => w.to_owned(),
        })
        .collect();
    words.join(" ")
}

const WHICH: &str = r"(?P<which>next|following|previous|prior|preceding|last|first|final|opening)";
const ITEM: &str = r"(?:verse|scripture|one|slide|passage)";
const FILLER: &str = r"(?:(?:okay|ok|alright|all right|so|and|now|then|yes|right|please|church|brethren|lets|let us|can we|could we|shall we|well)\s+)*";

/// The whole sentence is the command: "Next.", "Next verse please.", "The previous one."
static ALONE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"^{FILLER}(?:the\s+)?{WHICH}(?:\s+{ITEM})?(?:\s+please)?$")).unwrap());

/// An instruction anywhere in the sentence: "let's go to the next verse", "read the next verse",
/// "take us back to the previous verse", "move on to the following verse".
static INSTRUCTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"\b(?:go|move|turn|skip|jump|take us|take me|bring us|bring up|put up|show us|show|give us|read|look at|let us see|lets see)(?:\s+(?:on|back|ahead|forward|over|down|up))?(?:\s+(?:to|into|unto))?(?:\s+(?:the|our))?\s+{WHICH}\s+{ITEM}\b"
    ))
    .unwrap()
});

/// "Go back." / "Go back one verse." / "Back one." — only at the end of the sentence, so
/// "let's go back to Genesis 3" stays a reference.
static BACK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?:^{FILLER}|\b(?:lets|let us)\s+)(?:go\s+|move\s+|step\s+)?back(?:\s+(?:one|a verse|one verse|a bit))?(?:\s+please)?$")).unwrap()
});

/// "Go forward." / "Move on." on its own.
static FORWARD: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"^{FILLER}(?:go|move|step)\s+(?:forward|on|ahead)(?:\s+(?:one|a verse|one verse))?(?:\s+please)?$")).unwrap());

fn which(word: &str) -> Command {
    match word {
        "next" | "following" => Command::Next,
        "previous" | "prior" | "preceding" | "last" => Command::Previous,
        "first" | "opening" => Command::First,
        _ => Command::Final, // "final"
    }
}

/// The command in a finished sentence, if it contains one.
pub fn parse(text: &str) -> Option<Command> {
    let t = normalize(text);
    if t.is_empty() || t.split(' ').count() > 24 {
        return None;
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
        ] {
            assert_eq!(parse(said), None, "{said}");
        }
    }
}
