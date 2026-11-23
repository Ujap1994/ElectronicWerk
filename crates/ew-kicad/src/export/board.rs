//! Boards and footprints as KiCad files (`.kicad_pcb`, `.kicad_mod`): the
//! inverse of the board and footprint import, in the KiCad 8 format that
//! KiCad 8 and later open.

use std::collections::BTreeMap;

use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, LayerSpec, Side};
use ew_model::library::FootprintDef;
use ew_model::pcb::{
    BoardShape, DielectricKind, FootprintAttributes, FootprintInstance, FootprintText,
    FootprintTextKind, Graphic, Model3d, Mounting, Pad, PadKind, PadLayerShape, PadShape,
    PadStackLayers, Pcb, StackupLayer, ViaFill, ViaType, Zone,
};
use ew_model::shape::Geometry;
use ew_model::text::{HAlign, TextStyle, VAlign};
use ew_model::units::{Nm, Point};

use super::ExportReport;
use crate::write::{Sx, at, stroke, uuid, xy};

/// The KiCad board format written (KiCad 8).
pub const PCB_VERSION: &str = "20240108";

/// The page of the board file: its size sets where y = 0 lies, because
/// KiCad's y axis points down from the top of the page.
#[derive(Debug, Clone, Copy)]
pub struct Page {
    /// Page width (nm).
    pub width: Nm,
    /// Page height (nm).
    pub height: Nm,
}

impl Page {
    /// A4 landscape, or a user size that holds `bounds` with a margin.
    #[must_use]
    pub fn around(bounds: Option<(Point, Point)>) -> Self {
        const A4: (Nm, Nm) = (297_000_000, 210_000_000);
        let Some((low, high)) = bounds else {
            return Self {
                width: A4.0,
                height: A4.1,
            };
        };
        if low.x >= 0 && low.y >= 0 && high.x <= A4.0 && high.y <= A4.1 {
            return Self {
                width: A4.0,
                height: A4.1,
            };
        }
        let margin = 10_000_000;
        let round = |v: Nm| (v + margin).div_euclid(1_000_000) * 1_000_000 + 1_000_000;
        Self {
            width: round(high.x.max(A4.0 - margin)),
            height: round(high.y.max(A4.1 - margin)),
        }
    }

    /// `(paper …)`.
    #[must_use]
    pub fn sx(self) -> Sx {
        if self.width == 297_000_000 && self.height == 210_000_000 {
            Sx::named("paper", "A4")
        } else {
            Sx::named("paper", "User").mm(self.width).mm(self.height)
        }
    }

    /// A board position in KiCad page coordinates.
    #[must_use]
    pub const fn kicad(self, p: Point) -> (Nm, Nm) {
        (p.x, self.height - p.y)
    }

    fn at(self, p: Point, angle: Option<f64>) -> Sx {
        let (x, y) = self.kicad(p);
        at(x, y, angle)
    }

    fn xy(self, p: Point) -> Sx {
        let (x, y) = self.kicad(p);
        xy(x, y)
    }
}

/// The KiCad name of a board layer (`None`: KiCad has no such layer).
#[must_use]
pub fn layer_name(layer: Layer) -> Option<String> {
    let sided = |side: Side, front: &str, back: &str| {
        Some(if side == Side::Top { front } else { back }.to_owned())
    };
    match layer {
        Layer::Copper(copper) => Some(copper_name(copper)),
        Layer::Silkscreen(side) => sided(side, "F.SilkS", "B.SilkS"),
        Layer::SolderMask(side) => sided(side, "F.Mask", "B.Mask"),
        Layer::Paste(side) => sided(side, "F.Paste", "B.Paste"),
        Layer::Adhesive(side) => sided(side, "F.Adhes", "B.Adhes"),
        Layer::Fabrication(side) => sided(side, "F.Fab", "B.Fab"),
        Layer::Courtyard(side) => sided(side, "F.CrtYd", "B.CrtYd"),
        Layer::Outline => Some("Edge.Cuts".to_owned()),
        Layer::Mechanical(1) => Some("Margin".to_owned()),
        Layer::User(1) => Some("Dwgs.User".to_owned()),
        Layer::User(2) => Some("Cmts.User".to_owned()),
        Layer::User(3) => Some("Eco1.User".to_owned()),
        Layer::User(4) => Some("Eco2.User".to_owned()),
        Layer::User(n) if (5..=13).contains(&n) => Some(format!("User.{}", n - 4)),
        Layer::Mechanical(_) | Layer::User(_) => None,
    }
}

/// `F.Cu`, `In3.Cu`, `B.Cu`.
#[must_use]
pub fn copper_name(layer: CopperLayer) -> String {
    match layer {
        CopperLayer::Top => "F.Cu".to_owned(),
        CopperLayer::Bottom => "B.Cu".to_owned(),
        CopperLayer::Inner(n) => format!("In{n}.Cu"),
    }
}

/// The fixed (non-copper) layers of a KiCad 8 board with their numbers.
const FIXED_LAYERS: &[(u8, &str, &str)] = &[
    (32, "B.Adhes", "B.Adhesive"),
    (33, "F.Adhes", "F.Adhesive"),
    (34, "B.Paste", ""),
    (35, "F.Paste", ""),
    (36, "B.SilkS", "B.Silkscreen"),
    (37, "F.SilkS", "F.Silkscreen"),
    (38, "B.Mask", ""),
    (39, "F.Mask", ""),
    (40, "Dwgs.User", "User.Drawings"),
    (41, "Cmts.User", "User.Comments"),
    (42, "Eco1.User", "User.Eco1"),
    (43, "Eco2.User", "User.Eco2"),
    (44, "Edge.Cuts", ""),
    (45, "Margin", ""),
    (46, "B.CrtYd", "B.Courtyard"),
    (47, "F.CrtYd", "F.Courtyard"),
    (48, "B.Fab", ""),
    (49, "F.Fab", ""),
];

