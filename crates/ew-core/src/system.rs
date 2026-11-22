//! Systems of several boards (M18): the `system.*` engine methods.
//!
//! - Boards: `system.info`, `system.addBoard`, `system.updateBoard`,
//!   `system.removeBoard` — boards of this project or of other projects.
//! - Connectors: `system.connectors`, `system.pins`.
//! - Connections: `system.addLink`, `system.updateLink`, `system.removeLink`
//!   for connectors that mate directly; `system.addCable`,
//!   `system.updateCable`, `system.removeCable`, `system.wireCable` for
//!   cables.
//! - The connection check: `system.check`, `system.exclude`.
//! - Signals: `system.counterparts`, `system.renameSignal`.
//!
//! A system document has no objects: every edit replaces some of its
//! properties as one undo step.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::layer::Side;
use ew_model::pcb::{FootprintInstance, Pcb};
use ew_model::project::Project;
use ew_model::system::{
    BoardSource, Cable, CableEnd, ConnectorRef, Link, Pairing, System, SystemBoard, Wire, WireEnd,
};
use ew_model::units::{Angle, Nm, Vec3};
use ew_pcb::geom::V;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::change::Request;
use crate::error::{EngineError, ErrorCode};
use crate::session::Session;

/// History labels.
pub const BOARDS: &str = "system-boards";
/// History label of link edits.
pub const LINKS: &str = "system-links";
/// History label of cable edits.
pub const CABLES: &str = "system-cables";
/// History label of other system edits.
pub const EDIT: &str = "edit-system";
/// History label of `system.renameSignal`.
pub const RENAME: &str = "rename-signal";

/// The result of a `system.*` call.
#[derive(Debug)]
pub enum Outcome {
    /// A reply without changes.
    Read(Value),
    /// Requests to commit as one undo step.
    Commit {
        /// History label.
        label: &'static str,
        /// Requests.
        requests: Vec<Request>,
        /// Extra reply fields.
        reply: Value,
    },
}

// ---------------------------------------------------------------------------
// Boards of other projects.

type Stamp = Vec<(PathBuf, Option<SystemTime>)>;

fn stamp_of(paths: &[PathBuf]) -> Stamp {
    paths
        .iter()
        .map(|p| {
            (
                p.clone(),
                std::fs::metadata(p).and_then(|m| m.modified()).ok(),
            )
        })
        .collect()
}

/// Projects of other boards, read once and again when their files change.
#[derive(Default)]
pub struct Cache {
    projects: HashMap<PathBuf, (Stamp, Result<Arc<ew_format::store::LoadedProject>, String>)>,
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache")
            .field("projects", &self.projects.len())
            .finish()
    }
}

impl Cache {
    /// Forgets every project (a project was opened or closed).
    pub fn clear(&mut self) {
        self.projects.clear();
    }

    /// The project at a path (folder or manifest).
    fn project(&mut self, path: &Path) -> Result<Arc<ew_format::store::LoadedProject>, String> {
        let manifest = ew_format::store::find_manifest(path).map_err(|e| e.to_string())?;
        if let Some((stamp, loaded)) = self.projects.get(&manifest) {
            let paths: Vec<PathBuf> = stamp.iter().map(|(p, _)| p.clone()).collect();
            if stamp_of(&paths) == *stamp {
                return loaded.clone();
            }
        }
        let loaded = ew_format::store::load_project(&manifest)
            .map(Arc::new)
            .map_err(|e| e.to_string());
        let mut paths = vec![manifest.clone()];
        if let Ok(project) = &loaded {
            paths.extend(
                project
                    .project
                    .manifest
                    .documents
                    .iter()
                    .map(|d| ew_format::store::resolve(&project.root, &d.path)),
            );
        }
        self.projects
            .insert(manifest, (stamp_of(&paths), loaded.clone()));
        loaded
    }
}

/// A board of the system as read.
#[derive(Debug, Clone)]
pub struct Loaded {
    /// The system board.
    pub entry: SystemBoard,
    /// Its board document, if it could be read.
    pub pcb: Option<Arc<Pcb>>,
    /// The folder of the board's project.
    pub root: PathBuf,
    /// The project's name.
    pub project: String,
    /// `true` for a board of this project.
    pub local: bool,
    /// Why the board could not be read.
    pub error: Option<String>,
}

fn path_of(root: &Path, path: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

fn first_board<'a>(project: &'a Project, wanted: &str) -> Option<&'a Pcb> {
    let boards: Vec<&Pcb> = project
        .documents_of(DocumentKind::Pcb)
        .filter_map(|entry| match project.document(entry.id) {
            Some(Document::Pcb(pcb)) => Some(pcb),
            _ => None,
        })
        .collect();
    if wanted.is_empty() {
        return boards.first().copied();
    }
    boards
        .iter()
        .copied()
        .find(|b| b.name == wanted || b.id.to_string() == wanted)
}

