//! Footprint wizards: IPC-7351-style land patterns for the common package
//! families. Lands follow the IPC formulas without tolerances:
//!
//! - outer land extent `Z = L + 2·toe`,
//! - inner gap `G = L − 2·T − 2·heel`,
//! - land width `X = W + 2·side`,
//!
//! with toe, heel and side fillets per density level (least, nominal,
//! most), rounded to 1 µm (courtyards to 0.01 mm). Every footprint gets a fabrication outline with a pin-1 mark, a
//! silkscreen that keeps clear of the pads, a courtyard around everything
//! and designator and value texts.

#![allow(
    clippy::many_single_char_names,
    reason = "the names of the IPC formulas (Z, G, X)"
)]

use std::collections::BTreeMap;

use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, LayerSpec, Side};
use ew_model::library::FootprintDef;
use ew_model::pcb::{
    Drill, FootprintAttributes, FootprintText, FootprintTextKind, Graphic, Mounting, Pad, PadKind,
    PadShape,
};
use ew_model::shape::Geometry;
use ew_model::text::TextStyle;
use ew_model::units::{Angle, Nm, Point, Size};
use serde::{Deserialize, Serialize};

/// IPC-7351 density level: how much solder fillet the lands allow for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Density {
    /// Smallest lands (dense boards).
    Least,
    /// The usual choice.
    #[default]
    Nominal,
    /// Largest lands (hand soldering, rugged boards).
    Most,
}

impl Density {
    const fn pick(self, values: [f64; 3]) -> f64 {
        match self {
            Self::Most => values[0],
            Self::Nominal => values[1],
            Self::Least => values[2],
        }
    }
}

/// A package to generate. All lengths are nanometres.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Wizard {
    /// Two-terminal chip (resistors, capacitors, inductors: 0402 … 2512).
    #[serde(rename_all = "camelCase")]
    Chip {
        /// Body length (between the terminal ends).
        body_length: Nm,
        /// Body width.
        body_width: Nm,
        /// Terminal length.
        terminal: Nm,
        /// Density level.
        #[serde(default)]
        density: Density,
    },
    /// Gull-wing leads on two sides (SOIC, SSOP, TSSOP) or four (QFP).
    #[serde(rename_all = "camelCase")]
    GullWing {
        /// Number of pins.
        pins: u32,
        /// Leads on all four sides (QFP).
        #[serde(default)]
        quad: bool,
        /// Pin pitch.
        pitch: Nm,
        /// Lead span from tip to tip.
        lead_span: Nm,
        /// Length of the lead foot on the board.
        lead_length: Nm,
        /// Lead width.
        lead_width: Nm,
        /// Body size across the rows (x).
        body_width: Nm,
        /// Body size along the rows (y).
        body_length: Nm,
        /// Density level.
        #[serde(default)]
        density: Density,
    },
    /// Leadless packages with terminals under the edge (QFN on four sides,
    /// DFN on two), optionally with an exposed pad.
    #[serde(rename_all = "camelCase")]
    NoLead {
        /// Number of terminals (without the exposed pad).
        pins: u32,
        /// Terminals on all four sides (QFN).
        #[serde(default)]
        quad: bool,
        /// Pin pitch.
        pitch: Nm,
        /// Body size (x).
        body_width: Nm,
        /// Body size (y).
        body_length: Nm,
        /// Terminal length.
        terminal_length: Nm,
        /// Terminal width.
        terminal_width: Nm,
        /// Exposed pad size, if any.
        #[serde(default)]
        exposed_pad: Option<Size>,
        /// Density level.
        #[serde(default)]
        density: Density,
    },
    /// Ball grid array.
    #[serde(rename_all = "camelCase")]
    Bga {
        /// Rows (A, B, …).
        rows: u32,
        /// Columns (1, 2, …).
        columns: u32,
        /// Ball pitch.
        pitch: Nm,
        /// Ball diameter.
        ball: Nm,
        /// Body size (x).
        body_width: Nm,
        /// Body size (y).
        body_length: Nm,
        /// Density level.
        #[serde(default)]
        density: Density,
    },
    /// Through-hole pin header (one or two rows).
    #[serde(rename_all = "camelCase")]
    Header {
        /// Rows (1 or 2).
        rows: u32,
        /// Pins per row.
        pins_per_row: u32,
        /// Pin pitch.
        pitch: Nm,
        /// Drill diameter.
        drill: Nm,
        /// Pad diameter.
        pad: Nm,
    },
}

/// Why a wizard cannot make a footprint.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct WizardError(pub String);

fn invalid(message: impl Into<String>) -> WizardError {
    WizardError(message.into())
}

/// Millimetres (f64) to nanometres, rounded to 1 µm.
fn nm(mm: f64) -> Nm {
    ((mm * 1_000.0).round() * 1_000.0) as Nm
}

/// Nanometres to millimetres.
fn mm(value: Nm) -> f64 {
    value as f64 / 1_000_000.0
}

fn p(x: f64, y: f64) -> Point {
    Point::new(nm(x), nm(y))
}

