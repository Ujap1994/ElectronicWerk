//! Editing operations on schematics. Each operation works on a copy of the
//! sheet and returns the changes (insert, update, delete) for the engine to
//! commit as one undo step.
//!
//! Every operation that adds, moves or removes connection points also keeps
//! the junctions right: a junction is added where three or more connections
//! meet (a wire passing through counts twice) and removed where it is no
//! longer needed. Crossing wires never get one automatically.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::object::Object;
use ew_model::project::Project;
use ew_model::schematic::{
    Junction, Label, LabelKind, NoConnect, PlacedSymbol, PowerPort, PowerStyle, Schematic,
    SheetSide, Text, Wire,
};
use ew_model::shape::Geometry;
use ew_model::text::TextStyle;
use ew_model::units::{Angle, Point};
use serde::{Deserialize, Serialize};

use crate::connectivity::symbol_def;
use crate::geometry::{inside_segment, pin_point, rotate, sheet_pin_point};

/// One change of an edit.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "op")]
pub enum Change {
    /// A new object.
    Insert {
        /// Document.
        doc: Id,
        /// Object.
        object: Object,
    },
    /// A changed object.
    Update {
        /// Document.
        doc: Id,
        /// Object.
        object: Object,
    },
    /// A removed object.
    Delete {
        /// Document.
        doc: Id,
        /// Object id.
        id: Id,
    },
}

/// The result of an edit.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Edit {
    /// Changes in order.
    pub changes: Vec<Change>,
    /// Objects the user created or that should be selected afterwards.
    pub select: Vec<Id>,
}

/// Why an edit is not possible.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    /// The document is not a schematic.
    #[error("{0} is not a schematic")]
    NotASchematic(Id),
    /// The symbol is not in a project library.
    #[error("symbol `{0}` is not in a project library")]
    UnknownSymbol(String),
    /// An object does not exist.
    #[error("no object {0} on this sheet")]
    UnknownObject(Id),
    /// The input makes no sense.
    #[error("{0}")]
    Invalid(String),
}

/// A working copy of one sheet that records what changed.
struct Work<'a> {
    project: &'a Project,
    doc: Id,
    sheet: Document,
    original: &'a Schematic,
    original_doc: &'a Document,
    /// Ids in order of their first change.
    touched_ids: Vec<Id>,
    seen: BTreeSet<Id>,
    /// Points whose junctions must be checked.
    points: BTreeSet<Point>,
    select: Vec<Id>,
}

impl<'a> Work<'a> {
    fn new(project: &'a Project, doc: Id) -> Result<Self, EditError> {
        match project.document(doc) {
            Some(document @ Document::Schematic(original)) => Ok(Self {
                project,
                doc,
                sheet: document.clone(),
                original,
                original_doc: document,
                touched_ids: Vec::new(),
                seen: BTreeSet::new(),
                points: BTreeSet::new(),
                select: Vec::new(),
            }),
            _ => Err(EditError::NotASchematic(doc)),
        }
    }

    fn sheet(&self) -> &Schematic {
        match &self.sheet {
            Document::Schematic(sheet) => sheet,
            _ => self.original,
        }
    }

    fn touch(&mut self, id: Id) {
        if self.seen.insert(id) {
            self.touched_ids.push(id);
        }
    }

    fn get(&self, id: Id) -> Option<Object> {
        self.sheet.object(id)
    }

    /// Remembers the connection points of an object (before or after a
    /// change).
    fn note_points(&mut self, object: &Object) {
        let points = connection_points(self.project, object);
        self.points.extend(points);
    }

    fn insert(&mut self, object: Object) {
        self.note_points(&object);
        let id = object.id();
        self.touch(id);
        // Ids are new, so this cannot clash.
        let _ = self.sheet.insert_object(object);
    }

    fn update(&mut self, object: Object) {
        if let Some(old) = self.get(object.id()) {
            self.note_points(&old);
        }
        self.note_points(&object);
        let id = object.id();
        self.touch(id);
        let _ = self.sheet.update_object(object);
    }

    fn delete(&mut self, id: Id) {
        if let Ok(old) = self.sheet.remove_object(id) {
            self.note_points(&old);
            self.touch(id);
        }
    }

    /// Adds and removes junctions at the touched points and returns the
    /// changes.
    fn finish(mut self, fix_junctions: bool) -> Edit {
        if fix_junctions {
            self.fix_junctions();
        }
        let mut changes = Vec::new();
        for id in &self.touched_ids {
            let before = self.original_doc.object(*id);
            match (before, self.sheet.object(*id)) {
                (None, Some(object)) => changes.push(Change::Insert {
                    doc: self.doc,
                    object,
                }),
                (Some(old), Some(object)) => {
                    if old != object {
                        changes.push(Change::Update {
                            doc: self.doc,
                            object,
                        });
                    }
                }
                (Some(_), None) => changes.push(Change::Delete {
                    doc: self.doc,
                    id: *id,
                }),
                (None, None) => {}
            }
        }
        let select = self
            .select
            .iter()
            .copied()
            .filter(|id| self.sheet.object(*id).is_some())
            .collect();
        Edit { changes, select }
    }

    fn fix_junctions(&mut self) {
        let points: Vec<Point> = self.points.iter().copied().collect();
        let pins = pin_points(self.project, self.sheet());
        for at in points {
            let needed = junction_needed(self.sheet(), &pins, at);
            let existing: Vec<Id> = self
                .sheet()
                .junctions
                .iter()
                .filter(|j| j.at == at)
                .map(|j| j.id)
                .collect();
            if needed && existing.is_empty() {
                let id = Id::random();
                self.touch(id);
                let _ = self
                    .sheet
                    .insert_object(Object::Junction(Junction { id, at }));
            } else if !needed {
                for id in existing {
                    self.touch(id);
                    let _ = self.sheet.remove_object(id);
                }
            }
        }
    }
}

/// Every pin and sheet pin position of a sheet, with the count per point.
fn pin_points(project: &Project, sheet: &Schematic) -> HashMap<Point, usize> {
    let mut out: HashMap<Point, usize> = HashMap::new();
    for symbol in &sheet.symbols {
        for at in symbol_pins(project, symbol) {
            *out.entry(at).or_default() += 1;
        }
    }
    for symbol in &sheet.sheets {
        for pin in &symbol.pins {
            *out.entry(sheet_pin_point(symbol, pin)).or_default() += 1;
        }
    }
    out
}