/// `(effects (font …) (justify …))` of a text style.
#[must_use]
pub fn effects(style: &TextStyle, hidden: bool, mirrored: bool) -> Sx {
    let mut font = Sx::list("font").child(Sx::list("size").mm(style.size).mm(style.size));
    if style.thickness > 0 {
        font = font.child(Sx::list("thickness").mm(style.thickness));
    }
    if style.bold {
        font = font.child(Sx::flag("bold", true));
    }
    if style.italic {
        font = font.child(Sx::flag("italic", true));
    }
    let mut node = Sx::list("effects").child(font);
    let mut justify = Sx::list("justify");
    let mut any = false;
    match style.h_align {
        HAlign::Left => {
            justify = justify.word("left");
            any = true;
        }
        HAlign::Right => {
            justify = justify.word("right");
            any = true;
        }
        HAlign::Center => {}
    }
    match style.v_align {
        VAlign::Top => {
            justify = justify.word("top");
            any = true;
        }
        VAlign::Bottom => {
            justify = justify.word("bottom");
            any = true;
        }
        VAlign::Center => {}
    }
    if mirrored {
        justify = justify.word("mirror");
        any = true;
    }
    if any {
        node = node.child(justify);
    }
    if hidden {
        node = node.child(Sx::flag("hide", true));
    }
    node
}

/// Converts footprint-local points (y up, bottom parts as seen from the
/// top) into KiCad's footprint coordinates (y down, bottom parts flipped).
#[derive(Debug, Clone, Copy)]
struct Local {
    bottom: bool,
    copper_count: u8,
}

impl Local {
    const fn xy(self, p: Point) -> (Nm, Nm) {
        if self.bottom {
            (-p.x, -p.y)
        } else {
            (p.x, -p.y)
        }
    }

    fn at(self, p: Point, angle: Option<f64>) -> Sx {
        let (x, y) = self.xy(p);
        at(x, y, angle)
    }

    fn point(self, head: &str, p: Point) -> Sx {
        let (x, y) = self.xy(p);
        Sx::list(head).mm(x).mm(y)
    }

    fn layer(self, layer: Layer) -> Layer {
        if self.bottom {
            layer.flipped(self.copper_count)
        } else {
            layer
        }
    }

    /// KiCad's absolute angle of a pad or text from its angle on the part.
    fn angle(self, own: f64, footprint: f64) -> f64 {
        let a = if self.bottom {
            footprint - own
        } else {
            footprint + own
        };
        a.rem_euclid(360.0)
    }
}

fn pts(points: &[Point], xy: &dyn Fn(Point) -> Sx) -> Sx {
    Sx::list("pts").children(points.iter().map(|&p| xy(p)))
}

/// A drawn shape: `fp_line`, `gr_circle`, … (`prefix` is `fp` or `gr`).
#[allow(
    clippy::too_many_arguments,
    reason = "the parts of one KiCad item and its coordinate conversions"
)]
fn graphic(
    prefix: &str,
    geometry: &Geometry,
    width: Nm,
    filled: bool,
    layer: &str,
    id: Option<Id>,
    point: &dyn Fn(&str, Point) -> Sx,
    xy_of: &dyn Fn(Point) -> Sx,
) -> Sx {
    let closed = matches!(
        geometry,
        Geometry::Rect { .. } | Geometry::Circle { .. } | Geometry::Polygon { .. }
    );
    let kind = |name: &str| Sx::list(&format!("{prefix}_{name}"));
    let mut node = match geometry {
        Geometry::Line { start, end } => kind("line")
            .child(point("start", *start))
            .child(point("end", *end)),
        Geometry::Rect { start, end } => kind("rect")
            .child(point("start", *start))
            .child(point("end", *end)),
        Geometry::Circle { center, radius } => kind("circle")
            .child(point("center", *center))
            .child(point("end", Point::new(center.x + radius, center.y))),
        Geometry::Arc { start, mid, end } => kind("arc")
            .child(point("start", *start))
            .child(point("mid", *mid))
            .child(point("end", *end)),
        Geometry::Polyline { points } => {
            // KiCad has no open polyline outside schematics: one line per
            // segment would need ids; a polygon would close it. Lines it is.
            kind("poly").child(pts(points, xy_of))
        }
        Geometry::Polygon { points } => kind("poly").child(pts(points, xy_of)),
        Geometry::Bezier {
            start,
            control1,
            control2,
            end,
        } => kind("curve").child(pts(&[*start, *control1, *control2, *end], xy_of)),
    };
    node = node.child(stroke(width, "solid"));
    if closed {
        node = node.child(Sx::pair("fill", if filled { "solid" } else { "none" }));
    }
    node = node.child(Sx::named("layer", layer));
    if let Some(id) = id {
        node = node.child(uuid(id));
    }
    node
}

/// Polylines become one line per segment (KiCad boards have no open
/// polyline).
fn split_polyline(geometry: &Geometry) -> Vec<Geometry> {
    match geometry {
        Geometry::Polyline { points } => points
            .windows(2)
            .map(|pair| Geometry::Line {
                start: pair[0],
                end: pair[1],
            })
            .collect(),
        other => vec![other.clone()],
    }
}

/// What a footprint needs from the board around it.
pub struct FootprintContext<'a> {
    /// Copper layers of the board.
    pub copper_count: u8,
    /// Net codes and names by id (empty in libraries).
    pub nets: &'a BTreeMap<Id, (u32, String)>,
}

