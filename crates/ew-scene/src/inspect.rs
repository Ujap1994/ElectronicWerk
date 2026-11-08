//! Read-only object properties for the inspector (`view.object`).

use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::layer::{Layer, LayerSpec, Side};
use ew_model::pcb::{FootprintInstance, Pcb};
use ew_model::project::Project;
use ew_model::schematic::{PlacedSymbol, Schematic};
use ew_model::units::{Point, Size};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::builder::ObjType;
use crate::geom::{self, P, Xform};
use crate::schematic::{DEFAULT_DESIGNATOR, DEFAULT_VALUE};

/// Identifies an object or a child of an object (pad, pin, field, …).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObjectRef {
    /// Document id.
    pub doc: Id,
    /// Object id (the parent's id for children).
    pub id: Id,
    /// Child type and index within the parent.
    #[serde(default)]
    pub child: Option<ChildRef>,
}

/// A child of an object.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildRef {
    /// `pad`, `footprintText`, `pin`, `field` or `sheetPin`.
    pub kind: String,
    /// Index within the parent.
    pub index: usize,
}

/// One property row. `kind` tells the client how to format `value`:
/// `text`, `length`, `point`, `size`, `angle`, `bool`, `layer`, `layers`,
/// `net`, `enum`, `count`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Row {
    /// Property key (translated by the client).
    pub key: &'static str,
    /// Value (lengths in nanometres, angles in degrees).
    pub value: Value,
    /// Value kind.
    pub kind: &'static str,
}

/// The properties of one object.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ObjectInfo {
    /// Object type (`track`, `pad`, …).
    #[serde(rename = "type")]
    pub ty: &'static str,
    /// Object id (the parent's id for children).
    pub id: Id,
    /// A short title, for example a designator or net name.
    pub title: String,
    /// Property rows.
    pub rows: Vec<Row>,
}

fn row(key: &'static str, value: impl Into<Value>, kind: &'static str) -> Row {
    Row {
        key,
        value: value.into(),
        kind,
    }
}

fn point(key: &'static str, p: Point) -> Row {
    row(key, json!([p.x, p.y]), "point")
}

fn size(key: &'static str, s: Size) -> Row {
    row(key, json!([s.w, s.h]), "size")
}

fn name_of<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| match v {
            Value::String(s) => Some(s),
            Value::Object(map) => map.keys().next().cloned(),
            _ => None,
        })
        .unwrap_or_default()
}

fn rounded(p: P) -> Point {
    Point::new(p.x.round() as i64, p.y.round() as i64)
}

/// Looks up an object and describes it; `None` if it does not exist.
#[must_use]
pub fn describe(project: &Project, target: &ObjectRef) -> Option<ObjectInfo> {
    match project.document(target.doc)? {
        Document::Pcb(pcb) => board(pcb, target),
        Document::Schematic(sheet) => schematic(project, sheet, target),
        _ => None,
    }
}

fn net_name(pcb: &Pcb, net: Option<Id>) -> Value {
    net.and_then(|id| pcb.nets.get(id))
        .map_or(Value::Null, |net| Value::String(net.name.clone()))
}

fn footprint_xf(fp: &FootprintInstance) -> Xform {
    Xform {
        offset: P::of(fp.at),
        angle: geom::radians(fp.rotation),
        mirror: fp.side == Side::Bottom,
    }
}

