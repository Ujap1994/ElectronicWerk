//! Tessellation of STEP files whose faces are all planes bounded by
//! straight edges — the models ElectronicWerk writes (package bodies of the
//! part generator, synthetic enclosures) and many simple mechanical parts —
//! without `OpenCASCADE`, so the engine and the command line can check them.
//! Files with curved faces are refused; the desktop app tessellates those.

use std::collections::HashMap;

use super::part21::{self, Record, reference};
use crate::mesh::Mesh;

/// Why a file cannot be tessellated here.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TessellateError {
    /// Not a readable STEP file.
    #[error("not a STEP file: {0}")]
    Parse(String),
    /// A face or edge that is not flat or straight.
    #[error("the model has curved faces ({0}); it needs the app's tessellation")]
    Curved(String),
    /// No faces at all.
    #[error("the model has no faces")]
    Empty,
}

type Index<'a> = HashMap<u64, &'a Record>;

fn numbers(arg: &str) -> Vec<f64> {
    arg.trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split(',')
        .filter_map(|v| v.trim().parse::<f64>().ok())
        .collect()
}

fn list(arg: &str) -> Vec<u64> {
    part21::split_args(arg.trim().trim_start_matches('(').trim_end_matches(')'))
        .into_iter()
        .filter_map(reference)
        .collect()
}

fn point(index: &Index<'_>, id: u64) -> Option<[f64; 3]> {
    let r = index.get(&id)?;
    if r.kind() != "CARTESIAN_POINT" {
        return None;
    }
    let v = numbers(r.args().get(1)?);
    (v.len() >= 3).then(|| [v[0], v[1], v[2]])
}

fn direction(index: &Index<'_>, id: u64) -> Option<[f64; 3]> {
    let r = index.get(&id)?;
    let v = numbers(r.args().get(1)?);
    (v.len() >= 3).then(|| [v[0], v[1], v[2]])
}

