//! Design rules: constraints and net classes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::id::Id;
use crate::library::PinKind;
use crate::units::{Nm, um};

/// Board-wide manufacturing constraints (minimum values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Constraints {
    /// Minimum copper-to-copper clearance.
    pub min_clearance: Nm,
    /// Minimum track width.
    pub min_track_width: Nm,
    /// Minimum via pad diameter.
    pub min_via_diameter: Nm,
    /// Minimum via drill.
    pub min_via_drill: Nm,
    /// Minimum annular ring.
    pub min_annular_ring: Nm,
    /// Minimum drill size of any hole.
    pub min_hole: Nm,
    /// Minimum distance between holes.
    pub min_hole_to_hole: Nm,
    /// Minimum copper distance to the board edge.
    pub min_copper_to_edge: Nm,
    /// Minimum silkscreen line width.
    pub min_silk_width: Nm,
}

impl Default for Constraints {
    /// Values that common low-cost board houses can manufacture.
    fn default() -> Self {
        Self {
            min_clearance: um(200),
            min_track_width: um(200),
            min_via_diameter: um(500),
            min_via_drill: um(300),
            min_annular_ring: um(130),
            min_hole: um(300),
            min_hole_to_hole: um(250),
            min_copper_to_edge: um(300),
            min_silk_width: um(150),
        }
    }
}

/// Name of the net class every net belongs to unless assigned otherwise.
pub const DEFAULT_NET_CLASS: &str = "Default";

/// A net class: routing defaults for a group of nets.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetClass {
    /// Name (unique).
    pub name: String,
    /// Description.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// Clearance.
    pub clearance: Nm,
    /// Track width.
    pub track_width: Nm,
    /// Via pad diameter.
    pub via_diameter: Nm,
    /// Via drill.
    pub via_drill: Nm,
    /// Differential pair track width.
    pub diff_pair_width: Nm,
    /// Differential pair gap.
    pub diff_pair_gap: Nm,
}

impl NetClass {
    /// The `Default` net class.
    #[must_use]
    pub fn default_class() -> Self {
        Self {
            name: DEFAULT_NET_CLASS.to_owned(),
            description: String::new(),
            clearance: um(200),
            track_width: um(250),
            via_diameter: um(600),
            via_drill: um(300),
            diff_pair_width: um(200),
            diff_pair_gap: um(250),
        }
    }

    /// A `Power` class with wider tracks.
    #[must_use]
    pub fn power_class() -> Self {
        Self {
            name: "Power".to_owned(),
            track_width: um(500),
            via_diameter: um(800),
            via_drill: um(400),
            ..Self::default_class()
        }
    }
}

/// Assigns nets whose name matches a pattern to a net class.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetClassAssignment {
    /// Net name pattern; `*` matches any text, `?` one character.
    pub pattern: String,
    /// Net class name.
    pub class: String,
}

/// How serious an electrical rules check finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ErcSeverity {
    /// An error.
    Error,
    /// A warning.
    Warning,
    /// Not reported.
    Ignore,
}

/// Settings of the electrical rules check. Only changes from the defaults
/// are stored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErcSettings {
    /// Severity per check (`unconnected-pin` → `warning`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub severities: BTreeMap<String, ErcSeverity>,
    /// Pin type matrix: `output/output` → `error` (pin types in camelCase,
    /// in either order).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pin_matrix: BTreeMap<String, ErcSeverity>,
}

/// Every ERC check with its default severity (KiCad's defaults where KiCad
/// has the same check).
pub const ERC_CHECKS: &[(&str, ErcSeverity)] = &[
    ("pin-conflict", ErcSeverity::Error),
    ("power-not-driven", ErcSeverity::Error),
    ("input-not-driven", ErcSeverity::Error),
    ("unconnected-pin", ErcSeverity::Error),
    ("no-connect-connected", ErcSeverity::Warning),
    ("no-connect-dangling", ErcSeverity::Warning),
    ("dangling-wire", ErcSeverity::Warning),
    ("dangling-label", ErcSeverity::Error),
    ("hierarchical-label-unmatched", ErcSeverity::Error),
    ("sheet-pin-unmatched", ErcSeverity::Error),
    ("single-global-label", ErcSeverity::Ignore),
    ("net-name-conflict", ErcSeverity::Warning),
    ("duplicate-designator", ErcSeverity::Error),
    ("unannotated", ErcSeverity::Error),
    ("missing-symbol", ErcSeverity::Error),
    ("missing-sheet", ErcSeverity::Error),
    ("sheet-cycle", ErcSeverity::Error),
    ("off-grid", ErcSeverity::Warning),
];

