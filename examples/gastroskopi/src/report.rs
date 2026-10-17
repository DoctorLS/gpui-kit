//! The report record and the clinical vocabularies its fields choose from.
//!
//! Field names follow the browser edition of this tool (`gastroskopi-raport-v2.html`),
//! so a `form` object exported there deserializes here unchanged and the other way round.

use serde::{Deserialize, Serialize};

/// Everything the endoscopist enters for one procedure.
///
/// Free values stay `String` exactly as typed: numbers included, because the report
/// prints them verbatim and an empty field must stay distinguishable from zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Report {
    // Patient and procedure
    pub pt_name: String,
    pub pt_dob: String,
    pub pt_sex: String,
    pub pt_id: String,
    pub referrer: String,
    pub date: String,
    pub time: String,
    pub endo: String,
    pub assistant: String,
    pub scope: String,
    pub asa: String,
    pub ind: Vec<String>,
    pub ind_free: String,
    pub consent: bool,

    // Sedation
    pub sed_type: String,
    pub propofol: String,
    pub midazolam: String,
    pub fentanyl: String,
    pub o2: String,
    pub sed_other: String,
    pub monitor: String,
    pub tolerance: String,

    // Esophagus
    pub e_normal: bool,
    pub zline: String,
    pub la: String,
    pub hh: String,
    pub hh_cm: String,
    pub bar_c: String,
    pub bar_m: String,
    #[serde(rename = "var")]
    pub varices: String,
    pub var_red: bool,
    pub e_other: Vec<String>,
    pub e_free: String,

    // Stomach
    pub s_normal: bool,
    pub retro: String,
    pub hill: String,
    pub s_content: String,
    pub gastro: String,
    pub s_ul_loc: String,
    pub s_ul_mm: String,
    pub s_ul_f: String,
    pub pol_n: String,
    pub pol_mm: String,
    pub pol_loc: String,
    pub pol_paris: String,
    pub s_other: Vec<String>,
    pub s_free: String,

    // Duodenum
    pub d_normal: bool,
    pub extent: String,
    pub bulb: String,
    pub d_ul_loc: String,
    pub d_ul_mm: String,
    pub d_ul_f: String,
    pub d_other: Vec<String>,
    pub d_free: String,

    // Biopsy, interventions, complications
    pub sydney: Vec<String>,
    pub bx_other: String,
    pub hp_test: String,
    pub hp_res: String,
    #[serde(rename = "int")]
    pub interventions: Vec<String>,
    pub int_free: String,
    pub comp: String,
    pub comp_free: String,

    // Conclusion
    pub concl: String,
    pub recs: String,
}

impl Default for Report {
    fn default() -> Self {
        Self {
            pt_name: String::new(),
            pt_dob: String::new(),
            pt_sex: String::new(),
            pt_id: String::new(),
            referrer: String::new(),
            date: String::new(),
            time: String::new(),
            endo: String::new(),
            assistant: String::new(),
            scope: String::new(),
            asa: String::new(),
            ind: Vec::new(),
            ind_free: String::new(),
            consent: false,
            sed_type: String::new(),
            propofol: String::new(),
            midazolam: String::new(),
            fentanyl: String::new(),
            o2: String::new(),
            sed_other: String::new(),
            monitor: String::new(),
            tolerance: String::new(),
            e_normal: false,
            zline: String::new(),
            la: String::new(),
            hh: String::new(),
            hh_cm: String::new(),
            bar_c: String::new(),
            bar_m: String::new(),
            varices: String::new(),
            var_red: false,
            e_other: Vec::new(),
            e_free: String::new(),
            s_normal: false,
            retro: String::new(),
            hill: String::new(),
            s_content: String::new(),
            gastro: String::new(),
            s_ul_loc: String::new(),
            s_ul_mm: String::new(),
            s_ul_f: String::new(),
            pol_n: String::new(),
            pol_mm: String::new(),
            pol_loc: String::new(),
            pol_paris: String::new(),
            s_other: Vec::new(),
            s_free: String::new(),
            d_normal: false,
            // The first option of each select in the browser edition.
            extent: "D2".into(),
            bulb: String::new(),
            d_ul_loc: String::new(),
            d_ul_mm: String::new(),
            d_ul_f: String::new(),
            d_other: Vec::new(),
            d_free: String::new(),
            sydney: Vec::new(),
            bx_other: String::new(),
            hp_test: String::new(),
            hp_res: String::new(),
            interventions: Vec::new(),
            int_free: String::new(),
            comp: "asnjë".into(),
            comp_free: String::new(),
            concl: String::new(),
            recs: String::new(),
        }
    }
}