/// Millimetres for names (`1.27`, `0.5`, `3.9`).
fn fmt(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

const FAB_WIDTH: f64 = 0.1;
const SILK_WIDTH: f64 = 0.12;
const COURTYARD_WIDTH: f64 = 0.05;
/// Silkscreen clearance from pads.
const SILK_CLEARANCE: f64 = 0.2;

fn smd_layers() -> Vec<LayerSpec> {
    vec![
        LayerSpec::Layer(Layer::Copper(CopperLayer::Top)),
        LayerSpec::Layer(Layer::Paste(Side::Top)),
        LayerSpec::Layer(Layer::SolderMask(Side::Top)),
    ]
}

/// An axis-aligned box in millimetres.
#[derive(Debug, Clone, Copy)]
struct Bounds {
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
}

impl Bounds {
    const fn centred(half_x: f64, half_y: f64) -> Self {
        Self {
            min_x: -half_x,
            min_y: -half_y,
            max_x: half_x,
            max_y: half_y,
        }
    }

    fn union(self, other: Self) -> Self {
        Self {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    fn grow(self, by: f64) -> Self {
        Self {
            min_x: self.min_x - by,
            min_y: self.min_y - by,
            max_x: self.max_x + by,
            max_y: self.max_y + by,
        }
    }

    /// Rounded outwards to 0.01 mm (courtyards on a clean grid).
    fn rounded_out(self) -> Self {
        let down = |v: f64| (v * 100.0).floor() / 100.0;
        let up = |v: f64| (v * 100.0).ceil() / 100.0;
        Self {
            min_x: down(self.min_x),
            min_y: down(self.min_y),
            max_x: up(self.max_x),
            max_y: up(self.max_y),
        }
    }
}

/// A footprint being built.
struct Builder {
    pads: Vec<Pad>,
    graphics: Vec<Graphic>,
    extent: Option<Bounds>,
}

impl Builder {
    const fn new() -> Self {
        Self {
            pads: Vec::new(),
            graphics: Vec::new(),
            extent: None,
        }
    }

    fn include(&mut self, bounds: Bounds) {
        self.extent = Some(self.extent.map_or(bounds, |e| e.union(bounds)));
    }

    /// An SMD land, size `w × h` at (x, y).
    fn smd(&mut self, number: &str, x: f64, y: f64, w: f64, h: f64, shape: PadShape) {
        self.pads.push(Pad {
            id: Id::random(),
            number: number.to_owned(),
            net: None,
            kind: PadKind::Smd,
            shape,
            at: p(x, y),
            rotation: Angle::ZERO,
            size: Size::new(nm(w), nm(h)),
            drill: None,
            layers: smd_layers(),
            clearance: None,
            mask_margin: None,
            paste_margin: None,
        });
        self.include(Bounds {
            min_x: x - w / 2.0,
            min_y: y - h / 2.0,
            max_x: x + w / 2.0,
            max_y: y + h / 2.0,
        });
    }

    fn tht(&mut self, number: &str, x: f64, y: f64, pad: f64, drill: f64, shape: PadShape) {
        self.pads.push(Pad {
            id: Id::random(),
            number: number.to_owned(),
            net: None,
            kind: PadKind::ThroughHole,
            shape,
            at: p(x, y),
            rotation: Angle::ZERO,
            size: Size::new(nm(pad), nm(pad)),
            drill: Some(Drill {
                diameter: nm(drill),
                slot_length: None,
                offset: Point::ORIGIN,
            }),
            layers: vec![LayerSpec::AllCopper, LayerSpec::AllMask],
            clearance: None,
            mask_margin: None,
            paste_margin: None,
        });
        self.include(Bounds::centred(pad / 2.0, pad / 2.0).shifted(x, y));
    }

    fn line(&mut self, layer: Layer, width: f64, a: (f64, f64), b: (f64, f64)) {
        self.graphics.push(Graphic {
            layer,
            geometry: Geometry::Line {
                start: p(a.0, a.1),
                end: p(b.0, b.1),
            },
            width: nm(width),
            filled: false,
        });
    }

    fn polygon(&mut self, layer: Layer, width: f64, points: &[(f64, f64)]) {
        self.graphics.push(Graphic {
            layer,
            geometry: Geometry::Polygon {
                points: points.iter().map(|&(x, y)| p(x, y)).collect(),
            },
            width: nm(width),
            filled: false,
        });
    }

    fn rect(&mut self, layer: Layer, width: f64, b: Bounds) {
        self.graphics.push(Graphic {
            layer,
            geometry: Geometry::Rect {
                start: p(b.min_x, b.max_y),
                end: p(b.max_x, b.min_y),
            },
            width: nm(width),
            filled: false,
        });
    }

    /// A fabrication outline with the pin-1 corner (top left) cut off.
    fn fab_body(&mut self, half_x: f64, half_y: f64) {
        let chamfer = (half_x.min(half_y) * 0.5).min(1.0);
        self.polygon(
            Layer::Fabrication(Side::Top),
            FAB_WIDTH,
            &[
                (-half_x + chamfer, half_y),
                (half_x, half_y),
                (half_x, -half_y),
                (-half_x, -half_y),
                (-half_x, half_y - chamfer),
            ],
        );
        self.include(Bounds::centred(half_x, half_y));
    }

    /// Silkscreen corner marks of a body (each leg stops `clear` away from
    /// pads; legs shorter than 0.2 mm are left out).
    fn silk_corners(&mut self, half_x: f64, half_y: f64, leg_x: f64, leg_y: f64) {
        let silk = Layer::Silkscreen(Side::Top);
        let (x, y) = (half_x + 0.11, half_y + 0.11);
        for (sx, sy) in [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
            if leg_x >= 0.2 {
                self.line(
                    silk,
                    SILK_WIDTH,
                    (sx * x, sy * y),
                    (sx * (x - leg_x), sy * y),
                );
            }
            if leg_y >= 0.2 {
                self.line(
                    silk,
                    SILK_WIDTH,
                    (sx * x, sy * y),
                    (sx * x, sy * (y - leg_y)),
                );
            }
        }
    }

    /// The courtyard around everything, `excess` beyond pads and body.
    fn courtyard(&mut self, excess: f64) -> Bounds {
        let bounds = self
            .extent
            .unwrap_or_else(|| Bounds::centred(0.5, 0.5))
            .grow(excess)
            .rounded_out();
        self.rect(Layer::Courtyard(Side::Top), COURTYARD_WIDTH, bounds);
        bounds
    }

    fn finish(
        self,
        name: &str,
        description: String,
        keywords: &str,
        mounting: Mounting,
        courtyard: Bounds,
    ) -> FootprintDef {
        let style = TextStyle::board();
        let gap = mm(style.size);
        FootprintDef {
            id: Id::random(),
            name: name.to_owned(),
            description,
            keywords: keywords.to_owned(),
            pads: self.pads,
            graphics: self.graphics,
            texts: vec![
                FootprintText {
                    kind: FootprintTextKind::Designator,
                    text: String::new(),
                    layer: Layer::Silkscreen(Side::Top),
                    at: p(0.0, courtyard.max_y + gap),
                    rotation: Angle::ZERO,
                    style,
                    visible: true,
                },
                FootprintText {
                    kind: FootprintTextKind::Value,
                    text: String::new(),
                    layer: Layer::Fabrication(Side::Top),
                    at: p(0.0, courtyard.min_y - gap),
                    rotation: Angle::ZERO,
                    style,
                    visible: true,
                },
            ],
            models: Vec::new(),
            attributes: FootprintAttributes {
                mounting,
                ..FootprintAttributes::default()
            },
        }
    }
}

impl Bounds {
    fn shifted(self, x: f64, y: f64) -> Self {
        Self {
            min_x: self.min_x + x,
            min_y: self.min_y + y,
            max_x: self.max_x + x,
            max_y: self.max_y + y,
        }
    }
}

/// The land length and centre from the outer extent `z` and inner gap `g`.
fn lands(z: f64, g: f64) -> (f64, f64) {
    ((z - g) / 2.0, (z + g) / 4.0)
}

/// Positions of `n` pins with `pitch`, centred, from the first (top or
/// left) to the last.
fn row(n: u32, pitch: f64) -> impl Iterator<Item = f64> {
    let first = f64::from(n.saturating_sub(1)) * pitch / 2.0;
    (0..n).map(move |i| first - f64::from(i) * pitch)
}

/// JEDEC row names: A … Z without I, O, Q, S, X, Z, then AA, AB, ….
fn bga_row(index: u32) -> String {
    const LETTERS: &[u8] = b"ABCDEFGHJKLMNPRTUVWY";
    let base = LETTERS.len() as u32;
    let mut name = String::new();
    let mut i = index;
    if i >= base {
        name.push(LETTERS[(i / base - 1) as usize % LETTERS.len()] as char);
        i %= base;
    }
    name.push(LETTERS[i as usize] as char);
    name
}

/// A footprint name in the style of the KiCad libraries.
#[must_use]
pub fn suggest_name(wizard: &Wizard) -> String {
    match *wizard {
        Wizard::Chip {
            body_length,
            body_width,
            ..
        } => {
            let (l, w) = (mm(body_length), mm(body_width));
            let imperial = format!(
                "{:02}{:02}",
                (l / 0.254).round() as u32,
                (w / 0.254).round() as u32
            );
            format!(
                "Chip_{imperial}_{:02}{:02}Metric",
                (l * 10.0).round() as u32,
                (w * 10.0).round() as u32
            )
        }
        Wizard::GullWing {
            pins,
            quad,
            pitch,
            body_width,
            body_length,
            ..
        } => {
            let family = if quad {
                "QFP"
            } else if mm(pitch) >= 1.27 {
                "SOIC"
            } else {
                "SSOP"
            };
            format!(
                "{family}-{pins}_{}x{}mm_P{}mm",
                fmt(mm(body_width)),
                fmt(mm(body_length)),
                fmt(mm(pitch))
            )
        }
        Wizard::NoLead {
            pins,
            quad,
            pitch,
            body_width,
            body_length,
            exposed_pad,
            ..
        } => {
            let family = if quad { "QFN" } else { "DFN" };
            let ep = exposed_pad.map_or_else(String::new, |s| {
                format!("_EP{}x{}mm", fmt(mm(s.w)), fmt(mm(s.h)))
            });
            let count = if exposed_pad.is_some() {
                format!("{pins}-1EP")
            } else {
                pins.to_string()
            };
            format!(
                "{family}-{count}_{}x{}mm_P{}mm{ep}",
                fmt(mm(body_width)),
                fmt(mm(body_length)),
                fmt(mm(pitch))
            )
        }
        Wizard::Bga {
            rows,
            columns,
            pitch,
            body_width,
            body_length,
            ..
        } => format!(
            "BGA-{}_{rows}x{columns}_{}x{}mm_P{}mm",
            rows * columns,
            fmt(mm(body_width)),
            fmt(mm(body_length)),
            fmt(mm(pitch))
        ),
        Wizard::Header {
            rows,
            pins_per_row,
            pitch,
            ..
        } => format!(
            "PinHeader_{rows}x{pins_per_row:02}_P{}mm_Vertical",
            fmt(mm(pitch))
        ),
    }
}

/// Generates a footprint.
///
/// # Errors
///
/// Fails for impossible dimensions (lands that overlap, no pins, …).
pub fn generate(wizard: &Wizard, name: &str) -> Result<FootprintDef, WizardError> {
    match *wizard {
        Wizard::Chip {
            body_length,
            body_width,
            terminal,
            density,
        } => chip(name, mm(body_length), mm(body_width), mm(terminal), density),
        Wizard::GullWing {
            pins,
            quad,
            pitch,
            lead_span,
            lead_length,
            lead_width,
            body_width,
            body_length,
            density,
        } => gull_wing(
            name,
            pins,
            quad,
            mm(pitch),
            (mm(lead_span), mm(lead_length), mm(lead_width)),
            (mm(body_width), mm(body_length)),
            density,
        ),
        Wizard::NoLead {
            pins,
            quad,
            pitch,
            body_width,
            body_length,
            terminal_length,
            terminal_width,
            exposed_pad,
            density,
        } => no_lead(
            name,
            pins,
            quad,
            mm(pitch),
            (mm(body_width), mm(body_length)),
            (mm(terminal_length), mm(terminal_width)),
            exposed_pad.map(|s| (mm(s.w), mm(s.h))),
            density,
        ),
        Wizard::Bga {
            rows,
            columns,
            pitch,
            ball,
            body_width,
            body_length,
            density,
        } => bga(
            name,
            (rows, columns),
            mm(pitch),
            mm(ball),
            (mm(body_width), mm(body_length)),
            density,
        ),
        Wizard::Header {
            rows,
            pins_per_row,
            pitch,
            drill,
            pad,
        } => header(name, rows, pins_per_row, mm(pitch), mm(drill), mm(pad)),
    }
}

fn positive(values: &[(f64, &str)]) -> Result<(), WizardError> {
    for (value, what) in values {
        if *value <= 0.0 {
            return Err(invalid(format!("{what} must be greater than 0")));
        }
    }
    Ok(())
}

const ROUND_RECT: PadShape = PadShape::RoundRect { ratio: 250 };

fn chip(
    name: &str,
    length: f64,
    width: f64,
    terminal: f64,
    density: Density,
) -> Result<FootprintDef, WizardError> {
    positive(&[
        (length, "the body length"),
        (width, "the body width"),
        (terminal, "the terminal length"),
    ])?;
    if terminal * 2.0 >= length {
        return Err(invalid("the terminals are longer than the body"));
    }
    // IPC-7351B tables 3-5 (0603 and larger) and 3-6 (smaller chips).
    let small = length < 1.55;
    let toe = density.pick(if small {
        [0.25, 0.15, 0.05]
    } else {
        [0.55, 0.35, 0.15]
    });
    let side = density.pick([0.05, 0.0, -0.05]);
    let excess = density.pick(if small {
        [0.2, 0.15, 0.1]
    } else {
        [0.5, 0.25, 0.1]
    });
    let z = length + 2.0 * toe;
    let g = length - 2.0 * terminal;
    let (land, centre) = lands(z, g);
    let x = width + 2.0 * side;
    let mut b = Builder::new();
    b.smd("1", -centre, 0.0, land, x, ROUND_RECT);
    b.smd("2", centre, 0.0, land, x, ROUND_RECT);
    b.rect(
        Layer::Fabrication(Side::Top),
        FAB_WIDTH,
        Bounds::centred(length / 2.0, width / 2.0),
    );
    b.include(Bounds::centred(length / 2.0, width / 2.0));
    // Silkscreen between the lands, if there is room.
    let inner = centre - land / 2.0 - SILK_CLEARANCE;
    let y = (x / 2.0).max(width / 2.0) + SILK_CLEARANCE;
    if inner > 0.15 {
        let silk = Layer::Silkscreen(Side::Top);
        b.line(silk, SILK_WIDTH, (-inner, y), (inner, y));
        b.line(silk, SILK_WIDTH, (-inner, -y), (inner, -y));
    }
    let courtyard = b.courtyard(excess);
    let description = format!(
        "Chip, body {} × {} mm, IPC-7351 {density:?} density",
        fmt(length),
        fmt(width)
    );
    Ok(b.finish(
        name,
        description,
        "chip resistor capacitor inductor",
        Mounting::Smd,
        courtyard,
    ))
}

/// Pin numbers and positions around a package, counterclockwise from the
/// top of the left side: `(number, x, y, horizontal)`.
fn perimeter(
    pins: u32,
    quad: bool,
    pitch: f64,
    centre_x: f64,
    centre_y: f64,
) -> Result<Vec<(String, f64, f64, bool)>, WizardError> {
    let sides = if quad { 4 } else { 2 };
    if pins == 0 || pins % sides != 0 {
        return Err(invalid(format!(
            "the number of pins must be a multiple of {sides}"
        )));
    }
    let n = pins / sides;
    let mut out = Vec::new();
    let mut number = 0;
    let mut push = |x: f64, y: f64, horizontal: bool| {
        number += 1;
        out.push((number.to_string(), x, y, horizontal));
    };
    // Left side, top to bottom.
    for y in row(n, pitch) {
        push(-centre_x, y, true);
    }
    if quad {
        // Bottom side, left to right.
        for x in row(n, pitch) {
            push(-x, -centre_y, false);
        }
    }
    // Right side, bottom to top.
    for y in row(n, pitch) {
        push(centre_x, -y, true);
    }
    if quad {
        // Top side, right to left.
        for x in row(n, pitch) {
            push(x, centre_y, false);
        }
    }
    Ok(out)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per package dimension"
)]
fn gull_wing(
    name: &str,
    pins: u32,
    quad: bool,
    pitch: f64,
    (span, foot, lead): (f64, f64, f64),
    (body_x, body_y): (f64, f64),
    density: Density,
) -> Result<FootprintDef, WizardError> {
    positive(&[
        (pitch, "the pitch"),
        (span, "the lead span"),
        (foot, "the lead length"),
        (lead, "the lead width"),
        (body_x, "the body width"),
        (body_y, "the body length"),
    ])?;
    // IPC-7351B table 3-2 (gull-wing leads).
    let toe = density.pick([0.55, 0.35, 0.15]);
    let heel = density.pick([0.45, 0.35, 0.25]);
    let side = if pitch > 0.625 {
        density.pick([0.05, 0.03, 0.01])
    } else {
        density.pick([0.01, -0.02, -0.04])
    };
    let excess = density.pick([0.5, 0.25, 0.1]);
    let z = span + 2.0 * toe;
    let g = span - 2.0 * foot - 2.0 * heel;
    if g <= 0.0 {
        return Err(invalid("the leads are longer than half the lead span"));
    }
    let (land, centre) = lands(z, g);
    let x = lead + 2.0 * side;
    if x >= pitch {
        return Err(invalid(
            "the lands touch: the lead width is too large for the pitch",
        ));
    }
    let mut b = Builder::new();
    for (number, px, py, horizontal) in perimeter(pins, quad, pitch, centre, centre)? {
        let (w, h) = if horizontal { (land, x) } else { (x, land) };
        let shape = if number == "1" {
            PadShape::Rect
        } else {
            ROUND_RECT
        };
        b.smd(&number, px, py, w, h, shape);
    }
    let (half_x, half_y) = (body_x / 2.0, body_y / 2.0);
    b.fab_body(half_x, half_y);
    let silk = Layer::Silkscreen(Side::Top);
    let n = if quad { pins / 4 } else { pins / 2 };
    let pins_half = f64::from(n.saturating_sub(1)) * pitch / 2.0 + x / 2.0 + SILK_CLEARANCE;
    if quad {
        let leg = half_x + 0.11 - pins_half;
        b.silk_corners(half_x, half_y, leg, leg);
        // Pin 1: the top-left corner line continues outwards above the
        // left row of lands.
        let y = half_y + 0.11;
        b.line(
            silk,
            SILK_WIDTH,
            (-half_x - 0.11, y),
            (-(centre + land / 2.0), y),
        );
    } else {
        let y = half_y.max(pins_half) + 0.11;
        b.line(silk, SILK_WIDTH, (-(centre + land / 2.0), y), (half_x, y));
        b.line(silk, SILK_WIDTH, (-half_x, -y), (half_x, -y));
    }
    let courtyard = b.courtyard(excess);
    let description = format!(
        "{} {pins} pins, pitch {} mm, body {} × {} mm, lead span {} mm, IPC-7351 {density:?} density",
        if quad { "QFP" } else { "Small outline" },
        fmt(pitch),
        fmt(body_x),
        fmt(body_y),
        fmt(span)
    );
    Ok(b.finish(
        name,
        description,
        if quad {
            "QFP gull wing"
        } else {
            "SOIC SSOP TSSOP gull wing"
        },
        Mounting::Smd,
        courtyard,
    ))
}

#[allow(
    clippy::too_many_arguments,
    reason = "one parameter per package dimension"
)]
fn no_lead(
    name: &str,
    pins: u32,
    quad: bool,
    pitch: f64,
    (body_x, body_y): (f64, f64),
    (terminal, terminal_width): (f64, f64),
    exposed: Option<(f64, f64)>,
    density: Density,
) -> Result<FootprintDef, WizardError> {
    positive(&[
        (pitch, "the pitch"),
        (body_x, "the body width"),
        (body_y, "the body length"),
        (terminal, "the terminal length"),
        (terminal_width, "the terminal width"),
    ])?;
    // IPC-7351B table 3-15 (no-lead, QFN/SON).
    let toe = density.pick([0.4, 0.3, 0.2]);
    let heel = density.pick([0.1, 0.0, -0.05]);
    let side = density.pick([0.05, 0.0, -0.05]);
    let excess = density.pick([0.5, 0.25, 0.1]);
    let x = terminal_width + 2.0 * side;
    if x >= pitch {
        return Err(invalid(
            "the lands touch: the terminal width is too large for the pitch",
        ));
    }
    let land = terminal + toe + heel;
    let centre_x = body_x / 2.0 + toe - land / 2.0;
    let centre_y = body_y / 2.0 + toe - land / 2.0;
    let mut b = Builder::new();
    let positions = perimeter(pins, quad, pitch, centre_x, centre_y)?;
    for (number, px, py, horizontal) in positions {
        let (w, h) = if horizontal { (land, x) } else { (x, land) };
        b.smd(&number, px, py, w, h, ROUND_RECT);
    }
    if let Some((ew, eh)) = exposed {
        let inner_x = centre_x - land / 2.0 - SILK_CLEARANCE;
        if ew / 2.0 >= inner_x || (quad && eh / 2.0 >= centre_y - land / 2.0 - SILK_CLEARANCE) {
            return Err(invalid(
                "the exposed pad is too large: it comes too close to the terminals",
            ));
        }
        b.smd(&(pins + 1).to_string(), 0.0, 0.0, ew, eh, PadShape::Rect);
        // A smaller paste opening (about 65 % of the area) against voids.
        if let Some(pad) = b.pads.last_mut() {
            pad.paste_margin = Some(-nm(ew.min(eh) * 0.1));
        }
    }
    let (half_x, half_y) = (body_x / 2.0, body_y / 2.0);
    b.fab_body(half_x, half_y);
    let n = if quad { pins / 4 } else { pins / 2 };
    let pins_half = f64::from(n.saturating_sub(1)) * pitch / 2.0 + x / 2.0 + SILK_CLEARANCE;
    let leg = half_x + 0.11 - pins_half;
    b.silk_corners(
        half_x,
        half_y,
        if quad { leg } else { half_x * 2.0 + 0.22 },
        if quad { leg } else { half_y + 0.11 - pins_half },
    );
    let courtyard = b.courtyard(excess);
    let description = format!(
        "{} {pins} terminals{}, pitch {} mm, body {} × {} mm, IPC-7351 {density:?} density",
        if quad { "QFN" } else { "DFN" },
        if exposed.is_some() {
            " and an exposed pad"
        } else {
            ""
        },
        fmt(pitch),
        fmt(body_x),
        fmt(body_y)
    );
    Ok(b.finish(
        name,
        description,
        if quad {
            "QFN no lead"
        } else {
            "DFN SON no lead"
        },
        Mounting::Smd,
        courtyard,
    ))
}

