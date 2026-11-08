//! KiCad schematic sheets (`.kicad_sch`).

use std::collections::BTreeMap;

use ew_model::id::Id;
use ew_model::library::SymbolDef;
use ew_model::schematic::{
    Bus, BusEntry, ComponentFlags, FieldLabel, Junction, Label, LabelKind, NoConnect, Orientation,
    Page, PageFormat, PlacedSymbol, PortShape, Schematic, Shape, SheetPin, SheetSide, SheetSymbol,
    Text, TitleBlock, Wire,
};
use ew_model::shape::Fill;
use ew_model::text::{HAlign, TextStyle, VAlign};
use ew_model::units::{Point, Size};

use crate::common::{self, Frame, Ids, angle, derived_id, nm};
use crate::pcb::component_id;
use crate::report::ImportReport;
use crate::sexpr::Sexp;
use crate::symbol;

/// Symbol definitions collected from the symbol caches of every sheet.
#[derive(Debug, Default)]
pub struct SymbolTable {
    /// Definitions by name (`Device:R`).
    pub defs: BTreeMap<String, SymbolDef>,
}

impl SymbolTable {
    /// Reads the `lib_symbols` cache of a sheet.
    pub fn add_cache(&mut self, sheet: &Sexp<'_>, report: &mut ImportReport, file: &str) {
        let Some(cache) = sheet.get("lib_symbols") else {
            return;
        };
        for node in cache.all("symbol") {
            let name = node.arg(0).unwrap_or_default();
            if name.is_empty() || self.defs.contains_key(name) {
                continue;
            }
            let id = derived_id(&["symbol-def", name]);
            let def = symbol::symbol_def(node, id, name, None, report, file);
            self.defs.insert(name.to_owned(), def);
        }
    }
}

/// A sheet symbol that places another file.
#[derive(Debug, Clone)]
pub struct ChildSheet {
    /// The sheet symbol in the parent.
    pub symbol: Id,
    /// The file (relative to the project folder).
    pub file: String,
    /// The sheet symbol's KiCad UUID (part of the instance path).
    pub uuid: String,
    /// The sheet's name.
    pub name: String,
}

/// Page format and frame of a `(paper …)` entry.
#[must_use]
pub fn page(paper: Option<&Sexp<'_>>) -> Page {
    let Some(paper) = paper else {
        return Page::default();
    };
    let orientation = if paper.has_word("portrait") {
        Orientation::Portrait
    } else {
        Orientation::Landscape
    };
    let format = match paper.arg(0).unwrap_or("A4") {
        "A5" => PageFormat::A5,
        "A3" => PageFormat::A3,
        "A2" => PageFormat::A2,
        "A1" => PageFormat::A1,
        "A0" => PageFormat::A0,
        "A" | "USLetter" => PageFormat::Letter,
        "USLegal" => PageFormat::Legal,
        "B" | "USLedger" => PageFormat::Tabloid,
        "User" => {
            return Page {
                format: PageFormat::Custom,
                orientation: Orientation::Landscape,
                size: Some(Size::new(
                    nm(paper.num(1).unwrap_or(297.0)),
                    nm(paper.num(2).unwrap_or(210.0)),
                )),
            };
        }
        _ => PageFormat::A4,
    };
    Page {
        format,
        orientation,
        size: None,
    }
}

fn port_shape(word: Option<&str>) -> PortShape {
    match word {
        Some("input") => PortShape::Input,
        Some("output") => PortShape::Output,
        Some("bidirectional") => PortShape::Bidirectional,
        Some("tri_state") => PortShape::TriState,
        _ => PortShape::Passive,
    }
}

/// The parts of an imported sheet besides the [`Schematic`] itself.
#[derive(Debug, Default)]
pub struct SheetExtras {
    /// Sheets placed on this sheet.
    pub children: Vec<ChildSheet>,
}

/// Placement of a symbol: KiCad mirrors in library space first, then
/// rotates; ElectronicWerk only mirrors x, so a mirror about the x axis
/// becomes a mirror about the y axis plus 180°.
fn orientation(node: &Sexp<'_>) -> (f64, bool) {
    let rotation = node.get("at").and_then(|a| a.num(2)).unwrap_or(0.0);
    match node.value("mirror") {
        Some("y") => (rotation, true),
        Some("x") => (rotation + 180.0, true),
        Some("xy") => (rotation + 180.0, false),
        _ => (rotation, false),
    }
}