/// Maps field keys to struct fields, generating the reader and the writer from one
/// table so the two cannot disagree.
macro_rules! field_table {
    ($ty:ty, $what:literal, $read:ident, $write:ident, { $($key:literal => $field:ident),* $(,)? }) => {
        impl Report {
            pub fn $write(&mut self, key: &str) -> &mut $ty {
                match key {
                    $($key => &mut self.$field,)*
                    _ => unreachable!(concat!("unknown ", $what, " `{}`"), key),
                }
            }

            pub fn $read(&self, key: &str) -> &$ty {
                match key {
                    $($key => &self.$field,)*
                    _ => unreachable!(concat!("unknown ", $what, " `{}`"), key),
                }
            }
        }
    };
}

// Free text and single choices.
field_table!(String, "text field", text, text_mut, {
    "pt_name" => pt_name,
    "pt_dob" => pt_dob,
    "pt_sex" => pt_sex,
    "pt_id" => pt_id,
    "referrer" => referrer,
    "date" => date,
    "time" => time,
    "endo" => endo,
    "assistant" => assistant,
    "scope" => scope,
    "asa" => asa,
    "ind_free" => ind_free,
    "sed_type" => sed_type,
    "propofol" => propofol,
    "midazolam" => midazolam,
    "fentanyl" => fentanyl,
    "o2" => o2,
    "sed_other" => sed_other,
    "monitor" => monitor,
    "tolerance" => tolerance,
    "zline" => zline,
    "la" => la,
    "hh" => hh,
    "hh_cm" => hh_cm,
    "bar_c" => bar_c,
    "bar_m" => bar_m,
    "var" => varices,
    "e_free" => e_free,
    "retro" => retro,
    "hill" => hill,
    "s_content" => s_content,
    "gastro" => gastro,
    "s_ul_loc" => s_ul_loc,
    "s_ul_mm" => s_ul_mm,
    "s_ul_f" => s_ul_f,
    "pol_n" => pol_n,
    "pol_mm" => pol_mm,
    "pol_loc" => pol_loc,
    "pol_paris" => pol_paris,
    "s_free" => s_free,
    "extent" => extent,
    "bulb" => bulb,
    "d_ul_loc" => d_ul_loc,
    "d_ul_mm" => d_ul_mm,
    "d_ul_f" => d_ul_f,
    "d_free" => d_free,
    "bx_other" => bx_other,
    "hp_test" => hp_test,
    "hp_res" => hp_res,
    "int_free" => int_free,
    "comp" => comp,
    "comp_free" => comp_free,
    "concl" => concl,
    "recs" => recs,
});

// Independent yes/no answers.
field_table!(bool, "flag", flag, flag_mut, {
    "consent" => consent,
    "e_normal" => e_normal,
    "var_red" => var_red,
    "s_normal" => s_normal,
    "d_normal" => d_normal,
});

// Multiple choices.
field_table!(Vec<String>, "list", list, list_mut, {
    "ind" => ind,
    "e_other" => e_other,
    "s_other" => s_other,
    "d_other" => d_other,
    "sydney" => sydney,
    "int" => interventions,
});

impl Report {
    /// Whether nothing worth keeping has been entered yet.
    pub fn is_blank(&self) -> bool {
        self.pt_name.trim().is_empty()
            && self.concl.trim().is_empty()
            && !self.e_normal
            && !self.s_normal
            && !self.d_normal
            && self.la.is_empty()
            && self.gastro.is_empty()
    }
}