fn bga(
    name: &str,
    (rows, columns): (u32, u32),
    pitch: f64,
    ball: f64,
    (body_x, body_y): (f64, f64),
    density: Density,
) -> Result<FootprintDef, WizardError> {
    positive(&[
        (pitch, "the pitch"),
        (ball, "the ball diameter"),
        (body_x, "the body width"),
        (body_y, "the body length"),
    ])?;
    if rows == 0 || columns == 0 {
        return Err(invalid("a BGA needs at least one row and one column"));
    }
    // Lands 20 % smaller than the balls (IPC-7351B, non-solder-mask
    // defined); a larger reduction for the densest boards.
    let land = ball * density.pick([0.85, 0.8, 0.75]);
    if land >= pitch {
        return Err(invalid(
            "the lands touch: the balls are too large for the pitch",
        ));
    }
    let excess = density.pick([2.0, 1.0, 0.5]);
    let mut b = Builder::new();
    let ys: Vec<f64> = row(rows, pitch).collect();
    let xs: Vec<f64> = row(columns, pitch).map(|x| -x).collect();
    for (r, y) in ys.iter().enumerate() {
        for (c, x) in xs.iter().enumerate() {
            let number = format!("{}{}", bga_row(r as u32), c + 1);
            b.smd(&number, *x, *y, land, land, PadShape::Circle);
        }
    }
    let (half_x, half_y) = (body_x / 2.0, body_y / 2.0);
    b.fab_body(half_x, half_y);
    let leg = (half_x.min(half_y) * 0.3).max(0.5);
    b.silk_corners(half_x, half_y, leg, leg);
    // A1 mark outside the corner.
    b.line(
        Layer::Silkscreen(Side::Top),
        SILK_WIDTH,
        (-half_x - 0.4, half_y + 0.4),
        (-half_x - 0.4, half_y - leg),
    );
    let courtyard = b.courtyard(excess);
    let description = format!(
        "BGA {rows} × {columns} balls, pitch {} mm, ball {} mm, body {} × {} mm, IPC-7351 {density:?} density",
        fmt(pitch),
        fmt(ball),
        fmt(body_x),
        fmt(body_y)
    );
    Ok(b.finish(
        name,
        description,
        "BGA ball grid array",
        Mounting::Smd,
        courtyard,
    ))
}

