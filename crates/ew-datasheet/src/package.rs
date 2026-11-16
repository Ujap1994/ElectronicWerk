//! Package names and the standard dimensions of the common packages
//! (JEDEC and EIAJ outlines, typical manufacturer values). The standard
//! values fill in what a datasheet does not give as a table and help to
//! read tables that use their own letters.

use std::collections::BTreeMap;

use crate::draft::{Dimension, Family};

/// A package recognised in a name.
#[derive(Debug, Clone, PartialEq)]
pub struct Recognised {
    /// Family.
    pub family: Family,
    /// Number of terminals, when the name gives it.
    pub pins: Option<u32>,
    /// Body size `(x, y)` in millimetres, when the name gives it (`5x5`).
    pub body: Option<(f64, f64)>,
    /// Wide body (SOIC-W, wide DIP).
    pub wide: bool,
    /// Exposed pad in the name (`HTSSOP`, `PowerPAD`, `-EP`).
    pub exposed_pad: bool,
    /// A short canonical name (`SOIC-8`, `QFN-32 5x5`).
    pub canonical: String,
}

/// Unicode dashes to `-`, upper case.
#[must_use]
pub fn normalize(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\u{2010}'..='\u{2015}' | '\u{2212}' | '\u{fe63}' | '\u{ff0d}' => '-',
            '×' => 'X',
            c => c.to_ascii_uppercase(),
        })
        .collect()
}

/// Package words and their families, longest first so that `TSSOP` wins
/// over `SOP`.
const WORDS: &[(&str, Family)] = &[
    ("HTSSOP", Family::Sop),
    ("HVSSOP", Family::Sop),
    ("TSSOP", Family::Sop),
    ("VSSOP", Family::Sop),
    ("MSOP", Family::Sop),
    ("SSOP", Family::Sop),
    ("QSOP", Family::Sop),
    ("HSOP", Family::Sop),
    ("SOIC", Family::Sop),
    ("TSOP", Family::Sop),
    ("SOP", Family::Sop),
    ("LQFP", Family::Qfp),
    ("TQFP", Family::Qfp),
    ("MQFP", Family::Qfp),
    ("PQFP", Family::Qfp),
    ("HTQFP", Family::Qfp),
    ("QFP", Family::Qfp),
    ("HVQFN", Family::Qfn),
    ("VQFN", Family::Qfn),
    ("WQFN", Family::Qfn),
    ("UQFN", Family::Qfn),
    ("TQFN", Family::Qfn),
    ("LFCSP", Family::Qfn),
    ("QFN", Family::Qfn),
    ("MLF", Family::Qfn),
    ("X2SON", Family::Dfn),
    ("WSON", Family::Dfn),
    ("VSON", Family::Dfn),
    ("USON", Family::Dfn),
    ("TSON", Family::Dfn),
    ("TDFN", Family::Dfn),
    ("UDFN", Family::Dfn),
    ("WDFN", Family::Dfn),
    ("DFN", Family::Dfn),
    ("SON", Family::Dfn),
    ("LFBGA", Family::Bga),
    ("TFBGA", Family::Bga),
    ("UFBGA", Family::Bga),
    ("DSBGA", Family::Bga),
    ("WLCSP", Family::Bga),
    ("FBGA", Family::Bga),
    ("BGA", Family::Bga),
    ("TSOT-23", Family::Sot23),
    ("TSOT23", Family::Sot23),
    ("SOT-223", Family::Sot223),
    ("SOT223", Family::Sot223),
    ("SOT-89", Family::Sot89),
    ("SOT89", Family::Sot89),
    ("SOT-23", Family::Sot23),
    ("SOT23", Family::Sot23),
    ("SOT-363", Family::Sot23),
    ("SOT363", Family::Sot23),
    ("SOT-353", Family::Sot23),
    ("SOT353", Family::Sot23),
    ("SOT-323", Family::Sot23),
    ("SOT323", Family::Sot23),
    ("SOT-563", Family::Sot23),
    ("SOT563", Family::Sot23),
    ("SOT25", Family::Sot23),
    ("SOT26", Family::Sot23),
    ("SC-70", Family::Sot23),
    ("SC70", Family::Sot23),
    ("SC-74", Family::Sot23),
    ("SC74", Family::Sot23),
    ("TO-252", Family::Dpak),
    ("TO252", Family::Dpak),
    ("TO-263", Family::Dpak),
    ("TO263", Family::Dpak),
    ("D2PAK", Family::Dpak),
    ("DDPAK", Family::Dpak),
    ("DPAK", Family::Dpak),
    ("D-PAK", Family::Dpak),
    ("TO-220", Family::To220),
    ("TO220", Family::To220),
    ("TO-247", Family::To220),
    ("TO247", Family::To220),
    ("TO-92", Family::To92),
    ("TO92", Family::To92),
    ("PDIP", Family::Dip),
    ("CDIP", Family::Dip),
    ("DIP", Family::Dip),
    ("SOD-123", Family::Chip),
    ("SOD123", Family::Chip),
    ("SOD-323", Family::Chip),
    ("SOD323", Family::Chip),
    ("SOD-523", Family::Chip),
    ("SOD523", Family::Chip),
    ("SO", Family::Sop),
];

