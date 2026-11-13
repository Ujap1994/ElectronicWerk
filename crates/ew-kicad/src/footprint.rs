//! KiCad footprints (`.kicad_mod` files and footprints placed on boards).
//!
//! KiCad stores a footprint on the bottom side already flipped; the
//! ElectronicWerk model stores footprints as seen from the top and mirrors
//! them when they are placed on the bottom. So every local coordinate of a
//! bottom footprint is mirrored in x and every layer is flipped here.

use std::collections::BTreeMap;

use ew_model::layer::{Layer, LayerSpec};
use ew_model::pcb::{
    Drill, FootprintAttributes, FootprintText, FootprintTextKind, Graphic, Model3d, Mounting, Pad,
    PadKind, PadShape, Scale3,
};
use ew_model::shape::Geometry;
use ew_model::text::TextStyle;
use ew_model::units::{Angle, Point, Size, Vec3};

use crate::common::{self, Ids, angle, layer_specs, nm};
use crate::report::ImportReport;
use crate::sexpr::Sexp;

/// A net reference: KiCad 10 uses names, older versions net codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetRef {
    /// `(net "GND")` or `(net 3 "GND")`.
    Name(String),
    /// `(net 3)`.
    Code(u32),
}

/// Reads `(net …)` of an item.
#[must_use]
pub fn net_ref(node: &Sexp<'_>) -> Option<NetRef> {
    let net = node.get("net")?;
    match (net.args().first(), net.arg(1)) {
        (_, Some(name)) => Some(NetRef::Name(name.to_owned())),
        (Some(Sexp::Str(name)), None) => Some(NetRef::Name(name.to_string())),
        (Some(Sexp::Atom(code)), None) => code.parse().ok().map(NetRef::Code),
        _ => None,
    }
}

/// A parsed footprint.
#[derive(Debug, Clone)]
pub struct Footprint {
    /// Library link (`Resistor_SMD:R_0603_1608Metric`).
    pub name: String,
    /// Description.
    pub description: String,
    /// Keywords.
    pub keywords: String,
    /// On the bottom side.
    pub bottom: bool,
    /// Position (raw KiCad millimetres, y down) and rotation (degrees).
    pub at: (f64, f64, f64),
    /// `Reference`.
    pub designator: String,
    /// `Value`.
    pub value: String,
    /// Locked.
    pub locked: bool,
    /// Link to the schematic symbol (`/sheet-uuid/…/symbol-uuid`).
    pub path: Option<String>,
    /// Pads as seen from the top.
    pub pads: Vec<Pad>,
    /// The net of each pad.
    pub pad_nets: Vec<Option<NetRef>>,
    /// Graphics as seen from the top.
    pub graphics: Vec<Graphic>,
    /// Texts as seen from the top.
    pub texts: Vec<FootprintText>,
    /// 3D models.
    pub models: Vec<Model3d>,
    /// Further properties.
    pub properties: BTreeMap<String, String>,
    /// Attributes.
    pub attributes: FootprintAttributes,
}

/// Converts footprint-local coordinates: y up, mirrored in x on the bottom.
#[derive(Debug, Clone, Copy)]
struct Local {
    bottom: bool,
    copper_count: u8,
}

impl Local {
    fn xy(self, x: f64, y: f64) -> Point {
        let p = common::local(x, y);
        if self.bottom {
            Point::new(-p.x, p.y)
        } else {
            p
        }
    }

    fn at(self, node: Option<&Sexp<'_>>) -> Point {
        node.map_or(Point::ORIGIN, |n| {
            self.xy(n.num(0).unwrap_or(0.0), n.num(1).unwrap_or(0.0))
        })
    }

    fn layer(self, layer: Layer) -> Layer {
        if self.bottom {
            layer.flipped(self.copper_count)
        } else {
            layer
        }
    }

    /// An absolute KiCad angle as an angle relative to the footprint.
    fn rotation(self, absolute: f64, footprint: f64) -> Angle {
        if self.bottom {
            angle(footprint - absolute)
        } else {
            angle(absolute - footprint)
        }
    }
}

