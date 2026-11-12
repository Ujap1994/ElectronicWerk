//! Schematic ↔ board synchronisation.
//!
//! [`parts`] lists the parts of the schematic (one per designator and
//! sheet instance) with the net of every pin. [`diff`] compares them with
//! the footprints of a board and lists the changes an update would make
//! (an engineering change order); [`apply`] makes the selected ones.
//! [`back_diff`] goes the other way: designators, values and footprints
//! changed on the board go back to the schematic.
//!
//! Footprints link to parts by `component` id, then by designator.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::library::FootprintDef;
use ew_model::object::Object;
use ew_model::pcb::{FootprintInstance, Net, Pcb};
use ew_model::project::Project;
use ew_model::schematic::PlacedSymbol;
use ew_model::units::{Angle, Nm, Point, Rect, mm};
use ew_schematic::Connectivity;
use ew_schematic::connectivity::{natural_cmp, symbol_def};
use ew_schematic::edit::{Change as EditChange, Edit};
use serde::Serialize;

/// A part of the schematic, as the board sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    /// The link id the board footprint carries.
    pub key: Id,
    /// Designator.
    pub designator: String,
    /// Value.
    pub value: String,
    /// Footprint name (empty: none assigned).
    pub footprint: String,
    /// Symbol name.
    pub symbol: String,
    /// The placed symbols (units) and their sheets.
    pub units: Vec<(Id, Id)>,
    /// Sheet instance key.
    pub instance: String,
    /// Net of every pin number (`None`: unconnected).
    pub pins: BTreeMap<String, Option<String>>,
    /// Do not populate.
    pub dnp: bool,
}

/// The link id of a part in a sheet instance.
#[must_use]
pub fn part_key(symbol: &PlacedSymbol, instance: &str) -> Id {
    let base = symbol.component.unwrap_or(symbol.id);
    if instance == "/" {
        base
    } else {
        Id::derived(&["part", &base.to_string(), instance])
    }
}

/// The parts of the schematic, sorted by designator. Power symbols,
/// symbols left off the board and unannotated symbols are not parts; the
/// latter are reported as problems.
#[must_use]
#[allow(clippy::too_many_lines, reason = "parts, then their pins")]
pub fn parts(project: &Project, connectivity: &Connectivity) -> (Vec<Part>, Vec<Problem>) {
    let mut by_key: BTreeMap<(String, Id), Part> = BTreeMap::new();
    let mut problems = Vec::new();
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
            if symbol.flags.exclude_from_board {
                continue;
            }
            if designator.is_empty() || designator.ends_with('?') {
                problems.push(Problem {
                    code: "unannotated".into(),
                    message: format!("{} is not annotated", symbol.symbol),
                    designator: designator.to_owned(),
                });
                continue;
            }
            let key = part_key(symbol, &inst.key);
            let footprint = symbol
                .footprint
                .clone()
                .or_else(|| def.and_then(|d| d.footprint.clone()))
                .unwrap_or_default();
            let part = by_key
                .entry((inst.key.clone(), symbol.component.unwrap_or(symbol.id)))
                .or_insert_with(|| Part {
                    key,
                    designator: designator.to_owned(),
                    value: symbol.value.clone(),
                    footprint,
                    symbol: symbol.symbol.clone(),
                    units: Vec::new(),
                    instance: inst.key.clone(),
                    pins: BTreeMap::new(),
                    dnp: symbol.flags.dnp,
                });
            part.units.push((inst.sheet, symbol.id));
        }
    }
    // Pins and their nets.
    let index: HashMap<(usize, Id), (String, Id)> = connectivity
        .instances
        .iter()
        .enumerate()
        .flat_map(|(i, inst)| {
            let sheet = match project.document(inst.sheet) {
                Some(Document::Schematic(sheet)) => Some(sheet),
                _ => None,
            };
            sheet
                .into_iter()
                .flat_map(|s| s.symbols.iter())
                .map(move |symbol| {
                    (
                        (i, symbol.id),
                        (inst.key.clone(), symbol.component.unwrap_or(symbol.id)),
                    )
                })
        })
        .collect();
    let pins_per_net: Vec<usize> = connectivity
        .nets
        .iter()
        .map(|net| {
            net.pins
                .iter()
                .filter(|&&p| !connectivity.pins[p].power_symbol)
                .count()
        })
        .collect();
    for pin in &connectivity.pins {
        if pin.power_symbol {
            continue;
        }
        let Some(group) = index.get(&(pin.instance, pin.symbol)) else {
            continue;
        };
        let Some(part) = by_key.get_mut(group) else {
            continue;
        };
        let net = &connectivity.nets[pin.net];
        let name = (net.named || pins_per_net[pin.net] > 1).then(|| net.name.clone());
        part.pins.insert(pin.number.clone(), name);
    }
    let mut list: Vec<Part> = by_key.into_values().collect();
    list.sort_by(|a, b| natural_cmp(&a.designator, &b.designator));
    // Two parts with one designator cannot both reach the board.
    let mut seen = HashSet::new();
    list.retain(|part| {
        if seen.insert(part.designator.clone()) {
            true
        } else {
            problems.push(Problem {
                code: "duplicate-designator".into(),
                message: format!("{} is used twice", part.designator),
                designator: part.designator.clone(),
            });
            false
        }
    });
    (list, problems)
}