/// Chip sizes (EIA inch code) and their bodies in millimetres.
const CHIPS: &[(&str, f64, f64, f64)] = &[
    ("0201", 0.6, 0.3, 0.15),
    ("0402", 1.0, 0.5, 0.25),
    ("0603", 1.6, 0.8, 0.3),
    ("0805", 2.0, 1.25, 0.4),
    ("1206", 3.2, 1.6, 0.5),
    ("1210", 3.2, 2.5, 0.5),
    ("2010", 5.0, 2.5, 0.6),
    ("2512", 6.3, 3.2, 0.6),
];

fn number_after(text: &str, at: usize) -> Option<u32> {
    let rest = &text[at..];
    let rest = rest.trim_start_matches(['-', ' ', '/']);
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    let after = rest.chars().nth(digits.len());
    // `SOT-23` has no pin number in "23"; `SOT-23-5` has.
    if digits.is_empty()
        || after.is_some_and(|c| c == '.' || c.is_ascii_alphabetic() && c != 'L' && c != 'P')
    {
        return None;
    }
    digits.parse().ok().filter(|n| (1..=2000).contains(n))
}

fn number_before(text: &str, at: usize) -> Option<u32> {
    // "8-Pin SOIC", "8-Lead", "8 PIN", "16L".
    let before = text[..at].trim_end();
    for marker in [
        "-PIN",
        " PIN",
        "-LEAD",
        " LEAD",
        "-BALL",
        " BALL",
        "L",
        "-TERMINAL",
    ] {
        if let Some(rest) = before.strip_suffix(marker) {
            let digits: String = rest
                .chars()
                .rev()
                .take_while(char::is_ascii_digit)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            if let Ok(n) = digits.parse::<u32>()
                && (1..=2000).contains(&n)
            {
                return Some(n);
            }
        }
    }
    None
}

fn body_size(text: &str) -> Option<(f64, f64)> {
    // "5X5", "5 X 5", "4.4X5.0", "(3MM X 3MM)".
    let mut t: String = text.replace("MM", "");
    while t.contains("  ") {
        t = t.replace("  ", " ");
    }
    let t = t.replace(" X ", "X").replace(" X", "X").replace("X ", "X");
    let bytes: Vec<char> = t.chars().collect();
    for (i, c) in bytes.iter().enumerate() {
        if *c != 'X' || i == 0 {
            continue;
        }
        let left: String = bytes[..i]
            .iter()
            .rev()
            .take_while(|c| c.is_ascii_digit() || **c == '.')
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let right: String = bytes[i + 1..]
            .iter()
            .take_while(|c| c.is_ascii_digit() || **c == '.')
            .collect();
        if let (Ok(a), Ok(b)) = (left.parse::<f64>(), right.parse::<f64>())
            && (0.5..=60.0).contains(&a)
            && (0.5..=60.0).contains(&b)
        {
            return Some((a, b));
        }
    }
    None
}

