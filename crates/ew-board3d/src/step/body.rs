//! The board body as an exact B-rep: the outline with its lines and arcs,
//! cut-outs and drilled holes as true cylinders, extruded through the
//! dielectric stack.
//!
//! Every loop is a list of segments (lines and arcs; full circles are split
//! into two halves). The solid gets a top and a bottom face with all loops,
//! one side face per segment (a plane for lines, a cylinder for arcs), and
//! shares every edge between exactly two faces, as a closed shell must.
//! Boards whose holes overlap or cut the outline fall back to the polygons
//! of the 3D view (straight segments only), which are always valid.

use std::f64::consts::{PI, TAU};

use ew_model::layer::Layer;
use ew_model::pcb::Pcb;
use ew_model::shape::Geometry;
use ew_output::drill::Hole;
use ew_pcb::geom::{V, arc_through, signed_area2};
use ew_pcb::poly;

use super::{Writer, direction, point, real};
use crate::board::hole_polygon;
use crate::mesh::Polygons;

/// One segment of a loop (millimetres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Segment {
    /// A straight line from `a` to `b`.
    Line {
        /// Start.
        a: [f64; 2],
        /// End.
        b: [f64; 2],
    },
    /// An arc around `center` from `a` to `b` (less than a full turn).
    Arc {
        /// Start.
        a: [f64; 2],
        /// End.
        b: [f64; 2],
        /// Centre.
        center: [f64; 2],
        /// Radius.
        radius: f64,
        /// Counter-clockwise from `a` to `b`.
        ccw: bool,
    },
}

impl Segment {
    /// Start point.
    #[must_use]
    pub const fn start(&self) -> [f64; 2] {
        match self {
            Self::Line { a, .. } | Self::Arc { a, .. } => *a,
        }
    }

    /// End point.
    #[must_use]
    pub const fn end(&self) -> [f64; 2] {
        match self {
            Self::Line { b, .. } | Self::Arc { b, .. } => *b,
        }
    }

    /// The same segment walked backwards.
    #[must_use]
    pub const fn reversed(&self) -> Self {
        match *self {
            Self::Line { a, b } => Self::Line { a: b, b: a },
            Self::Arc {
                a,
                b,
                center,
                radius,
                ccw,
            } => Self::Arc {
                a: b,
                b: a,
                center,
                radius,
                ccw: !ccw,
            },
        }
    }

    /// Points along the segment (its start included, its end not).
    fn sample(&self, out: &mut Vec<[f64; 2]>) {
        match *self {
            Self::Line { a, .. } => out.push(a),
            Self::Arc {
                a,
                b,
                center,
                radius,
                ccw,
            } => {
                let a0 = (a[1] - center[1]).atan2(a[0] - center[0]);
                let a1 = (b[1] - center[1]).atan2(b[0] - center[0]);
                let sweep = if ccw {
                    (a1 - a0).rem_euclid(TAU)
                } else {
                    -(a0 - a1).rem_euclid(TAU)
                };
                let n = ((sweep.abs() / (PI / 32.0)).ceil() as usize).max(2);
                for i in 0..n {
                    let t = a0 + sweep * i as f64 / n as f64;
                    out.push([center[0] + radius * t.cos(), center[1] + radius * t.sin()]);
                }
            }
        }
    }
}

/// A closed loop of segments.
pub type Loop = Vec<Segment>;

fn mm2(v: V) -> [f64; 2] {
    [v.x / 1e6, v.y / 1e6]
}

/// A full circle as two half arcs (counter-clockwise).
fn circle(center: [f64; 2], radius: f64) -> Loop {
    let a = [center[0] + radius, center[1]];
    let b = [center[0] - radius, center[1]];
    vec![
        Segment::Arc {
            a,
            b,
            center,
            radius,
            ccw: true,
        },
        Segment::Arc {
            a: b,
            b: a,
            center,
            radius,
            ccw: true,
        },
    ]
}