fn header(
    name: &str,
    rows: u32,
    per_row: u32,
    pitch: f64,
    drill: f64,
    pad: f64,
) -> Result<FootprintDef, WizardError> {
    positive(&[
        (pitch, "the pitch"),
        (drill, "the drill"),
        (pad, "the pad diameter"),
    ])?;
    if !(1..=2).contains(&rows) || per_row == 0 {
        return Err(invalid("a header has one or two rows of at least one pin"));
    }
    if drill >= pad {
        return Err(invalid("the drill must be smaller than the pad"));
    }
    if pad >= pitch {
        return Err(invalid(
            "the pads touch: the pad is too large for the pitch",
        ));
    }
    let mut b = Builder::new();
    // Pin 1 at the origin, like KiCad's headers; rows go down.
    for i in 0..per_row {
        for r in 0..rows {
            let number = (i * rows + r + 1).to_string();
            let shape = if number == "1" {
                PadShape::Rect
            } else {
                PadShape::Oval
            };
            b.tht(
                &number,
                f64::from(r) * pitch,
                -f64::from(i) * pitch,
                pad,
                drill,
                shape,
            );
        }
    }
    let half = pitch / 2.0;
    let body = Bounds {
        min_x: -half,
        min_y: -f64::from(per_row - 1) * pitch - half,
        max_x: f64::from(rows - 1) * pitch + half,
        max_y: half,
    };
    let chamfer = half * 0.5;
    b.polygon(
        Layer::Fabrication(Side::Top),
        FAB_WIDTH,
        &[
            (body.min_x + chamfer, body.max_y),
            (body.max_x, body.max_y),
            (body.max_x, body.min_y),
            (body.min_x, body.min_y),
            (body.min_x, body.max_y - chamfer),
        ],
    );
    b.include(body);
    let silk = body.grow(0.11);
    b.rect(Layer::Silkscreen(Side::Top), SILK_WIDTH, silk);
    // Pin 1 mark: a short line left of the first pin.
    b.line(
        Layer::Silkscreen(Side::Top),
        SILK_WIDTH,
        (silk.min_x - 0.3, half),
        (silk.min_x - 0.3, -half),
    );
    let courtyard = b.courtyard(0.5);
    let description = format!(
        "Pin header {rows} × {per_row}, pitch {} mm, drill {} mm, pad {} mm",
        fmt(pitch),
        fmt(drill),
        fmt(pad)
    );
    Ok(b.finish(
        name,
        description,
        "pin header connector through hole",
        Mounting::ThroughHole,
        courtyard,
    ))
}

