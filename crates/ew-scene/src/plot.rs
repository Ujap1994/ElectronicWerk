//! Vector primitives of a scene for prints (PDF and SVG): the same drawing
//! as the viewer's, with polygons kept as outlines instead of triangles.

/// A stroked line segment (round caps).
#[derive(Debug, Clone, PartialEq)]
pub struct PlotSegment {
    /// Start.
    pub a: [f64; 2],
    /// End.
    pub b: [f64; 2],
    /// Width.
    pub width: f64,
}

/// A stroked arc.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotArc {
    /// Centre.
    pub center: [f64; 2],
    /// Radius.
    pub radius: f64,
    /// Width.
    pub width: f64,
    /// Start angle (radians).
    pub start: f64,
    /// Sweep (radians, counter-clockwise when positive).
    pub sweep: f64,
}

/// A disc (`inner` = 0) or ring.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotCircle {
    /// Centre.
    pub center: [f64; 2],
    /// Outer radius.
    pub outer: f64,
    /// Inner radius.
    pub inner: f64,
}

/// A filled, rotated rectangle with rounded corners.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotRect {
    /// Centre.
    pub center: [f64; 2],
    /// Half width.
    pub half_w: f64,
    /// Half height.
    pub half_h: f64,
    /// Rotation (radians).
    pub angle: f64,
    /// Corner radius.
    pub corner: f64,
}

/// A filled polygon with holes.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotPolygon {
    /// Outline.
    pub outline: Vec<[f64; 2]>,
    /// Holes.
    pub holes: Vec<Vec<[f64; 2]>>,
    /// Drawn as a hatch pattern in the viewer (unfilled zones).
    pub hatched: bool,
}

/// A text.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotText {
    /// Text (may contain `\n`).
    pub text: String,
    /// Anchor.
    pub at: [f64; 2],
    /// Cap height.
    pub size: f64,
    /// Rotation in degrees.
    pub rotation: f64,
    /// −1 left, 0 centre, 1 right.
    pub h: i8,
    /// −1 bottom, 0 centre, 1 top.
    pub v: i8,
    /// Mirrored.
    pub mirror: bool,
    /// Bold.
    pub bold: bool,
    /// Italic.
    pub italic: bool,
}

/// One layer of a plot.
#[derive(Debug, Clone, PartialEq)]
pub struct PlotLayer {
    /// Layer key (`sch.wires`, `cu.top`, …).
    pub key: String,
    /// Layer group.
    pub group: &'static str,
    /// Side.
    pub side: Option<&'static str>,
    /// Draw order (back to front).
    pub z: i32,
    /// Visible by default.
    pub visible: bool,
    /// Segments.
    pub segments: Vec<PlotSegment>,
    /// Arcs.
    pub arcs: Vec<PlotArc>,
    /// Discs and rings.
    pub circles: Vec<PlotCircle>,
    /// Rectangles.
    pub rects: Vec<PlotRect>,
    /// Polygons.
    pub polygons: Vec<PlotPolygon>,
    /// Texts.
    pub texts: Vec<PlotText>,
}

/// The vector drawing of a sheet or board.
#[derive(Debug, Clone, PartialEq)]
pub struct Plot {
    /// `schematic` or `pcb`.
    pub kind: &'static str,
    /// Document name.
    pub name: String,
    /// Page size of a sheet.
    pub page: Option<[i64; 2]>,
    /// Bounds of the drawing `[min x, min y, max x, max y]`.
    pub bounds: Option<[f64; 4]>,
    /// Layers.
    pub layers: Vec<PlotLayer>,
}
