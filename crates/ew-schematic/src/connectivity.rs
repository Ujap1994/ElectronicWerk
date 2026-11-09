//! Connectivity of a project's schematics: which pins, wires and labels form
//! which nets, across sheets and the sheet hierarchy.
//!
//! Rules (the same as KiCad's):
//!
//! - Points connect when they coincide, or when one lies on a wire (a wire
//!   end on another wire makes a T). Wires that only cross do not connect,
//!   unless a junction sits on the crossing.
//! - Local labels connect equal names within one sheet instance; global
//!   labels, power ports, power symbols and hidden power-input pins connect
//!   equal names everywhere.
//! - A hierarchical label in a child sheet connects to the sheet pin with the
//!   same name on the sheet symbol that places the child.
//! - A sheet that is placed twice has its own nets for each placement.
//!
//! Buses are drawn but do not carry connectivity here: bus members connect
//! through labels with the same names.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::library::{PinKind, SymbolDef};
use ew_model::project::Project;
use ew_model::schematic::{LabelKind, Schematic, instance_key};
use ew_model::units::Point;
use serde::Serialize;

use crate::geometry::{inside_segment, pin_point, sheet_pin_point};

/// The kind of a connected item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ItemKind {
    /// A wire.
    Wire,
    /// A junction.
    Junction,
    /// A local label.
    Label,
    /// A global label.
    GlobalLabel,
    /// A hierarchical label.
    HierarchicalLabel,
    /// A power port.
    PowerPort,
    /// A no-connect flag.
    NoConnect,
    /// A pin of a placed symbol (`index` is the pin index in the unit).
    Pin,
    /// A pin of a sheet symbol (`index` is the pin index).
    SheetPin,
}

/// An item of a sheet instance that takes part in a net.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemRef {
    /// Sheet instance.
    pub instance: usize,
    /// Kind.
    pub kind: ItemKind,
    /// Object id (the placed symbol or sheet symbol for pins).
    pub id: Id,
    /// Pin index, for pins and sheet pins.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<usize>,
    /// Connection point.
    pub at: Point,
}

/// What belongs to a net on one sheet.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetMembers {
    /// Wires, junctions, labels, power ports and flags.
    pub items: BTreeSet<Id>,
    /// Pins and sheet pins as scene keys.
    pub pins: BTreeSet<String>,
}

/// One placement of a schematic in the hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
    /// The schematic document.
    pub sheet: Id,
    /// Sheet symbols from the top sheet down to this placement.
    pub path: Vec<Id>,
    /// The path as text: `/` for the top sheet, `/Power/` below.
    pub name: String,
    /// The key of per-instance designators ([`instance_key`]).
    pub key: String,
    /// The instance that places this one.
    pub parent: Option<usize>,
}

/// A pin of a placed symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinInfo {
    /// Sheet instance.
    pub instance: usize,
    /// The placed symbol.
    pub symbol: Id,
    /// Designator.
    pub designator: String,
    /// Value.
    pub value: String,
    /// Pin number.
    pub number: String,
    /// Pin name.
    pub name: String,
    /// Electrical type.
    pub kind: PinKind,
    /// Connection point.
    pub at: Point,
    /// Hidden pin.
    pub hidden: bool,
    /// The pin belongs to a power symbol (it names a net instead of being a
    /// component pin).
    pub power_symbol: bool,
    /// Pin index in the symbol unit.
    pub index: usize,
    /// The local piece of the net.
    pub piece: usize,
    /// The net.
    pub net: usize,
}

/// The items of one sheet instance that touch each other directly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Piece {
    /// Sheet instance.
    pub instance: usize,
    /// Items (wires, labels, junctions, …; not pins).
    pub items: Vec<ItemRef>,
    /// Pins (indices into [`Connectivity::pins`]).
    pub pins: Vec<usize>,
    /// The net.
    pub net: usize,
}

/// A net.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Net {
    /// Name (from a label, a power port or symbol, or made up from a pin).
    pub name: String,
    /// `true` if the name comes from a label or power object.
    pub named: bool,
    /// Every name given to the net (more than one is suspicious).
    pub names: BTreeSet<String>,
    /// The pieces of the net.
    pub pieces: Vec<usize>,
    /// The pins of the net (indices into [`Connectivity::pins`]).
    pub pins: Vec<usize>,
}

/// A problem found while working out the connectivity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// `missing-symbol`, `missing-sheet` or `sheet-cycle`.
    pub code: String,
    /// The sheet instance.
    pub instance: usize,
    /// The object concerned.
    pub id: Id,
    /// The symbol or sheet name.
    pub name: String,
}