/// The parts of a footprint that a board instance and a library
/// definition share.
pub struct FootprintParts<'a> {
    /// `Library:Name`.
    pub name: &'a str,
    /// Description.
    pub description: &'a str,
    /// Keywords.
    pub keywords: &'a str,
    /// Pads.
    pub pads: &'a [Pad],
    /// Graphics.
    pub graphics: &'a [Graphic],
    /// Texts.
    pub texts: &'a [FootprintText],
    /// 3D models.
    pub models: &'a [Model3d],
    /// Attributes.
    pub attributes: FootprintAttributes,
}

/// Where a footprint lies on a board (`None` in a library).
pub struct Placement<'a> {
    /// Instance id.
    pub id: Id,
    /// Position on the board.
    pub at: Point,
    /// Rotation in degrees.
    pub rotation: f64,
    /// Board side.
    pub side: Side,
    /// Locked.
    pub locked: bool,
    /// Designator.
    pub designator: &'a str,
    /// Value.
    pub value: &'a str,
    /// Further properties.
    pub properties: &'a BTreeMap<String, String>,
    /// The page of the board.
    pub page: Page,
    /// The schematic symbol (`/<component id>`).
    pub path: Option<String>,
}

/// A footprint as `(footprint …)`.
#[allow(clippy::too_many_lines, reason = "one item kind after the other")]
#[must_use]
pub fn footprint(
    parts: &FootprintParts<'_>,
    place: Option<&Placement<'_>>,
    context: &FootprintContext<'_>,
    report: &mut ExportReport,
) -> Sx {
    let bottom = place.is_some_and(|p| p.side == Side::Bottom);
    let local = Local {
        bottom,
        copper_count: context.copper_count,
    };
    let rotation = place.map_or(0.0, |p| p.rotation);
    let mut node = Sx::list("footprint").text(parts.name);
    if place.is_some_and(|p| p.locked) {
        node = node.child(Sx::flag("locked", true));
    }
    node = node.child(Sx::named("layer", if bottom { "B.Cu" } else { "F.Cu" }));
    if let Some(place) = place {
        node = node
            .child(uuid(place.id))
            .child(place.page.at(place.at, Some(rotation)));
    }
    if !parts.description.is_empty() {
        node = node.child(Sx::named("descr", parts.description));
    }
    if !parts.keywords.is_empty() {
        node = node.child(Sx::named("tags", parts.keywords));
    }
    let layer_of = |layer: Layer, report: &mut ExportReport| -> Option<String> {
        let name = layer_name(local.layer(layer));
        if name.is_none() {
            report.note(
                "layer",
                format!("drawings on the layer {layer} have no KiCad layer and were left out"),
            );
        }
        name
    };
    // Reference and value: the first text of each kind, or a hidden one.
    let designator = place.map_or("REF**", |p| p.designator);
    let value = place.map_or(parts.name.rsplit(':').next().unwrap_or(parts.name), |p| {
        p.value
    });
    for (kind, key, content, fallback_layer) in [
        (
            FootprintTextKind::Designator,
            "Reference",
            designator,
            "F.SilkS",
        ),
        (FootprintTextKind::Value, "Value", value, "F.Fab"),
    ] {
        let text = parts.texts.iter().find(|t| t.kind == kind);
        let mut property = Sx::list("property").text(key).text(content);
        if let Some(text) = text {
            property = property
                .child(local.at(
                    text.at,
                    Some(local.angle(text.rotation.degrees(), rotation)),
                ))
                .child(Sx::named(
                    "layer",
                    layer_of(text.layer, report).unwrap_or_else(|| fallback_layer.to_owned()),
                ))
                .child(effects(&text.style, !text.visible, bottom));
        } else {
            let layer = if bottom {
                fallback_layer.replacen("F.", "B.", 1)
            } else {
                fallback_layer.to_owned()
            };
            property = property
                .child(at(0, 0, Some(rotation)))
                .child(Sx::named("layer", layer))
                .child(effects(&TextStyle::default(), true, bottom));
        }
        node = node.child(property);
    }
    if let Some(place) = place {
        for (key, value) in place.properties {
            node = node.child(
                Sx::list("property")
                    .text(key)
                    .text(value)
                    .child(at(0, 0, None))
                    .child(Sx::named("layer", if bottom { "B.Fab" } else { "F.Fab" }))
                    .child(effects(&TextStyle::default(), true, bottom)),
            );
        }
        if let Some(path) = &place.path {
            node = node.child(Sx::named("path", path.clone()));
        }
    }
    let attributes = parts.attributes;
    let mut attr = Sx::list("attr");
    match attributes.mounting {
        Mounting::Smd => attr = attr.word("smd"),
        Mounting::ThroughHole => attr = attr.word("through_hole"),
        Mounting::Other => {}
    }
    if attributes.exclude_from_placement {
        attr = attr.word("exclude_from_pos_files");
    }
    if attributes.exclude_from_bom {
        attr = attr.word("exclude_from_bom");
    }
    if attributes.dnp {
        attr = attr.word("dnp");
    }
    if matches!(&attr, Sx::List(items) if items.len() == 1) {
        // Neither SMD nor through-hole and no flags: a word KiCad accepts
        // that keeps the part "unspecified" when it is read back.
        attr = attr.word("allow_missing_courtyard");
    }
    node = node.child(attr);
    for text in parts
        .texts
        .iter()
        .filter(|t| t.kind == FootprintTextKind::User)
    {
        let Some(layer) = layer_of(text.layer, report) else {
            continue;
        };
        node = node.child(
            Sx::list("fp_text")
                .word("user")
                .text(text.text.clone())
                .child(local.at(
                    text.at,
                    Some(local.angle(text.rotation.degrees(), rotation)),
                ))
                .child(Sx::named("layer", layer))
                .child(effects(&text.style, !text.visible, bottom)),
        );
    }
    let point = |head: &str, p: Point| local.point(head, p);
    let xy_of = |p: Point| local.point("xy", p);
    for item in parts.graphics {
        let Some(layer) = layer_of(item.layer, report) else {
            continue;
        };
        for geometry in split_polyline(&item.geometry) {
            node = node.child(graphic(
                "fp",
                &geometry,
                item.width,
                item.filled,
                &layer,
                None,
                &point,
                &xy_of,
            ));
        }
    }
    for pad in parts.pads {
        node = node.child(pad_sx(pad, local, rotation, context, report));
    }
    for model in parts.models {
        node = node.child(model_sx(model, bottom));
    }
    node
}