fn symbol_pins(project: &Project, symbol: &PlacedSymbol) -> Vec<Point> {
    let Some(def) = symbol_def(project, &symbol.symbol) else {
        return Vec::new();
    };
    let Some(unit) = def.body(symbol.unit, symbol.alternate) else {
        return Vec::new();
    };
    unit.pins.iter().map(|pin| pin_point(symbol, pin)).collect()
}

/// `true` if a junction belongs at `at`: three or more connections meet
/// there (a wire passing through counts twice) and at least one of them ends
/// there.
#[must_use]
pub fn junction_needed<S: std::hash::BuildHasher>(
    sheet: &Schematic,
    pins: &HashMap<Point, usize, S>,
    at: Point,
) -> bool {
    let ends = sheet
        .wires
        .iter()
        .filter(|w| w.start != w.end && (w.start == at || w.end == at))
        .count();
    let through = sheet
        .wires
        .iter()
        .filter(|w| inside_segment(at, w.start, w.end))
        .count();
    let pins = pins.get(&at).copied().unwrap_or(0);
    ends + through > 0 && ends + pins > 0 && ends + 2 * through + pins >= 3
}

/// The points where an object can connect.
fn connection_points(project: &Project, object: &Object) -> Vec<Point> {
    match object {
        Object::Wire(wire) => vec![wire.start, wire.end],
        Object::Symbol(symbol) => symbol_pins(project, symbol),
        Object::Junction(j) => vec![j.at],
        Object::Label(l) => vec![l.at],
        Object::PowerPort(p) => vec![p.at],
        Object::NoConnect(n) => vec![n.at],
        Object::Sheet(sheet) => sheet
            .pins
            .iter()
            .map(|pin| sheet_pin_point(sheet, pin))
            .collect(),
        _ => Vec::new(),
    }
}

// --- Placing --------------------------------------------------------------

/// What to place.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlaceSymbol {
    /// Symbol name in the project library.
    pub symbol: String,
    /// Unit (1-based).
    pub unit: u16,
    /// Anchor position.
    pub at: Point,
    /// Rotation.
    pub rotation: Angle,
    /// Mirrored.
    pub mirror: bool,
    /// Give it the next free designator (otherwise `R?`).
    pub annotate: bool,
    /// Place a library component instead: its symbol with its value,
    /// footprint, parameters, manufacturer and part number.
    pub component: Option<String>,
}

/// A component of the project libraries (first library wins).
#[must_use]
pub fn component_def<'a>(
    project: &'a Project,
    name: &str,
) -> Option<&'a ew_model::library::ComponentDef> {
    project
        .manifest
        .documents
        .iter()
        .find_map(|entry| match project.document(entry.id) {
            Some(Document::Library(library)) => library.component(name),
            _ => None,
        })
}

/// Places a symbol (or a component).
///
/// # Errors
///
/// Fails if the sheet, the symbol or the component does not exist.
pub fn place_symbol(project: &Project, doc: Id, params: &PlaceSymbol) -> Result<Edit, EditError> {
    let component = match &params.component {
        Some(name) => Some(component_def(project, name).ok_or_else(|| {
            EditError::Invalid(format!("component `{name}` is not in a project library"))
        })?),
        None => None,
    };
    let symbol_name = component.map_or(params.symbol.as_str(), |c| c.symbol.as_str());
    let def = symbol_def(project, symbol_name)
        .ok_or_else(|| EditError::UnknownSymbol(symbol_name.to_owned()))?;
    let mut work = Work::new(project, doc)?;
    let prefix = designator_prefix(&def.prefix, def.power);
    let designator = if params.annotate {
        let used = used_designators(project);
        format!("{prefix}{}", next_number(&used, &prefix, 1))
    } else {
        format!("{prefix}?")
    };
    let mut value = if def.value.is_empty() {
        def.name.clone()
    } else {
        def.value.clone()
    };
    let mut footprint = def.footprint.clone();
    let mut properties = def.properties.clone();
    if let Some(c) = component {
        if !c.value.is_empty() {
            value.clone_from(&c.value);
        }
        if c.footprint.is_some() {
            footprint.clone_from(&c.footprint);
        }
        properties.extend(c.properties.clone());
        for (key, field) in [
            ("Manufacturer", &c.manufacturer),
            ("MPN", &c.mpn),
            ("Datasheet", &c.datasheet),
        ] {
            if !field.is_empty() {
                properties.insert(key.to_owned(), field.clone());
            }
        }
        for (index, supplier) in c.suppliers.iter().enumerate() {
            let n = index + 1;
            properties.insert(format!("Supplier {n}"), supplier.supplier.clone());
            properties.insert(format!("Supplier {n} PN"), supplier.part_number.clone());
        }
    }
    let id = Id::random();
    let unit = params
        .unit
        .clamp(1, u16::try_from(def.units.len().max(1)).unwrap_or(1));
    work.insert(Object::Symbol(PlacedSymbol {
        id,
        symbol: def.name.clone(),
        unit,
        at: params.at,
        rotation: params.rotation,
        mirror: params.mirror,
        designator,
        value,
        footprint,
        component: (!def.power).then(Id::random),
        properties,
        fields: Vec::new(),
        flags: ew_model::schematic::ComponentFlags::default(),
        instances: BTreeMap::new(),
        alternate: false,
    }));
    work.select.push(id);
    Ok(work.finish(true))
}

pub(crate) fn designator_prefix(prefix: &str, power: bool) -> String {
    if power {
        // Power symbols keep a `#` prefix of their own (`#FLG`).
        if prefix.starts_with('#') {
            prefix.to_owned()
        } else {
            "#PWR".to_owned()
        }
    } else if prefix.is_empty() {
        "U".to_owned()
    } else {
        prefix.to_owned()
    }
}

/// `true` for a designator that still needs a number (`R?` or empty).
#[must_use]
pub fn is_unannotated(designator: &str) -> bool {
    designator.is_empty() || designator.ends_with('?')
}

