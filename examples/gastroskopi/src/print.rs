//! The printable A4 report.
//!
//! GPUI has no print API, so the report is written as a self-contained HTML page and
//! opened in the system browser, whose print dialog also saves PDFs. The page carries
//! the browser edition's paper layout and opens that dialog by itself.

use crate::backup::PortablePhoto;
use crate::narrative::{ORGAN_SECTIONS, ReportDoc, dot, fmt_date};
use crate::report::Clinic;

pub fn html(doc: &ReportDoc, clinic: &Clinic, photos: &[PortablePhoto], endo: &str) -> String {
    let value = |v: &str| {
        if v.is_empty() {
            r#"<span class="empty">—</span>"#.to_string()
        } else {
            esc(v)
        }
    };

    let rows: String = doc
        .meta
        .chunks(2)
        .map(|pair| {
            let cell = |(key, val): &(&str, String)| {
                let shown = if *key == "Pacienti" && !val.is_empty() {
                    format!("<b>{}</b>", esc(val))
                } else {
                    value(val)
                };
                format!(r#"<td class="k">{key}</td><td>{shown}</td>"#)
            };
            format!("<tr>{}</tr>", pair.iter().map(cell).collect::<String>())
        })
        .collect();

    let sections: String = doc
        .sections
        .iter()
        .filter(|(title, body)| !body.is_empty() || ORGAN_SECTIONS.contains(title))
        .map(|(title, body)| {
            let body = if body.is_empty() {
                r#"<span class="empty">Pa të dhëna.</span>"#.to_string()
            } else {
                esc(body)
            };
            format!(r#"<section class="rs"><h4>{title}</h4><p>{body}</p></section>"#)
        })
        .collect();

    let pictures = if photos.is_empty() {
        String::new()
    } else {
        let figures: String = photos
            .iter()
            .enumerate()
            .map(|(ix, photo)| {
                format!(
                    r#"<figure><img src="{}" alt=""><figcaption>{}. {}</figcaption></figure>"#,
                    photo.src,
                    ix + 1,
                    esc(&photo.cap)
                )
            })
            .collect();
        format!(
            r#"<section class="rs"><h4>Fotodokumentimi</h4><div class="rimgs">{figures}</div></section>"#
        )
    };

    let conclusion = if doc.concl.is_empty() {
        r#"<p class="empty">Pa konkluzion.</p>"#.to_string()
    } else {
        format!(
            "<ol>{}</ol>",
            doc.concl
                .iter()
                .map(|line| format!("<li>{}</li>", esc(&dot(line))))
                .collect::<String>()
        )
    };

    let recommendations = if doc.recs.is_empty() {
        String::new()
    } else {
        format!(
            r#"<section class="rs"><h4>Rekomandime</h4><ul class="recs">{}</ul></section>"#,
            doc.recs
                .iter()
                .map(|line| format!("<li>{}</li>", esc(&dot(line))))
                .collect::<String>()
        )
    };

    let logo = if clinic.c_logo.starts_with("data:image/") {
        format!(r#"<img src="{}" alt="">"#, clinic.c_logo)
    } else {
        String::new()
    };
    let clinic_name = if clinic.c_name.trim().is_empty() {
        "Emri i klinikës"
    } else {
        clinic.c_name.trim()
    };
    let patient = doc
        .meta
        .first()
        .map(|(_, name)| name.as_str())
        .unwrap_or_default();
    let signer = if endo.trim().is_empty() {
        "Mjeku endoskopist"
    } else {
        endo.trim()
    };
    let today = fmt_date(&chrono::Local::now().date_naive().to_string());

    format!(
        r#"<!DOCTYPE html>
<html lang="sq">
<head>
<meta charset="utf-8">
<title>Raport gastroskopie – {title}</title>
<style>{STYLE}</style>
</head>
<body>
<div class="paper">
  <div class="rh">
    <div class="rh-l">{logo}<div><div class="clinic">{clinic_name}</div><div class="addr">{address}</div></div></div>
    <div class="rh-r"><div class="doc-t">Ezofagogastroduodenoskopi</div><div class="doc-s">Raport endoskopik</div></div>
  </div>
  <table class="meta">{rows}</table>
  {sections}{pictures}
  <div class="concl"><h4>Konkluzioni</h4>{conclusion}</div>
  {recommendations}
  <div class="sig"><div><div class="ln">{signer}</div><div class="stamp">Nënshkrimi dhe vula</div></div></div>
  <div class="foot">Raport i gjeneruar më {today}</div>
</div>
<script>window.addEventListener("load", () => setTimeout(() => window.print(), 300));</script>
</body>
</html>
"#,
        title = esc(patient),
        clinic_name = esc(clinic_name),
        address = esc(clinic.c_addr.trim()),
        signer = esc(signer),
    )
}

/// The file name a report is saved under: patient, date, kind.
pub fn file_stem(patient: &str, date: &str) -> String {
    let mut name = String::new();
    for c in patient.trim().chars() {
        if c.is_alphanumeric() {
            name.push(c);
        } else if !name.ends_with('_') {
            name.push('_');
        }
    }
    let name = name.trim_matches('_');
    let name = if name.is_empty() { "pacient" } else { name };
    let date = if date.is_empty() {
        chrono::Local::now().date_naive().to_string()
    } else {
        date.to_string()
    };
    format!("{name}_{date}_gastroskopi")
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

// The printed page is a document with its own fixed palette, not application chrome,
// so it does not follow the window theme.
const STYLE: &str = r#"
*{box-sizing:border-box}
html,body{margin:0;background:#f3f5f9}
.paper{background:#fff;color:#111;max-width:210mm;margin:24px auto;padding:14mm 14mm 12mm;box-shadow:0 10px 30px rgba(15,23,42,.12);font:10.5pt/1.45 "Segoe UI",Roboto,Helvetica,Arial,sans-serif}
.rh{display:flex;justify-content:space-between;gap:16px;align-items:flex-start;border-bottom:2px solid #0f766e;padding-bottom:8px}
.rh-l{display:flex;gap:10px;align-items:center}
.rh-l img{max-height:46px;max-width:120px}
.clinic{font-weight:700;font-size:12.5pt}
.addr{font-size:8.5pt;color:#555;white-space:pre-line}
.rh-r{text-align:right}
.doc-t{font-weight:700;font-size:12pt;color:#0f766e;text-transform:uppercase;letter-spacing:.3px}
.doc-s{font-size:9pt;color:#555}
table.meta{width:100%;border-collapse:collapse;margin:10px 0 6px;font-size:9.5pt}
table.meta td{padding:3px 6px;border:1px solid #e5e7eb;vertical-align:top}
table.meta td.k{color:#555;width:17%;background:#f8fafc;white-space:nowrap}
.rs{margin-top:9px;break-inside:avoid}
.rs h4,.concl h4{margin:0 0 2px;font-size:9.5pt;text-transform:uppercase;letter-spacing:.4px;color:#0f766e}
.rs p{margin:0;white-space:pre-line}
.empty{color:#9ca3af;font-style:italic}
.concl{margin-top:12px;padding:8px 12px;border-left:4px solid #0f766e;background:#f0fdfa;break-inside:avoid}
.concl ol{margin:2px 0 0;padding-left:20px;font-weight:600}
.rimgs{display:grid;grid-template-columns:repeat(4,1fr);gap:6px;margin-top:4px}
.rimgs figure{margin:0;break-inside:avoid}
.rimgs img{width:100%;aspect-ratio:4/3;object-fit:cover;border-radius:3px;display:block}
.rimgs figcaption{font-size:8pt;color:#444;text-align:center;margin-top:2px}
.recs{margin:2px 0 0;padding-left:18px}
.sig{display:flex;justify-content:flex-end;margin-top:26px;break-inside:avoid}
.sig>div{min-width:220px;text-align:center;font-size:9.5pt}
.sig .ln{border-top:1px solid #333;margin-top:34px;padding-top:4px}
.sig .stamp{color:#666;font-size:8.5pt}
.foot{margin-top:14px;font-size:8pt;color:#888;text-align:center}
@page{size:A4;margin:12mm}
@media print{html,body{background:#fff}.paper{box-shadow:none;margin:0;padding:0;max-width:none}}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::narrative::build;
    use crate::report::Report;

    #[test]
    fn test_html_escapes_entered_text() {
        let report = Report {
            pt_name: "<script>alert(1)</script>".into(),
            concl: "Gastrit & duodenit".into(),
            ..Report::default()
        };
        let page = html(&build(&report), &Clinic::default(), &[], "");
        assert!(!page.contains("<script>alert"));
        assert!(page.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(page.contains("<li>Gastrit &amp; duodenit.</li>"));
        assert!(page.contains("<h4>Ezofagu</h4><p><span class=\"empty\">Pa të dhëna.</span>"));
    }

    #[test]
    fn test_file_stem() {
        assert_eq!(
            file_stem("Ardit  Hoxha", "2026-10-11"),
            "Ardit_Hoxha_2026-10-11_gastroskopi"
        );
        assert!(file_stem(" ", "2026-10-11").starts_with("pacient_"));
    }
}
