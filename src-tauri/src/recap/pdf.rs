//! The recap as a PDF: A4 pages laid out here (word wrap, page breaks, page numbers), with Noto
//! Serif embedded and subsetted to the letters used. PDF's built-in fonts only know Western
//! European letters; Noto has the ones Yoruba, Igbo, Hausa and most Latin-script Bibles need.

use std::{collections::HashMap, io::Write};

use anyhow::{anyhow, Result};
use flate2::{write::ZlibEncoder, Compression};
use pdf_writer::{
    types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap},
    Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr,
};
use subsetter::GlyphRemapper;
use ttf_parser::{Face, GlyphId};

use super::{Block, Document, FOOTER};

const REGULAR: &[u8] = include_bytes!("../../assets/fonts/NotoSerif-Regular.ttf");
const BOLD: &[u8] = include_bytes!("../../assets/fonts/NotoSerif-Bold.ttf");
const ITALIC: &[u8] = include_bytes!("../../assets/fonts/NotoSerif-Italic.ttf");

// A4 in points, with 2.5 cm margins.
const PAGE_W: f32 = 595.28;
const PAGE_H: f32 = 841.89;
const MARGIN: f32 = 71.0;
const TEXT_W: f32 = PAGE_W - 2.0 * MARGIN;
const BOTTOM: f32 = MARGIN;

type Rgb = (f32, f32, f32);
const INK: Rgb = (0.11, 0.098, 0.09);
const MUTED: Rgb = (0.47, 0.443, 0.424);
const ACCENT: Rgb = (0.706, 0.325, 0.035);
const RULE: Rgb = (0.906, 0.898, 0.894);

const SYSTEM_INFO: SystemInfo = SystemInfo { registry: Str(b"Adobe"), ordering: Str(b"Identity"), supplement: 0 };

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Regular = 0,
    Bold = 1,
    Italic = 2,
}

struct Font {
    data: &'static [u8],
    face: Face<'static>,
    /// PostScript name with the subset tag PDF expects ("HKNREG+NotoSerif-Regular").
    name: String,
    italic: bool,
    remap: GlyphRemapper,
    /// New glyph id → the text it shows, so text copied out of the PDF is right.
    chars: HashMap<u16, char>,
}

impl Font {
    fn load(data: &'static [u8], tag: &str, italic: bool) -> Result<Self> {
        let face = Face::parse(data, 0).map_err(|e| anyhow!("bad font: {e}"))?;
        let ps = face
            .names()
            .into_iter()
            .find(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME)
            .and_then(|n| n.to_string())
            .unwrap_or_else(|| "NotoSerif".into());
        Ok(Self { data, face, name: format!("{tag}+{ps}"), italic, remap: GlyphRemapper::new(), chars: HashMap::new() })
    }

    fn scale(&self, size: f32) -> f32 {
        size / f32::from(self.face.units_per_em())
    }

    fn glyph(&self, c: char) -> u16 {
        self.face.glyph_index(c).map_or(0, |g| g.0)
    }

    fn width(&self, text: &str, size: f32) -> f32 {
        text.chars().map(|c| f32::from(self.face.glyph_hor_advance(GlyphId(self.glyph(c))).unwrap_or(0))).sum::<f32>() * self.scale(size)
    }

    /// The text as two-byte glyph ids in the subset (the font is written with Identity-H).
    fn encode(&mut self, text: &str) -> Vec<u8> {
        let mut out = Vec::with_capacity(text.len() * 2);
        for c in text.chars() {
            let new = self.remap.remap(self.glyph(c));
            self.chars.entry(new).or_insert(c);
            out.extend_from_slice(&new.to_be_bytes());
        }
        out
    }
}

struct Layout {
    fonts: [Font; 3],
    pages: Vec<Content>,
    y: f32,
}

impl Layout {
    fn page(&mut self) -> &mut Content {
        self.pages.last_mut().expect("a page")
    }

    fn new_page(&mut self) {
        self.pages.push(Content::new());
        self.y = PAGE_H - MARGIN;
    }

