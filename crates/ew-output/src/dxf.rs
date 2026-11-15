//! DXF export of the board outline and the mechanical and user layers, for
//! mechanical CAD (case design, front panels) and board makers that want
//! the outline as a drawing.
//!
//! The files are ASCII DXF R12 (`AC1009`), which every CAD program reads:
//! lines, arcs, circles and 2D polylines on one DXF layer per board layer.
//! Shapes are written as their centre lines; line widths and fills are not
//! drawn. Coordinates are relative to the board's drill and placement
//! origin; DXF's y axis points up like the board's.

use std::f64::consts::TAU;
use std::fmt::Write as _;

use ew_model::layer::Layer;
use ew_model::pcb::Pcb;
use ew_model::shape::Geometry;
use ew_model::units::Point;
use ew_pcb::geom::{V, Xform, arc_through};
use ew_pcb::items::footprint_xform;

use crate::print::layer_name;

/// Options of the DXF export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxfOptions {
    /// The board layers to write (empty: [`default_layers`]).
    pub layers: Vec<Layer>,
    /// Inches instead of millimetres.
    pub inch: bool,
    /// One file per layer instead of one file with a DXF layer each.
    pub separate: bool,
}

/// A written DXF file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxfFile {
    /// File name.
    pub name: String,
    /// Content.
    pub data: Vec<u8>,
}

/// The board outline and every mechanical and user layer with drawings.
#[must_use]
pub fn default_layers(pcb: &Pcb) -> Vec<Layer> {
    let mut layers = vec![Layer::Outline];
    let copper = pcb.stackup.copper_count();
    let used = pcb
        .shapes
        .iter()
        .map(|shape| shape.layer)
        .chain(pcb.texts.iter().map(|text| text.layer))
        .chain(pcb.footprints.iter().flat_map(|fp| {
            fp.graphics.iter().map(move |g| {
                if fp.side == ew_model::layer::Side::Bottom {
                    g.layer.flipped(copper)
                } else {
                    g.layer
                }
            })
        }));
    for layer in used {
        if matches!(layer, Layer::Mechanical(_) | Layer::User(_)) && !layers.contains(&layer) {
            layers.push(layer);
        }
    }
    layers[1..].sort_by_key(|layer| match *layer {
        Layer::Mechanical(n) => u16::from(n),
        Layer::User(n) => 100 + u16::from(n),
        _ => 0,
    });
    layers
}

/// The DXF layer name of a board layer (`Board_outline`, `Mechanical_1`, …).
#[must_use]
pub fn dxf_layer_name(layer: Layer) -> String {
    layer_name(layer).replace(' ', "_")
}

/// Writes the DXF files of a board.
#[must_use]
pub fn dxf_files(pcb: &Pcb, options: &DxfOptions, stem: &str) -> Vec<DxfFile> {
    let layers = if options.layers.is_empty() {
        default_layers(pcb)
    } else {
        options.layers.clone()
    };
    let drawing = Drawing::new(pcb, options.inch);
    if options.separate {
        layers
            .iter()
            .map(|&layer| DxfFile {
                name: format!("{stem}-{}.dxf", dxf_layer_name(layer)),
                data: drawing.write(&[layer]).into_bytes(),
            })
            .collect()
    } else {
        vec![DxfFile {
            name: format!("{stem}.dxf"),
            data: drawing.write(&layers).into_bytes(),
        }]
    }
}

/// The shapes of a board in DXF units.
struct Drawing<'a> {
    pcb: &'a Pcb,
    /// Nanometres per DXF unit.
    unit: f64,
    origin: Point,
}

/// Arcs and circles are written exactly; Bézier curves become this many
/// polyline segments.
const BEZIER_SEGMENTS: usize = 24;

impl<'a> Drawing<'a> {
    fn new(pcb: &'a Pcb, inch: bool) -> Self {
        Self {
            pcb,
            unit: if inch { 25_400_000.0 } else { 1_000_000.0 },
            origin: pcb.origin,
        }
    }

    /// A board point in DXF coordinates (both with y up).
    fn map(&self, p: V) -> V {
        V::new(
            (p.x - self.origin.x as f64) / self.unit,
            (p.y - self.origin.y as f64) / self.unit,
        )
    }

