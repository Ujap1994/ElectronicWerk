//! Bills of materials: one line per group of equal parts, with the columns
//! the user picks, as CSV, Excel workbook (XLSX) or a stand-alone HTML page.
//!
//! Parts come from the schematic (every symbol with a designator, power
//! symbols left out); the board adds the side and mounting type of each
//! part. A project without a schematic takes its parts from the board.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ew_model::document::Document;
use ew_model::layer::Side;
use ew_model::pcb::{Mounting, Pcb};
use ew_model::project::Project;
use ew_schematic::connectivity::natural_cmp;
use serde::{Deserialize, Serialize};

use crate::xlsx;

/// One part of the design.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Part {
    /// Designator.
    pub designator: String,
    /// Value.
    pub value: String,
    /// Footprint name.
    pub footprint: String,
    /// Symbol name.
    pub symbol: String,
    /// Description of the symbol.
    pub description: String,
    /// Further fields (manufacturer, part number, …).
    pub properties: BTreeMap<String, String>,
    /// Not placed.
    pub dnp: bool,
    /// Board side, if the part is on the board.
    pub side: Option<Side>,
    /// Mounting type, if the part is on the board.
    pub mounting: Option<Mounting>,
}

/// The parts of a project for the bill of materials, sorted by designator.
/// Parts marked "exclude from the bill of materials" are left out.
#[must_use]
pub fn parts(project: &Project, board: Option<&Pcb>) -> Vec<Part> {
    let on_board: BTreeMap<&str, &ew_model::pcb::FootprintInstance> = board
        .map(|pcb| {
            pcb.footprints
                .iter()
                .map(|fp| (fp.designator.as_str(), fp))
                .collect()
        })
        .unwrap_or_default();
    let has_schematic = project
        .documents
        .values()
        .any(|d| matches!(d, Document::Schematic(_)));
    let mut out: Vec<Part> = if has_schematic {
        let connectivity = ew_schematic::analyze(project);
        ew_schematic::export::components(project, &connectivity)
            .into_iter()
            .filter(|c| !c.exclude_from_bom)
            .map(|c| {
                let fp = on_board.get(c.designator.as_str());
                Part {
                    designator: c.designator,
                    value: c.value,
                    footprint: c.footprint,
                    symbol: c.symbol,
                    description: c.description,
                    properties: c.properties,
                    dnp: c.dnp || fp.is_some_and(|f| f.attributes.dnp),
                    side: fp.map(|f| f.side),
                    mounting: fp.map(|f| f.attributes.mounting),
                }
            })
            .collect()
    } else {
        on_board
            .values()
            .filter(|fp| !fp.attributes.exclude_from_bom)
            .map(|fp| Part {
                designator: fp.designator.clone(),
                value: fp.value.clone(),
                footprint: fp.footprint.clone(),
                symbol: String::new(),
                description: String::new(),
                properties: fp.properties.clone(),
                dnp: fp.attributes.dnp,
                side: Some(fp.side),
                mounting: Some(fp.attributes.mounting),
            })
            .collect()
    };
    out.sort_by(|a, b| natural_cmp(&a.designator, &b.designator));
    out
}

/// A column of the bill of materials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Column {
    /// What the column shows: `item`, `quantity`, `designators`, `value`,
    /// `footprint`, `symbol`, `description`, `side`, `mounting`, `dnp`, or
    /// the name of a field of the parts (for example `MPN`).
    pub field: String,
    /// Heading; empty uses the default heading of the field.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
}

impl Column {
    /// A column with the default heading.
    #[must_use]
    pub fn of(field: &str) -> Self {
        Self {
            field: field.to_owned(),
            label: String::new(),
        }
    }

    /// The heading.
    #[must_use]
    pub fn heading(&self) -> String {
        if !self.label.is_empty() {
            return self.label.clone();
        }
        match self.field.as_str() {
            "item" => "Item",
            "quantity" => "Qty",
            "designators" => "Designators",
            "value" => "Value",
            "footprint" => "Footprint",
            "symbol" => "Symbol",
            "description" => "Description",
            "side" => "Side",
            "mounting" => "Mounting",
            "dnp" => "DNP",
            other => other,
        }
        .to_owned()
    }
}

/// Options of the bill of materials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BomOptions {
    /// Columns in order.
    pub columns: Vec<Column>,
    /// Fields whose values must be equal for parts to share a line; empty
    /// lists every part on its own line.
    pub group_by: Vec<String>,
    /// List parts that are not placed (marked in the `dnp` column).
    pub include_dnp: bool,
    /// Field separator of CSV files.
    pub separator: char,
}

impl Default for BomOptions {
    fn default() -> Self {
        Self {
            columns: [
                "item",
                "quantity",
                "designators",
                "value",
                "footprint",
                "description",
                "Manufacturer",
                "MPN",
                "dnp",
            ]
            .into_iter()
            .map(Column::of)
            .collect(),
            group_by: ["value", "footprint", "Manufacturer", "MPN", "dnp"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            include_dnp: true,
            separator: ',',
        }
    }
}

/// A cell of the table.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    /// Text.
    Text(String),
    /// A number.
    Number(f64),
}

