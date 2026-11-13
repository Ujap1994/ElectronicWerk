//! Vector pages for prints and drawings (PDF and SVG): filled and stroked
//! paths, plus invisible text that makes PDF prints searchable.
//!
//! Page coordinates are nanometres on the paper with the origin in the
//! lower-left corner and y pointing up, like the model. A [`View`] maps
//! model coordinates onto the page.

use std::f64::consts::{FRAC_PI_2, TAU};

use ew_pcb::geom::{Core, Shape, V};

use crate::art::Feature;
use crate::font::{self, TextSpec};

/// Chord tolerance of curves split into lines, in page nanometres.
const TOLERANCE: f64 = 2_000.0;

/// An RGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

impl Color {
    /// Black.
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    /// White.
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    /// A colour from its components.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses `#rrggbb`.
    #[must_use]
    pub fn hex(text: &str) -> Option<Self> {
        let text = text.strip_prefix('#')?;
        if text.len() != 6 {
            return None;
        }
        let v = u32::from_str_radix(text, 16).ok()?;
        Some(Self::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
    }

    /// `#rrggbb`.
    #[must_use]
    pub fn css(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// A path segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Seg {
    /// Starts a sub-path.
    Move(V),
    /// A straight line.
    Line(V),
    /// A cubic Bézier curve (two control points and the end).
    Cubic(V, V, V),
    /// Closes the sub-path.
    Close,
}

/// A path made of sub-paths.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Path(pub Vec<Seg>);

impl Path {
    /// A polyline, closed or open.
    #[must_use]
    pub fn polyline(points: &[V], closed: bool) -> Self {
        let mut path = Self::default();
        path.add_polyline(points, closed);
        path
    }

    /// Adds a polyline.
    pub fn add_polyline(&mut self, points: &[V], closed: bool) {
        let Some(first) = points.first() else {
            return;
        };
        self.0.push(Seg::Move(*first));
        for p in &points[1..] {
            self.0.push(Seg::Line(*p));
        }
        if closed {
            self.0.push(Seg::Close);
        }
    }

    /// Adds a circular arc from angle `start` sweeping `sweep` radians
    /// (counter-clockwise when positive), as Bézier curves.
    pub fn add_arc(&mut self, center: V, radius: f64, start: f64, sweep: f64, move_first: bool) {
        let parts = (sweep.abs() / FRAC_PI_2).ceil().max(1.0) as usize;
        let step = sweep / parts as f64;
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        let point = |a: f64| center.add(V::new(radius * a.cos(), radius * a.sin()));
        if move_first {
            self.0.push(Seg::Move(point(start)));
        }
        for i in 0..parts {
            let a0 = start + step * i as f64;
            let a1 = a0 + step;
            let (p0, p3) = (point(a0), point(a1));
            let t0 = V::new(-a0.sin(), a0.cos()).scale(radius * k);
            let t1 = V::new(-a1.sin(), a1.cos()).scale(radius * k);
            self.0.push(Seg::Cubic(p0.add(t0), p3.sub(t1), p3));
        }
    }

    /// A full circle.
    #[must_use]
    pub fn circle(center: V, radius: f64) -> Self {
        let mut path = Self::default();
        path.add_arc(center, radius, 0.0, TAU, true);
        path.0.push(Seg::Close);
        path
    }

    /// `true` if the path has no segments.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// How a path is painted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paint {
    /// Fill colour (even-odd rule, so inner sub-paths are holes).
    pub fill: Option<Color>,
    /// Stroke colour and width (round caps and joins).
    pub stroke: Option<(Color, f64)>,
}

impl Paint {
    /// Filled only.
    #[must_use]
    pub const fn fill(color: Color) -> Self {
        Self {
            fill: Some(color),
            stroke: None,
        }
    }

    /// Stroked only.
    #[must_use]
    pub const fn stroke(color: Color, width: f64) -> Self {
        Self {
            fill: None,
            stroke: Some((color, width)),
        }
    }
}

/// A painted path.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    /// The path.
    pub path: Path,
    /// The paint.
    pub paint: Paint,
}

/// Invisible text over the drawn strokes (search and copy in PDF viewers).
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    /// One line of text.
    pub text: String,
    /// Start of the baseline.
    pub at: V,
    /// Cap height.
    pub size: f64,
    /// Rotation in degrees.
    pub rotation: f64,
    /// Mirrored.
    pub mirror: bool,
}

