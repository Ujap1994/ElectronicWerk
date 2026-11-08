//! Converts model geometry into scene primitives.

use ew_model::shape::Geometry;

use crate::builder::SceneBuilder;
use crate::geom::{self, P, Xform};

/// Where and how to draw a geometry: an optional stroke (layer, width) and
/// an optional fill layer.
#[derive(Debug, Clone, Copy)]
pub struct Paint {
    /// Stroke layer and width (0 = hairline).
    pub stroke: Option<(usize, f64)>,
    /// Fill layer.
    pub fill: Option<usize>,
}

impl Paint {
    /// Stroke, plus a fill on the same layer if `filled`.
    pub const fn board(layer: usize, width: f64, filled: bool) -> Self {
        Self {
            stroke: Some((layer, width)),
            fill: if filled { Some(layer) } else { None },
        }
    }
}

/// Segments used for full circles drawn as polygons.
const BEZIER_STEPS: usize = 24;

/// Draws a geometry placed with `xf`.
pub fn geometry(b: &mut SceneBuilder, obj: u32, g: &Geometry, paint: Paint, xf: &Xform) {
    match g {
        Geometry::Line { start, end } => {
            if let Some((layer, width)) = paint.stroke {
                b.segment(layer, obj, xf.point(*start), xf.point(*end), width);
            }
        }
        Geometry::Rect { start, end } => {
            let corners = [
                P::new(start.x as f64, start.y as f64),
                P::new(end.x as f64, start.y as f64),
                P::new(end.x as f64, end.y as f64),
                P::new(start.x as f64, end.y as f64),
            ]
            .map(|p| xf.apply(p));
            polygon(b, obj, &corners, paint);
        }
        Geometry::Circle { center, radius } => {
            let center = xf.point(*center);
            let radius = *radius as f64;
            let same_layer =
                matches!((paint.stroke, paint.fill), (Some((s, _)), Some(f)) if s == f);
            if let Some(fill) = paint.fill {
                let extra = if same_layer {
                    paint.stroke.map_or(0.0, |(_, w)| w / 2.0)
                } else {
                    0.0
                };
                b.circle(fill, obj, center, radius + extra, 0.0);
            }
            if let Some((layer, width)) = paint.stroke {
                if !same_layer {
                    let half = (width / 2.0).max(0.0);
                    b.arc(
                        layer,
                        obj,
                        center,
                        radius,
                        0.0,
                        std::f64::consts::TAU,
                        half * 2.0,
                    );
                }
            }
        }
        Geometry::Arc { start, mid, end } => {
            let (first, middle, last) = (xf.point(*start), xf.point(*mid), xf.point(*end));
            if let Some((layer, width)) = paint.stroke {
                match geom::arc_through(first, middle, last) {
                    Some((center, radius, from, sweep)) => {
                        b.arc(layer, obj, center, radius, from, sweep, width);
                    }
                    None => b.segment(layer, obj, first, last, width),
                }
            }
        }
        Geometry::Polyline { points } => {
            let points: Vec<P> = points.iter().map(|p| xf.point(*p)).collect();
            if let Some(fill) = paint.fill {
                b.polygon(fill, obj, &points, &[], 0);
            }
            if let Some((layer, width)) = paint.stroke {
                b.polyline(layer, obj, &points, false, width);
            }
        }
        Geometry::Polygon { points } => {
            let points: Vec<P> = points.iter().map(|p| xf.point(*p)).collect();
            polygon(b, obj, &points, paint);
        }
        Geometry::Bezier {
            start,
            control1,
            control2,
            end,
        } => {
            let points = geom::bezier(
                xf.point(*start),
                xf.point(*control1),
                xf.point(*control2),
                xf.point(*end),
                BEZIER_STEPS,
            );
            if let Some((layer, width)) = paint.stroke {
                b.polyline(layer, obj, &points, false, width);
            }
        }
    }
}

/// A closed polygon with optional fill and stroke.
pub fn polygon(b: &mut SceneBuilder, obj: u32, points: &[P], paint: Paint) {
    if let Some(fill) = paint.fill {
        b.polygon(fill, obj, points, &[], 0);
    }
    if let Some((layer, width)) = paint.stroke {
        if paint.fill != Some(layer) || width > 0.0 {
            b.polyline(layer, obj, points, true, width);
        }
    }
}