/// Reads every board of a system.
pub fn load(session: &Session, cache: &mut Cache, system: &System) -> Vec<Loaded> {
    let project = session.project();
    system
        .boards
        .iter()
        .map(|entry| match &entry.source {
            BoardSource::Local { doc } => {
                let pcb = match project.document(*doc) {
                    Some(Document::Pcb(pcb)) => Some(Arc::new(pcb.clone())),
                    _ => None,
                };
                Loaded {
                    entry: entry.clone(),
                    error: pcb
                        .is_none()
                        .then(|| "the board is not in the project".to_owned()),
                    pcb,
                    root: session.root().to_path_buf(),
                    project: project.manifest.name.clone(),
                    local: true,
                }
            }
            BoardSource::Project { path, board } => {
                let file = path_of(session.root(), path);
                match cache.project(&file) {
                    Ok(loaded) => {
                        let pcb = first_board(&loaded.project, board).map(|p| Arc::new(p.clone()));
                        Loaded {
                            entry: entry.clone(),
                            error: pcb
                                .is_none()
                                .then(|| format!("the project has no board {board}")),
                            pcb,
                            root: loaded.root.clone(),
                            project: loaded.project.manifest.name.clone(),
                            local: false,
                        }
                    }
                    Err(error) => Loaded {
                        entry: entry.clone(),
                        pcb: None,
                        root: file.parent().map(Path::to_path_buf).unwrap_or_default(),
                        project: String::new(),
                        local: false,
                        error: Some(error),
                    },
                }
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Connectors and signals.

/// A pin of a connector.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    /// Pin (pad) number.
    pub number: String,
    /// The net's name (none: not connected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net: Option<String>,
    /// The net is a supply (GND, VCC, +5V …).
    pub power: bool,
    /// Centre of the pad on its board (mm).
    pub at: [f64; 2],
}

/// A connector (any footprint) of a board.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connector {
    /// Designator.
    pub designator: String,
    /// Footprint name.
    pub footprint: String,
    /// Value.
    pub value: String,
    /// Board side.
    pub side: Side,
    /// Pins in number order.
    pub pins: Vec<Pin>,
}

/// `true` for footprints that look like connectors.
#[must_use]
pub fn looks_like_connector(fp: &FootprintInstance) -> bool {
    let prefix: String = fp
        .designator
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .collect::<String>()
        .to_ascii_uppercase();
    let name = format!("{} {}", fp.footprint, fp.value).to_ascii_lowercase();
    matches!(
        prefix.as_str(),
        "J" | "P" | "X" | "CN" | "CON" | "XS" | "XP"
    ) || [
        "connector",
        "header",
        "socket",
        "pinheader",
        "pinsocket",
        "usb",
        "jst",
        "molex",
        "terminal",
        "idc",
        "fpc",
        "ffc",
        "b2b",
    ]
    .iter()
    .any(|w| name.contains(w))
}

/// A net name without its sheet path (`/Sensors/SDA` → `SDA`).
#[must_use]
pub fn short_name(net: &str) -> &str {
    net.rsplit('/').next().unwrap_or(net)
}

/// `true` for names the tools make up for unnamed nets.
#[must_use]
pub fn is_auto_name(net: &str) -> bool {
    let short = short_name(net);
    short.starts_with("Net-(") || short.starts_with("unconnected-(") || short.starts_with("N$")
}

/// Names of supply nets (also with a suffix: `GND_A`, `VCC3`).
const SUPPLIES: [&str; 13] = [
    "GND", "AGND", "DGND", "PGND", "GNDA", "GNDD", "VSS", "VCC", "VDD", "VEE", "VBAT", "VIN",
    "VBUS",
];

/// `true` for supply nets: by name (`GND`, `VCC`, `+5V`, `3V3`, `VIN` …) or
/// by a net class with "power" in its name.
#[must_use]
pub fn is_power(net: &str, class: &str) -> bool {
    if class.to_ascii_lowercase().contains("power") {
        return true;
    }
    let n = short_name(net).to_ascii_uppercase();
    let n = n.trim();
    if n.starts_with('+')
        || (n.starts_with('-') && n[1..].starts_with(|c: char| c.is_ascii_digit()))
    {
        return true;
    }
    if SUPPLIES.iter().any(|p| {
        n == *p
            || n.starts_with(&format!("{p}_"))
            || n.starts_with(p)
                && n[p.len()..]
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == 'V' || c == '.')
    }) {
        return true;
    }
    // 3V3, 5V, 12V, 3.3V, 1V8.
    let bytes = n.as_bytes();
    !n.is_empty()
        && bytes[0].is_ascii_digit()
        && n.contains('V')
        && n.chars()
            .all(|c| c.is_ascii_digit() || c == 'V' || c == '.')
}

fn net_info(pcb: &Pcb, net: Option<Id>) -> (Option<String>, bool) {
    let Some(net) = net.and_then(|id| pcb.nets.get(id)) else {
        return (None, false);
    };
    (Some(net.name.clone()), is_power(&net.name, &net.class))
}

/// The pins of a footprint, in number order (pads without a number left
/// out; pads with the same number once).
#[must_use]
pub fn connector_of(pcb: &Pcb, fp: &FootprintInstance) -> Connector {
    let xf = ew_pcb::items::footprint_xform(fp);
    let mut pins: Vec<Pin> = Vec::new();
    for pad in &fp.pads {
        if pad.number.trim().is_empty() || pins.iter().any(|p| p.number == pad.number) {
            continue;
        }
        let at = xf.apply(V::of(pad.at));
        let (net, power) = net_info(pcb, pad.net);
        pins.push(Pin {
            number: pad.number.clone(),
            net,
            power,
            at: [at.x / 1e6, at.y / 1e6],
        });
    }
    pins.sort_by(|a, b| ew_board3d::place::natural(&a.number, &b.number));
    Connector {
        designator: fp.designator.clone(),
        footprint: fp.footprint.clone(),
        value: fp.value.clone(),
        side: fp.side,
        pins,
    }
}

fn find_connector(pcb: &Pcb, designator: &str) -> Option<Connector> {
    pcb.footprints
        .iter()
        .find(|fp| fp.designator == designator)
        .map(|fp| connector_of(pcb, fp))
}

// ---------------------------------------------------------------------------
// The connection check.

/// A pin of a board in the system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinRef {
    /// System board.
    pub board: Id,
    /// Connector designator.
    pub designator: String,
    /// Pin number.
    pub pin: String,
}

/// One finding of the system check.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemFinding {
    /// Check code (`system-mismatch` …).
    pub code: String,
    /// `error` or `warning`.
    pub severity: String,
    /// English description.
    pub message: String,
    /// Values for translated messages (`variant` picks a text).
    pub args: BTreeMap<String, String>,
    /// The link or cable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection: Option<Id>,
    /// The pins concerned.
    pub pins: Vec<PinRef>,
    /// A stable key (for accepting).
    pub key: String,
    /// Accepted by the user.
    pub excluded: bool,
}

/// One pair of pins of a connection, with the signals on both sides.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinPair {
    /// Pin on side a (none: no partner on that side).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a: Option<String>,
    /// Signal on side a.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a_net: Option<String>,
    /// Pin on side b.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b: Option<String>,
    /// Signal on side b.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b_net: Option<String>,
    /// The wire (cables).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wire: Option<Id>,
    /// `ok`, `mismatch`, `swapped`, `power`, `missing`, `unmated`, `open`,
    /// `unused`.
    pub status: String,
}

/// A connection with its pin pairs.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionReport {
    /// The link or cable.
    pub id: Id,
    /// `link` or `cable`.
    pub kind: &'static str,
    /// A name for lists (`Carrier J1 ↔ Shield P1`, the cable's name).
    pub name: String,
    /// Pairs.
    pub pairs: Vec<PinPair>,
}

/// The result of the check.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checked {
    /// Findings, errors first.
    pub findings: Vec<SystemFinding>,
    /// Every connection with its pins.
    pub connections: Vec<ConnectionReport>,
    /// Errors (not accepted).
    pub errors: usize,
    /// Warnings (not accepted).
    pub warnings: usize,
}

/// Signal names compared across boards: the short name, case-insensitive,
/// with the system's aliases.
struct Signals<'a> {
    aliases: &'a [Vec<String>],
}

impl Signals<'_> {
    fn canonical(&self, net: &str) -> String {
        let short = short_name(net).trim().to_ascii_uppercase();
        for group in self.aliases {
            if group
                .iter()
                .any(|a| short_name(a).trim().eq_ignore_ascii_case(&short))
            {
                return group
                    .first()
                    .map_or(short.clone(), |f| short_name(f).trim().to_ascii_uppercase());
            }
        }
        short
    }

    fn same(&self, a: &str, b: &str) -> bool {
        self.canonical(a) == self.canonical(b)
    }
}

struct Board<'a> {
    loaded: &'a Loaded,
}

impl Board<'_> {
    fn name(&self) -> &str {
        &self.loaded.entry.name
    }
}

/// The finder: the boards, signals and findings collected so far.
struct Checker<'a> {
    boards: HashMap<Id, Board<'a>>,
    signals: Signals<'a>,
    exclusions: BTreeSet<&'a str>,
    out: Checked,
}

