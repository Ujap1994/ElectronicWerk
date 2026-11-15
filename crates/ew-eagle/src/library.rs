//! Eagle libraries (inside schematics and boards, or `.lbr` files):
//! packages become footprints, symbols and device sets become symbols —
//! one per device, its pins numbered with the pads of the device.

use std::collections::BTreeMap;
use std::path::Path;

use ew_kicad::{ImportReport, KicadError};
use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, LayerSpec, Side};
use ew_model::library::{
    FootprintDef, Pin, PinDirection, PinKind, PinShape, PinText, SymbolDef, SymbolGraphic,
    SymbolText, SymbolUnit,
};
use ew_model::pcb::{
    Drill, FootprintAttributes, FootprintText, FootprintTextKind, Graphic, Mounting, Pad, PadKind,
    PadShape,
};
use ew_model::shape::{Fill, Geometry, Stroke};
use ew_model::text::TextStyle;
use ew_model::units::{Angle, Nm, Point, Size};
use roxmltree::Node;

use crate::util::{
    Rot, attr, auto_diameter, board_layer, child, children, flag, nm, num, opt, point, text_style,
    vertices, wire_geometry,
};

/// A `>NAME` or `>VALUE` text of a symbol: where the field goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FieldText {
    /// Anchor in symbol coordinates.
    pub at: Point,
    /// Angle of the text (degrees).
    pub angle: f64,
    /// Style.
    pub style: TextStyle,
}

/// One device of a device set: its symbol and package.
#[derive(Debug, Clone)]
pub struct Device {
    /// The symbol definition (one unit per gate).
    pub def: SymbolDef,
    /// Footprint name (`library:package`), if the device has a package.
    pub footprint: Option<String>,
    /// Gate names in unit order.
    pub gates: Vec<String>,
    /// The `>NAME` and `>VALUE` texts of each gate's symbol.
    pub fields: Vec<(Option<FieldText>, Option<FieldText>)>,
    /// The Eagle pin names (`VCC@2`) of each unit, in pin order.
    pub pin_names: Vec<Vec<String>>,
}

/// The content of an Eagle library.
#[derive(Debug, Clone, Default)]
pub struct EagleLibrary {
    /// Library name.
    pub name: String,
    /// Footprints by package name.
    pub packages: BTreeMap<String, FootprintDef>,
    /// Devices by (device set, device).
    pub devices: BTreeMap<(String, String), Device>,
}

/// Strips the HTML of Eagle descriptions down to text.
#[must_use]
pub fn plain_text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn description(node: Node<'_, '_>) -> String {
    child(node, "description")
        .and_then(|d| d.text())
        .map(plain_text)
        .unwrap_or_default()
}

/// The full name of a device: `R` + `0805`, or `?` replaced by the device.
#[must_use]
pub fn device_name(deviceset: &str, device: &str) -> String {
    let base = deviceset.replace('*', "");
    if base.contains('?') {
        base.replace('?', device)
    } else {
        format!("{base}{device}")
    }
}

fn eagle_angle(rot: Rot) -> Angle {
    Angle::from_degrees_f64(rot.angle.rem_euclid(360.0)).unwrap_or(Angle::ZERO)
}

fn octagon(diameter: Nm) -> Vec<Point> {
    let r = diameter as f64 / 2.0 / (std::f64::consts::PI / 8.0).cos();
    (0..8)
        .map(|i| {
            let a = std::f64::consts::PI / 8.0 + f64::from(i) * std::f64::consts::FRAC_PI_4;
            Point::new((r * a.cos()).round() as Nm, (r * a.sin()).round() as Nm)
        })
        .collect()
}