/// The loop of a drilled hole (counter-clockwise).
fn hole_loop(hole: &Hole) -> Loop {
    let r = hole.diameter / 2e6;
    let a = mm2(hole.start);
    match hole.end {
        None => circle(a, r),
        Some(end) => {
            let b = mm2(end);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let length = dx.hypot(dy).max(1e-12);
            // Left normal of a → b.
            let (nx, ny) = (-dy / length * r, dx / length * r);
            let a_right = [a[0] - nx, a[1] - ny];
            let b_right = [b[0] - nx, b[1] - ny];
            let b_left = [b[0] + nx, b[1] + ny];
            let a_left = [a[0] + nx, a[1] + ny];
            vec![
                Segment::Line {
                    a: a_right,
                    b: b_right,
                },
                Segment::Arc {
                    a: b_right,
                    b: b_left,
                    center: b,
                    radius: r,
                    ccw: true,
                },
                Segment::Line {
                    a: b_left,
                    b: a_left,
                },
                Segment::Arc {
                    a: a_left,
                    b: a_right,
                    center: a,
                    radius: r,
                    ccw: true,
                },
            ]
        }
    }
}

/// The segments of one outline shape.
fn shape_segments(geometry: &Geometry, out: &mut Vec<Vec<Segment>>) {
    let p = |point: ew_model::units::Point| [point.x as f64 / 1e6, point.y as f64 / 1e6];
    let lines = |points: &[[f64; 2]], closed: bool| -> Vec<Segment> {
        let mut segments: Vec<Segment> = points
            .windows(2)
            .map(|w| Segment::Line { a: w[0], b: w[1] })
            .collect();
        if closed && points.len() > 2 {
            segments.push(Segment::Line {
                a: points[points.len() - 1],
                b: points[0],
            });
        }
        segments
    };
    match geometry {
        Geometry::Line { start, end } => out.push(vec![Segment::Line {
            a: p(*start),
            b: p(*end),
        }]),
        Geometry::Rect { start, end } => {
            let (a, b) = (p(*start), p(*end));
            out.push(lines(&[a, [b[0], a[1]], b, [a[0], b[1]]], true));
        }
        Geometry::Circle { center, radius } => out.push(circle(p(*center), *radius as f64 / 1e6)),
        Geometry::Arc { start, mid, end } => {
            match arc_through(V::of(*start), V::of(*mid), V::of(*end)) {
                Some((center, radius, _, sweep)) => out.push(vec![Segment::Arc {
                    a: p(*start),
                    b: p(*end),
                    center: mm2(center),
                    radius: radius / 1e6,
                    ccw: sweep > 0.0,
                }]),
                None => out.push(vec![Segment::Line {
                    a: p(*start),
                    b: p(*end),
                }]),
            }
        }
        Geometry::Polyline { points } => {
            let points: Vec<[f64; 2]> = points.iter().map(|q| p(*q)).collect();
            out.push(lines(&points, false));
        }
        Geometry::Polygon { points } => {
            let points: Vec<[f64; 2]> = points.iter().map(|q| p(*q)).collect();
            out.push(lines(&points, true));
        }
        Geometry::Bezier {
            start,
            control1,
            control2,
            end,
        } => {
            let (p0, p1, p2, p3) = (p(*start), p(*control1), p(*control2), p(*end));
            let points: Vec<[f64; 2]> = (0..=16)
                .map(|i| {
                    let t = f64::from(i) / 16.0;
                    let u = 1.0 - t;
                    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                    [
                        w[0] * p0[0] + w[1] * p1[0] + w[2] * p2[0] + w[3] * p3[0],
                        w[0] * p0[1] + w[1] * p1[1] + w[2] * p2[1] + w[3] * p3[1],
                    ]
                })
                .collect();
            out.push(lines(&points, false));
        }
    }
}

fn close(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).hypot(a[1] - b[1]) <= 0.01
}