/// Something that keeps a part from reaching the board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// `unannotated`, `no-footprint`, `unknown-footprint`,
    /// `duplicate-designator` or `pin-without-pad`.
    pub code: String,
    /// English description.
    pub message: String,
    /// The part.
    pub designator: String,
}

/// One change of an update.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Change {
    /// Place a new footprint.
    AddFootprint {
        /// Designator.
        designator: String,
        /// Footprint name.
        footprint: String,
        /// Value.
        value: String,
    },
    /// Remove a footprint whose part is gone.
    RemoveFootprint {
        /// The footprint.
        id: Id,
        /// Designator.
        designator: String,
    },
    /// Replace the footprint of a part (position and side stay).
    ChangeFootprint {
        /// The footprint.
        id: Id,
        /// Designator.
        designator: String,
        /// Old footprint name.
        from: String,
        /// New footprint name.
        to: String,
    },
    /// Rename a footprint.
    SetDesignator {
        /// The footprint.
        id: Id,
        /// Old designator.
        from: String,
        /// New designator.
        to: String,
    },
    /// Change the value.
    SetValue {
        /// The footprint.
        id: Id,
        /// Designator.
        designator: String,
        /// Old value.
        from: String,
        /// New value.
        to: String,
    },
    /// Link a footprint to its part (matched by designator).
    Link {
        /// The footprint.
        id: Id,
        /// Designator.
        designator: String,
    },
    /// Add a net.
    AddNet {
        /// Name.
        name: String,
    },
    /// Rename a net (its tracks and zones keep it).
    RenameNet {
        /// The net.
        id: Id,
        /// Old name.
        from: String,
        /// New name.
        to: String,
    },
    /// Remove a net no pad uses any more.
    RemoveNet {
        /// The net.
        id: Id,
        /// Name.
        name: String,
    },
    /// Connect a pad to another net.
    SetPadNet {
        /// The footprint.
        id: Id,
        /// Designator.
        designator: String,
        /// Pad number.
        pad: String,
        /// Old net.
        from: Option<String>,
        /// New net.
        to: Option<String>,
    },
}

impl Change {
    /// An English description.
    #[must_use]
    pub fn describe(&self) -> String {
        let net = |n: &Option<String>| n.clone().unwrap_or_else(|| "no net".into());
        match self {
            Self::AddFootprint {
                designator,
                footprint,
                ..
            } => format!("Add {designator} ({footprint})"),
            Self::RemoveFootprint { designator, .. } => format!("Remove {designator}"),
            Self::ChangeFootprint {
                designator,
                from,
                to,
                ..
            } => format!("Change the footprint of {designator} from {from} to {to}"),
            Self::SetDesignator { from, to, .. } => format!("Rename {from} to {to}"),
            Self::SetValue {
                designator,
                from,
                to,
                ..
            } => format!("Change the value of {designator} from {from} to {to}"),
            Self::Link { designator, .. } => format!("Link {designator} to its symbol"),
            Self::AddNet { name } => format!("Add net {name}"),
            Self::RenameNet { from, to, .. } => format!("Rename net {from} to {to}"),
            Self::RemoveNet { name, .. } => format!("Remove net {name}"),
            Self::SetPadNet {
                designator,
                pad,
                from,
                to,
                ..
            } => format!(
                "Connect pad {designator}-{pad} to {} (was {})",
                net(to),
                net(from)
            ),
        }
    }
}

