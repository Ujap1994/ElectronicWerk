//! Eagle boards: elements become footprints (mirrored ones on the bottom),
//! signals become nets with tracks, vias and copper pours, the classes
//! become net classes and the design rules the board minimums.

use std::collections::{BTreeMap, BTreeSet};

use ew_kicad::ImportReport;
use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, LayerSpec, Side};
use ew_model::pcb::{
    BoardShape, BoardText, Drill, FootprintAttributes, FootprintInstance, Mounting, Net, Pad,
    PadKind, PadShape, Pcb, Stackup, Track, Via, Zone, ZoneFill,
};
use ew_model::rules::{Constraints, NetClass, Rules};
use ew_model::shape::Geometry;
use ew_model::units::{Angle, Nm, Point, Size};
use roxmltree::Node;

use crate::library::EagleLibrary;
use crate::schematic::component_id;
use crate::util::{
    Rot, attr, auto_diameter, board_layer, child, children, nm, num, opt, point, text_style,
    vertices, wire_geometry,
};

/// Moves a geometry by `offset`.
#[must_use]
pub fn shifted(geometry: &Geometry, offset: Point) -> Geometry {
    let p = |q: &Point| Point::new(q.x + offset.x, q.y + offset.y);
    match geometry {
        Geometry::Line { start, end } => Geometry::Line {
            start: p(start),
            end: p(end),
        },
        Geometry::Rect { start, end } => Geometry::Rect {
            start: p(start),
            end: p(end),
        },
        Geometry::Circle { center, radius } => Geometry::Circle {
            center: p(center),
            radius: *radius,
        },
        Geometry::Arc { start, mid, end } => Geometry::Arc {
            start: p(start),
            mid: p(mid),
            end: p(end),
        },
        Geometry::Polyline { points } => Geometry::Polyline {
            points: points.iter().map(p).collect(),
        },
        Geometry::Polygon { points } => Geometry::Polygon {
            points: points.iter().map(p).collect(),
        },
        Geometry::Bezier {
            start,
            control1,
            control2,
            end,
        } => Geometry::Bezier {
            start: p(start),
            control1: p(control1),
            control2: p(control2),
            end: p(end),
        },
    }
}

/// A length of the design rules (`8mil`, `0.3mm`, `0.012inch`).
fn rule_length(text: &str) -> Option<Nm> {
    let text = text.trim();
    let (value, unit) = text
        .find(|c: char| c.is_ascii_alphabetic())
        .map_or((text, "mm"), |i| (&text[..i], &text[i..]));
    let value: f64 = value.parse().ok()?;
    let mm = match unit {
        "mil" => value * 0.0254,
        "inch" => value * 25.4,
        "mic" => value / 1000.0,
        _ => value,
    };
    Some(nm(mm))
}

/// The imported board and its design rules.
pub struct Board {
    /// The board.
    pub pcb: Pcb,
    /// Net classes and minimums of the board.
    pub rules: Rules,
}

