//! Normalises ASR text so a single regex can match references however Whisper writes them:
//! "John 3:16", "John 3.16", "john chapter three verse sixteen", "First Corinthians 13, 4 to 7".

#[derive(Clone, Copy, PartialEq, Eq)]
enum NumWord {
    Unit(u32), // 0..=19
    Ten(u32),  // 20, 30, … 90
    Hundred,
}

fn num_word(w: &str) -> Option<NumWord> {
    use NumWord::*;
    Some(match w {
        "zero" => Unit(0),
        "one" => Unit(1),
        "two" => Unit(2),
        "three" => Unit(3),
        "four" => Unit(4),
        "five" => Unit(5),
        "six" => Unit(6),
        "seven" => Unit(7),
        "eight" => Unit(8),
        "nine" => Unit(9),
        "ten" => Unit(10),
        "eleven" => Unit(11),
        "twelve" => Unit(12),
        "thirteen" => Unit(13),
        "fourteen" => Unit(14),
        "fifteen" => Unit(15),
        "sixteen" => Unit(16),
        "seventeen" => Unit(17),
        "eighteen" => Unit(18),
        "nineteen" => Unit(19),
        "twenty" => Ten(20),
        "thirty" => Ten(30),
        "forty" | "fourty" => Ten(40),
        "fifty" => Ten(50),
        "sixty" => Ten(60),
        "seventy" => Ten(70),
        "eighty" => Ten(80),
        "ninety" => Ten(90),
        "hundred" => Hundred,
        _ => return None,
    })
}

/// Lowercase, strip punctuation (keeping chapter/verse separators), and convert spoken
/// numbers to digits. Adjacent numbers stay separate: "three sixteen" → "3 16".
pub fn normalize(input: &str) -> String {
    let lower = input.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    let mut s = String::with_capacity(lower.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        let prev_digit = i > 0 && chars[i - 1].is_ascii_digit();
        let next_digit = chars.get(i + 1).is_some_and(|c| c.is_ascii_digit());
        match c {
            c if c.is_alphanumeric() => s.push(c),
            ':' if prev_digit || next_digit => s.push(':'),
            '.' if prev_digit && next_digit => s.push(':'), // "3.16"
            '-' | '–' | '—' => s.push_str(" - "),
            '\'' | '’' => {}
            _ => s.push(' '),
        }
    }

    let tokens: Vec<&str> = s.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        // "a hundred and nineteen"
        if tokens[i] == "a" && tokens.get(i + 1) == Some(&"hundred") {
            i += 1;
            continue;
        }
        if num_word(tokens[i]).is_none() {
            out.push(tokens[i].to_string());
            i += 1;
            continue;
        }

        let mut value = 0u32;
        let mut last: Option<NumWord> = None;
        let mut j = i;
        while j < tokens.len() {
            match num_word(tokens[j]) {
                Some(NumWord::Unit(u)) => {
                    // "three sixteen" is two numbers; "twenty three" is one; "twenty twelve" is two.
                    match last {
                        Some(NumWord::Unit(_)) => break,
                        Some(NumWord::Ten(_)) if u >= 10 => break,
                        _ => {}
                    }
                    value += u;
                    last = Some(NumWord::Unit(u));
                }
                Some(NumWord::Ten(t)) => {
                    if matches!(last, Some(NumWord::Unit(_)) | Some(NumWord::Ten(_))) {
                        break;
                    }
                    value += t;
                    last = Some(NumWord::Ten(t));
                }
                Some(NumWord::Hundred) => {
                    if matches!(last, Some(NumWord::Hundred) | Some(NumWord::Ten(_))) {
                        break;
                    }
                    value = value.max(1) * 100;
                    last = Some(NumWord::Hundred);
                }
                None => {
                    // "one hundred and nineteen"
                    let continues = tokens[j] == "and"
                        && last == Some(NumWord::Hundred)
                        && matches!(tokens.get(j + 1).and_then(|w| num_word(w)), Some(NumWord::Unit(_) | NumWord::Ten(_)));
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
    out.join(" ")
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn spoken_numbers() {
        assert_eq!(normalize("John chapter three verse sixteen"), "john chapter 3 verse 16");
        assert_eq!(normalize("John three sixteen"), "john 3 16");
        assert_eq!(normalize("Psalm one hundred and nineteen"), "psalm 119");
        assert_eq!(normalize("Psalm a hundred and twenty one"), "psalm 121");
        assert_eq!(normalize("Proverbs twenty three, verse seven"), "proverbs 23 verse 7");
    }

    #[test]
    fn punctuation_and_separators() {
        assert_eq!(normalize("Turn to John 3:16-18."), "turn to john 3:16 - 18");
        assert_eq!(normalize("Romans 8.28"), "romans 8:28");
        assert_eq!(normalize("God's love, in John 3, 16"), "gods love in john 3 16");
    }
}
