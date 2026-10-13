//! The recap as a Word document (.docx): the minimum Office Open XML package, with real
//! paragraph styles (Title, Heading 1/2) so Word's navigation pane and outline work.

use std::io::{Cursor, Write};

use anyhow::Result;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use super::{Block, Document, FOOTER};

/// A4 with 2.5 cm margins, in twentieths of a point.
const TEXT_WIDTH: u32 = 11906 - 2 * 1418;

fn esc(s: &str) -> String {
    s.chars()
        .filter(|&c| matches!(c, '\t' | '\n' | '\r') || c >= ' ')
        .fold(String::with_capacity(s.len()), |mut out, c| {
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                c => out.push(c),
            }
            out
        })
}

fn run(text: &str, props: &str) -> String {
    let rpr = if props.is_empty() { String::new() } else { format!("<w:rPr>{props}</w:rPr>") };
    format!(r#"<w:r>{rpr}<w:t xml:space="preserve">{}</w:t></w:r>"#, esc(text))
}

fn para(style: &str, ppr_extra: &str, runs: &str) -> String {
    format!(r#"<w:p><w:pPr><w:pStyle w:val="{style}"/>{ppr_extra}</w:pPr>{runs}</w:p>"#)
}

fn document_xml(doc: &Document) -> String {
    let mut body = String::new();
    body.push_str(&para("Title", "", &run(&doc.title, "")));
    body.push_str(&para("Subtitle", "", &run(&doc.subtitle, "")));
    for b in &doc.blocks {
        body.push_str(&match b {
            Block::Section(t) => para("Heading1", "", &run(t, "")),
            Block::Heading { text, meta } => {
                let mut runs = run(text, "");
                if let Some(m) = meta {
                    runs.push_str(r#"<w:r><w:tab/></w:r>"#);
                    runs.push_str(&run(m, r#"<w:rStyle w:val="Meta"/>"#));
                }
                para("Heading2", &format!(r#"<w:tabs><w:tab w:val="right" w:pos="{TEXT_WIDTH}"/></w:tabs>"#), &runs)
            }
            Block::Text(t) => para("Normal", "", &run(t, "")),
            Block::Note(t) => para("Note", "", &run(t, "")),
            Block::Bullet(t) => para("Point", "", &format!("{}<w:r><w:tab/></w:r>{}", run("•", ""), run(t, ""))),
        });
    }
    body.push_str(&para("Note", r#"<w:spacing w:before="480"/>"#, &run(FOOTER, "")));
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}<w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1418" w:right="1418" w:bottom="1418" w:left="1418" w:header="709" w:footer="709" w:gutter="0"/></w:sectPr></w:body></w:document>"#
    )
}

const STYLES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Georgia" w:hAnsi="Georgia" w:cs="Georgia" w:eastAsia="Georgia"/><w:sz w:val="23"/><w:szCs w:val="23"/><w:lang w:val="en-GB"/></w:rPr></w:rPrDefault>
<w:pPrDefault><w:pPr><w:spacing w:after="80" w:line="300" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>
<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:qFormat/></w:style>
<w:style w:type="paragraph" w:styleId="Title"><w:name w:val="Title"/><w:basedOn w:val="Normal"/><w:next w:val="Subtitle"/><w:qFormat/><w:pPr><w:spacing w:after="40"/></w:pPr><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri"/><w:b/><w:sz w:val="48"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Subtitle"><w:name w:val="Subtitle"/><w:basedOn w:val="Normal"/><w:qFormat/><w:pPr><w:spacing w:after="360"/></w:pPr><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri"/><w:color w:val="78716C"/><w:sz w:val="24"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:pBdr><w:top w:val="single" w:sz="8" w:space="6" w:color="E7E5E4"/></w:pBdr><w:spacing w:before="480" w:after="60"/><w:outlineLvl w:val="0"/></w:pPr><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri"/><w:b/><w:caps/><w:color w:val="78716C"/><w:spacing w:val="20"/><w:sz w:val="20"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Heading2"><w:name w:val="heading 2"/><w:basedOn w:val="Normal"/><w:next w:val="Normal"/><w:qFormat/><w:pPr><w:keepNext/><w:spacing w:before="240" w:after="40"/><w:outlineLvl w:val="1"/></w:pPr><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri"/><w:b/><w:color w:val="B45309"/><w:sz w:val="27"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Note"><w:name w:val="Note"/><w:basedOn w:val="Normal"/><w:rPr><w:rFonts w:ascii="Calibri" w:hAnsi="Calibri"/><w:color w:val="78716C"/><w:sz w:val="20"/></w:rPr></w:style>
<w:style w:type="paragraph" w:styleId="Point"><w:name w:val="Point"/><w:basedOn w:val="Normal"/><w:pPr><w:tabs><w:tab w:val="left" w:pos="360"/></w:tabs><w:ind w:left="360" w:hanging="360"/></w:pPr></w:style>
<w:style w:type="character" w:styleId="Meta"><w:name w:val="Meta"/><w:rPr><w:b w:val="0"/><w:color w:val="78716C"/><w:sz w:val="19"/></w:rPr></w:style>
</w:styles>"#;

const CONTENT_TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#;

const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/></Relationships>"#;

const DOCUMENT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#;

fn core_xml(doc: &Document) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{} — {}</dc:title><dc:creator>Hearken Studio</dc:creator></cp:coreProperties>"#,
        esc(&doc.title),
        esc(&doc.subtitle)
    )
}

pub fn render(doc: &Document) -> Result<Vec<u8>> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let parts: [(&str, String); 6] = [
        ("[Content_Types].xml", CONTENT_TYPES.into()),
        ("_rels/.rels", RELS.into()),
        ("docProps/core.xml", core_xml(doc)),
        ("word/_rels/document.xml.rels", DOCUMENT_RELS.into()),
        ("word/styles.xml", STYLES.into()),
        ("word/document.xml", document_xml(doc)),
    ];
    for (name, body) in parts {
        zip.start_file(name, opts)?;
        zip.write_all(body.as_bytes())?;
    }
    Ok(zip.finish()?.into_inner())
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    #[test]
    fn package_holds_escaped_text_and_styles() {
        let bytes = render(&super::super::tests::sample()).unwrap();
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut xml = String::new();
        zip.by_name("word/document.xml").unwrap().read_to_string(&mut xml).unwrap();
        assert!(xml.contains("&lt;has&gt; no end &amp; no limit"));
        assert!(xml.contains("Ìfẹ́ Ọlọ́run"));
        assert!(xml.contains(r#"<w:pStyle w:val="Heading2"/>"#));
        assert!(zip.by_name("word/styles.xml").is_ok() && zip.by_name("[Content_Types].xml").is_ok());
    }
}