fn width(node: &Sexp<'_>) -> i64 {
    node.get("stroke")
        .and_then(|s| s.number("width"))
        .or_else(|| node.number("width"))
        .map_or(0, nm)
}

fn filled(node: &Sexp<'_>) -> bool {
    match node.get("fill") {
        Some(fill) => match fill.arg(0) {
            Some("yes" | "solid") => true,
            Some(_) => false,
            None => fill.value("type").is_some_and(|t| t != "none"),
        },
        None => false,
    }
}

/// A graphic item (`fp_line`, `gr_rect`, …) as a geometry in the given
/// coordinates.
pub fn graphic_geometry(node: &Sexp<'_>, xy: &dyn Fn(f64, f64) -> Point) -> Option<Geometry> {
    let at = |name: &str| -> Option<Point> {
        node.get(name)
            .map(|c| xy(c.num(0).unwrap_or(0.0), c.num(1).unwrap_or(0.0)))
    };
    let head = node.head()?;
    let kind = head.split_once('_').map_or(head, |(_, kind)| kind);
    Some(match kind {
        "line" => Geometry::Line {
            start: at("start")?,
            end: at("end")?,
        },
        "rect" => Geometry::Rect {
            start: at("start")?,
            end: at("end")?,
        },
        "circle" => {
            let center = at("center")?;
            let end = at("end")?;
            let radius = ((end.x - center.x) as f64)
                .hypot((end.y - center.y) as f64)
                .round() as i64;
            Geometry::Circle { center, radius }
        }
        "arc" => Geometry::Arc {
            start: at("start")?,
            mid: at("mid")?,
            end: at("end")?,
        },
        "poly" => Geometry::Polygon {
            points: common::points(node.get("pts"), xy),
        },
        "curve" => {
            let pts = common::points(node.get("pts"), xy);
            if pts.len() < 4 {
                return None;
            }
            Geometry::Bezier {
                start: pts[0],
                control1: pts[1],
                control2: pts[2],
                end: pts[3],
            }
        }
        _ => return None,
    })
}

fn pad_shape(node: &Sexp<'_>, local: Local, report: &mut ImportReport, file: &str) -> PadShape {
    let shape = node.arg(2).unwrap_or("rect");
    let chamfered = node.number("chamfer_ratio").is_some_and(|r| r > 0.0)
        && node.get("chamfer").is_some_and(|c| !c.args().is_empty());
    if chamfered {
        report.info(
            "chamfered-pad",
            Some(file),
            "chamfered pads are imported as rounded rectangles",
        );
    }
    match shape {
        "circle" => PadShape::Circle,
        "oval" => PadShape::Oval,
        "roundrect" => PadShape::RoundRect {
            ratio: (node
                .number("roundrect_rratio")
                .unwrap_or(0.25)
                .clamp(0.0, 0.5)
                * 1000.0)
                .round() as u16,
        },
        "trapezoid" => {
            let delta = node.get("rect_delta");
            PadShape::Trapezoid {
                delta: Size::new(
                    delta.and_then(|d| d.num(0)).map_or(0, nm),
                    delta.and_then(|d| d.num(1)).map_or(0, nm),
                ),
            }
        }
        "custom" => {
            let primitives = node.get("primitives");
            let polys: Vec<&Sexp<'_>> = primitives
                .map(|p| p.args().iter().filter(|item| item.is("gr_poly")).collect())
                .unwrap_or_default();
            let others = primitives.map_or(0, |p| p.args().len()) - polys.len();
            if polys.len() != 1 || others > 0 {
                report.warn(
                    "custom-pad",
                    Some(file),
                    "custom pads are imported with their first polygon only",
                );
            }
            match polys.first() {
                Some(poly) => PadShape::Custom {
                    points: common::points(poly.get("pts"), &|x, y| local_pad(local, x, y)),
                },
                None => PadShape::Rect,
            }
        }
        _ => PadShape::Rect,
    }
}

/// Pad-local coordinates (custom shapes, drill offsets).
fn local_pad(local: Local, x: f64, y: f64) -> Point {
    let p = common::local(x, y);
    if local.bottom {
        Point::new(-p.x, p.y)
    } else {
        p
    }
}

