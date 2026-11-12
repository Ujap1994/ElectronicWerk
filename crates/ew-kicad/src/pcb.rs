//! KiCad boards (`.kicad_pcb`).

use std::collections::{BTreeMap, HashMap};

use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, Side};
use ew_model::library::FootprintDef;
use ew_model::pcb::{
    BoardShape, BoardText, DielectricKind, FilledArea, FootprintInstance, Keepout, Net, Pcb,
    Stackup, StackupLayer, Track, Via, Zone, ZoneFill,
};
use ew_model::text::TextStyle;
use ew_model::units::{Nm, Point, mm};

use crate::common::{self, Frame, Ids, angle, derived_id, nm};
use crate::footprint::{self, NetRef};
use crate::report::ImportReport;
use crate::sexpr::Sexp;

/// Page height of a KiCad paper size (landscape unless `portrait`).
#[must_use]
pub fn page_height(paper: Option<&Sexp<'_>>) -> Nm {
    let Some(paper) = paper else {
        return mm(210);
    };
    let portrait = paper.has_word("portrait");
    let (w, h) = match paper.arg(0).unwrap_or("A4") {
        "A5" => (210.0, 148.0),
        "A3" => (420.0, 297.0),
        "A2" => (594.0, 420.0),
        "A1" => (841.0, 594.0),
        "A0" => (1189.0, 841.0),
        "A" | "USLetter" => (279.4, 215.9),
        "B" | "USLedger" => (431.8, 279.4),
        "C" => (558.8, 431.8),
        "D" => (863.6, 558.8),
        "E" => (1117.6, 863.6),
        "USLegal" => (355.6, 215.9),
        "User" => (paper.num(1).unwrap_or(297.0), paper.num(2).unwrap_or(210.0)),
        _ => (297.0, 210.0),
    };
    if portrait && paper.arg(0) != Some("User") {
        nm(w)
    } else {
        nm(h)
    }
}

/// The result of a board import.
#[derive(Debug, Clone)]
pub struct Board {
    /// The board.
    pub pcb: Pcb,
    /// One definition per footprint name (as seen from the top), for the
    /// project library.
    pub footprints: Vec<FootprintDef>,
}

/// Id of the component behind a KiCad symbol path (shared by the symbol
/// and its footprint).
#[must_use]
pub fn component_id(path: &str) -> Id {
    derived_id(&["component", path])
}

fn stackup(
    node: &Sexp<'_>,
    copper: &[CopperLayer],
    names: &HashMap<CopperLayer, String>,
) -> Stackup {
    let thickness = node
        .get("general")
        .and_then(|g| g.number("thickness"))
        .map_or(mm(1) + mm(6) / 10, nm);
    let Some(layers) = node.get("setup").and_then(|s| s.get("stackup")) else {
        let mut standard = Stackup::standard(u8::try_from(copper.len()).unwrap_or(2), thickness);
        for layer in &mut standard.layers {
            if let StackupLayer::Copper { layer, name, .. } = layer {
                if let Some(custom) = names.get(layer) {
                    custom.clone_into(name);
                }
            }
        }
        return standard;
    };
    let mut out = Vec::new();
    for item in layers.all("layer") {
        let name = item.arg(0).unwrap_or_default();
        let kind = item.value("type").unwrap_or_default().to_ascii_lowercase();
        let thick = item.number("thickness").map_or(0, nm);
        let color = item.value("color").unwrap_or_default().to_owned();
        let side = if name.starts_with("B.") {
            Side::Bottom
        } else {
            Side::Top
        };
        if let Some(layer) = common::copper(name) {
            out.push(StackupLayer::Copper {
                layer,
                name: names.get(&layer).cloned().unwrap_or_default(),
                thickness: thick,
            });
        } else if kind.contains("silk") {
            out.push(StackupLayer::Silkscreen { side, color });
        } else if kind.contains("solder mask") {
            out.push(StackupLayer::SolderMask {
                side,
                thickness: thick,
                color,
            });
        } else if kind == "core" || kind == "prepreg" {
            out.push(StackupLayer::Dielectric {
                kind: if kind == "core" {
                    DielectricKind::Core
                } else {
                    DielectricKind::Prepreg
                },
                thickness: thick,
                material: item.value("material").unwrap_or("FR4").to_owned(),
                epsilon_r: item.number("epsilon_r").unwrap_or(4.5),
                loss_tangent: item.number("loss_tangent").unwrap_or(0.02),
            });
        }
    }
    let ok = out
        .iter()
        .filter_map(|layer| match layer {
            StackupLayer::Copper { layer, .. } => Some(*layer),
            _ => None,
        })
        .eq(copper.iter().copied());
    if !ok {
        return Stackup::standard(u8::try_from(copper.len()).unwrap_or(2), thickness);
    }
    Stackup {
        layers: out,
        finish: layers.value("copper_finish").unwrap_or_default().to_owned(),
    }
}

