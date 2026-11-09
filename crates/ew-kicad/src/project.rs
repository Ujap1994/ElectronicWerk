//! KiCad project settings (`.kicad_pro`, JSON) and library tables.

use std::collections::BTreeMap;

use ew_model::rules::{
    Constraints, DEFAULT_NET_CLASS, ERC_PIN_MATRIX, ErcSettings, ErcSeverity, NetClass,
    NetClassAssignment,
};
use serde_json::Value;

use crate::common::nm;
use crate::sexpr::Sexp;

/// What the importer uses from a `.kicad_pro` file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectSettings {
    /// Text variables.
    pub variables: BTreeMap<String, String>,
    /// Board design rules.
    pub constraints: Option<Constraints>,
    /// Net classes (the default class first).
    pub net_classes: Vec<NetClass>,
    /// Net class patterns.
    pub assignments: Vec<NetClassAssignment>,
    /// Electrical rules check settings (changes from the defaults).
    pub erc: ErcSettings,
    /// Number of ERC findings marked as excluded (not imported).
    pub erc_exclusions: usize,
}

/// KiCad's ERC checks and the checks of ElectronicWerk they set.
const KICAD_ERC: &[(&str, &[&str])] = &[
    ("power_pin_not_driven", &["power-not-driven"]),
    ("pin_not_driven", &["input-not-driven"]),
    ("pin_not_connected", &["unconnected-pin"]),
    ("no_connect_connected", &["no-connect-connected"]),
    ("no_connect_dangling", &["no-connect-dangling"]),
    ("unconnected_wire_endpoint", &["dangling-wire"]),
    ("label_dangling", &["dangling-label"]),
    (
        "hier_label_mismatch",
        &["hierarchical-label-unmatched", "sheet-pin-unmatched"],
    ),
    ("single_global_label", &["single-global-label"]),
    ("multiple_net_names", &["net-name-conflict"]),
    ("duplicate_reference", &["duplicate-designator"]),
    ("unannotated", &["unannotated"]),
    ("endpoint_off_grid", &["off-grid"]),
];

fn severity(value: &Value) -> Option<ErcSeverity> {
    match value.as_str()? {
        "error" => Some(ErcSeverity::Error),
        "warning" => Some(ErcSeverity::Warning),
        "ignore" => Some(ErcSeverity::Ignore),
        _ => None,
    }
}

/// Reads the ERC settings (`erc.rule_severities`, `erc.pin_map`).
fn erc_settings(erc: &Value) -> ErcSettings {
    let mut severities = BTreeMap::new();
    if let Some(rules) = erc.get("rule_severities").and_then(Value::as_object) {
        for (kicad, codes) in KICAD_ERC {
            if let Some(value) = rules.get(*kicad).and_then(severity) {
                for code in *codes {
                    severities.insert((*code).to_owned(), value);
                }
            }
        }
        // KiCad's pin_to_pin only switches the matrix findings off.
        if rules.get("pin_to_pin").and_then(severity) == Some(ErcSeverity::Ignore) {
            severities.insert("pin-conflict".to_owned(), ErcSeverity::Ignore);
        }
    }
    let mut matrix = ERC_PIN_MATRIX;
    if let Some(rows) = erc.get("pin_map").and_then(Value::as_array) {
        for (a, row) in rows.iter().take(12).enumerate() {
            for (b, cell) in row.as_array().into_iter().flatten().take(12).enumerate() {
                matrix[a][b] = match cell.as_u64() {
                    Some(1) => ErcSeverity::Warning,
                    Some(2) => ErcSeverity::Error,
                    _ => ErcSeverity::Ignore,
                };
            }
        }
        // ElectronicWerk's matrix is symmetric; the stricter value wins.
        let read = matrix;
        for (a, row) in matrix.iter_mut().enumerate() {
            for (b, cell) in row.iter_mut().enumerate() {
                *cell = read[a][b].min(read[b][a]);
            }
        }
    }
    let mut out = ErcSettings::default();
    out.set_from(&severities, &matrix);
    out
}