/// One photo attached to a report. The image lives in the archive's photo folder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Photo {
    pub file: String,
    #[serde(default)]
    pub cap: String,
}

/// A report as the archive stores it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub id: String,
    /// Milliseconds since the Unix epoch; `0` until first saved.
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub updated: i64,
    #[serde(default)]
    pub form: Report,
    #[serde(default)]
    pub photos: Vec<Photo>,
}

impl Record {
    pub fn new(form: Report) -> Self {
        Self {
            id: new_id(),
            created: 0,
            updated: 0,
            form,
            photos: Vec::new(),
        }
    }
}

/// Clinic details printed in every report header, plus defaults for new reports.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Clinic {
    pub c_name: String,
    pub c_addr: String,
    pub c_endo: String,
    pub c_scope: String,
    /// The logo as a `data:` URL, the browser edition's representation.
    pub c_logo: String,
}

/// A record ID in the browser edition's format: time and randomness, base 36.
pub fn new_id() -> String {
    let mut bytes = [0u8; 8];
    getrandom::getrandom(&mut bytes).expect("the operating system provides randomness");
    format!(
        "{}{}",
        to_base36(chrono::Utc::now().timestamp_millis() as u64),
        &to_base36(u64::from_le_bytes(bytes))[..6]
    )
}

fn to_base36(mut n: u64) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = Vec::new();
    loop {
        out.push(DIGITS[(n % 36) as usize]);
        n /= 36;
        if n == 0 {
            break;
        }
    }
    out.reverse();
    String::from_utf8(out).expect("base 36 digits are ASCII")
}

/// A choice offered by a select: the stored value and the label people pick.
pub type Choice = (&'static str, &'static str);