/// A page.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Page {
    /// Name (bookmark in PDF files).
    pub name: String,
    /// Paper width.
    pub width: f64,
    /// Paper height.
    pub height: f64,
    /// Background colour, if not white paper.
    pub background: Option<Color>,
    /// Painted paths in drawing order.
    pub items: Vec<Item>,
    /// Invisible texts.
    pub labels: Vec<Label>,
}

/// A document of pages.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Doc {
    /// Title.
    pub title: String,
    /// Application version (PDF producer).
    pub version: String,
    /// Creation time as `YYYY-MM-DDTHH:MM:SS` (empty: none).
    pub created: String,
    /// Pages.
    pub pages: Vec<Page>,
}

/// Maps model coordinates onto the page: `page = model × scale + offset`,
/// optionally mirrored left to right around the page centre.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    /// Scale.
    pub scale: f64,
    /// Offset.
    pub offset: V,
    /// Mirror x (bottom views).
    pub mirror: bool,
    /// Page width (for mirroring).
    pub width: f64,
}

impl View {
    /// A view that fits the model box `[min x, min y, max x, max y]` into
    /// the page area `[x, y, w, h]`, centred, at most at `max_scale`.
    #[must_use]
    pub fn fit(model: [f64; 4], area: [f64; 4], max_scale: f64, mirror: bool, width: f64) -> Self {
        let (mw, mh) = (
            (model[2] - model[0]).max(1.0),
            (model[3] - model[1]).max(1.0),
        );
        let scale = (area[2] / mw).min(area[3] / mh).min(max_scale);
        let offset = V::new(
            area[0] + (area[2] - mw * scale) / 2.0 - model[0] * scale,
            area[1] + (area[3] - mh * scale) / 2.0 - model[1] * scale,
        );
        let offset = if mirror {
            // Mirroring flips around the page centre; keep the drawing in
            // the area.
            V::new(width - offset.x - (model[0] + model[2]) * scale, offset.y)
        } else {
            offset
        };
        Self {
            scale,
            offset,
            mirror,
            width,
        }
    }

    /// Maps a point.
    #[must_use]
    pub fn point(&self, p: V) -> V {
        let q = p.scale(self.scale).add(self.offset);
        if self.mirror {
            V::new(self.width - q.x, q.y)
        } else {
            q
        }
    }

    /// Maps a length.
    #[must_use]
    pub fn length(&self, d: f64) -> f64 {
        d * self.scale
    }
}

impl Page {
    /// An empty page.
    #[must_use]
    pub fn new(name: impl Into<String>, width: f64, height: f64) -> Self {
        Self {
            name: name.into(),
            width,
            height,
            ..Self::default()
        }
    }

    /// Adds a painted path.
    pub fn add(&mut self, path: Path, paint: Paint) {
        if !path.is_empty() {
            self.items.push(Item { path, paint });
        }
    }

    /// Draws a text with the stroke font (page coordinates) and adds it as
    /// invisible text.
    pub fn text(&mut self, spec: &TextSpec<'_>, color: Color, width: f64) {
        let mut path = Path::default();
        for stroke in font::strokes(spec) {
            let points: Vec<V> = stroke.iter().map(|p| V::new(p[0], p[1])).collect();
            path.add_polyline(&points, false);
        }
        self.add(path, Paint::stroke(color, width));
        for (line, at) in font::baselines(spec) {
            if !line.trim().is_empty() {
                self.labels.push(Label {
                    text: line,
                    at: V::new(at[0], at[1]),
                    size: spec.size,
                    rotation: spec.rotation,
                    mirror: spec.mirror,
                });
            }
        }
    }