fn pad_sx(
    pad: &Pad,
    local: Local,
    footprint_angle: f64,
    context: &FootprintContext<'_>,
    report: &mut ExportReport,
) -> Sx {
    let pad = &*kicad_sides(pad, local.bottom);
    let kind = match pad.kind {
        PadKind::Smd => "smd",
        PadKind::ThroughHole => "thru_hole",
        PadKind::NpThroughHole => "np_thru_hole",
        PadKind::Connector => "connect",
    };
    let shape = match pad.shape {
        PadShape::Circle => "circle",
        PadShape::Rect => "rect",
        PadShape::Oval => "oval",
        PadShape::RoundRect { .. } => "roundrect",
        PadShape::Trapezoid { .. } => "trapezoid",
        PadShape::Custom { .. } => "custom",
    };
    let mut node = Sx::list("pad")
        .text(pad.number.clone())
        .word(kind)
        .word(shape)
        .child(local.at(
            pad.at,
            Some(local.angle(pad.rotation.degrees(), footprint_angle)),
        ))
        .child(Sx::list("size").mm(pad.size.w).mm(pad.size.h));
    if let Some(drill) = pad.drill {
        let mut sx = match drill.slot_length {
            Some(length) => Sx::list("drill").word("oval").mm(length).mm(drill.diameter),
            None => Sx::list("drill").mm(drill.diameter),
        };
        if drill.offset != Point::ORIGIN {
            sx = sx.child(local.point("offset", drill.offset));
        }
        node = node.child(sx);
    }
    let mut layers = Sx::list("layers");
    for spec in &pad.layers {
        match *spec {
            LayerSpec::AllCopper => layers = layers.text("*.Cu"),
            LayerSpec::AllMask => layers = layers.text("*.Mask"),
            LayerSpec::AllPaste => layers = layers.text("*.Paste"),
            LayerSpec::Layer(layer) => match layer_name(local.layer(layer)) {
                Some(name) => layers = layers.text(name),
                None => report.note("layer", format!("a pad layer {layer} was left out")),
            },
        }
    }
    node = node.child(layers);
    match &pad.shape {
        PadShape::RoundRect { ratio } => {
            node = node.child(Sx::list("roundrect_rratio").num(f64::from(*ratio) / 1000.0));
        }
        PadShape::Trapezoid { delta } => {
            node = node.child(Sx::list("rect_delta").mm(delta.w).mm(delta.h));
        }
        PadShape::Custom { points } => {
            node = node
                .child(
                    Sx::list("options")
                        .child(Sx::pair("clearance", "outline"))
                        .child(Sx::pair("anchor", "rect")),
                )
                .child(
                    Sx::list("primitives").child(
                        Sx::list("gr_poly")
                            .child(pts(points, &|p| local.point("xy", p)))
                            .child(Sx::list("width").num(0.0))
                            .child(Sx::flag("fill", true)),
                    ),
                );
        }
        _ => {}
    }
    if let Some((code, name)) = pad.net.and_then(|id| context.nets.get(&id)) {
        node = node.child(Sx::list("net").word(code.to_string()).text(name.clone()));
    }
    node = node.child(uuid(pad.id));
    if let Some(margin) = pad.clearance {
        node = node.child(Sx::list("clearance").mm(margin));
    }
    if let Some(margin) = pad.mask_margin {
        node = node.child(Sx::list("solder_mask_margin").mm(margin));
    }
    if let Some(margin) = pad.paste_margin {
        node = node.child(Sx::list("solder_paste_margin").mm(margin));
    }
    if !pad.stack.is_empty() {
        node = node.child(padstack_sx(pad, report));
    }
    node
}

/// On the bottom, KiCad keeps the shape of the board's top in the pad
/// itself and the footprint's own front in the `B.Cu` padstack entry.
fn kicad_sides(pad: &Pad, bottom: bool) -> std::borrow::Cow<'_, Pad> {
    let Some(back) = pad
        .stack
        .iter()
        .find(|e| e.layers == PadStackLayers::Back)
        .filter(|_| bottom)
    else {
        return std::borrow::Cow::Borrowed(pad);
    };
    let mut stack: Vec<PadLayerShape> = pad
        .stack
        .iter()
        .filter(|e| e.layers != PadStackLayers::Back)
        .cloned()
        .collect();
    stack.push(PadLayerShape {
        layers: PadStackLayers::Back,
        shape: pad.shape.clone(),
        size: pad.size,
    });
    std::borrow::Cow::Owned(Pad {
        shape: back.shape.clone(),
        size: back.size,
        stack,
        ..pad.clone()
    })
}