#[allow(clippy::too_many_lines, reason = "one arm per object type")]
fn board(pcb: &Pcb, target: &ObjectRef) -> Option<ObjectInfo> {
    let info = |ty: ObjType, title: String, rows: Vec<Row>| ObjectInfo {
        ty: ty.name(),
        id: target.id,
        title,
        rows,
    };
    if let Some(fp) = pcb.footprints.get(target.id) {
        let xf = footprint_xf(fp);
        let copper_count = pcb.stackup.copper_count();
        let flip = |layer: Layer| {
            if fp.side == Side::Bottom {
                layer.flipped(copper_count)
            } else {
                layer
            }
        };
        return match &target.child {
            None => Some(info(
                ObjType::Footprint,
                fp.designator.clone(),
                vec![
                    row("designator", fp.designator.clone(), "text"),
                    row("value", fp.value.clone(), "text"),
                    row("footprint", fp.footprint.clone(), "text"),
                    point("position", fp.at),
                    row("rotation", fp.rotation.degrees(), "angle"),
                    row("side", fp.side.as_str(), "enum"),
                    row("pads", fp.pads.len(), "count"),
                    row("mounting", name_of(&fp.attributes.mounting), "enum"),
                    row("locked", fp.locked, "bool"),
                    row("dnp", fp.attributes.dnp, "bool"),
                    row("excludeFromBom", fp.attributes.exclude_from_bom, "bool"),
                ],
            )),
            Some(child) if child.kind == "pad" => {
                let pad = fp.pads.get(child.index)?;
                let at = rounded(xf.point(pad.at));
                let layers: Vec<String> = pad
                    .layers
                    .iter()
                    .map(|spec| match spec {
                        LayerSpec::Layer(layer) => flip(*layer).to_string(),
                        other => other.to_string(),
                    })
                    .collect();
                let mut rows = vec![
                    row("number", pad.number.clone(), "text"),
                    row("footprint", fp.designator.clone(), "text"),
                    row("net", net_name(pcb, pad.net), "net"),
                    row("padType", name_of(&pad.kind), "enum"),
                    row("shape", name_of(&pad.shape), "enum"),
                    point("position", at),
                    size("size", pad.size),
                    row("rotation", pad.rotation.degrees(), "angle"),
                    row("layers", layers, "layers"),
                ];
                if let Some(drill) = pad.drill {
                    rows.push(row("drill", drill.diameter, "length"));
                    if let Some(length) = drill.slot_length {
                        rows.push(row("slotLength", length, "length"));
                    }
                }
                Some(ObjectInfo {
                    ty: ObjType::Pad.name(),
                    id: target.id,
                    title: format!("{}-{}", fp.designator, pad.number),
                    rows,
                })
            }
            Some(child) if child.kind == "footprintText" => {
                let text = fp.texts.get(child.index)?;
                let content = match text.kind {
                    ew_model::pcb::FootprintTextKind::Designator if text.text.is_empty() => {
                        fp.designator.clone()
                    }
                    ew_model::pcb::FootprintTextKind::Value if text.text.is_empty() => {
                        fp.value.clone()
                    }
                    _ => text.text.clone(),
                };
                Some(ObjectInfo {
                    ty: ObjType::FootprintText.name(),
                    id: target.id,
                    title: content.clone(),
                    rows: vec![
                        row("text", content, "text"),
                        row("textKind", name_of(&text.kind), "enum"),
                        row("footprint", fp.designator.clone(), "text"),
                        row("layer", flip(text.layer).to_string(), "layer"),
                        point("position", rounded(xf.point(text.at))),
                        row("textSize", text.style.size, "length"),
                    ],
                })
            }
            Some(_) => None,
        };
    }
    if target.child.is_some() {
        return None;
    }
    if let Some(track) = pcb.tracks.get(target.id) {
        let length = P::of(track.end).sub(P::of(track.start)).len().round() as i64;
        let mut rows = vec![
            row("layer", track.layer.to_string(), "layer"),
            row("net", net_name(pcb, track.net), "net"),
            row("width", track.width, "length"),
            point("start", track.start),
            point("end", track.end),
        ];
        if let Some(mid) = track.mid {
            rows.push(point("mid", mid));
        } else {
            rows.push(row("length", length, "length"));
        }
        rows.push(row("locked", track.locked, "bool"));
        let title = net_name(pcb, track.net)
            .as_str()
            .unwrap_or_default()
            .to_owned();
        return Some(info(ObjType::Track, title, rows));
    }
    if let Some(via) = pcb.vias.get(target.id) {
        let title = net_name(pcb, via.net)
            .as_str()
            .unwrap_or_default()
            .to_owned();
        return Some(info(
            ObjType::Via,
            title,
            vec![
                row("net", net_name(pcb, via.net), "net"),
                point("position", via.at),
                row("diameter", via.diameter, "length"),
                row("drill", via.drill, "length"),
                row("from", via.from.to_string(), "layer"),
                row("to", via.to.to_string(), "layer"),
                row("micro", via.micro, "bool"),
                row("locked", via.locked, "bool"),
            ],
        ));
    }
    if let Some(zone) = pcb.zones.get(target.id) {
        let layers: Vec<String> = zone.layers.iter().map(ToString::to_string).collect();
        let title = if zone.name.is_empty() {
            net_name(pcb, zone.net)
                .as_str()
                .unwrap_or_default()
                .to_owned()
        } else {
            zone.name.clone()
        };
        return Some(info(
            ObjType::Zone,
            title,
            vec![
                row("name", zone.name.clone(), "text"),
                row("net", net_name(pcb, zone.net), "net"),
                row("layers", layers, "layers"),
                row("priority", zone.priority, "count"),
                row("corners", zone.outline.len(), "count"),
                row("keepout", zone.keepout.is_some(), "bool"),
                row("clearance", zone.fill.clearance, "length"),
            ],
        ));
    }
    if let Some(text) = pcb.texts.get(target.id) {
        return Some(info(
            ObjType::BoardText,
            text.text.clone(),
            vec![
                row("text", text.text.clone(), "text"),
                row("layer", text.layer.to_string(), "layer"),
                point("position", text.at),
                row("rotation", text.rotation.degrees(), "angle"),
                row("textSize", text.style.size, "length"),
                row("mirrored", text.mirrored, "bool"),
            ],
        ));
    }
    if let Some(shape) = pcb.shapes.get(target.id) {
        return Some(info(
            ObjType::BoardShape,
            String::new(),
            vec![
                row("shape", shape.geometry.kind_name(), "enum"),
                row("layer", shape.layer.to_string(), "layer"),
                row("width", shape.width, "length"),
                row("filled", shape.filled, "bool"),
            ],
        ));
    }
    None
}