/// Chains outline pieces into closed loops; `None` if an end stays open.
fn chain(mut pieces: Vec<Vec<Segment>>) -> Option<Vec<Loop>> {
    let mut loops = Vec::new();
    while let Some(mut current) = pieces.pop() {
        loop {
            let first = current.first()?.start();
            let last = current.last()?.end();
            if close(first, last)
                && (current.len() > 1 || matches!(current[0], Segment::Arc { .. }))
            {
                // Snap the joints exactly.
                let n = current.len();
                for i in 0..n {
                    let next = current[(i + 1) % n].start();
                    snap_end(&mut current[i], next);
                }
                loops.push(current);
                break;
            }
            let index = pieces.iter().position(|piece| {
                piece.first().is_some_and(|s| close(s.start(), last))
                    || piece.last().is_some_and(|s| close(s.end(), last))
            })?;
            let mut piece = pieces.swap_remove(index);
            if !piece.first().is_some_and(|s| close(s.start(), last)) {
                piece = piece.iter().rev().map(Segment::reversed).collect();
            }
            current.extend(piece);
        }
    }
    Some(loops)
}

fn snap_end(segment: &mut Segment, to: [f64; 2]) {
    match segment {
        Segment::Line { b, .. } | Segment::Arc { b, .. } => *b = to,
    }
}

/// The signed area of a loop (positive counter-clockwise), in mm².
#[must_use]
pub fn loop_area(segments: &[Segment]) -> f64 {
    let mut points = Vec::new();
    for segment in segments {
        segment.sample(&mut points);
    }
    let ring: Vec<V> = points.iter().map(|p| V::new(p[0], p[1])).collect();
    signed_area2(&ring) / 2.0
}

fn oriented(segments: Loop, ccw: bool) -> Loop {
    if (loop_area(&segments) > 0.0) == ccw {
        segments
    } else {
        segments.iter().rev().map(Segment::reversed).collect()
    }
}

/// The loops of the body: the outline (counter-clockwise) first, then the
/// cut-outs and holes (clockwise), and `true` if they are exact. Uses the
/// exact outline and hole shapes when they form a valid solid, else the
/// polygons of `drilled` (the board area minus the holes, as the 3D view
/// has it).
#[must_use]
pub fn body_loops(
    pcb: &Pcb,
    holes: &[Hole],
    area: &Polygons,
    drilled: &Polygons,
) -> (Vec<Loop>, bool) {
    exact_loops(pcb, holes, area).map_or_else(|| (polygon_loops(drilled), false), |l| (l, true))
}

fn exact_loops(pcb: &Pcb, holes: &[Hole], area: &Polygons) -> Option<Vec<Loop>> {
    let mut pieces = Vec::new();
    for shape in pcb.shapes.iter().filter(|s| s.layer == Layer::Outline) {
        shape_segments(&shape.geometry, &mut pieces);
    }
    let mut loops = chain(pieces)?;
    if loops.is_empty() {
        return None;
    }
    loops.sort_by(|a, b| loop_area(b).abs().total_cmp(&loop_area(a).abs()));
    // The exact solid needs holes inside the board that touch nothing:
    // then drilling removes exactly the holes' area.
    let hole_polygons: Polygons = holes
        .iter()
        .map(|hole| vec![hole_polygon(hole, 1_000.0)])
        .collect();
    let drilled = poly::difference(area, &hole_polygons);
    let expected = poly::area(area)
        - hole_polygons
            .iter()
            .map(|p| poly::area(std::slice::from_ref(p)))
            .sum::<f64>();
    let rings: usize = drilled.iter().map(Vec::len).sum();
    let expected_rings = loops.len() + holes.len();
    if (poly::area(&drilled) - expected).abs() > 1e-4 * expected.abs() || rings != expected_rings {
        return None;
    }
    let mut out = Vec::with_capacity(loops.len() + holes.len());
    let mut iter = loops.into_iter();
    out.push(oriented(iter.next()?, true));
    out.extend(iter.map(|l| oriented(l, false)));
    out.extend(holes.iter().map(|h| oriented(hole_loop(h), false)));
    Some(out)
}

/// Loops of straight segments from polygons (the fallback).
fn polygon_loops(polygons: &Polygons) -> Vec<Loop> {
    let mut out = Vec::new();
    // Only the largest polygon: a solid has one outer loop.
    let Some(largest) = polygons.iter().max_by(|a, b| {
        let area = |p: &Vec<Vec<V>>| p.first().map_or(0.0, |r| signed_area2(r).abs());
        area(a).total_cmp(&area(b))
    }) else {
        return out;
    };
    for (index, ring) in largest.iter().enumerate() {
        let points: Vec<[f64; 2]> = ring.iter().map(|v| mm2(*v)).collect();
        let n = points.len();
        if n < 3 {
            continue;
        }
        let segments: Loop = (0..n)
            .map(|i| Segment::Line {
                a: points[i],
                b: points[(i + 1) % n],
            })
            .collect();
        out.push(oriented(segments, index == 0));
    }
    out
}