/// A padstack as KiCad 9 writes it: `front_inner_back` mode with an entry
/// for the inner layers and one for the back (the pad's own shape where
/// the stack has none).
fn padstack_sx(pad: &Pad, report: &mut ExportReport) -> Sx {
    let mut node = Sx::list("padstack").child(Sx::pair("mode", "front_inner_back"));
    for (layers, name) in [
        (PadStackLayers::Inner, "Inner"),
        (PadStackLayers::Back, "B.Cu"),
    ] {
        let (shape, size) = pad
            .stack
            .iter()
            .find(|e| e.layers == layers)
            .map_or((&pad.shape, pad.size), |e| (&e.shape, e.size));
        let word = match shape {
            PadShape::Circle => "circle",
            PadShape::Oval => "oval",
            PadShape::RoundRect { .. } => "roundrect",
            PadShape::Trapezoid { .. } => "trapezoid",
            PadShape::Rect => "rect",
            PadShape::Custom { .. } => {
                report.note(
                    "padstack",
                    "a custom shape on one padstack layer was written as a rectangle".to_owned(),
                );
                "rect"
            }
        };
        let mut entry = Sx::named("layer", name)
            .child(Sx::pair("shape", word))
            .child(Sx::list("size").mm(size.w).mm(size.h));
        match shape {
            PadShape::RoundRect { ratio } => {
                entry = entry.child(Sx::list("roundrect_rratio").num(f64::from(*ratio) / 1000.0));
            }
            PadShape::Trapezoid { delta } => {
                entry = entry.child(Sx::list("rect_delta").mm(delta.w).mm(delta.h));
            }
            _ => {}
        }
        node = node.child(entry);
    }
    node
}

fn model_sx(model: &Model3d, bottom: bool) -> Sx {
    let mut offset = [model.offset.x, model.offset.y, model.offset.z].map(|v| v as f64 / 1e6);
    let mut rotate = model.rotation.map(ew_model::units::Angle::degrees);
    if bottom {
        offset = [-offset[0], -offset[1], offset[2]];
        rotate[2] = (rotate[2] + 180.0).rem_euclid(360.0);
    }
    let xyz = |values: [f64; 3]| Sx::list("xyz").num(values[0]).num(values[1]).num(values[2]);
    let mut node = Sx::list("model").text(model.path.clone());
    if model.hidden {
        node = node.child(Sx::flag("hide", true));
    }
    node.child(Sx::list("offset").child(xyz(offset)))
        .child(Sx::list("scale").child(xyz(model.scale.0)))
        .child(Sx::list("rotate").child(xyz(rotate)))
}

/// A footprint definition of a library as a `.kicad_mod` file.
#[must_use]
pub fn footprint_file(def: &FootprintDef, report: &mut ExportReport) -> String {
    let nets = BTreeMap::new();
    let node = footprint(
        &FootprintParts {
            name: &def.name,
            description: &def.description,
            keywords: &def.keywords,
            pads: &def.pads,
            graphics: &def.graphics,
            texts: &def.texts,
            models: &def.models,
            attributes: def.attributes,
        },
        None,
        &FootprintContext {
            copper_count: 2,
            nets: &nets,
        },
        report,
    );
    // Library files carry their format version after the name.
    let Sx::List(mut items) = node else {
        return String::new();
    };
    items.insert(2, Sx::pair("version", PCB_VERSION));
    items.insert(3, Sx::named("generator", "electronicwerk"));
    Sx::List(items).render()
}

fn stackup_sx(pcb: &Pcb) -> Sx {
    let mut node = Sx::list("stackup");
    let mut dielectric = 0;
    for layer in &pcb.stackup.layers {
        node = node.child(match layer {
            StackupLayer::Silkscreen { side, color } => {
                let mut sx = Sx::list("layer")
                    .text(if *side == Side::Top {
                        "F.SilkS"
                    } else {
                        "B.SilkS"
                    })
                    .child(Sx::named(
                        "type",
                        if *side == Side::Top {
                            "Top Silk Screen"
                        } else {
                            "Bottom Silk Screen"
                        },
                    ));
                if !color.is_empty() {
                    sx = sx.child(Sx::named("color", color.clone()));
                }
                sx
            }
            StackupLayer::SolderMask {
                side,
                thickness,
                color,
            } => {
                let mut sx = Sx::list("layer")
                    .text(if *side == Side::Top {
                        "F.Mask"
                    } else {
                        "B.Mask"
                    })
                    .child(Sx::named(
                        "type",
                        if *side == Side::Top {
                            "Top Solder Mask"
                        } else {
                            "Bottom Solder Mask"
                        },
                    ));
                if !color.is_empty() {
                    sx = sx.child(Sx::named("color", color.clone()));
                }
                sx.child(Sx::list("thickness").mm(*thickness))
            }
            StackupLayer::Copper {
                layer, thickness, ..
            } => Sx::list("layer")
                .text(copper_name(*layer))
                .child(Sx::named("type", "copper"))
                .child(Sx::list("thickness").mm(*thickness)),
            StackupLayer::Dielectric {
                kind,
                thickness,
                material,
                epsilon_r,
                loss_tangent,
            } => {
                dielectric += 1;
                Sx::list("layer")
                    .text(format!("dielectric {dielectric}"))
                    .child(Sx::named(
                        "type",
                        match kind {
                            DielectricKind::Core => "core",
                            DielectricKind::Prepreg => "prepreg",
                        },
                    ))
                    .child(Sx::list("thickness").mm(*thickness))
                    .child(Sx::named("material", material.clone()))
                    .child(Sx::list("epsilon_r").num(*epsilon_r))
                    .child(Sx::list("loss_tangent").num(*loss_tangent))
            }
        });
    }
    if !pcb.stackup.finish.is_empty() {
        node = node.child(Sx::named("copper_finish", pcb.stackup.finish.clone()));
    }
    node.child(Sx::flag("dielectric_constraints", false))
}