impl Cell {
    /// The cell as text.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::Text(t) => t.clone(),
            Self::Number(n) => {
                if n.fract() == 0.0 {
                    format!("{n:.0}")
                } else {
                    n.to_string()
                }
            }
        }
    }
}

/// The bill of materials as a table.
#[derive(Debug, Clone, PartialEq)]
pub struct Bom {
    /// Headings.
    pub headers: Vec<String>,
    /// Lines.
    pub rows: Vec<Vec<Cell>>,
    /// Number of parts listed.
    pub parts: usize,
}

/// The value of a field of a part (fields compare without case).
fn field(part: &Part, name: &str) -> String {
    match name {
        "designator" | "designators" => part.designator.clone(),
        "value" => part.value.clone(),
        "footprint" => part.footprint.clone(),
        "symbol" => part.symbol.clone(),
        "description" => part.description.clone(),
        "side" => match part.side {
            Some(Side::Top) => "Top".to_owned(),
            Some(Side::Bottom) => "Bottom".to_owned(),
            None => String::new(),
        },
        "mounting" => match part.mounting {
            Some(Mounting::Smd) => "SMD".to_owned(),
            Some(Mounting::ThroughHole) => "THT".to_owned(),
            Some(Mounting::Other) | None => String::new(),
        },
        "dnp" => if part.dnp { "DNP" } else { "" }.to_owned(),
        other => part
            .properties
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(other))
            .map(|(_, v)| v.clone())
            .unwrap_or_default(),
    }
}