/// Character advance relative to the text size (the monospace font of the
/// viewer, see `ew-scene`).
const ADVANCE: f64 = 0.6 / 0.73;

/// Where KiCad draws a symbol field: it lays out the text box around the
/// field position in the unrotated symbol, turns that box with the symbol
/// and draws the text centred in it, horizontal or vertical. Returns the
/// centre and the drawing angle (0 or 90).
fn field_placement(
    position: Point,
    field_angle: f64,
    text: &str,
    style: &TextStyle,
    symbol_angle: f64,
    mirror: bool,
) -> (Point, f64) {
    let size = style.size as f64;
    let width = text.chars().count() as f64 * ADVANCE * size;
    let dx = match style.h_align {
        HAlign::Left => width / 2.0,
        HAlign::Center => 0.0,
        HAlign::Right => -width / 2.0,
    };
    let dy = match style.v_align {
        VAlign::Bottom => size / 2.0,
        VAlign::Center => 0.0,
        VAlign::Top => -size / 2.0,
    };
    let (sin, cos) = field_angle.to_radians().sin_cos();
    // KiCad applies the symbol transform (which maps library coordinates,
    // y up) to the box in sheet coordinates, where y points down: so the
    // box offset is flipped in y before it is transformed.
    let (fx, fy) = (dx * cos - dy * sin, -(dx * sin + dy * cos));
    let fx = if mirror { -fx } else { fx };
    let (sin, cos) = symbol_angle.to_radians().sin_cos();
    let offset = (fx * cos - fy * sin, fx * sin + fy * cos);
    let quarters = ((field_angle + symbol_angle) / 90.0).round() as i64;
    let turn = if quarters.rem_euclid(2) == 1 {
        90.0
    } else {
        0.0
    };
    (
        Point::new(
            position.x + offset.0.round() as i64,
            position.y + offset.1.round() as i64,
        ),
        turn,
    )
}

/// The instance paths of a placed symbol: `(path, reference, unit)`.
fn instances(node: &Sexp<'_>) -> Vec<(String, String, u16)> {
    let mut out = Vec::new();
    if let Some(list) = node.get("instances") {
        for project in list.all("project") {
            for path in project.all("path") {
                out.push((
                    path.arg(0).unwrap_or_default().to_owned(),
                    path.value("reference").unwrap_or_default().to_owned(),
                    path.number("unit").map_or(1, |u| u.max(1.0) as u16),
                ));
            }
        }
    }
    out
}

/// The board path of a symbol instance: the sheet path without the root
/// sheet, followed by the symbol's UUID (as in a footprint's `path`).
#[must_use]
pub fn board_path(instance: &str, symbol_uuid: &str) -> String {
    let sheets: Vec<&str> = instance
        .split('/')
        .filter(|part| !part.is_empty())
        .skip(1)
        .collect();
    let mut path = String::new();
    for sheet in sheets {
        path.push('/');
        path.push_str(sheet);
    }
    path.push('/');
    path.push_str(symbol_uuid);
    path
}