fn zone_sx(zone: &Zone, page: Page, nets: &BTreeMap<Id, (u32, String)>) -> Sx {
    let (code, name) = zone
        .net
        .and_then(|id| nets.get(&id))
        .cloned()
        .unwrap_or((0, String::new()));
    let mut layers = Sx::list("layers");
    for &layer in &zone.layers {
        layers = layers.text(copper_name(layer));
    }
    let mut node = Sx::list("zone")
        .child(Sx::list("net").word(code.to_string()))
        .child(Sx::named("net_name", name))
        .child(layers)
        .child(uuid(zone.id));
    if !zone.name.is_empty() {
        node = node.child(Sx::named("name", zone.name.clone()));
    }
    node = node.child(Sx::list("hatch").word("edge").num(0.5));
    if zone.priority > 0 {
        node = node.child(Sx::list("priority").word(zone.priority.to_string()));
    }
    if zone.teardrop {
        node = node.child(
            Sx::list("attr").child(Sx::list("teardrop").child(Sx::list("type").word("padvia"))),
        );
    }
    let mut connect = Sx::list("connect_pads");
    match zone.fill.pad_connection {
        ew_model::pcb::PadConnection::Solid => connect = connect.word("yes"),
        ew_model::pcb::PadConnection::None => connect = connect.word("no"),
        ew_model::pcb::PadConnection::Thermal => {}
    }
    node = node
        .child(connect.child(Sx::list("clearance").mm(zone.fill.clearance)))
        .child(Sx::list("min_thickness").mm(zone.fill.min_width))
        .child(Sx::flag("filled_areas_thickness", false));
    if let Some(keepout) = zone.keepout {
        let rule = |allowed: bool| if allowed { "not_allowed" } else { "allowed" };
        node = node.child(
            Sx::list("keepout")
                .child(Sx::pair("tracks", rule(keepout.tracks)))
                .child(Sx::pair("vias", rule(keepout.vias)))
                .child(Sx::pair("pads", rule(keepout.pads)))
                .child(Sx::pair("copperpour", rule(keepout.pours)))
                .child(Sx::pair("footprints", rule(keepout.footprints))),
        );
    }
    node = node.child(
        Sx::list("fill")
            .word("yes")
            .child(Sx::list("thermal_gap").mm(zone.fill.thermal_gap))
            .child(Sx::list("thermal_bridge_width").mm(zone.fill.thermal_spoke_width)),
    );
    let ring = |points: &[Point]| Sx::list("polygon").child(pts(points, &|p| page.xy(p)));
    node = node.child(ring(&zone.outline));
    for hole in &zone.holes {
        node = node.child(ring(hole));
    }
    // Fills with holes need KiCad's own fill; the others are kept.
    if zone.filled.iter().all(|area| area.holes.is_empty()) {
        for area in &zone.filled {
            node = node.child(
                Sx::list("filled_polygon")
                    .child(Sx::named("layer", copper_name(area.layer)))
                    .child(pts(&area.points, &|p| page.xy(p))),
            );
        }
    }
    node
}

fn shape_sx(shape: &BoardShape, page: Page, report: &mut ExportReport) -> Vec<Sx> {
    let Some(layer) = layer_name(shape.layer) else {
        report.note(
            "layer",
            format!(
                "drawings on the layer {} have no KiCad layer and were left out",
                shape.layer
            ),
        );
        return Vec::new();
    };
    let point = |head: &str, p: Point| {
        let (x, y) = page.kicad(p);
        Sx::list(head).mm(x).mm(y)
    };
    let xy_of = |p: Point| page.xy(p);
    let pieces = split_polyline(&shape.geometry);
    let single = pieces.len() == 1;
    pieces
        .iter()
        .map(|geometry| {
            graphic(
                "gr",
                geometry,
                shape.width,
                shape.filled,
                &layer,
                single.then_some(shape.id),
                &point,
                &xy_of,
            )
        })
        .collect()
}