    fn write(&self, layers: &[Layer]) -> String {
        let mut out = String::new();
        header(&mut out, self.unit > 2_000_000.0);
        tables(&mut out, layers);
        out.push_str("0\nSECTION\n2\nENTITIES\n");
        let copper = self.pcb.stackup.copper_count();
        for &layer in layers {
            let name = dxf_layer_name(layer);
            for shape in self.pcb.shapes.iter().filter(|shape| shape.layer == layer) {
                self.geometry(&mut out, &name, &shape.geometry, &Xform::IDENTITY);
            }
            for fp in &self.pcb.footprints {
                let xf = footprint_xform(fp);
                for graphic in &fp.graphics {
                    let on = if fp.side == ew_model::layer::Side::Bottom {
                        graphic.layer.flipped(copper)
                    } else {
                        graphic.layer
                    };
                    if on == layer {
                        self.geometry(&mut out, &name, &graphic.geometry, &xf);
                    }
                }
            }
            for text in self.pcb.texts.iter().filter(|text| text.layer == layer) {
                self.text(&mut out, &name, text);
            }
        }
        out.push_str("0\nENDSEC\n0\nEOF\n");
        out
    }

    fn point(&self, xf: &Xform, p: Point) -> V {
        self.map(xf.apply(V::of(p)))
    }

    fn geometry(&self, out: &mut String, layer: &str, geometry: &Geometry, xf: &Xform) {
        match geometry {
            Geometry::Line { start, end } => {
                let (a, b) = (self.point(xf, *start), self.point(xf, *end));
                entity(out, "LINE", layer);
                coords(out, 10, a);
                coords(out, 11, b);
            }
            Geometry::Rect { start, end } => {
                let corners = [
                    Point::new(start.x, start.y),
                    Point::new(end.x, start.y),
                    Point::new(end.x, end.y),
                    Point::new(start.x, end.y),
                ];
                let points: Vec<V> = corners.iter().map(|&p| self.point(xf, p)).collect();
                polyline(out, layer, &points, true);
            }
            Geometry::Circle { center, radius } => {
                let c = self.point(xf, *center);
                entity(out, "CIRCLE", layer);
                coords(out, 10, c);
                number(out, 40, *radius as f64 / self.unit);
            }
            Geometry::Arc { start, mid, end } => {
                let (a, m, b) = (
                    self.point(xf, *start),
                    self.point(xf, *mid),
                    self.point(xf, *end),
                );
                arc(out, layer, a, m, b);
            }
            Geometry::Polyline { points } => {
                let points: Vec<V> = points.iter().map(|&p| self.point(xf, p)).collect();
                polyline(out, layer, &points, false);
            }
            Geometry::Polygon { points } => {
                let points: Vec<V> = points.iter().map(|&p| self.point(xf, p)).collect();
                polyline(out, layer, &points, true);
            }
            Geometry::Bezier {
                start,
                control1,
                control2,
                end,
            } => {
                let [p0, p1, p2, p3] = [start, control1, control2, end].map(|&p| self.point(xf, p));
                let points: Vec<V> = (0..=BEZIER_SEGMENTS)
                    .map(|i| {
                        let t = i as f64 / BEZIER_SEGMENTS as f64;
                        let u = 1.0 - t;
                        p0.scale(u * u * u)
                            .add(p1.scale(3.0 * u * u * t))
                            .add(p2.scale(3.0 * u * t * t))
                            .add(p3.scale(t * t * t))
                    })
                    .collect();
                polyline(out, layer, &points, false);
            }
        }
    }

    fn text(&self, out: &mut String, layer: &str, text: &ew_model::pcb::BoardText) {
        if text.text.trim().is_empty() {
            return;
        }
        let at = self.map(V::of(text.at));
        entity(out, "TEXT", layer);
        coords(out, 10, at);
        number(out, 40, text.style.size as f64 / self.unit);
        // One line of text; DXF R12 has no line breaks in TEXT.
        let content: String = text
            .text
            .lines()
            .next()
            .unwrap_or_default()
            .chars()
            .map(|c| {
                if c.is_ascii_graphic() || c == ' ' {
                    c
                } else {
                    '?'
                }
            })
            .collect();
        let _ = writeln!(out, "1\n{content}");
        let rotation = text.rotation.degrees();
        if rotation != 0.0 {
            number(out, 50, rotation);
        }
    }
}

