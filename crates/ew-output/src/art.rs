//! What a board layer looks like when it is plotted: flashed pads and vias,
//! lines and arcs, filled regions. Gerber files, drill maps and the PDF and
//! SVG prints of boards are made from these features.

use std::collections::HashMap;
use std::f64::consts::TAU;

use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, LayerSpec, Side};
use ew_model::pcb::{FootprintInstance, FootprintTextKind, Pad, PadKind, Pcb};
use ew_model::shape::Geometry;
use ew_model::text::{HAlign, TextStyle, VAlign};
use ew_model::units::Point;
use ew_pcb::geom::{self, Core, Shape, V, Xform};
use ew_pcb::items::{footprint_xform, pad_shape};

use crate::font::{self, TextSpec};

/// Tolerance for curves that are split into lines (Béziers), in nm.
const CURVE_TOLERANCE: f64 = 1_000.0;

/// What a feature is, for the Gerber aperture function attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Function {
    /// Surface-mount pad (copper defined).
    SmdPad,
    /// Through-hole component pad.
    ComponentPad,
    /// Edge connector contact.
    ConnectorPad,
    /// Copper of a non-plated hole.
    WasherPad,
    /// Via pad.
    ViaPad,
    /// Track.
    Conductor,
    /// Copper that is not part of a circuit (text, logos on copper).
    NonConductor,
    /// Board outline.
    Profile,
    /// Anything on non-copper layers.
    Other,
}

impl Function {
    /// The value of the `.AperFunction` attribute, if there is one.
    #[must_use]
    pub const fn gerber(self) -> Option<&'static str> {
        Some(match self {
            Self::SmdPad => "SMDPad,CuDef",
            Self::ComponentPad => "ComponentPad",
            Self::ConnectorPad => "ConnectorPad",
            Self::WasherPad => "WasherPad",
            Self::ViaPad => "ViaPad",
            Self::Conductor => "Conductor",
            Self::NonConductor => "NonConductor",
            Self::Profile => "Profile",
            Self::Other => return None,
        })
    }
}

/// Object attributes of a feature (Gerber X2 `.N`, `.P`, `.C`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Attributes {
    /// Net name; `Some("")` for copper without a net, `None` for features
    /// that are not copper.
    pub net: Option<String>,
    /// Designator and pad number of a pad.
    pub pin: Option<(String, String)>,
    /// Designator of the component the feature belongs to.
    pub component: Option<String>,
}

/// One plotted feature in board coordinates (nm).
#[derive(Debug, Clone)]
pub enum Feature {
    /// A pad or via: a shape placed at a point.
    Flash {
        /// Placement point.
        at: V,
        /// The shape (in board coordinates).
        shape: Shape,
        /// Function.
        function: Function,
        /// Attributes.
        attributes: Attributes,
    },
    /// A straight line drawn with a round pen.
    Line {
        /// Start.
        a: V,
        /// End.
        b: V,
        /// Pen width.
        width: f64,
        /// Function.
        function: Function,
        /// Attributes.
        attributes: Attributes,
    },
    /// A circular arc drawn with a round pen; start = end is a full circle.
    Arc {
        /// Start.
        start: V,
        /// End.
        end: V,
        /// Centre.
        center: V,
        /// Counter-clockwise from start to end.
        ccw: bool,
        /// Pen width.
        width: f64,
        /// Function.
        function: Function,
        /// Attributes.
        attributes: Attributes,
    },
    /// A filled area.
    Region {
        /// Outline.
        outline: Vec<V>,
        /// Holes.
        holes: Vec<Vec<V>>,
        /// Function.
        function: Function,
        /// Attributes.
        attributes: Attributes,
    },
}

impl Feature {
    /// The function of the feature.
    #[must_use]
    pub const fn function(&self) -> Function {
        match self {
            Self::Flash { function, .. }
            | Self::Line { function, .. }
            | Self::Arc { function, .. }
            | Self::Region { function, .. } => *function,
        }
    }