/// The changes that bring a board in line with the schematic.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Diff {
    /// Changes, in the order they apply.
    pub changes: Vec<Change>,
    /// Parts that cannot be placed.
    pub problems: Vec<Problem>,
}

/// A footprint definition of the project libraries by name.
#[must_use]
pub fn footprint_def<'a>(project: &'a Project, name: &str) -> Option<&'a FootprintDef> {
    project
        .manifest
        .documents
        .iter()
        .find_map(|entry| match project.document(entry.id) {
            Some(Document::Library(library)) => library.footprints.iter().find(|f| f.name == name),
            _ => None,
        })
}

/// Matches parts and footprints: part index → footprint id.
fn matching(parts: &[Part], pcb: &Pcb) -> (HashMap<usize, Id>, HashSet<Id>) {
    let mut matched: HashMap<usize, Id> = HashMap::new();
    let mut used: HashSet<Id> = HashSet::new();
    let by_component: HashMap<Id, Id> = pcb
        .footprints
        .iter()
        .filter_map(|fp| fp.component.map(|c| (c, fp.id)))
        .collect();
    for (index, part) in parts.iter().enumerate() {
        if let Some(id) = by_component.get(&part.key)
            && used.insert(*id)
        {
            matched.insert(index, *id);
        }
    }
    let mut by_designator: HashMap<&str, Vec<Id>> = HashMap::new();
    for fp in &pcb.footprints {
        by_designator
            .entry(fp.designator.as_str())
            .or_default()
            .push(fp.id);
    }
    for (index, part) in parts.iter().enumerate() {
        if matched.contains_key(&index) {
            continue;
        }
        let candidate = by_designator
            .get(part.designator.as_str())
            .and_then(|ids| ids.iter().find(|id| !used.contains(id)).copied());
        if let Some(id) = candidate {
            used.insert(id);
            matched.insert(index, id);
        }
    }
    (matched, used)
}