impl Checker<'_> {
    fn connector(&self, r: &ConnectorRef) -> Result<Connector, String> {
        let board = self
            .boards
            .get(&r.board)
            .ok_or_else(|| "a board that is not in the system".to_owned())?;
        let pcb = board.loaded.pcb.as_ref().ok_or_else(|| {
            format!(
                "{} ({})",
                board.name(),
                board.loaded.error.clone().unwrap_or_default()
            )
        })?;
        find_connector(pcb, &r.designator)
            .ok_or_else(|| format!("{} has no {}", board.name(), r.designator))
    }

    fn board_name(&self, id: Id) -> String {
        self.boards
            .get(&id)
            .map_or_else(String::new, |b| b.name().to_owned())
    }

    fn label(&self, r: &ConnectorRef) -> String {
        format!("{} {}", self.board_name(r.board), r.designator)
    }

    #[allow(clippy::too_many_arguments, reason = "the parts of a finding")]
    fn push(
        &mut self,
        code: &str,
        error: bool,
        message: String,
        args: &[(&str, String)],
        connection: Option<Id>,
        pins: Vec<PinRef>,
        key: &str,
    ) {
        let key = format!("{code}:{key}");
        let excluded = self.exclusions.contains(key.as_str());
        if !excluded {
            if error {
                self.out.errors += 1;
            } else {
                self.out.warnings += 1;
            }
        }
        self.out.findings.push(SystemFinding {
            code: code.to_owned(),
            severity: if error { "error" } else { "warning" }.to_owned(),
            message,
            args: args
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect(),
            connection,
            pins,
            key,
            excluded,
        });
    }

    /// Compares the two sides of a pin pair; returns the status.
    #[allow(clippy::too_many_arguments, reason = "both sides of a pair")]
    fn compare(
        &mut self,
        connection: Id,
        a: &ConnectorRef,
        pa: &Pin,
        b: &ConnectorRef,
        pb: &Pin,
        swapped: bool,
        via: &str,
    ) -> &'static str {
        let pins = vec![
            PinRef {
                board: a.board,
                designator: a.designator.clone(),
                pin: pa.number.clone(),
            },
            PinRef {
                board: b.board,
                designator: b.designator.clone(),
                pin: pb.number.clone(),
            },
        ];
        let where_a = format!("{} pin {}", self.label(a), pa.number);
        let where_b = format!("{} pin {}", self.label(b), pb.number);
        let key = format!("{connection}/{}/{}", pa.number, pb.number);
        match (&pa.net, &pb.net) {
            (None, None) => "unused",
            (Some(net), None) | (None, Some(net)) => {
                let (with, without) = if pa.net.is_some() {
                    (&where_a, &where_b)
                } else {
                    (&where_b, &where_a)
                };
                if is_auto_name(net) {
                    return "ok";
                }
                self.push(
                    "system-missing",
                    false,
                    format!(
                        "{with} carries {} but {without} is not connected{via}",
                        short_name(net)
                    ),
                    &[
                        ("signal", short_name(net).to_owned()),
                        ("with", with.clone()),
                        ("without", without.clone()),
                    ],
                    Some(connection),
                    pins,
                    &key,
                );
                "missing"
            }
            (Some(na), Some(nb)) => {
                let (sa, sb) = (short_name(na).to_owned(), short_name(nb).to_owned());
                if pa.power != pb.power || (pa.power && pb.power && !self.signals.same(na, nb)) {
                    self.push(
                        "system-power",
                        true,
                        format!("{where_a} ({sa}) meets {where_b} ({sb}){via}"),
                        &[
                            ("a", where_a.clone()),
                            ("b", where_b.clone()),
                            ("signalA", sa),
                            ("signalB", sb),
                        ],
                        Some(connection),
                        pins,
                        &key,
                    );
                    return "power";
                }
                if is_auto_name(na) || is_auto_name(nb) || self.signals.same(na, nb) {
                    return "ok";
                }
                if swapped {
                    return "swapped";
                }
                self.push(
                    "system-mismatch",
                    true,
                    format!("{where_a} carries {sa} but {where_b} carries {sb}{via}"),
                    &[
                        ("variant", "pin".to_owned()),
                        ("a", where_a.clone()),
                        ("b", where_b.clone()),
                        ("signalA", sa),
                        ("signalB", sb),
                    ],
                    Some(connection),
                    pins,
                    &key,
                );
                "mismatch"
            }
        }
    }
}

/// The pin pairs of two connectors.
fn pair_pins<'p>(
    pairing: Pairing,
    a: &'p Connector,
    b: &'p Connector,
    positions: Option<(&ew_board3d::place::Matrix, &ew_board3d::place::Matrix)>,
) -> Vec<(Option<&'p Pin>, Option<&'p Pin>)> {
    let mut out = Vec::new();
    let mut used = vec![false; b.pins.len()];
    for pa in &a.pins {
        let partner = match pairing {
            Pairing::Number => b.pins.iter().position(|pb| pb.number == pa.number),
            Pairing::Mirrored => pa.number.parse::<u64>().ok().and_then(|n| {
                let other = if n % 2 == 1 { n + 1 } else { n - 1 };
                b.pins
                    .iter()
                    .position(|pb| pb.number.parse::<u64>().ok() == Some(other))
            }),
            Pairing::Position => positions.and_then(|(ma, mb)| {
                let p = ma.apply([pa.at[0], pa.at[1], 0.0]);
                b.pins
                    .iter()
                    .enumerate()
                    .map(|(i, pb)| {
                        let q = mb.apply([pb.at[0], pb.at[1], 0.0]);
                        (i, (p[0] - q[0]).hypot(p[1] - q[1]))
                    })
                    .filter(|(_, d)| *d < 0.5)
                    .min_by(|x, y| x.1.total_cmp(&y.1))
                    .map(|(i, _)| i)
            }),
        };
        match partner {
            Some(i) if !used[i] => {
                used[i] = true;
                out.push((Some(pa), Some(&b.pins[i])));
            }
            _ => out.push((Some(pa), None)),
        }
    }
    for (i, pb) in b.pins.iter().enumerate() {
        if !used[i] {
            out.push((None, Some(pb)));
        }
    }
    out
}

/// The placement of a system board (mm, z from its bottom face).
#[must_use]
pub fn board_matrix(board: &SystemBoard) -> ew_board3d::place::Matrix {
    use ew_board3d::place::Matrix;
    let p = board.position;
    let [rx, ry, rz] = board.rotation.map(|a| a.degrees().to_radians());
    #[allow(clippy::cast_precision_loss, reason = "board coordinates fit f64")]
    Matrix::translation(p.x as f64 / 1e6, p.y as f64 / 1e6, p.z as f64 / 1e6)
        .mul(&Matrix::rotation(2, rz))
        .mul(&Matrix::rotation(1, ry))
        .mul(&Matrix::rotation(0, rx))
}