/// Groups the parts and fills the columns.
#[must_use]
pub fn build(parts: &[Part], options: &BomOptions) -> Bom {
    let mut groups: Vec<(Vec<String>, Vec<&Part>)> = Vec::new();
    let mut index: BTreeMap<Vec<String>, usize> = BTreeMap::new();
    for part in parts.iter().filter(|p| options.include_dnp || !p.dnp) {
        if options.group_by.is_empty() {
            groups.push((Vec::new(), vec![part]));
            continue;
        }
        let key: Vec<String> = options.group_by.iter().map(|g| field(part, g)).collect();
        if let Some(i) = index.get(&key) {
            groups[*i].1.push(part);
        } else {
            index.insert(key.clone(), groups.len());
            groups.push((key, vec![part]));
        }
    }
    for (_, members) in &mut groups {
        members.sort_by(|a, b| natural_cmp(&a.designator, &b.designator));
    }
    groups.sort_by(|a, b| natural_cmp(&a.1[0].designator, &b.1[0].designator));
    let headers = options.columns.iter().map(Column::heading).collect();
    let mut count = 0;
    let rows = groups
        .iter()
        .enumerate()
        .map(|(i, (_, members))| {
            count += members.len();
            options
                .columns
                .iter()
                .map(|column| match column.field.as_str() {
                    "item" => Cell::Number((i + 1) as f64),
                    "quantity" => Cell::Number(members.len() as f64),
                    "designators" => Cell::Text(
                        members
                            .iter()
                            .map(|p| p.designator.as_str())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                    other => Cell::Text(field(members[0], other)),
                })
                .collect()
        })
        .collect();
    Bom {
        headers,
        rows,
        parts: count,
    }
}

/// A CSV field, quoted when needed.
#[must_use]
pub fn csv_field(text: &str, separator: char) -> String {
    if text.contains(separator) || text.contains('"') || text.contains('\n') || text.contains('\r')
    {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// The table as CSV (UTF-8 with a byte order mark, so spreadsheet programs
/// read umlauts and µ correctly).
#[must_use]
pub fn csv(bom: &Bom, separator: char) -> String {
    let mut out = String::from("\u{feff}");
    let line = |cells: Vec<String>| -> String {
        cells
            .iter()
            .map(|c| csv_field(c, separator))
            .collect::<Vec<_>>()
            .join(&separator.to_string())
    };
    out.push_str(&line(bom.headers.clone()));
    out.push_str("\r\n");
    for row in &bom.rows {
        out.push_str(&line(row.iter().map(Cell::text).collect()));
        out.push_str("\r\n");
    }
    out
}

/// Escapes text for HTML and XML.
#[must_use]
pub fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if c.is_control() && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

/// Title lines of an HTML bill of materials.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Heading {
    /// Title, for example the project name.
    pub title: String,
    /// Further lines (revision, date, …) as label and value.
    pub details: Vec<(String, String)>,
}

/// The table as a stand-alone HTML page (light and dark, printable).
#[must_use]
pub fn html(bom: &Bom, heading: &Heading) -> String {
    let mut out = String::new();
    let title = escape_xml(&heading.title);
    let _ = write!(
        out,
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{title} – Bill of materials</title>\n<style>\n\
         :root {{ color-scheme: light dark; --fg: #1d232b; --muted: #5d6875; --line: #d9dee5; --head: #eef2f6; --stripe: #f7f9fb; }}\n\
         @media (prefers-color-scheme: dark) {{ :root {{ --fg: #e3e8ee; --muted: #9aa5b1; --line: #36404c; --head: #222a33; --stripe: #1a2028; }} }}\n\
         body {{ font: 14px/1.45 system-ui, sans-serif; color: var(--fg); margin: 24px; }}\n\
         h1 {{ font-size: 20px; margin: 0 0 4px; }}\n\
         .meta {{ color: var(--muted); margin: 0 0 16px; }}\n\
         .meta span + span::before {{ content: ' · '; }}\n\
         table {{ border-collapse: collapse; width: 100%; }}\n\
         th, td {{ border: 1px solid var(--line); padding: 4px 8px; text-align: left; vertical-align: top; }}\n\
         th {{ background: var(--head); position: sticky; top: 0; }}\n\
         tbody tr:nth-child(even) {{ background: var(--stripe); }}\n\
         td.n {{ text-align: right; font-variant-numeric: tabular-nums; }}\n\
         @media print {{ body {{ margin: 0; }} th {{ position: static; }} }}\n\
         </style>\n</head>\n<body>\n<h1>{title}</h1>\n<p class=\"meta\">"
    );
    let _ = write!(
        out,
        "<span>{} lines</span><span>{} parts</span>",
        bom.rows.len(),
        bom.parts
    );
    for (label, value) in &heading.details {
        let _ = write!(
            out,
            "<span>{}: {}</span>",
            escape_xml(label),
            escape_xml(value)
        );
    }
    out.push_str("</p>\n<table>\n<thead><tr>");
    for header in &bom.headers {
        let _ = write!(out, "<th>{}</th>", escape_xml(header));
    }
    out.push_str("</tr></thead>\n<tbody>\n");
    for row in &bom.rows {
        out.push_str("<tr>");
        for cell in row {
            match cell {
                Cell::Number(_) => {
                    let _ = write!(out, "<td class=\"n\">{}</td>", cell.text());
                }
                Cell::Text(t) => {
                    let _ = write!(out, "<td>{}</td>", escape_xml(t));
                }
            }
        }
        out.push_str("</tr>\n");
    }
    out.push_str("</tbody>\n</table>\n</body>\n</html>\n");
    out
}

/// The table as an Excel workbook.
#[must_use]
pub fn workbook(bom: &Bom, sheet: &str) -> Vec<u8> {
    xlsx::workbook(sheet, &bom.headers, &bom.rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(designator: &str, value: &str, mpn: &str) -> Part {
        Part {
            designator: designator.into(),
            value: value.into(),
            footprint: "R_0603".into(),
            properties: [("MPN".to_owned(), mpn.to_owned())].into_iter().collect(),
            ..Part::default()
        }
    }

    #[test]
    fn groups_equal_parts() {
        let parts = vec![
            part("R10", "10k", "A"),
            part("R2", "10k", "A"),
            part("R1", "1k", "B"),
            Part {
                dnp: true,
                ..part("R3", "10k", "A")
            },
        ];
        let bom = build(&parts, &BomOptions::default());
        let text: Vec<Vec<String>> = bom
            .rows
            .iter()
            .map(|r| r.iter().map(Cell::text).collect())
            .collect();
        assert_eq!(text.len(), 3);
        assert_eq!(text[0][..4], ["1", "1", "R1", "1k"]);
        assert_eq!(text[1][..4], ["2", "2", "R2, R10", "10k"]);
        assert_eq!(text[2][2], "R3");
        assert_eq!(text[2][8], "DNP");
        assert_eq!(bom.parts, 4);
        let without = build(
            &parts,
            &BomOptions {
                include_dnp: false,
                group_by: Vec::new(),
                ..BomOptions::default()
            },
        );
        assert_eq!(without.rows.len(), 3);
    }

    #[test]
    fn csv_quotes_and_html_escapes() {
        let bom = Bom {
            headers: vec!["A".into(), "B".into()],
            rows: vec![vec![
                Cell::Text("x, \"y\"".into()),
                Cell::Text("<10µF>".into()),
            ]],
            parts: 1,
        };
        let text = csv(&bom, ',');
        assert!(text.starts_with('\u{feff}'));
        assert!(text.contains("\"x, \"\"y\"\"\",<10µF>\r\n"));
        let page = html(
            &bom,
            &Heading {
                title: "Demo & co".into(),
                details: vec![("Revision".into(), "B".into())],
            },
        );
        assert!(page.contains("<h1>Demo &amp; co</h1>"));
        assert!(page.contains("<td>&lt;10µF&gt;</td>"));
        assert!(page.contains("Revision: B"));
    }
}