/// Splits `R12` into (`R`, 12); `None` without a trailing number.
#[must_use]
pub fn split_designator(designator: &str) -> Option<(&str, u32)> {
    let digits = designator.len()
        - designator
            .trim_end_matches(|c: char| c.is_ascii_digit())
            .len();
    if digits == 0 {
        return None;
    }
    let (prefix, number) = designator.split_at(designator.len() - digits);
    number.parse().ok().map(|n| (prefix, n))
}

/// Every designator in the project (all instances).
fn used_designators(project: &Project) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for document in project.documents.values() {
        if let Document::Schematic(sheet) = document {
            for symbol in &sheet.symbols {
                out.insert(symbol.designator.clone());
                out.extend(symbol.instances.values().cloned());
            }
        }
    }
    out
}

fn next_number(used: &BTreeSet<String>, prefix: &str, start: u32) -> u32 {
    let mut n = start.max(1);
    while used.contains(&format!("{prefix}{n}")) {
        n += 1;
    }
    n
}

/// Something to add at a point.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AddItem {
    /// A net label.
    Label {
        /// Label kind.
        #[serde(default)]
        label: LabelKind,
        /// Net name.
        text: String,
    },
    /// A power port.
    PowerPort {
        /// Net name.
        net: String,
        /// Symbol style.
        #[serde(default)]
        style: PowerStyle,
    },
    /// A no-connect flag.
    NoConnect,
    /// A junction.
    Junction,
    /// A free text.
    Text {
        /// Text.
        text: String,
    },
}

/// Adds a label, power port, flag, junction or text.
///
/// # Errors
///
/// Fails if the sheet does not exist or a name is empty.
pub fn add_item(
    project: &Project,
    doc: Id,
    item: &AddItem,
    at: Point,
    rotation: Angle,
) -> Result<Edit, EditError> {
    let mut work = Work::new(project, doc)?;
    let id = Id::random();
    let object = match item {
        AddItem::Label { label, text } => {
            let text = text.trim();
            if text.is_empty() {
                return Err(EditError::Invalid("a label needs a name".to_owned()));
            }
            Object::Label(Label {
                id,
                kind: *label,
                text: text.to_owned(),
                at,
                rotation,
                shape: ew_model::schematic::PortShape::default(),
                style: TextStyle::schematic(),
            })
        }
        AddItem::PowerPort { net, style } => {
            let net = net.trim();
            if net.is_empty() {
                return Err(EditError::Invalid(
                    "a power port needs a net name".to_owned(),
                ));
            }
            Object::PowerPort(PowerPort {
                id,
                net: net.to_owned(),
                at,
                rotation,
                style: *style,
            })
        }
        AddItem::NoConnect => Object::NoConnect(NoConnect { id, at }),
        AddItem::Junction => Object::Junction(Junction { id, at }),
        AddItem::Text { text } => Object::Text(Text {
            id,
            text: text.clone(),
            at,
            rotation,
            style: TextStyle::schematic(),
        }),
    };
    let explicit_junction = matches!(item, AddItem::Junction);
    work.insert(object);
    work.select.push(id);
    Ok(work.finish(!explicit_junction))
}

/// Draws a wire (or, with `bus`, a bus) through the points (consecutive
/// duplicates and zero-length segments are dropped, straight runs are
/// merged).
///
/// # Errors
///
/// Fails if the sheet does not exist or fewer than two distinct points are
/// given.
pub fn draw_wire(
    project: &Project,
    doc: Id,
    points: &[Point],
    bus: bool,
) -> Result<Edit, EditError> {
    let mut path: Vec<Point> = Vec::new();
    for &p in points {
        if path.last() == Some(&p) {
            continue;
        }
        // Merge straight runs: drop the middle of three collinear points.
        if path.len() >= 2 {
            let (a, b) = (path[path.len() - 2], path[path.len() - 1]);
            let cross = i128::from(b.x - a.x) * i128::from(p.y - b.y)
                - i128::from(b.y - a.y) * i128::from(p.x - b.x);
            let forward = i128::from(b.x - a.x) * i128::from(p.x - b.x)
                + i128::from(b.y - a.y) * i128::from(p.y - b.y);
            if cross == 0 && forward > 0 {
                path.pop();
            }
        }
        path.push(p);
    }
    if path.len() < 2 {
        return Err(EditError::Invalid(
            "a wire needs two different points".to_owned(),
        ));
    }
    let mut work = Work::new(project, doc)?;
    for pair in path.windows(2) {
        let id = Id::random();
        let (start, end) = (pair[0], pair[1]);
        work.insert(if bus {
            Object::Bus(ew_model::schematic::Bus { id, start, end })
        } else {
            Object::Wire(Wire { id, start, end })
        });
        work.select.push(id);
    }
    // Inner corners are connection points too.
    work.points.extend(path.iter().copied());
    Ok(work.finish(true))
}

// --- Moving, turning and mirroring -----------------------------------------

/// How objects are transformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Turn {
    /// Moved only.
    None,
    /// Turned by 90° counterclockwise.
    Left,
    /// Turned by 90° clockwise.
    Right,
    /// Mirrored left to right (about a vertical axis).
    Horizontal,
    /// Mirrored top to bottom (about a horizontal axis).
    Vertical,
}

impl Turn {
    /// Applies the turn to a vector (no translation).
    fn vector(self, v: Point) -> Point {
        match self {
            Self::None => v,
            Self::Left => rotate(v, Angle::DEG_90),
            Self::Right => rotate(v, Angle::DEG_270),
            Self::Horizontal => Point::new(-v.x, v.y),
            Self::Vertical => Point::new(v.x, -v.y),
        }
    }

    /// The new rotation of a text-like object (labels, texts).
    fn text_rotation(self, r: Angle) -> Angle {
        let deg = i64::from(r.millidegrees()) / 1000;
        // Mirrored left to right, horizontal labels point the other way
        // (vertical ones stay), and the other way round.
        let flips = match self {
            Self::Horizontal => deg % 180 == 0,
            Self::Vertical => deg % 180 != 0,
            _ => false,
        };
        match self {
            Self::Left => Angle::from_degrees(deg + 90),
            Self::Right => Angle::from_degrees(deg - 90),
            _ if flips => Angle::from_degrees(deg + 180),
            _ => r,
        }
    }