fn symbol_xf(symbol: &PlacedSymbol) -> Xform {
    Xform {
        offset: P::of(symbol.at),
        angle: geom::radians(symbol.rotation),
        mirror: symbol.mirror,
    }
}

#[allow(clippy::too_many_lines, reason = "one arm per object type")]
fn schematic(project: &Project, sheet: &Schematic, target: &ObjectRef) -> Option<ObjectInfo> {
    let info = |ty: ObjType, title: String, rows: Vec<Row>| ObjectInfo {
        ty: ty.name(),
        id: target.id,
        title,
        rows,
    };
    if let Some(symbol) = sheet.symbols.get(target.id) {
        let def = project.documents.values().find_map(|doc| match doc {
            Document::Library(library) => library.symbol(&symbol.symbol),
            _ => None,
        });
        let unit = def.and_then(|def| def.units.get(usize::from(symbol.unit.max(1) - 1)));
        return match &target.child {
            None => Some(info(
                ObjType::Symbol,
                symbol.designator.clone(),
                vec![
                    row("designator", symbol.designator.clone(), "text"),
                    row("value", symbol.value.clone(), "text"),
                    row("symbol", symbol.symbol.clone(), "text"),
                    row(
                        "footprint",
                        symbol.footprint.clone().unwrap_or_default(),
                        "text",
                    ),
                    point("position", symbol.at),
                    row("rotation", symbol.rotation.degrees(), "angle"),
                    row("mirror", symbol.mirror, "bool"),
                    row("unit", symbol.unit, "count"),
                    row("pins", unit.map_or(0, |unit| unit.pins.len()), "count"),
                    row(
                        "description",
                        def.map(|d| d.description.clone()).unwrap_or_default(),
                        "text",
                    ),
                    row("dnp", symbol.flags.dnp, "bool"),
                    row("excludeFromBom", symbol.flags.exclude_from_bom, "bool"),
                    row("excludeFromBoard", symbol.flags.exclude_from_board, "bool"),
                ],
            )),
            Some(child) if child.kind == "pin" => {
                let pin = unit?.pins.get(child.index)?;
                let at = rounded(symbol_xf(symbol).point(pin.at));
                Some(ObjectInfo {
                    ty: ObjType::Pin.name(),
                    id: target.id,
                    title: format!("{}-{}", symbol.designator, pin.number),
                    rows: vec![
                        row("number", pin.number.clone(), "text"),
                        row("name", pin.name.clone(), "text"),
                        row("symbol", symbol.designator.clone(), "text"),
                        row("pinType", name_of(&pin.kind), "enum"),
                        row("pinShape", name_of(&pin.shape), "enum"),
                        point("position", at),
                        row("length", pin.length, "length"),
                    ],
                })
            }
            Some(child) if child.kind == "field" => {
                let (name, value) = match child.index {
                    DEFAULT_DESIGNATOR => ("designator".to_owned(), symbol.designator.clone()),
                    DEFAULT_VALUE => ("value".to_owned(), symbol.value.clone()),
                    index => {
                        let field = symbol.fields.get(index)?;
                        let value = match field.name.to_ascii_lowercase().as_str() {
                            "designator" | "reference" => symbol.designator.clone(),
                            "value" => symbol.value.clone(),
                            "footprint" => symbol.footprint.clone().unwrap_or_default(),
                            _ => symbol
                                .properties
                                .get(&field.name)
                                .cloned()
                                .unwrap_or_default(),
                        };
                        (field.name.clone(), value)
                    }
                };
                Some(ObjectInfo {
                    ty: ObjType::Field.name(),
                    id: target.id,
                    title: value.clone(),
                    rows: vec![
                        row("fieldName", name, "text"),
                        row("text", value, "text"),
                        row("symbol", symbol.designator.clone(), "text"),
                    ],
                })
            }
            Some(_) => None,
        };
    }
    if let Some(sheet_symbol) = sheet.sheets.get(target.id) {
        let file = project
            .reference(sheet_symbol.schematic)
            .map(|entry| entry.path.clone())
            .unwrap_or_default();
        return match &target.child {
            None => Some(info(
                ObjType::Sheet,
                sheet_symbol.name.clone(),
                vec![
                    row("name", sheet_symbol.name.clone(), "text"),
                    row("file", file, "text"),
                    point("position", sheet_symbol.at),
                    size("size", sheet_symbol.size),
                    row("pins", sheet_symbol.pins.len(), "count"),
                ],
            )),
            Some(child) if child.kind == "sheetPin" => {
                let pin = sheet_symbol.pins.get(child.index)?;
                Some(ObjectInfo {
                    ty: ObjType::SheetPin.name(),
                    id: target.id,
                    title: pin.name.clone(),
                    rows: vec![
                        row("name", pin.name.clone(), "text"),
                        row("shape", name_of(&pin.shape), "enum"),
                        row("edge", name_of(&pin.side), "enum"),
                        row("sheet", sheet_symbol.name.clone(), "text"),
                    ],
                })
            }
            Some(_) => None,
        };
    }
    if target.child.is_some() {
        return None;
    }
    let segment = |ty: ObjType, start: Point, end: Point| {
        let length = P::of(end).sub(P::of(start)).len().round() as i64;
        info(
            ty,
            String::new(),
            vec![
                point("start", start),
                point("end", end),
                row("length", length, "length"),
            ],
        )
    };
    if let Some(wire) = sheet.wires.get(target.id) {
        return Some(segment(ObjType::Wire, wire.start, wire.end));
    }
    if let Some(bus) = sheet.buses.get(target.id) {
        return Some(segment(ObjType::Bus, bus.start, bus.end));
    }
    if let Some(entry) = sheet.bus_entries.get(target.id) {
        return Some(info(
            ObjType::BusEntry,
            String::new(),
            vec![point("position", entry.at), point("offset", entry.size)],
        ));
    }
    if let Some(junction) = sheet.junctions.get(target.id) {
        return Some(info(
            ObjType::Junction,
            String::new(),
            vec![point("position", junction.at)],
        ));
    }
    if let Some(flag) = sheet.no_connects.get(target.id) {
        return Some(info(
            ObjType::NoConnect,
            String::new(),
            vec![point("position", flag.at)],
        ));
    }
    if let Some(label) = sheet.labels.get(target.id) {
        return Some(info(
            ObjType::Label,
            label.text.clone(),
            vec![
                row("text", label.text.clone(), "text"),
                row("labelKind", name_of(&label.kind), "enum"),
                row("shape", name_of(&label.shape), "enum"),
                point("position", label.at),
                row("rotation", label.rotation.degrees(), "angle"),
                row("textSize", label.style.size, "length"),
            ],
        ));
    }
    if let Some(port) = sheet.power_ports.get(target.id) {
        return Some(info(
            ObjType::PowerPort,
            port.net.clone(),
            vec![
                row("net", port.net.clone(), "net"),
                row("powerStyle", name_of(&port.style), "enum"),
                point("position", port.at),
                row("rotation", port.rotation.degrees(), "angle"),
            ],
        ));
    }
    if let Some(text) = sheet.texts.get(target.id) {
        return Some(info(
            ObjType::Text,
            text.text.clone(),
            vec![
                row("text", text.text.clone(), "text"),
                point("position", text.at),
                row("rotation", text.rotation.degrees(), "angle"),
                row("textSize", text.style.size, "length"),
            ],
        ));
    }
    if let Some(shape) = sheet.shapes.get(target.id) {
        return Some(info(
            ObjType::Shape,
            String::new(),
            vec![
                row("shape", shape.geometry.kind_name(), "enum"),
                row("width", shape.stroke.width, "length"),
                row("fill", name_of(&shape.fill), "enum"),
            ],
        ));
    }
    None
}