/// The connectivity of all schematics of a project.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connectivity {
    /// Sheet instances, top sheet first (depth first).
    pub instances: Vec<Instance>,
    /// Local pieces.
    pub pieces: Vec<Piece>,
    /// Nets.
    pub nets: Vec<Net>,
    /// Pins of placed symbols.
    pub pins: Vec<PinInfo>,
    /// Problems.
    pub problems: Vec<Problem>,
}

/// A union-find over indices.
#[derive(Debug, Default)]
struct Sets {
    parent: Vec<usize>,
}

impl Sets {
    fn add(&mut self) -> usize {
        self.parent.push(self.parent.len());
        self.parent.len() - 1
    }

    fn find(&mut self, mut i: usize) -> usize {
        while self.parent[i] != i {
            self.parent[i] = self.parent[self.parent[i]];
            i = self.parent[i];
        }
        i
    }

    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.parent[a.max(b)] = a.min(b);
        }
    }
}

/// Compares designators and pin numbers naturally (`R2` before `R10`).
#[must_use]
pub fn natural_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let split = |s: &str| -> Vec<(String, u64)> {
        let mut parts = Vec::new();
        let mut text = String::new();
        let mut digits = String::new();
        for c in s.chars() {
            if c.is_ascii_digit() {
                digits.push(c);
            } else {
                if !digits.is_empty() {
                    parts.push((
                        std::mem::take(&mut text),
                        digits.parse().unwrap_or(u64::MAX),
                    ));
                    digits.clear();
                }
                text.push(c);
            }
        }
        parts.push((text, digits.parse().unwrap_or(0)));
        parts
    };
    split(a).cmp(&split(b)).then_with(|| a.cmp(b))
}

/// Finds a symbol definition in the project libraries.
#[must_use]
pub fn symbol_def<'a>(project: &'a Project, name: &str) -> Option<&'a SymbolDef> {
    project.documents.values().find_map(|doc| match doc {
        Document::Library(library) => library.symbol(name),
        _ => None,
    })
}

/// The sheet instances of a project: the top sheet and everything it
/// places, depth first, then sheets that nothing places.
#[must_use]
pub fn instances(project: &Project) -> (Vec<Instance>, Vec<Problem>) {
    let mut out: Vec<Instance> = Vec::new();
    let mut problems = Vec::new();
    let sheet = |id: Id| match project.document(id) {
        Some(Document::Schematic(sheet)) => Some(sheet),
        _ => None,
    };
    let mut roots: Vec<Id> = Vec::new();
    if let Some(root) = project
        .manifest
        .root_schematic
        .filter(|id| sheet(*id).is_some())
    {
        roots.push(root);
    }
    let mut placed: BTreeSet<Id> = roots.iter().copied().collect();
    let mut stack: Vec<Instance> = roots
        .iter()
        .map(|id| Instance {
            sheet: *id,
            path: Vec::new(),
            name: "/".to_owned(),
            key: "/".to_owned(),
            parent: None,
        })
        .collect();
    let visit =
        |stack: &mut Vec<Instance>, out: &mut Vec<Instance>, problems: &mut Vec<Problem>| {
            while let Some(instance) = stack.pop() {
                let index = out.len();
                let doc = instance.sheet;
                let ancestors: Vec<Id> = {
                    let mut list = vec![doc];
                    let mut parent = instance.parent;
                    while let Some(p) = parent {
                        let inst: &Instance = &out[p];
                        list.push(inst.sheet);
                        parent = inst.parent;
                    }
                    list
                };
                out.push(instance);
                let Some(schematic) = sheet(doc) else {
                    continue;
                };
                let mut children: Vec<_> = schematic.sheets.iter().collect();
                // Top to bottom, left to right (y up).
                children.sort_by_key(|s| (-s.at.y, s.at.x, s.id));
                for child in children.into_iter().rev() {
                    if sheet(child.schematic).is_none() {
                        problems.push(Problem {
                            code: "missing-sheet".to_owned(),
                            instance: index,
                            id: child.id,
                            name: child.name.clone(),
                        });
                        continue;
                    }
                    if ancestors.contains(&child.schematic) {
                        problems.push(Problem {
                            code: "sheet-cycle".to_owned(),
                            instance: index,
                            id: child.id,
                            name: child.name.clone(),
                        });
                        continue;
                    }
                    let parent = &out[index];
                    let mut path = parent.path.clone();
                    path.push(child.id);
                    stack.push(Instance {
                        sheet: child.schematic,
                        key: instance_key(&path),
                        path,
                        name: format!("{}{}/", parent.name, child.name),
                        parent: Some(index),
                    });
                }
            }
        };
    visit(&mut stack, &mut out, &mut problems);
    for entry in &out {
        placed.insert(entry.sheet);
    }
    // Sheets that nothing places are treated as extra top sheets.
    for entry in project.documents_of(DocumentKind::Schematic) {
        if !placed.contains(&entry.id) {
            let name = sheet(entry.id).map_or_else(String::new, |s| s.name.clone());
            stack.push(Instance {
                sheet: entry.id,
                path: Vec::new(),
                name: format!("/{name}/"),
                key: "/".to_owned(),
                parent: None,
            });
            visit(&mut stack, &mut out, &mut problems);
            for item in &out {
                placed.insert(item.sheet);
            }
        }
    }
    (out, problems)
}

