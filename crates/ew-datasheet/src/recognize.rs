//! Recognises part data in a read datasheet: the part's name and
//! manufacturer, pin tables (with a column of pad numbers per package),
//! pinout drawings of two-row packages when there is no pin table, the
//! packages and their dimension tables.

use std::collections::{BTreeMap, BTreeSet};

use crate::Datasheet;
use crate::draft::{Dimension, Draft, Family, PackageDraft, PinDraft, PinType, Source, pad_order};
use crate::package::{self, Recognised, normalize};
use crate::table::{self, Table};
use crate::text::{Page, Rect, Word};

/// Recognises everything the datasheet tells about the part.
#[must_use]
pub fn recognize(sheet: &Datasheet) -> Draft {
    recognize_named(sheet, "")
}

/// Recognises the part, with the datasheet's file name as a hint for the
/// part number (`lm358.pdf`).
#[must_use]
pub fn recognize_named(sheet: &Datasheet, file_name: &str) -> Draft {
    let mut draft = Draft::default();
    info(sheet, file_name, &mut draft);
    let tables = tables(sheet);
    let (pins, columns) = pin_tables(&tables);
    let mut packages = packages(sheet, &columns, &pins);
    let mut pins = pins;
    if pins.is_empty()
        && let Some((figure_pins, count)) = pinout(sheet)
    {
        let names: Vec<String> = packages
            .iter()
            .filter(|p| p.pins == count || p.pins == 0)
            .map(|p| p.name.clone())
            .collect();
        let names = if names.is_empty() {
            vec![String::new()]
        } else {
            names
        };
        for (name, number, source) in figure_pins {
            pins.push(PinDraft {
                kind: infer_type(&name),
                numbers: names
                    .iter()
                    .map(|p| (p.clone(), vec![number.clone()]))
                    .collect(),
                name,
                function: String::new(),
                source: Some(source),
            });
        }
        draft.notes.push(
            "Pins were read from a pinout drawing; check the names against the datasheet.".into(),
        );
    }
    // Pins of a table without package columns belong to every package
    // with the same pin count.
    spread_unnamed(&mut pins, &packages);
    let known: BTreeSet<String> = packages.iter().map(|p| p.name.clone()).collect();
    let missing: BTreeSet<String> = pins
        .iter()
        .flat_map(|p| p.numbers.keys())
        .filter(|k| !known.contains(*k))
        .cloned()
        .collect();
    for name in missing {
        let count = pins
            .iter()
            .filter_map(|p| p.numbers.get(&name))
            .flatten()
            .filter_map(|n| n.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        packages.push(PackageDraft {
            name,
            family: Family::Other,
            pins: count,
            exposed_pad: false,
            dimensions: BTreeMap::new(),
            source: None,
            drawing: None,
        });
    }
    for table in dimension_tables(sheet, &tables) {
        attach(&mut packages, &table);
    }
    for p in &mut packages {
        if p.family == Family::Other {
            continue;
        }
        let recognised = Recognised {
            family: p.family,
            pins: (p.pins > 0).then_some(p.pins),
            body: body_of(p),
            wide: p.name.to_ascii_uppercase().contains("WIDE"),
            exposed_pad: p.exposed_pad,
            canonical: package::recognise(&p.name).map_or_else(|| p.name.clone(), |r| r.canonical),
        };
        for (k, v) in package::standard(&recognised) {
            p.dimensions.entry(k).or_insert(v);
        }
    }
    if pins.iter().any(|p| p.numbers.is_empty()) {
        draft.notes.push("Some pins have no pad number yet.".into());
    }
    for page in &sheet.pages {
        if page.scanned {
            draft.notes.push(format!(
                "Page {} is a scanned image without text; its values must be entered by hand.",
                page.number
            ));
        }
    }
    pins.sort_by(|a, b| {
        let first = |p: &PinDraft| {
            p.numbers
                .values()
                .flatten()
                .next()
                .cloned()
                .unwrap_or_default()
        };
        pad_order(&first(a), &first(b))
    });
    draft.pins = pins;
    draft.packages = packages;
    draft
}

// ----- part information --------------------------------------------------

const MANUFACTURERS: &[&str] = &[
    "Texas Instruments",
    "Analog Devices",
    "Microchip",
    "STMicroelectronics",
    "NXP",
    "Nexperia",
    "onsemi",
    "ON Semiconductor",
    "Infineon",
    "Renesas",
    "Maxim Integrated",
    "Diodes Incorporated",
    "ROHM",
    "Toshiba",
    "Vishay",
    "Espressif",
    "Nordic Semiconductor",
    "Silicon Labs",
    "Monolithic Power",
    "Richtek",
    "Torex",
    "Littelfuse",
    "Bourns",
    "Murata",
    "Würth Elektronik",
];

fn is_part_number(token: &str) -> bool {
    let t = token.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    let letters = t.chars().filter(char::is_ascii_alphabetic).count();
    let digits = t.chars().filter(char::is_ascii_digit).count();
    (3..=20).contains(&t.len())
        && letters >= 1
        && digits >= 2
        && t.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '/')
        && !t.chars().all(|c| c.is_ascii_digit() || c == '-')
        && package::recognise(t).is_none()
        && !t.to_ascii_uppercase().starts_with("REV")
}

fn clean_token(token: &str) -> String {
    token
        .trim_matches(|c: char| !c.is_ascii_alphanumeric())
        .to_owned()
}

