//! DXF import for board outlines and mechanical drawings.
//!
//! Reads the entities of an ASCII DXF file: lines, circles, arcs, light
//! and classic polylines (with bulges), splines (through their fit or
//! control points) and ellipses. Block inserts are not expanded. DXF's y
//! axis points up, the board's down, so y is mirrored.

use std::collections::BTreeSet;
use std::f64::consts::TAU;

use ew_model::shape::Geometry;
use ew_model::units::{Point, Rect};
use serde::Serialize;

use crate::geom::V;

/// What a DXF file contains.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drawing {
    /// The shapes, in nanometres on the board.
    #[serde(skip)]
    pub shapes: Vec<(String, Geometry)>,
    /// Layer names of the file.
    pub layers: Vec<String>,
    /// Units given in the file (`mm`, `inch`, …; `unitless` is read as mm).
    pub units: String,
    /// Number of shapes.
    pub count: usize,
    /// Bounding box `[x0, y0, x1, y1]` in nm.
    pub bounds: Option<[i64; 4]>,
    /// Entity kinds that were skipped, with counts.
    pub skipped: Vec<(String, usize)>,
}

/// Why a file cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct DxfError(pub String);

fn pairs(text: &str) -> Result<Vec<(i32, String)>, DxfError> {
    let mut lines = text.lines();
    let mut out = Vec::new();
    while let Some(code) = lines.next() {
        let code = code.trim();
        if code.is_empty() {
            continue;
        }
        let code: i32 = code
            .parse()
            .map_err(|_| DxfError(format!("not a DXF file (group code `{code}`)")))?;
        let value = lines
            .next()
            .ok_or_else(|| DxfError("the file ends in the middle of a group".into()))?;
        out.push((code, value.trim().to_owned()));
    }
    Ok(out)
}

fn units_of(code: i64) -> (&'static str, f64) {
    match code {
        1 => ("inch", 25.4e6),
        2 => ("foot", 304.8e6),
        5 => ("cm", 10e6),
        6 => ("m", 1e9),
        8 => ("microinch", 25.4),
        9 => ("mil", 25_400.0),
        13 => ("um", 1_000.0),
        4 => ("mm", 1e6),
        _ => ("unitless", 1e6),
    }
}

struct Entity {
    kind: String,
    groups: Vec<(i32, String)>,
}

impl Entity {
    fn number(&self, code: i32) -> Option<f64> {
        self.groups
            .iter()
            .find(|(c, _)| *c == code)
            .and_then(|(_, v)| v.parse().ok())
    }

    fn numbers(&self, code: i32) -> Vec<f64> {
        self.groups
            .iter()
            .filter(|(c, _)| *c == code)
            .filter_map(|(_, v)| v.parse().ok())
            .collect()
    }

    fn layer(&self) -> String {
        self.groups
            .iter()
            .find(|(c, _)| *c == 8)
            .map_or_else(|| "0".to_owned(), |(_, v)| v.clone())
    }
}