/// What a point of a sheet belongs to.
#[derive(Debug, Clone)]
enum Member {
    Item(ItemRef),
    Pin(usize),
}

/// The names a piece gives its net.
#[derive(Default)]
struct PieceNames {
    local: BTreeSet<String>,
    global: BTreeSet<String>,
    hierarchical: BTreeSet<String>,
}

/// Works out the connectivity of every schematic of a project.
#[must_use]
pub fn analyze(project: &Project) -> Connectivity {
    let (instances, mut problems) = instances(project);
    let mut out = Connectivity {
        instances,
        ..Connectivity::default()
    };
    let mut names: Vec<PieceNames> = Vec::new();
    // (parent instance, sheet symbol, pin name) → piece.
    let mut sheet_pins: HashMap<(usize, Id, String), usize> = HashMap::new();
    for index in 0..out.instances.len() {
        let Some(Document::Schematic(sheet)) = project.document(out.instances[index].sheet) else {
            continue;
        };
        sheet_pieces(
            project,
            sheet,
            index,
            &mut out,
            &mut names,
            &mut sheet_pins,
            &mut problems,
        );
    }
    out.problems = problems;
    merge_nets(&mut out, &names, &sheet_pins);
    out
}

#[allow(
    clippy::too_many_lines,
    reason = "one pass over every kind of sheet item"
)]
fn sheet_pieces(
    project: &Project,
    sheet: &Schematic,
    instance: usize,
    out: &mut Connectivity,
    names: &mut Vec<PieceNames>,
    sheet_pins: &mut HashMap<(usize, Id, String), usize>,
    problems: &mut Vec<Problem>,
) {
    let mut members: Vec<(Point, Member)> = Vec::new();
    let mut sets = Sets::default();
    let item = |kind: ItemKind, id: Id, index: Option<usize>, at: Point| {
        Member::Item(ItemRef {
            instance,
            kind,
            id,
            index,
            at,
        })
    };
    // Wires: both ends are one piece.
    let mut wires: Vec<(Point, Point, usize)> = Vec::new();
    for wire in &sheet.wires {
        let a = sets.add();
        members.push((wire.start, item(ItemKind::Wire, wire.id, None, wire.start)));
        let b = sets.add();
        members.push((wire.end, item(ItemKind::Wire, wire.id, None, wire.end)));
        sets.join(a, b);
        wires.push((wire.start, wire.end, a));
    }
    let push = |members: &mut Vec<(Point, Member)>, sets: &mut Sets, at: Point, member: Member| {
        sets.add();
        members.push((at, member));
    };
    for junction in &sheet.junctions {
        push(
            &mut members,
            &mut sets,
            junction.at,
            item(ItemKind::Junction, junction.id, None, junction.at),
        );
    }
    for label in &sheet.labels {
        let kind = match label.kind {
            LabelKind::Local => ItemKind::Label,
            LabelKind::Global => ItemKind::GlobalLabel,
            LabelKind::Hierarchical => ItemKind::HierarchicalLabel,
        };
        push(
            &mut members,
            &mut sets,
            label.at,
            item(kind, label.id, None, label.at),
        );
    }
    for port in &sheet.power_ports {
        push(
            &mut members,
            &mut sets,
            port.at,
            item(ItemKind::PowerPort, port.id, None, port.at),
        );
    }
    for flag in &sheet.no_connects {
        push(
            &mut members,
            &mut sets,
            flag.at,
            item(ItemKind::NoConnect, flag.id, None, flag.at),
        );
    }
    for sheet_symbol in &sheet.sheets {
        for (index, pin) in sheet_symbol.pins.iter().enumerate() {
            let at = sheet_pin_point(sheet_symbol, pin);
            push(
                &mut members,
                &mut sets,
                at,
                item(ItemKind::SheetPin, sheet_symbol.id, Some(index), at),
            );
        }
    }
    for symbol in &sheet.symbols {
        let Some(def) = symbol_def(project, &symbol.symbol) else {
            problems.push(Problem {
                code: "missing-symbol".to_owned(),
                instance,
                id: symbol.id,
                name: symbol.symbol.clone(),
            });
            continue;
        };
        let Some(unit) = def
            .units
            .get(usize::from(symbol.unit.max(1) - 1))
            .or_else(|| def.units.first())
        else {
            continue;
        };
        for (index, pin) in unit.pins.iter().enumerate() {
            let at = pin_point(symbol, pin);
            let pin_index = out.pins.len();
            out.pins.push(PinInfo {
                instance,
                symbol: symbol.id,
                designator: symbol
                    .designator_in(&out.instances[instance].key)
                    .to_owned(),
                value: symbol.value.clone(),
                number: pin.number.clone(),
                name: pin.name.clone(),
                kind: pin.kind,
                at,
                hidden: pin.hidden,
                power_symbol: def.power,
                index,
                piece: 0,
                net: 0,
            });
            push(&mut members, &mut sets, at, Member::Pin(pin_index));
        }
    }

    // Coinciding points.
    let mut first_at: HashMap<Point, usize> = HashMap::new();
    for (index, (at, _)) in members.iter().enumerate() {
        match first_at.get(at) {
            Some(&first) => sets.join(first, index),
            None => {
                first_at.insert(*at, index);
            }
        }
    }
    // Points inside wires (horizontal and vertical wires are indexed).
    let mut horizontal: HashMap<i64, Vec<usize>> = HashMap::new();
    let mut vertical: HashMap<i64, Vec<usize>> = HashMap::new();
    let mut other: Vec<usize> = Vec::new();
    for (index, (a, b, _)) in wires.iter().enumerate() {
        if a.y == b.y {
            horizontal.entry(a.y).or_default().push(index);
        } else if a.x == b.x {
            vertical.entry(a.x).or_default().push(index);
        } else {
            other.push(index);
        }
    }
    let mut points: Vec<(Point, usize)> = first_at.into_iter().collect();
    points.sort_unstable();
    for (at, member) in points {
        let candidates = horizontal
            .get(&at.y)
            .into_iter()
            .flatten()
            .chain(vertical.get(&at.x).into_iter().flatten())
            .chain(other.iter());
        for &wire in candidates {
            let (a, b, start) = wires[wire];
            if inside_segment(at, a, b) {
                sets.join(member, start);
            }
        }
    }

    // Pieces.
    let mut piece_of: BTreeMap<usize, usize> = BTreeMap::new();
    for (index, (_, member)) in members.iter().enumerate() {
        let root = sets.find(index);
        let piece = *piece_of.entry(root).or_insert_with(|| {
            out.pieces.push(Piece {
                instance,
                ..Piece::default()
            });
            names.push(PieceNames::default());
            out.pieces.len() - 1
        });
        match member {
            Member::Pin(pin) => {
                out.pins[*pin].piece = piece;
                out.pieces[piece].pins.push(*pin);
                // Power-input pins of power symbols name a global net after
                // the symbol's value, hidden power-input pins after the pin
                // name (KiCad's rule; a power flag has a power-output pin
                // and names nothing).
                let info = &out.pins[*pin];
                if info.kind == PinKind::PowerIn {
                    if info.power_symbol {
                        names[piece].global.insert(info.value.clone());
                    } else if info.hidden && !info.name.is_empty() {
                        names[piece].global.insert(info.name.clone());
                    }
                }
            }
            Member::Item(item) => {
                let text = || -> Option<String> {
                    match item.kind {
                        ItemKind::Label | ItemKind::GlobalLabel | ItemKind::HierarchicalLabel => {
                            sheet.labels.get(item.id).map(|l| l.text.clone())
                        }
                        ItemKind::PowerPort => {
                            sheet.power_ports.get(item.id).map(|p| p.net.clone())
                        }
                        _ => None,
                    }
                };
                match item.kind {
                    ItemKind::Label => names[piece].local.extend(text()),
                    ItemKind::GlobalLabel | ItemKind::PowerPort => {
                        names[piece].global.extend(text());
                    }
                    ItemKind::HierarchicalLabel => names[piece].hierarchical.extend(text()),
                    ItemKind::SheetPin => {
                        if let (Some(symbol), Some(pin)) = (sheet.sheets.get(item.id), item.index) {
                            if let Some(pin) = symbol.pins.get(pin) {
                                sheet_pins.insert((instance, item.id, pin.name.clone()), piece);
                            }
                        }
                    }
                    _ => {}
                }
                // A wire is listed once, with its start.
                let duplicate = item.kind == ItemKind::Wire
                    && out.pieces[piece]
                        .items
                        .iter()
                        .any(|i| i.kind == ItemKind::Wire && i.id == item.id);
                if !duplicate {
                    out.pieces[piece].items.push(item.clone());
                }
            }
        }
    }
}