struct Nets {
    by_name: BTreeMap<String, Id>,
    by_code: HashMap<u32, String>,
}

impl Nets {
    fn resolve(&mut self, net: Option<NetRef>) -> Option<Id> {
        let name = match net? {
            NetRef::Name(name) => name,
            NetRef::Code(code) => self.by_code.get(&code)?.clone(),
        };
        if name.is_empty() {
            return None;
        }
        Some(
            *self
                .by_name
                .entry(name.clone())
                .or_insert_with(|| derived_id(&["net", &name])),
        )
    }
}

fn copper_layers_of(node: &Sexp<'_>) -> (Vec<CopperLayer>, HashMap<CopperLayer, String>) {
    let mut copper = Vec::new();
    let mut names = HashMap::new();
    if let Some(layers) = node.get("layers") {
        for entry in layers.args() {
            let name = entry.arg(0).unwrap_or_default();
            if let Some(layer) = common::copper(name) {
                copper.push(layer);
                if let Some(user) = entry.arg(2) {
                    if user != name && !user.is_empty() {
                        names.insert(layer, user.to_owned());
                    }
                }
            }
        }
    }
    let count = u8::try_from(copper.len()).unwrap_or(2).max(1);
    (CopperLayer::stack(count), names)
}

fn width(node: &Sexp<'_>) -> Nm {
    node.get("stroke")
        .and_then(|s| s.number("width"))
        .or_else(|| node.number("width"))
        .map_or(0, nm)
}

fn zone(
    node: &Sexp<'_>,
    frame: Frame,
    nets: &mut Nets,
    ids: &mut Ids,
    report: &mut ImportReport,
    file: &str,
) -> Option<Zone> {
    let mut layers: Vec<CopperLayer> = Vec::new();
    let mut names: Vec<&str> = node.value("layer").into_iter().collect();
    if let Some(list) = node.get("layers") {
        names.extend(list.args().iter().filter_map(Sexp::text));
    }
    for name in names {
        match name {
            "*.Cu" | "F&B.Cu" => {
                layers.extend([CopperLayer::Top, CopperLayer::Bottom]);
            }
            _ => match common::copper(name) {
                Some(layer) => layers.push(layer),
                None => report.info(
                    "zone-layer",
                    Some(file),
                    format!("zones and rule areas on `{name}` are not imported"),
                ),
            },
        }
    }
    layers.dedup();
    if layers.is_empty() {
        return None;
    }
    let xy = |x: f64, y: f64| frame.point(x, y);
    let mut polygons = node
        .all("polygon")
        .map(|p| common::points(p.get("pts"), &xy));
    let outline = polygons.next().unwrap_or_default();
    let holes: Vec<Vec<Point>> = polygons.collect();
    let keepout = node.get("keepout").map(|k| {
        let not = |name: &str| k.value(name) == Some("not_allowed");
        Keepout {
            tracks: not("tracks"),
            vias: not("vias"),
            pads: not("pads"),
            pours: not("copperpour"),
            footprints: not("footprints"),
        }
    });
    let fill_settings = node.get("fill");
    let defaults = ZoneFill::default();
    let filled: Vec<FilledArea> = node
        .all("filled_polygon")
        .filter_map(|area| {
            let layer = area
                .value("layer")
                .and_then(common::copper)
                .or_else(|| layers.first().copied())?;
            let points = common::points(area.get("pts"), &xy);
            (points.len() >= 3).then_some(FilledArea {
                layer,
                points,
                holes: Vec::new(),
            })
        })
        .collect();
    let net_name = node
        .value("net_name")
        .map(|name| NetRef::Name(name.to_owned()));
    let net = nets.resolve(
        footprint::net_ref(node)
            .filter(|n| !matches!(n, NetRef::Code(0)))
            .or(net_name),
    );
    Some(Zone {
        id: ids.of(node, &["zone", node.value("name").unwrap_or_default()]),
        name: node.value("name").unwrap_or_default().to_owned(),
        net,
        layers,
        outline,
        holes,
        priority: node.number("priority").map_or(0, |p| p.max(0.0) as u32),
        keepout,
        fill: ZoneFill {
            clearance: node
                .get("connect_pads")
                .and_then(|c| c.number("clearance"))
                .map_or(defaults.clearance, nm),
            min_width: node.number("min_thickness").map_or(defaults.min_width, nm),
            thermal_gap: fill_settings
                .and_then(|f| f.number("thermal_gap"))
                .map_or(defaults.thermal_gap, nm),
            thermal_spoke_width: fill_settings
                .and_then(|f| f.number("thermal_bridge_width"))
                .map_or(defaults.thermal_spoke_width, nm),
            ..defaults
        },
        filled,
    })
}