    /// The attributes of the feature.
    #[must_use]
    pub const fn attributes(&self) -> &Attributes {
        match self {
            Self::Flash { attributes, .. }
            | Self::Line { attributes, .. }
            | Self::Arc { attributes, .. }
            | Self::Region { attributes, .. } => attributes,
        }
    }
}

/// Plot options of board layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools, reason = "independent switches")]
pub struct ArtOptions {
    /// Vias are covered by solder mask (no mask opening).
    pub tent_vias: bool,
    /// Pads of parts that are not placed get no solder paste.
    pub paste_dnp: bool,
    /// Plot the value text of footprints on silkscreen layers.
    pub silk_values: bool,
    /// Plot the designators of footprints on silkscreen layers.
    pub silk_designators: bool,
}

impl Default for ArtOptions {
    fn default() -> Self {
        Self {
            tent_vias: true,
            paste_dnp: false,
            silk_values: true,
            silk_designators: true,
        }
    }
}

/// Expands text variables (`${REVISION}`, …).
pub type Resolver<'a> = &'a dyn Fn(&str) -> String;

/// Builds the features of board layers.
pub struct BoardArt<'a> {
    pcb: &'a Pcb,
    nets: HashMap<Id, &'a str>,
    copper_count: u8,
    stack: Vec<CopperLayer>,
    options: ArtOptions,
    resolve: Resolver<'a>,
}

impl std::fmt::Debug for BoardArt<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoardArt")
            .field("board", &self.pcb.name)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

/// The layer of a footprint item on the board (flipped for the bottom).
fn placed(fp: &FootprintInstance, layer: Layer, copper_count: u8) -> Layer {
    if fp.side == Side::Bottom {
        layer.flipped(copper_count)
    } else {
        layer
    }
}

/// `true` if a pad of a footprint is on a board layer.
#[must_use]
pub fn pad_on(fp: &FootprintInstance, pad: &Pad, layer: Layer, copper_count: u8) -> bool {
    let local = placed(fp, layer, copper_count);
    pad.layers.iter().any(|spec| match spec {
        LayerSpec::Layer(own) => *own == local,
        LayerSpec::AllCopper => layer.is_copper(),
        other => other.contains(layer),
    })
}

/// Text alignment as −1/0/1.
const fn align(style: &TextStyle) -> (i8, i8) {
    let h = match style.h_align {
        HAlign::Left => -1,
        HAlign::Center => 0,
        HAlign::Right => 1,
    };
    let v = match style.v_align {
        VAlign::Bottom => -1,
        VAlign::Center => 0,
        VAlign::Top => 1,
    };
    (h, v)
}

/// Normalizes a text rotation (degrees) into `(-90, 90]` so that text never
/// reads upside down (as in the viewer).
fn readable(degrees: f64) -> f64 {
    let mut d = degrees % 360.0;
    if d < 0.0 {
        d += 360.0;
    }
    if d > 90.0 + 1e-6 && d <= 270.0 + 1e-6 {
        d -= 180.0;
    } else if d > 270.0 {
        d -= 360.0;
    }
    d
}

/// The rotation of a text keeping its box but never upside down.
fn upright(rotation: f64, h: i8, v: i8) -> (f64, i8, i8) {
    let turned = readable(rotation);
    if ((turned - rotation).rem_euclid(360.0).round() as i64) % 360 == 180 {
        (turned, -h, -v)
    } else {
        (turned, h, v)
    }
}

