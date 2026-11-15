//! Eagle schematics: one ElectronicWerk sheet per Eagle sheet.
//!
//! Eagle connects what a `<net>` lists, by name; ElectronicWerk connects
//! what touches. The import keeps Eagle's nets: labels sit on their wires,
//! wires are split where a pin meets them in the middle, and every segment
//! of a net that has several segments (or is on several sheets) gets a
//! label with the net's name. Eagle's net names are global, so its labels
//! become global labels.

use std::collections::{BTreeMap, BTreeSet};

use ew_kicad::ImportReport;
use ew_model::id::Id;
use ew_model::schematic::{
    Bus, ComponentFlags, FieldLabel, Junction, Label, LabelKind, Page, PageFormat, PlacedSymbol,
    PortShape, Schematic, Shape, Text, Wire,
};
use ew_model::shape::{Fill, LineStyle, Stroke};
use ew_model::text::{HAlign, TextStyle, VAlign};
use ew_model::units::{Angle, Nm, Point};
use roxmltree::Node;

use crate::library::{EagleLibrary, FieldText};
use crate::util::{Rot, attr, child, children, point, text_style};

/// A part of the schematic (`<part>`).
#[derive(Debug, Clone)]
pub struct Part {
    /// Library name.
    pub library: String,
    /// Device set.
    pub deviceset: String,
    /// Device.
    pub device: String,
    /// Value.
    pub value: String,
    /// Attributes (`MPN`, …).
    pub attributes: BTreeMap<String, String>,
}

/// The id of the component behind an Eagle part (shared by the symbol and
/// its footprint on the board).
#[must_use]
pub fn component_id(part: &str) -> Id {
    Id::derived(&["eagle-part", part])
}

/// Reads the parts of a schematic.
#[must_use]
pub fn parts(node: Node<'_, '_>) -> BTreeMap<String, Part> {
    let mut out = BTreeMap::new();
    if let Some(list) = child(node, "parts") {
        for part in children(list, "part") {
            let attributes = children(part, "attribute")
                .filter(|a| a.attribute("value").is_some())
                .map(|a| (attr(a, "name").to_owned(), attr(a, "value").to_owned()))
                .collect();
            out.insert(
                attr(part, "name").to_owned(),
                Part {
                    library: attr(part, "library").to_owned(),
                    deviceset: attr(part, "deviceset").to_owned(),
                    device: attr(part, "device").to_owned(),
                    value: attr(part, "value").to_owned(),
                    attributes,
                },
            );
        }
    }
    out
}

/// One segment of a net on a sheet.
#[derive(Debug, Clone, Default)]
struct Segment {
    wires: Vec<(Point, Point)>,
    pins: Vec<Point>,
    supply: bool,
}

/// Page sizes (landscape) from small to large.
const PAGES: [(PageFormat, Nm, Nm); 5] = [
    (PageFormat::A4, 297_000_000, 210_000_000),
    (PageFormat::A3, 420_000_000, 297_000_000),
    (PageFormat::A2, 594_000_000, 420_000_000),
    (PageFormat::A1, 841_000_000, 594_000_000),
    (PageFormat::A0, 1_189_000_000, 841_000_000),
];

/// The smallest page that holds `bounds` with a margin, and the shift that
/// moves the drawing onto it (none if it is on the page already).
fn page_for(bounds: Option<(Point, Point)>) -> (Page, Point) {
    let margin = 15_000_000;
    let Some((low, high)) = bounds else {
        return (Page::default(), Point::ORIGIN);
    };
    let (w, h) = (high.x - low.x + 2 * margin, high.y - low.y + 2 * margin);
    let (format, pw, ph) = PAGES
        .iter()
        .copied()
        .find(|(_, pw, ph)| w <= *pw && h <= *ph)
        .unwrap_or(PAGES[4]);
    let page = Page {
        format,
        ..Page::default()
    };
    let fits = low.x >= margin / 2
        && low.y >= margin / 2
        && high.x <= pw - margin / 2
        && high.y <= ph - margin / 2;
    let shift = if fits {
        Point::ORIGIN
    } else {
        Point::new(margin - low.x, margin - low.y)
    };
    (page, shift)
}

