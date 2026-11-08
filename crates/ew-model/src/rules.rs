//! Design rules: constraints and net classes.

use serde::{Deserialize, Serialize};

use crate::id::Id;
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
        }
    }

    /// Replaces the editable properties.
    pub fn set_props(&mut self, props: RulesProps) {
        self.name = props.name;
        self.constraints = props.constraints;
        self.net_classes = props.net_classes;
        self.assignments = props.assignments;
    }

    /// The net class with this name.
    #[must_use]
    pub fn net_class(&self, name: &str) -> Option<&NetClass> {
        self.net_classes.iter().find(|class| class.name == name)
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