const O: ErcSeverity = ErcSeverity::Ignore;
const W: ErcSeverity = ErcSeverity::Warning;
const E: ErcSeverity = ErcSeverity::Error;

/// The default pin type matrix (KiCad's), rows and columns in
/// [`PinKind::ALL`] order: what connecting two pin types on one net means.
pub const ERC_PIN_MATRIX: [[ErcSeverity; 12]; 12] = [
    //  In Out Bi  3S  Pas NIC Uns PwI PwO OC  OE  NC
    [O, O, O, O, O, O, W, O, O, O, O, E], // input
    [O, E, O, W, O, O, W, O, E, E, E, E], // output
    [O, O, O, O, O, O, W, O, W, O, W, E], // bidirectional
    [O, W, O, O, O, O, W, W, E, W, W, E], // tri-state
    [O, O, O, O, O, O, W, O, O, O, O, E], // passive
    [O, O, O, O, O, O, O, O, O, O, O, E], // free (not internally connected)
    [W, W, W, W, W, O, W, W, W, W, W, E], // unspecified
    [O, O, O, W, O, O, W, O, O, O, O, E], // power input
    [O, E, W, E, O, O, W, O, E, E, E, E], // power output
    [O, E, O, W, O, O, W, O, E, O, O, E], // open collector
    [O, E, W, W, O, O, W, O, E, O, O, E], // open emitter
    [E, E, E, E, E, E, E, E, E, E, E, E], // not connected
];

impl ErcSettings {
    /// `true` if nothing differs from the defaults.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.severities.is_empty() && self.pin_matrix.is_empty()
    }

    /// The severity of a check (unknown checks are errors).
    #[must_use]
    pub fn severity(&self, code: &str) -> ErcSeverity {
        self.severities.get(code).copied().unwrap_or_else(|| {
            ERC_CHECKS
                .iter()
                .find(|(c, _)| *c == code)
                .map_or(ErcSeverity::Error, |(_, s)| *s)
        })
    }

    /// The matrix key of two pin types (`output/powerOut`, in matrix order).
    #[must_use]
    pub fn matrix_key(a: PinKind, b: PinKind) -> String {
        let (a, b) = if a.index() <= b.index() {
            (a, b)
        } else {
            (b, a)
        };
        format!("{}/{}", a.name(), b.name())
    }

    /// The effective pin type matrix: the defaults with the changes.
    #[must_use]
    pub fn matrix(&self) -> [[ErcSeverity; 12]; 12] {
        let mut out = ERC_PIN_MATRIX;
        for (key, severity) in &self.pin_matrix {
            let Some((a, b)) = key.split_once('/') else {
                continue;
            };
            if let (Some(a), Some(b)) = (PinKind::from_name(a), PinKind::from_name(b)) {
                out[a.index()][b.index()] = *severity;
                out[b.index()][a.index()] = *severity;
            }
        }
        out
    }

    /// Sets the severities and the matrix from complete tables, storing
    /// only what differs from the defaults.
    pub fn set_from(
        &mut self,
        severities: &BTreeMap<String, ErcSeverity>,
        matrix: &[[ErcSeverity; 12]; 12],
    ) {
        self.severities = severities
            .iter()
            .filter(|(code, severity)| ERC_CHECKS.iter().any(|(c, s)| c == code && s != *severity))
            .map(|(c, s)| (c.clone(), *s))
            .collect();
        self.pin_matrix.clear();
        for (a, row) in matrix.iter().enumerate() {
            for (b, severity) in row.iter().enumerate().skip(a) {
                if *severity != ERC_PIN_MATRIX[a][b] {
                    self.pin_matrix.insert(
                        Self::matrix_key(PinKind::ALL[a], PinKind::ALL[b]),
                        *severity,
                    );
                }
            }
        }
    }
}