#[allow(clippy::too_many_lines, reason = "one field after the other")]
fn pad(
    node: &Sexp<'_>,
    local: Local,
    footprint_angle: f64,
    ids: &mut Ids,
    owner: &str,
    report: &mut ImportReport,
    file: &str,
) -> Pad {
    let number = node.arg(0).unwrap_or_default().to_owned();
    let kind = match node.arg(1) {
        Some("thru_hole") => PadKind::ThroughHole,
        Some("np_thru_hole") => PadKind::NpThroughHole,
        Some("connect") => PadKind::Connector,
        _ => PadKind::Smd,
    };
    let at = node.get("at");
    let size = node.get("size");
    let drill = node.get("drill").and_then(|drill| {
        let oval = drill.has_word("oval");
        let numbers: Vec<f64> = drill
            .args()
            .iter()
            .filter_map(|a| a.text()?.parse().ok())
            .collect();
        let first = *numbers.first()?;
        let offset = drill.get("offset").map_or(Point::ORIGIN, |o| {
            local_pad(local, o.num(0).unwrap_or(0.0), o.num(1).unwrap_or(0.0))
        });
        if oval {
            let second = numbers.get(1).copied().unwrap_or(first);
            if second > first {
                report.info(
                    "vertical-slot",
                    Some(file),
                    "vertical slots are imported along the pad width",
                );
            }
            Some(Drill {
                diameter: nm(first.min(second)),
                slot_length: (first - second)
                    .abs()
                    .gt(&1e-6)
                    .then(|| nm(first.max(second))),
                offset,
            })
        } else {
            (first > 0.0).then(|| Drill {
                diameter: nm(first),
                slot_length: None,
                offset,
            })
        }
    });
    let mut layers: Vec<LayerSpec> = Vec::new();
    if let Some(list) = node.get("layers") {
        for name in list.args().iter().filter_map(Sexp::text) {
            for spec in layer_specs(name) {
                let spec = match spec {
                    LayerSpec::Layer(layer) => LayerSpec::Layer(local.layer(layer)),
                    other => other,
                };
                if !layers.contains(&spec) {
                    layers.push(spec);
                }
            }
        }
    }
    let margin = |name: &str| node.number(name).filter(|v| *v != 0.0).map(nm);
    Pad {
        id: ids.of(node, &[owner, "pad", &number]),
        number,
        net: None,
        kind,
        shape: pad_shape(node, local, report, file),
        at: local.at(at),
        rotation: local.rotation(at.and_then(|a| a.num(2)).unwrap_or(0.0), footprint_angle),
        size: Size::new(
            size.and_then(|s| s.num(0)).map_or(0, nm),
            size.and_then(|s| s.num(1)).map_or(0, nm),
        ),
        drill,
        layers,
        clearance: margin("clearance"),
        mask_margin: margin("solder_mask_margin"),
        paste_margin: margin("solder_paste_margin"),
    }
}

/// A 3D model. On the bottom side, KiCad's flipped local coordinates are
/// the library's mirrored in y; ElectronicWerk mirrors them in x (see
/// [`Local`]), which turns the footprint by a half turn. The model gets the
/// same half turn (offset negated in x and y, 180° more about z), so it
/// still sits on its pads.
fn model(node: &Sexp<'_>, bottom: bool) -> Model3d {
    let xyz = |name: &str, scale: f64| -> [f64; 3] {
        let xyz = node.get(name).and_then(|n| n.get("xyz"));
        [0, 1, 2].map(|i| xyz.and_then(|x| x.num(i)).unwrap_or(0.0) * scale)
    };
    // KiCad 5 stored the offset as `(at (xyz …))` in inches.
    let offset = if node.get("offset").is_some() {
        xyz("offset", 1.0)
    } else {
        xyz("at", 25.4)
    };
    let scale = node
        .get("scale")
        .and_then(|s| s.get("xyz"))
        .map_or([1.0; 3], |x| [0, 1, 2].map(|i| x.num(i).unwrap_or(1.0)));
    let mut rotate = xyz("rotate", 1.0);
    let mut offset = offset;
    if bottom {
        offset = [-offset[0], -offset[1], offset[2]];
        // z − 180°, kept within [−180°, 180°).
        rotate[2] = rotate[2].rem_euclid(360.0) - 180.0;
    }
    Model3d {
        path: node.arg(0).unwrap_or_default().to_owned(),
        offset: Vec3 {
            x: nm(offset[0]),
            y: nm(offset[1]),
            z: nm(offset[2]),
        },
        rotation: rotate.map(angle),
        scale: Scale3(scale),
        hidden: node.flag("hide"),
    }
}