#[allow(clippy::too_many_lines, reason = "one property after the other")]
fn placed_symbol(
    node: &Sexp<'_>,
    frame: Frame,
    ids: &mut Ids,
    table: &SymbolTable,
    legacy_paths: &BTreeMap<String, (String, u16)>,
    report: &mut ImportReport,
    file: &str,
) -> PlacedSymbol {
    let uuid = node.value("uuid").unwrap_or_default().to_owned();
    let lib = node
        .value("lib_name")
        .or_else(|| node.value("lib_id"))
        .unwrap_or_default()
        .to_owned();
    if !table.defs.contains_key(&lib) {
        report.warn(
            "missing-symbol",
            Some(file),
            format!("symbol `{lib}` is not in the symbol cache of the sheet"),
        );
    }
    let (rotation, mirror) = orientation(node);
    let mut designator = String::new();
    let mut value = String::new();
    let mut footprint = None;
    let mut properties = BTreeMap::new();
    let mut fields = Vec::new();
    for property in node.all("property") {
        let key = property.arg(0).unwrap_or_default();
        let text = property.arg(1).unwrap_or_default();
        let field_name = match key {
            "Reference" => {
                text.clone_into(&mut designator);
                "designator".to_owned()
            }
            "Value" => {
                text.clone_into(&mut value);
                "value".to_owned()
            }
            "Footprint" => {
                footprint = (!text.is_empty()).then(|| text.to_owned());
                "footprint".to_owned()
            }
            _ => {
                if !text.is_empty() && text != "~" {
                    properties.insert(key.to_owned(), text.to_owned());
                }
                key.to_owned()
            }
        };
        let at = property.get("at");
        let style = common::text_style(property.get("effects"));
        let (position, turn) = field_placement(
            frame.at(at),
            at.and_then(|a| a.num(2)).unwrap_or(0.0),
            text,
            &style,
            rotation,
            mirror,
        );
        fields.push(FieldLabel {
            name: field_name,
            at: position,
            rotation: angle(turn),
            style: TextStyle {
                h_align: HAlign::Center,
                v_align: VAlign::Center,
                ..style
            },
            visible: !common::hidden(property) && !text.is_empty() && text != "~",
        });
    }
    let mut unit = node.number("unit").map_or(1, |u| u.max(1.0) as u16);
    let mut map = BTreeMap::new();
    let mut component = None;
    let list = instances(node);
    for (index, (path, reference, instance_unit)) in list.iter().enumerate() {
        map.insert(path.clone(), reference.clone());
        if index == 0 {
            component = Some(component_id(&board_path(path, &uuid)));
            if !reference.is_empty() {
                reference.clone_into(&mut designator);
            }
            unit = *instance_unit;
        }
    }
    if list.is_empty() {
        // KiCad 6 keeps the instances in the root sheet (`symbol_instances`).
        if let Some((path, (reference, instance_unit))) = legacy_paths
            .iter()
            .find(|(path, _)| path.ends_with(&format!("/{uuid}")))
        {
            component = Some(component_id(path));
            if !reference.is_empty() {
                reference.clone_into(&mut designator);
            }
            unit = *instance_unit;
        }
    }
    if list.len() > 1 {
        report.info(
            "reused-sheet",
            Some(file),
            "a sheet used more than once keeps one designator per use; the board link uses the first",
        );
    }
    PlacedSymbol {
        id: ids.take(common::parse_uuid(&uuid), &["symbol", file, &designator]),
        symbol: lib,
        unit,
        at: frame.at(node.get("at")),
        rotation: angle(rotation),
        mirror,
        designator,
        value,
        footprint,
        component,
        properties,
        fields,
        flags: ComponentFlags {
            dnp: node.flag("dnp"),
            exclude_from_bom: node.value("in_bom") == Some("no"),
            exclude_from_board: node.value("on_board") == Some("no"),
        },
        instances: if map.len() > 1 { map } else { BTreeMap::new() },
    }
}

fn sheet_symbol(
    node: &Sexp<'_>,
    frame: Frame,
    ids: &mut Ids,
    file: &str,
) -> (SheetSymbol, String, String) {
    let at = node.get("at");
    let (sheet_x, sheet_y) = (
        at.and_then(|a| a.num(0)).unwrap_or(0.0),
        at.and_then(|a| a.num(1)).unwrap_or(0.0),
    );
    let size_node = node.get("size");
    let (width, height) = (
        size_node.and_then(|s| s.num(0)).unwrap_or(25.0),
        size_node.and_then(|s| s.num(1)).unwrap_or(15.0),
    );
    let property = |key: &str| -> String {
        node.all("property")
            .find(|p| p.arg(0) == Some(key))
            .and_then(|p| p.arg(1))
            .unwrap_or_default()
            .to_owned()
    };
    // KiCad 6 used `Sheet name` / `Sheet file`.
    let name = {
        let n = property("Sheetname");
        if n.is_empty() {
            property("Sheet name")
        } else {
            n
        }
    };
    let target = {
        let f = property("Sheetfile");
        if f.is_empty() {
            property("Sheet file")
        } else {
            f
        }
    };
    let uuid = node.value("uuid").unwrap_or_default().to_owned();
    let pins = node
        .all("pin")
        .map(|pin| {
            let pin_at = pin.get("at");
            let (pin_left, pin_top, pin_turn) = (
                pin_at.and_then(|a| a.num(0)).unwrap_or(sheet_x),
                pin_at.and_then(|a| a.num(1)).unwrap_or(sheet_y),
                pin_at.and_then(|a| a.num(2)).unwrap_or(180.0),
            );
            let side = match (pin_turn.rem_euclid(360.0) / 90.0).round() as i64 % 4 {
                0 => SheetSide::Right,
                1 => SheetSide::Top,
                3 => SheetSide::Bottom,
                _ => SheetSide::Left,
            };
            let offset = match side {
                SheetSide::Left | SheetSide::Right => nm(pin_top - sheet_y),
                SheetSide::Top | SheetSide::Bottom => nm(pin_left - sheet_x),
            };
            let pin_name = pin.arg(0).unwrap_or_default().to_owned();
            SheetPin {
                id: ids.of(pin, &["sheet-pin", file, &pin_name]),
                name: pin_name,
                shape: port_shape(pin.arg(1)),
                side,
                offset,
            }
        })
        .collect();
    let symbol = SheetSymbol {
        id: ids.take(common::parse_uuid(&uuid), &["sheet", file, &name]),
        name,
        schematic: Id::NIL,
        at: frame.point(sheet_x, sheet_y),
        size: Size::new(nm(width), nm(height)),
        pins,
    };
    (symbol, target, uuid)
}