/// Every design rules check with its default severity.
pub const DRC_CHECKS: &[(&str, ErcSeverity)] = &[
    ("short", ErcSeverity::Error),
    ("clearance", ErcSeverity::Error),
    ("unrouted", ErcSeverity::Error),
    ("track-width", ErcSeverity::Error),
    ("via-diameter", ErcSeverity::Error),
    ("via-drill", ErcSeverity::Error),
    ("annular-ring", ErcSeverity::Error),
    ("hole-size", ErcSeverity::Error),
    ("hole-to-hole", ErcSeverity::Error),
    ("copper-to-edge", ErcSeverity::Error),
    ("no-outline", ErcSeverity::Error),
    ("outline-open", ErcSeverity::Error),
    ("outside-board", ErcSeverity::Error),
    ("keepout", ErcSeverity::Error),
    ("courtyard-overlap", ErcSeverity::Error),
    ("silk-over-pad", ErcSeverity::Warning),
    ("silk-width", ErcSeverity::Warning),
    ("dangling-track", ErcSeverity::Warning),
    ("dangling-via", ErcSeverity::Warning),
    ("zone-unfilled", ErcSeverity::Warning),
    ("duplicate-designator", ErcSeverity::Error),
    ("missing-footprint", ErcSeverity::Warning),
    ("extra-footprint", ErcSeverity::Warning),
    ("net-mismatch", ErcSeverity::Error),
    ("rule-error", ErcSeverity::Error),
];

/// Settings of the design rules check. Only changes from the defaults are
/// stored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrcSettings {
    /// Severity per check (`dangling-via` → `ignore`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub severities: BTreeMap<String, ErcSeverity>,
}

impl DrcSettings {
    /// `true` if nothing differs from the defaults.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.severities.is_empty()
    }

    /// The severity of a check (unknown checks are errors).
    #[must_use]
    pub fn severity(&self, code: &str) -> ErcSeverity {
        self.severities.get(code).copied().unwrap_or_else(|| {
            DRC_CHECKS
                .iter()
                .find(|(c, _)| *c == code)
                .map_or(ErcSeverity::Error, |(_, s)| *s)
        })
    }

    /// Sets the severities from a complete table, storing only what
    /// differs from the defaults.
    pub fn set_from(&mut self, severities: &BTreeMap<String, ErcSeverity>) {
        self.severities = severities
            .iter()
            .filter(|(code, severity)| DRC_CHECKS.iter().any(|(c, s)| c == code && s != *severity))
            .map(|(c, s)| (c.clone(), *s))
            .collect();
    }
}

/// The constraints a custom rule sets. Every field is optional: a rule only
/// changes what it names.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RuleConstraints {
    /// Copper clearance between the two items.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clearance: Option<Nm>,
    /// Minimum track width.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_width_min: Option<Nm>,
    /// Maximum track width.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_width_max: Option<Nm>,
    /// Track width used when routing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_width: Option<Nm>,
    /// Minimum via pad diameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via_diameter_min: Option<Nm>,
    /// Minimum via drill.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via_drill_min: Option<Nm>,
    /// Minimum annular ring.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annular_ring_min: Option<Nm>,
    /// Minimum hole size.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hole_min: Option<Nm>,
    /// Minimum distance between holes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hole_to_hole_min: Option<Nm>,
    /// Minimum copper distance to the board edge.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edge_clearance: Option<Nm>,
}

/// A custom design rule: constraints for the items that match a condition
/// in the rule query language, for example
/// `A.netclass == 'HV' && B.netclass != 'HV'`. Later rules win over earlier
/// ones.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomRule {
    /// Name, shown in DRC findings.
    pub name: String,
    /// Switched on.
    #[serde(default = "crate::default_true", skip_serializing_if = "Clone::clone")]
    pub enabled: bool,
    /// Condition in the rule query language (empty: every item).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub condition: String,
    /// The constraints.
    #[serde(default)]
    pub constraints: RuleConstraints,
}

/// A rules document (`.ewrules`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rules {
    /// Document id.
    pub id: Id,
    /// Name.
    pub name: String,
    /// Board-wide constraints.
    #[serde(default)]
    pub constraints: Constraints,
    /// Net classes; the first is normally `Default`.
    #[serde(default)]
    pub net_classes: Vec<NetClass>,
    /// Pattern-based net class assignments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assignments: Vec<NetClassAssignment>,
    /// Electrical rules check settings.
    #[serde(default, skip_serializing_if = "ErcSettings::is_empty")]
    pub erc: ErcSettings,
    /// Custom rules, applied in order (later rules win).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom: Vec<CustomRule>,
    /// Design rules check settings.
    #[serde(default, skip_serializing_if = "DrcSettings::is_empty")]
    pub drc: DrcSettings,
}

