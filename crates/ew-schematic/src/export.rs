//! Exports of the schematic data: netlists (KiCad format and CSV) and a
//! preliminary bill of materials (CSV).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use ew_model::document::Document;
use ew_model::library::PinKind;
use ew_model::project::Project;

use crate::connectivity::{Connectivity, natural_cmp, symbol_def};

/// One component (all units of one designator).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Component {
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
    /// Parameters.
    pub properties: BTreeMap<String, String>,
    /// Do not place.
    pub dnp: bool,
    /// Left out of the bill of materials.
    pub exclude_from_bom: bool,
}

/// Every component of the project (power symbols are left out), sorted by
/// designator; sheets used more than once give one component per use.
#[must_use]
pub fn components(project: &Project, connectivity: &Connectivity) -> Vec<Component> {
    let mut out: BTreeMap<String, Component> = BTreeMap::new();
    for inst in &connectivity.instances {
        let Some(Document::Schematic(sheet)) = project.document(inst.sheet) else {
            continue;
        };
        for symbol in &sheet.symbols {
            let def = symbol_def(project, &symbol.symbol);
            let designator = symbol.designator_in(&inst.key);
            if def.is_some_and(|d| d.power) || designator.starts_with('#') {
                continue;
            }
            out.entry(designator.to_owned())
                .or_insert_with(|| Component {
                    designator: designator.to_owned(),
                    value: symbol.value.clone(),
                    footprint: symbol.footprint.clone().unwrap_or_default(),
                    symbol: symbol.symbol.clone(),
                    description: def.map(|d| d.description.clone()).unwrap_or_default(),
                    properties: symbol.properties.clone(),
                    dnp: symbol.flags.dnp,
                    exclude_from_bom: symbol.flags.exclude_from_bom,
                });
        }
    }
    let mut list: Vec<Component> = out.into_values().collect();
    list.sort_by(|a, b| natural_cmp(&a.designator, &b.designator));
    list
}

/// KiCad's name of a pin type.
const fn kicad_pin_type(kind: PinKind) -> &'static str {
    match kind {
        PinKind::Input => "input",
        PinKind::Output => "output",
        PinKind::Bidirectional => "bidirectional",
        PinKind::TriState => "tri_state",
        PinKind::Passive => "passive",
        PinKind::Free => "free",
        PinKind::Unspecified => "unspecified",
        PinKind::PowerIn => "power_in",
        PinKind::PowerOut => "power_out",
        PinKind::OpenCollector => "open_collector",
        PinKind::OpenEmitter => "open_emitter",
        PinKind::NoConnect => "no_connect",
    }
}

fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// A KiCad netlist (`.net`, version E) that KiCad's board editor and other
/// tools can read.
#[must_use]
pub fn kicad_netlist(project: &Project, connectivity: &Connectivity) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "(export (version \"E\")");
    let _ = writeln!(
        out,
        "  (design (source {}) (tool \"ElectronicWerk {}\"))",
        quote(&project.manifest.name),
        env!("CARGO_PKG_VERSION")
    );
    let _ = writeln!(out, "  (components");
    for c in components(project, connectivity) {
        let _ = write!(
            out,
            "    (comp (ref {}) (value {})",
            quote(&c.designator),
            quote(&c.value)
        );
        if !c.footprint.is_empty() {
            let _ = write!(out, " (footprint {})", quote(&c.footprint));
        }
        let _ = writeln!(out, " (libsource (lib \"\") (part {})))", quote(&c.symbol));
    }
    let _ = writeln!(out, "  )");
    let _ = writeln!(out, "  (nets");
    let mut code = 0;
    let mut nets: Vec<&crate::connectivity::Net> = connectivity.nets.iter().collect();
    nets.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    for net in nets {
        let mut nodes: Vec<(&str, &str, String)> = net
            .pins
            .iter()
            .map(|&p| &connectivity.pins[p])
            .filter(|p| !p.power_symbol)
            .map(|p| {
                (
                    p.designator.as_str(),
                    p.number.as_str(),
                    kicad_pin_type(p.kind).to_owned(),
                )
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if nodes.is_empty() {
            continue;
        }
        nodes.sort_by(|a, b| natural_cmp(a.0, b.0).then_with(|| natural_cmp(a.1, b.1)));
        code += 1;
        let _ = writeln!(
            out,
            "    (net (code \"{code}\") (name {})",
            quote(&net.name)
        );
        for (designator, number, kind) in nodes {
            let _ = writeln!(
                out,
                "      (node (ref {}) (pin {}) (pintype {}))",
                quote(designator),
                quote(number),
                quote(&kind)
            );
        }
        let _ = writeln!(out, "    )");
    }
    let _ = writeln!(out, "  )");
    let _ = writeln!(out, ")");
    out
}

fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\n', ';']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_owned()
    }
}