fn bounds_of(points: impl IntoIterator<Item = Point>) -> Option<(Point, Point)> {
    let mut out: Option<(Point, Point)> = None;
    for p in points {
        out = Some(out.map_or((p, p), |(a, b)| {
            (
                Point::new(a.x.min(p.x), a.y.min(p.y)),
                Point::new(b.x.max(p.x), b.y.max(p.y)),
            )
        }));
    }
    out
}

/// `true` if `p` lies on the segment `a`–`b` (not at its ends).
fn inside(p: Point, a: Point, b: Point) -> bool {
    if p == a || p == b {
        return false;
    }
    let cross = (b.x - a.x) as f64 * (p.y - a.y) as f64 - (b.y - a.y) as f64 * (p.x - a.x) as f64;
    let len = ((b.x - a.x) as f64).hypot((b.y - a.y) as f64);
    if len == 0.0 || (cross / len).abs() > 1_000.0 {
        return false;
    }
    let dot = (p.x - a.x) as f64 * (b.x - a.x) as f64 + (p.y - a.y) as f64 * (b.y - a.y) as f64;
    dot > 0.0 && dot < len * len
}

/// The point of the segment's wires nearest to `p`.
fn snap(p: Point, wires: &[(Point, Point)]) -> Point {
    let mut best = (f64::MAX, p);
    for &(a, b) in wires {
        let (ax, ay, bx, by) = (a.x as f64, a.y as f64, b.x as f64, b.y as f64);
        let (dx, dy) = (bx - ax, by - ay);
        let len2 = dx.mul_add(dx, dy * dy);
        let t = if len2 == 0.0 {
            0.0
        } else {
            ((p.x as f64 - ax).mul_add(dx, (p.y as f64 - ay) * dy) / len2).clamp(0.0, 1.0)
        };
        let q = Point::new(
            ax.mul_add(1.0, t * dx).round() as Nm,
            ay.mul_add(1.0, t * dy).round() as Nm,
        );
        let d = ((q.x - p.x) as f64).hypot((q.y - p.y) as f64);
        if d < best.0 {
            best = (d, q);
        }
    }
    best.1
}

/// A field (`designator`, `value`) placed from a symbol text or a smashed
/// attribute: centred, upright or turned by 90°.
fn field(
    name: &str,
    text: FieldText,
    origin: Point,
    rot: Rot,
    content: &str,
    smashed: bool,
) -> FieldLabel {
    let (at, angle) = if smashed {
        (text.at, text.angle)
    } else {
        let p = rot.apply(text.at);
        (
            Point::new(origin.x + p.x, origin.y + p.y),
            text.angle + rot.angle,
        )
    };
    let quarter = ((angle / 90.0).round() as i64).rem_euclid(2) == 1;
    let size = text.style.size as f64;
    let width = content.chars().count().max(1) as f64 * 0.6 * size;
    let (dx, dy) = (width / 2.0, size / 2.0);
    let center = if quarter {
        Point::new(at.x - dy.round() as Nm, at.y + dx.round() as Nm)
    } else {
        Point::new(at.x + dx.round() as Nm, at.y + dy.round() as Nm)
    };
    FieldLabel {
        name: name.to_owned(),
        at: center,
        rotation: if quarter { Angle::DEG_90 } else { Angle::ZERO },
        style: TextStyle {
            h_align: HAlign::Center,
            v_align: VAlign::Center,
            ..text.style
        },
        visible: true,
    }
}

/// What the sheets of a schematic need.
pub struct Context<'a> {
    /// Libraries by name.
    pub libraries: &'a BTreeMap<String, EagleLibrary>,
    /// Parts by name.
    pub parts: &'a BTreeMap<String, Part>,
    /// The file name (for the report).
    pub file: &'a str,
}