/// Editable rule properties (the whole document except its id).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RulesProps {
    /// Name.
    pub name: String,
    /// Constraints.
    pub constraints: Constraints,
    /// Net classes.
    pub net_classes: Vec<NetClass>,
    /// Assignments.
    #[serde(default)]
    pub assignments: Vec<NetClassAssignment>,
    /// Electrical rules check settings.
    #[serde(default)]
    pub erc: ErcSettings,
    /// Custom rules.
    #[serde(default)]
    pub custom: Vec<CustomRule>,
    /// Design rules check settings.
    #[serde(default)]
    pub drc: DrcSettings,
}

impl Rules {
    /// Default rules with the `Default` and `Power` net classes.
    #[must_use]
    pub fn new(id: Id, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            constraints: Constraints::default(),
            net_classes: vec![NetClass::default_class(), NetClass::power_class()],
            assignments: Vec::new(),
            erc: ErcSettings::default(),
            custom: Vec::new(),
            drc: DrcSettings::default(),
        }
    }

    /// The editable properties.
    #[must_use]
    pub fn props(&self) -> RulesProps {
        RulesProps {
            name: self.name.clone(),
            constraints: self.constraints,
            net_classes: self.net_classes.clone(),
            assignments: self.assignments.clone(),
            erc: self.erc.clone(),
            custom: self.custom.clone(),
            drc: self.drc.clone(),
        }
    }

    /// Replaces the editable properties.
    pub fn set_props(&mut self, props: RulesProps) {
        self.name = props.name;
        self.constraints = props.constraints;
        self.net_classes = props.net_classes;
        self.assignments = props.assignments;
        self.erc = props.erc;
        self.custom = props.custom;
        self.drc = props.drc;
    }

    /// The net class with this name.
    #[must_use]
    pub fn net_class(&self, name: &str) -> Option<&NetClass> {
        self.net_classes.iter().find(|class| class.name == name)
    }

    /// The net class of a net: the class set on the board net, else the
    /// first matching assignment, else `Default`.
    #[must_use]
    pub fn class_of<'a>(&'a self, net_name: &str, board_class: &'a str) -> &'a str {
        if !board_class.is_empty() {
            return board_class;
        }
        self.assignments
            .iter()
            .find(|assignment| matches_pattern(&assignment.pattern, net_name))
            .map_or(DEFAULT_NET_CLASS, |assignment| assignment.class.as_str())
    }
}

/// Matches a net name against a wildcard pattern (`*` and `?`).
#[must_use]
pub fn matches_pattern(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    // Iterative wildcard matching with backtracking to the last `*`.
    let (mut p, mut n) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, n));
                p += 1;
            }
            Some(&c) if c == '?' || c == name[n] => {
                p += 1;
                n += 1;
            }
            _ => match star {
                Some((star_p, star_n)) => {
                    p = star_p + 1;
                    n = star_n + 1;
                    star = Some((star_p, star_n + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_patterns() {
        assert!(matches_pattern("GND", "GND"));
        assert!(matches_pattern("GND*", "GND_A"));
        assert!(matches_pattern("*", ""));
        assert!(matches_pattern("+?V?", "+3V3"));
        assert!(matches_pattern("*V*", "+5V"));
        assert!(!matches_pattern("GND", "AGND"));
        assert!(!matches_pattern("?", ""));
        assert!(matches_pattern("a*b*c", "axxbyyc"));
        assert!(!matches_pattern("a*b*c", "axxbyy"));
    }

    #[test]
    fn default_rules_round_trip() {
        let rules = Rules::new(Id::random(), "Design rules");
        assert!(rules.net_class(DEFAULT_NET_CLASS).is_some());
        let json = serde_json::to_string(&rules).unwrap();
        assert_eq!(serde_json::from_str::<Rules>(&json).unwrap(), rules);
        let mut copy = rules.clone();
        copy.set_props(rules.props());
        assert_eq!(copy, rules);
    }
}