/// Reads a DXF file. `scale` overrides the file's units (nm per unit).
///
/// # Errors
///
/// The text is not a DXF file.
#[allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    reason = "one arm per entity kind; DXF geometry"
)]
pub fn read(text: &str, scale: Option<f64>) -> Result<Drawing, DxfError> {
    let groups = pairs(text)?;
    if !groups.iter().any(|(c, v)| *c == 0 && v == "SECTION") {
        return Err(DxfError("not a DXF file (no SECTION)".into()));
    }
    // Header units.
    let mut units = 0;
    for window in groups.windows(2) {
        if window[0].0 == 9 && window[0].1 == "$INSUNITS" && window[1].0 == 70 {
            units = window[1].1.parse().unwrap_or(0);
        }
    }
    let (unit_name, file_scale) = units_of(units);
    let k = scale.unwrap_or(file_scale);
    // Entities.
    let mut entities: Vec<Entity> = Vec::new();
    let mut in_entities = false;
    let mut current: Option<Entity> = None;
    for (code, value) in &groups {
        if *code == 2 && value == "ENTITIES" {
            in_entities = true;
            continue;
        }
        if *code == 0 && value == "ENDSEC" {
            if let Some(entity) = current.take() {
                entities.push(entity);
            }
            in_entities = false;
            continue;
        }
        if !in_entities {
            continue;
        }
        if *code == 0 {
            if let Some(entity) = current.take() {
                entities.push(entity);
            }
            current = Some(Entity {
                kind: value.clone(),
                groups: Vec::new(),
            });
        } else if let Some(entity) = &mut current {
            entity.groups.push((*code, value.clone()));
        }
    }
    let p = |x: f64, y: f64| V::new(x * k, -y * k);
    let mut shapes: Vec<(String, Geometry)> = Vec::new();
    let mut skipped: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    let mut layers = BTreeSet::new();
    let mut index = 0;
    while index < entities.len() {
        let e = &entities[index];
        index += 1;
        let layer = e.layer();
        let num = |code: i32| e.number(code).unwrap_or(0.0);
        let geometry: Vec<Geometry> = match e.kind.as_str() {
            "LINE" => vec![Geometry::Line {
                start: p(num(10), num(20)).point(),
                end: p(num(11), num(21)).point(),
            }],
            "CIRCLE" => vec![Geometry::Circle {
                center: p(num(10), num(20)).point(),
                radius: (num(40) * k).round() as i64,
            }],
            "ARC" => {
                let (c, r) = (V::new(num(10), num(20)), num(40));
                let (a1, a2) = (num(50).to_radians(), num(51).to_radians());
                let mut sweep = (a2 - a1).rem_euclid(TAU);
                if sweep.abs() < 1e-9 {
                    sweep = TAU;
                }
                if (sweep - TAU).abs() < 1e-9 {
                    vec![Geometry::Circle {
                        center: p(c.x, c.y).point(),
                        radius: (r * k).round() as i64,
                    }]
                } else {
                    let at = |a: f64| c.add(V::new(a.cos(), a.sin()).scale(r));
                    let (s, m, en) = (at(a1), at(a1 + sweep / 2.0), at(a1 + sweep));
                    vec![Geometry::Arc {
                        start: p(s.x, s.y).point(),
                        mid: p(m.x, m.y).point(),
                        end: p(en.x, en.y).point(),
                    }]
                }
            }
            "LWPOLYLINE" => {
                let closed = e.number(70).is_some_and(|f| (f as i64) & 1 == 1);
                // Vertices: 10/20 pairs, each optionally followed by 42.
                let mut vertices: Vec<(V, f64)> = Vec::new();
                let mut x = None;
                for (code, value) in &e.groups {
                    let v: f64 = value.parse().unwrap_or(0.0);
                    match code {
                        10 => x = Some(v),
                        20 => {
                            if let Some(xv) = x.take() {
                                vertices.push((V::new(xv, v), 0.0));
                            }
                        }
                        42 => {
                            if let Some(last) = vertices.last_mut() {
                                last.1 = v;
                            }
                        }
                        _ => {}
                    }
                }
                polyline(&vertices, closed, &p)
            }
            "POLYLINE" => {
                let closed = e.number(70).is_some_and(|f| (f as i64) & 1 == 1);
                let mut vertices = Vec::new();
                while index < entities.len() && entities[index].kind == "VERTEX" {
                    let v = &entities[index];
                    vertices.push((
                        V::new(v.number(10).unwrap_or(0.0), v.number(20).unwrap_or(0.0)),
                        v.number(42).unwrap_or(0.0),
                    ));
                    index += 1;
                }
                if index < entities.len() && entities[index].kind == "SEQEND" {
                    index += 1;
                }
                polyline(&vertices, closed, &p)
            }
            "SPLINE" => {
                let (fx, fy) = (e.numbers(11), e.numbers(21));
                let (cx, cy) = (e.numbers(10), e.numbers(20));
                let (xs, ys) = if fx.len() >= 2 { (fx, fy) } else { (cx, cy) };
                let points: Vec<Point> = xs
                    .iter()
                    .zip(ys.iter())
                    .map(|(x, y)| p(*x, *y).point())
                    .collect();
                let closed = e.number(70).is_some_and(|f| (f as i64) & 1 == 1);
                if points.len() < 2 {
                    Vec::new()
                } else if closed && points.len() >= 3 {
                    vec![Geometry::Polygon { points }]
                } else {
                    vec![Geometry::Polyline { points }]
                }
            }
            "ELLIPSE" => {
                let c = V::new(num(10), num(20));
                let major = V::new(num(11), num(21));
                let ratio = e.number(40).unwrap_or(1.0);
                let (t0, t1) = (e.number(41).unwrap_or(0.0), e.number(42).unwrap_or(TAU));
                let minor = major.perp().scale(ratio);
                let mut sweep = (t1 - t0).rem_euclid(TAU);
                if sweep.abs() < 1e-9 {
                    sweep = TAU;
                }
                let n = 64;
                let points: Vec<Point> = (0..=n)
                    .map(|i| {
                        let t = t0 + sweep * f64::from(i) / f64::from(n);
                        let q = c.add(major.scale(t.cos())).add(minor.scale(t.sin()));
                        p(q.x, q.y).point()
                    })
                    .collect();
                if (sweep - TAU).abs() < 1e-9 {
                    vec![Geometry::Polygon {
                        points: points[..points.len() - 1].to_vec(),
                    }]
                } else {
                    vec![Geometry::Polyline { points }]
                }
            }
            "VERTEX" | "SEQEND" => Vec::new(),
            other => {
                *skipped.entry(other.to_owned()).or_default() += 1;
                Vec::new()
            }
        };
        if !geometry.is_empty() {
            layers.insert(layer.clone());
        }
        shapes.extend(geometry.into_iter().map(|g| (layer.clone(), g)));
    }
    let bounds = Rect::from_points(shapes.iter().flat_map(|(_, g)| g.points()))
        .map(|r| [r.min.x, r.min.y, r.max.x, r.max.y]);
    Ok(Drawing {
        count: shapes.len(),
        shapes,
        layers: layers.into_iter().collect(),
        units: unit_name.to_owned(),
        bounds,
        skipped: skipped.into_iter().collect(),
    })
}

