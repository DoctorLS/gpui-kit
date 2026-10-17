//! Turns the entered findings into report prose.
//!
//! The sentences match the browser edition word for word, so a report reads the
//! same whichever edition produced it. Everything here is pure and shared by the
//! on-screen preview, the printed page and the plain-text copy.

use chrono::{Datelike, NaiveDate};

use crate::report::{Clinic, Report, forrest_description};

/// A report laid out for display: rows of the patient table, findings by
/// section, and the numbered conclusion and recommendation lists.
#[derive(Debug, Clone, PartialEq)]
pub struct ReportDoc {
    pub meta: Vec<(&'static str, String)>,
    pub sections: Vec<(&'static str, String)>,
    pub concl: Vec<String>,
    pub recs: Vec<String>,
}

/// Sections printed even when empty, so a missing organ description is visible.
pub const ORGAN_SECTIONS: [&str; 3] = ["Ezofagu", "Stomaku", "Duodeni"];

pub fn build(r: &Report) -> ReportDoc {
    let age = age(&r.pt_dob, &r.date);
    let mut ind: Vec<&str> = r.ind.iter().map(String::as_str).collect();
    if !r.ind_free.trim().is_empty() {
        ind.push(r.ind_free.trim());
    }
    let indication = if ind.is_empty() {
        String::new()
    } else {
        let mut text = cap(&ind.join(", ")) + ".";
        if r.consent {
            text.push_str(" Pëlqimi i informuar u mor me shkrim.");
        }
        text
    };

    ReportDoc {
        meta: vec![
            ("Pacienti", r.pt_name.trim().to_string()),
            (
                "Data e procedurës",
                [fmt_date(&r.date), r.time.trim().to_string()]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(", "),
            ),
            (
                "Datëlindja",
                match (r.pt_dob.is_empty(), age) {
                    (true, _) => String::new(),
                    (false, Some(age)) => format!("{} ({age} vjeç)", fmt_date(&r.pt_dob)),
                    (false, None) => fmt_date(&r.pt_dob),
                },
            ),
            ("Mjeku endoskopist", r.endo.trim().to_string()),
            ("Gjinia", r.pt_sex.clone()),
            ("Asistenti/ja", r.assistant.trim().to_string()),
            ("Nr. i dosjes", r.pt_id.trim().to_string()),
            ("Aparati", r.scope.trim().to_string()),
            ("Referuar nga", r.referrer.trim().to_string()),
            ("ASA", r.asa.clone()),
        ],
        sections: vec![
            ("Indikacioni", indication),
            ("Sedacioni", sedation(r)),
            ("Ezofagu", esophagus(r)),
            ("Stomaku", stomach(r)),
            ("Duodeni", duodenum(r)),
            ("Biopsi dhe ndërhyrje", biopsy(r)),
            ("Komplikime", complications(r)),
        ],
        concl: lines(&r.concl),
        recs: lines(&r.recs),
    }
}

/// The report as plain text, for pasting into another record system.
pub fn plain_text(doc: &ReportDoc, r: &Report, clinic: &Clinic, captions: &[String]) -> String {
    let mut out = vec!["RAPORT I EZOFAGOGASTRODUODENOSKOPISË".to_string()];
    if !clinic.c_name.trim().is_empty() {
        out.push(clinic.c_name.trim().to_string());
    }
    out.push(String::new());
    for (key, value) in &doc.meta {
        if !value.is_empty() {
            out.push(format!("{key}: {value}"));
        }
    }
    for (title, body) in &doc.sections {
        if !body.is_empty() {
            out.extend([String::new(), title.to_uppercase(), body.clone()]);
        }
    }
    if !captions.is_empty() {
        let list = captions
            .iter()
            .enumerate()
            .map(|(ix, cap)| {
                let cap = if cap.trim().is_empty() {
                    "foto"
                } else {
                    cap.trim()
                };
                format!("{}. {cap}", ix + 1)
            })
            .collect::<Vec<_>>()
            .join("; ");
        out.extend([String::new(), "FOTODOKUMENTIMI".into(), list]);
    }
    if !doc.concl.is_empty() {
        out.extend([String::new(), "KONKLUZIONI".into()]);
        out.extend(
            doc.concl
                .iter()
                .enumerate()
                .map(|(ix, line)| format!("{}. {}", ix + 1, dot(line))),
        );
    }
    if !doc.recs.is_empty() {
        out.extend([String::new(), "REKOMANDIME".into()]);
        out.extend(doc.recs.iter().map(|line| format!("- {}", dot(line))));
    }
    out.extend([String::new(), r.endo.trim().to_string()]);
    out.join("\n").trim().to_string() + "\n"
}

/// Proposes a conclusion, one diagnosis per line, from the structured findings.
pub fn auto_conclusion(s: &Report) -> String {
    let mut d: Vec<String> = Vec::new();
    if !s.e_normal {
        if !s.la.is_empty() {
            d.push(format!("Ezofagit refluksi, gradë LA {}", s.la));
        }
        if has(&s.bar_c) || has(&s.bar_m) {
            d.push(format!("Ezofag Barrett (Praga {})", prague(s)));
        }
        if s.hh == "po" {
            d.push(format!("Hernie hiatale{}", opt(" ", &s.hh_cm, " cm")));
        }
        if !s.varices.is_empty() {
            d.push(format!(
                "Varice ezofageale {}{}",
                s.varices,
                if s.var_red { " me shenja të kuqe" } else { "" }
            ));
        }
        d.extend(s.e_other.iter().map(|x| format!("{} – ezofag", cap(x))));
    }
    if s.hill == "III" || s.hill == "IV" {
        d.push(format!("Insuficiencë e kardias (Hill {})", s.hill));
    }
    if !s.s_normal {
        if !s.gastro.is_empty() {
            d.push(format!("Gastropati {}", s.gastro));
        }
        if !s.s_ul_loc.is_empty() || !s.s_ul_f.is_empty() {
            d.push(format!(
                "Ulçerë gastrike{}{}",
                opt(" (", &s.s_ul_loc, ")"),
                opt(", Forrest ", &s.s_ul_f, "")
            ));
        }
        if has(&s.pol_n) || has(&s.pol_mm) {
            d.push(polyp(s).trim_end_matches('.').to_string());
        }
        d.extend(s.s_other.iter().map(|x| format!("{} – stomak", cap(x))));
    }
    if !s.d_normal {
        if !s.bulb.is_empty() {
            d.push(cap(&s.bulb));
        }
        if !s.d_ul_loc.is_empty() || !s.d_ul_f.is_empty() {
            d.push(format!(
                "Ulçerë duodenale{}{}",
                opt(" (", &s.d_ul_loc, ")"),
                opt(", Forrest ", &s.d_ul_f, "")
            ));
        }
        d.extend(
            s.d_other
                .iter()
                .filter(|x| !x.contains("pa veçori"))
                .map(|x| format!("{} – duoden", cap(x))),
        );
    }
    if s.hp_res == "pozitiv" {
        d.push("Infeksion me H. pylori".into());
    }
    if d.is_empty() {
        "Ezofagogastroduodenoskopi pa gjetje patologjike.".into()
    } else {
        d.join("\n")
    }
}

/// The fields a printed report should not go without, as the user knows them.
pub fn missing_for_print(r: &Report) -> Vec<&'static str> {
    let mut missing = Vec::new();
    if r.pt_name.trim().is_empty() {
        missing.push("emri i pacientit");
    }
    if r.date.is_empty() {
        missing.push("data e procedurës");
    }
    if r.concl.trim().is_empty() {
        missing.push("konkluzioni");
    }
    missing
}

/// A one-line summary of a section's findings for its header.
pub fn section_summary(r: &Report, section: &str) -> String {
    let join = |parts: Vec<Option<String>>| {
        parts
            .into_iter()
            .flatten()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let when = |cond: bool, text: &str| cond.then(|| text.to_string());
    match section {
        "e" if r.e_normal => "normal".into(),
        "e" => join(vec![
            when(!r.la.is_empty(), &format!("LA {}", r.la)),
            when(has(&r.bar_c) || has(&r.bar_m), "Barrett"),
            when(r.hh == "po", "HH"),
            Some(r.varices.clone()),
        ]),
        "s" if r.s_normal => "normal".into(),
        "s" => join(vec![
            Some(r.gastro.clone()),
            when(!r.s_ul_loc.is_empty() || !r.s_ul_f.is_empty(), "ulçerë"),
            when(has(&r.pol_n) || has(&r.pol_mm), "polip"),
        ]),
        "d" if r.d_normal => "normal".into(),
        "d" => join(vec![
            Some(r.bulb.clone()),
            when(!r.d_ul_loc.is_empty() || !r.d_ul_f.is_empty(), "ulçerë"),
        ]),
        _ => String::new(),
    }
}

fn esophagus(s: &Report) -> String {
    let mut o = Vec::new();
    if s.e_normal {
        o.push(
            "Ezofagu me kalueshmëri të lirë dhe mukozë me pamje normale në gjithë gjatësinë."
                .to_string(),
        );
        o.push(format!(
            "Linja Z e rregullt{}.",
            opt(", në ", &s.zline, " cm nga harqet dentare")
        ));
    } else {
        if has(&s.zline) {
            o.push(format!(
                "Linja Z në {} cm nga harqet dentare.",
                s.zline.trim()
            ));
        }
        if !s.la.is_empty() {
            o.push(format!("Ezofagit refluksi, gradë LA {}.", s.la));
        }
        if has(&s.bar_c) || has(&s.bar_m) {
            o.push(format!(
                "Mukozë me pamje cilindrike (Barrett), klasifikimi i Pragës {}.",
                prague(s)
            ));
        }
        if s.hh == "po" {
            o.push(format!(
                "Hernie hiatale rrëshqitëse{}.",
                opt(" ", &s.hh_cm, " cm")
            ));
        }
        if !s.varices.is_empty() {
            o.push(format!(
                "Varice ezofageale {}, {} shenja të kuqe.",
                s.varices,
                if s.var_red { "me" } else { "pa" }
            ));
        }
        if !s.e_other.is_empty() {
            o.push(cap(&s.e_other.join(", ")) + ".");
        }
    }
    if has(&s.e_free) {
        o.push(dot(&s.e_free));
    }
    o.join(" ")
}

fn retroflexion(s: &Report) -> String {
    match s.retro.as_str() {
        "nuk u krye" => "Retrofleksioni nuk u krye.".into(),
        "u krye" => format!(
            "Në retrofleksion kardia pa veçori{}.",
            opt(", valvula gastroezofageale Hill ", &s.hill, "")
        ),
        _ if !s.hill.is_empty() => format!("Valvula gastroezofageale Hill {}.", s.hill),
        _ => String::new(),
    }
}

fn ulcer(site: &str, loc: &str, mm: &str, forrest: &str) -> String {
    if loc.is_empty() && forrest.is_empty() {
        return String::new();
    }
    let mut t = format!("Ulçerë {site}");
    if !loc.is_empty() {
        t += &format!(" në {loc}");
    }
    if has(mm) {
        t += &format!(", {} mm", mm.trim());
    }
    if !forrest.is_empty() {
        t += &format!(", Forrest {forrest} ({})", forrest_description(forrest));
    }
    t + "."
}

fn polyp(s: &Report) -> String {
    if !has(&s.pol_n) && !has(&s.pol_mm) {
        return String::new();
    }
    let n = s.pol_n.trim().parse::<f64>().ok().filter(|n| *n != 0.0);
    let single = n.is_none_or(|n| n == 1.0);
    let mut t = if single {
        "Polip".to_string()
    } else {
        format!("{} polipe", s.pol_n.trim())
    };
    if !s.pol_loc.is_empty() {
        t += &format!(" në {}", s.pol_loc);
    }
    if has(&s.pol_mm) {
        t += &format!(
            ", {}{} mm",
            if single { "" } else { "deri në " },
            s.pol_mm.trim()
        );
    }
    if !s.pol_paris.is_empty() {
        t += &format!(", Paris {}", s.pol_paris);
    }
    t + "."
}

fn stomach(s: &Report) -> String {
    let mut o = Vec::new();
    if s.s_normal {
        o.push("Stomaku me sasi të vogël lëngu të pastër. Mukozë me pamje normale në fundus, korpus, angulus dhe antrum. Pilori i rrumbullakët dhe i kalueshëm.".to_string());
    } else {
        if !s.s_content.is_empty() {
            o.push(format!("Përmbajtja: {}.", s.s_content));
        }
        if !s.gastro.is_empty() {
            o.push(format!("Gastropati {}.", s.gastro));
        }
        o.push(ulcer("gastrike", &s.s_ul_loc, &s.s_ul_mm, &s.s_ul_f));
        o.push(polyp(s));
        if !s.s_other.is_empty() {
            o.push(cap(&s.s_other.join(", ")) + ".");
        }
    }
    o.push(retroflexion(s));
    if has(&s.s_free) {
        o.push(dot(&s.s_free));
    }
    join_sentences(o)
}

fn extent_text(extent: &str) -> &'static str {
    match extent {
        "D3" => "pjesa e tretë e duodenit (D3)",
        "bulb" => "bulbi duodenal",
        "pilor" => "pilori",
        _ => "pjesa e dytë e duodenit (D2)",
    }
}

fn duodenum(s: &Report) -> String {
    let mut o = Vec::new();
    let pylorus = s.extent == "pilor";
    if pylorus {
        o.push("Pilori nuk u kalua.".to_string());
    }
    if s.d_normal {
        if !pylorus {
            o.push(if s.extent == "bulb" {
                "Bulbi duodenal me mukozë normale.".to_string()
            } else {
                format!(
                    "Bulbi duodenal dhe {} me mukozë dhe pamje vilore normale.",
                    extent_text(&s.extent)
                )
            });
        }
    } else {
        if !s.bulb.is_empty() {
            o.push(cap(&s.bulb) + ".");
        }
        o.push(ulcer("duodenale", &s.d_ul_loc, &s.d_ul_mm, &s.d_ul_f));
        if !s.d_other.is_empty() {
            o.push(cap(&s.d_other.join(", ")) + ".");
        }
        if s.bulb.is_empty()
            && s.d_ul_loc.is_empty()
            && s.d_ul_f.is_empty()
            && s.d_other.is_empty()
            && !pylorus
        {
            o.push(format!("Ekzaminim deri në {}.", extent_text(&s.extent)));
        }
    }
    if has(&s.d_free) {
        o.push(dot(&s.d_free));
    }
    join_sentences(o)
}

fn sedation(s: &Report) -> String {
    let mut drugs = Vec::new();
    if has(&s.propofol) {
        drugs.push(format!("propofol {} mg", s.propofol.trim()));
    }
    if has(&s.midazolam) {
        drugs.push(format!("midazolam {} mg", s.midazolam.trim()));
    }
    if has(&s.fentanyl) {
        drugs.push(format!("fentanil {} µg", s.fentanyl.trim()));
    }
    if has(&s.sed_other) {
        drugs.push(s.sed_other.trim().to_string());
    }
    let mut o = Vec::new();
    if !s.sed_type.is_empty() {
        let list = if drugs.is_empty() {
            String::new()
        } else {
            format!(": {}", drugs.join(", "))
        };
        o.push(format!("{}{list}.", cap(&s.sed_type)));
    } else if !drugs.is_empty() {
        o.push(cap(&drugs.join(", ")) + ".");
    }
    if has(&s.o2) {
        o.push(format!("O₂ {} l/min.", s.o2.trim()));
    }
    if !s.monitor.is_empty() {
        o.push(format!("Monitorim: {}.", s.monitor));
    }
    if !s.tolerance.is_empty() {
        let how = match s.tolerance.as_str() {
            "e mirë" => "mirë",
            "e dobët" => "dobët",
            _ => "në mënyrë të pranueshme",
        };
        o.push(format!("Procedura u tolerua {how}."));
    }
    o.join(" ")
}

fn biopsy(s: &Report) -> String {
    let mut o = Vec::new();
    if !s.sydney.is_empty() {
        o.push(format!("Biopsi sipas Sydney: {}.", s.sydney.join(", ")));
    }
    if has(&s.bx_other) {
        o.push(format!("Biopsi: {}", dot(&s.bx_other)));
    }
    if !s.hp_test.is_empty() {
        let result = if s.hp_res.is_empty() {
            "në pritje"
        } else {
            &s.hp_res
        };
        o.push(format!("H. pylori ({}): {result}.", s.hp_test));
    }
    if !s.interventions.is_empty() {
        o.push(format!(
            "Ndërhyrje: {}{}.",
            s.interventions.join(", "),
            opt(" – ", &s.int_free, "")
        ));
    } else if has(&s.int_free) {
        o.push(format!("Ndërhyrje: {}", dot(&s.int_free)));
    }
    o.join(" ")
}

fn complications(s: &Report) -> String {
    let base = if s.comp.is_empty() {
        "Asnjë".to_string()
    } else {
        cap(&s.comp)
    };
    format!("{base}{}.", opt(" – ", &s.comp_free, ""))
}

fn prague(s: &Report) -> String {
    let or_zero = |v: &str| {
        let v = v.trim();
        if v.is_empty() {
            "0".to_string()
        } else {
            v.to_string()
        }
    };
    format!("C{}M{}", or_zero(&s.bar_c), or_zero(&s.bar_m))
}

fn join_sentences(parts: Vec<String>) -> String {
    parts
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Splits a multi-line field into entries, dropping list markers people type.
fn lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| {
            let line = line.trim_start();
            let digits = line.chars().take_while(char::is_ascii_digit).count();
            let rest = if digits > 0 && matches!(line[digits..].chars().next(), Some('.' | ')')) {
                &line[digits + 1..]
            } else if let Some(rest) = line.strip_prefix(['-', '•', '*']) {
                rest
            } else {
                line
            };
            rest.trim().to_string()
        })
        .filter(|line| !line.is_empty())
        .collect()
}