#[allow(
    clippy::too_many_lines,
    reason = "name, maker and description in one pass"
)]
fn info(sheet: &Datasheet, file_name: &str, draft: &mut Draft) {
    // Part number: tokens of the title and of the first page, counted over
    // the page headers and footers of the whole datasheet.
    let mut counts: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut order = 0;
    let mut note = |token: &str, weight: usize, counts: &mut BTreeMap<String, (usize, usize)>| {
        if is_part_number(token) {
            order += 1;
            let e = counts.entry(clean_token(token)).or_insert((0, order));
            e.0 += weight;
        }
    };
    for token in sheet.title.split([' ', ',', ';']) {
        note(token, 3, &mut counts);
    }
    // The file name names the part when the datasheet covers a family.
    let stem = std::path::Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .split(['_', ' ', '.'])
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    let stem = stem.trim_end_matches("-D").to_owned();
    if let Some(first) = sheet.pages.first() {
        let largest = first.lines.iter().map(|l| l.size).fold(0.0, f64::max);
        for line in &first.lines {
            let weight = if line.size >= largest * 0.8 { 4 } else { 1 };
            for w in &line.words {
                note(&w.text, weight, &mut counts);
            }
        }
    }
    for page in sheet.pages.iter().skip(1) {
        for line in page
            .lines
            .iter()
            .filter(|l| l.bbox[1] < 80.0 || l.bbox[3] > page.height - 60.0)
        {
            for w in &line.words {
                if counts.contains_key(&clean_token(&w.text)) {
                    note(&w.text, 1, &mut counts);
                }
            }
        }
    }
    if is_part_number(&stem) {
        for (token, (weight, _)) in &mut counts {
            if token.to_ascii_uppercase() == stem {
                *weight += 1000;
            }
        }
    }
    if let Some((name, _)) = counts
        .iter()
        .max_by(|a, b| a.1.0.cmp(&b.1.0).then(b.1.1.cmp(&a.1.1)))
    {
        draft.name.clone_from(name);
        draft.mpn.clone_from(name);
        if let Some(first) = sheet.pages.first() {
            draft.source = first
                .lines
                .iter()
                .flat_map(|l| l.words.iter())
                .find(|w| clean_token(&w.text) == *name)
                .map(|w| Source::at(1, w.bbox));
        }
    }
    // Manufacturer: the author, else a known name on the first page.
    let author = sheet
        .author
        .split(['[', ','])
        .next()
        .unwrap_or("")
        .replace("Incorporated", "")
        .replace("Inc.", "")
        .replace("Corporation", "")
        .trim()
        .to_owned();
    draft.manufacturer = if author.is_empty() {
        let text: String = sheet
            .pages
            .iter()
            .take(2)
            .flat_map(|p| p.lines.iter())
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        MANUFACTURERS
            .iter()
            .find(|m| text.contains(**m))
            .map(|m| (*m).to_owned())
            .unwrap_or_default()
    } else {
        author
    };
    // Description: the subject, else the title without the part names.
    let title_rest: Vec<&str> = sheet
        .title
        .split(' ')
        .filter(|t| !is_part_number(t) && !t.eq_ignore_ascii_case("datasheet") && *t != "-")
        .collect();
    let generic = {
        let s = sheet.subject.to_ascii_lowercase();
        s.is_empty() || s.contains("data sheet") || s.contains("datasheet") && s.len() < 30
    };
    draft.description = if generic {
        title_rest
            .join(" ")
            .split(" (Rev")
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches(['-', ',', ' '])
            .to_owned()
    } else {
        let first = sheet
            .subject
            .split(". ")
            .next()
            .unwrap_or("")
            .trim()
            .trim_end_matches('.');
        first.chars().take(160).collect()
    };
}

// ----- tables ------------------------------------------------------------

const PIN_WORDS: &[&str] = &[
    "PIN",
    "PINS",
    "NAME",
    "NO",
    "NO.",
    "NUMBER",
    "FUNCTION",
    "DESCRIPTION",
    "TYPE",
    "I/O",
    "SYMBOL",
    "SIGNAL",
    "BALL",
    "TERMINAL",
];
/// Header words of tables that are not pin tables.
const NOT_PIN_WORDS: &[&str] = &[
    "PARAMETER",
    "CONDITIONS",
    "CONDITION",
    "UNIT",
    "UNITS",
    "MIN",
    "MIN.",
    "MAX",
    "MAX.",
    "TYP",
    "TYP.",
    "ORDERABLE",
    "STATUS",
    "QTY",
    "QUANTITY",
    "MARKING",
    "RATING",
    "VALUE",
    "DEVICE",
];
const DIMENSION_WORDS: &[&str] = &[
    "MIN",
    "MIN.",
    "NOM",
    "NOM.",
    "MAX",
    "MAX.",
    "DIM",
    "TYP",
    "TYP.",
    "MILLIMETERS",
    "INCHES",
];

fn keyword_count(text: &str, words: &[&str]) -> usize {
    let upper = normalize(text);
    let tokens: BTreeSet<&str> = upper.split([' ', '\n']).collect();
    words.iter().filter(|w| tokens.contains(**w)).count()
}

fn tables(sheet: &Datasheet) -> Vec<Table> {
    let mut out = Vec::new();
    for page in &sheet.pages {
        if page.scanned {
            continue;
        }
        let ruled = table::ruled(page);
        let inside = |r: &Rect| {
            ruled.iter().any(|t| {
                r[1] >= t.bbox[1] - 2.0
                    && r[3] <= t.bbox[3] + 2.0
                    && r[0] >= t.bbox[0] - 2.0
                    && r[2] <= t.bbox[2] + 2.0
            })
        };
        let lines: Vec<_> = page.lines.iter().filter(|l| l.angle == 0).collect();
        let mut used_until = 0.0;
        for (i, line) in lines.iter().enumerate() {
            if inside(&line.bbox) || line.bbox[1] < used_until {
                continue;
            }
            let pins = keyword_count(&line.text, PIN_WORDS);
            let dims = keyword_count(&line.text, DIMENSION_WORDS);
            if (pins >= 2 || dims >= 2)
                && line.words.len() >= 2
                && let Some(t) = table::under_header(page, i, 0)
            {
                used_until = t.bbox[3];
                out.push(t);
            }
        }
        out.extend(ruled);
    }
    out
}

// ----- pins --------------------------------------------------------------

/// A column of pad numbers and the packages it is for (empty: not named).
#[derive(Debug, Clone, PartialEq)]
pub struct NumberColumn {
    /// Packages named in the column header.
    pub packages: Vec<Recognised>,
    /// Where the header is.
    pub source: Source,
    /// Highest pad count in the column.
    pub count: u32,
}

/// Pad numbers of a cell: `1`, `6, 7`, `9–16`, `A1, B2`; `—` is none.
/// A variant in parentheses (`3 (R5)`) is returned apart.
fn numbers(text: &str) -> Option<(Vec<String>, Option<String>)> {
    let t = normalize(text);
    let mut variant = None;
    let mut out = Vec::new();
    let mut rest = t.as_str();
    // Footnotes and variants in parentheses.
    let mut cleaned = String::new();
    while let Some(open) = rest.find('(') {
        cleaned.push_str(&rest[..open]);
        let close = rest[open..].find(')').map_or(rest.len(), |c| open + c);
        let inner = rest[open + 1..close].trim();
        if !inner.chars().all(|c| c.is_ascii_digit()) && !inner.is_empty() {
            variant = Some(inner.to_owned());
        }
        rest = rest.get(close + 1..).unwrap_or("");
    }
    cleaned.push_str(rest);
    let cleaned = cleaned.trim();
    if matches!(cleaned, "-" | "—" | "–" | "N/A" | "NA" | "") {
        return Some((Vec::new(), variant));
    }
    for part in cleaned.split([',', ' ', ';']).filter(|p| !p.is_empty()) {
        if let Some((a, b)) = part.split_once('-')
            && let (Ok(a), Ok(b)) = (a.parse::<u32>(), b.parse::<u32>())
        {
            if b < a || b - a > 256 {
                return None;
            }
            out.extend((a..=b).map(|n| n.to_string()));
            continue;
        }
        let is_number = part.chars().all(|c| c.is_ascii_digit()) && part.len() <= 4;
        let is_ball = part.len() <= 4
            && part.starts_with(|c: char| c.is_ascii_uppercase())
            && part
                .chars()
                .skip_while(char::is_ascii_uppercase)
                .all(|c| c.is_ascii_digit())
            && part.chars().any(|c| c.is_ascii_digit())
            && part.chars().take_while(char::is_ascii_uppercase).count() <= 2;
        if is_number || is_ball {
            out.push(part.to_owned());
        } else {
            return None;
        }
    }
    if out.is_empty() {
        return None;
    }
    Some((out, variant))
}