fn merge_nets(
    out: &mut Connectivity,
    names: &[PieceNames],
    sheet_pins: &HashMap<(usize, Id, String), usize>,
) {
    let mut sets = Sets::default();
    for _ in 0..out.pieces.len() {
        sets.add();
    }
    let mut local: HashMap<(usize, &str), usize> = HashMap::new();
    let mut global: HashMap<&str, usize> = HashMap::new();
    for (piece, piece_names) in names.iter().enumerate() {
        let instance = out.pieces[piece].instance;
        for name in &piece_names.local {
            match local.get(&(instance, name.as_str())) {
                Some(&other) => sets.join(other, piece),
                None => {
                    local.insert((instance, name.as_str()), piece);
                }
            }
        }
        for name in &piece_names.global {
            match global.get(name.as_str()) {
                Some(&other) => sets.join(other, piece),
                None => {
                    global.insert(name.as_str(), piece);
                }
            }
        }
        // Hierarchical labels meet the sheet pin of the same name above.
        let inst = &out.instances[instance];
        if let (Some(parent), Some(symbol)) = (inst.parent, inst.path.last()) {
            for name in &piece_names.hierarchical {
                if let Some(&other) = sheet_pins.get(&(parent, *symbol, name.clone())) {
                    sets.join(other, piece);
                }
            }
        }
    }
    // Nets in the order of their first piece.
    let mut net_of: BTreeMap<usize, usize> = BTreeMap::new();
    for piece in 0..out.pieces.len() {
        let root = sets.find(piece);
        let net = *net_of.entry(root).or_insert_with(|| {
            out.nets.push(Net::default());
            out.nets.len() - 1
        });
        out.pieces[piece].net = net;
        out.nets[net].pieces.push(piece);
        for &pin in &out.pieces[piece].pins {
            out.pins[pin].net = net;
            out.nets[net].pins.push(pin);
        }
    }
    for net in 0..out.nets.len() {
        name_net(out, names, net);
    }
    // Made-up names must not collide (a designator used twice).
    let mut seen: HashMap<String, usize> = HashMap::new();
    for net in &mut out.nets {
        let count = seen.entry(net.name.clone()).or_insert(0);
        *count += 1;
        if *count > 1 && !net.named {
            net.name = format!("{}-{}", net.name, count);
        }
    }
}