/// A package (in its own coordinates, as seen from the top).
#[allow(clippy::too_many_lines, reason = "one element kind after the other")]
fn package(
    node: Node<'_, '_>,
    library: &str,
    report: &mut ImportReport,
    file: &str,
) -> FootprintDef {
    let name = attr(node, "name").to_owned();
    let full = format!("{library}:{name}");
    let mut def = FootprintDef {
        id: Id::derived(&["eagle-package", &full]),
        name: full.clone(),
        description: description(node),
        keywords: String::new(),
        pads: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        models: Vec::new(),
        attributes: FootprintAttributes::default(),
    };
    let no_inner = |_: u32| None;
    let mut smd_count = 0;
    let mut tht_count = 0;
    for item in node.children().filter(Node::is_element) {
        match item.tag_name().name() {
            "smd" => {
                smd_count += 1;
                let rot = Rot::of(item);
                let (dx, dy) = (nm(num(item, "dx")), nm(num(item, "dy")));
                let roundness = num(item, "roundness").clamp(0.0, 100.0);
                let shape = if roundness >= 100.0 {
                    if dx == dy {
                        PadShape::Circle
                    } else {
                        PadShape::Oval
                    }
                } else if roundness > 0.0 {
                    PadShape::RoundRect {
                        ratio: (roundness * 5.0).round() as u16,
                    }
                } else {
                    PadShape::Rect
                };
                let side = if attr(item, "layer") == "16" {
                    Side::Bottom
                } else {
                    Side::Top
                };
                let copper = if side == Side::Top {
                    CopperLayer::Top
                } else {
                    CopperLayer::Bottom
                };
                let mut layers = vec![LayerSpec::Layer(Layer::Copper(copper))];
                if flag(item, "stop", true) {
                    layers.push(LayerSpec::Layer(Layer::SolderMask(side)));
                }
                if flag(item, "cream", true) {
                    layers.push(LayerSpec::Layer(Layer::Paste(side)));
                }
                let number = attr(item, "name").to_owned();
                def.pads.push(Pad {
                    id: Id::derived(&["eagle-pad", &full, &number]),
                    number,
                    net: None,
                    kind: PadKind::Smd,
                    shape,
                    at: point(item, "x", "y"),
                    rotation: eagle_angle(rot),
                    size: Size::new(dx, dy),
                    drill: None,
                    layers,
                    clearance: None,
                    mask_margin: None,
                    paste_margin: None,
                });
            }
            "pad" => {
                tht_count += 1;
                let drill = num(item, "drill");
                let diameter = opt(item, "diameter")
                    .filter(|d| *d > 0.0)
                    .unwrap_or_else(|| auto_diameter(drill));
                let d = nm(diameter);
                let rot = Rot::of(item);
                let mut at = point(item, "x", "y");
                let mut offset = Point::ORIGIN;
                let (shape, size) = match attr(item, "shape") {
                    "square" => (PadShape::Rect, Size::new(d, d)),
                    "octagon" => (PadShape::Custom { points: octagon(d) }, Size::new(d, d)),
                    "long" => (PadShape::Oval, Size::new(2 * d, d)),
                    "offset" => {
                        // The oval reaches out to one side of the hole.
                        let shift = rot.apply(Point::new(d / 2, 0));
                        at = Point::new(at.x + shift.x, at.y + shift.y);
                        offset = Point::new(-d / 2, 0);
                        (PadShape::Oval, Size::new(2 * d, d))
                    }
                    _ => (PadShape::Circle, Size::new(d, d)),
                };
                let number = attr(item, "name").to_owned();
                def.pads.push(Pad {
                    id: Id::derived(&["eagle-pad", &full, &number]),
                    number,
                    net: None,
                    kind: PadKind::ThroughHole,
                    shape,
                    at,
                    rotation: eagle_angle(rot),
                    size,
                    drill: Some(Drill {
                        diameter: nm(drill),
                        slot_length: None,
                        offset,
                    }),
                    layers: if flag(item, "stop", true) {
                        vec![LayerSpec::AllCopper, LayerSpec::AllMask]
                    } else {
                        vec![LayerSpec::AllCopper]
                    },
                    clearance: None,
                    mask_margin: None,
                    paste_margin: None,
                });
            }
            "hole" => {
                let d = nm(num(item, "drill"));
                let index = def.pads.len().to_string();
                def.pads.push(Pad {
                    id: Id::derived(&["eagle-hole", &full, &index]),
                    number: String::new(),
                    net: None,
                    kind: PadKind::NpThroughHole,
                    shape: PadShape::Circle,
                    at: point(item, "x", "y"),
                    rotation: Angle::ZERO,
                    size: Size::new(d, d),
                    drill: Some(Drill {
                        diameter: d,
                        slot_length: None,
                        offset: Point::ORIGIN,
                    }),
                    layers: vec![LayerSpec::AllCopper, LayerSpec::AllMask],
                    clearance: None,
                    mask_margin: None,
                    paste_margin: None,
                });
            }
            "wire" | "circle" | "rectangle" | "polygon" => {
                let number: u32 = attr(item, "layer").parse().unwrap_or(0);
                let Some(layer) = board_layer(number, &no_inner) else {
                    report.info(
                        "eagle-layer",
                        Some(file),
                        format!("drawings on Eagle layer {number} are not imported"),
                    );
                    continue;
                };
                if let Some((geometry, width, filled)) = drawing(item) {
                    def.graphics.push(Graphic {
                        layer,
                        geometry,
                        width,
                        filled,
                    });
                }
            }
            "text" => {
                let content = item.text().unwrap_or_default();
                let number: u32 = attr(item, "layer").parse().unwrap_or(0);
                let kind = match content.to_ascii_uppercase().as_str() {
                    ">NAME" => FootprintTextKind::Designator,
                    ">VALUE" => FootprintTextKind::Value,
                    _ => FootprintTextKind::User,
                };
                let Some(layer) = board_layer(number, &no_inner) else {
                    continue;
                };
                let rot = Rot::of(item);
                def.texts.push(FootprintText {
                    kind,
                    text: if kind == FootprintTextKind::User {
                        content.to_owned()
                    } else {
                        String::new()
                    },
                    layer,
                    at: point(item, "x", "y"),
                    rotation: eagle_angle(rot),
                    style: text_style(item),
                    visible: true,
                });
            }
            "description" => {}
            other => report.info(
                "unsupported-item",
                Some(file),
                format!("`{other}` items in packages are not imported"),
            ),
        }
    }
    def.attributes.mounting = if tht_count > 0 {
        Mounting::ThroughHole
    } else if smd_count > 0 {
        Mounting::Smd
    } else {
        Mounting::Other
    };
    def
}