fn text(
    node: &Sexp<'_>,
    kind: FootprintTextKind,
    content: &str,
    local: Local,
    footprint_angle: f64,
) -> Option<FootprintText> {
    let layer = common::layer(node.value("layer")?)?;
    let at = node.get("at");
    let mut style: TextStyle = common::text_style(node.get("effects"));
    if style.thickness == 0 {
        style.thickness = style.size / 8;
    }
    Some(FootprintText {
        kind,
        text: match kind {
            FootprintTextKind::User => content.to_owned(),
            _ => String::new(),
        },
        layer: local.layer(layer),
        at: local.at(at),
        rotation: local.rotation(at.and_then(|a| a.num(2)).unwrap_or(0.0), footprint_angle),
        style,
        visible: !common::hidden(node),
    })
}

/// Reads a footprint (`(footprint …)` or the KiCad 5 `(module …)`).
/// `copper_count` is needed to flip inner layers of bottom footprints.
#[allow(clippy::too_many_lines, reason = "one item kind after the other")]
pub fn footprint(
    node: &Sexp<'_>,
    copper_count: u8,
    ids: &mut Ids,
    report: &mut ImportReport,
    file: &str,
) -> Footprint {
    let name = node.arg(0).unwrap_or_default().to_owned();
    let bottom = node.value("layer") == Some("B.Cu");
    let local = Local {
        bottom,
        copper_count,
    };
    let at = node.get("at");
    let at = (
        at.and_then(|a| a.num(0)).unwrap_or(0.0),
        at.and_then(|a| a.num(1)).unwrap_or(0.0),
        at.and_then(|a| a.num(2)).unwrap_or(0.0),
    );
    let mut out = Footprint {
        name: name.clone(),
        description: node.value("descr").unwrap_or_default().to_owned(),
        keywords: node.value("tags").unwrap_or_default().to_owned(),
        bottom,
        at,
        designator: String::new(),
        value: String::new(),
        locked: node.flag("locked"),
        path: node.value("path").map(str::to_owned),
        pads: Vec::new(),
        pad_nets: Vec::new(),
        graphics: Vec::new(),
        texts: Vec::new(),
        models: Vec::new(),
        properties: BTreeMap::new(),
        attributes: FootprintAttributes::default(),
    };
    let xy = |x: f64, y: f64| local.xy(x, y);
    for item in node.args() {
        let Some(head) = item.head() else {
            continue;
        };
        match head {
            "pad" => {
                out.pad_nets.push(net_ref(item));
                out.pads
                    .push(pad(item, local, at.2, ids, &name, report, file));
            }
            "property" => {
                let key = item.arg(0).unwrap_or_default();
                let value = item.arg(1).unwrap_or_default();
                let kind = match key {
                    "Reference" => {
                        value.clone_into(&mut out.designator);
                        Some(FootprintTextKind::Designator)
                    }
                    "Value" => {
                        value.clone_into(&mut out.value);
                        Some(FootprintTextKind::Value)
                    }
                    _ => {
                        if !value.is_empty() && key != "Footprint" {
                            out.properties.insert(key.to_owned(), value.to_owned());
                        }
                        (!common::hidden(item) && item.get("layer").is_some())
                            .then_some(FootprintTextKind::User)
                    }
                };
                if let Some(kind) = kind {
                    if item.get("layer").is_some() {
                        out.texts.extend(text(item, kind, value, local, at.2));
                    }
                }
            }
            "fp_text" => {
                let kind = match item.arg(0) {
                    Some("reference") => FootprintTextKind::Designator,
                    Some("value") => FootprintTextKind::Value,
                    _ => FootprintTextKind::User,
                };
                let content = item.arg(1).unwrap_or_default();
                match kind {
                    FootprintTextKind::Designator => content.clone_into(&mut out.designator),
                    FootprintTextKind::Value => content.clone_into(&mut out.value),
                    FootprintTextKind::User => {}
                }
                out.texts.extend(text(item, kind, content, local, at.2));
            }
            "fp_line" | "fp_rect" | "fp_circle" | "fp_arc" | "fp_poly" | "fp_curve" => {
                let Some(layer) = item.value("layer").and_then(common::layer) else {
                    report.warn(
                        "unknown-layer",
                        Some(file),
                        format!(
                            "graphic on an unknown layer `{}`",
                            item.value("layer").unwrap_or("?")
                        ),
                    );
                    continue;
                };
                if let Some(geometry) = graphic_geometry(item, &xy) {
                    out.graphics.push(Graphic {
                        layer: local.layer(layer),
                        geometry,
                        width: width(item),
                        filled: filled(item),
                    });
                }
            }
            "fp_text_box" => report.info(
                "text-box",
                Some(file),
                "text boxes in footprints are not imported",
            ),
            "model" => out.models.push(model(item, bottom)),
            "attr" => {
                out.attributes = FootprintAttributes {
                    mounting: if item.has_word("smd") {
                        Mounting::Smd
                    } else if item.has_word("through_hole") {
                        Mounting::ThroughHole
                    } else {
                        Mounting::Other
                    },
                    exclude_from_bom: item.has_word("exclude_from_bom"),
                    exclude_from_placement: item.has_word("exclude_from_pos_files"),
                    dnp: item.has_word("dnp"),
                };
            }
            "zone" => report.warn(
                "footprint-zone",
                Some(file),
                "zones inside footprints are not imported",
            ),
            "dimension" => report.info(
                "unsupported-item",
                Some(file),
                "dimensions are not imported",
            ),
            _ => {}
        }
    }
    if node.flag("dnp") {
        out.attributes.dnp = true;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sexpr::parse;
    use ew_model::layer::{CopperLayer, Side};
    use ew_model::units::mm;

    const SOT23: &str = r#"(footprint "Package_TO_SOT_SMD:SOT-23"
        (layer "B.Cu") (uuid "125ea8ec-0cf9-4d29-9e6d-acb56a5a23b3") (at 10 20 90)
        (descr "SOT-23") (tags "SOT TO_SOT_SMD")
        (property "Reference" "Q1" (at 0 -2.4 90) (layer "B.SilkS") (effects (font (size 1 1) (thickness 0.15)) (justify mirror)))
        (property "Value" "BC817" (at 0 2.4 90) (layer "B.Fab") (hide yes))
        (property "Datasheet" "" (at 0 0 90) (layer "B.Fab") (hide yes))
        (path "/a/b")
        (attr smd exclude_from_bom)
        (fp_line (start 0.76 1.58) (end -0.7 1.58) (stroke (width 0.12) (type solid)) (layer "B.SilkS"))
        (fp_rect (start -1.7 -1.75) (end 1.7 1.75) (stroke (width 0.05) (type solid)) (fill no) (layer "B.CrtYd"))
        (fp_circle (center 0 0) (end 0.5 0) (stroke (width 0.1)) (fill yes) (layer "B.Fab"))
        (pad "1" smd roundrect (at -1 0.95 90) (size 1.475 0.6) (layers "B.Cu" "B.Paste" "B.Mask") (roundrect_rratio 0.25) (net "GND") (uuid "0df0b73d-700f-47ab-bea9-25e8cb1f3f66"))
        (pad "2" smd rect (at -1 -0.95 90) (size 1.475 0.6) (layers "B.Cu" "B.Paste" "B.Mask") (net 3 "VCC"))
        (pad "" np_thru_hole circle (at 0 0) (size 1 1) (drill 1) (layers "*.Cu" "*.Mask"))
        (model "${KICAD10_3DMODEL_DIR}/Package_TO_SOT_SMD.3dshapes/SOT-23.step" (offset (xyz 0 0 0)) (scale (xyz 1 1 1)) (rotate (xyz 0 0 90))))"#;

    #[test]
    fn reads_a_bottom_footprint_as_seen_from_the_top() {
        let node = parse(SOT23).unwrap();
        let mut ids = Ids::default();
        let mut report = ImportReport::default();
        let fp = footprint(&node, 4, &mut ids, &mut report, "board");
        assert!(fp.bottom);
        assert_eq!(fp.designator, "Q1");
        assert_eq!(fp.value, "BC817");
        assert_eq!(fp.path.as_deref(), Some("/a/b"));
        assert_eq!(fp.at, (10.0, 20.0, 90.0));
        assert!(fp.attributes.exclude_from_bom);
        assert_eq!(fp.attributes.mounting, Mounting::Smd);
        // Pads: mirrored in x, y up, layers flipped to the top.
        let pad = &fp.pads[0];
        assert_eq!(pad.at, Point::new(mm(1), -nm(0.95)));
        assert_eq!(pad.rotation, Angle::ZERO, "90° absolute on a 90° footprint");
        assert!(pad.layers.contains(&LayerSpec::Layer(Layer::TOP_COPPER)));
        assert!(
            pad.layers
                .contains(&LayerSpec::Layer(Layer::Paste(Side::Top)))
        );
        assert_eq!(pad.shape, PadShape::RoundRect { ratio: 250 });
        // The model turns with the pads: 90° − 180° about z.
        assert_eq!(fp.models[0].rotation[2], Angle::DEG_270);
        assert_eq!(fp.pad_nets[0], Some(NetRef::Name("GND".into())));
        assert_eq!(fp.pad_nets[1], Some(NetRef::Name("VCC".into())));
        assert_eq!(fp.pad_nets[2], None);
        assert_eq!(fp.pads[2].kind, PadKind::NpThroughHole);
        assert_eq!(fp.pads[2].drill.map(|d| d.diameter), Some(mm(1)));
        assert_eq!(
            fp.pads[2].layers,
            vec![LayerSpec::AllCopper, LayerSpec::AllMask]
        );
        assert_eq!(pad.id.to_string(), "0df0b73d-700f-47ab-bea9-25e8cb1f3f66");
        // Graphics and texts.
        assert_eq!(fp.graphics.len(), 3);
        assert_eq!(fp.graphics[0].layer, Layer::Silkscreen(Side::Top));
        assert_eq!(fp.graphics[1].layer, Layer::Courtyard(Side::Top));
        assert!(fp.graphics[2].filled);
        assert_eq!(fp.texts.len(), 2);
        assert_eq!(fp.texts[0].kind, FootprintTextKind::Designator);
        assert!(fp.texts[0].visible);
        assert!(!fp.texts[1].visible);
        assert_eq!(fp.texts[0].layer, Layer::Silkscreen(Side::Top));
        let _ = CopperLayer::Top;
    }

    #[test]
    fn reads_net_references_of_every_version() {
        let new = parse(r#"(segment (net "GND"))"#).unwrap();
        assert_eq!(net_ref(&new), Some(NetRef::Name("GND".into())));
        let old = parse("(segment (net 5))").unwrap();
        assert_eq!(net_ref(&old), Some(NetRef::Code(5)));
        let pad = parse(r#"(pad (net 5 "VCC"))"#).unwrap();
        assert_eq!(net_ref(&pad), Some(NetRef::Name("VCC".into())));
    }

    #[test]
    fn reads_slots_and_old_text_items() {
        let text = r#"(module "Conn:Slot" (layer "F.Cu")
            (fp_text reference "J1" (at 0 -3) (layer "F.SilkS") (effects (font (size 1 1))))
            (fp_text user "${REFERENCE}" (at 0 0) (layer "F.Fab") (effects (font (size 1 1))))
            (pad "1" thru_hole oval (at 0 0) (size 3 2) (drill oval 2 1) (layers "*.Cu" "*.Mask")))"#;
        let node = parse(text).unwrap();
        let mut ids = Ids::default();
        let mut report = ImportReport::default();
        let fp = footprint(&node, 2, &mut ids, &mut report, "lib");
        assert_eq!(fp.designator, "J1");
        assert_eq!(fp.texts[1].text, "${REFERENCE}");
        let drill = fp.pads[0].drill.unwrap();
        assert_eq!((drill.diameter, drill.slot_length), (mm(1), Some(mm(2))));
    }
}