/// The electrical type in a type column.
fn parse_type(text: &str) -> Option<PinType> {
    let t = normalize(text);
    let t = t
        .trim()
        .trim_end_matches(|c: char| c == ')' || c.is_ascii_digit() || c == '(');
    Some(match t.trim() {
        "I" | "IN" | "INPUT" | "DI" | "AI" | "I (ANALOG)" => PinType::Input,
        "O" | "OUT" | "OUTPUT" | "DO" | "AO" => PinType::Output,
        "I/O" | "IO" | "B" | "BIDIR" | "BIDIRECTIONAL" | "I/O/D" | "DIO" => PinType::Bidirectional,
        "P" | "PWR" | "POWER" | "S" | "SUPPLY" | "G" | "GND" | "GROUND" | "—" | "-" => {
            if matches!(t.trim(), "—" | "-") {
                return None;
            }
            PinType::PowerIn
        }
        "OD" | "OC" | "O (OD)" | "OPEN DRAIN" | "OPEN-DRAIN" | "OPEN COLLECTOR" => {
            PinType::OpenCollector
        }
        "A" | "ANALOG" | "A/I" | "AIO" => PinType::Passive,
        "NC" | "N/C" => PinType::NotConnected,
        "Z" | "T" | "TRI-STATE" | "O/Z" => PinType::TriState,
        _ => return None,
    })
}

/// The type of a pin by its name, when no column tells.
#[must_use]
pub fn infer_type(name: &str) -> PinType {
    let n = normalize(name);
    let n = n.trim();
    let starts = |prefixes: &[&str]| prefixes.iter().any(|p| n.starts_with(p));
    if matches!(n, "NC" | "N.C." | "DNC" | "N/C") {
        PinType::NotConnected
    } else if starts(&[
        "GND", "VSS", "AGND", "DGND", "PGND", "SGND", "V-", "VEE", "AVSS", "DVSS", "VSSA", "VSSIO",
    ]) || n == "GROUND"
        || starts(&[
            "VCC", "VDD", "V+", "VIN", "VBAT", "AVDD", "DVDD", "VDDA", "VDDIO", "VS", "PVIN",
            "VBUS", "VREF",
        ])
    {
        PinType::PowerIn
    } else if starts(&["EP", "PAD", "THERMAL", "EXPOSED", "POWERPAD"]) {
        PinType::Passive
    } else if starts(&["VOUT", "VO", "SW", "LX"]) {
        PinType::PowerOut
    } else if starts(&[
        "OUT", "Q", "TX", "MISO", "SDO", "DOUT", "PG", "FLT", "INT", "IRQ",
    ]) {
        PinType::Output
    } else if starts(&[
        "IN", "EN", "CE", "CS", "SS", "RX", "MOSI", "SDI", "DIN", "CLK", "SCK", "SCL", "RST",
        "RESET", "SHDN", "FB", "ADJ",
    ]) {
        PinType::Input
    } else if starts(&["SDA", "GPIO", "IO", "P", "D"]) && n.chars().any(|c| c.is_ascii_digit()) {
        PinType::Bidirectional
    } else {
        PinType::Unspecified
    }
}

fn clean_name(text: &str) -> String {
    let mut t: String = text
        .chars()
        .map(|c| match c {
            '\u{2010}'..='\u{2015}' | '\u{2212}' => '-',
            c => c,
        })
        .collect();
    // Footnote markers.
    while let Some(open) = t.rfind('(') {
        let inner = &t[open + 1..];
        let close = inner.find(')').map_or(t.len(), |c| open + 1 + c);
        if t[open + 1..close].chars().all(|c| c.is_ascii_digit()) {
            t.replace_range(open..=close.min(t.len() - 1), "");
        } else {
            break;
        }
    }
    t.trim().trim_end_matches(['*', '†']).trim().to_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Name,
    Number,
    Type,
    Function,
    Other,
}

fn role(header: &str) -> Role {
    let h = normalize(header);
    let tokens: BTreeSet<&str> = h
        .split([' ', '/', '\n'])
        .map(|t| t.trim_matches(['.', ':', '(', ')']))
        .collect();
    let has = |w: &str| tokens.contains(w);
    if has("NAME") || has("SYMBOL") || has("SIGNAL") || has("MNEMONIC") {
        return Role::Name;
    }
    if h.contains("I/O") || has("TYPE") || has("DIR") || has("DIRECTION") {
        return Role::Type;
    }
    if has("DESCRIPTION") || has("FUNCTION") || has("FUNCTIONS") {
        return Role::Function;
    }
    if has("NO")
        || has("NUMBER")
        || h.contains('#')
        || has("BALL")
        || has("PAD")
        || has("TERMINAL")
        || has("PIN")
        || has("PINS")
        || !package::find_all(header).is_empty()
    {
        return Role::Number;
    }
    Role::Other
}

#[derive(Debug, Default)]
struct PinTable {
    pins: Vec<PinDraft>,
    columns: Vec<NumberColumn>,
}

fn column_key(column: &NumberColumn) -> Vec<String> {
    if column.packages.is_empty() {
        return vec![String::new()];
    }
    column
        .packages
        .iter()
        .map(|p| {
            if p.pins.is_none() && column.count > 0 {
                format!("{}-{}", p.canonical, column.count)
            } else {
                p.canonical.clone()
            }
        })
        .collect()
}