/// Writes the solid; returns the `MANIFOLD_SOLID_BREP`.
#[allow(
    clippy::too_many_lines,
    reason = "vertices, edges, then top, bottom and side faces of one shell"
)]
pub fn write_solid(w: &mut Writer, loops: &[Loop], height: f64) -> u64 {
    // Vertices: bottom and top of every segment start.
    struct Edges {
        bottom: Vec<u64>,
        top: Vec<u64>,
        vertical: Vec<u64>,
    }
    let z_axis = direction(w, [0.0, 0.0, 1.0]);
    let mut all = Vec::with_capacity(loops.len());
    for segments in loops {
        let n = segments.len();
        let mut bottom_v = Vec::with_capacity(n);
        let mut top_v = Vec::with_capacity(n);
        for segment in segments {
            let [x, y] = segment.start();
            let p0 = point(w, [x, y, 0.0]);
            let p1 = point(w, [x, y, height]);
            bottom_v.push(w.add(format!("VERTEX_POINT('',#{p0})")));
            top_v.push(w.add(format!("VERTEX_POINT('',#{p1})")));
        }
        let mut edges = Edges {
            bottom: Vec::with_capacity(n),
            top: Vec::with_capacity(n),
            vertical: Vec::with_capacity(n),
        };
        for (i, segment) in segments.iter().enumerate() {
            let j = (i + 1) % n;
            for (z, vertices, list) in [
                (0.0, &bottom_v, &mut edges.bottom),
                (height, &top_v, &mut edges.top),
            ] {
                let (curve, sense) = curve(w, segment, z);
                list.push(w.add(format!(
                    "EDGE_CURVE('',#{},#{},#{curve},{})",
                    vertices[i],
                    vertices[j],
                    if sense { ".T." } else { ".F." }
                )));
            }
            let [x, y] = segment.start();
            let p = point(w, [x, y, 0.0]);
            let vector = w.add(format!("VECTOR('',#{z_axis},{})", real(height)));
            let line = w.add(format!("LINE('',#{p},#{vector})"));
            edges.vertical.push(w.add(format!(
                "EDGE_CURVE('',#{},#{},#{line},.T.)",
                bottom_v[i], top_v[i]
            )));
        }
        all.push(edges);
    }
    let mut faces = Vec::new();
    // Top and bottom faces with every loop.
    for top in [true, false] {
        let mut bounds = Vec::new();
        for (index, edges) in all.iter().enumerate() {
            let oriented: Vec<u64> = if top {
                edges
                    .top
                    .iter()
                    .map(|e| w.add(format!("ORIENTED_EDGE('',*,*,#{e},.T.)")))
                    .collect()
            } else {
                // Seen from below, the loop runs the other way.
                edges
                    .bottom
                    .iter()
                    .rev()
                    .map(|e| w.add(format!("ORIENTED_EDGE('',*,*,#{e},.F.)")))
                    .collect()
            };
            let list = refs(&oriented);
            let edge_loop = w.add(format!("EDGE_LOOP('',({list}))"));
            let kind = if index == 0 {
                "FACE_OUTER_BOUND"
            } else {
                "FACE_BOUND"
            };
            bounds.push(w.add(format!("{kind}('',#{edge_loop},.T.)")));
        }
        let z = if top { height } else { 0.0 };
        let origin = point(w, [0.0, 0.0, z]);
        let normal = direction(w, [0.0, 0.0, if top { 1.0 } else { -1.0 }]);
        let x_axis = direction(w, [1.0, 0.0, 0.0]);
        let placement = w.add(format!(
            "AXIS2_PLACEMENT_3D('',#{origin},#{normal},#{x_axis})"
        ));
        let plane = w.add(format!("PLANE('',#{placement})"));
        let list = refs(&bounds);
        faces.push(w.add(format!("ADVANCED_FACE('',({list}),#{plane},.T.)")));
    }
    // Side faces.
    for (segments, edges) in loops.iter().zip(&all) {
        let n = segments.len();
        for (i, segment) in segments.iter().enumerate() {
            let j = (i + 1) % n;
            let loop_edges = [
                w.add(format!("ORIENTED_EDGE('',*,*,#{},.T.)", edges.bottom[i])),
                w.add(format!("ORIENTED_EDGE('',*,*,#{},.T.)", edges.vertical[j])),
                w.add(format!("ORIENTED_EDGE('',*,*,#{},.F.)", edges.top[i])),
                w.add(format!("ORIENTED_EDGE('',*,*,#{},.F.)", edges.vertical[i])),
            ];
            let list = refs(&loop_edges);
            let edge_loop = w.add(format!("EDGE_LOOP('',({list}))"));
            let bound = w.add(format!("FACE_OUTER_BOUND('',#{edge_loop},.T.)"));
            let (surface, sense) = side_surface(w, segment);
            faces.push(w.add(format!(
                "ADVANCED_FACE('',(#{bound}),#{surface},{})",
                if sense { ".T." } else { ".F." }
            )));
        }
    }
    let list = refs(&faces);
    let shell = w.add(format!("CLOSED_SHELL('',({list}))"));
    w.add(format!("MANIFOLD_SOLID_BREP('',#{shell})"))
}