    /// The new rotation of a power port (it points somewhere).
    fn port_rotation(self, r: Angle) -> Angle {
        let deg = i64::from(r.millidegrees()) / 1000;
        match self {
            Self::None => r,
            Self::Left => Angle::from_degrees(deg + 90),
            Self::Right => Angle::from_degrees(deg - 90),
            Self::Horizontal => Angle::from_degrees(-deg),
            Self::Vertical => Angle::from_degrees(180 - deg),
        }
    }

    /// The new (rotation, mirror) of a symbol. Symbols are mirrored left to
    /// right before they are turned.
    fn symbol(self, rotation: Angle, mirror: bool) -> (Angle, bool) {
        let deg = i64::from(rotation.millidegrees()) / 1000;
        match self {
            Self::None => (rotation, mirror),
            Self::Left => (Angle::from_degrees(deg + 90), mirror),
            Self::Right => (Angle::from_degrees(deg - 90), mirror),
            Self::Horizontal => (Angle::from_degrees(-deg), !mirror),
            Self::Vertical => (Angle::from_degrees(180 - deg), !mirror),
        }
    }
}

/// Transforms an object: `f` maps points, `turn` says how orientations
/// change.
fn transform(object: &mut Object, f: &dyn Fn(Point) -> Point, turn: Turn) {
    match object {
        Object::Symbol(s) => {
            s.at = f(s.at);
            (s.rotation, s.mirror) = turn.symbol(s.rotation, s.mirror);
            for field in &mut s.fields {
                field.at = f(field.at);
                field.rotation = match turn {
                    Turn::Left | Turn::Right => turn.text_rotation(field.rotation),
                    _ => field.rotation,
                };
            }
        }
        Object::Wire(w) => {
            w.start = f(w.start);
            w.end = f(w.end);
        }
        Object::Bus(b) => {
            b.start = f(b.start);
            b.end = f(b.end);
        }
        Object::BusEntry(e) => {
            e.at = f(e.at);
            e.size = turn.vector(e.size);
        }
        Object::Junction(j) => j.at = f(j.at),
        Object::NoConnect(n) => n.at = f(n.at),
        Object::Label(l) => {
            l.at = f(l.at);
            l.rotation = turn.text_rotation(l.rotation);
        }
        Object::PowerPort(p) => {
            p.at = f(p.at);
            p.rotation = turn.port_rotation(p.rotation);
        }
        Object::Text(t) => {
            t.at = f(t.at);
            t.rotation = turn.text_rotation(t.rotation);
        }
        Object::Sheet(sheet) => {
            // Sheets keep their size; their centre follows.
            let (w, h) = (sheet.size.w, sheet.size.h);
            let center = Point::new(sheet.at.x + w / 2, sheet.at.y - h / 2);
            let moved = f(center);
            sheet.at = Point::new(moved.x - w / 2, moved.y + h / 2);
            if turn == Turn::Horizontal {
                for pin in &mut sheet.pins {
                    pin.side = match pin.side {
                        SheetSide::Left => SheetSide::Right,
                        SheetSide::Right => SheetSide::Left,
                        other => other,
                    };
                }
            }
        }
        Object::Shape(shape) => transform_geometry(&mut shape.geometry, f),
        _ => {}
    }
}

fn transform_geometry(geometry: &mut Geometry, f: &dyn Fn(Point) -> Point) {
    match geometry {
        Geometry::Line { start, end } | Geometry::Rect { start, end } => {
            *start = f(*start);
            *end = f(*end);
        }
        Geometry::Circle { center, .. } => *center = f(*center),
        Geometry::Arc { start, mid, end } => {
            *start = f(*start);
            *mid = f(*mid);
            *end = f(*end);
        }
        Geometry::Polyline { points } | Geometry::Polygon { points } => {
            for p in points {
                *p = f(*p);
            }
        }
        Geometry::Bezier {
            start,
            control1,
            control2,
            end,
        } => {
            for p in [start, control1, control2, end] {
                *p = f(*p);
            }
        }
    }
}

fn objects(work: &Work<'_>, ids: &[Id]) -> Result<Vec<Object>, EditError> {
    let mut seen = BTreeSet::new();
    ids.iter()
        .filter(|id| seen.insert(**id))
        .map(|id| work.get(*id).ok_or(EditError::UnknownObject(*id)))
        .collect()
}

/// Moves objects. With `drag`, wires that end at a connection point of a
/// moved object stretch to follow it (rubber-banding), and junctions,
/// labels and flags at those points move along.
///
/// # Errors
///
/// Fails if the sheet or an object does not exist.
pub fn move_items(
    project: &Project,
    doc: Id,
    ids: &[Id],
    delta: Point,
    drag: bool,
) -> Result<Edit, EditError> {
    let mut work = Work::new(project, doc)?;
    let moved = objects(&work, ids)?;
    let moved_ids: BTreeSet<Id> = moved.iter().map(Object::id).collect();
    let anchors: BTreeSet<Point> = if drag {
        moved
            .iter()
            .flat_map(|o| connection_points(project, o))
            .collect()
    } else {
        BTreeSet::new()
    };
    let shift = |p: Point| p + delta;
    for mut object in moved {
        transform(&mut object, &shift, Turn::None);
        work.update(object);
    }
    if drag && !anchors.is_empty() {
        let sheet = work.sheet().clone();
        for wire in &sheet.wires {
            if moved_ids.contains(&wire.id) {
                continue;
            }
            let (start, end) = (anchors.contains(&wire.start), anchors.contains(&wire.end));
            if start && end {
                let mut wire = wire.clone();
                wire.start = wire.start + delta;
                wire.end = wire.end + delta;
                work.update(Object::Wire(wire));
            } else if start || end {
                let (fixed, moving) = if start {
                    (wire.end, wire.start)
                } else {
                    (wire.start, wire.end)
                };
                let path = stretch(fixed, moving, delta);
                // The wire keeps its direction; a corner adds a wire.
                let mut first = wire.clone();
                if start {
                    (first.start, first.end) = (path[1], path[0]);
                } else {
                    (first.start, first.end) = (path[0], path[1]);
                }
                work.update(Object::Wire(first));
                if let Some(&last) = path.get(2) {
                    let id = Id::random();
                    work.insert(Object::Wire(Wire {
                        id,
                        start: path[1],
                        end: last,
                    }));
                }
            }
        }
        for mut object in followers(&sheet, &anchors, &moved_ids) {
            transform(&mut object, &shift, Turn::None);
            work.update(object);
        }
        // Wires that shrank to nothing go.
        let empty: Vec<Id> = work
            .sheet()
            .wires
            .iter()
            .filter(|w| w.start == w.end)
            .map(|w| w.id)
            .collect();
        for id in empty {
            work.delete(id);
        }
    }
    work.select = ids.to_vec();
    Ok(work.finish(true))
}