fn has(value: &str) -> bool {
    !value.trim().is_empty()
}

/// `before + value + after` when `value` is filled in, otherwise nothing.
fn opt(before: &str, value: &str, after: &str) -> String {
    if has(value) {
        format!("{before}{}{after}", value.trim())
    } else {
        String::new()
    }
}

/// Capitalizes the first letter.
pub fn cap(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Ends a sentence with a full stop unless it already has terminal punctuation.
pub fn dot(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() || s.ends_with(['.', '!', '?']) {
        s.to_string()
    } else {
        format!("{s}.")
    }
}

/// `2026-10-11` → `11.10.2026`; anything else unchanged.
pub fn fmt_date(iso: &str) -> String {
    match parse_date(iso) {
        Some(date) => date.format("%d.%m.%Y").to_string(),
        None => iso.trim().to_string(),
    }
}

pub fn parse_date(iso: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(iso.trim(), "%Y-%m-%d").ok()
}

/// Age in whole years at the procedure date, or today when it has none.
pub fn age(dob: &str, at: &str) -> Option<i32> {
    let born = parse_date(dob)?;
    let at = parse_date(at).unwrap_or_else(|| chrono::Local::now().date_naive());
    let mut years = at.year() - born.year();
    if (at.month(), at.day()) < (born.month(), born.day()) {
        years -= 1;
    }
    (years >= 0).then_some(years)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Report {
        Report {
            pt_name: "Ardit Hoxha".into(),
            pt_dob: "1970-05-12".into(),
            date: "2026-10-11".into(),
            ind: vec!["Dispepsi".into()],
            ..Report::default()
        }
    }

    #[test]
    fn test_normal_findings_read_as_the_browser_edition() {
        let r = Report {
            e_normal: true,
            s_normal: true,
            d_normal: true,
            zline: "38".into(),
            retro: "u krye".into(),
            hill: "I".into(),
            tolerance: "e mirë".into(),
            ..sample()
        };
        let doc = build(&r);
        let section = |title| {
            doc.sections
                .iter()
                .find(|(t, _)| *t == title)
                .unwrap()
                .1
                .clone()
        };
        assert_eq!(section("Indikacioni"), "Dispepsi.");
        assert_eq!(
            section("Ezofagu"),
            "Ezofagu me kalueshmëri të lirë dhe mukozë me pamje normale në gjithë gjatësinë. \
             Linja Z e rregullt, në 38 cm nga harqet dentare."
        );
        assert!(
            section("Stomaku")
                .ends_with("Në retrofleksion kardia pa veçori, valvula gastroezofageale Hill I.")
        );
        assert_eq!(
            section("Duodeni"),
            "Bulbi duodenal dhe pjesa e dytë e duodenit (D2) me mukozë dhe pamje vilore normale."
        );
        assert_eq!(section("Sedacioni"), "Procedura u tolerua mirë.");
        assert_eq!(section("Komplikime"), "Asnjë.");
        assert_eq!(doc.meta[2].1, "12.05.1970 (56 vjeç)");
        assert_eq!(
            auto_conclusion(&r),
            "Ezofagogastroduodenoskopi pa gjetje patologjike."
        );
    }

    #[test]
    fn test_pathology_and_conclusion() {
        let r = Report {
            la: "B".into(),
            hh: "po".into(),
            hh_cm: "3".into(),
            bar_m: "2".into(),
            gastro: "erozive e antrumit".into(),
            s_ul_loc: "antrum".into(),
            s_ul_mm: "8".into(),
            s_ul_f: "III".into(),
            pol_n: "3".into(),
            pol_mm: "5".into(),
            d_other: vec![
                "papila major pa veçori".into(),
                "divertikul periampular".into(),
            ],
            hp_res: "pozitiv".into(),
            hill: "III".into(),
            ..sample()
        };
        let doc = build(&r);
        let stomach = &doc.sections[3].1;
        assert!(
            stomach.contains(
                "Ulçerë gastrike në antrum, 8 mm, Forrest III (bazë e pastër me fibrinë)."
            )
        );
        assert!(stomach.contains("3 polipe, deri në 5 mm."));
        assert!(doc.sections[2].1.contains("klasifikimi i Pragës C0M2."));
        assert_eq!(
            auto_conclusion(&r),
            [
                "Ezofagit refluksi, gradë LA B",
                "Ezofag Barrett (Praga C0M2)",
                "Hernie hiatale 3 cm",
                "Insuficiencë e kardias (Hill III)",
                "Gastropati erozive e antrumit",
                "Ulçerë gastrike (antrum), Forrest III",
                "3 polipe, deri në 5 mm",
                "Divertikul periampular – duoden",
                "Infeksion me H. pylori",
            ]
            .join("\n")
        );
    }

    #[test]
    fn test_single_polyp_and_pylorus() {
        let r = Report {
            pol_mm: "4".into(),
            pol_loc: "fundus".into(),
            extent: "pilor".into(),
            d_normal: true,
            ..sample()
        };
        assert_eq!(polyp(&r), "Polip në fundus, 4 mm.");
        assert_eq!(build(&r).sections[4].1, "Pilori nuk u kalua.");
    }

    #[test]
    fn test_list_markers_are_dropped() {
        assert_eq!(
            lines("1. Gastrit\n2) Ulçerë\n- IPP\n• kontroll\n\n  "),
            vec!["Gastrit", "Ulçerë", "IPP", "kontroll"]
        );
    }

    #[test]
    fn test_plain_text() {
        let r = Report {
            concl: "Gastrit".into(),
            endo: "Dr. Leka".into(),
            ..sample()
        };
        let text = plain_text(&build(&r), &r, &Clinic::default(), &["antrum".into()]);
        assert!(
            text.starts_with("RAPORT I EZOFAGOGASTRODUODENOSKOPISË\n\nPacienti: Ardit Hoxha\n")
        );
        assert!(text.contains("\nFOTODOKUMENTIMI\n1. antrum\n"));
        assert!(text.contains("\nKONKLUZIONI\n1. Gastrit.\n"));
        assert!(text.ends_with("\nDr. Leka\n"));
    }

    #[test]
    fn test_helpers() {
        assert_eq!(cap("ëzofag"), "Ëzofag");
        assert_eq!(dot(" tekst "), "tekst.");
        assert_eq!(dot("tekst?"), "tekst?");
        assert_eq!(fmt_date("2026-01-02"), "02.01.2026");
        assert_eq!(age("2000-10-12", "2026-10-11"), Some(25));
        assert_eq!(age("2000-10-11", "2026-10-11"), Some(26));
        assert_eq!(missing_for_print(&Report::default()).len(), 3);
    }
}