/// The circle through three points: centre and whether start → mid → end
/// runs counter-clockwise.
fn circle_through(start: V, mid: V, end: V) -> Option<(V, bool)> {
    let (ax, ay, bx, by, cx, cy) = (start.x, start.y, mid.x, mid.y, end.x, end.y);
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-6 {
        return None;
    }
    let a2 = ax * ax + ay * ay;
    let b2 = bx * bx + by * by;
    let c2 = cx * cx + cy * cy;
    let center = V::new(
        (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / d,
        (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / d,
    );
    let ccw = (mid.x - start.x) * (end.y - start.y) - (mid.y - start.y) * (end.x - start.x) > 0.0;
    Some((center, ccw))
}

fn bezier(p0: V, p1: V, p2: V, p3: V) -> Vec<V> {
    let length = p0.dist(p1) + p1.dist(p2) + p2.dist(p3);
    let steps = ((length / CURVE_TOLERANCE).sqrt().ceil() as usize).clamp(4, 64);
    (0..=steps)
        .map(|i| {
            let t = i as f64 / steps as f64;
            let u = 1.0 - t;
            p0.scale(u * u * u)
                .add(p1.scale(3.0 * u * u * t))
                .add(p2.scale(3.0 * u * t * t))
                .add(p3.scale(t * t * t))
        })
        .collect()
}

impl<'a> BoardArt<'a> {
    /// Prepares the features of a board.
    #[must_use]
    pub fn new(pcb: &'a Pcb, options: ArtOptions, resolve: Resolver<'a>) -> Self {
        Self {
            pcb,
            nets: pcb.nets.iter().map(|n| (n.id, n.name.as_str())).collect(),
            copper_count: pcb.stackup.copper_count(),
            stack: pcb.stackup.copper_layers().collect(),
            options,
            resolve,
        }
    }

    /// The board.
    #[must_use]
    pub const fn pcb(&self) -> &'a Pcb {
        self.pcb
    }

    /// The copper layers from top to bottom.
    #[must_use]
    pub fn stack(&self) -> &[CopperLayer] {
        &self.stack
    }

    fn net_name(&self, net: Option<Id>) -> String {
        net.and_then(|id| self.nets.get(&id))
            .map_or_else(String::new, |name| (*name).to_owned())
    }

    fn copper_attributes(&self, net: Option<Id>) -> Attributes {
        Attributes {
            net: Some(self.net_name(net)),
            ..Attributes::default()
        }
    }

    /// Every feature of a layer, in drawing order (zones first).
    #[must_use]
    pub fn features(&self, layer: Layer) -> Vec<Feature> {
        let mut out = Vec::new();
        if let Layer::Copper(copper) = layer {
            if !self.stack.contains(&copper) {
                return out;
            }
            self.zones(copper, &mut out);
        }
        self.shapes(layer, &mut out);
        for fp in &self.pcb.footprints {
            self.footprint(fp, layer, &mut out);
        }
        if let Layer::Copper(copper) = layer {
            self.tracks(copper, &mut out);
            self.vias(Some(copper), &mut out);
        } else if let Layer::SolderMask(_) = layer
            && !self.options.tent_vias
        {
            self.vias(None, &mut out);
        }
        self.texts(layer, &mut out);
        out
    }

    fn zones(&self, copper: CopperLayer, out: &mut Vec<Feature>) {
        let mut zones: Vec<_> = self
            .pcb
            .zones
            .iter()
            .filter(|z| z.keepout.is_none())
            .collect();
        zones.sort_by_key(|z| z.priority);
        for zone in zones {
            for area in zone.filled.iter().filter(|a| a.layer == copper) {
                if area.points.len() < 3 {
                    continue;
                }
                out.push(Feature::Region {
                    outline: area.points.iter().map(|p| V::of(*p)).collect(),
                    holes: area
                        .holes
                        .iter()
                        .filter(|h| h.len() >= 3)
                        .map(|h| h.iter().map(|p| V::of(*p)).collect())
                        .collect(),
                    function: Function::Conductor,
                    attributes: self.copper_attributes(zone.net),
                });
            }
        }
    }

    fn tracks(&self, copper: CopperLayer, out: &mut Vec<Feature>) {
        for track in self.pcb.tracks.iter().filter(|t| t.layer == copper) {
            let (a, b) = (V::of(track.start), V::of(track.end));
            let width = track.width as f64;
            let attributes = self.copper_attributes(track.net);
            let arc = track.mid.and_then(|mid| circle_through(a, V::of(mid), b));
            out.push(match arc {
                Some((center, ccw)) => Feature::Arc {
                    start: a,
                    end: b,
                    center,
                    ccw,
                    width,
                    function: Function::Conductor,
                    attributes,
                },
                None => Feature::Line {
                    a,
                    b,
                    width,
                    function: Function::Conductor,
                    attributes,
                },
            });
        }
    }

    /// Vias on a copper layer, or (with `None`) their mask openings.
    fn vias(&self, copper: Option<CopperLayer>, out: &mut Vec<Feature>) {
        let index = |layer: CopperLayer| self.stack.iter().position(|own| *own == layer);
        for via in &self.pcb.vias {
            if let Some(copper) = copper {
                let (Some(here), Some(a), Some(b)) =
                    (index(copper), index(via.from), index(via.to))
                else {
                    continue;
                };
                if here < a.min(b) || here > a.max(b) {
                    continue;
                }
            }
            let at = V::of(via.at);
            out.push(Feature::Flash {
                at,
                shape: Shape::disc(at, via.diameter as f64 / 2.0),
                function: if copper.is_some() {
                    Function::ViaPad
                } else {
                    Function::Other
                },
                attributes: if copper.is_some() {
                    self.copper_attributes(via.net)
                } else {
                    Attributes::default()
                },
            });
        }
    }

    fn footprint(&self, fp: &FootprintInstance, layer: Layer, out: &mut Vec<Feature>) {
        let xf = footprint_xform(fp);
        for graphic in &fp.graphics {
            if placed(fp, graphic.layer, self.copper_count) != layer {
                continue;
            }
            let function = if layer.is_copper() {
                Function::NonConductor
            } else {
                Function::Other
            };
            let attributes = Attributes {
                component: Some(fp.designator.clone()),
                net: layer.is_copper().then(String::new),
                ..Attributes::default()
            };
            geometry(
                &graphic.geometry,
                &xf,
                graphic.width as f64,
                graphic.filled,
                function_for(layer, function),
                &attributes,
                out,
            );
        }
        for pad in &fp.pads {
            self.pad(fp, pad, &xf, layer, out);
        }
        for text in &fp.texts {
            if !text.visible || placed(fp, text.layer, self.copper_count) != layer {
                continue;
            }
            let silk = matches!(layer, Layer::Silkscreen(_));
            match text.kind {
                FootprintTextKind::Designator if silk && !self.options.silk_designators => continue,
                FootprintTextKind::Value if silk && !self.options.silk_values => continue,
                _ => {}
            }
            let content = match text.kind {
                FootprintTextKind::Designator if text.text.is_empty() => fp.designator.clone(),
                FootprintTextKind::Value if text.text.is_empty() => fp.value.clone(),
                _ => text
                    .text
                    .replace("${REFERENCE}", &fp.designator)
                    .replace("${VALUE}", &fp.value),
            };
            let content = if content.contains("${") {
                (self.resolve)(&content)
            } else {
                content
            };
            let local = geom::radians(text.rotation);
            let local = if xf.mirror { -local } else { local };
            let rotation = (local + xf.angle).rem_euclid(TAU).to_degrees();
            let (h, v) = align(&text.style);
            let (rotation, h, v) = upright(rotation, h, v);
            let at = xf.apply(V::of(text.at));
            let attributes = Attributes {
                component: Some(fp.designator.clone()),
                net: layer.is_copper().then(String::new),
                ..Attributes::default()
            };
            text_features(
                &content,
                at,
                rotation,
                (h, v),
                fp.side == Side::Bottom,
                &text.style,
                function_for(layer, Function::NonConductor),
                &attributes,
                out,
            );
        }
    }

    fn pad(
        &self,
        fp: &FootprintInstance,
        pad: &Pad,
        xf: &Xform,
        layer: Layer,
        out: &mut Vec<Feature>,
    ) {
        if !pad_on(fp, pad, layer, self.copper_count) {
            return;
        }
        let margin = match layer {
            Layer::Copper(_) => {
                if pad.kind == PadKind::NpThroughHole
                    && pad
                        .drill
                        .is_some_and(|d| pad.size.w <= d.diameter && pad.size.h <= d.diameter)
                {
                    // Only a hole, no copper ring.
                    return;
                }
                0.0
            }
            Layer::SolderMask(_) => pad.mask_margin.unwrap_or(0) as f64,
            Layer::Paste(_) => {
                if fp.attributes.dnp && !self.options.paste_dnp {
                    return;
                }
                pad.paste_margin.unwrap_or(0) as f64
            }
            _ => 0.0,
        };
        let shape = pad_shape(pad, xf, margin);
        if shape.radius <= 0.0 && !matches!(shape.core, Core::Poly(_)) {
            return;
        }
        let at = xf.apply(V::of(pad.at));
        let (function, attributes) = if layer.is_copper() {
            let function = match pad.kind {
                PadKind::Smd => Function::SmdPad,
                PadKind::ThroughHole => Function::ComponentPad,
                PadKind::NpThroughHole => Function::WasherPad,
                PadKind::Connector => Function::ConnectorPad,
            };
            let net = match pad.net {
                Some(_) => self.net_name(pad.net),
                None => "N/C".to_owned(),
            };
            (
                function,
                Attributes {
                    net: Some(net),
                    pin: Some((fp.designator.clone(), pad.number.clone())),
                    component: None,
                },
            )
        } else {
            (Function::Other, Attributes::default())
        };
        out.push(Feature::Flash {
            at,
            shape,
            function,
            attributes,
        });
    }

    fn shapes(&self, layer: Layer, out: &mut Vec<Feature>) {
        for shape in self.pcb.shapes.iter().filter(|s| s.layer == layer) {
            let attributes = Attributes {
                net: layer.is_copper().then(String::new),
                ..Attributes::default()
            };
            let function = if layer == Layer::Outline {
                Function::Profile
            } else {
                function_for(layer, Function::NonConductor)
            };
            geometry(
                &shape.geometry,
                &Xform::IDENTITY,
                shape.width as f64,
                shape.filled,
                function,
                &attributes,
                out,
            );
        }
    }

    fn texts(&self, layer: Layer, out: &mut Vec<Feature>) {
        for text in self.pcb.texts.iter().filter(|t| t.layer == layer) {
            let content = if text.text.contains("${") {
                (self.resolve)(&text.text)
            } else {
                text.text.clone()
            };
            let attributes = Attributes {
                net: layer.is_copper().then(String::new),
                ..Attributes::default()
            };
            text_features(
                &content,
                V::of(text.at),
                text.rotation.degrees(),
                align(&text.style),
                text.mirrored,
                &text.style,
                function_for(layer, Function::NonConductor),
                &attributes,
                out,
            );
        }
    }
}

/// Copper functions only exist on copper layers.
const fn function_for(layer: Layer, copper: Function) -> Function {
    if layer.is_copper() {
        copper
    } else {
        Function::Other
    }
}

/// The features of a drawn geometry.
#[allow(clippy::too_many_lines, reason = "one arm per geometry kind")]
pub fn geometry(
    geometry: &Geometry,
    xf: &Xform,
    width: f64,
    filled: bool,
    function: Function,
    attributes: &Attributes,
    out: &mut Vec<Feature>,
) {
    let p = |point: Point| xf.apply(V::of(point));
    let line = |a: V, b: V, out: &mut Vec<Feature>| {
        if width > 0.0 {
            out.push(Feature::Line {
                a,
                b,
                width,
                function,
                attributes: attributes.clone(),
            });
        }
    };
    let closed = |ring: Vec<V>, out: &mut Vec<Feature>| {
        if filled && ring.len() >= 3 {
            out.push(Feature::Region {
                outline: ring.clone(),
                holes: Vec::new(),
                function,
                attributes: attributes.clone(),
            });
        }
        if width > 0.0 {
            for i in 0..ring.len() {
                line(ring[i], ring[(i + 1) % ring.len()], out);
            }
        }
    };
    match geometry {
        Geometry::Line { start, end } => line(p(*start), p(*end), out),
        Geometry::Rect { start, end } => {
            let ring = vec![
                p(Point::new(start.x, start.y)),
                p(Point::new(end.x, start.y)),
                p(Point::new(end.x, end.y)),
                p(Point::new(start.x, end.y)),
            ];
            closed(ring, out);
        }
        Geometry::Circle { center, radius } => {
            let c = p(*center);
            let r = *radius as f64;
            if filled {
                out.push(Feature::Flash {
                    at: c,
                    shape: Shape::disc(c, r + width / 2.0),
                    function,
                    attributes: attributes.clone(),
                });
            } else if width > 0.0 {
                let start = c.add(V::new(r, 0.0));
                out.push(Feature::Arc {
                    start,
                    end: start,
                    center: c,
                    ccw: true,
                    width,
                    function,
                    attributes: attributes.clone(),
                });
            }
        }
        Geometry::Arc { start, mid, end } => {
            let (a, m, b) = (p(*start), p(*mid), p(*end));
            match circle_through(a, m, b) {
                Some((center, ccw)) if width > 0.0 => out.push(Feature::Arc {
                    start: a,
                    end: b,
                    center,
                    ccw,
                    width,
                    function,
                    attributes: attributes.clone(),
                }),
                _ => line(a, b, out),
            }
        }
        Geometry::Polyline { points } => {
            let points: Vec<V> = points.iter().map(|q| p(*q)).collect();
            if filled && points.len() >= 3 {
                out.push(Feature::Region {
                    outline: points.clone(),
                    holes: Vec::new(),
                    function,
                    attributes: attributes.clone(),
                });
            }
            for pair in points.windows(2) {
                line(pair[0], pair[1], out);
            }
        }
        Geometry::Polygon { points } => {
            closed(points.iter().map(|q| p(*q)).collect(), out);
        }
        Geometry::Bezier {
            start,
            control1,
            control2,
            end,
        } => {
            let points = bezier(p(*start), p(*control1), p(*control2), p(*end));
            for pair in points.windows(2) {
                line(pair[0], pair[1], out);
            }
        }
    }
}

/// The strokes of a text as line features.
#[allow(clippy::too_many_arguments, reason = "plain text parameters")]
pub fn text_features(
    text: &str,
    at: V,
    rotation: f64,
    (h, v): (i8, i8),
    mirror: bool,
    style: &TextStyle,
    function: Function,
    attributes: &Attributes,
    out: &mut Vec<Feature>,
) {
    let size = style.size as f64;
    if size <= 0.0 || text.trim().is_empty() {
        return;
    }
    let width = if style.thickness > 0 {
        style.thickness as f64
    } else {
        font::default_thickness(size, style.bold)
    };
    let spec = TextSpec {
        text,
        at: [at.x, at.y],
        size,
        rotation,
        h,
        v,
        mirror,
        italic: style.italic,
    };
    for stroke in font::strokes(&spec) {
        for pair in stroke.windows(2) {
            out.push(Feature::Line {
                a: V::new(pair[0][0], pair[0][1]),
                b: V::new(pair[1][0], pair[1][1]),
                width,
                function,
                attributes: attributes.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arcs_through_three_points() {
        let (c, ccw) =
            circle_through(V::new(1.0, 0.0), V::new(0.0, 1.0), V::new(-1.0, 0.0)).unwrap();
        assert!(c.len() < 1e-9 && ccw);
        let (_, ccw) =
            circle_through(V::new(-1.0, 0.0), V::new(0.0, 1.0), V::new(1.0, 0.0)).unwrap();
        assert!(!ccw);
        assert!(circle_through(V::new(0.0, 0.0), V::new(1.0, 0.0), V::new(2.0, 0.0)).is_none());
    }

    #[test]
    fn texts_turn_upright() {
        assert_eq!(upright(180.0, -1, -1), (0.0, 1, 1));
        assert_eq!(upright(90.0, -1, 0), (90.0, -1, 0));
        let (r, _, _) = upright(270.0, 0, 0);
        assert!((r - 90.0).abs() < 1e-9);
    }
}