/// The changes that bring `pcb` in line with the schematic.
#[must_use]
#[allow(clippy::too_many_lines, reason = "the change kinds follow one another")]
pub fn diff(project: &Project, connectivity: &Connectivity, pcb: &Pcb) -> Diff {
    let (parts, mut problems) = parts(project, connectivity);
    let (matched, used) = matching(&parts, pcb);
    let net_names: HashMap<Id, &str> = pcb.nets.iter().map(|n| (n.id, n.name.as_str())).collect();
    let mut changes = Vec::new();
    let mut pad_changes = Vec::new();
    // Which old net becomes which new net, for renames.
    let mut flows: HashMap<String, BTreeSet<Option<String>>> = HashMap::new();
    let mut wanted_nets: BTreeSet<String> = BTreeSet::new();
    for (index, part) in parts.iter().enumerate() {
        wanted_nets.extend(part.pins.values().flatten().cloned());
        let Some(&id) = matched.get(&index) else {
            if part.footprint.is_empty() {
                problems.push(Problem {
                    code: "no-footprint".into(),
                    message: format!("{} has no footprint", part.designator),
                    designator: part.designator.clone(),
                });
            } else if footprint_def(project, &part.footprint).is_none() {
                problems.push(Problem {
                    code: "unknown-footprint".into(),
                    message: format!(
                        "The footprint {} of {} is not in the project libraries",
                        part.footprint, part.designator
                    ),
                    designator: part.designator.clone(),
                });
            } else {
                changes.push(Change::AddFootprint {
                    designator: part.designator.clone(),
                    footprint: part.footprint.clone(),
                    value: part.value.clone(),
                });
            }
            continue;
        };
        let Some(fp) = pcb.footprints.get(id) else {
            continue;
        };
        if fp.component != Some(part.key) {
            changes.push(Change::Link {
                id,
                designator: part.designator.clone(),
            });
        }
        if fp.designator != part.designator {
            changes.push(Change::SetDesignator {
                id,
                from: fp.designator.clone(),
                to: part.designator.clone(),
            });
        }
        if fp.value != part.value {
            changes.push(Change::SetValue {
                id,
                designator: part.designator.clone(),
                from: fp.value.clone(),
                to: part.value.clone(),
            });
        }
        let mut pads: Vec<(String, Option<String>)> = Vec::new();
        if !part.footprint.is_empty() && fp.footprint != part.footprint {
            if footprint_def(project, &part.footprint).is_some() {
                changes.push(Change::ChangeFootprint {
                    id,
                    designator: part.designator.clone(),
                    from: fp.footprint.clone(),
                    to: part.footprint.clone(),
                });
                // The new footprint's pads get their nets with it.
                continue;
            }
            problems.push(Problem {
                code: "unknown-footprint".into(),
                message: format!(
                    "The footprint {} of {} is not in the project libraries",
                    part.footprint, part.designator
                ),
                designator: part.designator.clone(),
            });
        }
        let numbers: HashSet<&str> = fp.pads.iter().map(|p| p.number.as_str()).collect();
        for pin in part.pins.keys() {
            if !numbers.contains(pin.as_str()) {
                problems.push(Problem {
                    code: "pin-without-pad".into(),
                    message: format!("Pin {} of {} has no pad", pin, part.designator),
                    designator: part.designator.clone(),
                });
            }
        }
        for pad in &fp.pads {
            if pad.number.is_empty() {
                continue;
            }
            let target = part.pins.get(&pad.number).cloned().flatten();
            let current = pad
                .net
                .and_then(|n| net_names.get(&n))
                .map(|s| (*s).to_owned());
            if let Some(old) = &current {
                flows.entry(old.clone()).or_default().insert(target.clone());
            }
            if current != target && !pads.iter().any(|(n, _)| *n == pad.number) {
                pads.push((pad.number.clone(), current.clone()));
                pad_changes.push(Change::SetPadNet {
                    id,
                    designator: part.designator.clone(),
                    pad: pad.number.clone(),
                    from: current,
                    to: target,
                });
            }
        }
    }
    for fp in &pcb.footprints {
        if !used.contains(&fp.id) && fp.component.is_some() {
            changes.push(Change::RemoveFootprint {
                id: fp.id,
                designator: fp.designator.clone(),
            });
        }
    }
    // Nets: renames where every pad of an old net moves to one new net that
    // the board does not have yet.
    let existing: HashSet<&str> = pcb.nets.iter().map(|n| n.name.as_str()).collect();
    let mut renamed: HashMap<String, String> = HashMap::new();
    let mut targets_taken: HashSet<String> = HashSet::new();
    let mut old_names: Vec<&String> = flows.keys().collect();
    old_names.sort();
    for old in old_names {
        let targets = &flows[old];
        if targets.len() != 1 || wanted_nets.contains(old) {
            continue;
        }
        let Some(Some(new)) = targets.iter().next() else {
            continue;
        };
        if existing.contains(new.as_str()) || !targets_taken.insert(new.clone()) {
            continue;
        }
        renamed.insert(old.clone(), new.clone());
    }
    let mut net_changes = Vec::new();
    for net in &pcb.nets {
        if let Some(new) = renamed.get(&net.name) {
            net_changes.push(Change::RenameNet {
                id: net.id,
                from: net.name.clone(),
                to: new.clone(),
            });
        }
    }
    let after_rename: HashSet<String> = pcb
        .nets
        .iter()
        .map(|n| {
            renamed
                .get(&n.name)
                .cloned()
                .unwrap_or_else(|| n.name.clone())
        })
        .collect();
    for name in &wanted_nets {
        if !after_rename.contains(name) {
            net_changes.push(Change::AddNet { name: name.clone() });
        }
    }
    // Pad changes that a rename already covers disappear.
    pad_changes.retain(|change| match change {
        Change::SetPadNet {
            from: Some(from),
            to: Some(to),
            ..
        } => renamed.get(from) != Some(to),
        _ => true,
    });
    // Nets no pad uses after the update.
    let mut still_used: HashSet<String> = HashSet::new();
    for (index, part) in parts.iter().enumerate() {
        if matched.contains_key(&index) || !part.footprint.is_empty() {
            still_used.extend(part.pins.values().flatten().cloned());
        }
    }
    for fp in &pcb.footprints {
        if !used.contains(&fp.id) && fp.component.is_none() {
            for pad in &fp.pads {
                if let Some(name) = pad.net.and_then(|n| net_names.get(&n)) {
                    still_used.insert((*name).to_owned());
                }
            }
        }
    }
    for net in &pcb.nets {
        let name = renamed.get(&net.name).unwrap_or(&net.name);
        if !still_used.contains(name) {
            net_changes.push(Change::RemoveNet {
                id: net.id,
                name: net.name.clone(),
            });
        }
    }
    changes.splice(0..0, net_changes);
    changes.extend(pad_changes);
    problems.sort_by(|a, b| natural_cmp(&a.designator, &b.designator));
    Diff { changes, problems }
}