/// Runs the connection check of a system.
#[must_use]
#[allow(clippy::too_many_lines, reason = "links, then cables")]
pub fn check(system: &System, loaded: &[Loaded]) -> Checked {
    let mut c = Checker {
        boards: loaded
            .iter()
            .map(|l| (l.entry.id, Board { loaded: l }))
            .collect(),
        signals: Signals {
            aliases: &system.aliases,
        },
        exclusions: system.exclusions.iter().map(String::as_str).collect(),
        out: Checked::default(),
    };
    // Boards that cannot be read.
    for l in loaded {
        if let Some(error) = &l.error {
            c.push(
                "system-connector",
                true,
                format!("The board {} cannot be read: {error}", l.entry.name),
                &[
                    ("variant", "board".to_owned()),
                    ("board", l.entry.name.clone()),
                    ("error", error.clone()),
                ],
                None,
                Vec::new(),
                &l.entry.id.to_string(),
            );
        }
    }
    for link in &system.links {
        let name = format!("{} ↔ {}", c.label(&link.a), c.label(&link.b));
        let (ca, cb) = match (c.connector(&link.a), c.connector(&link.b)) {
            (Ok(a), Ok(b)) => (a, b),
            (a, b) => {
                for (r, result) in [(&link.a, a), (&link.b, b)] {
                    if let Err(error) = result {
                        c.push(
                            "system-connector",
                            true,
                            format!("The connection {name} refers to {error}"),
                            &[
                                ("variant", "missing".to_owned()),
                                ("connection", name.clone()),
                                ("designator", r.designator.clone()),
                            ],
                            Some(link.id),
                            Vec::new(),
                            &format!("{}/{}", link.id, r.designator),
                        );
                    }
                }
                c.out.connections.push(ConnectionReport {
                    id: link.id,
                    kind: "link",
                    name,
                    pairs: Vec::new(),
                });
                continue;
            }
        };
        if ca.pins.len() != cb.pins.len() {
            c.push(
                "system-pins",
                false,
                format!(
                    "{} has {} pins but {} has {}",
                    c.label(&link.a),
                    ca.pins.len(),
                    c.label(&link.b),
                    cb.pins.len()
                ),
                &[
                    ("variant", "count".to_owned()),
                    ("a", c.label(&link.a)),
                    ("b", c.label(&link.b)),
                    ("countA", ca.pins.len().to_string()),
                    ("countB", cb.pins.len().to_string()),
                ],
                Some(link.id),
                Vec::new(),
                &format!("{}/count", link.id),
            );
        }
        let matrices = (link.pairing == Pairing::Position).then(|| {
            (
                c.boards
                    .get(&link.a.board)
                    .map(|b| board_matrix(&b.loaded.entry)),
                c.boards
                    .get(&link.b.board)
                    .map(|b| board_matrix(&b.loaded.entry)),
            )
        });
        let positions = match &matrices {
            Some((Some(ma), Some(mb))) => Some((ma, mb)),
            _ => None,
        };
        let pairs = pair_pins(link.pairing, &ca, &cb, positions);
        // Swapped signals: a pair whose signals appear crosswise on another
        // pair.
        let named = |p: Option<&Pin>| {
            p.and_then(|p| p.net.as_deref())
                .filter(|n| !is_auto_name(n))
                .map(|n| c.signals.canonical(n))
        };
        let signals: Vec<(Option<String>, Option<String>)> =
            pairs.iter().map(|(a, b)| (named(*a), named(*b))).collect();
        let mut swapped: Vec<Option<usize>> = vec![None; pairs.len()];
        for i in 0..pairs.len() {
            if let (Some(x), Some(y)) = &signals[i]
                && x != y
            {
                swapped[i] = (0..pairs.len()).find(|&j| {
                    j != i && signals[j].0.as_ref() == Some(y) && signals[j].1.as_ref() == Some(x)
                });
            }
        }
        let mut report = ConnectionReport {
            id: link.id,
            kind: "link",
            name: name.clone(),
            pairs: Vec::new(),
        };
        let mut reported_swaps = BTreeSet::new();
        for (i, (pa, pb)) in pairs.iter().enumerate() {
            let status = match (pa, pb) {
                (Some(pa), Some(pb)) => {
                    let status =
                        c.compare(link.id, &link.a, pa, &link.b, pb, swapped[i].is_some(), "");
                    if status == "swapped"
                        && let Some(j) = swapped[i]
                        && reported_swaps.insert((i.min(j), i.max(j)))
                        && let (Some(qa), Some(qb)) = pairs[j]
                    {
                        let (sa, sb) = (
                            pa.net
                                .as_deref()
                                .map(short_name)
                                .unwrap_or_default()
                                .to_owned(),
                            pb.net
                                .as_deref()
                                .map(short_name)
                                .unwrap_or_default()
                                .to_owned(),
                        );
                        c.push(
                            "system-mismatch",
                            true,
                            format!(
                                "{sa} and {sb} are swapped: {} pins {} and {} meet {} pins {} and {}",
                                c.label(&link.a),
                                pa.number,
                                qa.number,
                                c.label(&link.b),
                                pb.number,
                                qb.number
                            ),
                            &[
                                ("variant", "swapped".to_owned()),
                                ("signalA", sa),
                                ("signalB", sb),
                                ("a", c.label(&link.a)),
                                ("b", c.label(&link.b)),
                                ("pinsA", format!("{}, {}", pa.number, qa.number)),
                                ("pinsB", format!("{}, {}", pb.number, qb.number)),
                            ],
                            Some(link.id),
                            vec![
                                PinRef {
                                    board: link.a.board,
                                    designator: link.a.designator.clone(),
                                    pin: pa.number.clone(),
                                },
                                PinRef {
                                    board: link.a.board,
                                    designator: link.a.designator.clone(),
                                    pin: qa.number.clone(),
                                },
                            ],
                            &format!("{}/swap/{}/{}", link.id, pa.number, qa.number),
                        );
                    }
                    status
                }
                (Some(p), None) | (None, Some(p)) => {
                    let side = if pa.is_some() { &link.a } else { &link.b };
                    if let Some(net) = p.net.as_deref().filter(|n| !is_auto_name(n)) {
                        c.push(
                            "system-pins",
                            false,
                            format!(
                                "{} pin {} ({}) has no partner on the other connector",
                                c.label(side),
                                p.number,
                                short_name(net)
                            ),
                            &[
                                ("variant", "unmated".to_owned()),
                                ("pin", format!("{} pin {}", c.label(side), p.number)),
                                ("signal", short_name(net).to_owned()),
                            ],
                            Some(link.id),
                            vec![PinRef {
                                board: side.board,
                                designator: side.designator.clone(),
                                pin: p.number.clone(),
                            }],
                            &format!("{}/unmated/{}/{}", link.id, side.designator, p.number),
                        );
                        "unmated"
                    } else {
                        "unused"
                    }
                }
                (None, None) => "unused",
            };
            report.pairs.push(PinPair {
                a: pa.map(|p| p.number.clone()),
                a_net: pa.and_then(|p| p.net.clone()),
                b: pb.map(|p| p.number.clone()),
                b_net: pb.and_then(|p| p.net.clone()),
                wire: None,
                status: status.to_owned(),
            });
        }
        c.out.connections.push(report);
    }
    for cable in &system.cables {
        let mut report = ConnectionReport {
            id: cable.id,
            kind: "cable",
            name: cable.name.clone(),
            pairs: Vec::new(),
        };
        // The connectors of the ends.
        let mut ends: HashMap<Id, (Option<ConnectorRef>, Option<Connector>, String)> =
            HashMap::new();
        for end in &cable.ends {
            let connector = match &end.connector {
                Some(r) => match c.connector(r) {
                    Ok(found) => Some(found),
                    Err(error) => {
                        c.push(
                            "system-connector",
                            true,
                            format!("Cable {} refers to {error}", cable.name),
                            &[
                                ("variant", "cable".to_owned()),
                                ("cable", cable.name.clone()),
                                ("designator", r.designator.clone()),
                            ],
                            Some(cable.id),
                            Vec::new(),
                            &format!("{}/{}", cable.id, end.id),
                        );
                        None
                    }
                },
                None => None,
            };
            let label = end
                .connector
                .as_ref()
                .map_or_else(|| end.label.clone(), |r| c.label(r));
            ends.insert(end.id, (end.connector.clone(), connector, label));
        }
        for wire in &cable.wires {
            let side = |we: &WireEnd| ends.get(&we.end);
            let (Some((ra, ca, la)), Some((rb, cb, lb))) = (side(&wire.from), side(&wire.to))
            else {
                continue;
            };
            let pin_of = |conn: &Option<Connector>, pin: &str| {
                conn.as_ref()
                    .and_then(|c| c.pins.iter().find(|p| p.number == pin).cloned())
            };
            let pa = pin_of(ca, &wire.from.pin);
            let pb = pin_of(cb, &wire.to.pin);
            let mut status = "ok";
            for (conn, pin, label, number) in
                [(ca, &pa, la, &wire.from.pin), (cb, &pb, lb, &wire.to.pin)]
            {
                if conn.is_some() && pin.is_none() {
                    status = "open";
                    c.push(
                        "system-cable",
                        false,
                        format!(
                            "A wire of {} ends on {label} pin {number}, which does not exist",
                            cable.name
                        ),
                        &[
                            ("variant", "pin".to_owned()),
                            ("cable", cable.name.clone()),
                            ("pin", format!("{label} pin {number}")),
                        ],
                        Some(cable.id),
                        Vec::new(),
                        &format!("{}/{}/pin/{number}", cable.id, wire.id),
                    );
                }
            }
            let via = format!(" (cable {})", cable.name);
            if let (Some(ra), Some(rb), Some(pa), Some(pb)) = (ra, rb, &pa, &pb) {
                status = c.compare(cable.id, ra, pa, rb, pb, false, &via);
                if status == "unused" {
                    status = "open";
                    c.push(
                        "system-cable",
                        false,
                        format!(
                            "A wire of {} joins {la} pin {} and {lb} pin {}, which are both unconnected",
                            cable.name, pa.number, pb.number
                        ),
                        &[
                            ("variant", "open".to_owned()),
                            ("cable", cable.name.clone()),
                            ("a", format!("{la} pin {}", pa.number)),
                            ("b", format!("{lb} pin {}", pb.number)),
                        ],
                        Some(cable.id),
                        Vec::new(),
                        &format!("{}/{}/open", cable.id, wire.id),
                    );
                }
            }
            report.pairs.push(PinPair {
                a: Some(wire.from.pin.clone()),
                a_net: pa.and_then(|p| p.net),
                b: Some(wire.to.pin.clone()),
                b_net: pb.and_then(|p| p.net),
                wire: Some(wire.id),
                status: status.to_owned(),
            });
        }
        // Connector pins with a signal and no wire.
        for end in &cable.ends {
            let Some((Some(r), Some(conn), label)) = ends.get(&end.id) else {
                continue;
            };
            for pin in &conn.pins {
                let wired = cable.wires.iter().any(|w| {
                    (w.from.end == end.id && w.from.pin == pin.number)
                        || (w.to.end == end.id && w.to.pin == pin.number)
                });
                if wired {
                    continue;
                }
                if let Some(net) = pin.net.as_deref().filter(|n| !is_auto_name(n)) {
                    c.push(
                        "system-cable",
                        false,
                        format!(
                            "{label} pin {} ({}) has no wire in {}",
                            pin.number,
                            short_name(net),
                            cable.name
                        ),
                        &[
                            ("variant", "unwired".to_owned()),
                            ("cable", cable.name.clone()),
                            ("pin", format!("{label} pin {}", pin.number)),
                            ("signal", short_name(net).to_owned()),
                        ],
                        Some(cable.id),
                        vec![PinRef {
                            board: r.board,
                            designator: r.designator.clone(),
                            pin: pin.number.clone(),
                        }],
                        &format!("{}/unwired/{}/{}", cable.id, end.id, pin.number),
                    );
                }
            }
        }
        c.out.connections.push(report);
    }
    c.out.findings.sort_by(|a, b| {
        (a.severity != "error")
            .cmp(&(b.severity != "error"))
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.key.cmp(&b.key))
    });
    c.out
}

