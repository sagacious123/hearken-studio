//! The recap as Rich Text, offered as a .doc file: Word 97 onwards, Pages, LibreOffice, Google
//! Docs and WordPad all open it. Non-ASCII letters are written as \u escapes, so any
//! translation's text survives.

use super::{Block, Document, FOOTER};

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\n' => out.push_str("\\line "),
            '\t' => out.push_str("\\tab "),
            c if (c as u32) < 0x20 => {}
            c if c.is_ascii() => out.push(c),
            c => {
                // RTF counts in UTF-16 units, as signed 16-bit numbers; "?" is the fallback
                // shown by readers that don't understand \u.
                for unit in c.encode_utf16(&mut [0; 2]) {
                    out.push_str(&format!("\\u{}?", *unit as i16));
                }
            }
        }
    }
    out
}

/// A4, 2.5 cm margins: text width in twips (for the right-aligned tab on headings).
const TEXT_WIDTH: u32 = 11906 - 2 * 1418;

pub fn render(doc: &Document) -> String {
    // Fonts: 0 serif body, 1 sans headings. Colours: 1 accent, 2 muted, 3 rule.
    let mut out = String::from(
        "{\\rtf1\\ansi\\ansicpg1252\\deff0\\uc1\n{\\fonttbl{\\f0\\froman\\fcharset0 Georgia;}{\\f1\\fswiss\\fcharset0 Calibri;}}\n\
         {\\colortbl;\\red180\\green83\\blue9;\\red120\\green113\\blue108;\\red231\\green229\\blue228;}\n\
         \\paperw11906\\paperh16838\\margl1418\\margr1418\\margt1418\\margb1418\\widowctrl\n",
    );
    out.push_str(&format!("\\pard\\sa40\\f1\\fs48\\b {}\\b0\\par\n", esc(&doc.title)));
    out.push_str(&format!("\\pard\\sa360\\f1\\fs24\\cf2 {}\\cf0\\par\n", esc(&doc.subtitle)));
    for b in &doc.blocks {
        match b {
            Block::Section(t) => out.push_str(&format!(
                "\\pard\\keepn\\sb480\\sa60\\brdrt\\brdrs\\brdrw15\\brsp120\\brdrcf3\\f1\\fs20\\b\\caps\\cf2 {}\\caps0\\b0\\cf0\\par\n",
                esc(t)
            )),
            Block::Heading { text, meta } => {
                out.push_str(&format!("\\pard\\keepn\\sb240\\sa40\\tqr\\tx{TEXT_WIDTH}\\f1\\fs27\\b\\cf1 {}\\b0", esc(text)));
                if let Some(m) = meta {
                    out.push_str(&format!("\\tab\\fs19\\cf2 {}", esc(m)));
                }
                out.push_str("\\cf0\\par\n");
            }
            Block::Text(t) => out.push_str(&format!("\\pard\\sa80\\sl300\\slmult1\\f0\\fs23 {}\\par\n", esc(t))),
            Block::Note(t) => out.push_str(&format!("\\pard\\sa80\\f1\\fs20\\cf2 {}\\cf0\\par\n", esc(t))),
            Block::Bullet(t) => out.push_str(&format!("\\pard\\sa80\\sl300\\slmult1\\fi-360\\li360\\tx360\\f0\\fs23 \\bullet\\tab {}\\par\n", esc(t))),
        }
    }
    out.push_str(&format!("\\pard\\sb480\\f1\\fs18\\cf2 {}\\cf0\\par\n}}", esc(FOOTER)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_braces_and_unicode() {
        assert_eq!(esc("a{b}\\c"), "a\\{b\\}\\\\c");
        assert_eq!(esc("ẹ"), "\\u7865?");
        assert_eq!(esc("😀"), "\\u-10179?\\u-8704?"); // surrogate pair, as signed numbers
        let rtf = render(&super::super::tests::sample());
        assert!(rtf.starts_with("{\\rtf1") && rtf.ends_with('}'));
        assert_eq!(rtf.matches('{').count() - rtf.matches("\\{").count(), rtf.matches('}').count() - rtf.matches("\\}").count());
    }
}