/// A new footprint instance from a library definition.
#[must_use]
pub fn instance(def: &FootprintDef, designator: &str, value: &str, at: Point) -> FootprintInstance {
    let mut pads = def.pads.clone();
    for pad in &mut pads {
        pad.id = Id::random();
        pad.net = None;
    }
    FootprintInstance {
        id: Id::random(),
        component: None,
        footprint: def.name.clone(),
        designator: designator.to_owned(),
        value: value.to_owned(),
        at,
        rotation: Angle::ZERO,
        side: ew_model::layer::Side::Top,
        locked: false,
        pads,
        graphics: def.graphics.clone(),
        texts: def.texts.clone(),
        models: def.models.clone(),
        properties: BTreeMap::new(),
        attributes: def.attributes,
    }
}

/// The extent of a footprint definition around its origin.
fn extent(def: &FootprintDef) -> Rect {
    let mut points: Vec<Point> = Vec::new();
    for pad in &def.pads {
        let (w, h) = (
            pad.size.w.max(pad.size.h) / 2,
            pad.size.w.max(pad.size.h) / 2,
        );
        points.push(Point::new(pad.at.x - w, pad.at.y - h));
        points.push(Point::new(pad.at.x + w, pad.at.y + h));
    }
    for graphic in &def.graphics {
        if let Some(bounds) = graphic.geometry.bounds() {
            points.push(bounds.min);
            points.push(bounds.max);
        }
    }
    Rect::from_points(points).unwrap_or(Rect::from_corners(
        Point::new(-mm(1), -mm(1)),
        Point::new(mm(1), mm(1)),
    ))
}

/// Where new footprints go: in rows below the board outline (or below the
/// footprints already placed).
struct Placer {
    left: Nm,
    right: Nm,
    x: Nm,
    y: Nm,
    row: Nm,
}

impl Placer {
    fn new(pcb: &Pcb) -> Self {
        let bounds = pcb
            .outline_bounds()
            .or_else(|| Rect::from_points(pcb.footprints.iter().map(|fp| fp.at)));
        let (left, right, top) = bounds.map_or((0, mm(100), 0), |b| {
            (b.min.x, b.max.x.max(b.min.x + mm(50)), b.max.y + mm(5))
        });
        Self {
            left,
            right,
            x: left,
            y: top,
            row: 0,
        }
    }

    fn place(&mut self, def: &FootprintDef) -> Point {
        let extent = extent(def);
        let (w, h) = (extent.width() + mm(1), extent.height() + mm(1));
        if self.x + w > self.right && self.x > self.left {
            self.x = self.left;
            self.y += self.row;
            self.row = 0;
        }
        let at = Point::new(self.x - extent.min.x, self.y - extent.min.y);
        self.x += w;
        self.row = self.row.max(h);
        at
    }
}