/// Reads the sheets of a schematic.
#[allow(clippy::too_many_lines, reason = "one item kind after the other")]
pub fn sheets(
    schematic: Node<'_, '_>,
    project: &str,
    context: &Context<'_>,
    report: &mut ImportReport,
) -> Vec<Schematic> {
    let sheet_nodes: Vec<Node<'_, '_>> = child(schematic, "sheets")
        .map(|s| children(s, "sheet").collect())
        .unwrap_or_default();
    let device_of = |part: &Part| {
        context.libraries.get(&part.library).and_then(|lib| {
            lib.devices
                .get(&(part.deviceset.clone(), part.device.clone()))
        })
    };
    // On which sheets is each net, and how many segments does it have?
    let mut net_sheets: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    let mut net_segments: BTreeMap<String, usize> = BTreeMap::new();
    for (index, sheet) in sheet_nodes.iter().enumerate() {
        if let Some(nets) = child(*sheet, "nets") {
            for net in children(nets, "net") {
                let name = attr(net, "name").to_owned();
                net_sheets.entry(name.clone()).or_default().insert(index);
                *net_segments.entry(name).or_default() += children(net, "segment").count();
            }
        }
    }

    let mut out = Vec::new();
    for (index, sheet) in sheet_nodes.iter().enumerate() {
        let number = (index + 1).to_string();
        let title = child(*sheet, "description")
            .and_then(|d| d.text())
            .map(crate::library::plain_text)
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| format!("Sheet {number}"));
        let mut sch = Schematic::new(
            Id::derived(&["eagle-sheet", project, &number]),
            title.clone(),
        );
        sch.title_block.title.clone_from(&title);

        // Where everything lies, to choose the page.
        let mut points: Vec<Point> = Vec::new();
        for node in sheet.descendants().filter(Node::is_element) {
            match node.tag_name().name() {
                "instance" | "junction" | "label" | "text" => points.push(point(node, "x", "y")),
                "wire" => {
                    points.push(point(node, "x1", "y1"));
                    points.push(point(node, "x2", "y2"));
                }
                _ => {}
            }
        }
        let (page, shift) = page_for(bounds_of(points));
        sch.page = page;
        let at = |p: Point| Point::new(p.x + shift.x, p.y + shift.y);

        // Plain drawings and texts.
        if let Some(plain) = child(*sheet, "plain") {
            for item in plain.children().filter(Node::is_element) {
                let id = Id::derived(&[
                    "eagle-plain",
                    project,
                    &number,
                    &item.range().start.to_string(),
                ]);
                match item.tag_name().name() {
                    "text" => {
                        sch.texts.insert(Text {
                            id,
                            text: item.text().unwrap_or_default().to_owned(),
                            at: at(point(item, "x", "y")),
                            rotation: Angle::from_degrees_f64(Rot::of(item).angle.rem_euclid(360.0))
                                .unwrap_or(Angle::ZERO),
                            style: text_style(item),
                        });
                    }
                    "wire" | "circle" | "rectangle" | "polygon" => {
                        if let Some((geometry, width, filled)) = crate::library::drawing(item) {
                            let geometry = crate::board::shifted(&geometry, shift);
                            sch.shapes.insert(Shape {
                                id,
                                geometry,
                                stroke: Stroke {
                                    width,
                                    style: if attr(item, "style").contains("dash") {
                                        LineStyle::Dash
                                    } else {
                                        LineStyle::Solid
                                    },
                                },
                                fill: if filled { Fill::Solid } else { Fill::None },
                            });
                        }
                    }
                    "frame" => report.info(
                        "eagle-frame",
                        Some(context.file),
                        "drawing frames are not imported; sheets get the ElectronicWerk title block",
                    ),
                    other => report.info(
                        "unsupported-item",
                        Some(context.file),
                        format!("`{other}` items on sheets are not imported"),
                    ),
                }
            }
        }

        // Symbols.
        let mut pin_points: BTreeMap<(String, String, String), Point> = BTreeMap::new();
        if let Some(instances) = child(*sheet, "instances") {
            for instance in children(instances, "instance") {
                let name = attr(instance, "part");
                let gate = attr(instance, "gate");
                let Some(part) = context.parts.get(name) else {
                    report.warn(
                        "missing-part",
                        Some(context.file),
                        format!("the part {name} is missing"),
                    );
                    continue;
                };
                let Some(device) = device_of(part) else {
                    report.warn(
                        "missing-symbol",
                        Some(context.file),
                        format!(
                            "the device {}/{} of {name} is missing",
                            part.deviceset, part.device
                        ),
                    );
                    continue;
                };
                let unit = device.gates.iter().position(|g| g == gate).unwrap_or(0);
                let rot = Rot::of(instance);
                let origin = at(point(instance, "x", "y"));
                if let (Some(symbol_unit), Some(names)) =
                    (device.def.units.get(unit), device.pin_names.get(unit))
                {
                    for (pin, raw) in symbol_unit.pins.iter().zip(names) {
                        let p = rot.apply(pin.at);
                        pin_points.insert(
                            (name.to_owned(), gate.to_owned(), raw.clone()),
                            Point::new(origin.x + p.x, origin.y + p.y),
                        );
                    }
                }
                let value = if part.value.is_empty() {
                    device.def.value.clone()
                } else {
                    part.value.clone()
                };
                let mut fields = Vec::new();
                let smashed: BTreeMap<&str, Node<'_, '_>> = children(instance, "attribute")
                    .map(|a| (attr(a, "name"), a))
                    .collect();
                let (name_text, value_text) =
                    device.fields.get(unit).copied().unwrap_or((None, None));
                for (key, text, content) in [
                    ("designator", name_text, name),
                    ("value", value_text, value.as_str()),
                ] {
                    let attribute = smashed.get(if key == "designator" { "NAME" } else { "VALUE" });
                    if let Some(a) = attribute {
                        let text = FieldText {
                            at: at(point(*a, "x", "y")),
                            angle: Rot::of(*a).angle,
                            style: text_style(*a),
                        };
                        let mut label = field(key, text, origin, rot, content, true);
                        label.visible = attr(*a, "display") != "off";
                        fields.push(label);
                    } else if let Some(text) = text {
                        fields.push(field(key, text, origin, rot, content, false));
                    }
                }
                let has_package = device.footprint.is_some();
                sch.symbols.insert(PlacedSymbol {
                    id: Id::derived(&["eagle-instance", project, name, gate]),
                    symbol: device.def.name.clone(),
                    unit: u16::try_from(unit + 1).unwrap_or(1),
                    at: origin,
                    rotation: rot.model_angle(),
                    mirror: rot.mirror,
                    designator: name.to_owned(),
                    value,
                    footprint: device.footprint.clone(),
                    component: has_package.then(|| component_id(name)),
                    properties: part.attributes.clone(),
                    fields,
                    flags: ComponentFlags {
                        dnp: false,
                        exclude_from_bom: !has_package,
                        exclude_from_board: !has_package,
                    },
                    instances: BTreeMap::new(),
                    alternate: false,
                });
            }
        }

        // Nets.
        let mut wire_count = 0_usize;
        let mut label_count = 0_usize;
        if let Some(nets) = child(*sheet, "nets") {
            for net in children(nets, "net") {
                let net_name = attr(net, "name");
                let global = net_sheets.get(net_name).is_some_and(|s| s.len() > 1);
                let segments_total = net_segments.get(net_name).copied().unwrap_or(1);
                let auto = net_name.starts_with("N$");
                for (s_index, segment) in children(net, "segment").enumerate() {
                    let mut seg = Segment::default();
                    for item in segment.children().filter(Node::is_element) {
                        match item.tag_name().name() {
                            "wire" => seg
                                .wires
                                .push((at(point(item, "x1", "y1")), at(point(item, "x2", "y2")))),
                            "junction" => {
                                sch.junctions.insert(Junction {
                                    id: Id::derived(&[
                                        "eagle-junction",
                                        project,
                                        &number,
                                        &item.range().start.to_string(),
                                    ]),
                                    at: at(point(item, "x", "y")),
                                });
                            }
                            "pinref" => {
                                let part_name = attr(item, "part");
                                let key = (
                                    part_name.to_owned(),
                                    attr(item, "gate").to_owned(),
                                    attr(item, "pin").to_owned(),
                                );
                                if let Some(&p) = pin_points.get(&key) {
                                    seg.pins.push(p);
                                }
                                if context
                                    .parts
                                    .get(part_name)
                                    .and_then(&device_of)
                                    .is_some_and(|d| d.def.power)
                                {
                                    seg.supply = true;
                                }
                            }
                            _ => {}
                        }
                    }
                    // Pins in the middle of a wire: split the wire there.
                    for &pin in &seg.pins {
                        if let Some(index) = seg.wires.iter().position(|&(a, b)| inside(pin, a, b))
                        {
                            let (a, b) = seg.wires.remove(index);
                            seg.wires.push((a, pin));
                            seg.wires.push((pin, b));
                        }
                    }
                    for (w_index, &(start, end)) in seg.wires.iter().enumerate() {
                        sch.wires.insert(Wire {
                            id: Id::derived(&[
                                "eagle-wire",
                                project,
                                &number,
                                net_name,
                                &s_index.to_string(),
                                &w_index.to_string(),
                            ]),
                            start,
                            end,
                        });
                        wire_count += 1;
                    }
                    // Eagle's net names are global.
                    let kind = LabelKind::Global;
                    let mut labelled = false;
                    let put = |sch: &mut Schematic,
                               p: Point,
                               style: TextStyle,
                               rotation: Angle,
                               tag: &str| {
                        sch.labels.insert(Label {
                            id: Id::derived(&[
                                "eagle-label",
                                project,
                                &number,
                                net_name,
                                &s_index.to_string(),
                                tag,
                            ]),
                            kind,
                            text: net_name.to_owned(),
                            at: p,
                            rotation,
                            shape: PortShape::Passive,
                            style,
                        });
                    };
                    for (l_index, label) in children(segment, "label").enumerate() {
                        let raw = at(point(label, "x", "y"));
                        let p = if seg.wires.is_empty() {
                            seg.pins.first().copied().unwrap_or(raw)
                        } else {
                            snap(raw, &seg.wires)
                        };
                        let mut style = text_style(label);
                        style.thickness = 0;
                        put(
                            &mut sch,
                            p,
                            style,
                            Angle::from_degrees_f64(Rot::of(label).angle.rem_euclid(360.0))
                                .unwrap_or(Angle::ZERO),
                            &format!("eagle-{l_index}"),
                        );
                        label_count += 1;
                        labelled = true;
                    }
                    let wire_ends: BTreeSet<Point> =
                        seg.wires.iter().flat_map(|&(a, b)| [a, b]).collect();
                    let needs_name = !seg.supply && (segments_total > 1 || global || !auto);
                    if !labelled && needs_name {
                        if let Some(p) = seg
                            .wires
                            .first()
                            .map(|w| w.0)
                            .or_else(|| seg.pins.first().copied())
                        {
                            put(&mut sch, p, TextStyle::schematic(), Angle::ZERO, "name");
                            label_count += 1;
                        }
                    }
                    // Pins no wire reaches: a label with the net's name.
                    for (p_index, &pin) in seg.pins.iter().enumerate() {
                        let touched = wire_ends.contains(&pin);
                        if !touched && (!seg.supply || seg.pins.len() != 1) {
                            put(
                                &mut sch,
                                pin,
                                TextStyle::schematic(),
                                Angle::ZERO,
                                &format!("pin-{p_index}"),
                            );
                            label_count += 1;
                        }
                    }
                }
            }
        }
        if let Some(busses) = child(*sheet, "busses") {
            for bus in children(busses, "bus") {
                for (b_index, wire) in children(bus, "segment")
                    .flat_map(|s| children(s, "wire"))
                    .enumerate()
                {
                    sch.buses.insert(Bus {
                        id: Id::derived(&[
                            "eagle-bus",
                            project,
                            &number,
                            attr(bus, "name"),
                            &b_index.to_string(),
                        ]),
                        start: at(point(wire, "x1", "y1")),
                        end: at(point(wire, "x2", "y2")),
                    });
                }
                report.info(
                    "eagle-bus",
                    Some(context.file),
                    "buses are imported as lines; their member nets connect through labels",
                );
            }
        }
        report.count("wires", wire_count);
        report.count("labels", label_count);
        out.push(sch);
    }
    out
}