/// Reads a sheet. `legacy_paths` are the KiCad 6 symbol instances of the
/// root sheet (path → reference, unit).
#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "one item kind after the other; the import state is passed explicitly"
)]
pub fn sheet(
    node: &Sexp<'_>,
    id: Id,
    name: &str,
    ids: &mut Ids,
    table: &mut SymbolTable,
    legacy_paths: &BTreeMap<String, (String, u16)>,
    report: &mut ImportReport,
    file: &str,
) -> (Schematic, SheetExtras) {
    if let Some(version) = node.value("version") {
        report
            .versions
            .entry("schematic".to_owned())
            .or_insert_with(|| version.to_owned());
    }
    table.add_cache(node, report, file);
    let page = page(node.get("paper"));
    let frame = Frame {
        height: page.dimensions().h,
    };
    let mut out = Schematic::new(id, name);
    out.page = page;
    if let Some(tb) = node.get("title_block") {
        out.title_block = TitleBlock {
            title: tb
                .value("title")
                .map_or_else(|| TitleBlock::default().title, str::to_owned),
            date: tb
                .value("date")
                .map_or_else(|| TitleBlock::default().date, str::to_owned),
            revision: tb
                .value("rev")
                .map_or_else(|| TitleBlock::default().revision, str::to_owned),
            company: tb.value("company").unwrap_or_default().to_owned(),
            comments: tb
                .all("comment")
                .filter_map(|c| c.arg(1))
                .filter(|c| !c.is_empty())
                .map(str::to_owned)
                .collect(),
        };
    }
    let mut extras = SheetExtras::default();
    let xy = |x: f64, y: f64| frame.point(x, y);
    for item in node.args() {
        let Some(head) = item.head() else {
            continue;
        };
        match head {
            "symbol" => {
                let symbol = placed_symbol(item, frame, ids, table, legacy_paths, report, file);
                out.symbols.insert(symbol);
            }
            "wire" | "bus" => {
                let points = common::points(item.get("pts"), &xy);
                for pair in points.windows(2) {
                    let wire_id = ids.of(item, &[head, file]);
                    if head == "wire" {
                        out.wires.insert(Wire {
                            id: wire_id,
                            start: pair[0],
                            end: pair[1],
                        });
                    } else {
                        out.buses.insert(Bus {
                            id: wire_id,
                            start: pair[0],
                            end: pair[1],
                        });
                    }
                }
            }
            "bus_entry" => {
                let size = item.get("size");
                out.bus_entries.insert(BusEntry {
                    id: ids.of(item, &["bus-entry", file]),
                    at: frame.at(item.get("at")),
                    size: Point::new(
                        size.and_then(|s| s.num(0)).map_or(0, nm),
                        -size.and_then(|s| s.num(1)).map_or(0, nm),
                    ),
                });
            }
            "junction" => {
                out.junctions.insert(Junction {
                    id: ids.of(item, &["junction", file]),
                    at: frame.at(item.get("at")),
                });
            }
            "no_connect" => {
                out.no_connects.insert(NoConnect {
                    id: ids.of(item, &["no-connect", file]),
                    at: frame.at(item.get("at")),
                });
            }
            "label" | "global_label" | "hierarchical_label" => {
                let at = item.get("at");
                out.labels.insert(Label {
                    id: ids.of(item, &["label", file]),
                    kind: match head {
                        "global_label" => LabelKind::Global,
                        "hierarchical_label" => LabelKind::Hierarchical,
                        _ => LabelKind::Local,
                    },
                    text: item.arg(0).unwrap_or_default().to_owned(),
                    at: frame.at(at),
                    rotation: angle(at.and_then(|a| a.num(2)).unwrap_or(0.0)),
                    shape: port_shape(item.value("shape")),
                    style: common::text_style(item.get("effects")),
                });
            }
            "text" => {
                let at = item.get("at");
                out.texts.insert(Text {
                    id: ids.of(item, &["text", file]),
                    text: item.arg(0).unwrap_or_default().to_owned(),
                    at: frame.at(at),
                    rotation: symbol::text_angle(at),
                    style: common::text_style(item.get("effects")),
                });
            }
            "polyline" | "rectangle" | "circle" | "arc" | "bezier" => {
                if let Some(geometry) = symbol::geometry(item, &xy) {
                    out.shapes.insert(Shape {
                        id: ids.of(item, &["shape", file]),
                        geometry,
                        stroke: symbol::stroke(item),
                        fill: match symbol::fill(item) {
                            Fill::Background => Fill::None,
                            other => other,
                        },
                    });
                }
            }
            "sheet" => {
                let (symbol, target, uuid) = sheet_symbol(item, frame, ids, file);
                extras.children.push(ChildSheet {
                    symbol: symbol.id,
                    file: target,
                    uuid,
                    name: symbol.name.clone(),
                });
                out.sheets.insert(symbol);
            }
            "text_box" | "image" | "netclass_flag" | "directive_label" | "rule_area" | "table" => {
                report.info(
                    "unsupported-item",
                    Some(file),
                    format!("`{head}` items are not imported"),
                );
            }
            _ => {}
        }
    }
    let counts = [
        ("symbols", out.symbols.len()),
        ("wires", out.wires.len()),
        ("labels", out.labels.len()),
        ("junctions", out.junctions.len()),
        (
            "sheet texts and graphics",
            out.texts.len() + out.shapes.len(),
        ),
    ];
    for (kind, n) in counts {
        report.count(kind, n);
    }
    (out, extras)
}