// ---------------------------------------------------------------------------
// Signals across boards.

/// A pin on another board connected to a signal.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counterpart {
    /// System board.
    pub board: Id,
    /// Its name.
    pub board_name: String,
    /// `true` for a board of this project.
    pub local: bool,
    /// Connector.
    pub designator: String,
    /// Pin.
    pub pin: String,
    /// The net on that board.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net: Option<String>,
}

/// The pins on other boards connected to a net of a board (through links
/// and cables, transitively).
#[must_use]
#[allow(clippy::too_many_lines, reason = "edges, then a walk over them")]
pub fn counterparts(system: &System, loaded: &[Loaded], board: Id, net: &str) -> Vec<Counterpart> {
    let by_id: HashMap<Id, &Loaded> = loaded.iter().map(|l| (l.entry.id, l)).collect();
    let connector = |r: &ConnectorRef| {
        by_id
            .get(&r.board)
            .and_then(|l| l.pcb.as_ref())
            .and_then(|pcb| find_connector(pcb, &r.designator))
    };
    // Every connection as pin-to-pin edges.
    let mut edges: Vec<(PinRef, PinRef)> = Vec::new();
    for link in &system.links {
        if let (Some(ca), Some(cb)) = (connector(&link.a), connector(&link.b)) {
            let ma = by_id.get(&link.a.board).map(|l| board_matrix(&l.entry));
            let mb = by_id.get(&link.b.board).map(|l| board_matrix(&l.entry));
            let positions = match (&ma, &mb) {
                (Some(a), Some(b)) => Some((a, b)),
                _ => None,
            };
            for (pa, pb) in pair_pins(link.pairing, &ca, &cb, positions) {
                if let (Some(pa), Some(pb)) = (pa, pb) {
                    edges.push((
                        PinRef {
                            board: link.a.board,
                            designator: link.a.designator.clone(),
                            pin: pa.number.clone(),
                        },
                        PinRef {
                            board: link.b.board,
                            designator: link.b.designator.clone(),
                            pin: pb.number.clone(),
                        },
                    ));
                }
            }
        }
    }
    for cable in &system.cables {
        let end = |id: Id| {
            cable
                .ends
                .iter()
                .find(|e| e.id == id)
                .and_then(|e| e.connector.clone())
        };
        for wire in &cable.wires {
            if let (Some(a), Some(b)) = (end(wire.from.end), end(wire.to.end)) {
                edges.push((
                    PinRef {
                        board: a.board,
                        designator: a.designator,
                        pin: wire.from.pin.clone(),
                    },
                    PinRef {
                        board: b.board,
                        designator: b.designator,
                        pin: wire.to.pin.clone(),
                    },
                ));
            }
        }
    }
    let net_of = |p: &PinRef| -> Option<String> {
        connector(&ConnectorRef {
            board: p.board,
            designator: p.designator.clone(),
        })
        .and_then(|c| c.pins.into_iter().find(|q| q.number == p.pin))
        .and_then(|q| q.net)
    };
    // Start: the pins of the net on the board's connectors that take part.
    let mut seen: BTreeSet<PinRef> = BTreeSet::new();
    let mut queue: Vec<PinRef> = Vec::new();
    for (a, b) in &edges {
        for p in [a, b] {
            if p.board == board
                && net_of(p).is_some_and(|n| short_name(&n).eq_ignore_ascii_case(short_name(net)))
                && seen.insert(p.clone())
            {
                queue.push(p.clone());
            }
        }
    }
    let mut out = Vec::new();
    while let Some(p) = queue.pop() {
        for (a, b) in &edges {
            let other = if *a == p {
                b
            } else if *b == p {
                a
            } else {
                continue;
            };
            if !seen.insert(other.clone()) {
                continue;
            }
            let other_net = net_of(other);
            out.push(Counterpart {
                board: other.board,
                board_name: by_id
                    .get(&other.board)
                    .map_or_else(String::new, |l| l.entry.name.clone()),
                local: by_id.get(&other.board).is_some_and(|l| l.local),
                designator: other.designator.clone(),
                pin: other.pin.clone(),
                net: other_net.clone(),
            });
            // Follow the signal through the other board's net (to its
            // other connectors).
            if let Some(n) = other_net {
                for (x, y) in &edges {
                    for q in [x, y] {
                        if q.board == other.board
                            && net_of(q).is_some_and(|m| m == n)
                            && seen.insert(q.clone())
                        {
                            queue.push(q.clone());
                        }
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| {
        a.board_name
            .cmp(&b.board_name)
            .then_with(|| ew_board3d::place::natural(&a.designator, &b.designator))
            .then_with(|| ew_board3d::place::natural(&a.pin, &b.pin))
    });
    out
}

// ---------------------------------------------------------------------------
// Methods.

fn parse<T: for<'de> Deserialize<'de>>(params: &Value) -> Result<T, EngineError> {
    serde_json::from_value(params.clone()).map_err(|e| EngineError::invalid_params(e.to_string()))
}

fn to_value<T: Serialize>(value: &T) -> Result<Value, EngineError> {
    serde_json::to_value(value).map_err(EngineError::internal)
}

fn system_of(project: &Project, doc: Id) -> Result<&System, EngineError> {
    match project.document(doc) {
        Some(Document::System(system)) => Ok(system),
        Some(_) => Err(EngineError::invalid_params("the document is not a system")),
        None => Err(EngineError::not_found(format!("no document {doc}"))),
    }
}

/// A request that replaces some properties of a system.
fn set(doc: Id, fields: &[(&str, Value)]) -> Request {
    let mut props = Map::new();
    for (k, v) in fields {
        props.insert((*k).to_owned(), v.clone());
    }
    Request::SetProps { doc, props }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DocParams {
    doc: Id,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AddBoardParams {
    doc: Id,
    source: BoardSource,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    quantity: Option<u32>,
    #[serde(default)]
    position: Option<Vec3>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateBoardParams {
    doc: Id,
    id: Id,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    quantity: Option<u32>,
    #[serde(default)]
    position: Option<Vec3>,
    #[serde(default)]
    rotation: Option<[Angle; 3]>,
    #[serde(default)]
    hidden: Option<bool>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IdParams {
    doc: Id,
    id: Id,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BoardParams {
    doc: Id,
    board: Id,
    #[serde(default)]
    all: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PinsParams {
    doc: Id,
    board: Id,
    designator: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AddLinkParams {
    doc: Id,
    a: ConnectorRef,
    b: ConnectorRef,
    #[serde(default)]
    pairing: Pairing,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateLinkParams {
    doc: Id,
    id: Id,
    #[serde(default)]
    a: Option<ConnectorRef>,
    #[serde(default)]
    b: Option<ConnectorRef>,
    #[serde(default)]
    pairing: Option<Pairing>,
}

/// A cable end as clients send it (ids are made when missing).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EndInput {
    #[serde(default)]
    id: Option<Id>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    connector: Option<ConnectorRef>,
    #[serde(default)]
    housing: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireInput {
    #[serde(default)]
    id: Option<Id>,
    from: WireEnd,
    to: WireEnd,
    #[serde(default)]
    signal: String,
    #[serde(default)]
    color: String,
    #[serde(default)]
    gauge: String,
    #[serde(default)]
    length: Option<Nm>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AddCableParams {
    doc: Id,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    part: String,
    #[serde(default)]
    length: Nm,
    #[serde(default)]
    ends: Vec<EndInput>,
    #[serde(default)]
    wires: Vec<WireInput>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct UpdateCableParams {
    doc: Id,
    id: Id,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    part: Option<String>,
    #[serde(default)]
    length: Option<Nm>,
    #[serde(default)]
    ends: Option<Vec<EndInput>>,
    #[serde(default)]
    wires: Option<Vec<WireInput>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WireCableParams {
    doc: Id,
    cable: Id,
    from: Id,
    to: Id,
    #[serde(default)]
    color: String,
    #[serde(default)]
    gauge: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ExcludeParams {
    doc: Id,
    keys: Vec<String>,
    excluded: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SignalParams {
    doc: Id,
    board: Id,
    net: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RenameParams {
    doc: Id,
    board: Id,
    net: String,
    to: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AliasesParams {
    doc: Id,
    aliases: Vec<Vec<String>>,
}

fn ends_of(inputs: Vec<EndInput>) -> Vec<CableEnd> {
    inputs
        .into_iter()
        .enumerate()
        .map(|(i, e)| CableEnd {
            id: e.id.unwrap_or_else(Id::random),
            label: e.label.unwrap_or_else(|| format!("X{}", i + 1)),
            connector: e.connector,
            housing: e.housing,
        })
        .collect()
}

fn wires_of(inputs: Vec<WireInput>) -> Vec<Wire> {
    inputs
        .into_iter()
        .map(|w| Wire {
            id: w.id.unwrap_or_else(Id::random),
            from: w.from,
            to: w.to,
            signal: w.signal,
            color: w.color,
            gauge: w.gauge,
            length: w.length,
        })
        .collect()
}

/// A path to another project, relative to this project's folder when it
/// can be.
fn relative_path(root: &Path, path: &Path) -> String {
    let rel = pathdiff(path, root).unwrap_or_else(|| path.to_path_buf());
    rel.to_string_lossy().replace('\\', "/")
}

/// `path` relative to `base` (both absolute).
fn pathdiff(path: &Path, base: &Path) -> Option<PathBuf> {
    use std::path::Component;
    if !path.is_absolute() || !base.is_absolute() {
        return None;
    }
    let p: Vec<Component<'_>> = path.components().collect();
    let b: Vec<Component<'_>> = base.components().collect();
    let common = p.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if common == 0 {
        return None;
    }
    let mut out = PathBuf::new();
    for _ in common..b.len() {
        out.push("..");
    }
    for c in &p[common..] {
        out.push(c.as_os_str());
    }
    Some(out)
}

fn board_json(l: &Loaded) -> Result<Value, EngineError> {
    let mut value = to_value(&l.entry)?;
    if let Some(object) = value.as_object_mut() {
        object.insert("quantity".into(), Value::from(l.entry.quantity));
        object.insert("position".into(), to_value(&l.entry.position)?);
        object.insert("rotation".into(), to_value(&l.entry.rotation)?);
        object.insert("hidden".into(), Value::Bool(l.entry.hidden));
        object.insert("local".into(), Value::Bool(l.local));
        object.insert("project".into(), Value::String(l.project.clone()));
        object.insert(
            "root".into(),
            Value::String(l.root.to_string_lossy().into_owned()),
        );
        object.insert("loaded".into(), Value::Bool(l.pcb.is_some()));
        if let Some(error) = &l.error {
            object.insert("error".into(), Value::String(error.clone()));
        }
        if let Some(pcb) = &l.pcb {
            object.insert("board".into(), Value::String(pcb.name.clone()));
            object.insert("boardId".into(), Value::String(pcb.id.to_string()));
            if let Some(b) = pcb.outline_bounds() {
                #[allow(clippy::cast_precision_loss, reason = "millimetres")]
                object.insert(
                    "size".into(),
                    json!([b.width() as f64 / 1e6, b.height() as f64 / 1e6]),
                );
            }
            object.insert(
                "connectors".into(),
                Value::from(
                    pcb.footprints
                        .iter()
                        .filter(|f| looks_like_connector(f))
                        .count(),
                ),
            );
        }
    }
    Ok(value)
}

/// Runs a `system.*` method.
///
/// # Errors
///
/// Bad parameters, unknown documents, boards, links or cables.
#[allow(clippy::too_many_lines, reason = "one match over the methods")]
pub fn call(
    session: Option<&Session>,
    cache: &mut Cache,
    method: &str,
    params: &Value,
) -> Result<Outcome, EngineError> {
    let session = session.ok_or_else(EngineError::no_project)?;
    let project = session.project();
    match method {
        "system.info" => {
            let p: DocParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            let loaded = load(session, cache, system);
            let boards = loaded
                .iter()
                .map(board_json)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Outcome::Read(json!({
                "id": system.id,
                "name": system.name,
                "boards": boards,
                "links": system.links,
                "cables": system.cables,
                "aliases": system.aliases,
                "exclusions": system.exclusions,
            })))
        }
        "system.addBoard" => {
            let p: AddBoardParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            let mut source = p.source;
            let default_name = match &mut source {
                BoardSource::Local { doc } => match project.document(*doc) {
                    Some(Document::Pcb(pcb)) => pcb.name.clone(),
                    _ => return Err(EngineError::not_found(format!("no board {doc}"))),
                },
                BoardSource::Project { path, board } => {
                    let file = path_of(session.root(), path);
                    let loaded = cache.project(&file).map_err(EngineError::invalid_params)?;
                    let pcb = first_board(&loaded.project, board).ok_or_else(|| {
                        EngineError::invalid_params(format!("the project has no board {board}"))
                    })?;
                    let manifest = ew_format::store::find_manifest(&file)
                        .map_err(|e| EngineError::invalid_params(e.to_string()))?;
                    *path = relative_path(session.root(), &manifest);
                    pcb.name.clone()
                }
            };
            let mut name = p.name.unwrap_or(default_name);
            if system.boards.iter().any(|b| b.name == name) {
                let base = name.clone();
                name = (2..=system.boards.len() + 2)
                    .map(|n| format!("{base} {n}"))
                    .find(|n| !system.boards.iter().any(|b| &b.name == n))
                    .unwrap_or(base);
            }
            let board = SystemBoard {
                id: Id::random(),
                name,
                source,
                quantity: p.quantity.unwrap_or(1).max(1),
                position: p.position.unwrap_or_default(),
                rotation: [Angle::ZERO; 3],
                hidden: false,
            };
            let id = board.id;
            let mut boards = system.boards.clone();
            boards.push(board);
            Ok(Outcome::Commit {
                label: BOARDS,
                requests: vec![set(p.doc, &[("boards", to_value(&boards)?)])],
                reply: json!({"id": id}),
            })
        }
        "system.updateBoard" => {
            let p: UpdateBoardParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            let mut boards = system.boards.clone();
            let board = boards
                .iter_mut()
                .find(|b| b.id == p.id)
                .ok_or_else(|| EngineError::not_found(format!("no system board {}", p.id)))?;
            if let Some(name) = p.name {
                if name.trim().is_empty() {
                    return Err(EngineError::invalid_params("the name must not be empty"));
                }
                board.name = name;
            }
            if let Some(q) = p.quantity {
                board.quantity = q.max(1);
            }
            if let Some(position) = p.position {
                board.position = position;
            }
            if let Some(rotation) = p.rotation {
                board.rotation = rotation;
            }
            if let Some(hidden) = p.hidden {
                board.hidden = hidden;
            }
            Ok(Outcome::Commit {
                label: BOARDS,
                requests: vec![set(p.doc, &[("boards", to_value(&boards)?)])],
                reply: json!({"id": p.id}),
            })
        }
        "system.removeBoard" => {
            let p: IdParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            if system.board(p.id).is_none() {
                return Err(EngineError::not_found(format!("no system board {}", p.id)));
            }
            let boards: Vec<&SystemBoard> = system.boards.iter().filter(|b| b.id != p.id).collect();
            let links: Vec<&Link> = system
                .links
                .iter()
                .filter(|l| l.a.board != p.id && l.b.board != p.id)
                .collect();
            let cables: Vec<Cable> = system
                .cables
                .iter()
                .cloned()
                .map(|mut c| {
                    for end in &mut c.ends {
                        if end.connector.as_ref().is_some_and(|r| r.board == p.id) {
                            end.connector = None;
                        }
                    }
                    c
                })
                .collect();
            Ok(Outcome::Commit {
                label: BOARDS,
                requests: vec![set(
                    p.doc,
                    &[
                        ("boards", to_value(&boards)?),
                        ("links", to_value(&links)?),
                        ("cables", to_value(&cables)?),
                    ],
                )],
                reply: json!({"removed": p.id}),
            })
        }
        "system.connectors" => {
            let p: BoardParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            let loaded = load(session, cache, system);
            let l = loaded
                .iter()
                .find(|l| l.entry.id == p.board)
                .ok_or_else(|| EngineError::not_found(format!("no system board {}", p.board)))?;
            let pcb = l
                .pcb
                .as_ref()
                .ok_or_else(|| EngineError::invalid_params(l.error.clone().unwrap_or_default()))?;
            let mut list: Vec<Value> = pcb
                .footprints
                .iter()
                .filter(|fp| p.all || looks_like_connector(fp))
                .map(|fp| {
                    let c = connector_of(pcb, fp);
                    json!({
                        "designator": c.designator,
                        "footprint": c.footprint,
                        "value": c.value,
                        "side": c.side,
                        "pins": c.pins.len(),
                        "signals": c.pins.iter().filter(|p| p.net.is_some()).count(),
                    })
                })
                .collect();
            list.sort_by(|a, b| {
                ew_board3d::place::natural(
                    a["designator"].as_str().unwrap_or(""),
                    b["designator"].as_str().unwrap_or(""),
                )
            });
            Ok(Outcome::Read(json!({"connectors": list})))
        }
        "system.pins" => {
            let p: PinsParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            let loaded = load(session, cache, system);
            let pcb = loaded
                .iter()
                .find(|l| l.entry.id == p.board)
                .and_then(|l| l.pcb.clone())
                .ok_or_else(|| EngineError::not_found(format!("no system board {}", p.board)))?;
            let c = find_connector(&pcb, &p.designator).ok_or_else(|| {
                EngineError::not_found(format!("the board has no {}", p.designator))
            })?;
            Ok(Outcome::Read(to_value(&c)?))
        }
        "system.addLink" => {
            let p: AddLinkParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            for r in [&p.a, &p.b] {
                if system.board(r.board).is_none() {
                    return Err(EngineError::not_found(format!(
                        "no system board {}",
                        r.board
                    )));
                }
            }
            if p.a.board == p.b.board {
                return Err(EngineError::invalid_params(
                    "a connection joins two different boards",
                ));
            }
            let link = Link {
                id: Id::random(),
                a: p.a,
                b: p.b,
                pairing: p.pairing,
            };
            let id = link.id;
            let mut links = system.links.clone();
            links.push(link);
            Ok(Outcome::Commit {
                label: LINKS,
                requests: vec![set(p.doc, &[("links", to_value(&links)?)])],
                reply: json!({"id": id}),
            })
        }
        "system.updateLink" => {
            let p: UpdateLinkParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            let mut links = system.links.clone();
            let link = links
                .iter_mut()
                .find(|l| l.id == p.id)
                .ok_or_else(|| EngineError::not_found(format!("no connection {}", p.id)))?;
            if let Some(a) = p.a {
                link.a = a;
            }
            if let Some(b) = p.b {
                link.b = b;
            }
            if let Some(pairing) = p.pairing {
                link.pairing = pairing;
            }
            Ok(Outcome::Commit {
                label: LINKS,
                requests: vec![set(p.doc, &[("links", to_value(&links)?)])],
                reply: json!({"id": p.id}),
            })
        }
        "system.removeLink" => {
            let p: IdParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            if !system.links.iter().any(|l| l.id == p.id) {
                return Err(EngineError::not_found(format!("no connection {}", p.id)));
            }
            let links: Vec<&Link> = system.links.iter().filter(|l| l.id != p.id).collect();
            Ok(Outcome::Commit {
                label: LINKS,
                requests: vec![set(p.doc, &[("links", to_value(&links)?)])],
                reply: json!({"removed": p.id}),
            })
        }
        "system.addCable" => {
            let p: AddCableParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            let name = p.name.unwrap_or_else(|| {
                (1..=system.cables.len() + 1)
                    .map(|n| format!("W{n}"))
                    .find(|n| !system.cables.iter().any(|c| &c.name == n))
                    .unwrap_or_default()
            });
            let cable = Cable {
                id: Id::random(),
                name,
                part: p.part,
                length: p.length,
                ends: ends_of(p.ends),
                wires: wires_of(p.wires),
            };
            let reply = json!({"id": cable.id, "ends": cable.ends.iter().map(|e| e.id).collect::<Vec<_>>()});
            let mut cables = system.cables.clone();
            cables.push(cable);
            Ok(Outcome::Commit {
                label: CABLES,
                requests: vec![set(p.doc, &[("cables", to_value(&cables)?)])],
                reply,
            })
        }
        "system.updateCable" => {
            let p: UpdateCableParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            let mut cables = system.cables.clone();
            let cable = cables
                .iter_mut()
                .find(|c| c.id == p.id)
                .ok_or_else(|| EngineError::not_found(format!("no cable {}", p.id)))?;
            if let Some(name) = p.name {
                cable.name = name;
            }
            if let Some(part) = p.part {
                cable.part = part;
            }
            if let Some(length) = p.length {
                cable.length = length.max(0);
            }
            if let Some(ends) = p.ends {
                cable.ends = ends_of(ends);
            }
            if let Some(wires) = p.wires {
                cable.wires = wires_of(wires);
            }
            Ok(Outcome::Commit {
                label: CABLES,
                requests: vec![set(p.doc, &[("cables", to_value(&cables)?)])],
                reply: json!({"id": p.id}),
            })
        }
        "system.removeCable" => {
            let p: IdParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            if !system.cables.iter().any(|c| c.id == p.id) {
                return Err(EngineError::not_found(format!("no cable {}", p.id)));
            }
            let cables: Vec<&Cable> = system.cables.iter().filter(|c| c.id != p.id).collect();
            Ok(Outcome::Commit {
                label: CABLES,
                requests: vec![set(p.doc, &[("cables", to_value(&cables)?)])],
                reply: json!({"removed": p.id}),
            })
        }
        "system.wireCable" => {
            // One wire per pin of the `from` end's connector, to the same
            // pin number of the `to` end.
            let p: WireCableParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            let loaded = load(session, cache, system);
            let mut cables = system.cables.clone();
            let cable = cables
                .iter_mut()
                .find(|c| c.id == p.cable)
                .ok_or_else(|| EngineError::not_found(format!("no cable {}", p.cable)))?;
            let from = cable
                .ends
                .iter()
                .find(|e| e.id == p.from)
                .and_then(|e| e.connector.clone())
                .ok_or_else(|| EngineError::invalid_params("the first end has no connector"))?;
            let pcb = loaded
                .iter()
                .find(|l| l.entry.id == from.board)
                .and_then(|l| l.pcb.clone())
                .ok_or_else(|| EngineError::invalid_params("the board cannot be read"))?;
            let conn = find_connector(&pcb, &from.designator).ok_or_else(|| {
                EngineError::not_found(format!("the board has no {}", from.designator))
            })?;
            if !cable.ends.iter().any(|e| e.id == p.to) {
                return Err(EngineError::not_found(format!("no cable end {}", p.to)));
            }
            cable.wires = conn
                .pins
                .iter()
                .map(|pin| Wire {
                    id: Id::random(),
                    from: WireEnd {
                        end: p.from,
                        pin: pin.number.clone(),
                    },
                    to: WireEnd {
                        end: p.to,
                        pin: pin.number.clone(),
                    },
                    signal: String::new(),
                    color: p.color.clone(),
                    gauge: p.gauge.clone(),
                    length: None,
                })
                .collect();
            let count = cable.wires.len();
            Ok(Outcome::Commit {
                label: CABLES,
                requests: vec![set(p.doc, &[("cables", to_value(&cables)?)])],
                reply: json!({"wires": count}),
            })
        }
        "system.check" => {
            let p: DocParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            let loaded = load(session, cache, system);
            Ok(Outcome::Read(to_value(&check(system, &loaded))?))
        }
        "system.exclude" => {
            let p: ExcludeParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            let mut keys: BTreeSet<String> = system.exclusions.iter().cloned().collect();
            for key in p.keys {
                if p.excluded {
                    keys.insert(key);
                } else {
                    keys.remove(&key);
                }
            }
            let keys: Vec<String> = keys.into_iter().collect();
            Ok(Outcome::Commit {
                label: EDIT,
                requests: vec![set(p.doc, &[("exclusions", to_value(&keys)?)])],
                reply: json!({"exclusions": keys.len()}),
            })
        }
        "system.setAliases" => {
            let p: AliasesParams = parse(params)?;
            let aliases: Vec<Vec<String>> = p
                .aliases
                .into_iter()
                .map(|g| {
                    g.into_iter()
                        .filter(|s| !s.trim().is_empty())
                        .collect::<Vec<_>>()
                })
                .filter(|g| g.len() >= 2)
                .collect();
            Ok(Outcome::Commit {
                label: EDIT,
                requests: vec![set(p.doc, &[("aliases", to_value(&aliases)?)])],
                reply: json!({"aliases": aliases.len()}),
            })
        }
        "system.counterparts" => {
            let p: SignalParams = parse(params)?;
            let system = system_of(project, p.doc)?;
            let loaded = load(session, cache, system);
            Ok(Outcome::Read(json!({
                "counterparts": counterparts(system, &loaded, p.board, &p.net),
            })))
        }
        "system.renameSignal" => {
            let p: RenameParams = parse(params)?;
            rename_signal(session, cache, &p)
        }
        other => Err(EngineError::new(
            ErrorCode::MethodNotFound,
            format!("unknown method `{other}`"),
        )),
    }
}

/// Renames a signal on every board of this project that it reaches (the
/// board's own net and the counterparts'): the nets of the boards and the
/// labels of the schematics. Boards of other projects are named in the
/// reply.
fn rename_signal(
    session: &Session,
    cache: &mut Cache,
    p: &RenameParams,
) -> Result<Outcome, EngineError> {
    let project = session.project();
    let system = system_of(project, p.doc)?;
    let to = p.to.trim();
    if to.is_empty() || to.contains('/') {
        return Err(EngineError::invalid_params("not a valid signal name"));
    }
    let loaded = load(session, cache, system);
    let mut nets: BTreeSet<(Id, String)> = BTreeSet::new(); // (board doc, net name)
    let mut others: Vec<Value> = Vec::new();
    let start = loaded
        .iter()
        .find(|l| l.entry.id == p.board)
        .ok_or_else(|| EngineError::not_found(format!("no system board {}", p.board)))?;
    let doc_of = |l: &Loaded| match &l.entry.source {
        BoardSource::Local { doc } => Some(*doc),
        BoardSource::Project { .. } => None,
    };
    if let Some(doc) = doc_of(start) {
        nets.insert((doc, p.net.clone()));
    } else {
        others.push(json!({"board": start.entry.name, "net": p.net}));
    }
    for cp in counterparts(system, &loaded, p.board, &p.net) {
        let Some(net) = cp.net.clone() else { continue };
        if short_name(&net) == to {
            continue;
        }
        let l = loaded.iter().find(|l| l.entry.id == cp.board);
        match l.and_then(doc_of) {
            Some(doc) => {
                nets.insert((doc, net));
            }
            None => others.push(json!({"board": cp.board_name, "net": net})),
        }
    }
    let mut requests = Vec::new();
    let mut renamed = 0;
    let olds: BTreeSet<String> = nets.iter().map(|(_, n)| short_name(n).to_owned()).collect();
    for (doc, old) in &nets {
        if let Some(Document::Pcb(pcb)) = project.document(*doc) {
            for net in pcb.nets.iter().filter(|n| &n.name == old) {
                let prefix = net.name.rsplit_once('/').map_or("", |(a, _)| a);
                let name = if prefix.is_empty() && !net.name.starts_with('/') {
                    to.to_owned()
                } else {
                    format!("{prefix}/{to}")
                };
                let mut fields = Map::new();
                fields.insert("name".into(), Value::String(name));
                requests.push(Request::Patch {
                    doc: *doc,
                    id: net.id,
                    fields,
                });
                renamed += 1;
            }
        }
    }
    // Labels in the schematics with one of the old names.
    for entry in project.documents_of(DocumentKind::Schematic) {
        if let Some(Document::Schematic(sheet)) = project.document(entry.id) {
            for label in sheet.labels.iter().filter(|l| olds.contains(&l.text)) {
                let mut fields = Map::new();
                fields.insert("text".into(), Value::String(to.to_owned()));
                requests.push(Request::Patch {
                    doc: entry.id,
                    id: label.id,
                    fields,
                });
            }
        }
    }
    if requests.is_empty() {
        return Ok(Outcome::Read(json!({"renamed": 0, "others": others})));
    }
    Ok(Outcome::Commit {
        label: RENAME,
        requests,
        reply: json!({"renamed": renamed, "others": others}),
    })
}