/// A drawn element: geometry, line width and fill.
#[must_use]
pub fn drawing(item: Node<'_, '_>) -> Option<(Geometry, Nm, bool)> {
    let width = nm(num(item, "width"));
    Some(match item.tag_name().name() {
        "wire" => (wire_geometry(item), width, false),
        "circle" => (
            Geometry::Circle {
                center: point(item, "x", "y"),
                radius: nm(num(item, "radius")),
            },
            width,
            width == 0,
        ),
        "rectangle" => {
            let (a, b) = (point(item, "x1", "y1"), point(item, "x2", "y2"));
            let rot = Rot::of(item);
            if rot.angle.abs() < 1e-9 {
                (Geometry::Rect { start: a, end: b }, 0, true)
            } else {
                // A turned rectangle turns about its centre.
                let center = Point::new((a.x + b.x) / 2, (a.y + b.y) / 2);
                let corners = [
                    Point::new(a.x, a.y),
                    Point::new(b.x, a.y),
                    Point::new(b.x, b.y),
                    Point::new(a.x, b.y),
                ];
                let points = corners
                    .iter()
                    .map(|p| {
                        let q = rot.apply(Point::new(p.x - center.x, p.y - center.y));
                        Point::new(q.x + center.x, q.y + center.y)
                    })
                    .collect();
                (Geometry::Polygon { points }, 0, true)
            }
        }
        "polygon" => {
            let points = vertices(item);
            if points.len() < 3 {
                return None;
            }
            (Geometry::Polygon { points }, width, true)
        }
        _ => return None,
    })
}

fn pin_kind(direction: &str) -> PinKind {
    match direction {
        "nc" => PinKind::NoConnect,
        "in" => PinKind::Input,
        "out" => PinKind::Output,
        "io" => PinKind::Bidirectional,
        "oc" => PinKind::OpenCollector,
        "hiz" => PinKind::TriState,
        "pwr" => PinKind::PowerIn,
        "sup" => PinKind::PowerOut,
        _ => PinKind::Passive,
    }
}

fn pin_length(length: &str) -> Nm {
    match length {
        "point" => 0,
        "short" => 2_540_000,
        "middle" => 5_080_000,
        _ => 7_620_000,
    }
}

/// The pins, drawings and texts of a symbol, and its `>NAME`/`>VALUE`
/// texts.
struct SymbolData {
    pins: Vec<(Pin, String)>,
    graphics: Vec<SymbolGraphic>,
    texts: Vec<SymbolText>,
    name: Option<FieldText>,
    value: Option<FieldText>,
}