/// The KiCad 6 `symbol_instances` of a root sheet.
#[must_use]
pub fn legacy_instances(root: &Sexp<'_>) -> BTreeMap<String, (String, u16)> {
    let mut out = BTreeMap::new();
    if let Some(list) = root.get("symbol_instances") {
        for path in list.all("path") {
            out.insert(
                path.arg(0).unwrap_or_default().to_owned(),
                (
                    path.value("reference").unwrap_or_default().to_owned(),
                    path.number("unit").map_or(1, |u| u.max(1.0) as u16),
                ),
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sexpr::parse;
    use ew_model::schematic::PageFormat;
    use ew_model::units::{Angle, mm};

    const SHEET: &str = r#"(kicad_sch (version 20260306) (generator "eeschema")
        (uuid "4c4ccc3c-5b3e-5c49-bbce-e93dbd48b458")
        (paper "A4")
        (title_block (title "Demo") (date "2026-01-02") (rev "B") (company "Lab") (comment 1 "first"))
        (lib_symbols
            (symbol "Device:R" (property "Reference" "R" (at 0 0 0))
                (symbol "R_0_1" (rectangle (start -1 -2.5) (end 1 2.5)))
                (symbol "R_1_1" (pin passive line (at 0 3.81 270) (length 1.27) (name "~") (number "1")))))
        (wire (pts (xy 10 20) (xy 30 20)) (stroke (width 0) (type default)) (uuid "a0000000-0000-0000-0000-000000000001"))
        (junction (at 30 20) (diameter 0) (uuid "a0000000-0000-0000-0000-000000000002"))
        (no_connect (at 40 20) (uuid "a0000000-0000-0000-0000-000000000003"))
        (label "SDA" (at 10 20 0) (effects (font (size 1.27 1.27)) (justify left bottom)) (uuid "a0000000-0000-0000-0000-000000000004"))
        (global_label "VBUS" (shape input) (at 30 20 180) (effects (font (size 1.27 1.27))) (uuid "a0000000-0000-0000-0000-000000000005"))
        (symbol (lib_id "Device:R") (at 50 60 90) (mirror x) (unit 1) (in_bom yes) (on_board yes) (dnp no)
            (uuid "b0000000-0000-0000-0000-000000000001")
            (property "Reference" "R1" (at 52 58 90) (effects (font (size 1.27 1.27))))
            (property "Value" "10k" (at 52 62 90) (effects (font (size 1.27 1.27))))
            (property "Footprint" "Resistor_SMD:R_0603" (at 50 60 0) (effects (font (size 1.27 1.27)) (hide yes)))
            (property "MPN" "RC0603" (at 50 60 0) (effects (font (size 1.27 1.27)) (hide yes)))
            (instances (project "demo" (path "/4c4ccc3c-5b3e-5c49-bbce-e93dbd48b458" (reference "R1") (unit 1)))))
        (sheet (at 100 50) (size 30 20) (uuid "c0000000-0000-0000-0000-000000000001")
            (property "Sheetname" "Power" (at 100 49 0))
            (property "Sheetfile" "power.kicad_sch" (at 100 71 0))
            (pin "VIN" input (at 100 55 180) (uuid "c0000000-0000-0000-0000-000000000002")))
        (text "Note" (at 20 100 0) (effects (font (size 2 2) bold) (justify left bottom)) (uuid "a0000000-0000-0000-0000-000000000006"))
        (rectangle (start 10 90) (end 60 110) (stroke (width 0.2) (type dash)) (fill (type none)) (uuid "a0000000-0000-0000-0000-000000000007"))
        (image (at 0 0)))"#;

    #[test]
    fn imports_a_sheet() {
        let node = parse(SHEET).unwrap();
        let mut ids = Ids::default();
        let mut table = SymbolTable::default();
        let mut report = ImportReport::default();
        let (sheet, extras) = sheet(
            &node,
            Id::from_u128(9),
            "Root",
            &mut ids,
            &mut table,
            &BTreeMap::new(),
            &mut report,
            "demo.kicad_sch",
        );
        assert_eq!(sheet.page.format, PageFormat::A4);
        assert_eq!(sheet.title_block.revision, "B");
        assert_eq!(sheet.title_block.comments, vec!["first".to_owned()]);
        assert!(table.defs.contains_key("Device:R"));
        // y is flipped on the A4 page (210 mm high).
        let wire = sheet.wires.iter().next().unwrap();
        assert_eq!(
            (wire.start, wire.end),
            (Point::new(mm(10), mm(190)), Point::new(mm(30), mm(190)))
        );
        assert_eq!(sheet.junctions.len(), 1);
        assert_eq!(sheet.no_connects.len(), 1);
        let global = sheet
            .labels
            .iter()
            .find(|l| l.kind == LabelKind::Global)
            .unwrap();
        assert_eq!(
            (global.shape, global.rotation),
            (PortShape::Input, Angle::DEG_180)
        );
        let symbol = sheet.symbols.iter().next().unwrap();
        assert_eq!(symbol.designator, "R1");
        assert_eq!(symbol.value, "10k");
        assert_eq!(symbol.footprint.as_deref(), Some("Resistor_SMD:R_0603"));
        assert_eq!(
            symbol.properties.get("MPN").map(String::as_str),
            Some("RC0603")
        );
        assert_eq!(
            (symbol.rotation, symbol.mirror),
            (Angle::DEG_270, true),
            "mirror x = mirror y + 180°"
        );
        assert_eq!(symbol.at, Point::new(mm(50), mm(150)));
        assert_eq!(
            symbol.component,
            Some(component_id("/b0000000-0000-0000-0000-000000000001"))
        );
        assert!(
            symbol
                .fields
                .iter()
                .any(|f| f.name == "designator" && f.visible)
        );
        assert!(
            symbol
                .fields
                .iter()
                .any(|f| f.name == "footprint" && !f.visible)
        );
        let sheet_symbol = sheet.sheets.iter().next().unwrap();
        assert_eq!(sheet_symbol.name, "Power");
        assert_eq!(sheet_symbol.at, Point::new(mm(100), mm(160)));
        assert_eq!(sheet_symbol.pins[0].side, SheetSide::Left);
        assert_eq!(sheet_symbol.pins[0].offset, mm(5));
        assert_eq!(extras.children[0].file, "power.kicad_sch");
        assert_eq!(sheet.texts.len(), 1);
        assert_eq!(sheet.shapes.len(), 1);
        assert!(report.findings.iter().any(|f| f.message.contains("image")));
    }

    #[test]
    fn board_paths_skip_the_root_sheet() {
        assert_eq!(board_path("/root", "sym"), "/sym");
        assert_eq!(board_path("/root/a/b", "sym"), "/a/b/sym");
    }
}