    /// Draws a plotted board feature in one colour.
    pub fn feature(&mut self, feature: &Feature, view: &View, color: Color) {
        match feature {
            Feature::Flash { shape, .. } => {
                self.add(shape_path(shape, view), Paint::fill(color));
            }
            Feature::Line { a, b, width, .. } => self.add(
                Path::polyline(&[view.point(*a), view.point(*b)], false),
                Paint::stroke(color, view.length(*width)),
            ),
            Feature::Arc {
                start,
                end,
                center,
                ccw,
                width,
                ..
            } => {
                let radius = center.dist(*start);
                let a0 = (start.y - center.y).atan2(start.x - center.x);
                let a1 = (end.y - center.y).atan2(end.x - center.x);
                let mut sweep = if *ccw { a1 - a0 } else { a0 - a1 };
                if sweep <= 1e-12 {
                    sweep += TAU;
                }
                let signed = if *ccw { sweep } else { -sweep };
                let steps = ew_pcb::geom::arc_segments(view.length(radius), sweep, TOLERANCE);
                let points: Vec<V> = (0..=steps)
                    .map(|i| {
                        let a = a0 + signed * i as f64 / steps as f64;
                        view.point(center.add(V::new(radius * a.cos(), radius * a.sin())))
                    })
                    .collect();
                self.add(
                    Path::polyline(&points, false),
                    Paint::stroke(color, view.length(*width)),
                );
            }
            Feature::Region { outline, holes, .. } => {
                let mut path = Path::default();
                for ring in std::iter::once(outline).chain(holes) {
                    let points: Vec<V> = ring.iter().map(|p| view.point(*p)).collect();
                    path.add_polyline(&points, true);
                }
                self.add(path, Paint::fill(color));
            }
        }
    }
}

/// The outline of a board shape on the page.
#[must_use]
pub fn shape_path(shape: &Shape, view: &View) -> Path {
    if let Core::Point(c) = &shape.core {
        Path::circle(view.point(*c), view.length(shape.radius))
    } else {
        let mut path = Path::default();
        for polygon in shape.outlines(0.0, TOLERANCE / view.scale.max(1e-9)) {
            for ring in polygon {
                let points: Vec<V> = ring.iter().map(|p| view.point(*p)).collect();
                path.add_polyline(&points, true);
            }
        }
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours() {
        assert_eq!(Color::hex("#c83434"), Some(Color::rgb(200, 52, 52)));
        assert_eq!(Color::rgb(1, 2, 255).css(), "#0102ff");
        assert_eq!(Color::hex("red"), None);
    }

    #[test]
    fn views_fit_and_mirror() {
        let view = View::fit(
            [0.0, 0.0, 100.0, 50.0],
            [10.0, 10.0, 200.0, 200.0],
            10.0,
            false,
            220.0,
        );
        assert!((view.scale - 2.0).abs() < 1e-9);
        let p = view.point(V::new(0.0, 0.0));
        assert!((p.x - 10.0).abs() < 1e-9 && (p.y - 60.0).abs() < 1e-9);
        let mirrored = View::fit(
            [0.0, 0.0, 100.0, 50.0],
            [10.0, 10.0, 200.0, 200.0],
            10.0,
            true,
            220.0,
        );
        let a = mirrored.point(V::new(0.0, 0.0));
        let b = mirrored.point(V::new(100.0, 0.0));
        assert!(a.x > b.x && (b.x - 10.0).abs() < 1e-9 && (a.x - 210.0).abs() < 1e-9);
    }

    #[test]
    fn circles_are_four_curves() {
        let path = Path::circle(V::new(0.0, 0.0), 1.0);
        assert_eq!(path.0.len(), 6);
        let Seg::Cubic(_, _, end) = path.0[4] else {
            panic!("curve expected");
        };
        assert!((end.x - 1.0).abs() < 1e-9 && end.y.abs() < 1e-9);
    }
}