pub const SEX: &[Choice] = &[("", "—"), ("Mashkull", "Mashkull"), ("Femër", "Femër")];
pub const ASA: &[Choice] = &[
    ("", "—"),
    ("I", "I"),
    ("II", "II"),
    ("III", "III"),
    ("IV", "IV"),
];
pub const SEDATION: &[Choice] = &[
    ("", "—"),
    ("pa sedacion", "pa sedacion"),
    (
        "anestezi lokale e faringut (lidokainë spraj)",
        "anestezi lokale e faringut (lidokainë spraj)",
    ),
    ("sedacion i ndërgjegjshëm", "sedacion i ndërgjegjshëm"),
    (
        "sedacion i thellë me propofol",
        "sedacion i thellë me propofol",
    ),
    ("anestezi e përgjithshme", "anestezi e përgjithshme"),
];
pub const MONITORING: &[Choice] = &[
    ("", "—"),
    ("SpO₂, TA, EKG", "SpO₂, TA, EKG"),
    ("SpO₂, TA", "SpO₂, TA"),
    ("SpO₂", "SpO₂"),
];
pub const TOLERANCE: &[Choice] = &[
    ("", "—"),
    ("e mirë", "e mirë"),
    ("e pranueshme", "e pranueshme"),
    ("e dobët", "e dobët"),
];
pub const LA_GRADE: &[Choice] = &[
    ("", "asnjë"),
    ("A", "A – lezione ≤5 mm"),
    ("B", "B – lezione >5 mm, pa u bashkuar"),
    ("C", "C – të bashkuara, <75% e perimetrit"),
    ("D", "D – ≥75% e perimetrit"),
];
pub const YES_NO: &[Choice] = &[("", "jo"), ("po", "po")];
pub const VARICES: &[Choice] = &[
    ("", "jo"),
    ("F1", "F1 – të vogla, të drejta"),
    ("F2", "F2 – të zgjeruara, gjarpëruese, <⅓ e lumenit"),
    ("F3", "F3 – të mëdha, >⅓ e lumenit"),
];
pub const RETROFLEXION: &[Choice] = &[
    ("", "—"),
    ("u krye", "u krye"),
    ("nuk u krye", "nuk u krye"),
];
pub const HILL: &[Choice] = &[
    ("", "—"),
    ("I", "I"),
    ("II", "II"),
    ("III", "III"),
    ("IV", "IV"),
];
pub const GASTRIC_CONTENT: &[Choice] = &[
    ("", "lëng i pastër"),
    ("mbetje ushqimore", "mbetje ushqimore"),
    ("biliare", "biliare"),
    ("hematinë", "hematinë"),
    ("gjak i freskët", "gjak i freskët"),
];
pub const GASTROPATHY: &[Choice] = &[
    ("", "asnjë"),
    ("eritematoze e antrumit", "eritematoze e antrumit"),
    ("eritematoze pangastrike", "eritematoze pangastrike"),
    ("erozive e antrumit", "erozive e antrumit"),
    ("erozive pangastrike", "erozive pangastrike"),
    ("nodulare e antrumit", "nodulare e antrumit"),
    ("atrofike (dyshim)", "atrofike (dyshim)"),
];
pub const GASTRIC_ULCER_SITE: &[Choice] = &[
    ("", "asnjë ulçerë"),
    ("kardia", "kardia"),
    ("fundus", "fundus"),
    ("korpus", "korpus"),
    ("angulus", "angulus"),
    ("antrum", "antrum"),
    ("regjioni prepilorik", "regjioni prepilorik"),
];
pub const FORREST: &[Choice] = &[
    ("", "—"),
    ("Ia", "Ia – gjakosje aktive në rrymë"),
    ("Ib", "Ib – gjakosje aktive me pikim"),
    ("IIa", "IIa – enë e dukshme jo-gjakosëse"),
    ("IIb", "IIb – koagul aderent"),
    ("IIc", "IIc – njollë hematine e sheshtë"),
    ("III", "III – bazë e pastër me fibrinë"),
];
pub const POLYP_SITE: &[Choice] = &[
    ("", "—"),
    ("fundus", "fundus"),
    ("korpus", "korpus"),
    ("angulus", "angulus"),
    ("antrum", "antrum"),
];
pub const PARIS: &[Choice] = &[
    ("", "—"),
    ("0-Ip", "0-Ip – me kërcell"),
    ("0-Is", "0-Is – sesil"),
    ("0-IIa", "0-IIa – pak i ngritur"),
    ("0-IIb", "0-IIb – i sheshtë"),
    ("0-IIc", "0-IIc – pak i zhytur"),
];
pub const EXTENT: &[Choice] = &[
    ("D2", "D2"),
    ("D3", "D3"),
    ("bulb", "bulbin duodenal"),
    ("pilor", "pilor (nuk u kalua)"),
];
pub const BULB: &[Choice] = &[
    ("", "pa veçori"),
    ("bulbit eritematoz", "bulbit eritematoz"),
    ("bulbit eroziv", "bulbit eroziv"),
    (
        "bulb i deformuar (cikatricial)",
        "bulb i deformuar (cikatricial)",
    ),
];
pub const DUODENAL_ULCER_SITE: &[Choice] = &[
    ("", "asnjë ulçerë"),
    ("muri i përparmë i bulbit", "muri i përparmë i bulbit"),
    ("muri i pasmë i bulbit", "muri i pasmë i bulbit"),
    ("postbulbare", "postbulbare"),
];
pub const HP_TEST: &[Choice] = &[
    ("", "nuk u krye"),
    ("test i shpejtë i ureazës", "test i shpejtë i ureazës"),
    ("histologji", "histologji"),
];
pub const HP_RESULT: &[Choice] = &[
    ("", "në pritje"),
    ("pozitiv", "pozitiv"),
    ("negativ", "negativ"),
];
pub const COMPLICATIONS: &[Choice] = &[
    ("asnjë", "asnjë"),
    (
        "gjakosje e vogël, e kontrolluar endoskopikisht",
        "gjakosje e vogël, e kontrolluar endoskopikisht",
    ),
    ("desaturim i përkohshëm", "desaturim i përkohshëm"),
    ("hipotension i përkohshëm", "hipotension i përkohshëm"),
    ("perforacion", "perforacion"),
    ("tjetër", "tjetër"),
];