/// The board as a `.kicad_pcb` file.
#[allow(clippy::too_many_lines, reason = "the sections of one file")]
#[must_use]
pub fn board_file(
    pcb: &Pcb,
    title: &super::Title<'_>,
    components: &dyn Fn(Id) -> Option<String>,
    report: &mut ExportReport,
) -> String {
    let copper = CopperLayer::stack(pcb.stackup.copper_count());
    let page = Page::around(board_bounds(pcb));
    let mut nets: BTreeMap<Id, (u32, String)> = BTreeMap::new();
    let mut by_name: Vec<(&str, Id)> = pcb.nets.iter().map(|n| (n.name.as_str(), n.id)).collect();
    by_name.sort_unstable();
    for (index, (name, id)) in by_name.into_iter().enumerate() {
        nets.insert(id, (u32::try_from(index + 1).unwrap_or(0), name.to_owned()));
    }

    let thickness: Nm = pcb
        .stackup
        .layers
        .iter()
        .map(|layer| match layer {
            StackupLayer::Copper { thickness, .. }
            | StackupLayer::Dielectric { thickness, .. }
            | StackupLayer::SolderMask { thickness, .. } => *thickness,
            StackupLayer::Silkscreen { .. } => 0,
        })
        .sum();
    let mut root = Sx::list("kicad_pcb")
        .child(Sx::pair("version", PCB_VERSION))
        .child(Sx::named("generator", "electronicwerk"))
        .child(Sx::named("generator_version", title.version))
        .child(
            Sx::list("general")
                .child(Sx::list("thickness").mm(thickness))
                .child(Sx::flag("legacy_teardrops", false)),
        )
        .child(page.sx())
        .child(title.block());

    let mut layers = Sx::list("layers");
    let names: BTreeMap<CopperLayer, &str> = pcb
        .stackup
        .layers
        .iter()
        .filter_map(|layer| match layer {
            StackupLayer::Copper { layer, name, .. } if !name.is_empty() => {
                Some((*layer, name.as_str()))
            }
            _ => None,
        })
        .collect();
    for &layer in &copper {
        let number = match layer {
            CopperLayer::Top => 0,
            CopperLayer::Inner(n) => u32::from(n),
            CopperLayer::Bottom => 31,
        };
        let mut entry = Sx::list(&number.to_string())
            .text(copper_name(layer))
            .word("signal");
        if let Some(name) = names.get(&layer) {
            entry = entry.text(*name);
        }
        layers = layers.child(entry);
    }
    for (number, name, user) in FIXED_LAYERS {
        let mut entry = Sx::list(&number.to_string()).text(*name).word("user");
        if !user.is_empty() {
            entry = entry.text(*user);
        }
        layers = layers.child(entry);
    }
    for n in 1..=9_u8 {
        layers = layers.child(
            Sx::list(&(49 + u32::from(n)).to_string())
                .text(format!("User.{n}"))
                .word("user"),
        );
    }
    root = root.child(layers);
    let (ox, oy) = page.kicad(pcb.origin);
    root = root.child(
        Sx::list("setup")
            .child(stackup_sx(pcb))
            .child(Sx::list("pad_to_mask_clearance").num(0.0))
            .child(Sx::flag("allow_soldermask_bridges_in_footprints", false))
            .child(Sx::list("aux_axis_origin").mm(ox).mm(oy))
            .child(Sx::list("grid_origin").mm(ox).mm(oy)),
    );
    root = root.child(Sx::list("net").word("0").text(""));
    for (code, name) in nets.values() {
        root = root.child(Sx::list("net").word(code.to_string()).text(name.clone()));
    }

    let context = FootprintContext {
        copper_count: pcb.stackup.copper_count(),
        nets: &nets,
    };
    let mut footprints: Vec<&FootprintInstance> = pcb.footprints.iter().collect();
    footprints.sort_by(|a, b| natural(&a.designator, &b.designator));
    for fp in footprints {
        let name = if fp.footprint.contains(':') {
            fp.footprint.clone()
        } else {
            format!("{}:{}", title.library, fp.footprint)
        };
        root = root.child(footprint(
            &FootprintParts {
                name: &name,
                description: "",
                keywords: "",
                pads: &fp.pads,
                graphics: &fp.graphics,
                texts: &fp.texts,
                models: &fp.models,
                attributes: fp.attributes,
            },
            Some(&Placement {
                id: fp.id,
                at: fp.at,
                rotation: fp.rotation.degrees(),
                side: fp.side,
                locked: fp.locked,
                designator: &fp.designator,
                value: &fp.value,
                properties: &fp.properties,
                page,
                path: fp.component.and_then(components),
            }),
            &context,
            report,
        ));
    }
    for shape in &pcb.shapes {
        root = root.children(shape_sx(shape, page, report));
    }
    for text in &pcb.texts {
        let Some(layer) = layer_name(text.layer) else {
            report.note(
                "layer",
                format!("a text on the layer {} was left out", text.layer),
            );
            continue;
        };
        root = root.child(
            Sx::list("gr_text")
                .text(text.text.clone())
                .child(page.at(text.at, Some(text.rotation.degrees())))
                .child(Sx::named("layer", layer))
                .child(uuid(text.id))
                .child(effects(&text.style, false, text.mirrored)),
        );
    }
    for dim in &pcb.dimensions {
        let Some(layer) = layer_name(dim.layer) else {
            report.note(
                "layer",
                format!("a dimension on the layer {} was left out", dim.layer),
            );
            continue;
        };
        root = root.child(dimension_sx(dim, &layer, page));
    }
    let net_of = |net: Option<Id>| -> Sx {
        let code = net
            .and_then(|id| nets.get(&id))
            .map_or(0, |(code, _)| *code);
        Sx::list("net").word(code.to_string())
    };
    for track in &pcb.tracks {
        let (sx, sy) = page.kicad(track.start);
        let (ex, ey) = page.kicad(track.end);
        let mut node = match track.mid {
            Some(mid) => {
                let (mx, my) = page.kicad(mid);
                Sx::list("arc")
                    .child(Sx::list("start").mm(sx).mm(sy))
                    .child(Sx::list("mid").mm(mx).mm(my))
                    .child(Sx::list("end").mm(ex).mm(ey))
            }
            None => Sx::list("segment")
                .child(Sx::list("start").mm(sx).mm(sy))
                .child(Sx::list("end").mm(ex).mm(ey)),
        };
        node = node
            .child(Sx::list("width").mm(track.width))
            .child(Sx::named("layer", copper_name(track.layer)));
        if track.locked {
            node = node.child(Sx::flag("locked", true));
        }
        root = root.child(node.child(net_of(track.net)).child(uuid(track.id)));
    }
    for via in &pcb.vias {
        let mut node = Sx::list("via");
        match via.via_type() {
            ViaType::Micro => node = node.word("micro"),
            ViaType::Blind | ViaType::Buried => node = node.word("blind"),
            ViaType::Through => {}
        }
        if via.locked {
            node = node.child(Sx::flag("locked", true));
        }
        node = node
            .child(page.at(via.at, None))
            .child(Sx::list("size").mm(via.diameter))
            .child(Sx::list("drill").mm(via.drill))
            .child(
                Sx::list("layers")
                    .text(copper_name(via.from))
                    .text(copper_name(via.to)),
            );
        if let Some(fill) = via.fill {
            node = via_fill_sx(node, fill);
        }
        root = root.child(node.child(net_of(via.net)).child(uuid(via.id)));
    }
    for zone in &pcb.zones {
        root = root.child(zone_sx(zone, page, &nets));
    }
    root.render()
}