/// A named set of wizard parameters (the presets of the wizard dialog).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    /// Preset name (`0603`, `SOIC-8`, …).
    pub name: String,
    /// Parameters.
    pub wizard: Wizard,
}

const fn um(value: i64) -> Nm {
    value * 1_000
}

/// The presets: common packages with their JEDEC/EIA dimensions.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one table per package family")]
pub fn presets() -> Vec<Preset> {
    let density = Density::Nominal;
    let mut out = Vec::new();
    for (name, l, w, t) in [
        ("0402", 1_000, 500, 250),
        ("0603", 1_600, 800, 300),
        ("0805", 2_000, 1_250, 400),
        ("1206", 3_200, 1_600, 500),
        ("1210", 3_200, 2_500, 500),
        ("2010", 5_000, 2_500, 600),
        ("2512", 6_300, 3_200, 600),
    ] {
        out.push(Preset {
            name: name.to_owned(),
            wizard: Wizard::Chip {
                body_length: um(l),
                body_width: um(w),
                terminal: um(t),
                density,
            },
        });
    }
    for (name, pins, quad, pitch, span, foot, lead, bx, by) in [
        ("SOIC-8", 8, false, 1_270, 6_000, 835, 410, 3_900, 4_900),
        ("SOIC-14", 14, false, 1_270, 6_000, 835, 410, 3_900, 8_650),
        ("SOIC-16", 16, false, 1_270, 6_000, 835, 410, 3_900, 9_900),
        ("TSSOP-14", 14, false, 650, 6_400, 600, 245, 4_400, 5_000),
        ("TSSOP-20", 20, false, 650, 6_400, 600, 245, 4_400, 6_500),
        ("SSOP-20", 20, false, 650, 7_800, 750, 300, 5_300, 7_200),
        ("LQFP-32 7x7", 32, true, 800, 9_000, 600, 370, 7_000, 7_000),
        ("LQFP-48 7x7", 48, true, 500, 9_000, 600, 220, 7_000, 7_000),
        (
            "LQFP-64 10x10",
            64,
            true,
            500,
            12_000,
            600,
            220,
            10_000,
            10_000,
        ),
        (
            "LQFP-100 14x14",
            100,
            true,
            500,
            16_000,
            600,
            220,
            14_000,
            14_000,
        ),
    ] {
        out.push(Preset {
            name: name.to_owned(),
            wizard: Wizard::GullWing {
                pins,
                quad,
                pitch: um(pitch),
                lead_span: um(span),
                lead_length: um(foot),
                lead_width: um(lead),
                body_width: um(bx),
                body_length: um(by),
                density,
            },
        });
    }
    for (name, pins, quad, pitch, body, l, b, ep) in [
        ("DFN-8 3x3", 8, false, 650, 3_000, 400, 300, Some(1_650)),
        ("QFN-16 3x3", 16, true, 500, 3_000, 400, 250, Some(1_700)),
        ("QFN-20 4x4", 20, true, 500, 4_000, 400, 250, Some(2_600)),
        ("QFN-32 5x5", 32, true, 500, 5_000, 400, 250, Some(3_450)),
        ("QFN-48 7x7", 48, true, 500, 7_000, 400, 250, Some(5_150)),
    ] {
        out.push(Preset {
            name: name.to_owned(),
            wizard: Wizard::NoLead {
                pins,
                quad,
                pitch: um(pitch),
                body_width: um(body),
                body_length: um(body),
                terminal_length: um(l),
                terminal_width: um(b),
                exposed_pad: ep.map(|e| Size::new(um(e), um(e))),
                density,
            },
        });
    }
    for (name, n, pitch, ball, body) in [
        ("BGA-64 8x8 P0.8", 8, 800, 400, 7_000),
        ("BGA-100 10x10 P1.0", 10, 1_000, 500, 11_000),
        ("BGA-256 16x16 P1.0", 16, 1_000, 500, 17_000),
    ] {
        out.push(Preset {
            name: name.to_owned(),
            wizard: Wizard::Bga {
                rows: n,
                columns: n,
                pitch: um(pitch),
                ball: um(ball),
                body_width: um(body),
                body_length: um(body),
                density,
            },
        });
    }
    for (rows, pins) in [
        (1, 2),
        (1, 3),
        (1, 4),
        (1, 6),
        (1, 8),
        (1, 10),
        (2, 5),
        (2, 10),
        (2, 20),
    ] {
        out.push(Preset {
            name: format!("Pin header {rows}x{pins:02} 2.54 mm"),
            wizard: Wizard::Header {
                rows,
                pins_per_row: pins,
                pitch: um(2_540),
                drill: um(1_000),
                pad: um(1_700),
            },
        });
    }
    out
}