/// The new path of a wire whose end `moving` follows a drag by `delta`:
/// a horizontal or vertical wire stays straight and gets a corner (as in
/// KiCad), any other wire just stretches.
#[must_use]
pub fn stretch(fixed: Point, moving: Point, delta: Point) -> Vec<Point> {
    let target = moving + delta;
    let horizontal = fixed.y == moving.y && fixed.x != moving.x;
    let vertical = fixed.x == moving.x && fixed.y != moving.y;
    let corner = if horizontal && delta.y != 0 {
        Point::new(target.x, fixed.y)
    } else if vertical && delta.x != 0 {
        Point::new(fixed.x, target.y)
    } else {
        return vec![fixed, target];
    };
    if corner == fixed || corner == target {
        vec![fixed, target]
    } else {
        vec![fixed, corner, target]
    }
}

/// Junctions, labels, power ports and flags that sit on a dragged
/// connection point (they move along).
fn followers(sheet: &Schematic, anchors: &BTreeSet<Point>, moved: &BTreeSet<Id>) -> Vec<Object> {
    sheet
        .junctions
        .iter()
        .filter(|j| anchors.contains(&j.at))
        .map(|j| Object::Junction(j.clone()))
        .chain(
            sheet
                .labels
                .iter()
                .filter(|l| anchors.contains(&l.at))
                .map(|l| Object::Label(l.clone())),
        )
        .chain(
            sheet
                .power_ports
                .iter()
                .filter(|p| anchors.contains(&p.at))
                .map(|p| Object::PowerPort(p.clone())),
        )
        .chain(
            sheet
                .no_connects
                .iter()
                .filter(|n| anchors.contains(&n.at))
                .map(|n| Object::NoConnect(n.clone())),
        )
        .filter(|o| !moved.contains(&o.id()))
        .collect()
}

/// What a drag of some objects drags along, for the preview.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DragInfo {
    /// Wires that stretch.
    pub wires: Vec<DragWire>,
    /// Junctions, labels, power ports and flags that move along.
    pub followers: Vec<Id>,
}

/// A wire that stretches when objects are dragged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DragWire {
    /// Wire id.
    pub id: Id,
    /// Start point.
    pub start: Point,
    /// End point.
    pub end: Point,
    /// The start follows the dragged objects.
    pub start_moves: bool,
    /// The end follows the dragged objects.
    pub end_moves: bool,
}

/// The wires that stretch and the objects that move along when `ids` are
/// dragged, for the preview of [`move_items`] with `drag`.
///
/// # Errors
///
/// Fails if the sheet or an object does not exist.
pub fn drag_info(project: &Project, doc: Id, ids: &[Id]) -> Result<DragInfo, EditError> {
    let work = Work::new(project, doc)?;
    let moved = objects(&work, ids)?;
    let moved_ids: BTreeSet<Id> = moved.iter().map(Object::id).collect();
    let anchors: BTreeSet<Point> = moved
        .iter()
        .flat_map(|o| connection_points(project, o))
        .collect();
    let sheet = work.sheet();
    let wires = sheet
        .wires
        .iter()
        .filter(|w| !moved_ids.contains(&w.id))
        .filter_map(|w| {
            let (start_moves, end_moves) = (anchors.contains(&w.start), anchors.contains(&w.end));
            (start_moves || end_moves).then_some(DragWire {
                id: w.id,
                start: w.start,
                end: w.end,
                start_moves,
                end_moves,
            })
        })
        .collect();
    let followers = followers(sheet, &anchors, &moved_ids)
        .iter()
        .map(Object::id)
        .collect();
    Ok(DragInfo { wires, followers })
}

/// The direction of a turn or mirror.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Orientation {
    /// Counterclockwise by 90°.
    Left,
    /// Clockwise by 90°.
    Right,
    /// Mirror left to right.
    Horizontal,
    /// Mirror top to bottom.
    Vertical,
}

/// Turns or mirrors objects about `center` (default: the centre of their
/// anchors, on the grid).
///
/// # Errors
///
/// Fails if the sheet or an object does not exist.
pub fn orient_items(
    project: &Project,
    doc: Id,
    ids: &[Id],
    orientation: Orientation,
    center: Option<Point>,
) -> Result<Edit, EditError> {
    let mut work = Work::new(project, doc)?;
    let items = objects(&work, ids)?;
    // One object turns about its anchor; several about their centre on the
    // grid, so objects on the grid stay there.
    let center = center.unwrap_or_else(|| {
        let middle = anchor_center(&items);
        if items.len() == 1 {
            return middle;
        }
        let grid = project.manifest.settings.schematic_grid;
        let origin = crate::geometry::grid_origin(work.sheet());
        crate::geometry::snap(middle, grid, origin)
    });
    let turn = match orientation {
        Orientation::Left => Turn::Left,
        Orientation::Right => Turn::Right,
        Orientation::Horizontal => Turn::Horizontal,
        Orientation::Vertical => Turn::Vertical,
    };
    let f = |p: Point| center + turn.vector(p - center);
    for mut object in items {
        transform(&mut object, &f, turn);
        work.update(object);
    }
    work.select = ids.to_vec();
    Ok(work.finish(true))
}