/// Reads an Eagle `<board>` element.
#[allow(clippy::too_many_lines, reason = "one item kind after the other")]
pub fn read(
    board: Node<'_, '_>,
    name: &str,
    libraries: &BTreeMap<String, EagleLibrary>,
    report: &mut ImportReport,
    file: &str,
) -> Board {
    // Copper layers in use: Top, Bottom and the inner ones that carry
    // signals (in Eagle's order).
    let mut inner_used: BTreeSet<u32> = BTreeSet::new();
    for node in board.descendants().filter(Node::is_element) {
        if let Ok(layer) = attr(node, "layer").parse::<u32>() {
            if (2..=15).contains(&layer) {
                inner_used.insert(layer);
            }
        }
        if node.has_tag_name("via") {
            if let Some((a, b)) = attr(node, "extent").split_once('-') {
                for layer in [a, b] {
                    if let Ok(layer) = layer.parse::<u32>() {
                        if (2..=15).contains(&layer) {
                            inner_used.insert(layer);
                        }
                    }
                }
            }
        }
    }
    let inner: Vec<u32> = inner_used.into_iter().collect();
    let inner_of = |number: u32| -> Option<u8> {
        inner
            .iter()
            .position(|&n| n == number)
            .and_then(|i| u8::try_from(i + 1).ok())
    };
    let copper_count = u8::try_from(inner.len() + 2).unwrap_or(2);
    let copper_of = |number: u32| -> Option<CopperLayer> {
        match board_layer(number, &inner_of)? {
            Layer::Copper(c) => Some(c),
            _ => None,
        }
    };
    let mut pcb = Pcb::new(
        Id::derived(&["eagle-board", name]),
        name,
        Stackup::standard(copper_count, 1_600_000),
    );

    // Classes and design rules.
    let mut rules = Rules::new(Id::derived(&["eagle-rules", name]), "Design rules");
    let mut class_names: BTreeMap<String, String> = BTreeMap::new();
    if let Some(classes) = child(board, "classes") {
        let mut list = Vec::new();
        for class in children(classes, "class") {
            let class_name = attr(class, "name");
            let display = if class_name == "default" {
                ew_model::rules::DEFAULT_NET_CLASS.to_owned()
            } else {
                class_name.to_owned()
            };
            class_names.insert(attr(class, "number").to_owned(), display.clone());
            let mut net_class = NetClass::default_class();
            net_class.name = display;
            if let Some(width) = opt(class, "width").filter(|w| *w > 0.0) {
                net_class.track_width = nm(width);
            }
            if let Some(drill) = opt(class, "drill").filter(|d| *d > 0.0) {
                net_class.via_drill = nm(drill);
                net_class.via_diameter = nm(auto_diameter(drill));
            }
            if let Some(clearance) = children(class, "clearance")
                .find(|c| attr(*c, "class") == attr(class, "number"))
                .and_then(|c| opt(c, "value"))
                .or_else(|| opt(class, "clearance"))
                .filter(|c| *c > 0.0)
            {
                net_class.clearance = nm(clearance);
            }
            list.push(net_class);
        }
        if !list.is_empty() {
            list.sort_by_key(|c| c.name != ew_model::rules::DEFAULT_NET_CLASS);
            rules.net_classes = list;
        }
    }
    if let Some(design) = child(board, "designrules") {
        let param = |key: &str| {
            children(design, "param")
                .find(|p| attr(*p, "name") == key)
                .and_then(|p| rule_length(attr(p, "value").split_whitespace().next().unwrap_or("")))
        };
        let defaults = Constraints::default();
        rules.constraints = Constraints {
            min_clearance: param("mdWireWire").unwrap_or(defaults.min_clearance),
            min_track_width: param("msWidth").unwrap_or(defaults.min_track_width),
            min_hole: param("msDrill").unwrap_or(defaults.min_hole),
            min_copper_to_edge: param("mdCopperDimension").unwrap_or(defaults.min_copper_to_edge),
            ..defaults
        };
    }

    // Nets.
    let mut nets: BTreeMap<String, Id> = BTreeMap::new();
    let mut pad_nets: BTreeMap<(String, String), Id> = BTreeMap::new();
    if let Some(signals) = child(board, "signals") {
        for signal in children(signals, "signal") {
            let net_name = attr(signal, "name").to_owned();
            let id = Id::derived(&["eagle-net", name, &net_name]);
            let class = class_names
                .get(attr(signal, "class"))
                .filter(|c| c.as_str() != ew_model::rules::DEFAULT_NET_CLASS)
                .cloned()
                .unwrap_or_default();
            pcb.nets.insert(Net {
                id,
                name: net_name.clone(),
                class,
            });
            nets.insert(net_name, id);
            for contact in children(signal, "contactref") {
                pad_nets.insert(
                    (
                        attr(contact, "element").to_owned(),
                        attr(contact, "pad").to_owned(),
                    ),
                    id,
                );
            }
        }
    }

    // Elements.
    let mut placed = 0;
    if let Some(elements) = child(board, "elements") {
        for element in children(elements, "element") {
            let designator = attr(element, "name").to_owned();
            let library = attr(element, "library");
            let package = attr(element, "package");
            let Some(def) = libraries
                .get(library)
                .and_then(|lib| lib.packages.get(package))
            else {
                report.warn(
                    "missing-footprint",
                    Some(file),
                    format!("the package {library}/{package} of {designator} is missing"),
                );
                continue;
            };
            let rot = Rot::of(element);
            let pads = def
                .pads
                .iter()
                .map(|pad| Pad {
                    id: Id::derived(&[
                        "eagle-element-pad",
                        name,
                        &designator,
                        &pad.number,
                        &pad.id.to_string(),
                    ]),
                    net: pad_nets
                        .get(&(designator.clone(), pad.number.clone()))
                        .copied(),
                    ..pad.clone()
                })
                .collect();
            let properties = children(element, "attribute")
                .filter(|a| {
                    a.attribute("value").is_some() && !matches!(attr(*a, "name"), "NAME" | "VALUE")
                })
                .map(|a| (attr(a, "name").to_owned(), attr(a, "value").to_owned()))
                .collect();
            pcb.footprints.insert(FootprintInstance {
                id: Id::derived(&["eagle-element", name, &designator]),
                component: Some(component_id(&designator)),
                footprint: def.name.clone(),
                designator,
                value: attr(element, "value").to_owned(),
                at: point(element, "x", "y"),
                rotation: rot.model_angle(),
                side: if rot.mirror { Side::Bottom } else { Side::Top },
                locked: attr(element, "locked") == "yes",
                pads,
                graphics: def.graphics.clone(),
                texts: def.texts.clone(),
                models: Vec::new(),
                properties,
                attributes: def.attributes,
            });
            placed += 1;
        }
    }
    report.count("footprints", placed);

    // Plain drawings: outline, texts, holes.
    let mut holes = 0;
    if let Some(plain) = child(board, "plain") {
        for item in plain.children().filter(Node::is_element) {
            let id = Id::derived(&["eagle-board-plain", name, &item.range().start.to_string()]);
            let number: u32 = attr(item, "layer").parse().unwrap_or(0);
            match item.tag_name().name() {
                "wire" | "circle" | "rectangle" | "polygon" => {
                    let Some(layer) = board_layer(number, &inner_of) else {
                        report.info(
                            "eagle-layer",
                            Some(file),
                            format!("drawings on Eagle layer {number} are not imported"),
                        );
                        continue;
                    };
                    if let Some((geometry, width, filled)) = crate::library::drawing(item) {
                        pcb.shapes.insert(BoardShape {
                            id,
                            layer,
                            geometry,
                            width,
                            filled: filled && layer != Layer::Outline,
                        });
                    }
                }
                "text" => {
                    let Some(layer) = board_layer(number, &inner_of) else {
                        continue;
                    };
                    let rot = Rot::of(item);
                    pcb.texts.insert(BoardText {
                        id,
                        layer,
                        text: item.text().unwrap_or_default().to_owned(),
                        at: point(item, "x", "y"),
                        rotation: Angle::from_degrees_f64(rot.angle.rem_euclid(360.0))
                            .unwrap_or(Angle::ZERO),
                        style: text_style(item),
                        mirrored: rot.mirror,
                    });
                }
                "hole" => {
                    holes += 1;
                    let drill = nm(num(item, "drill"));
                    let designator = format!("H{holes}");
                    pcb.footprints.insert(FootprintInstance {
                        id: Id::derived(&["eagle-hole", name, &designator]),
                        component: None,
                        footprint: format!("eagle:HOLE_{}", crate::util::fmt_mm(drill)),
                        designator,
                        value: String::new(),
                        at: point(item, "x", "y"),
                        rotation: Angle::ZERO,
                        side: Side::Top,
                        locked: true,
                        pads: vec![Pad {
                            id: Id::derived(&["eagle-hole-pad", name, &holes.to_string()]),
                            number: String::new(),
                            net: None,
                            kind: PadKind::NpThroughHole,
                            shape: PadShape::Circle,
                            at: Point::ORIGIN,
                            rotation: Angle::ZERO,
                            size: Size::new(drill, drill),
                            drill: Some(Drill {
                                diameter: drill,
                                slot_length: None,
                                offset: Point::ORIGIN,
                            }),
                            layers: vec![LayerSpec::AllCopper, LayerSpec::AllMask],
                            clearance: None,
                            mask_margin: None,
                            paste_margin: None,
                        }],
                        graphics: Vec::new(),
                        texts: Vec::new(),
                        models: Vec::new(),
                        properties: BTreeMap::new(),
                        attributes: FootprintAttributes {
                            mounting: Mounting::Other,
                            exclude_from_bom: true,
                            exclude_from_placement: true,
                            dnp: false,
                        },
                    });
                }
                "dimension" | "frame" => report.info(
                    "unsupported-item",
                    Some(file),
                    format!("`{}` items are not imported", item.tag_name().name()),
                ),
                _ => {}
            }
        }
    }

    // Routing.
    let mut tracks = 0;
    let mut vias = 0;
    let mut zones = 0;
    if let Some(signals) = child(board, "signals") {
        for signal in children(signals, "signal") {
            let net = nets.get(attr(signal, "name")).copied();
            for item in signal.children().filter(Node::is_element) {
                let key = item.range().start.to_string();
                match item.tag_name().name() {
                    "wire" => {
                        let number: u32 = attr(item, "layer").parse().unwrap_or(0);
                        let Some(layer) = copper_of(number) else {
                            // Airwires (layer 19) are what is not routed yet.
                            continue;
                        };
                        let (start, mid, end) = match wire_geometry(item) {
                            Geometry::Arc { start, mid, end } => (start, Some(mid), end),
                            Geometry::Line { start, end } => (start, None, end),
                            _ => continue,
                        };
                        pcb.tracks.insert(Track {
                            id: Id::derived(&["eagle-track", name, &key]),
                            layer,
                            net,
                            width: nm(num(item, "width")),
                            start,
                            end,
                            mid,
                            locked: false,
                        });
                        tracks += 1;
                    }
                    "via" => {
                        let drill = num(item, "drill");
                        let diameter = opt(item, "diameter")
                            .filter(|d| *d > 0.0)
                            .unwrap_or_else(|| auto_diameter(drill));
                        let (from, to) = attr(item, "extent")
                            .split_once('-')
                            .map_or((1, 16), |(a, b)| {
                                (a.parse().unwrap_or(1), b.parse().unwrap_or(16))
                            });
                        pcb.vias.insert(Via {
                            id: Id::derived(&["eagle-via", name, &key]),
                            at: point(item, "x", "y"),
                            diameter: nm(diameter),
                            drill: nm(drill),
                            net,
                            from: copper_of(from).unwrap_or(CopperLayer::Top),
                            to: copper_of(to).unwrap_or(CopperLayer::Bottom),
                            micro: false,
                            locked: false,
                        });
                        vias += 1;
                    }
                    "polygon" => {
                        let number: u32 = attr(item, "layer").parse().unwrap_or(0);
                        let Some(layer) = copper_of(number) else {
                            continue;
                        };
                        let rank = opt(item, "rank").unwrap_or(1.0);
                        let defaults = ZoneFill::default();
                        pcb.zones.insert(Zone {
                            id: Id::derived(&["eagle-zone", name, &key]),
                            name: String::new(),
                            net,
                            layers: vec![layer],
                            outline: vertices(item),
                            holes: Vec::new(),
                            // Eagle's rank 1 fills first.
                            priority: u32::try_from(7 - (rank as i64).clamp(1, 7)).unwrap_or(0),
                            keepout: None,
                            fill: ZoneFill {
                                clearance: opt(item, "isolate")
                                    .filter(|v| *v > 0.0)
                                    .map_or(defaults.clearance, nm),
                                min_width: opt(item, "width")
                                    .filter(|v| *v > 0.0)
                                    .map_or(defaults.min_width, nm),
                                ..defaults
                            },
                            filled: Vec::new(),
                        });
                        zones += 1;
                    }
                    _ => {}
                }
            }
        }
    }
    report.count("tracks", tracks);
    report.count("vias", vias);
    report.count("zones", zones);
    if zones > 0 {
        report.info(
            "eagle-pour",
            Some(file),
            "copper pours are imported unfilled: fill the zones on the board",
        );
    }
    Board { pcb, rules }
}
