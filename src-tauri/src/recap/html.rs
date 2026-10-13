//! The recap as a self-contained web page: saved from the dashboard, or served to phones with a
//! download bar on top. Light and dark, readable on a phone, clean when printed.

use super::{Block, Document, FOOTER};

pub(crate) fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn page(doc: &Document, toolbar: Option<&str>) -> String {
    let mut body = String::new();
    let mut in_list = false;
    for b in &doc.blocks {
        let bullet = matches!(b, Block::Bullet(_));
        if in_list && !bullet {
            body.push_str("</ul>");
            in_list = false;
        }
        match b {
            Block::Section(t) => body.push_str(&format!("<h2>{}</h2>", esc(t))),
            Block::Heading { text, meta } => body.push_str(&format!(
                "<h3><span>{}</span>{}</h3>",
                esc(text),
                meta.as_deref().map(|m| format!("<small>{}</small>", esc(m))).unwrap_or_default()
            )),
            Block::Text(t) => body.push_str(&format!("<p>{}</p>", esc(t))),
            Block::Note(t) => body.push_str(&format!("<p class=\"note\">{}</p>", esc(t))),
            Block::Bullet(t) => {
                if !in_list {
                    body.push_str("<ul>");
                    in_list = true;
                }
                body.push_str(&format!("<li>{}</li>", esc(t)));
            }
        }
    }
    if in_list {
        body.push_str("</ul>");
    }
    let (title, subtitle) = (esc(&doc.title), esc(&doc.subtitle));
    let toolbar = toolbar.unwrap_or("");
    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>{title} — {subtitle}</title>
<style>
:root{{--ink:#1c1917;--muted:#78716c;--line:#e7e5e4;--accent:#b45309;--bg:#fafaf9;--chip:#f5f5f4}}
@media (prefers-color-scheme:dark){{:root{{--ink:#f5f5f4;--muted:#a8a29e;--line:#292524;--accent:#fbbf24;--bg:#0c0a09;--chip:#1c1917}}}}
*{{box-sizing:border-box}}
body{{margin:0;background:var(--bg);color:var(--ink);font:17px/1.6 Georgia,"Iowan Old Style","Noto Serif",serif;-webkit-text-size-adjust:100%}}
main{{max-width:720px;margin:0 auto;padding:40px max(20px,env(safe-area-inset-right)) 48px max(20px,env(safe-area-inset-left));overflow-wrap:anywhere}}
h1{{font:600 28px/1.2 system-ui,sans-serif;margin:0}} .date{{color:var(--muted);font-family:system-ui,sans-serif;margin:6px 0 28px}}
h2{{font:600 13px system-ui,sans-serif;text-transform:uppercase;letter-spacing:.08em;color:var(--muted);margin:32px 0 4px;padding-top:14px;border-top:2px solid var(--line)}}
h3{{display:flex;justify-content:space-between;align-items:baseline;gap:12px;font:600 18px system-ui,sans-serif;color:var(--accent);margin:18px 0 0}}
h3 small{{color:var(--muted);font:13px system-ui,sans-serif;white-space:nowrap}}
p{{margin:6px 0 0}} .note{{color:var(--muted);font:14px/1.5 system-ui,sans-serif}}
ul{{margin:6px 0 0;padding-left:1.2em}} li{{margin:6px 0}}
footer{{margin-top:40px;color:var(--muted);font:12px system-ui,sans-serif}}
.bar{{position:sticky;top:0;z-index:1;display:flex;flex-wrap:wrap;gap:8px;align-items:center;background:var(--bg);border-bottom:1px solid var(--line);
  padding:max(10px,env(safe-area-inset-top)) max(16px,env(safe-area-inset-right)) 10px max(16px,env(safe-area-inset-left));font:15px system-ui,sans-serif}}
.bar a{{color:var(--ink);text-decoration:none;background:var(--chip);border:1px solid var(--line);border-radius:8px;padding:8px 12px;min-height:40px;display:inline-flex;align-items:center}}
.bar .back{{margin-right:auto}} .bar .dl{{color:var(--muted);font-size:13px}}
@media print{{body{{background:#fff;color:#000}} main{{padding:0}} .bar{{display:none}} h3,li{{break-inside:avoid}} h2,h3{{break-after:avoid}}}}
</style></head><body>{toolbar}<main><h1>{title}</h1><p class="date">{subtitle}</p>{body}<footer>{FOOTER}</footer></main></body></html>"#
    )
}