#[allow(clippy::too_many_lines, reason = "one pass over a table")]
fn read_pin_table(t: &Table) -> Option<PinTable> {
    if t.columns < 2 || t.rows.len() < 3 {
        return None;
    }
    // Header rows: until a row has a pad number and a name-like cell.
    let mut header_rows = 0;
    for (i, row) in t.rows.iter().enumerate().take(4) {
        let numeric = row
            .cells
            .iter()
            .filter(|c| numbers(&c.text).is_some_and(|n| !n.0.is_empty()))
            .count();
        let words = keyword_count(
            &row.cells
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            PIN_WORDS,
        );
        if numeric > 0 && words == 0 && i > 0 {
            break;
        }
        header_rows = i + 1;
        if numeric > 0 && words == 0 {
            break;
        }
    }
    if header_rows == 0 || header_rows >= t.rows.len() {
        return None;
    }
    let headers: Vec<String> = (0..t.columns)
        .map(|c| {
            t.rows[..header_rows]
                .iter()
                .map(|r| r.text(c))
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    let all_headers = headers.join(" ");
    if keyword_count(&all_headers, PIN_WORDS) < 1 || keyword_count(&all_headers, NOT_PIN_WORDS) > 0
    {
        return None;
    }
    let upper = normalize(&all_headers);
    if [
        "PACKAGE SIZE",
        "BODY SIZE",
        "PART NUMBER",
        "ORDERING",
        "THERMAL METRIC",
        "PACKAGE DRAWING",
    ]
    .iter()
    .any(|w| upper.contains(w))
    {
        return None;
    }
    let body = &t.rows[header_rows..];
    let mut roles: Vec<Role> = headers.iter().map(|h| role(h)).collect();
    // A header cell over several columns ("PIN" over "NAME" and "NO.") gives
    // the more specific role below it; columns that only say "PIN" are
    // numbers unless their values are names.
    let share = |c: usize, test: &dyn Fn(&str) -> bool| {
        let cells: Vec<&str> = body
            .iter()
            .map(|r| r.text(c))
            .filter(|s| !s.is_empty())
            .collect();
        if cells.is_empty() {
            return 0.0;
        }
        cells.iter().filter(|s| test(s)).count() as f64 / cells.len() as f64
    };
    let is_numbers = |s: &str| numbers(s).is_some();
    let is_name = |s: &str| {
        let n = clean_name(s);
        !n.is_empty()
            && n.len() <= 24
            && n.chars().any(char::is_alphabetic)
            && numbers(&n).is_none()
    };
    for c in 0..t.columns {
        if roles[c] == Role::Number && share(c, &is_numbers) < 0.6 {
            roles[c] = if share(c, &is_name) > 0.7 && !roles.contains(&Role::Name) {
                Role::Name
            } else {
                Role::Other
            };
        }
    }
    if !roles.contains(&Role::Name) {
        // The column with the most name-like values.
        let best = (0..t.columns)
            .filter(|c| roles[*c] == Role::Other)
            .max_by(|a, b| share(*a, &is_name).total_cmp(&share(*b, &is_name)))?;
        if share(best, &is_name) < 0.6 {
            return None;
        }
        roles[best] = Role::Name;
    }
    if !roles.contains(&Role::Number) {
        for (c, role) in roles.iter_mut().enumerate() {
            if *role == Role::Other && share(c, &is_numbers) > 0.8 {
                *role = Role::Number;
            }
        }
    }
    let name_column = roles.iter().position(|r| *r == Role::Name)?;
    let number_columns: Vec<usize> = (0..t.columns)
        .filter(|c| roles[*c] == Role::Number)
        .collect();
    if number_columns.is_empty() {
        return None;
    }
    let type_column = roles.iter().position(|r| *r == Role::Type);
    let function_column = roles.iter().position(|r| *r == Role::Function);
    let mut columns: Vec<NumberColumn> = number_columns
        .iter()
        .map(|&c| {
            let header_cell = t.rows[..header_rows]
                .iter()
                .rev()
                .find_map(|r| r.cell(c).filter(|cell| !cell.text.is_empty()));
            let header_text = header_cell.map_or("", |cell| cell.text.as_str());
            let mut found = package::find_all(header_text);
            if found.is_empty() {
                found = package::find_all(&headers[c]);
            }
            if found.is_empty() {
                // A package word the recognition does not know (`LCCC`).
                let generic = [
                    "PIN", "PINS", "NO", "NO.", "NUMBER", "#", "BALL", "PAD", "TERMINAL",
                ];
                let rest: Vec<String> = normalize(header_text)
                    .split([' ', ','])
                    .map(|w| clean_name(w).trim_matches(['(', ')', '.']).to_owned())
                    .filter(|w| !w.is_empty() && !generic.contains(&w.as_str()))
                    .collect();
                if rest.len() == 1
                    && (2..=8).contains(&rest[0].len())
                    && rest[0].chars().all(|c| c.is_ascii_alphanumeric())
                {
                    found.push(Recognised {
                        family: Family::Other,
                        pins: None,
                        body: None,
                        wide: false,
                        exposed_pad: false,
                        canonical: rest[0].clone(),
                    });
                }
            }
            NumberColumn {
                packages: found,
                source: Source::at(t.page, header_cell.map_or(t.bbox, |cell| cell.bbox)),
                count: 0,
            }
        })
        .collect();
    let mut pins: Vec<PinDraft> = Vec::new();
    let mut raw_numbers: Vec<Vec<(Vec<String>, Option<String>)>> = Vec::new();
    for row in body {
        let name = clean_name(row.text(name_column));
        let cells: Vec<Option<(Vec<String>, Option<String>)>> = number_columns
            .iter()
            .map(|&c| numbers(row.text(c)))
            .collect();
        if name.is_empty() || keyword_count(&name, PIN_WORDS) > 0 && name.split(' ').count() > 1 {
            // A sub-row: more pad numbers of the pin above (another variant).
            if let Some(last) = raw_numbers.last_mut() {
                for (i, cell) in cells.into_iter().enumerate() {
                    if let Some((mut n, variant)) = cell
                        && !n.is_empty()
                    {
                        if let Some(v) = variant {
                            n = n.into_iter().map(|x| format!("{x}\u{1f}{v}")).collect();
                        }
                        last[i].0.extend(n);
                    }
                }
            }
            continue;
        }
        if !is_name(&name) {
            continue;
        }
        if cells.iter().all(Option::is_none) {
            continue;
        }
        let kind = type_column
            .and_then(|c| parse_type(row.text(c)))
            .unwrap_or_else(|| infer_type(&name));
        let kind = if kind == PinType::PowerIn
            && matches!(infer_type(&name), PinType::PowerOut | PinType::NotConnected)
        {
            infer_type(&name)
        } else {
            kind
        };
        let source = Some(Source::at(t.page, row.bbox));
        let numbers_of_row: Vec<(Vec<String>, Option<String>)> = cells
            .into_iter()
            .map(|c| {
                c.map(|(n, v)| match v {
                    Some(v) => (
                        n.into_iter().map(|x| format!("{x}\u{1f}{v}")).collect(),
                        None,
                    ),
                    None => (n, None),
                })
                .unwrap_or_default()
            })
            .collect();
        raw_numbers.push(numbers_of_row);
        pins.push(PinDraft {
            name,
            numbers: BTreeMap::new(),
            kind,
            function: function_column
                .map(|c| row.text(c).to_owned())
                .unwrap_or_default(),
            source,
        });
    }
    if pins.len() < 2 {
        return None;
    }
    // Pad counts per column, then the numbers per package (variants split
    // a package into one per variant).
    for (i, column) in columns.iter_mut().enumerate() {
        let mut highest = 0;
        let mut count = 0;
        for row in &raw_numbers {
            for n in &row[i].0 {
                count += 1;
                let base = n.split('\u{1f}').next().unwrap_or("");
                highest = highest.max(base.parse::<u32>().unwrap_or(0));
            }
        }
        column.count = highest.max(u32::try_from(count).unwrap_or(0));
    }
    for (pin, row) in pins.iter_mut().zip(&raw_numbers) {
        for (i, column) in columns.iter().enumerate() {
            for key in column_key(column) {
                for n in &row[i].0 {
                    let (number, variant) = n
                        .split_once('\u{1f}')
                        .map_or((n.as_str(), None), |(a, b)| (a, Some(b)));
                    let key = match variant {
                        Some(v) => format!("{key} ({v})"),
                        None => key.clone(),
                    };
                    pin.numbers.entry(key).or_default().push(number.to_owned());
                }
                if row[i].0.is_empty() {
                    // Present in the column but without a pad in it.
                    pin.numbers.entry(key.clone()).or_default();
                }
            }
        }
        for v in pin.numbers.values_mut() {
            v.sort_by(|a, b| pad_order(a, b));
            v.dedup();
        }
    }
    // A variant package also has the pins without variant numbers.
    let variants: BTreeSet<String> = pins
        .iter()
        .flat_map(|p| p.numbers.keys())
        .filter(|k| k.ends_with(')') && k.contains(" ("))
        .cloned()
        .collect();
    for pin in &mut pins {
        for v in &variants {
            let base = v.split(" (").next().unwrap_or("").to_owned();
            if let Some(n) = pin.numbers.get(&base).cloned()
                && !n.is_empty()
                && !pin.numbers.contains_key(v)
            {
                pin.numbers.insert(v.clone(), n);
            }
        }
    }
    if !variants.is_empty() {
        for pin in &mut pins {
            for v in &variants {
                let base = v.split(" (").next().unwrap_or("");
                if pin.numbers.get(base).is_some_and(Vec::is_empty) {
                    pin.numbers.remove(base);
                }
            }
            let bases: BTreeSet<String> = variants
                .iter()
                .map(|v| v.split(" (").next().unwrap_or("").to_owned())
                .collect();
            for b in bases {
                pin.numbers.remove(&b);
            }
        }
    }
    for pin in &mut pins {
        pin.numbers.retain(|_, v| !v.is_empty());
    }
    Some(PinTable { pins, columns })
}

fn pin_tables(tables: &[Table]) -> (Vec<PinDraft>, Vec<NumberColumn>) {
    let mut pins: Vec<PinDraft> = Vec::new();
    let mut columns: Vec<NumberColumn> = Vec::new();
    for t in tables {
        let Some(found) = read_pin_table(t) else {
            continue;
        };
        for pin in found.pins {
            match pins.iter_mut().find(|p| p.name == pin.name) {
                Some(existing) => {
                    for (k, v) in pin.numbers {
                        existing.numbers.entry(k).or_insert(v);
                    }
                    if existing.function.is_empty() {
                        existing.function = pin.function;
                    }
                }
                None => pins.push(pin),
            }
        }
        columns.extend(found.columns);
    }
    (pins, columns)
}

fn spread_unnamed(pins: &mut [PinDraft], packages: &[PackageDraft]) {
    let unnamed = pins.iter().any(|p| p.numbers.contains_key(""));
    if !unnamed {
        return;
    }
    let count = pins
        .iter()
        .filter(|p| !pins_named(&[p], &["EP", "PAD", "THERMAL", "POWERPAD"]))
        .filter_map(|p| p.numbers.get(""))
        .flatten()
        .filter_map(|n| n.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    let mut targets: Vec<String> = packages
        .iter()
        .filter(|p| !pins.iter().any(|pin| pin.numbers.contains_key(&p.name)))
        .filter(|p| p.pins == 0 || p.pins == count || p.exposed_pad && p.pins + 1 == count)
        .map(|p| p.name.clone())
        .collect();
    if targets.is_empty() {
        targets.push(format!("Package-{count}"));
    }
    for pin in pins {
        if let Some(n) = pin.numbers.remove("") {
            for t in &targets {
                pin.numbers.insert(t.clone(), n.clone());
            }
        }
    }
}

// ----- pinout drawings ---------------------------------------------------

/// Pins of a two-row pinout drawing (DIP, SOIC): pad numbers 1…N/2 down
/// the left side and N…N/2+1 down the right side, names outside.
/// Pins of a drawing: name, pad number and where the number is.
type DrawnPins = Vec<(String, String, Source)>;

fn pinout(sheet: &Datasheet) -> Option<(DrawnPins, u32)> {
    for page in sheet.pages.iter().take(6) {
        let text = normalize(
            &page
                .lines
                .iter()
                .map(|l| l.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        );
        if !(text.contains("TOP VIEW")
            || text.contains("PIN CONNECTION")
            || text.contains("PIN CONFIGURATION")
            || text.contains("PINOUT"))
        {
            continue;
        }
        let words: Vec<&Word> = page
            .lines
            .iter()
            .filter(|l| l.angle == 0)
            .flat_map(|l| l.words.iter())
            .collect();
        let numbered: Vec<(&Word, u32)> = words
            .iter()
            .filter_map(|w| {
                w.text
                    .parse::<u32>()
                    .ok()
                    .filter(|n| (1..=64).contains(n))
                    .map(|n| (*w, n))
            })
            .collect();
        // Columns of numbers by their x position.
        let mut columns: Vec<Vec<(&Word, u32)>> = Vec::new();
        for (w, n) in &numbered {
            let x = f64::midpoint(w.bbox[0], w.bbox[2]);
            match columns
                .iter_mut()
                .find(|c| (f64::midpoint(c[0].0.bbox[0], c[0].0.bbox[2]) - x).abs() < 4.0)
            {
                Some(c) => c.push((*w, *n)),
                None => columns.push(vec![(*w, *n)]),
            }
        }
        for left in &columns {
            let mut l = left.clone();
            l.sort_by(|a, b| a.0.bbox[1].total_cmp(&b.0.bbox[1]));
            let half = l.len() as u32;
            if half < 2 || l.iter().enumerate().any(|(i, (_, n))| *n != i as u32 + 1) {
                continue;
            }
            let total = half * 2;
            let Some(right) = columns.iter().find(|c| {
                let mut r = (*c).clone();
                r.sort_by(|a, b| a.0.bbox[1].total_cmp(&b.0.bbox[1]));
                r.len() as u32 == half
                    && r.iter()
                        .enumerate()
                        .all(|(i, (_, n))| *n == total - i as u32)
                    && r[0].0.bbox[0] > l[0].0.bbox[2]
            }) else {
                continue;
            };
            let mut out = Vec::new();
            let label = |number: &Word, outward_left: bool| -> String {
                let mut found: Vec<&Word> = words
                    .iter()
                    .copied()
                    .filter(|w| {
                        let overlap = w.bbox[3].min(number.bbox[3]) - w.bbox[1].max(number.bbox[1]);
                        overlap > 0.4 * (number.bbox[3] - number.bbox[1])
                            && if outward_left {
                                w.bbox[2] <= number.bbox[0]
                            } else {
                                w.bbox[0] >= number.bbox[2]
                            }
                            && w.text.parse::<u32>().is_err()
                    })
                    .collect();
                found.sort_by(|a, b| {
                    let da = if outward_left {
                        number.bbox[0] - a.bbox[2]
                    } else {
                        a.bbox[0] - number.bbox[2]
                    };
                    let db = if outward_left {
                        number.bbox[0] - b.bbox[2]
                    } else {
                        b.bbox[0] - number.bbox[2]
                    };
                    da.total_cmp(&db)
                });
                let mut picked: Vec<&Word> = Vec::new();
                let mut edge = if outward_left {
                    number.bbox[0]
                } else {
                    number.bbox[2]
                };
                for w in found {
                    let gap = if outward_left {
                        edge - w.bbox[2]
                    } else {
                        w.bbox[0] - edge
                    };
                    if gap > 1.5 * w.size.max(6.0) * if picked.is_empty() { 3.0 } else { 1.0 } {
                        break;
                    }
                    edge = if outward_left { w.bbox[0] } else { w.bbox[2] };
                    picked.push(w);
                }
                if outward_left {
                    picked.reverse();
                }
                picked
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            for (w, n) in &l {
                out.push((
                    clean_name(&label(w, true)),
                    n.to_string(),
                    Source::at(page.number, w.bbox),
                ));
            }
            let mut r = right.clone();
            r.sort_by(|a, b| a.0.bbox[1].total_cmp(&b.0.bbox[1]));
            for (w, n) in &r {
                out.push((
                    clean_name(&label(w, false)),
                    n.to_string(),
                    Source::at(page.number, w.bbox),
                ));
            }
            if out.iter().filter(|(name, ..)| !name.is_empty()).count() * 2 >= out.len() {
                out.sort_by(|a, b| pad_order(&a.1, &b.1));
                return Some((out, total));
            }
        }
    }
    None
}

// ----- packages ----------------------------------------------------------

fn body_of(p: &PackageDraft) -> Option<(f64, f64)> {
    package::recognise(&p.name).and_then(|r| r.body)
}

fn packages(sheet: &Datasheet, columns: &[NumberColumn], pins: &[PinDraft]) -> Vec<PackageDraft> {
    let mut out: Vec<PackageDraft> = Vec::new();
    let pins_with = |name: &str| -> Vec<&PinDraft> {
        pins.iter()
            .filter(|p| p.numbers.contains_key(name))
            .collect()
    };
    let add = |name: &str, r: &Recognised, source: Option<Source>, out: &mut Vec<PackageDraft>| {
        if out.iter().any(|p| p.name == name) {
            return;
        }
        let pins = r.pins.unwrap_or(0);
        out.push(PackageDraft {
            name: name.to_owned(),
            family: r.family,
            pins,
            exposed_pad: r.exposed_pad || pins_named(&pins_with(name), &["EP", "PAD", "THERMAL"]),
            dimensions: BTreeMap::new(),
            source,
            drawing: None,
        });
    };
    // Packages of the pin table columns, with their pad counts.
    let keys: BTreeSet<String> = pins
        .iter()
        .flat_map(|p| p.numbers.keys())
        .cloned()
        .collect();
    for key in &keys {
        if key.is_empty() {
            continue;
        }
        let base = key.split(" (").next().unwrap_or(key);
        let mut r = package::recognise(base).unwrap_or_else(|| Recognised {
            family: Family::Other,
            pins: base.rsplit_once('-').and_then(|(_, n)| n.parse().ok()),
            body: None,
            wide: false,
            exposed_pad: false,
            canonical: base.to_owned(),
        });
        let pads: BTreeSet<&String> = pins_with(key)
            .iter()
            .flat_map(|p| p.numbers.get(key).into_iter().flatten())
            .collect();
        let highest = pads
            .iter()
            .filter_map(|n| n.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        let ep = pins_named(&pins_with(key), &["EP", "PAD", "THERMAL"]);
        let count = if ep {
            highest.saturating_sub(1).max(r.pins.unwrap_or(0))
        } else {
            highest.max(u32::try_from(pads.len()).unwrap_or(0))
        };
        if r.pins.is_none() {
            r.pins = Some(count);
        }
        let source = columns
            .iter()
            .find(|c| c.packages.iter().any(|p| key.starts_with(&p.canonical)))
            .map(|c| c.source.clone());
        add(key, &r, source, &mut out);
    }
    if out.is_empty() {
        // Packages named in the text: captions, ordering tables.
        let mut seen: BTreeMap<String, (usize, Recognised, Source)> = BTreeMap::new();
        for page in &sheet.pages {
            for line in page.lines.iter().filter(|l| l.angle == 0) {
                if line.text.len() > 80 {
                    continue;
                }
                for r in package::find_all(&line.text) {
                    if r.pins.is_none() && !matches!(r.family, Family::Sop | Family::Dip) {
                        continue;
                    }
                    let e = seen.entry(r.canonical.clone()).or_insert((
                        0,
                        r.clone(),
                        Source::at(page.number, line.bbox),
                    ));
                    e.0 += 1;
                }
            }
        }
        let mut found: Vec<(usize, Recognised, Source)> = seen.into_values().collect();
        let counted: BTreeSet<Family> = found
            .iter()
            .filter(|f| f.1.pins.is_some())
            .map(|f| f.1.family)
            .collect();
        found.retain(|f| f.1.pins.is_some() || !counted.contains(&f.1.family));
        found.sort_by_key(|f| std::cmp::Reverse(f.0));
        for (_, r, source) in found.into_iter().take(6) {
            let name = r.canonical.clone();
            add(&name, &r, Some(source), &mut out);
        }
    }
    out
}

fn pins_named(pins: &[&PinDraft], names: &[&str]) -> bool {
    pins.iter().any(|p| {
        let n = normalize(&p.name);
        names
            .iter()
            .any(|x| n == *x || n.starts_with(&format!("{x} ")))
            || n.contains("THERMAL PAD")
            || n.contains("EXPOSED")
            || n.contains("POWERPAD")
    })
}

// ----- dimension tables --------------------------------------------------

/// A table of package dimensions.
#[derive(Debug, Clone, PartialEq)]
struct DimensionTable {
    page: usize,
    bbox: Rect,
    hint: Option<Recognised>,
    rows: Vec<(String, Dimension)>,
}

fn parse_value(text: &str) -> Option<f64> {
    let t = normalize(text);
    let t = t
        .replace("BSC", "")
        .replace("TYP.", "")
        .replace("TYP", "")
        .replace("REF.", "")
        .replace("REF", "")
        .replace(',', ".");
    let t = t.trim().trim_start_matches('(').trim_end_matches(')');
    if t.contains('°') {
        return None;
    }
    t.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0 && *v < 200.0)
}

fn is_symbol(text: &str) -> bool {
    let t = text.trim();
    (1..=4).contains(&t.chars().count())
        && t.chars().next().is_some_and(char::is_alphabetic)
        && t.chars().all(char::is_alphanumeric)
        && parse_value(t).is_none()
}

#[allow(clippy::too_many_lines, reason = "one pass over a table")]
fn read_dimension_table(t: &Table, page: &Page) -> Option<DimensionTable> {
    // The header row with MIN / MAX (and NOM or TYP).
    let header = t.rows.iter().position(|r| {
        let text = normalize(
            &r.cells
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        );
        let tokens: BTreeSet<&str> = text.split([' ', '.']).collect();
        tokens.contains("MIN") && (tokens.contains("MAX") || tokens.contains("NOM"))
    })?;
    let head = &t.rows[header];
    let mut groups: Vec<[Option<usize>; 3]> = Vec::new();
    for c in 0..t.columns {
        let h = normalize(head.text(c));
        let h = h.trim().trim_end_matches('.');
        let slot = match h {
            "MIN" | "MINIMUM" => 0,
            "NOM" | "TYP" | "NOMINAL" | "TYPICAL" => 1,
            "MAX" | "MAXIMUM" => 2,
            _ => continue,
        };
        if groups
            .last()
            .is_none_or(|g| g[slot].is_some() || g.iter().flatten().any(|x| *x > c && slot == 0))
        {
            groups.push([None; 3]);
        }
        if let Some(g) = groups.last_mut() {
            g[slot] = Some(c);
        }
    }
    if groups.is_empty() {
        return None;
    }
    // Units: a row above the header naming them per group, else the text
    // around the table.
    let unit_of = |c: usize| -> Option<bool> {
        for row in t.rows[..header].iter().rev() {
            let h = normalize(row.text(c));
            if h.contains("MILLIMETER") || h.contains("MM") {
                return Some(false);
            }
            if h.contains("INCH") || h.contains("MIL") {
                return Some(true);
            }
        }
        None
    };
    let mut chosen = groups[0];
    let mut inch = unit_of(chosen.iter().flatten().copied().next().unwrap_or(0));
    for g in &groups {
        let u = unit_of(g.iter().flatten().copied().next().unwrap_or(0));
        if u == Some(false) {
            chosen = *g;
            inch = u;
            break;
        }
    }
    if inch.is_none() {
        let around: String = t
            .rows
            .iter()
            .flat_map(|r| r.cells.iter())
            .map(|c| c.text.as_str())
            .chain(
                page.lines
                    .iter()
                    .filter(|l| l.bbox[1] > t.bbox[1] - 120.0 && l.bbox[1] < t.bbox[3] + 60.0)
                    .map(|l| l.text.as_str()),
            )
            .collect::<Vec<_>>()
            .join(" ");
        let n = normalize(&around);
        if n.contains("INCHES") && !n.contains("MILLIMETERS") && !n.contains(" MM") {
            inch = Some(true);
        }
    }
    let scale = if inch == Some(true) { 25.4 } else { 1.0 };
    // The symbol column: before the values, most cells look like symbols.
    let first_value = chosen.iter().flatten().copied().min().unwrap_or(0);
    let body = &t.rows[header + 1..];
    let symbol_column = (0..first_value)
        .rev()
        .max_by_key(|c| body.iter().filter(|r| is_symbol(r.text(*c))).count())?;
    let mut rows = Vec::new();
    for row in body {
        let symbol = row.text(symbol_column).trim().to_owned();
        if !is_symbol(&symbol) {
            continue;
        }
        let value = |slot: usize| {
            chosen[slot]
                .and_then(|c| parse_value(row.text(c)))
                .map(|v| v * scale)
        };
        let (min, nom, max) = (value(0), value(1), value(2));
        if min.is_none() && nom.is_none() && max.is_none() {
            continue;
        }
        rows.push((
            symbol,
            Dimension {
                min,
                nom,
                max,
                source: Some(Source::at(t.page, row.bbox)),
            },
        ));
    }
    if rows.len() < 3 {
        return None;
    }
    // The package: in the table above the header, else in the lines above
    // the table.
    let above: String = t.rows[..header]
        .iter()
        .flat_map(|r| r.cells.iter())
        .map(|c| c.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let mut hint = package::recognise(&above);
    if hint.is_none() {
        let mut lines: Vec<_> = page
            .lines
            .iter()
            .filter(|l| {
                l.angle == 0 && l.bbox[3] <= t.bbox[1] + 2.0 && l.bbox[3] > t.bbox[1] - 200.0
            })
            .collect();
        lines.sort_by(|a, b| b.bbox[3].total_cmp(&a.bbox[3]));
        hint = lines.iter().find_map(|l| package::recognise(&l.text));
    }
    Some(DimensionTable {
        page: t.page,
        bbox: t.bbox,
        hint,
        rows,
    })
}

fn dimension_tables(sheet: &Datasheet, tables: &[Table]) -> Vec<DimensionTable> {
    tables
        .iter()
        .filter_map(|t| {
            let page = sheet.pages.get(t.page.checked_sub(1)?)?;
            read_dimension_table(t, page)
        })
        .collect()
}

/// Maps the symbols of a table to JEDEC symbols: directly when the table
/// uses them, else by matching the values against the standard
/// dimensions of the package.
fn map_symbols(
    table: &DimensionTable,
    standard: &BTreeMap<String, Dimension>,
) -> BTreeMap<String, Dimension> {
    let symbols: BTreeSet<&str> = table.rows.iter().map(|(s, _)| s.as_str()).collect();
    let jedec = symbols.contains("e")
        || symbols.contains("E1")
        || symbols.contains("A1")
        || symbols.contains("bp")
        || symbols.contains("HE");
    let mut out = BTreeMap::new();
    if jedec {
        let swap_e = symbols.contains("HE");
        let swap_d = symbols.contains("HD");
        for (s, d) in &table.rows {
            let k = match s.as_str() {
                "E" if swap_e => "E1",
                "HE" => "E",
                "D" if swap_d => "D1",
                "HD" => "D",
                "bp" | "B" => "b",
                "Lp" => "L",
                other => other,
            };
            out.entry(k.to_owned()).or_insert_with(|| d.clone());
        }
        // Body narrower than the lead span.
        if let (Some(e), Some(e1)) = (
            out.get("E").and_then(Dimension::value),
            out.get("E1").and_then(Dimension::value),
        ) && e1 > e
            && let (Some(a), Some(b)) = (out.remove("E"), out.remove("E1"))
        {
            out.insert("E".into(), b);
            out.insert("E1".into(), a);
        }
        return out;
    }
    // Matching by relative difference: greedy, then improved by swaps. A
    // table letter that is also the JEDEC letter counts a little closer.
    let rows: Vec<(usize, f64, &str)> = table
        .rows
        .iter()
        .enumerate()
        .filter_map(|(i, (s, d))| d.value().map(|v| (i, v, s.as_str())))
        .collect();
    let targets: Vec<(&String, f64)> = standard
        .iter()
        .filter_map(|(k, d)| d.value().map(|v| (k, v)))
        .collect();
    let cost = |r: usize, t: usize| {
        let (_, v, symbol) = rows[r];
        let (key, sv) = targets[t];
        let bonus = if symbol.eq_ignore_ascii_case(key) {
            0.05
        } else {
            0.0
        };
        (v - sv).abs() / sv.max(0.3) - bonus
    };
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for r in 0..rows.len() {
        for t in 0..targets.len() {
            pairs.push((cost(r, t), r, t));
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut assigned: Vec<Option<usize>> = vec![None; rows.len()];
    let mut taken = vec![false; targets.len()];
    for (c, r, t) in pairs {
        if c > 0.25 || assigned[r].is_some() || taken[t] {
            continue;
        }
        assigned[r] = Some(t);
        taken[t] = true;
    }
    for _ in 0..20 {
        let mut improved = false;
        for a in 0..rows.len() {
            for b in a + 1..rows.len() {
                if let (Some(ta), Some(tb)) = (assigned[a], assigned[b]) {
                    let now = cost(a, ta) + cost(b, tb);
                    let swapped = cost(a, tb) + cost(b, ta);
                    if swapped + 1e-9 < now && cost(a, tb) <= 0.25 && cost(b, ta) <= 0.25 {
                        assigned[a] = Some(tb);
                        assigned[b] = Some(ta);
                        improved = true;
                    }
                }
            }
        }
        if !improved {
            break;
        }
    }
    for (r, t) in assigned.iter().enumerate() {
        if let Some(t) = t {
            out.insert(targets[*t].0.clone(), table.rows[rows[r].0].1.clone());
        }
    }
    out
}

fn attach(packages: &mut [PackageDraft], table: &DimensionTable) {
    let candidates: Vec<usize> = (0..packages.len())
        .filter(|&i| {
            let p = &packages[i];
            match &table.hint {
                Some(h) => {
                    h.family == p.family
                        && (h.pins.is_none() || p.pins == 0 || h.pins == Some(p.pins))
                        && p.family != Family::Other
                }
                None => p.family != Family::Other,
            }
        })
        .collect();
    let best = candidates.into_iter().min_by(|&a, &b| {
        let cost = |i: usize| {
            let p = &packages[i];
            let r = Recognised {
                family: p.family,
                pins: (p.pins > 0).then_some(p.pins),
                body: body_of(p),
                wide: false,
                exposed_pad: p.exposed_pad,
                canonical: package::recognise(&p.name)
                    .map_or_else(|| p.name.clone(), |r| r.canonical),
            };
            let standard = package::standard(&r);
            let mapped = map_symbols(table, &standard);
            let mut total = 0.0;
            let mut n = 0.0;
            for (k, d) in &mapped {
                if let (Some(v), Some(s)) = (d.value(), standard.get(k).and_then(Dimension::value))
                {
                    total += (v - s).abs() / s.max(0.3);
                    n += 1.0;
                }
            }
            if n == 0.0 {
                f64::INFINITY
            } else {
                total / n - n * 0.01 + if p.dimensions.is_empty() { 0.0 } else { 0.5 }
            }
        };
        cost(a).total_cmp(&cost(b))
    });
    let Some(i) = best else {
        return;
    };
    let p = &mut packages[i];
    let r = Recognised {
        family: p.family,
        pins: (p.pins > 0).then_some(p.pins),
        body: body_of(p),
        wide: false,
        exposed_pad: p.exposed_pad,
        canonical: package::recognise(&p.name).map_or_else(|| p.name.clone(), |r| r.canonical),
    };
    let standard = package::standard(&r);
    for (k, d) in map_symbols(table, &standard) {
        p.dimensions.insert(k, d);
    }
    p.drawing = Some(Source::at(table.page, table.bbox));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_pad_number_cells() {
        assert_eq!(numbers("1"), Some((vec!["1".to_owned()], None)));
        assert_eq!(
            numbers("6, 7").map(|n| n.0),
            Some(vec!["6".to_owned(), "7".to_owned()])
        );
        assert_eq!(numbers("9–11").map(|n| n.0.len()), Some(3));
        assert_eq!(
            numbers("A1, B2").map(|n| n.0),
            Some(vec!["A1".to_owned(), "B2".to_owned()])
        );
        assert_eq!(
            numbers("3 (R5)"),
            Some((vec!["3".to_owned()], Some("R5".to_owned())))
        );
        assert_eq!(numbers("5(1)"), Some((vec!["5".to_owned()], None)));
        assert_eq!(numbers("—"), Some((Vec::new(), None)));
        assert_eq!(numbers("Output"), None);
    }

    #[test]
    fn infers_pin_types_from_names() {
        assert_eq!(infer_type("VCC"), PinType::PowerIn);
        assert_eq!(infer_type("GND"), PinType::PowerIn);
        assert_eq!(infer_type("NC"), PinType::NotConnected);
        assert_eq!(infer_type("OUT1"), PinType::Output);
        assert_eq!(infer_type("IN1-"), PinType::Input);
        assert_eq!(infer_type("PA7"), PinType::Bidirectional);
        assert_eq!(infer_type("VOUT"), PinType::PowerOut);
    }

    #[test]
    fn reads_type_cells() {
        assert_eq!(parse_type("I/O"), Some(PinType::Bidirectional));
        assert_eq!(parse_type("O"), Some(PinType::Output));
        assert_eq!(parse_type("P"), Some(PinType::PowerIn));
        assert_eq!(parse_type("—"), None);
    }

    #[test]
    fn maps_vendor_letters_by_value() {
        let table = DimensionTable {
            page: 1,
            bbox: [0.0; 4],
            hint: None,
            rows: [
                ("A", 0.38),
                ("B", 1.6),
                ("C", 2.8),
                ("D", 0.95),
                ("H", 3.0),
                ("J", 0.05),
                ("K", 1.1),
                ("L", 0.4),
                ("M", 0.15),
            ]
            .iter()
            .map(|(s, v)| ((*s).to_owned(), Dimension::nominal(*v)))
            .collect(),
        };
        let standard =
            package::standard(&package::recognise("SOT25").unwrap_or_else(|| panic!("no package")));
        let mapped = map_symbols(&table, &standard);
        let value = |k: &str| mapped.get(k).and_then(Dimension::value);
        assert_eq!(value("e"), Some(0.95));
        assert_eq!(value("E"), Some(2.8));
        assert_eq!(value("E1"), Some(1.6));
        assert_eq!(value("D"), Some(3.0));
        assert_eq!(value("b"), Some(0.38));
        assert_eq!(value("A1"), Some(0.05));
    }
}