/// Names a net: a global name first, then a local label of the highest
/// sheet (`/Path/NAME`), then the first pin (`Net-(D1-A)`, or
/// `Net-(R1-Pad2)` for pins without a name).
fn name_net(out: &mut Connectivity, piece_names: &[PieceNames], net: usize) {
    let mut global: BTreeSet<String> = BTreeSet::new();
    let mut local: Vec<(usize, String)> = Vec::new();
    let mut all = BTreeSet::new();
    for &piece in &out.nets[net].pieces {
        let instance = out.pieces[piece].instance;
        let depth = out.instances[instance].path.len();
        global.extend(piece_names[piece].global.iter().cloned());
        for name in &piece_names[piece].local {
            local.push((depth, format!("{}{name}", out.instances[instance].name)));
        }
        for name in &piece_names[piece].hierarchical {
            local.push((depth + 1, format!("{}{name}", out.instances[instance].name)));
        }
    }
    all.extend(global.iter().cloned());
    all.extend(local.iter().map(|(_, n)| n.clone()));
    local.sort();
    let (name, named) = if let Some(first) = global.first() {
        (first.clone(), true)
    } else if let Some((_, first)) = local.first() {
        (first.clone(), true)
    } else {
        let pins = &out.nets[net].pins;
        let first = pins
            .iter()
            .map(|&p| &out.pins[p])
            .filter(|p| !p.power_symbol)
            .min_by(|a, b| {
                natural_cmp(&a.designator, &b.designator)
                    .then_with(|| natural_cmp(&a.number, &b.number))
            });
        (
            first.map_or_else(
                || format!("Net-{}", net + 1),
                |pin| {
                    // KiCad: the pin name if the symbol has it once, else
                    // the number.
                    let unique = !pin.name.is_empty()
                        && pin.name != "~"
                        && out
                            .pins
                            .iter()
                            .filter(|p| p.symbol == pin.symbol && p.instance == pin.instance)
                            .filter(|p| p.name == pin.name)
                            .count()
                            == 1;
                    if unique {
                        format!("Net-({}-{})", pin.designator, pin.name)
                    } else {
                        format!("Net-({}-Pad{})", pin.designator, pin.number)
                    }
                },
            ),
            false,
        )
    };
    let entry = &mut out.nets[net];
    entry.name = name;
    entry.named = named;
    entry.names = all;
}