fn vertex(index: &Index<'_>, id: u64) -> Option<[f64; 3]> {
    let r = index.get(&id)?;
    point(index, reference(r.args().get(1)?)?)
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = dot(a, a).sqrt().max(1e-12);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// The points of a loop, in its direction.
fn loop_points(index: &Index<'_>, id: u64) -> Result<Vec<[f64; 3]>, TessellateError> {
    let bad = |what: &str| TessellateError::Parse(format!("{what} #{id}"));
    let r = index.get(&id).ok_or_else(|| bad("missing loop"))?;
    let args = r.args();
    match r.kind() {
        "POLY_LOOP" => Ok(list(args.get(1).copied().unwrap_or("()"))
            .into_iter()
            .filter_map(|p| point(index, p))
            .collect()),
        "EDGE_LOOP" => {
            let mut out = Vec::new();
            for oriented in list(args.get(1).copied().unwrap_or("()")) {
                let o = index.get(&oriented).ok_or_else(|| bad("missing edge"))?;
                let oa = o.args();
                let edge_id = oa
                    .get(3)
                    .and_then(|a| reference(a))
                    .ok_or_else(|| bad("edge"))?;
                let forward = oa.get(4).is_none_or(|a| a.trim() == ".T.");
                let edge = index
                    .get(&edge_id)
                    .ok_or_else(|| bad("missing edge curve"))?;
                let ea = edge.args();
                let curve = ea
                    .get(3)
                    .and_then(|a| reference(a))
                    .and_then(|c| index.get(&c))
                    .map_or("", |c| c.kind());
                if curve != "LINE" && curve != "POLYLINE" {
                    return Err(TessellateError::Curved(curve.to_owned()));
                }
                let start = ea.get(1).and_then(|a| reference(a));
                let end = ea.get(2).and_then(|a| reference(a));
                let first = if forward { start } else { end };
                let p = first
                    .and_then(|v| vertex(index, v))
                    .ok_or_else(|| bad("vertex"))?;
                out.push(p);
            }
            Ok(out)
        }
        other => Err(TessellateError::Curved(other.to_owned())),
    }
}

/// Millimetres per unit of the file's length unit.
fn unit_scale(text: &str) -> f64 {
    let upper = text.to_ascii_uppercase();
    if upper.contains("'INCH'") {
        25.4
    } else if upper.contains("SI_UNIT(.MILLI.,.METRE.)")
        || upper.contains("SI_UNIT(.MILLI., .METRE.)")
    {
        1.0
    } else if upper.contains("SI_UNIT($,.METRE.)") {
        1000.0
    } else {
        1.0
    }
}

/// A mesh of every face of a STEP file whose faces are planar polygons.
///
/// # Errors
///
/// Unreadable files, curved faces or edges, files without faces.
#[allow(clippy::too_many_lines, reason = "one pass over the faces")]
pub fn planar_mesh(text: &str) -> Result<Mesh, TessellateError> {
    let file = part21::parse(text).map_err(|e| TessellateError::Parse(e.0))?;
    let index: Index<'_> = file.records.iter().map(|r| (r.id, r)).collect();
    let scale = unit_scale(text);
    let mut mesh = Mesh::default();
    let mut faces = 0;
    for record in &file.records {
        let kind = record.kind();
        if kind != "ADVANCED_FACE" && kind != "FACE_SURFACE" {
            continue;
        }
        let args = record.args();
        let surface = args
            .get(2)
            .and_then(|a| reference(a))
            .and_then(|id| index.get(&id))
            .ok_or_else(|| TessellateError::Parse(format!("face #{}", record.id)))?;
        if surface.kind() != "PLANE" {
            return Err(TessellateError::Curved(surface.kind().to_owned()));
        }
        let same_sense = args.get(3).is_none_or(|a| a.trim() == ".T.");
        let placement = surface
            .args()
            .get(1)
            .and_then(|a| reference(a))
            .and_then(|id| index.get(&id))
            .ok_or_else(|| TessellateError::Parse(format!("plane #{}", surface.id)))?;
        let pa = placement.args();
        let axis = pa
            .get(2)
            .and_then(|a| reference(a))
            .and_then(|id| direction(&index, id))
            .unwrap_or([0.0, 0.0, 1.0]);
        let normal = unit(if same_sense {
            axis
        } else {
            [-axis[0], -axis[1], -axis[2]]
        });
        // A basis of the plane for the triangulation.
        let helper = if normal[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let u = unit(cross(helper, normal));
        let v = cross(normal, u);
        let mut rings: Vec<Vec<[f64; 3]>> = Vec::new();
        for bound in list(args.get(1).copied().unwrap_or("()")) {
            let b = index
                .get(&bound)
                .ok_or_else(|| TessellateError::Parse(format!("bound #{bound}")))?;
            let ba = b.args();
            let loop_id = ba
                .get(1)
                .and_then(|a| reference(a))
                .ok_or_else(|| TessellateError::Parse(format!("bound #{bound}")))?;
            let mut points = loop_points(&index, loop_id)?;
            if ba.get(2).is_some_and(|a| a.trim() == ".F.") {
                points.reverse();
            }
            if points.len() >= 3 {
                // The outer loop first.
                if b.kind() == "FACE_OUTER_BOUND" {
                    rings.insert(0, points);
                } else {
                    rings.push(points);
                }
            }
        }
        if rings.is_empty() {
            continue;
        }
        faces += 1;
        // Earcut in the plane: outer ring counter-clockwise around the
        // normal, holes after it.
        let mut flat: Vec<[f64; 2]> = Vec::new();
        let mut holes: Vec<u32> = Vec::new();
        let mut all: Vec<[f64; 3]> = Vec::new();
        for (i, ring) in rings.iter().enumerate() {
            if i > 0 {
                holes.push(u32::try_from(all.len()).unwrap_or(u32::MAX));
            }
            for p in ring {
                flat.push([dot(*p, u), dot(*p, v)]);
                all.push(*p);
            }
        }
        let mut triangles: Vec<u32> = Vec::new();
        earcut::Earcut::new().earcut_slice(&flat, &holes, &mut triangles);
        let base = u32::try_from(mesh.positions.len()).unwrap_or(u32::MAX);
        for p in &all {
            mesh.positions.push([
                (p[0] * scale) as f32,
                (p[1] * scale) as f32,
                (p[2] * scale) as f32,
            ]);
            mesh.normals
                .push([normal[0] as f32, normal[1] as f32, normal[2] as f32]);
        }
        for t in triangles.chunks_exact(3) {
            // Wind the triangle so it faces along the normal.
            let at = |i: u32| all[i as usize];
            let (a, b, c) = (at(t[0]), at(t[1]), at(t[2]));
            let facing = dot(cross(sub(b, a), sub(c, a)), normal) >= 0.0;
            let (i, j, k) = if facing {
                (t[0], t[1], t[2])
            } else {
                (t[0], t[2], t[1])
            };
            mesh.indices.extend([base + i, base + j, base + k]);
        }
    }
    if faces == 0 {
        return Err(TessellateError::Empty);
    }
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::solids::{Prism, file};

    #[test]
    fn tessellates_prisms_with_holes() {
        // A 10 × 10 × 2 plate with a 4 × 4 hole.
        let square = |x0: f64, y0: f64, s: f64| {
            vec![
                [x0, y0, 0.0],
                [x0 + s, y0, 0.0],
                [x0 + s, y0 + s, 0.0],
                [x0, y0 + s, 0.0],
            ]
        };
        let plate = Prism {
            outline: square(0.0, 0.0, 10.0),
            holes: vec![square(3.0, 3.0, 4.0)],
            extrude: [0.0, 0.0, 2.0],
            color: [0.5, 0.5, 0.5],
        };
        let text = file("plate", &[plate], "test");
        let mesh = planar_mesh(&text).unwrap_or_else(|e| panic!("{e}"));
        // Top and bottom: 8 triangles each; 8 side faces of 2 triangles.
        assert_eq!(mesh.triangles(), 8 + 8 + 16);
        let bounds = mesh.bounds().unwrap_or([0.0; 6]);
        assert_eq!(bounds, [0.0, 0.0, 0.0, 10.0, 10.0, 2.0]);
        // A closed surface: the signed volume is the plate's volume.
        let mut volume = 0.0;
        for t in mesh.indices.chunks_exact(3) {
            let p = |i: u32| {
                let q = mesh.positions[i as usize];
                [f64::from(q[0]), f64::from(q[1]), f64::from(q[2])]
            };
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            volume += dot(a, cross(b, c)) / 6.0;
        }
        assert!((volume - (100.0 - 16.0) * 2.0).abs() < 1e-6, "{volume}");
    }

    #[test]
    fn refuses_curved_faces() {
        let text = "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n#1 = CYLINDRICAL_SURFACE('',#2,1.);\n#3 = ADVANCED_FACE('',(),#1,.T.);\nENDSEC;\nEND-ISO-10303-21;\n";
        assert!(matches!(planar_mesh(text), Err(TessellateError::Curved(_))));
    }
}
