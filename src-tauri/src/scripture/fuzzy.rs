//! Sound-alike word matching shared by the reference parser (misheard book names, "Philippines
//! 4:13") and the quote matcher (misheard verse words, "pray without seizing").

/// Consonant skeleton with common sound merges ("ceasing"/"seizing" → "sng",
/// "perish"/"parish" → "prx").
pub fn sound(word: &str) -> String {
    let w: Vec<u8> = word.bytes().filter(u8::is_ascii_lowercase).collect();
    let mut out: Vec<u8> = Vec::with_capacity(w.len());
    let mut i = 0;
    while i < w.len() {
        let (code, step) = match (w[i], w.get(i + 1).copied()) {
            (b'p', Some(b'h')) => (Some(b'f'), 2),
            (b's' | b'c', Some(b'h')) => (Some(b'x'), 2),
            (b't', Some(b'h')) => (Some(b'0'), 2),
            (b'g', Some(b'h')) => (None, 2),
            (b'c', Some(b'k')) => (Some(b'k'), 2),
            (b'c', Some(b'e' | b'i' | b'y')) => (Some(b's'), 1),
            (b'c' | b'q', _) => (Some(b'k'), 1),
            (b'z', _) => (Some(b's'), 1),
            (b'd', _) => (Some(b't'), 1),
            (b'v', _) => (Some(b'f'), 1),
            (b'a' | b'e' | b'i' | b'o' | b'u' | b'y' | b'w' | b'h', _) => ((i == 0).then_some(b'a'), 1),
            (c, _) => (Some(c), 1),
        };
        if let Some(c) = code {
            if out.last() != Some(&c) {
                out.push(c);
            }
        }
        i += step;
    }
    String::from_utf8(out).unwrap_or_default()
}

/// Levenshtein distance in bytes (words are lowercase and almost always ASCII).
pub fn edits(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            cur[j] = (prev[j - 1] + usize::from(a[i - 1] != b[j - 1])).min(prev[j] + 1).min(cur[j - 1] + 1);
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Whether two different words are close enough to be an ASR confusion: same sound skeleton
/// and at most about half the letters changed, or at most a third changed regardless of sound.
/// Words under four letters never count: they have too many sound-alikes ("good" would match
/// "God" in thousands of verses). Pass each word's `sound` (callers precompute it in loops).
pub fn alike(a: &str, a_sound: &str, b: &str, b_sound: &str) -> bool {
    let (la, lb) = (a.len(), b.len());
    let longest = la.max(lb);
    if la < 4 || lb < 4 || la.abs_diff(lb) * 2 > longest {
        return false;
    }
    let d = edits(a, b);
    (a_sound.len() >= 2 && a_sound == b_sound && d * 2 <= longest + 1) || d * 3 <= longest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alike_words(a: &str, b: &str) -> bool {
        alike(a, &sound(a), b, &sound(b))
    }

    #[test]
    fn asr_confusions() {
        for (heard, said) in [("seizing", "ceasing"), ("parish", "perish"), ("shepard", "shepherd"),
                              ("forgotten", "begotten"), ("whoever", "whosoever"), ("philippines", "philippians"),
                              ("romance", "romans"), ("galations", "galatians"), ("collisions", "colossians")] {
            assert!(alike_words(heard, said), "{heard} / {said}");
        }
        for (heard, said) in [("separate", "shepherd"), ("crisis", "christ"), ("table", "temple"), ("good", "god"),
                              ("about", "amos"), ("times", "titus")] {
            assert!(!alike_words(heard, said), "{heard} / {said}");
        }
    }
}