/// The curve of a segment at height `z` and whether the edge runs along
/// it.
fn curve(w: &mut Writer, segment: &Segment, z: f64) -> (u64, bool) {
    match *segment {
        Segment::Line { a, b } => {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let length = dx.hypot(dy).max(1e-12);
            let p = point(w, [a[0], a[1], z]);
            let d = direction(w, [dx / length, dy / length, 0.0]);
            let vector = w.add(format!("VECTOR('',#{d},{})", real(length)));
            (w.add(format!("LINE('',#{p},#{vector})")), true)
        }
        Segment::Arc {
            center,
            radius,
            ccw,
            ..
        } => {
            let c = point(w, [center[0], center[1], z]);
            let axis = direction(w, [0.0, 0.0, 1.0]);
            let x_axis = direction(w, [1.0, 0.0, 0.0]);
            let placement = w.add(format!("AXIS2_PLACEMENT_3D('',#{c},#{axis},#{x_axis})"));
            (
                w.add(format!("CIRCLE('',#{placement},{})", real(radius))),
                ccw,
            )
        }
    }
}

/// The surface of a side face and whether the face's normal follows it.
/// Side faces look away from the material, which lies left of the loop.
fn side_surface(w: &mut Writer, segment: &Segment) -> (u64, bool) {
    match *segment {
        Segment::Line { a, b } => {
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let length = dx.hypot(dy).max(1e-12);
            let p = point(w, [a[0], a[1], 0.0]);
            // Right of the walking direction.
            let normal = direction(w, [dy / length, -dx / length, 0.0]);
            let x_axis = direction(w, [dx / length, dy / length, 0.0]);
            let placement = w.add(format!("AXIS2_PLACEMENT_3D('',#{p},#{normal},#{x_axis})"));
            (w.add(format!("PLANE('',#{placement})")), true)
        }
        Segment::Arc {
            center,
            radius,
            ccw,
            ..
        } => {
            let c = point(w, [center[0], center[1], 0.0]);
            let axis = direction(w, [0.0, 0.0, 1.0]);
            let x_axis = direction(w, [1.0, 0.0, 0.0]);
            let placement = w.add(format!("AXIS2_PLACEMENT_3D('',#{c},#{axis},#{x_axis})"));
            let cylinder = w.add(format!(
                "CYLINDRICAL_SURFACE('',#{placement},{})",
                real(radius)
            ));
            // Counter-clockwise arcs have the material inside: the face
            // looks outwards like the cylinder; clockwise arcs (holes) look
            // to the axis.
            (cylinder, ccw)
        }
    }
}

fn refs(ids: &[u64]) -> String {
    ids.iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(",")
}