/// A netlist as CSV: one line per pin (`Net,Designator,Pin`).
#[must_use]
pub fn csv_netlist(connectivity: &Connectivity) -> String {
    let mut out = String::from("Net,Designator,Pin\n");
    let mut lines: Vec<(String, String, String)> = Vec::new();
    for (net, nodes) in connectivity.netlist() {
        for (designator, pin) in nodes {
            lines.push((net.clone(), designator, pin));
        }
    }
    lines.sort_by(|a, b| {
        natural_cmp(&a.0, &b.0)
            .then_with(|| natural_cmp(&a.1, &b.1))
            .then_with(|| natural_cmp(&a.2, &b.2))
    });
    for (net, designator, pin) in lines {
        let _ = writeln!(
            out,
            "{},{},{}",
            csv_field(&net),
            csv_field(&designator),
            csv_field(&pin)
        );
    }
    out
}

/// Parameters that get their own column in the bill of materials.
const BOM_COLUMNS: &[&str] = &["Manufacturer", "MPN"];

/// The bill of materials as CSV, one line per group of equal parts (same
/// value, footprint, symbol and part number). Parts marked "do not place"
/// are listed in their own lines.
#[must_use]
pub fn bom_csv(project: &Project, connectivity: &Connectivity) -> String {
    type Key = (String, String, String, Vec<String>, bool);
    let mut groups: BTreeMap<Key, (Vec<String>, String)> = BTreeMap::new();
    for c in components(project, connectivity) {
        if c.exclude_from_bom {
            continue;
        }
        let extra: Vec<String> = BOM_COLUMNS
            .iter()
            .map(|k| {
                c.properties
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(k))
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default()
            })
            .collect();
        let key = (
            c.value.clone(),
            c.footprint.clone(),
            c.symbol.clone(),
            extra,
            c.dnp,
        );
        let entry = groups
            .entry(key)
            .or_insert_with(|| (Vec::new(), c.description.clone()));
        entry.0.push(c.designator);
    }
    let mut rows: Vec<(Key, Vec<String>, String)> = groups
        .into_iter()
        .map(|(key, (mut designators, description))| {
            designators.sort_by(|a, b| natural_cmp(a, b));
            (key, designators, description)
        })
        .collect();
    rows.sort_by(|a, b| natural_cmp(&a.1[0], &b.1[0]));
    let mut out = String::from("Item,Quantity,Designators,Value,Footprint,Symbol,Description");
    for column in BOM_COLUMNS {
        let _ = write!(out, ",{column}");
    }
    out.push_str(",DNP\n");
    for (index, ((value, footprint, symbol, extra, dnp), designators, description)) in
        rows.into_iter().enumerate()
    {
        let _ = write!(
            out,
            "{},{},{},{},{},{},{}",
            index + 1,
            designators.len(),
            csv_field(&designators.join(" ")),
            csv_field(&value),
            csv_field(&footprint),
            csv_field(&symbol),
            csv_field(&description)
        );
        for value in extra {
            let _ = write!(out, ",{}", csv_field(&value));
        }
        let _ = writeln!(out, ",{}", if dnp { "DNP" } else { "" });
    }
    out
}