/// Recognises a package in a short text (a table header, a caption, an
/// ordering code). Returns the first package word found.
#[must_use]
pub fn recognise(text: &str) -> Option<Recognised> {
    let t = normalize(text);
    for (code, w, l, _) in CHIPS {
        if t.split(|c: char| !c.is_ascii_alphanumeric())
            .any(|word| word == *code)
        {
            return Some(Recognised {
                family: Family::Chip,
                pins: Some(2),
                body: Some((*w, *l)),
                wide: false,
                exposed_pad: false,
                canonical: (*code).to_owned(),
            });
        }
    }
    let (at, word, family) = WORDS.iter().find_map(|(word, family)| {
        let mut from = 0;
        while let Some(found) = t[from..].find(word) {
            let at = from + found;
            let before = t[..at].chars().last();
            let after = t[at + word.len()..].chars().next();
            // A word of its own (or followed by a pin count).
            let starts = before.is_none_or(|c| !c.is_ascii_alphabetic());
            let ends = after.is_none_or(|c| {
                !c.is_ascii_alphabetic() || matches!(c, 'N' | 'W') && *word == "SOIC"
            });
            if starts && ends {
                return Some((at, *word, *family));
            }
            from = at + word.len();
        }
        None
    })?;
    let end = at + word.len();
    let wide = t[end..].starts_with('W') || t.contains("WIDE") || t.contains("7.5");
    let mut pins = number_after(&t, end + usize::from(t[end..].starts_with(['W', 'N'])))
        .or_else(|| number_before(&t, at));
    // Names with a fixed pin count.
    let fixed = match word {
        "SOT25" | "SOT-353" | "SOT353" => Some(5),
        "SOT26" | "SOT-363" | "SOT363" | "SOT-563" | "SOT563" => Some(6),
        "SOT-323" | "SOT323" => Some(3),
        "SOD-123" | "SOD123" | "SOD-323" | "SOD323" | "SOD-523" | "SOD523" => Some(2),
        _ => None,
    };
    if fixed.is_some() {
        pins = fixed;
    }
    if pins.is_none() {
        pins = match family {
            Family::Sot23
            | Family::Sot223
            | Family::Sot89
            | Family::To92
            | Family::To220
            | Family::Dpak => Some(3),
            _ => None,
        };
    }
    let body = body_size(&t[end..]);
    let exposed_pad = word.starts_with('H') && family == Family::Sop
        || t.contains("POWERPAD")
        || t.contains("-EP")
        || t.contains("EXPOSED");
    let base = match word {
        "SO" | "SOP" => "SOIC",
        "SOT25" | "SOT26" | "SOT23" | "TSOT23" | "TSOT-23" => "SOT-23",
        "SOT223" => "SOT-223",
        "SOT89" => "SOT-89",
        "SC70" | "SOT-323" | "SOT323" | "SOT-353" | "SOT353" | "SOT-363" | "SOT363" => "SC-70",
        "TO252" | "DPAK" | "D-PAK" => "TO-252",
        "TO263" | "D2PAK" | "DDPAK" => "TO-263",
        "TO220" => "TO-220",
        "TO92" => "TO-92",
        "PDIP" | "CDIP" => "DIP",
        "MLF" => "QFN",
        other => other,
    };
    let mut canonical = base.to_owned();
    if let Some(n) = pins
        && !matches!(family, Family::Chip)
    {
        canonical = format!("{canonical}-{n}");
    }
    if let Some((a, b)) = body
        && matches!(
            family,
            Family::Qfn | Family::Qfp | Family::Dfn | Family::Bga
        )
    {
        canonical = format!("{canonical} {}x{}", trim(a), trim(b));
    }
    Some(Recognised {
        family,
        pins,
        body,
        wide,
        exposed_pad,
        canonical,
    })
}

fn trim(v: f64) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Every package named in a longer text, in order.
#[must_use]
pub fn find_all(text: &str) -> Vec<Recognised> {
    let mut out = Vec::new();
    // Split at separators that end a package name.
    for part in text.split([',', ';', '/', '(', ')', '|']) {
        if let Some(r) = recognise(part) {
            out.push(r);
        }
    }
    out
}

fn dim(min: f64, nom: f64, max: f64) -> Dimension {
    Dimension {
        min: Some(min),
        nom: Some(nom),
        max: Some(max),
        source: None,
    }
}

fn nominal(value: f64) -> Dimension {
    Dimension::nominal(value)
}