/// Applies the selected changes (indices into `diff.changes`) to a board.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one arm per change kind")]
pub fn apply(
    project: &Project,
    connectivity: &Connectivity,
    pcb_doc: Id,
    pcb: &Pcb,
    diff: &Diff,
    selected: &[usize],
) -> Edit {
    let (parts, _) = parts(project, connectivity);
    let part_by_designator: HashMap<&str, &Part> =
        parts.iter().map(|p| (p.designator.as_str(), p)).collect();
    let mut nets: Vec<Net> = pcb.nets.iter().cloned().collect();
    let mut footprints: HashMap<Id, FootprintInstance> = HashMap::new();
    let mut removed_footprints: Vec<Id> = Vec::new();
    let mut added: Vec<FootprintInstance> = Vec::new();
    let mut removed_nets: HashSet<Id> = HashSet::new();
    let mut placer = Placer::new(pcb);
    let chosen: BTreeSet<usize> = selected.iter().copied().collect();
    let fp_of = |id: Id, footprints: &mut HashMap<Id, FootprintInstance>| {
        if !footprints.contains_key(&id)
            && let Some(fp) = pcb.footprints.get(id)
        {
            footprints.insert(id, fp.clone());
        }
    };
    let net_id = |nets: &mut Vec<Net>, name: &str| -> Id {
        if let Some(net) = nets.iter().find(|n| n.name == name) {
            return net.id;
        }
        let id = Id::random();
        nets.push(Net {
            id,
            name: name.to_owned(),
            class: String::new(),
        });
        id
    };
    let pad_nets = |fp: &mut FootprintInstance, part: &Part, nets: &mut Vec<Net>| {
        for pad in &mut fp.pads {
            pad.net = part
                .pins
                .get(&pad.number)
                .cloned()
                .flatten()
                .map(|name| net_id(nets, &name));
        }
    };
    for (index, change) in diff.changes.iter().enumerate() {
        if !chosen.contains(&index) {
            continue;
        }
        match change {
            Change::AddNet { name } => {
                net_id(&mut nets, name);
            }
            Change::RenameNet { id, to, .. } => {
                if let Some(net) = nets.iter_mut().find(|n| n.id == *id) {
                    net.name.clone_from(to);
                }
            }
            Change::RemoveNet { id, .. } => {
                removed_nets.insert(*id);
            }
            Change::AddFootprint {
                designator,
                footprint,
                value,
            } => {
                let (Some(def), Some(part)) = (
                    footprint_def(project, footprint),
                    part_by_designator.get(designator.as_str()),
                ) else {
                    continue;
                };
                let at = placer.place(def);
                let mut fp = instance(def, designator, value, at);
                fp.component = Some(part.key);
                fp.attributes.dnp = part.dnp;
                pad_nets(&mut fp, part, &mut nets);
                added.push(fp);
            }
            Change::RemoveFootprint { id, .. } => removed_footprints.push(*id),
            Change::ChangeFootprint {
                id, designator, to, ..
            } => {
                let (Some(def), Some(part)) = (
                    footprint_def(project, to),
                    part_by_designator.get(designator.as_str()),
                ) else {
                    continue;
                };
                fp_of(*id, &mut footprints);
                if let Some(fp) = footprints.get_mut(id) {
                    let fresh = instance(def, &fp.designator, &fp.value, fp.at);
                    fp.footprint = fresh.footprint;
                    fp.pads = fresh.pads;
                    fp.graphics = fresh.graphics;
                    fp.texts = fresh.texts;
                    fp.models = fresh.models;
                    fp.attributes = fresh.attributes;
                    pad_nets(fp, part, &mut nets);
                }
            }
            Change::SetDesignator { id, to, .. } => {
                fp_of(*id, &mut footprints);
                if let Some(fp) = footprints.get_mut(id) {
                    fp.designator.clone_from(to);
                }
            }
            Change::SetValue { id, to, .. } => {
                fp_of(*id, &mut footprints);
                if let Some(fp) = footprints.get_mut(id) {
                    fp.value.clone_from(to);
                }
            }
            Change::Link { id, designator } => {
                fp_of(*id, &mut footprints);
                if let (Some(fp), Some(part)) = (
                    footprints.get_mut(id),
                    part_by_designator.get(designator.as_str()),
                ) {
                    fp.component = Some(part.key);
                }
            }
            Change::SetPadNet { id, pad, to, .. } => {
                let net = to.as_ref().map(|name| net_id(&mut nets, name));
                fp_of(*id, &mut footprints);
                if let Some(fp) = footprints.get_mut(id) {
                    for p in fp.pads.iter_mut().filter(|p| p.number == *pad) {
                        p.net = net;
                    }
                }
            }
        }
    }
    let mut edit = Edit::default();
    // Nets first, so pads never point at a missing net.
    for net in &nets {
        if removed_nets.contains(&net.id) {
            continue;
        }
        match pcb.nets.get(net.id) {
            Some(old) if old == net => {}
            Some(_) => edit.changes.push(EditChange::Update {
                doc: pcb_doc,
                object: Object::Net(net.clone()),
            }),
            None => edit.changes.push(EditChange::Insert {
                doc: pcb_doc,
                object: Object::Net(net.clone()),
            }),
        }
    }
    for fp in added {
        edit.select.push(fp.id);
        edit.changes.push(EditChange::Insert {
            doc: pcb_doc,
            object: Object::Footprint(fp),
        });
    }
    let mut changed: Vec<FootprintInstance> = footprints.into_values().collect();
    changed.sort_by_key(|fp| fp.id);
    for fp in changed {
        if removed_footprints.contains(&fp.id) || pcb.footprints.get(fp.id) == Some(&fp) {
            continue;
        }
        edit.changes.push(EditChange::Update {
            doc: pcb_doc,
            object: Object::Footprint(fp),
        });
    }
    for id in removed_footprints {
        edit.changes.push(EditChange::Delete { doc: pcb_doc, id });
    }
    // Copper of removed nets loses its net.
    if !removed_nets.is_empty() {
        for track in &pcb.tracks {
            if track.net.is_some_and(|n| removed_nets.contains(&n)) {
                let mut track = track.clone();
                track.net = None;
                edit.changes.push(EditChange::Update {
                    doc: pcb_doc,
                    object: Object::Track(track),
                });
            }
        }
        for via in &pcb.vias {
            if via.net.is_some_and(|n| removed_nets.contains(&n)) {
                let mut via = via.clone();
                via.net = None;
                edit.changes.push(EditChange::Update {
                    doc: pcb_doc,
                    object: Object::Via(via),
                });
            }
        }
        for zone in &pcb.zones {
            if zone.net.is_some_and(|n| removed_nets.contains(&n)) {
                let mut zone = zone.clone();
                zone.net = None;
                edit.changes.push(EditChange::Update {
                    doc: pcb_doc,
                    object: Object::Zone(zone),
                });
            }
        }
        for id in &removed_nets {
            edit.changes.push(EditChange::Delete {
                doc: pcb_doc,
                id: *id,
            });
        }
    }
    edit
}