/// The IPC-4761 protection of a via as KiCad 9 writes it (checked with
/// KiCad's own writer).
fn via_fill_sx(mut node: Sx, fill: ViaFill) -> Sx {
    let sides = |head: &str| {
        Sx::list(head)
            .child(Sx::flag("front", true))
            .child(Sx::flag("back", true))
    };
    if matches!(fill, ViaFill::Tented | ViaFill::TentedCovered) {
        node = node.child(sides("tenting"));
    }
    if matches!(
        fill,
        ViaFill::TentedCovered | ViaFill::PluggedCovered | ViaFill::FilledCovered
    ) {
        node = node.child(sides("covering"));
    }
    if matches!(fill, ViaFill::Plugged | ViaFill::PluggedCovered) {
        node = node.child(sides("plugging"));
    }
    if fill == ViaFill::FilledCapped {
        node = node.child(Sx::flag("capping", true));
    }
    if matches!(
        fill,
        ViaFill::Filled | ViaFill::FilledCovered | ViaFill::FilledCapped
    ) {
        node = node.child(Sx::flag("filling", true));
    }
    node
}

/// The box around every position of a board (for the page size).
/// A dimension as KiCad writes it (KiCad 8). KiCad's `height` puts the
/// dimension line on the side of the rotated direction in its y-down page:
/// measured with kicad-cli, a positive height moves a horizontal or aligned
/// dimension line down and a vertical one to the right.
fn dimension_sx(dim: &ew_model::dimension::Dimension, layer: &str, page: Page) -> Sx {
    use ew_model::dimension::DimensionKind;
    use ew_model::project::Units;
    let drawing = ew_model::dimension::draw(dim, dim.units.unwrap_or(Units::Mm));
    let mut start = dim.start;
    let kind = match dim.kind {
        DimensionKind::Aligned | DimensionKind::Diameter => "aligned",
        DimensionKind::Horizontal | DimensionKind::Vertical => "orthogonal",
        DimensionKind::Radial => "radial",
        DimensionKind::Leader => "leader",
    };
    if dim.kind == DimensionKind::Diameter {
        // KiCad has no diameter dimension: an aligned one across the circle.
        start = dim.start - (dim.end - dim.start);
    }
    let mut node = Sx::list("dimension")
        .child(Sx::list("type").word(kind))
        .child(Sx::named("layer", layer))
        .child(uuid(dim.id))
        .child(
            Sx::list("pts")
                .child(page.xy(start))
                .child(page.xy(dim.end)),
        );
    match dim.kind {
        DimensionKind::Aligned | DimensionKind::Horizontal => {
            node = node.child(Sx::list("height").mm(-dim.offset));
        }
        DimensionKind::Vertical => node = node.child(Sx::list("height").mm(dim.offset)),
        DimensionKind::Radial => node = node.child(Sx::list("leader_length").mm(dim.offset)),
        DimensionKind::Diameter | DimensionKind::Leader => {}
    }
    if dim.kind == DimensionKind::Horizontal {
        node = node.child(Sx::list("orientation").word("0"));
    } else if dim.kind == DimensionKind::Vertical {
        node = node.child(Sx::list("orientation").word("1"));
    }
    node = node.child(
        Sx::list("gr_text")
            .text(drawing.text.clone())
            .child(page.at(drawing.at, Some(drawing.rotation)))
            .child(Sx::named("layer", layer))
            .child(effects(&dim.style, false, false)),
    );
    // The text: `{value}` splits into prefix and suffix; a text without it
    // overrides the value.
    let (prefix, suffix, fixed) = match dim.text.split_once("{value}") {
        Some((before, after)) => (before.to_owned(), after.to_owned(), None),
        None if dim.text.is_empty() => (String::new(), String::new(), None),
        None => (String::new(), String::new(), Some(dim.text.clone())),
    };
    let prefix = if dim.kind == DimensionKind::Diameter {
        format!("Ø{prefix}")
    } else {
        prefix
    };
    let units = match dim.units {
        Some(Units::Inch) => "0",
        Some(Units::Mil) => "1",
        Some(Units::Mm) => "2",
        None => "3",
    };
    let mut format = Sx::list("format")
        .child(Sx::list("prefix").text(prefix))
        .child(Sx::list("suffix").text(suffix))
        .child(Sx::list("units").word(units))
        .child(Sx::list("units_format").word(if dim.hide_units { "0" } else { "1" }))
        .child(Sx::list("precision").word(dim.precision.unwrap_or(2).to_string()));
    if let Some(fixed) = fixed
        && dim.kind != DimensionKind::Leader
    {
        format = format.child(Sx::list("override_value").text(fixed));
    }
    node.child(format).child(
        Sx::list("style")
            .child(Sx::list("thickness").mm(dim.width))
            .child(Sx::list("arrow_length").mm(dim.style.size))
            .child(Sx::list("text_position_mode").word("0"))
            .child(Sx::list("extension_height").mm(dim.style.size / 2))
            .child(Sx::list("extension_offset").mm(dim.style.size * 2 / 5))
            .word("keep_text_aligned"),
    )
}

fn board_bounds(pcb: &Pcb) -> Option<(Point, Point)> {
    let points = pcb
        .shapes
        .iter()
        .flat_map(|s| s.geometry.points())
        .chain(pcb.footprints.iter().map(|f| f.at))
        .chain(pcb.tracks.iter().flat_map(|t| [t.start, t.end]))
        .chain(pcb.vias.iter().map(|v| v.at))
        .chain(pcb.zones.iter().flat_map(|z| z.outline.iter().copied()));
    let rect = ew_model::units::Rect::from_points(points)?;
    Some((rect.min, rect.max))
}

/// Designators in natural order (`R2` before `R10`).
#[must_use]
pub fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let split = |s: &str| {
        let digits = s.trim_start_matches(|c: char| !c.is_ascii_digit());
        let prefix = &s[..s.len() - digits.len()];
        (
            prefix.to_owned(),
            digits.parse::<u64>().unwrap_or(0),
            s.to_owned(),
        )
    };
    split(a).cmp(&split(b))
}