/// Reads a board.
#[allow(clippy::too_many_lines, reason = "one item kind after the other")]
pub fn board(
    node: &Sexp<'_>,
    name: &str,
    ids: &mut Ids,
    report: &mut ImportReport,
    file: &str,
) -> Board {
    if let Some(version) = node.value("version") {
        report
            .versions
            .insert("board".to_owned(), version.to_owned());
    }
    let frame = Frame {
        height: page_height(node.get("paper")),
    };
    let (copper, names) = copper_layers_of(node);
    let copper_count = u8::try_from(copper.len()).unwrap_or(2);
    let mut nets = Nets {
        by_name: BTreeMap::new(),
        by_code: HashMap::new(),
    };
    for net in node.all("net") {
        if let (Some(code), Some(name)) = (net.arg(0).and_then(|c| c.parse().ok()), net.arg(1)) {
            nets.by_code.insert(code, name.to_owned());
        }
    }
    let origin = node
        .get("setup")
        .and_then(|s| s.get("aux_axis_origin"))
        .map_or(Point::ORIGIN, |o| {
            frame.point(o.num(0).unwrap_or(0.0), o.num(1).unwrap_or(0.0))
        });
    let mut pcb = Pcb::new(
        ids.take(None, &["board", name]),
        name,
        stackup(node, &copper, &names),
    );
    pcb.origin = origin;
    let mut definitions: BTreeMap<String, FootprintDef> = BTreeMap::new();
    let has_layer = |layer: CopperLayer| copper.contains(&layer);

    for item in node.args() {
        let Some(head) = item.head() else {
            continue;
        };
        match head {
            "footprint" | "module" => {
                let fp = footprint::footprint(item, copper_count, ids, report, file);
                let id = ids.of(item, &["footprint", &fp.designator]);
                let mut pads = fp.pads.clone();
                for (pad, net) in pads.iter_mut().zip(fp.pad_nets.iter()) {
                    pad.net = nets.resolve(net.clone());
                }
                definitions
                    .entry(fp.name.clone())
                    .or_insert_with(|| FootprintDef {
                        id: derived_id(&["footprint-def", &fp.name]),
                        name: fp.name.clone(),
                        description: fp.description.clone(),
                        keywords: fp.keywords.clone(),
                        pads: fp
                            .pads
                            .iter()
                            .enumerate()
                            .map(|(index, pad)| ew_model::pcb::Pad {
                                id: derived_id(&[
                                    "footprint-def",
                                    &fp.name,
                                    "pad",
                                    &index.to_string(),
                                ]),
                                net: None,
                                ..pad.clone()
                            })
                            .collect(),
                        graphics: fp.graphics.clone(),
                        texts: fp.texts.clone(),
                        models: fp.models.clone(),
                        attributes: fp.attributes,
                    });
                if fp.designator.is_empty() {
                    report.info(
                        "no-designator",
                        Some(file),
                        "a footprint without designator was imported",
                    );
                }
                pcb.footprints.insert(FootprintInstance {
                    id,
                    component: fp.path.as_deref().map(component_id),
                    footprint: fp.name,
                    designator: fp.designator,
                    value: fp.value,
                    at: frame.point(fp.at.0, fp.at.1),
                    rotation: angle(fp.at.2),
                    side: if fp.bottom { Side::Bottom } else { Side::Top },
                    locked: fp.locked,
                    pads,
                    graphics: fp.graphics,
                    texts: fp.texts,
                    models: fp.models,
                    properties: fp.properties,
                    attributes: fp.attributes,
                });
            }
            "segment" | "arc" => {
                let Some(layer) = item
                    .value("layer")
                    .and_then(common::copper)
                    .filter(|l| has_layer(*l))
                else {
                    report.warn(
                        "unknown-layer",
                        Some(file),
                        "a track on an unknown layer was skipped",
                    );
                    continue;
                };
                let at = |name: &str| frame.at(item.get(name));
                pcb.tracks.insert(Track {
                    id: ids.of(item, &["track"]),
                    layer,
                    net: nets.resolve(footprint::net_ref(item)),
                    width: item.number("width").map_or(0, nm),
                    start: at("start"),
                    end: at("end"),
                    mid: (head == "arc").then(|| at("mid")),
                    locked: item.flag("locked"),
                });
            }
            "via" => {
                let layers: Vec<CopperLayer> = item
                    .get("layers")
                    .map(|l| {
                        l.args()
                            .iter()
                            .filter_map(Sexp::text)
                            .filter_map(common::copper)
                            .collect()
                    })
                    .unwrap_or_default();
                pcb.vias.insert(Via {
                    id: ids.of(item, &["via"]),
                    at: frame.at(item.get("at")),
                    diameter: item.number("size").map_or(0, nm),
                    drill: item.number("drill").map_or(0, nm),
                    net: nets.resolve(footprint::net_ref(item)),
                    from: layers.first().copied().unwrap_or(CopperLayer::Top),
                    to: layers.last().copied().unwrap_or(CopperLayer::Bottom),
                    micro: item.has_word("micro"),
                    locked: item.flag("locked"),
                });
            }
            "zone" => {
                if let Some(zone) = zone(item, frame, &mut nets, ids, report, file) {
                    pcb.zones.insert(zone);
                }
            }
            "gr_line" | "gr_rect" | "gr_circle" | "gr_arc" | "gr_poly" | "gr_curve" => {
                let Some(layer) = item.value("layer").and_then(common::layer) else {
                    report.warn(
                        "unknown-layer",
                        Some(file),
                        "a graphic on an unknown layer was skipped",
                    );
                    continue;
                };
                if matches!(layer, Layer::Copper(c) if !has_layer(c)) {
                    continue;
                }
                let xy = |x: f64, y: f64| frame.point(x, y);
                if let Some(geometry) = footprint::graphic_geometry(item, &xy) {
                    let filled = item
                        .get("fill")
                        .is_some_and(|f| matches!(f.arg(0), Some("yes" | "solid")));
                    pcb.shapes.insert(BoardShape {
                        id: ids.of(item, &["shape"]),
                        layer,
                        geometry,
                        width: width(item),
                        filled,
                    });
                }
            }
            "gr_text" => {
                let Some(layer) = item.value("layer").and_then(common::layer) else {
                    continue;
                };
                let at = item.get("at");
                let mut style: TextStyle = common::text_style(item.get("effects"));
                if style.thickness == 0 {
                    style.thickness = style.size / 8;
                }
                let mirrored = item
                    .get("effects")
                    .and_then(|e| e.get("justify"))
                    .is_some_and(|j| j.has_word("mirror"));
                pcb.texts.insert(BoardText {
                    id: ids.of(item, &["text"]),
                    layer,
                    text: item.arg(0).unwrap_or_default().to_owned(),
                    at: frame.at(at),
                    rotation: angle(at.and_then(|a| a.num(2)).unwrap_or(0.0)),
                    style,
                    mirrored,
                });
            }
            "dimension" | "gr_text_box" | "target" | "image" | "table" | "generated"
            | "gr_bbox" => {
                report.info(
                    "unsupported-item",
                    Some(file),
                    format!("`{head}` items are not imported"),
                );
            }
            _ => {}
        }
    }
    for (name, id) in &nets.by_name {
        pcb.nets.insert(Net {
            id: *id,
            name: name.clone(),
            class: String::new(),
        });
    }
    let counts = [
        ("footprints", pcb.footprints.len()),
        ("pads", pcb.footprints.iter().map(|fp| fp.pads.len()).sum()),
        ("tracks", pcb.tracks.len()),
        ("vias", pcb.vias.len()),
        ("zones", pcb.zones.len()),
        ("nets", pcb.nets.len()),
        ("board graphics", pcb.shapes.len() + pcb.texts.len()),
    ];
    for (kind, n) in counts {
        report.count(kind, n);
    }
    Board {
        pcb,
        footprints: definitions.into_values().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sexpr::parse;
    use ew_model::units::um;

    const BOARD: &str = r#"(kicad_pcb (version 20260206) (generator "pcbnew")
        (general (thickness 1.6))
        (paper "A4")
        (layers (0 "F.Cu" signal) (4 "In1.Cu" power "GND plane") (6 "In2.Cu" signal) (2 "B.Cu" signal)
                (5 "F.SilkS" user "F.Silkscreen") (25 "Edge.Cuts" user))
        (setup (aux_axis_origin 10 200))
        (footprint "R:R_0603" (layer "F.Cu") (uuid "11111111-1111-1111-1111-111111111111") (at 50 100)
            (property "Reference" "R1" (at 0 -1.5 0) (layer "F.SilkS"))
            (property "Value" "10k" (at 0 1.5 0) (layer "F.Fab"))
            (path "/aaaa/bbbb")
            (pad "1" smd roundrect (at -0.8 0) (size 0.8 0.95) (layers "F.Cu" "F.Mask" "F.Paste") (roundrect_rratio 0.25) (net "GND"))
            (pad "2" smd roundrect (at 0.8 0) (size 0.8 0.95) (layers "F.Cu" "F.Mask" "F.Paste") (roundrect_rratio 0.25) (net "VCC")))
        (segment (start 49.2 100) (end 40 100) (width 0.25) (layer "F.Cu") (net "GND") (uuid "22222222-2222-2222-2222-222222222222"))
        (arc (start 40 100) (mid 38 98) (end 36 100) (width 0.25) (layer "In2.Cu") (net "GND"))
        (via (at 40 100) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu") (net "GND"))
        (zone (net "GND") (layer "In1.Cu") (name "GND") (hatch edge 0.5) (priority 1) (connect_pads (clearance 0.3)) (min_thickness 0.25)
            (fill yes (thermal_gap 0.4) (thermal_bridge_width 0.45))
            (polygon (pts (xy 0 0) (xy 100 0) (xy 100 150) (xy 0 150)))
            (filled_polygon (layer "In1.Cu") (pts (xy 1 1) (xy 99 1) (xy 99 149) (xy 1 149))))
        (gr_rect (start 0 0) (end 100 150) (stroke (width 0.1) (type solid)) (fill no) (layer "Edge.Cuts"))
        (gr_text "Rev A" (at 80 140 0) (layer "F.SilkS") (effects (font (size 1.5 1.5) (thickness 0.3))))
        (dimension (type aligned) (layer "Dwgs.User")))"#;

    #[test]
    fn imports_a_four_layer_board() {
        let node = parse(BOARD).unwrap();
        let mut ids = Ids::default();
        let mut report = ImportReport::default();
        let result = board(&node, "Demo", &mut ids, &mut report, "demo.kicad_pcb");
        let pcb = &result.pcb;
        assert_eq!(pcb.stackup.copper_count(), 4);
        assert!(pcb.stackup.layers.iter().any(|l| matches!(l, StackupLayer::Copper { layer: CopperLayer::Inner(1), name, .. } if name == "GND plane")));
        assert_eq!(pcb.origin, Point::new(mm(10), mm(10)));
        assert_eq!(pcb.nets.len(), 2);
        let gnd = pcb.nets.iter().find(|n| n.name == "GND").unwrap().id;
        let fp = pcb.footprints.iter().next().unwrap();
        assert_eq!(fp.designator, "R1");
        assert_eq!(fp.at, Point::new(mm(50), mm(110)));
        assert_eq!(fp.pads[0].net, Some(gnd));
        assert_eq!(fp.component, Some(component_id("/aaaa/bbbb")));
        assert_eq!(pcb.tracks.len(), 2);
        let arc = pcb.tracks.iter().find(|t| t.mid.is_some()).unwrap();
        assert_eq!(arc.layer, CopperLayer::Inner(2));
        assert_eq!(arc.mid, Some(Point::new(mm(38), mm(112))));
        let via = pcb.vias.iter().next().unwrap();
        assert_eq!(
            (via.diameter, via.drill, via.net),
            (um(800), um(400), Some(gnd))
        );
        let zone = pcb.zones.iter().next().unwrap();
        assert_eq!(zone.layers, vec![CopperLayer::Inner(1)]);
        assert_eq!(zone.priority, 1);
        assert_eq!(zone.fill.clearance, um(300));
        assert_eq!(zone.filled.len(), 1);
        assert_eq!(zone.outline.len(), 4);
        assert_eq!(pcb.shapes.iter().next().unwrap().layer, Layer::Outline);
        assert_eq!(pcb.texts.iter().next().unwrap().text, "Rev A");
        assert_eq!(result.footprints.len(), 1);
        assert_eq!(result.footprints[0].pads[0].net, None);
        assert_eq!(
            report.versions.get("board").map(String::as_str),
            Some("20260206")
        );
        assert_eq!(report.counts.get("tracks"), Some(&2));
        assert!(
            report
                .findings
                .iter()
                .any(|f| f.message.contains("dimension"))
        );
    }

    #[test]
    fn resolves_net_codes_of_older_versions() {
        let text = r#"(kicad_pcb (version 20211014) (layers (0 "F.Cu" signal) (31 "B.Cu" signal))
            (net 0 "") (net 1 "GND")
            (segment (start 0 0) (end 1 0) (width 0.2) (layer "B.Cu") (net 1))
            (segment (start 0 0) (end 1 0) (width 0.2) (layer "B.Cu") (net 0)))"#;
        let node = parse(text).unwrap();
        let mut report = ImportReport::default();
        let result = board(
            &node,
            "Old",
            &mut Ids::default(),
            &mut report,
            "old.kicad_pcb",
        );
        let tracks: Vec<_> = result.pcb.tracks.iter().collect();
        assert_eq!(result.pcb.nets.len(), 1);
        assert_eq!(tracks.iter().filter(|t| t.net.is_some()).count(), 1);
        assert_eq!(result.pcb.stackup.copper_count(), 2);
    }

    #[test]
    fn page_sizes() {
        assert_eq!(page_height(None), mm(210));
        assert_eq!(page_height(parse(r#"(paper "A3")"#).ok().as_ref()), mm(297));
        assert_eq!(
            page_height(parse(r#"(paper "A4" portrait)"#).ok().as_ref()),
            mm(297)
        );
        assert_eq!(
            page_height(parse(r#"(paper "User" 100 80)"#).ok().as_ref()),
            mm(80)
        );
    }
}