impl Connectivity {
    /// The net at a point of a sheet instance (a wire, pin or label there).
    #[must_use]
    pub fn net_at(&self, instance: usize, at: Point) -> Option<usize> {
        self.pieces
            .iter()
            .filter(|piece| piece.instance == instance)
            .find(|piece| {
                piece.items.iter().any(|item| item.at == at)
                    || piece.pins.iter().any(|&p| self.pins[p].at == at)
            })
            .map(|piece| piece.net)
    }

    /// The net of an object of a sheet instance (a wire, label, …).
    #[must_use]
    pub fn net_of(&self, instance: usize, id: Id) -> Option<usize> {
        self.pieces
            .iter()
            .filter(|piece| piece.instance == instance)
            .find(|piece| piece.items.iter().any(|item| item.id == id))
            .map(|piece| piece.net)
    }

    /// The net of a pin (`index` in the unit's pin list) of a placed symbol.
    #[must_use]
    pub fn net_of_pin(&self, instance: usize, symbol: Id, index: usize) -> Option<usize> {
        self.pins
            .iter()
            .find(|p| p.instance == instance && p.symbol == symbol && p.index == index)
            .map(|p| p.net)
    }

    /// The first instance of a sheet document.
    #[must_use]
    pub fn instance_of(&self, sheet: Id) -> Option<usize> {
        self.instances.iter().position(|i| i.sheet == sheet)
    }

    /// What belongs to a net, per sheet document: objects (wires,
    /// junctions, labels, flags, sheet symbols with a pin on it) and pins
    /// as scene keys (`<symbol id>/pin/<index>`).
    #[must_use]
    pub fn members(&self, net: usize) -> BTreeMap<Id, NetMembers> {
        let mut out: BTreeMap<Id, NetMembers> = BTreeMap::new();
        let Some(net) = self.nets.get(net) else {
            return out;
        };
        for &piece in &net.pieces {
            let piece = &self.pieces[piece];
            let sheet = self.instances[piece.instance].sheet;
            let entry = out.entry(sheet).or_default();
            for item in &piece.items {
                match (item.kind, item.index) {
                    (ItemKind::SheetPin, Some(index)) => {
                        entry.pins.insert(format!("{}/sheetPin/{index}", item.id));
                    }
                    _ => {
                        entry.items.insert(item.id);
                    }
                }
            }
            for &pin in &piece.pins {
                let pin = &self.pins[pin];
                entry
                    .pins
                    .insert(format!("{}/pin/{}", pin.symbol, pin.index));
            }
        }
        out
    }

    /// Component pins per net name, as a netlist: power symbols are left
    /// out, and each sheet instance's designators are used.
    #[must_use]
    pub fn netlist(&self) -> BTreeMap<String, BTreeSet<(String, String)>> {
        let mut out: BTreeMap<String, BTreeSet<(String, String)>> = BTreeMap::new();
        for net in &self.nets {
            let nodes: BTreeSet<(String, String)> = net
                .pins
                .iter()
                .map(|&p| &self.pins[p])
                .filter(|p| !p.power_symbol)
                .map(|p| (p.designator.clone(), p.number.clone()))
                .collect();
            if !nodes.is_empty() {
                out.entry(net.name.clone()).or_default().extend(nodes);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut list = vec!["R10", "R2", "C1", "R1", "U1"];
        list.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(list, ["C1", "R1", "R2", "R10", "U1"]);
        assert_eq!(natural_cmp("10", "9"), std::cmp::Ordering::Greater);
    }
}