fn symbol(node: Node<'_, '_>) -> SymbolData {
    let mut out = SymbolData {
        pins: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        name: None,
        value: None,
    };
    for item in node.children().filter(Node::is_element) {
        match item.tag_name().name() {
            "pin" => {
                let rot = Rot::of(item);
                let direction = match (rot.angle.rem_euclid(360.0) / 90.0).round() as i64 % 4 {
                    1 => PinDirection::Up,
                    2 => PinDirection::Left,
                    3 => PinDirection::Down,
                    _ => PinDirection::Right,
                };
                let shape = match attr(item, "function") {
                    "dot" => PinShape::Inverted,
                    "clk" => PinShape::Clock,
                    "dotclk" => PinShape::InvertedClock,
                    _ => PinShape::Line,
                };
                let raw = attr(item, "name");
                // `VCC@2`: several pins of one name; the name shows without it.
                let name = raw.split('@').next().unwrap_or(raw).to_owned();
                out.pins.push((
                    Pin {
                        number: raw.to_owned(),
                        name,
                        kind: pin_kind(attr(item, "direction")),
                        shape,
                        at: point(item, "x", "y"),
                        direction,
                        length: pin_length(item.attribute("length").unwrap_or("long")),
                        hidden: false,
                    },
                    item.attribute("visible").unwrap_or("both").to_owned(),
                ));
            }
            "wire" | "circle" | "rectangle" | "polygon" => {
                if let Some((geometry, width, filled)) = drawing(item) {
                    out.graphics.push(SymbolGraphic {
                        geometry,
                        stroke: Stroke {
                            width,
                            ..Stroke::default()
                        },
                        fill: if filled { Fill::Solid } else { Fill::None },
                    });
                }
            }
            "text" => {
                let content = item.text().unwrap_or_default();
                let field = FieldText {
                    at: point(item, "x", "y"),
                    angle: Rot::of(item).angle,
                    style: text_style(item),
                };
                match content.to_ascii_uppercase().as_str() {
                    ">NAME" => out.name = Some(field),
                    ">VALUE" => out.value = Some(field),
                    _ => out.texts.push(SymbolText {
                        text: content.to_owned(),
                        at: field.at,
                        rotation: Angle::from_degrees_f64(field.angle.rem_euclid(360.0))
                            .unwrap_or(Angle::ZERO),
                        style: field.style,
                    }),
                }
            }
            _ => {}
        }
    }
    out
}