/// A change that goes back from the board to the schematic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BackChange {
    /// Rename a part.
    SetDesignator {
        /// Old designator.
        from: String,
        /// New designator.
        to: String,
    },
    /// Change the value of a part.
    SetValue {
        /// Designator.
        designator: String,
        /// Old value.
        from: String,
        /// New value.
        to: String,
    },
    /// Change the footprint of a part.
    SetFootprint {
        /// Designator.
        designator: String,
        /// Old footprint.
        from: String,
        /// New footprint.
        to: String,
    },
}

impl BackChange {
    /// An English description.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::SetDesignator { from, to } => format!("Rename {from} to {to}"),
            Self::SetValue {
                designator,
                from,
                to,
            } => format!("Change the value of {designator} from {from} to {to}"),
            Self::SetFootprint {
                designator,
                from,
                to,
            } => format!("Change the footprint of {designator} from {from} to {to}"),
        }
    }
}

/// The changes that bring the schematic in line with the board's
/// designators, values and footprints (footprints linked by id only).
#[must_use]
pub fn back_diff(project: &Project, connectivity: &Connectivity, pcb: &Pcb) -> Vec<BackChange> {
    let (parts, _) = parts(project, connectivity);
    let by_key: HashMap<Id, &FootprintInstance> = pcb
        .footprints
        .iter()
        .filter_map(|fp| fp.component.map(|c| (c, fp)))
        .collect();
    let mut changes = Vec::new();
    for part in &parts {
        let Some(fp) = by_key.get(&part.key) else {
            continue;
        };
        if fp.designator != part.designator && !fp.designator.is_empty() {
            changes.push(BackChange::SetDesignator {
                from: part.designator.clone(),
                to: fp.designator.clone(),
            });
        }
        if fp.value != part.value {
            changes.push(BackChange::SetValue {
                designator: part.designator.clone(),
                from: part.value.clone(),
                to: fp.value.clone(),
            });
        }
        if fp.footprint != part.footprint && !fp.footprint.is_empty() {
            changes.push(BackChange::SetFootprint {
                designator: part.designator.clone(),
                from: part.footprint.clone(),
                to: fp.footprint.clone(),
            });
        }
    }
    changes
}