/// The pad count and pad numbers of a footprint, for tests and previews.
#[must_use]
pub fn pad_numbers(def: &FootprintDef) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for pad in &def.pads {
        *out.entry(pad.number.clone()).or_default() += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preset(name: &str) -> Wizard {
        presets()
            .into_iter()
            .find(|p| p.name == name)
            .unwrap()
            .wizard
    }

    fn pad<'a>(def: &'a FootprintDef, number: &str) -> &'a Pad {
        def.pads.iter().find(|p| p.number == number).unwrap()
    }

    #[test]
    fn chip_0603_follows_ipc() {
        let def = generate(&preset("0603"), "R_0603").unwrap();
        assert_eq!(def.pads.len(), 2);
        let one = pad(&def, "1");
        // Z = 1.6 + 2·0.35 = 2.3, G = 1.6 − 0.6 = 1.0 → land 0.65 at ±0.825.
        assert_eq!(one.at, Point::new(-825_000, 0));
        assert_eq!(one.size, Size::new(650_000, 800_000));
        assert!(
            def.graphics
                .iter()
                .any(|g| g.layer == Layer::Courtyard(Side::Top))
        );
        assert!(
            def.graphics
                .iter()
                .any(|g| g.layer == Layer::Silkscreen(Side::Top))
        );
        assert_eq!(suggest_name(&preset("0603")), "Chip_0603_1608Metric");
        // Least density: smaller lands.
        let Wizard::Chip {
            body_length,
            body_width,
            terminal,
            ..
        } = preset("0603")
        else {
            unreachable!()
        };
        let least = generate(
            &Wizard::Chip {
                body_length,
                body_width,
                terminal,
                density: Density::Least,
            },
            "x",
        )
        .unwrap();
        assert!(pad(&least, "1").size.w < one.size.w);
    }

    #[test]
    fn soic_numbering_runs_counterclockwise() {
        let def = generate(&preset("SOIC-8"), "SOIC-8").unwrap();
        assert_eq!(def.pads.len(), 8);
        let (one, four, five, eight) = (
            pad(&def, "1"),
            pad(&def, "4"),
            pad(&def, "5"),
            pad(&def, "8"),
        );
        assert!(one.at.x < 0 && one.at.y > 0, "pin 1 top left");
        assert!(four.at.x < 0 && four.at.y < 0, "pin 4 bottom left");
        assert!(five.at.x > 0 && five.at.y < 0, "pin 5 bottom right");
        assert!(eight.at.x > 0 && eight.at.y > 0, "pin 8 top right");
        assert_eq!(one.at.y - pad(&def, "2").at.y, 1_270_000);
        assert_eq!(one.shape, PadShape::Rect);
        assert_eq!(suggest_name(&preset("SOIC-8")), "SOIC-8_3.9x4.9mm_P1.27mm");
    }

    #[test]
    fn qfp_has_four_sides() {
        let def = generate(&preset("LQFP-48 7x7"), "LQFP-48").unwrap();
        assert_eq!(def.pads.len(), 48);
        let (left, bottom, right, top) = (
            pad(&def, "1"),
            pad(&def, "13"),
            pad(&def, "25"),
            pad(&def, "37"),
        );
        assert!(left.at.x < 0 && left.size.w > left.size.h);
        assert!(bottom.at.y < 0 && bottom.size.h > bottom.size.w);
        assert!(right.at.x > 0);
        assert!(top.at.y > 0);
    }

    #[test]
    fn qfn_gets_an_exposed_pad() {
        let def = generate(&preset("QFN-32 5x5"), "QFN").unwrap();
        assert_eq!(def.pads.len(), 33);
        let ep = pad(&def, "33");
        assert_eq!(ep.at, Point::ORIGIN);
        assert!(ep.paste_margin.unwrap() < 0);
        assert_eq!(
            suggest_name(&preset("QFN-32 5x5")),
            "QFN-32-1EP_5x5mm_P0.5mm_EP3.45x3.45mm"
        );
    }

    #[test]
    fn bga_rows_skip_letters() {
        assert_eq!(
            (bga_row(0), bga_row(7), bga_row(8), bga_row(19), bga_row(20)),
            ("A".into(), "H".into(), "J".into(), "Y".into(), "AA".into())
        );
        let def = generate(&preset("BGA-256 16x16 P1.0"), "BGA").unwrap();
        assert_eq!(def.pads.len(), 256);
        assert!(def.pads.iter().any(|p| p.number == "T16"));
        assert!(!def.pads.iter().any(|p| p.number.starts_with('I')));
        let a1 = pad(&def, "A1");
        assert!(a1.at.x < 0 && a1.at.y > 0, "A1 top left");
    }

    #[test]
    fn headers_number_across_rows() {
        let def = generate(&preset("Pin header 2x05 2.54 mm"), "H").unwrap();
        assert_eq!(def.pads.len(), 10);
        assert_eq!(pad(&def, "1").at, Point::ORIGIN);
        assert_eq!(pad(&def, "2").at, Point::new(2_540_000, 0));
        assert_eq!(pad(&def, "3").at, Point::new(0, -2_540_000));
        assert_eq!(def.attributes.mounting, Mounting::ThroughHole);
        assert!(pad(&def, "1").drill.is_some());
    }

    #[test]
    fn impossible_dimensions_are_refused() {
        let bad = Wizard::Header {
            rows: 1,
            pins_per_row: 2,
            pitch: um(2_540),
            drill: um(1_800),
            pad: um(1_700),
        };
        assert!(generate(&bad, "x").is_err());
        let Wizard::GullWing {
            quad,
            pitch,
            lead_span,
            lead_length,
            body_width,
            body_length,
            density,
            ..
        } = preset("SOIC-8")
        else {
            unreachable!()
        };
        let touching = Wizard::GullWing {
            pins: 8,
            quad,
            pitch,
            lead_span,
            lead_length,
            lead_width: um(1_300),
            body_width,
            body_length,
            density,
        };
        assert!(generate(&touching, "x").is_err());
        let odd = Wizard::GullWing {
            pins: 7,
            quad,
            pitch,
            lead_span,
            lead_length,
            lead_width: um(410),
            body_width,
            body_length,
            density,
        };
        assert!(generate(&odd, "x").is_err());
    }

    #[test]
    fn every_preset_generates() {
        for preset in presets() {
            let name = suggest_name(&preset.wizard);
            let def =
                generate(&preset.wizard, &name).unwrap_or_else(|e| panic!("{}: {e}", preset.name));
            assert!(!def.pads.is_empty(), "{}", preset.name);
            assert_eq!(def.texts.len(), 2);
            // Every pad lies inside the courtyard.
            let courtyard = def
                .graphics
                .iter()
                .find_map(|g| match (&g.layer, &g.geometry) {
                    (Layer::Courtyard(_), Geometry::Rect { start, end }) => Some((*start, *end)),
                    _ => None,
                })
                .unwrap();
            let (lo_x, hi_x) = (
                courtyard.0.x.min(courtyard.1.x),
                courtyard.0.x.max(courtyard.1.x),
            );
            let (lo_y, hi_y) = (
                courtyard.0.y.min(courtyard.1.y),
                courtyard.0.y.max(courtyard.1.y),
            );
            for pad in &def.pads {
                assert!(
                    pad.at.x - pad.size.w / 2 >= lo_x && pad.at.x + pad.size.w / 2 <= hi_x,
                    "{} pad {}",
                    preset.name,
                    pad.number
                );
                assert!(
                    pad.at.y - pad.size.h / 2 >= lo_y && pad.at.y + pad.size.h / 2 <= hi_y,
                    "{} pad {}",
                    preset.name,
                    pad.number
                );
            }
        }
    }
}