/// The centre of the anchors of objects.
fn anchor_center(items: &[Object]) -> Point {
    let mut points: Vec<Point> = Vec::new();
    for object in items {
        match object {
            Object::Symbol(s) => points.push(s.at),
            Object::Wire(w) => points.extend([w.start, w.end]),
            Object::Bus(b) => points.extend([b.start, b.end]),
            Object::Junction(j) => points.push(j.at),
            Object::NoConnect(n) => points.push(n.at),
            Object::Label(l) => points.push(l.at),
            Object::PowerPort(p) => points.push(p.at),
            Object::Text(t) => points.push(t.at),
            Object::BusEntry(e) => points.push(e.at),
            Object::Sheet(s) => {
                points.push(Point::new(s.at.x + s.size.w / 2, s.at.y - s.size.h / 2));
            }
            _ => {}
        }
    }
    if points.is_empty() {
        return Point::ORIGIN;
    }
    let (min_x, max_x) = (
        points.iter().map(|p| p.x).min().unwrap_or(0),
        points.iter().map(|p| p.x).max().unwrap_or(0),
    );
    let (min_y, max_y) = (
        points.iter().map(|p| p.y).min().unwrap_or(0),
        points.iter().map(|p| p.y).max().unwrap_or(0),
    );
    Point::new(min_x + (max_x - min_x) / 2, min_y + (max_y - min_y) / 2)
}

/// Deletes objects (junctions that are no longer needed go too).
///
/// # Errors
///
/// Fails if the sheet does not exist.
pub fn delete_items(project: &Project, doc: Id, ids: &[Id]) -> Result<Edit, EditError> {
    let mut work = Work::new(project, doc)?;
    for id in ids {
        work.delete(*id);
    }
    Ok(work.finish(true))
}

/// Pastes objects (from a copy, possibly of another sheet) with new ids,
/// moved by `offset`. Pasted symbols keep their designator if it is free,
/// otherwise they are numbered again (`annotate`) or left unannotated.
///
/// # Errors
///
/// Fails if the sheet does not exist or an object does not belong on a
/// schematic.
pub fn paste(
    project: &Project,
    doc: Id,
    objects: &[Object],
    offset: Point,
    annotate: bool,
) -> Result<Edit, EditError> {
    let mut work = Work::new(project, doc)?;
    let mut used = used_designators(project);
    // Units of one copied part stay one part.
    let mut components: BTreeMap<Id, Id> = BTreeMap::new();
    let mut renumbered: BTreeMap<String, String> = BTreeMap::new();
    let shift = |p: Point| p + offset;
    for object in objects {
        let mut object = object.clone();
        let id = Id::random();
        match &mut object {
            Object::Symbol(s) => {
                s.id = id;
                s.instances.clear();
                s.component = s
                    .component
                    .map(|c| *components.entry(c).or_insert_with(Id::random));
                if s.designator.starts_with('#') || used.contains(&s.designator) {
                    let new = renumbered.get(&s.designator).cloned().unwrap_or_else(|| {
                        let def = symbol_def(project, &s.symbol);
                        let prefix = def.map_or_else(
                            || {
                                split_designator(&s.designator)
                                    .map_or("U", |(p, _)| p)
                                    .to_owned()
                            },
                            |d| designator_prefix(&d.prefix, d.power),
                        );
                        if annotate {
                            let n = next_number(&used, &prefix, 1);
                            format!("{prefix}{n}")
                        } else {
                            format!("{prefix}?")
                        }
                    });
                    renumbered.insert(s.designator.clone(), new.clone());
                    s.designator = new;
                }
                used.insert(s.designator.clone());
            }
            Object::Wire(o) => o.id = id,
            Object::Bus(o) => o.id = id,
            Object::BusEntry(o) => o.id = id,
            Object::Junction(o) => o.id = id,
            Object::Label(o) => o.id = id,
            Object::PowerPort(o) => o.id = id,
            Object::NoConnect(o) => o.id = id,
            Object::Text(o) => o.id = id,
            Object::Shape(o) => o.id = id,
            Object::Sheet(o) => {
                o.id = id;
                for pin in &mut o.pins {
                    pin.id = Id::random();
                }
            }
            other => {
                return Err(EditError::Invalid(format!(
                    "a {} cannot be pasted on a schematic",
                    other.object_type().name()
                )));
            }
        }
        transform(&mut object, &shift, Turn::None);
        work.insert(object);
        work.select.push(id);
    }
    Ok(work.finish(true))
}

/// Gives a sheet symbol a pin for every hierarchical label of its sheet
/// that has none yet (new pins go down the left side), and with `remove`
/// removes pins without a label.
///
/// # Errors
///
/// Fails if the sheet or the sheet symbol does not exist.
pub fn sync_sheet_pins(
    project: &Project,
    doc: Id,
    id: Id,
    remove: bool,
) -> Result<Edit, EditError> {
    let mut work = Work::new(project, doc)?;
    let Some(Object::Sheet(mut symbol)) = work.get(id) else {
        return Err(EditError::UnknownObject(id));
    };
    let Some(Document::Schematic(child)) = project.document(symbol.schematic) else {
        return Err(EditError::Invalid(format!(
            "sheet `{}` has no schematic",
            symbol.name
        )));
    };
    let mut labels: Vec<&Label> = child
        .labels
        .iter()
        .filter(|l| l.kind == LabelKind::Hierarchical)
        .collect();
    labels.sort_by_key(|l| (-l.at.y, l.at.x));
    let names: BTreeSet<&str> = labels.iter().map(|l| l.text.as_str()).collect();
    if remove {
        symbol.pins.retain(|pin| names.contains(pin.name.as_str()));
    }
    let grid = project.manifest.settings.schematic_grid.max(1);
    let step = grid * 2;
    let mut seen: BTreeSet<String> = symbol.pins.iter().map(|p| p.name.clone()).collect();
    let mut offset = symbol
        .pins
        .iter()
        .filter(|p| p.side == SheetSide::Left)
        .map(|p| p.offset)
        .max()
        .unwrap_or(0)
        + step;
    for label in labels {
        if !seen.insert(label.text.clone()) {
            continue;
        }
        symbol.pins.push(ew_model::schematic::SheetPin {
            id: Id::random(),
            name: label.text.clone(),
            shape: label.shape,
            side: SheetSide::Left,
            offset,
        });
        offset += step;
    }
    // Make the sheet tall enough for its pins.
    let needed = symbol
        .pins
        .iter()
        .filter(|p| matches!(p.side, SheetSide::Left | SheetSide::Right))
        .map(|p| p.offset)
        .max()
        .unwrap_or(0)
        + step;
    if needed > symbol.size.h {
        symbol.size.h = (needed + grid - 1) / grid * grid;
    }
    work.update(Object::Sheet(symbol));
    work.select.push(id);
    Ok(work.finish(true))
}