/// Reads an Eagle `<library>` element.
#[allow(
    clippy::too_many_lines,
    reason = "devices are built from several parts"
)]
pub fn read(node: Node<'_, '_>, report: &mut ImportReport, file: &str) -> EagleLibrary {
    let name = node
        .attribute("name")
        .map_or_else(|| "eagle".to_owned(), str::to_owned);
    let mut out = EagleLibrary {
        name: name.clone(),
        ..EagleLibrary::default()
    };
    if let Some(packages) = child(node, "packages") {
        for item in children(packages, "package") {
            let def = package(item, &name, report, file);
            out.packages.insert(attr(item, "name").to_owned(), def);
        }
    }
    let mut symbols: BTreeMap<String, SymbolData> = BTreeMap::new();
    if let Some(list) = child(node, "symbols") {
        for item in children(list, "symbol") {
            symbols.insert(attr(item, "name").to_owned(), symbol(item));
        }
    }
    let Some(devicesets) = child(node, "devicesets") else {
        return out;
    };
    for set in children(devicesets, "deviceset") {
        let set_name = attr(set, "name").to_owned();
        let gates: Vec<(String, String)> = child(set, "gates")
            .map(|g| {
                children(g, "gate")
                    .map(|gate| {
                        (
                            attr(gate, "name").to_owned(),
                            attr(gate, "symbol").to_owned(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let Some(devices) = child(set, "devices") else {
            continue;
        };
        for device in children(devices, "device") {
            let device_name_attr = attr(device, "name").to_owned();
            let full = device_name(&set_name, &device_name_attr);
            let package_name = device.attribute("package").map(str::to_owned);
            // (gate, pin) → pad.
            let mut pads: BTreeMap<(String, String), String> = BTreeMap::new();
            if let Some(connects) = child(device, "connects") {
                for connect in children(connects, "connect") {
                    let pad_names = attr(connect, "pad");
                    let first = pad_names.split_whitespace().next().unwrap_or_default();
                    if pad_names.split_whitespace().count() > 1 {
                        report.info(
                            "eagle-multi-pad",
                            Some(file),
                            format!(
                                "a pin of {full} connects several pads; only the first ({first}) is linked"
                            ),
                        );
                    }
                    pads.insert(
                        (
                            attr(connect, "gate").to_owned(),
                            attr(connect, "pin").to_owned(),
                        ),
                        first.to_owned(),
                    );
                }
            }
            let mut units = Vec::new();
            let mut fields = Vec::new();
            let mut pin_names = Vec::new();
            let mut all_pins: Vec<(PinKind, String)> = Vec::new();
            for (gate, symbol_name) in &gates {
                let Some(data) = symbols.get(symbol_name) else {
                    report.warn(
                        "missing-symbol",
                        Some(file),
                        format!("the symbol {symbol_name} of {set_name} is missing"),
                    );
                    continue;
                };
                let pins = data
                    .pins
                    .iter()
                    .map(|(pin, visible)| {
                        all_pins.push((pin.kind, visible.clone()));
                        let number = pads
                            .get(&(gate.clone(), pin.number.clone()))
                            .cloned()
                            .unwrap_or_else(|| pin.name.clone());
                        Pin {
                            number,
                            ..pin.clone()
                        }
                    })
                    .collect();
                pin_names.push(
                    data.pins
                        .iter()
                        .map(|(pin, _)| pin.number.clone())
                        .collect(),
                );
                units.push(SymbolUnit {
                    name: if gates.len() > 1 {
                        gate.clone()
                    } else {
                        String::new()
                    },
                    pins,
                    graphics: data.graphics.clone(),
                    texts: data.texts.clone(),
                    alternate: None,
                });
                fields.push((data.name, data.value));
            }
            let power =
                !all_pins.is_empty() && all_pins.iter().all(|(k, _)| *k == PinKind::PowerOut);
            let hide = |words: &[&str]| {
                !all_pins.is_empty() && all_pins.iter().all(|(_, v)| words.contains(&v.as_str()))
            };
            let footprint = package_name.as_ref().map(|p| format!("{name}:{p}"));
            let def = SymbolDef {
                id: Id::derived(&["eagle-device", &name, &full]),
                name: format!("{name}:{full}"),
                description: description(set),
                keywords: String::new(),
                prefix: set.attribute("prefix").unwrap_or_default().to_owned(),
                value: if flag(set, "uservalue", false) {
                    String::new()
                } else {
                    full.clone()
                },
                footprint: footprint.clone(),
                datasheet: String::new(),
                power,
                units,
                properties: BTreeMap::new(),
                pin_text: PinText {
                    hide_names: hide(&["off", "pad"]),
                    hide_numbers: hide(&["off", "pin"]),
                    name_offset: 0,
                },
            };
            out.devices.insert(
                (set_name.clone(), device_name_attr),
                Device {
                    def,
                    footprint,
                    gates: gates.iter().map(|(g, _)| g.clone()).collect(),
                    fields,
                    pin_names,
                },
            );
        }
    }
    out
}

/// Reads an Eagle library file (`.lbr`) into a project library: its
/// footprints and one symbol per device.
///
/// # Errors
///
/// Fails if the file cannot be read or is not an Eagle library.
pub fn import(
    path: &Path,
    report: &mut ImportReport,
) -> Result<ew_kicad::library::Library, KicadError> {
    let text = std::fs::read_to_string(path).map_err(|source| KicadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let document = crate::parse(path, &text)?;
    let file = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let node =
        crate::util::path(document.root_element(), &["drawing", "library"]).ok_or_else(|| {
            KicadError::Unsupported {
                path: path.to_path_buf(),
                message: "not an Eagle library (no <library>)".to_owned(),
            }
        })?;
    let eagle = read(node, report, &file);
    let library = ew_kicad::library::Library {
        symbols: eagle
            .devices
            .into_values()
            .map(|device| device.def)
            .collect(),
        footprints: eagle.packages.into_values().collect(),
    };
    report.count("symbols", library.symbols.len());
    report.count("footprints", library.footprints.len());
    Ok(library)
}