/// Standard dimensions of a package (JEDEC symbols, millimetres): `A`
/// height, `A1` stand-off, `b` lead width, `c` lead thickness, `D` body
/// length (along the pin rows), `E` lead span, `E1` body width, `e`
/// pitch, `L` foot length, `D2`/`E2` exposed pad, `b2` tab width.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one table per package family")]
pub fn standard(r: &Recognised) -> BTreeMap<String, Dimension> {
    let mut d = BTreeMap::new();
    let mut set = |k: &str, v: Dimension| {
        d.insert(k.to_owned(), v);
    };
    let pins = r.pins.unwrap_or(0);
    let name = r.canonical.as_str();
    match r.family {
        Family::Sop => {
            if name.starts_with("TSSOP") || name.starts_with("HTSSOP") {
                let length = match pins {
                    0..=8 => 3.0,
                    9..=16 => 5.0,
                    17..=20 => 6.5,
                    21..=24 => 7.8,
                    25..=28 => 9.7,
                    _ => 12.5,
                };
                set("e", nominal(if pins >= 38 { 0.5 } else { 0.65 }));
                set("E", dim(6.2, 6.4, 6.6));
                set("E1", dim(4.3, 4.4, 4.5));
                set("D", nominal(length));
                set("b", dim(0.19, 0.245, 0.3));
                set("L", dim(0.45, 0.6, 0.75));
                set("A", dim(1.0, 1.1, 1.2));
                set("A1", dim(0.05, 0.1, 0.15));
                set("c", dim(0.09, 0.15, 0.2));
            } else if name.starts_with("MSOP")
                || name.starts_with("VSSOP")
                || name.starts_with("HVSSOP")
            {
                set("e", nominal(if pins >= 10 { 0.5 } else { 0.65 }));
                set("E", dim(4.75, 4.9, 5.05));
                set("E1", dim(2.9, 3.0, 3.1));
                set("D", dim(2.9, 3.0, 3.1));
                set("b", dim(0.22, 0.3, 0.38));
                set("L", dim(0.4, 0.6, 0.8));
                set("A", dim(0.9, 1.0, 1.1));
                set("A1", dim(0.05, 0.1, 0.15));
                set("c", dim(0.13, 0.18, 0.23));
            } else if name.starts_with("SSOP") {
                let length = match pins {
                    0..=16 => 6.2,
                    17..=20 => 7.2,
                    21..=24 => 8.2,
                    _ => 10.2,
                };
                set("e", nominal(0.65));
                set("E", dim(7.4, 7.8, 8.2));
                set("E1", dim(5.0, 5.3, 5.6));
                set("D", nominal(length));
                set("b", dim(0.22, 0.3, 0.38));
                set("L", dim(0.55, 0.75, 0.95));
                set("A", dim(1.7, 1.85, 2.0));
                set("A1", dim(0.05, 0.12, 0.2));
                set("c", dim(0.09, 0.17, 0.25));
            } else if name.starts_with("QSOP") {
                set("e", nominal(0.635));
                set("E", dim(5.8, 6.0, 6.2));
                set("E1", dim(3.8, 3.9, 4.0));
                set("D", nominal(if pins <= 16 { 4.9 } else { 8.65 }));
                set("b", dim(0.2, 0.25, 0.3));
                set("L", dim(0.4, 0.635, 1.27));
                set("A", dim(1.35, 1.55, 1.75));
                set("A1", dim(0.1, 0.17, 0.25));
            } else if r.wide || pins >= 18 {
                let length = match pins {
                    0..=16 => 10.3,
                    17..=18 => 11.55,
                    19..=20 => 12.8,
                    21..=24 => 15.4,
                    _ => 17.9,
                };
                set("e", nominal(1.27));
                set("E", dim(10.0, 10.3, 10.65));
                set("E1", dim(7.4, 7.5, 7.6));
                set("D", nominal(length));
                set("b", dim(0.31, 0.41, 0.51));
                set("L", dim(0.4, 0.835, 1.27));
                set("A", dim(2.35, 2.5, 2.65));
                set("A1", dim(0.1, 0.2, 0.3));
                set("c", dim(0.2, 0.27, 0.33));
            } else {
                let length = match pins {
                    0..=8 => 4.9,
                    9..=14 => 8.65,
                    _ => 9.9,
                };
                set("e", nominal(1.27));
                set("E", dim(5.8, 6.0, 6.2));
                set("E1", dim(3.8, 3.9, 4.0));
                set("D", nominal(length));
                set("b", dim(0.31, 0.41, 0.51));
                set("L", dim(0.4, 0.835, 1.27));
                set("A", dim(1.35, 1.55, 1.75));
                set("A1", dim(0.1, 0.175, 0.25));
                set("c", dim(0.17, 0.21, 0.25));
            }
        }
        Family::Qfp => {
            let (body, pitch) = r.body.map_or_else(
                || match pins {
                    0..=32 => (7.0, 0.8),
                    33..=44 => (10.0, 0.8),
                    45..=48 => (7.0, 0.5),
                    49..=64 => (10.0, 0.5),
                    65..=80 => (12.0, 0.5),
                    81..=100 => (14.0, 0.5),
                    101..=144 => (20.0, 0.5),
                    _ => (28.0, 0.5),
                },
                |(w, _)| {
                    (
                        w,
                        if f64::from(pins) / 4.0 * 0.8 < w - 1.0 {
                            0.8
                        } else {
                            0.5
                        },
                    )
                },
            );
            set("e", nominal(pitch));
            set("D1", nominal(body));
            set("E1", nominal(body));
            set("D", nominal(body + 2.0));
            set("E", nominal(body + 2.0));
            if pitch >= 0.8 {
                set("b", dim(0.3, 0.37, 0.45));
            } else {
                set("b", dim(0.17, 0.22, 0.27));
            }
            set("L", dim(0.45, 0.6, 0.75));
            set("A", dim(1.4, 1.5, 1.6));
            set("A1", dim(0.05, 0.1, 0.15));
            set("c", dim(0.09, 0.15, 0.2));
        }
        Family::Qfn => {
            let (body, ep) = r.body.map_or_else(
                || match pins {
                    0..=16 => ((3.0, 3.0), 1.7),
                    17..=24 => ((4.0, 4.0), 2.6),
                    25..=32 => ((5.0, 5.0), 3.45),
                    33..=40 => ((6.0, 6.0), 4.1),
                    41..=48 => ((7.0, 7.0), 5.15),
                    49..=56 => ((8.0, 8.0), 6.0),
                    _ => ((9.0, 9.0), 7.15),
                },
                |b| (b, (b.0 - 1.4).max(0.5)),
            );
            let perimeter = 2.0 * (body.0 + body.1);
            let pitch = if pins > 0 && perimeter / f64::from(pins) < 0.55 {
                0.4
            } else {
                0.5
            };
            set("e", nominal(pitch));
            set("D", nominal(body.1));
            set("E", nominal(body.0));
            set("D2", nominal(ep));
            set("E2", nominal(ep));
            set(
                "b",
                if pitch < 0.5 {
                    dim(0.15, 0.2, 0.25)
                } else {
                    dim(0.18, 0.25, 0.3)
                },
            );
            set("L", dim(0.3, 0.4, 0.5));
            set("A", dim(0.8, 0.9, 1.0));
            set("A1", dim(0.0, 0.02, 0.05));
        }
        Family::Dfn => {
            let (body, pitch, ep) = r.body.map_or_else(
                || match pins {
                    0..=6 => ((2.0, 2.0), 0.65, (1.0, 1.6)),
                    7..=8 => ((3.0, 3.0), 0.65, (1.65, 2.38)),
                    9..=10 => ((3.0, 3.0), 0.5, (1.65, 2.38)),
                    _ => ((4.0, 4.0), 0.5, (2.4, 3.3)),
                },
                |b| {
                    let rows = f64::from(pins.max(2) / 2);
                    let pitch = if (rows - 1.0) * 0.65 < b.1 - 0.4 {
                        0.65
                    } else {
                        0.5
                    };
                    (b, pitch, ((b.0 - 1.4).max(0.5), (b.1 - 0.6).max(0.5)))
                },
            );
            set("e", nominal(pitch));
            set("D", nominal(body.1));
            set("E", nominal(body.0));
            set("D2", nominal(ep.1));
            set("E2", nominal(ep.0));
            set("b", dim(0.25, 0.3, 0.35));
            set("L", dim(0.3, 0.4, 0.5));
            set("A", dim(0.7, 0.75, 0.8));
            set("A1", dim(0.0, 0.02, 0.05));
        }
        Family::Sot23 => {
            if name.starts_with("SC-70") || name.starts_with("SC-74") && pins == 0 {
                set("e", nominal(0.65));
                set("D", dim(1.8, 2.0, 2.2));
                set("E", dim(1.8, 2.1, 2.4));
                set("E1", dim(1.15, 1.25, 1.35));
                set("b", dim(0.15, 0.22, 0.3));
                set("L", dim(0.26, 0.36, 0.46));
                set("A", dim(0.8, 0.95, 1.1));
                set("A1", dim(0.0, 0.05, 0.1));
            } else if pins <= 3 {
                set("e", nominal(0.95));
                set("D", dim(2.8, 2.9, 3.04));
                set("E", dim(2.1, 2.4, 2.64));
                set("E1", dim(1.2, 1.3, 1.4));
                set("b", dim(0.3, 0.4, 0.5));
                set("L", dim(0.3, 0.45, 0.6));
                set("A", dim(0.89, 1.0, 1.12));
                set("A1", dim(0.01, 0.05, 0.1));
            } else {
                set("e", nominal(if pins >= 8 { 0.65 } else { 0.95 }));
                set("D", dim(2.8, 2.9, 3.0));
                set("E", dim(2.6, 2.8, 3.0));
                set("E1", dim(1.5, 1.6, 1.7));
                set("b", dim(0.3, 0.4, 0.5));
                set("L", dim(0.3, 0.45, 0.6));
                set("A", dim(0.9, 1.15, 1.45));
                set("A1", dim(0.0, 0.05, 0.15));
            }
            set("c", dim(0.08, 0.15, 0.22));
        }
        Family::Sot223 => {
            set("e", nominal(2.3));
            set("D", dim(6.3, 6.5, 6.7));
            set("E", dim(6.7, 7.0, 7.3));
            set("E1", dim(3.3, 3.5, 3.7));
            set("b", dim(0.6, 0.7, 0.84));
            set("b2", dim(2.9, 3.0, 3.1));
            set("L", dim(0.75, 0.9, 1.1));
            set("A", dim(1.5, 1.65, 1.8));
            set("A1", dim(0.02, 0.06, 0.1));
            set("c", dim(0.24, 0.3, 0.35));
        }
        Family::Sot89 => {
            set("e", nominal(1.5));
            set("D", dim(4.4, 4.5, 4.6));
            set("E", dim(3.94, 4.1, 4.25));
            set("E1", dim(2.4, 2.5, 2.6));
            set("b", dim(0.36, 0.44, 0.48));
            set("b2", dim(1.6, 1.7, 1.8));
            set("L", dim(0.8, 1.0, 1.2));
            set("A", dim(1.4, 1.5, 1.6));
        }
        Family::Dpak => {
            if name.starts_with("TO-263") {
                set("e", nominal(2.54));
                set("D", dim(8.6, 9.0, 9.4));
                set("E", dim(14.6, 15.25, 15.9));
                set("E1", dim(9.65, 10.1, 10.5));
                set("b", dim(0.7, 0.81, 0.95));
                set("b2", dim(7.5, 8.0, 8.5));
                set("L", dim(1.8, 2.3, 2.8));
                set("A", dim(4.3, 4.45, 4.6));
            } else {
                set("e", nominal(2.29));
                set("D", dim(6.0, 6.1, 6.2));
                set("E", dim(9.4, 9.9, 10.4));
                set("E1", dim(6.35, 6.5, 6.73));
                set("b", dim(0.64, 0.76, 0.89));
                set("b2", dim(5.2, 5.35, 5.5));
                set("L", dim(1.4, 1.6, 1.78));
                set("A", dim(2.2, 2.3, 2.4));
            }
        }
        Family::Dip => {
            let wide = r.wide || pins >= 32;
            let length = match pins {
                0..=8 => 9.27,
                9..=14 => 19.05,
                15..=16 => 19.3,
                17..=18 => 22.9,
                19..=20 => 25.0,
                21..=24 => {
                    if wide {
                        31.75
                    } else {
                        31.8
                    }
                }
                25..=28 => 35.6,
                _ => 52.3,
            };
            set("e", nominal(2.54));
            set("E", nominal(if wide { 15.24 } else { 7.62 }));
            set("E1", dim(6.1, 6.35, 7.11));
            set("D", nominal(length));
            set("b", dim(0.36, 0.46, 0.56));
            set("b1", dim(1.14, 1.52, 1.78));
            set("L", dim(2.92, 3.3, 3.81));
            set("A", dim(4.0, 4.5, 5.33));
            set("A1", nominal(0.38));
        }
        Family::To220 => {
            set("e", nominal(2.54));
            set("E", dim(9.66, 10.0, 10.66));
            set("D", dim(8.6, 9.0, 9.4));
            set("D1", dim(15.0, 15.6, 16.0));
            set("b", dim(0.7, 0.8, 0.9));
            set("L", dim(12.7, 13.4, 14.0));
            set("A", dim(4.4, 4.5, 4.6));
        }
        Family::To92 => {
            set("e", nominal(1.27));
            set("D", dim(4.4, 4.6, 4.8));
            set("E", dim(3.4, 3.6, 3.8));
            set("b", dim(0.36, 0.45, 0.53));
            set("L", dim(12.7, 14.0, 15.5));
            set("A", dim(4.3, 4.5, 4.7));
        }
        Family::Chip => {
            if let Some((_, w, l, t)) = CHIPS.iter().find(|(c, ..)| *c == name) {
                set("D", nominal(*w));
                set("E", nominal(*l));
                set("L", nominal(*t));
                set("A", nominal((l * 0.7).clamp(0.3, 1.0)));
            } else if name.contains("123") {
                set("D", dim(3.55, 3.7, 3.85));
                set("D1", dim(2.55, 2.7, 2.85));
                set("E", dim(1.4, 1.6, 1.8));
                set("b", dim(0.45, 0.55, 0.65));
                set("A", dim(0.95, 1.1, 1.25));
            } else if name.contains("323") {
                set("D", dim(2.3, 2.5, 2.7));
                set("D1", dim(1.6, 1.7, 1.8));
                set("E", dim(1.15, 1.25, 1.35));
                set("b", dim(0.25, 0.3, 0.4));
                set("A", dim(0.8, 0.9, 1.0));
            } else if name.contains("523") {
                set("D", dim(1.5, 1.6, 1.7));
                set("D1", dim(1.1, 1.2, 1.3));
                set("E", dim(0.7, 0.8, 0.9));
                set("b", dim(0.25, 0.3, 0.35));
                set("A", dim(0.55, 0.6, 0.65));
            }
        }
        Family::Bga | Family::Header | Family::Other => {}
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical(text: &str) -> Option<String> {
        recognise(text).map(|r| r.canonical)
    }

    #[test]
    fn recognises_package_names() {
        assert_eq!(canonical("SOIC-8").as_deref(), Some("SOIC-8"));
        assert_eq!(canonical("8-Pin SOIC").as_deref(), Some("SOIC-8"));
        assert_eq!(canonical("SO−8").as_deref(), Some("SOIC-8"));
        assert_eq!(canonical("TSSOP20").as_deref(), Some("TSSOP-20"));
        assert_eq!(canonical("SOT25").as_deref(), Some("SOT-23-5"));
        assert_eq!(canonical("SOT-23").as_deref(), Some("SOT-23-3"));
        assert_eq!(canonical("SOT89-5").as_deref(), Some("SOT-89-5"));
        assert_eq!(
            canonical("QFN-32 (5 mm × 5 mm)").as_deref(),
            Some("QFN-32 5x5")
        );
        assert_eq!(canonical("LQFP64 10x10").as_deref(), Some("LQFP-64 10x10"));
        assert_eq!(canonical("PDIP-8").as_deref(), Some("DIP-8"));
        assert_eq!(canonical("TO-220").as_deref(), Some("TO-220-3"));
        assert_eq!(canonical("0805").as_deref(), Some("0805"));
        assert_eq!(canonical("Supply voltage"), None);
        assert_eq!(canonical("SOLDER"), None);
    }

    #[test]
    fn finds_every_package_of_a_list() {
        let names: Vec<String> = find_all("SOIC, SOT23-8, VSSOP, PDIP")
            .into_iter()
            .map(|r| r.canonical)
            .collect();
        assert_eq!(names, vec!["SOIC", "SOT-23-8", "VSSOP", "DIP"]);
    }

    #[test]
    fn standard_dimensions_fit_the_package() {
        let soic = standard(&recognise("SOIC-8").unwrap_or_else(|| panic!("no package")));
        assert_eq!(soic.get("e").and_then(Dimension::value), Some(1.27));
        assert_eq!(soic.get("D").and_then(Dimension::value), Some(4.9));
        let qfn = standard(&recognise("QFN-32").unwrap_or_else(|| panic!("no package")));
        assert_eq!(qfn.get("E").and_then(Dimension::value), Some(5.0));
        assert_eq!(qfn.get("e").and_then(Dimension::value), Some(0.5));
    }
}