fn mm_value(object: &Value, key: &str) -> Option<i64> {
    object.get(key)?.as_f64().map(nm)
}

/// Reads the settings of a `.kicad_pro` file.
#[must_use]
pub fn settings(json: &Value) -> ProjectSettings {
    let mut out = ProjectSettings::default();
    if let Some(vars) = json.get("text_variables").and_then(Value::as_object) {
        for (key, value) in vars {
            if let Some(text) = value.as_str() {
                out.variables.insert(key.clone(), text.to_owned());
            }
        }
    }
    if let Some(erc) = json.get("erc") {
        out.erc = erc_settings(erc);
        out.erc_exclusions = erc
            .get("erc_exclusions")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
    }
    if let Some(rules) = json.pointer("/board/design_settings/rules") {
        let defaults = Constraints::default();
        let get =
            |key: &str, fallback: i64| mm_value(rules, key).filter(|v| *v > 0).unwrap_or(fallback);
        let hole = get("min_through_hole_diameter", defaults.min_hole);
        out.constraints = Some(Constraints {
            min_clearance: get("min_clearance", defaults.min_clearance),
            min_track_width: get("min_track_width", defaults.min_track_width),
            min_via_diameter: get("min_via_diameter", defaults.min_via_diameter),
            min_via_drill: hole,
            min_annular_ring: get("min_via_annular_width", defaults.min_annular_ring),
            min_hole: hole,
            min_hole_to_hole: get("min_hole_to_hole", defaults.min_hole_to_hole),
            min_copper_to_edge: get("min_copper_edge_clearance", defaults.min_copper_to_edge),
            min_silk_width: get("min_text_thickness", defaults.min_silk_width),
        });
    }
    if let Some(classes) = json
        .pointer("/net_settings/classes")
        .and_then(Value::as_array)
    {
        let defaults = NetClass::default_class();
        for class in classes {
            let Some(name) = class.get("name").and_then(Value::as_str) else {
                continue;
            };
            let get = |key: &str, fallback: i64| {
                mm_value(class, key).filter(|v| *v > 0).unwrap_or(fallback)
            };
            out.net_classes.push(NetClass {
                name: name.to_owned(),
                description: class
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                clearance: get("clearance", defaults.clearance),
                track_width: get("track_width", defaults.track_width),
                via_diameter: get("via_diameter", defaults.via_diameter),
                via_drill: get("via_drill", defaults.via_drill),
                diff_pair_width: get("diff_pair_width", defaults.diff_pair_width),
                diff_pair_gap: get("diff_pair_gap", defaults.diff_pair_gap),
            });
        }
        out.net_classes
            .sort_by_key(|class| class.name != DEFAULT_NET_CLASS);
    }
    if let Some(patterns) = json
        .pointer("/net_settings/netclass_patterns")
        .and_then(Value::as_array)
    {
        for entry in patterns {
            if let (Some(pattern), Some(class)) = (
                entry.get("pattern").and_then(Value::as_str),
                entry.get("netclass").and_then(Value::as_str),
            ) {
                out.assignments.push(NetClassAssignment {
                    pattern: pattern.to_owned(),
                    class: class.to_owned(),
                });
            }
        }
    }
    out
}

/// One entry of a `sym-lib-table` or `fp-lib-table`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryEntry {
    /// Nickname.
    pub name: String,
    /// The library path (variables unresolved).
    pub uri: String,
}

/// Reads a library table.
#[must_use]
pub fn library_table(node: &Sexp<'_>) -> Vec<LibraryEntry> {
    node.all("lib")
        .filter(|lib| !lib.flag("disabled"))
        .filter_map(|lib| {
            Some(LibraryEntry {
                name: lib.value("name")?.to_owned(),
                uri: lib.value("uri")?.to_owned(),
            })
        })
        .collect()
}