/// Applies the selected back-annotation changes to the schematic.
#[must_use]
pub fn back_apply(
    project: &Project,
    connectivity: &Connectivity,
    changes: &[BackChange],
    selected: &[usize],
) -> Edit {
    let (parts, _) = parts(project, connectivity);
    let by_designator: HashMap<&str, &Part> =
        parts.iter().map(|p| (p.designator.as_str(), p)).collect();
    let mut symbols: BTreeMap<(Id, Id), PlacedSymbol> = BTreeMap::new();
    for (index, change) in changes.iter().enumerate() {
        if !selected.contains(&index) {
            continue;
        }
        let designator = match change {
            BackChange::SetDesignator { from, .. } => from,
            BackChange::SetValue { designator, .. }
            | BackChange::SetFootprint { designator, .. } => designator,
        };
        let Some(part) = by_designator.get(designator.as_str()) else {
            continue;
        };
        for (sheet, id) in &part.units {
            let symbol = symbols.entry((*sheet, *id)).or_insert_with(|| {
                match project.document(*sheet) {
                    Some(Document::Schematic(s)) => s.symbols.get(*id).cloned(),
                    _ => None,
                }
                .unwrap_or_else(|| PlacedSymbol {
                    id: *id,
                    ..placeholder()
                })
            });
            match change {
                BackChange::SetDesignator { to, .. } => {
                    if part.instance == "/" || !symbol.instances.contains_key(&part.instance) {
                        symbol.designator.clone_from(to);
                    }
                    if symbol.instances.contains_key(&part.instance) {
                        symbol.instances.insert(part.instance.clone(), to.clone());
                    }
                }
                BackChange::SetValue { to, .. } => symbol.value.clone_from(to),
                BackChange::SetFootprint { to, .. } => symbol.footprint = Some(to.clone()),
            }
        }
    }
    let mut edit = Edit::default();
    for ((sheet, _), symbol) in symbols {
        if symbol.symbol.is_empty() {
            continue;
        }
        edit.changes.push(EditChange::Update {
            doc: sheet,
            object: Object::Symbol(symbol),
        });
    }
    edit
}

fn placeholder() -> PlacedSymbol {
    PlacedSymbol {
        id: Id::NIL,
        symbol: String::new(),
        unit: 1,
        at: Point::ORIGIN,
        rotation: Angle::ZERO,
        mirror: false,
        designator: String::new(),
        value: String::new(),
        footprint: None,
        component: None,
        properties: BTreeMap::new(),
        fields: Vec::new(),
        flags: ew_model::schematic::ComponentFlags::default(),
        instances: BTreeMap::new(),
        alternate: false,
    }
}

/// Cross-probing: the footprints of placed symbols.
#[must_use]
pub fn footprints_of(
    project: &Project,
    connectivity: &Connectivity,
    pcb: &Pcb,
    symbols: &[Id],
) -> Vec<Id> {
    let (parts, _) = parts(project, connectivity);
    let (matched, _) = matching(&parts, pcb);
    let mut out: Vec<Id> = parts
        .iter()
        .enumerate()
        .filter(|(_, part)| part.units.iter().any(|(_, id)| symbols.contains(id)))
        .filter_map(|(index, _)| matched.get(&index).copied())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Cross-probing: the placed symbols (sheet, symbol) of footprints.
#[must_use]
pub fn symbols_of(
    project: &Project,
    connectivity: &Connectivity,
    pcb: &Pcb,
    footprints: &[Id],
) -> Vec<(Id, Id)> {
    let (parts, _) = parts(project, connectivity);
    let (matched, _) = matching(&parts, pcb);
    let mut out: Vec<(Id, Id)> = matched
        .iter()
        .filter(|(_, fp)| footprints.contains(fp))
        .flat_map(|(index, _)| parts[*index].units.iter().copied())
        .collect();
    out.sort();
    out.dedup();
    out
}