/// Lines and bulge arcs of a polyline.
#[allow(clippy::many_single_char_names, reason = "geometry")]
fn polyline(vertices: &[(V, f64)], closed: bool, p: &dyn Fn(f64, f64) -> V) -> Vec<Geometry> {
    let n = vertices.len();
    if n < 2 {
        return Vec::new();
    }
    let has_bulge = vertices.iter().any(|(_, b)| b.abs() > 1e-12);
    if !has_bulge {
        let points: Vec<Point> = vertices.iter().map(|(v, _)| p(v.x, v.y).point()).collect();
        return vec![if closed && n >= 3 {
            Geometry::Polygon { points }
        } else {
            Geometry::Polyline { points }
        }];
    }
    let count = if closed { n } else { n - 1 };
    (0..count)
        .map(|i| {
            let (a, bulge) = vertices[i];
            let b = vertices[(i + 1) % n].0;
            if bulge.abs() < 1e-12 {
                return Geometry::Line {
                    start: p(a.x, a.y).point(),
                    end: p(b.x, b.y).point(),
                };
            }
            // The arc's middle lies right of the chord for positive
            // (counter-clockwise) bulges.
            let d = b.sub(a);
            let chord = d.len();
            let right = V::new(d.y, -d.x).unit();
            let mid = a.add(b).scale(0.5).add(right.scale(bulge * chord / 2.0));
            Geometry::Arc {
                start: p(a.x, a.y).point(),
                mid: p(mid.x, mid.y).point(),
                end: p(b.x, b.y).point(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DXF: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
0\nLWPOLYLINE\n8\nOUTLINE\n90\n4\n70\n1\n10\n0\n20\n0\n10\n50\n20\n0\n10\n50\n20\n30\n42\n1\n10\n0\n20\n30\n\
0\nCIRCLE\n8\nHOLES\n10\n5\n20\n5\n40\n1.6\n\
0\nARC\n8\nOUTLINE\n10\n0\n20\n0\n40\n10\n50\n0\n51\n90\n\
0\nTEXT\n8\nNOTES\n1\nhello\n\
0\nENDSEC\n0\nEOF\n";

    #[test]
    fn entities_units_and_bulges() {
        let drawing = read(DXF, None).unwrap();
        assert_eq!(drawing.units, "mm");
        assert_eq!(drawing.layers, vec!["HOLES", "OUTLINE"]);
        assert_eq!(drawing.skipped, vec![("TEXT".to_owned(), 1)]);
        // The polyline has a bulge: 4 pieces (3 lines, 1 half circle).
        assert_eq!(drawing.count, 4 + 1 + 1);
        let arc = drawing
            .shapes
            .iter()
            .find_map(|(_, g)| match g {
                Geometry::Arc { start, mid, end } if start.x == 50_000_000 => {
                    Some((*start, *mid, *end))
                }
                _ => None,
            })
            .unwrap();
        // From (50, 30) to (0, 30) counter-clockwise in DXF (y up): the
        // half circle lies above, at y = 30 + 25 mm; mirrored: y = -55 mm.
        assert_eq!(arc.0, Point::new(50_000_000, -30_000_000));
        assert_eq!(arc.1, Point::new(25_000_000, -55_000_000));
        let circle = drawing.shapes.iter().find(|(l, _)| l == "HOLES").unwrap();
        assert_eq!(
            circle.1,
            Geometry::Circle {
                center: Point::new(5_000_000, -5_000_000),
                radius: 1_600_000
            }
        );
        // Inches when asked.
        let inches = read(DXF, Some(25.4e6)).unwrap();
        assert_eq!(inches.bounds.unwrap()[2], 50 * 25_400_000);
        assert!(read("hello", None).is_err());
    }
}