/// Number of custom rules in a `.kicad_dru` file.
#[must_use]
pub fn custom_rule_count(text: &str) -> usize {
    text.matches("(rule ").count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sexpr::parse;
    use ew_model::units::um;
    use serde_json::json;

    #[test]
    fn reads_rules_classes_patterns_and_variables() {
        let pro = json!({
            "text_variables": {"AUTHOR": "Lab", "N": 3},
            "board": {"design_settings": {"rules": {
                "min_clearance": 0.2, "min_track_width": 0.15, "min_via_diameter": 0.6,
                "min_through_hole_diameter": 0.3, "min_via_annular_width": 0.15,
                "min_hole_to_hole": 0.5, "min_copper_edge_clearance": 0.5, "min_text_thickness": 0.1
            }}},
            "net_settings": {
                "classes": [
                    {"name": "Power", "clearance": 0.3, "track_width": 0.6, "via_diameter": 1.0, "via_drill": 0.5},
                    {"name": "Default", "clearance": 0.2, "track_width": 0.25}
                ],
                "netclass_patterns": [{"netclass": "Power", "pattern": "+*"}]
            }
        });
        let settings = settings(&pro);
        assert_eq!(
            settings.variables.get("AUTHOR").map(String::as_str),
            Some("Lab")
        );
        assert_eq!(settings.variables.len(), 1);
        let rules = settings.constraints.unwrap();
        assert_eq!(rules.min_track_width, um(150));
        assert_eq!(rules.min_copper_to_edge, um(500));
        assert_eq!(settings.net_classes[0].name, "Default");
        assert_eq!(settings.net_classes[1].track_width, um(600));
        assert_eq!(
            settings.net_classes[1].diff_pair_gap,
            NetClass::default_class().diff_pair_gap
        );
        assert_eq!(settings.assignments[0].pattern, "+*");
    }

    #[test]
    fn reads_erc_settings() {
        let mut pin_map: Vec<Vec<u8>> = ERC_PIN_MATRIX
            .iter()
            .map(|row| {
                row.iter()
                    .map(|s| match s {
                        ErcSeverity::Error => 2,
                        ErcSeverity::Warning => 1,
                        ErcSeverity::Ignore => 0,
                    })
                    .collect()
            })
            .collect();
        // Output to output becomes a warning; passive to unspecified fine.
        pin_map[1][1] = 1;
        pin_map[4][6] = 0;
        pin_map[6][4] = 0;
        let pro = json!({"erc": {
            "rule_severities": {
                "pin_not_connected": "warning", "single_global_label": "ignore",
                "hier_label_mismatch": "warning", "endpoint_off_grid": "warning",
                "pin_to_pin": "warning"
            },
            "pin_map": pin_map,
            "erc_exclusions": [["x", "y"]]
        }});
        let settings = settings(&pro);
        let erc = &settings.erc;
        assert_eq!(settings.erc_exclusions, 1);
        // Only changes from the defaults are kept.
        assert_eq!(
            erc.severities
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            [
                "hierarchical-label-unmatched",
                "sheet-pin-unmatched",
                "unconnected-pin"
            ]
        );
        assert_eq!(erc.severity("unconnected-pin"), ErcSeverity::Warning);
        assert_eq!(
            erc.pin_matrix
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["output/output", "passive/unspecified"]
        );
        assert_eq!(erc.matrix()[6][4], ErcSeverity::Ignore);
    }

    #[test]
    fn reads_library_tables() {
        let table = parse(r#"(sym_lib_table (version 7)
            (lib (name "Project")(type "KiCad")(uri "${KIPRJMOD}/Project.kicad_sym")(options "")(descr ""))
            (lib (name "Old")(type "KiCad")(uri "/x.kicad_sym")(options "")(descr "")(disabled)))"#).unwrap();
        let libs = library_table(&table);
        assert_eq!(libs.len(), 1);
        assert_eq!(libs[0].uri, "${KIPRJMOD}/Project.kicad_sym");
        assert_eq!(custom_rule_count("(rule \"a\") (rule \"b\")"), 2);
    }
}
