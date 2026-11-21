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
    /// Vias that do not go through the whole board (blind and buried)
    /// may be used.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub allow_blind_buried_vias: bool,
    /// Laser-drilled microvias between neighbouring layers may be used.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub allow_micro_vias: bool,
    /// Minimum microvia pad diameter.
    pub min_micro_via_diameter: Nm,
    /// Minimum microvia drill.
    pub min_micro_via_drill: Nm,
    /// Minimum distance between the board or its parts and the mechanical
    /// bodies of its 3D assembly (enclosure, panels, screws).
    pub min_mechanical_clearance: Nm,
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
            allow_blind_buried_vias: false,
            allow_micro_vias: false,
            min_micro_via_diameter: um(200),
            min_micro_via_drill: um(100),
            min_mechanical_clearance: um(500),
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
    /// Microvia pad diameter.
    #[serde(default = "micro_via_diameter")]
    pub micro_via_diameter: Nm,
    /// Microvia drill.
    #[serde(default = "micro_via_drill")]
    pub micro_via_drill: Nm,
    /// Differential pair track width.
    pub diff_pair_width: Nm,
    /// Differential pair gap.
    pub diff_pair_gap: Nm,
    /// The impedance profile its tracks follow (empty: none).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub impedance: String,
    /// The largest length difference of the two nets of a differential
    /// pair in the class (0: not checked).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub max_skew: Nm,
}

/// Nets whose routed lengths must match (the data lines of a parallel bus,
/// for example).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LengthGroup {
    /// Name (unique).
    pub name: String,
    /// Net name patterns (`*` and `?` wildcards), separated by spaces or
    /// commas: `DQ? DQS*`.
    pub nets: String,
    /// How much shorter than the longest net of the group a net may be.
    pub tolerance: Nm,
}

impl LengthGroup {
    /// The patterns of the group.
    pub fn patterns(&self) -> impl Iterator<Item = &str> {
        self.nets
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|p| !p.is_empty())
    }
}

/// How teardrops are made: the smooth widening of a track where it meets a
/// pad or via.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[allow(clippy::struct_excessive_bools, reason = "independent switches")]
pub struct TeardropSettings {
    /// At pads.
    pub pads: bool,
    /// At vias.
    pub vias: bool,
    /// Length beyond the pad edge, in per cent of the pad size.
    pub length_ratio: u16,
    /// The longest teardrop.
    pub max_length: Nm,
    /// Width at the pad, in per cent of the pad size.
    pub width_ratio: u16,
    /// The widest teardrop.
    pub max_width: Nm,
    /// Curved sides (otherwise straight).
    pub curved: bool,
}

impl Default for TeardropSettings {
    fn default() -> Self {
        Self {
            pads: true,
            vias: true,
            length_ratio: 50,
            max_length: um(1_000),
            width_ratio: 100,
            max_width: um(2_000),
            curved: true,
        }
    }
}

impl TeardropSettings {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[allow(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if signature"
)]
const fn is_zero(value: &Nm) -> bool {
    *value == 0
}

/// A target impedance for the tracks of net classes: their width (and the
/// gap of pairs) on every copper layer follow from it and the stack-up.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpedanceProfile {
    /// Name (unique).
    pub name: String,
    /// A pair of tracks (the differential impedance) instead of one.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub differential: bool,
    /// Target impedance in ohms.
    pub target: u16,
    /// Allowed deviation in per cent.
    #[serde(default = "impedance_tolerance")]
    pub tolerance: u8,
    /// The interface it was made for (`usb2`, `ethernet`, …), if any.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preset: String,
    /// Widths and gaps set by hand per copper layer (`cu.top`, …); other
    /// layers are computed from the stack-up.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub layers: BTreeMap<String, TrackGeometry>,
}

/// Track width and pair gap on one layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackGeometry {
    /// Track width.
    pub width: Nm,
    /// Gap between the tracks of a pair.
    #[serde(default)]
    pub gap: Nm,
}

const fn impedance_tolerance() -> u8 {
    10
}

const fn micro_via_diameter() -> Nm {
    um(300)
}

const fn micro_via_drill() -> Nm {
    um(100)
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
            micro_via_diameter: micro_via_diameter(),
            micro_via_drill: micro_via_drill(),
            diff_pair_width: um(200),
            diff_pair_gap: um(250),
            impedance: String::new(),
            max_skew: 0,
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
    ("via-type", ErcSeverity::Error),
    ("via-layers", ErcSeverity::Error),
    ("impedance", ErcSeverity::Warning),
    ("pair-gap", ErcSeverity::Warning),
    ("pair-skew", ErcSeverity::Warning),
    ("length-match", ErcSeverity::Warning),
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
    ("mechanical-collision", ErcSeverity::Error),
    ("mechanical-clearance", ErcSeverity::Warning),
    ("mechanical-height", ErcSeverity::Warning),
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
    /// Impedance profiles of the net classes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub impedance_profiles: Vec<ImpedanceProfile>,
    /// Groups of nets with matched lengths.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub length_groups: Vec<LengthGroup>,
    /// How teardrops are made.
    #[serde(default, skip_serializing_if = "TeardropSettings::is_default")]
    pub teardrops: TeardropSettings,
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
    /// Impedance profiles.
    #[serde(default)]
    pub impedance_profiles: Vec<ImpedanceProfile>,
    /// Groups of nets with matched lengths.
    #[serde(default)]
    pub length_groups: Vec<LengthGroup>,
    /// How teardrops are made.
    #[serde(default)]
    pub teardrops: TeardropSettings,
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
            impedance_profiles: Vec::new(),
            length_groups: Vec::new(),
            teardrops: TeardropSettings::default(),
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
            impedance_profiles: self.impedance_profiles.clone(),
            length_groups: self.length_groups.clone(),
            teardrops: self.teardrops,
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
        self.impedance_profiles = props.impedance_profiles;
        self.length_groups = props.length_groups;
        self.teardrops = props.teardrops;
    }

    /// The impedance profile with this name.
    #[must_use]
    pub fn impedance_profile(&self, name: &str) -> Option<&ImpedanceProfile> {
        self.impedance_profiles.iter().find(|p| p.name == name)
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