fn header(out: &mut String, inch: bool) {
    out.push_str("999\nElectronicWerk\n0\nSECTION\n2\nHEADER\n");
    out.push_str("9\n$ACADVER\n1\nAC1009\n");
    // 1: inches, 4: millimetres (read by most programs even in R12 files).
    let _ = writeln!(out, "9\n$INSUNITS\n70\n{}", if inch { 1 } else { 4 });
    let _ = writeln!(out, "9\n$MEASUREMENT\n70\n{}", u8::from(!inch));
    out.push_str("0\nENDSEC\n");
}

fn tables(out: &mut String, layers: &[Layer]) {
    out.push_str("0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n");
    let _ = writeln!(out, "70\n{}", layers.len());
    for (index, &layer) in layers.iter().enumerate() {
        // Colours 1 to 6 (red, yellow, green, cyan, blue, magenta) in turn,
        // the outline in white/black (7).
        let colour = if layer == Layer::Outline {
            7
        } else {
            index % 6 + 1
        };
        let _ = writeln!(
            out,
            "0\nLAYER\n2\n{}\n70\n0\n62\n{colour}\n6\nCONTINUOUS",
            dxf_layer_name(layer)
        );
    }
    out.push_str("0\nENDTAB\n0\nENDSEC\n");
}

fn entity(out: &mut String, kind: &str, layer: &str) {
    let _ = writeln!(out, "0\n{kind}\n8\n{layer}");
}

/// A number with up to six decimals (a nanometre in millimetres).
fn number(out: &mut String, code: u16, value: f64) {
    let text = format!("{value:.6}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    let text = if text == "-0" { "0" } else { text };
    let _ = writeln!(out, "{code}\n{text}");
}

fn coords(out: &mut String, code: u16, p: V) {
    number(out, code, p.x);
    number(out, code + 10, p.y);
}

fn polyline(out: &mut String, layer: &str, points: &[V], closed: bool) {
    if points.len() < 2 {
        return;
    }
    entity(out, "POLYLINE", layer);
    // Vertices follow; flag 1: closed.
    let _ = writeln!(out, "66\n1\n70\n{}", u8::from(closed));
    coords(out, 10, V::new(0.0, 0.0));
    for &p in points {
        entity(out, "VERTEX", layer);
        coords(out, 10, p);
    }
    entity(out, "SEQEND", layer);
}

/// An arc through three points (already in DXF coordinates). DXF arcs run
/// counter-clockwise from the start to the end angle.
fn arc(out: &mut String, layer: &str, start: V, mid: V, end: V) {
    let Some((center, radius, from, sweep)) = arc_through(start, mid, end) else {
        entity(out, "LINE", layer);
        coords(out, 10, start);
        coords(out, 11, end);
        return;
    };
    let (a, b) = if sweep >= 0.0 {
        (from, from + sweep)
    } else {
        (from + sweep, from)
    };
    if sweep.abs() >= TAU - 1e-9 {
        entity(out, "CIRCLE", layer);
        coords(out, 10, center);
        number(out, 40, radius);
        return;
    }
    entity(out, "ARC", layer);
    coords(out, 10, center);
    number(out, 40, radius);
    number(out, 50, degrees(a));
    number(out, 51, degrees(b));
}

/// An angle in degrees in `[0, 360)`, rounded to the written precision.
fn degrees(radians: f64) -> f64 {
    let d = (radians.to_degrees() * 1e6).round() / 1e6;
    d.rem_euclid(360.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_short() {
        let mut out = String::new();
        number(&mut out, 10, 12.5);
        number(&mut out, 20, -0.000_000_1);
        number(&mut out, 30, 3.0);
        assert_eq!(out, "10\n12.5\n20\n0\n30\n3\n");
    }

    #[test]
    fn arcs_run_counter_clockwise() {
        let mut out = String::new();
        // A quarter circle from (1, 0) through 45° to (0, 1): counter-clockwise.
        let s = std::f64::consts::FRAC_1_SQRT_2;
        arc(
            &mut out,
            "L",
            V::new(1.0, 0.0),
            V::new(s, s),
            V::new(0.0, 1.0),
        );
        assert!(out.contains("50\n0\n51\n90\n"), "{out}");
        // The same arc the other way round.
        let mut back = String::new();
        arc(
            &mut back,
            "L",
            V::new(0.0, 1.0),
            V::new(s, s),
            V::new(1.0, 0.0),
        );
        assert!(back.contains("50\n0\n51\n90\n"), "{back}");
    }
}