/// Moves objects onto the grid: symbols and sheets by their anchor, wires
/// by each end, labels, junctions, flags and texts by their position. All
/// objects of the sheet if `ids` is empty.
///
/// # Errors
///
/// Fails if the sheet does not exist.
pub fn snap_to_grid(project: &Project, doc: Id, ids: &[Id]) -> Result<Edit, EditError> {
    let mut work = Work::new(project, doc)?;
    let grid = project.manifest.settings.schematic_grid;
    let origin = crate::geometry::grid_origin(work.sheet());
    let snap = |p: Point| crate::geometry::snap(p, grid, origin);
    let sheet = work.sheet().clone();
    let all: Vec<Object> = sheet
        .symbols
        .iter()
        .cloned()
        .map(Object::Symbol)
        .chain(sheet.wires.iter().cloned().map(Object::Wire))
        .chain(sheet.junctions.iter().cloned().map(Object::Junction))
        .chain(sheet.labels.iter().cloned().map(Object::Label))
        .chain(sheet.power_ports.iter().cloned().map(Object::PowerPort))
        .chain(sheet.no_connects.iter().cloned().map(Object::NoConnect))
        .chain(sheet.sheets.iter().cloned().map(Object::Sheet))
        .chain(sheet.texts.iter().cloned().map(Object::Text))
        .filter(|o| ids.is_empty() || ids.contains(&o.id()))
        .collect();
    for mut object in all {
        match &mut object {
            Object::Wire(w) => {
                w.start = snap(w.start);
                w.end = snap(w.end);
                if w.start == w.end {
                    work.delete(w.id);
                    continue;
                }
            }
            other => {
                let anchor = match other {
                    Object::Symbol(s) => s.at,
                    Object::Junction(j) => j.at,
                    Object::Label(l) => l.at,
                    Object::PowerPort(p) => p.at,
                    Object::NoConnect(n) => n.at,
                    Object::Sheet(s) => s.at,
                    Object::Text(t) => t.at,
                    _ => continue,
                };
                let delta = snap(anchor) - anchor;
                if delta == Point::ORIGIN {
                    continue;
                }
                if let Object::Sheet(s) = other {
                    s.at = s.at + delta;
                } else {
                    transform(other, &|p| p + delta, Turn::None);
                }
            }
        }
        work.update(object);
    }
    Ok(work.finish(true))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use ew_model::units::mm;

    /// A project with one sheet and a two-pin resistor (pins 5 mm above
    /// and below the anchor).
    pub(crate) fn project() -> (Project, Id) {
        let mut project = Project::new(Id::random(), "Test");
        let sheet = Id::random();
        let add = |project: &mut Project, document: Document, kind, path: &str| {
            let reference = ew_model::project::DocumentRef {
                id: document.id(),
                kind,
                path: path.to_owned(),
            };
            project
                .add_document(reference, usize::MAX, document)
                .unwrap();
        };
        add(
            &mut project,
            Document::Schematic(Schematic::new(sheet, "Main")),
            ew_model::document::DocumentKind::Schematic,
            "main.ewsch",
        );
        let mut library = ew_model::library::Library::new(Id::random(), "Project");
        let def: ew_model::library::SymbolDef = serde_json::from_value(serde_json::json!({
            "id": Id::random(),
            "name": "R",
            "prefix": "R",
            "value": "10k",
            "units": [{"pins": [
                {"number": "1", "name": "~", "kind": "passive", "at": [0, mm(5)], "direction": "down", "length": mm(1)},
                {"number": "2", "name": "~", "kind": "passive", "at": [0, -mm(5)], "direction": "up", "length": mm(1)}
            ]}]
        }))
        .unwrap();
        library.symbols.insert(def);
        add(
            &mut project,
            Document::Library(library),
            ew_model::document::DocumentKind::Library,
            "project.ewlib",
        );
        (project, sheet)
    }

    pub(crate) fn apply(project: &mut Project, edit: &Edit) {
        for change in &edit.changes {
            match change {
                Change::Insert { doc, object } => {
                    project
                        .document_mut(*doc)
                        .unwrap()
                        .insert_object(object.clone())
                        .unwrap();
                }
                Change::Update { doc, object } => {
                    project
                        .document_mut(*doc)
                        .unwrap()
                        .update_object(object.clone())
                        .unwrap();
                }
                Change::Delete { doc, id } => {
                    project
                        .document_mut(*doc)
                        .unwrap()
                        .remove_object(*id)
                        .unwrap();
                }
            }
        }
    }

    pub(crate) fn sheet(project: &Project, id: Id) -> &Schematic {
        match project.document(id) {
            Some(Document::Schematic(s)) => s,
            _ => panic!("no sheet"),
        }
    }

    pub(crate) fn place(project: &mut Project, doc: Id, at: Point, annotate: bool) -> Id {
        let edit = place_symbol(
            project,
            doc,
            &PlaceSymbol {
                symbol: "R".to_owned(),
                at,
                annotate,
                ..PlaceSymbol::default()
            },
        )
        .unwrap();
        apply(project, &edit);
        edit.select[0]
    }

    #[test]
    fn placing_numbers_designators() {
        let (mut project, doc) = project();
        place(&mut project, doc, Point::ORIGIN, true);
        place(&mut project, doc, Point::new(mm(10), 0), true);
        let names: BTreeSet<String> = sheet(&project, doc)
            .symbols
            .iter()
            .map(|s| s.designator.clone())
            .collect();
        assert_eq!(names, BTreeSet::from(["R1".to_owned(), "R2".to_owned()]));
        assert_eq!(split_designator("R12"), Some(("R", 12)));
        assert_eq!(split_designator("R?"), None);
    }

    #[test]
    fn wires_get_junctions_at_tees_but_not_at_crossings() {
        let (mut project, doc) = project();
        // A horizontal wire and a vertical one crossing it.
        let edit = draw_wire(
            &project,
            doc,
            &[Point::new(0, 0), Point::new(mm(10), 0)],
            false,
        )
        .unwrap();
        apply(&mut project, &edit);
        let edit = draw_wire(
            &project,
            doc,
            &[Point::new(mm(5), mm(5)), Point::new(mm(5), -mm(5))],
            false,
        )
        .unwrap();
        apply(&mut project, &edit);
        assert_eq!(sheet(&project, doc).junctions.len(), 0);
        // A wire ending on the middle of another makes a tee.
        let edit = draw_wire(
            &project,
            doc,
            &[Point::new(mm(2), mm(5)), Point::new(mm(2), 0)],
            false,
        )
        .unwrap();
        apply(&mut project, &edit);
        let junctions: Vec<Point> = sheet(&project, doc)
            .junctions
            .iter()
            .map(|j| j.at)
            .collect();
        assert_eq!(junctions, [Point::new(mm(2), 0)]);
        // Deleting the branch removes the junction.
        let branch = edit.select.clone();
        let edit = delete_items(&project, doc, &branch).unwrap();
        apply(&mut project, &edit);
        assert_eq!(sheet(&project, doc).junctions.len(), 0);
    }

    #[test]
    fn straight_runs_are_merged() {
        let (project, doc) = project();
        let edit = draw_wire(
            &project,
            doc,
            &[
                Point::new(0, 0),
                Point::new(mm(5), 0),
                Point::new(mm(10), 0),
                Point::new(mm(10), mm(5)),
            ],
            false,
        )
        .unwrap();
        assert_eq!(edit.changes.len(), 2);
        assert!(draw_wire(&project, doc, &[Point::new(0, 0), Point::new(0, 0)], false).is_err());
    }

    #[test]
    fn dragging_a_symbol_stretches_its_wires() {
        let (mut project, doc) = project();
        let r = place(&mut project, doc, Point::ORIGIN, true);
        let edit = draw_wire(
            &project,
            doc,
            &[Point::new(0, mm(5)), Point::new(0, mm(15))],
            false,
        )
        .unwrap();
        apply(&mut project, &edit);
        let wire = edit.select[0];
        let edit = move_items(&project, doc, &[r], Point::new(mm(5), 0), true).unwrap();
        apply(&mut project, &edit);
        // The vertical wire stays vertical; a corner wire reaches the pin.
        let moved = sheet(&project, doc).wires.get(wire).unwrap();
        assert_eq!(
            (moved.start, moved.end),
            (Point::new(0, mm(5)), Point::new(0, mm(15)))
        );
        let wires: BTreeSet<(Point, Point)> = sheet(&project, doc)
            .wires
            .iter()
            .map(|w| (w.start, w.end))
            .collect();
        assert!(
            wires.contains(&(Point::new(0, mm(5)), Point::new(mm(5), mm(5)))),
            "{wires:?}"
        );
        // Without drag the wire stays.
        let edit = move_items(&project, doc, &[r], Point::new(mm(5), 0), false).unwrap();
        assert_eq!(edit.changes.len(), 1);
    }

    #[test]
    fn straight_wires_stay_straight_when_dragged() {
        let (a, b) = (Point::new(0, 0), Point::new(mm(10), 0));
        assert_eq!(
            stretch(a, b, Point::new(mm(2), mm(5))),
            [a, Point::new(mm(12), 0), Point::new(mm(12), mm(5))]
        );
        assert_eq!(
            stretch(a, b, Point::new(mm(2), 0)),
            [a, Point::new(mm(12), 0)]
        );
        assert_eq!(
            stretch(a, b, Point::new(-mm(10), mm(5))),
            [a, Point::new(0, mm(5))]
        );
        let diagonal = Point::new(mm(3), mm(4));
        assert_eq!(
            stretch(a, diagonal, Point::new(mm(1), mm(1))),
            [a, Point::new(mm(4), mm(5))]
        );
    }

    #[test]
    fn turning_and_mirroring_symbols() {
        let (mut project, doc) = project();
        let r = place(&mut project, doc, Point::new(mm(10), mm(10)), true);
        let edit = orient_items(&project, doc, &[r], Orientation::Left, None).unwrap();
        apply(&mut project, &edit);
        let s = sheet(&project, doc).symbols.get(r).unwrap();
        assert_eq!(
            (s.at, s.rotation, s.mirror),
            (Point::new(mm(10), mm(10)), Angle::DEG_90, false)
        );
        // Pin 1 (above the anchor) now points left.
        let pins = symbol_pins(&project, s);
        assert_eq!(pins[0], Point::new(mm(5), mm(10)));
        let edit = orient_items(&project, doc, &[r], Orientation::Horizontal, None).unwrap();
        apply(&mut project, &edit);
        let s = sheet(&project, doc).symbols.get(r).unwrap();
        assert_eq!((s.rotation, s.mirror), (Angle::DEG_270, true));
        // Mirroring left to right moves the left pin to the right.
        assert_eq!(symbol_pins(&project, s)[0], Point::new(mm(15), mm(10)));
    }

    #[test]
    fn snapping_moves_wire_ends_onto_the_grid() {
        let (mut project, doc) = project();
        let grid = project.manifest.settings.schematic_grid;
        let top = sheet(&project, doc).page.dimensions().h;
        let edit = draw_wire(
            &project,
            doc,
            &[
                Point::new(grid + 10, top - grid),
                Point::new(grid * 5, top - grid + 7),
            ],
            false,
        )
        .unwrap();
        apply(&mut project, &edit);
        let edit = snap_to_grid(&project, doc, &[]).unwrap();
        apply(&mut project, &edit);
        let wire = sheet(&project, doc).wires.iter().next().unwrap();
        assert_eq!(
            (wire.start, wire.end),
            (
                Point::new(grid, top - grid),
                Point::new(grid * 5, top - grid)
            )
        );
        assert_eq!(snap_to_grid(&project, doc, &[]).unwrap().changes.len(), 0);
    }

    #[test]
    fn pasting_renumbers_taken_designators() {
        let (mut project, doc) = project();
        let r = place(&mut project, doc, Point::ORIGIN, true);
        let copy = project.document(doc).unwrap().object(r).unwrap();
        let edit = paste(&project, doc, &[copy], Point::new(mm(20), 0), true).unwrap();
        apply(&mut project, &edit);
        let names: BTreeSet<String> = sheet(&project, doc)
            .symbols
            .iter()
            .map(|s| s.designator.clone())
            .collect();
        assert_eq!(names, BTreeSet::from(["R1".to_owned(), "R2".to_owned()]));
    }
}