    /// Start a new page unless `height` more points fit on this one.
    fn ensure(&mut self, height: f32) {
        if self.y - height < BOTTOM {
            self.new_page();
        }
    }

    fn draw(&mut self, style: Style, size: f32, color: Rgb, x: f32, y: f32, text: &str) {
        let bytes = self.fonts[style as usize].encode(text);
        let page = self.page();
        page.begin_text();
        page.set_font(Name(FONT_NAMES[style as usize]), size);
        page.set_fill_rgb(color.0, color.1, color.2);
        page.set_text_matrix([1.0, 0.0, 0.0, 1.0, x, y]);
        page.show(Str(&bytes));
        page.end_text();
    }

    fn rule(&mut self, y: f32, width: f32, color: Rgb) {
        let page = self.page();
        page.set_stroke_rgb(color.0, color.1, color.2);
        page.set_line_width(width);
        page.move_to(MARGIN, y);
        page.line_to(PAGE_W - MARGIN, y);
        page.stroke();
    }

    /// Greedy word wrap to `width`; words longer than a line are broken between letters.
    fn wrap(&self, style: Style, size: f32, width: f32, text: &str) -> Vec<String> {
        let font = &self.fonts[style as usize];
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in text.split_whitespace() {
            let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if font.width(&candidate, size) <= width {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            if font.width(word, size) <= width {
                line = word.to_owned();
            } else {
                for c in word.chars() {
                    if !line.is_empty() && font.width(&format!("{line}{c}"), size) > width {
                        lines.push(std::mem::take(&mut line));
                    }
                    line.push(c);
                }
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }

    /// A wrapped paragraph; its first `keep_lines` lines start on the same page.
    #[allow(clippy::too_many_arguments)]
    fn paragraph(&mut self, style: Style, size: f32, leading: f32, color: Rgb, indent: f32, text: &str, keep_lines: usize) {
        let lines = self.wrap(style, size, TEXT_W - indent, text);
        self.ensure(leading * keep_lines.min(lines.len()).max(1) as f32);
        for line in lines {
            self.ensure(leading);
            self.y -= leading;
            self.draw(style, size, color, MARGIN + indent, self.y + leading * 0.25, &line);
        }
    }
}

const FONT_NAMES: [&[u8]; 3] = [b"F0", b"F1", b"F2"];

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(data).expect("in-memory write");
    z.finish().expect("in-memory write")
}

pub fn render(doc: &Document) -> Result<Vec<u8>> {
    let fonts = [Font::load(REGULAR, "HKNREG", false)?, Font::load(BOLD, "HKNBLD", false)?, Font::load(ITALIC, "HKNITA", true)?];
    let mut l = Layout { fonts, pages: Vec::new(), y: 0.0 };
    l.new_page();

    // Title block.
    l.paragraph(Style::Bold, 24.0, 30.0, INK, 0.0, &doc.title, 1);
    l.y -= 2.0;
    l.paragraph(Style::Regular, 11.5, 16.0, MUTED, 0.0, &doc.subtitle, 1);
    l.y -= 10.0;

    for (i, b) in doc.blocks.iter().enumerate() {
        match b {
            Block::Section(t) => {
                l.ensure(70.0); // with room for what follows, so a section title never ends a page
                l.y -= 22.0;
                l.rule(l.y, 1.2, RULE);
                l.y -= 6.0;
                l.paragraph(Style::Bold, 9.5, 14.0, MUTED, 0.0, &t.to_uppercase(), 1);
            }
            Block::Heading { text, meta } => {
                // Keep the heading with the start of what's under it.
                let next_lines = match doc.blocks.get(i + 1) {
                    Some(Block::Text(_) | Block::Bullet(_)) => 2.0,
                    _ => 0.0,
                };
                l.ensure(22.0 + 16.0 * next_lines);
                l.y -= 10.0;
                let meta_w = meta.as_deref().map_or(0.0, |m| l.fonts[Style::Regular as usize].width(m, 9.5) + 12.0);
                let lines = l.wrap(Style::Bold, 13.0, TEXT_W - meta_w, text);
                for (n, line) in lines.iter().enumerate() {
                    l.y -= 18.0;
                    l.draw(Style::Bold, 13.0, ACCENT, MARGIN, l.y + 4.0, line);
                    if let (0, Some(m)) = (n, meta) {
                        let w = l.fonts[Style::Regular as usize].width(m, 9.5);
                        l.draw(Style::Regular, 9.5, MUTED, PAGE_W - MARGIN - w, l.y + 4.0, m);
                    }
                }
            }
            Block::Text(t) => l.paragraph(Style::Regular, 11.5, 16.5, INK, 0.0, t, 2),
            Block::Note(t) => {
                l.y -= 2.0;
                l.paragraph(Style::Italic, 10.0, 14.5, MUTED, 0.0, t, 1);
            }
            Block::Bullet(t) => {
                l.y -= 3.0;
                let lines = l.wrap(Style::Regular, 11.5, TEXT_W - 16.0, t);
                l.ensure(16.5 * lines.len().min(2) as f32);
                for (n, line) in lines.iter().enumerate() {
                    l.ensure(16.5);
                    l.y -= 16.5;
                    if n == 0 {
                        l.draw(Style::Regular, 11.5, ACCENT, MARGIN + 2.0, l.y + 4.0, "•");
                    }
                    l.draw(Style::Regular, 11.5, INK, MARGIN + 16.0, l.y + 4.0, line);
                }
            }
        }
    }

    // Footers, now that the page count is known.
    let total = l.pages.len();
    for n in 0..total {
        let footer = format!("{FOOTER}  ·  {} / {total}", n + 1);
        let w = l.fonts[0].width(&footer, 8.5);
        let bytes = l.fonts[0].encode(&footer);
        let page = &mut l.pages[n];
        page.begin_text();
        page.set_font(Name(FONT_NAMES[0]), 8.5);
        page.set_fill_rgb(MUTED.0, MUTED.1, MUTED.2);
        page.set_text_matrix([1.0, 0.0, 0.0, 1.0, (PAGE_W - w) / 2.0, MARGIN / 2.0]);
        page.show(Str(&bytes));
        page.end_text();
    }

    write_pdf(doc, l)
}

fn write_pdf(doc: &Document, l: Layout) -> Result<Vec<u8>> {
    let mut pdf = Pdf::new();
    let mut next = 1;
    let mut alloc = || {
        let r = Ref::new(next);
        next += 1;
        r
    };
    let catalog = alloc();
    let tree = alloc();
    let info = alloc();
    // Per font: Type0, CIDFont, descriptor, font file, ToUnicode map.
    let font_refs: Vec<[Ref; 5]> = (0..3).map(|_| [alloc(), alloc(), alloc(), alloc(), alloc()]).collect();
    let page_refs: Vec<(Ref, Ref)> = (0..l.pages.len()).map(|_| (alloc(), alloc())).collect();

    pdf.catalog(catalog).pages(tree);
    pdf.pages(tree).kids(page_refs.iter().map(|p| p.0)).count(page_refs.len() as i32);
    pdf.document_info(info)
        .title(TextStr(&format!("{} — {}", doc.title, doc.subtitle)))
        .creator(TextStr("Hearken Studio"))
        .producer(TextStr("Hearken Studio"));

    for (content, &(page_ref, content_ref)) in l.pages.into_iter().zip(&page_refs) {
        let mut page = pdf.page(page_ref);
        page.media_box(Rect::new(0.0, 0.0, PAGE_W, PAGE_H)).parent(tree).contents(content_ref);
        let mut resources = page.resources();
        let mut fonts = resources.fonts();
        for (i, refs) in font_refs.iter().enumerate() {
            fonts.pair(Name(FONT_NAMES[i]), refs[0]);
        }
        fonts.finish();
        resources.finish();
        page.finish();
        pdf.stream(content_ref, &deflate(&content.finish())).filter(Filter::FlateDecode);
    }

    for (font, [type0, cid, descriptor, file, cmap]) in l.fonts.iter().zip(font_refs) {
        let name = Name(font.name.as_bytes());
        let upem = f32::from(font.face.units_per_em());
        let to_pdf = |v: f32| v * 1000.0 / upem;

        pdf.type0_font(type0).base_font(name).encoding_predefined(Name(b"Identity-H")).descendant_font(cid).to_unicode(cmap);

        let widths: Vec<f32> = font.remap.remapped_gids().map(|old| to_pdf(f32::from(font.face.glyph_hor_advance(GlyphId(old)).unwrap_or(0)))).collect();
        let mut cid_font = pdf.cid_font(cid);
        cid_font
            .subtype(CidFontType::Type2)
            .base_font(name)
            .system_info(SYSTEM_INFO)
            .font_descriptor(descriptor)
            .default_width(0.0)
            .cid_to_gid_map_predefined(Name(b"Identity"));
        cid_font.widths().consecutive(0, widths);
        cid_font.finish();

        let bbox = font.face.global_bounding_box();
        let mut flags = FontFlags::SERIF | FontFlags::NON_SYMBOLIC;
        if font.italic {
            flags |= FontFlags::ITALIC;
        }
        pdf.font_descriptor(descriptor)
            .name(name)
            .flags(flags)
            .bbox(Rect::new(to_pdf(bbox.x_min.into()), to_pdf(bbox.y_min.into()), to_pdf(bbox.x_max.into()), to_pdf(bbox.y_max.into())))
            .italic_angle(font.face.italic_angle())
            .ascent(to_pdf(font.face.ascender().into()))
            .descent(to_pdf(font.face.descender().into()))
            .cap_height(to_pdf(font.face.capital_height().unwrap_or(font.face.ascender()).into()))
            .stem_v(80.0)
            .font_file2(file);

        let subset = subsetter::subset(font.data, 0, &font.remap).map_err(|e| anyhow!("couldn't subset font: {e:?}"))?;
        pdf.stream(file, &deflate(&subset)).filter(Filter::FlateDecode).pair(Name(b"Length1"), subset.len() as i32);

        let mut map = UnicodeCmap::new(Name(b"Custom"), SYSTEM_INFO);
        let mut pairs: Vec<_> = font.chars.iter().collect();
        pairs.sort();
        for (&gid, &c) in pairs {
            map.pair(gid, c);
        }
        pdf.stream(cmap, &map.finish());
    }

    Ok(pdf.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_pdf_with_subsetted_fonts() {
        let bytes = render(&super::super::tests::sample()).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.windows(6).any(|w| w == b"%%EOF\n" || &w[..5] == b"%%EOF"));
        // Subsetting keeps it small: the three fonts are ~1.5 MB whole.
        assert!(bytes.len() < 120_000, "{} bytes", bytes.len());
        if let Ok(dir) = std::env::var("RECAP_SAMPLE_DIR") {
            std::fs::write(format!("{dir}/sample.pdf"), &bytes).unwrap();
        }
    }

    #[test]
    fn long_text_flows_onto_more_pages() {
        let mut doc = super::super::tests::sample();
        for i in 0..80 {
            doc.blocks.push(Block::Heading { text: format!("Point {i}"), meta: Some("10:00".into()) });
            doc.blocks.push(Block::Text("Ẹ jẹ́ kí a gbàdúrà. ".repeat(12)));
        }
        let fonts = [Font::load(REGULAR, "A", false).unwrap(), Font::load(BOLD, "B", false).unwrap(), Font::load(ITALIC, "C", true).unwrap()];
        let l = Layout { fonts, pages: vec![Content::new()], y: PAGE_H - MARGIN };
        assert!(l.wrap(Style::Regular, 11.5, TEXT_W, &"word ".repeat(200)).len() > 10);
        assert!(l.wrap(Style::Regular, 11.5, 50.0, &"x".repeat(60)).len() > 1, "long words break");
        let bytes = render(&doc).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        let count: usize = text.split("/Count ").nth(1).and_then(|r| r.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()).unwrap();
        assert!(count >= 3, "{count} pages");
    }
}