pub const INDICATIONS: &[&str] = &[
    "Dispepsi",
    "Simptoma refluksi",
    "Disfagi",
    "Dhimbje epigastrike",
    "Nauze / të vjella",
    "Hemorragji GI e sipërme",
    "Anemi nga mungesa e hekurit",
    "Humbje peshe",
    "Kontroll i ezofagut Barrett",
    "Kontroll i ulçerës",
    "Skrining për varice",
    "Dyshim për celiaki",
];
pub const ESOPHAGUS_OTHER: &[&str] = &[
    "kandidozë",
    "ezofagit eozinofilik (dyshim)",
    "strikturë peptike",
    "unazë Schatzki",
    "lezion Mallory-Weiss",
    "divertikul",
    "masë / tumor",
];
pub const STOMACH_OTHER: &[&str] = &[
    "gastropati portal-hipertensive",
    "GAVE",
    "metaplazi intestinale (dyshim)",
    "lezion subepitelial",
    "masë / tumor",
    "ksantelazmë",
];
pub const DUODENUM_OTHER: &[&str] = &[
    "atrofi vilore (dyshim për celiaki)",
    "duodenit postbulbar",
    "divertikul periampular",
    "papila major pa veçori",
    "masë / tumor",
];
pub const SYDNEY: &[&str] = &["A1", "A2", "IA", "B1", "B2"];
pub const INTERVENTIONS: &[&str] = &[
    "polipektomi me ansë",
    "injektim adrenaline",
    "klip hemostatik",
    "koagulim me argon-plazmë (APC)",
    "ligaturë varicesh",
    "dilatim me balonë",
    "heqje trupi të huaj",
    "hemostazë e arritur",
];
pub const RECOMMENDATIONS: &[&str] = &[
    "IPP (p.sh. pantoprazol 40 mg) 1× në ditë para mëngjesit, për 8 javë.",
    "Në pritje të rezultatit histopatologjik.",
    "Terapi eradikuese për H. pylori nëse rezultati është pozitiv.",
    "Kontroll endoskopik pas 8–12 javësh.",
    "Mos hani dhe mos pini për 1 orë pas anestezisë lokale.",
    "Mos drejtoni automjet për 24 orë pas sedacionit.",
    "Kontroll te mjeku referues me rezultatet.",
];

/// The Forrest description for a grade, as printed after it.
pub fn forrest_description(grade: &str) -> &'static str {
    FORREST
        .iter()
        .find(|(value, _)| *value == grade)
        .and_then(|(_, label)| label.split_once(" – ").map(|(_, description)| description))
        .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_form_round_trip() {
        let json = r#"{"pt_name":"Ardit Hoxha","var":"F2","int":["klip hemostatik"],"ind":[],"unknown":1}"#;
        let report: Report = serde_json::from_str(json).unwrap();
        assert_eq!(report.pt_name, "Ardit Hoxha");
        assert_eq!(report.varices, "F2");
        assert_eq!(report.interventions, vec!["klip hemostatik"]);
        // Fields absent from the file keep the browser edition's select defaults.
        assert_eq!(report.extent, "D2");
        assert_eq!(report.comp, "asnjë");

        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["var"], "F2");
        assert_eq!(value["int"][0], "klip hemostatik");
    }

    #[test]
    fn test_field_keys() {
        let mut report = Report::default();
        *report.text_mut("var") = "F1".into();
        report.list_mut("int").push("x".into());
        *report.flag_mut("e_normal") = true;
        assert_eq!(report.text("var"), "F1");
        assert_eq!(report.interventions, vec!["x"]);
        assert!(report.e_normal);
    }

    #[test]
    fn test_new_id_is_unique() {
        assert_ne!(new_id(), new_id());
    }

    #[test]
    fn test_forrest_description() {
        assert_eq!(forrest_description("IIb"), "koagul aderent");
        assert_eq!(forrest_description(""), "");
    }
}
