//! A simple box enclosure as STEP files: a bottom shell with a floor, walls,
//! bosses for the board and an optional opening in the front wall, and a
//! flat lid. Used for a first fit check, for tests and for the
//! documentation (no real enclosure is in the repository).
//!
//! Model coordinates: millimetres, the origin in the middle of the floor's
//! top face, z up. A board resting on the bosses has its bottom face at
//! z = `standoff`.

use serde::{Deserialize, Serialize};

use crate::step::solids::{Prism, file};

/// The dimensions of a box enclosure (mm).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BoxSpec {
    /// Inner width (x).
    pub width: f64,
    /// Inner depth (y).
    pub depth: f64,
    /// Inner height from the floor to the lid.
    pub height: f64,
    /// Wall, floor and lid thickness.
    pub wall: f64,
    /// Height of the bosses: where the board's bottom face rests.
    pub standoff: f64,
    /// Centres of the bosses (x, y).
    pub bosses: Vec<[f64; 2]>,
    /// Outer diameter of the bosses.
    pub boss_diameter: f64,
    /// Diameter of the holes in the bosses.
    pub hole_diameter: f64,
    /// An opening in the front wall (y min): centre x, centre z, width,
    /// height.
    pub opening: Option<[f64; 4]>,
}

impl Default for BoxSpec {
    /// A box for a board of 40 × 30 mm (the blinky example) with bosses
    /// 3.5 mm inside its corners.
    fn default() -> Self {
        Self {
            width: 41.0,
            depth: 31.0,
            height: 16.0,
            wall: 2.0,
            standoff: 4.0,
            bosses: vec![[-16.5, -11.5], [16.5, -11.5], [16.5, 11.5], [-16.5, 11.5]],
            boss_diameter: 4.5,
            hole_diameter: 2.2,
            opening: Some([-6.0, 7.5, 10.0, 4.0]),
        }
    }
}

const GREY: [f32; 3] = [0.55, 0.57, 0.6];
const LID: [f32; 3] = [0.7, 0.72, 0.75];

fn rect(x0: f64, y0: f64, x1: f64, y1: f64, z: f64) -> Vec<[f64; 3]> {
    vec![[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]]
}

fn circle(c: [f64; 2], d: f64, z: f64) -> Vec<[f64; 3]> {
    let r = d / 2.0;
    (0..16)
        .map(|i| {
            let a = f64::from(i) * std::f64::consts::TAU / 16.0;
            [c[0] + r * a.cos(), c[1] + r * a.sin(), z]
        })
        .collect()
}

/// The prisms of the bottom shell.
#[must_use]
pub fn bottom_prisms(s: &BoxSpec) -> Vec<Prism> {
    let (hw, hd, w) = (s.width / 2.0, s.depth / 2.0, s.wall);
    let mut out = vec![Prism {
        outline: rect(-hw - w, -hd - w, hw + w, hd + w, -w),
        holes: Vec::new(),
        extrude: [0.0, 0.0, w],
        color: GREY,
    }];
    // Front and back walls over the whole width, the side walls between
    // them, so no two solids overlap.
    let front_back = |y0: f64, opening: Option<[f64; 4]>| {
        // In the x–z plane, extruded along y.
        let outline = vec![
            [-hw - w, y0, 0.0],
            [hw + w, y0, 0.0],
            [hw + w, y0, s.height],
            [-hw - w, y0, s.height],
        ];
        let holes = opening
            .map(|[x, z, ow, oh]| {
                vec![vec![
                    [x - ow / 2.0, y0, z - oh / 2.0],
                    [x + ow / 2.0, y0, z - oh / 2.0],
                    [x + ow / 2.0, y0, z + oh / 2.0],
                    [x - ow / 2.0, y0, z + oh / 2.0],
                ]]
            })
            .unwrap_or_default();
        Prism {
            outline,
            holes,
            extrude: [0.0, w, 0.0],
            color: GREY,
        }
    };
    out.push(front_back(-hd - w, s.opening));
    out.push(front_back(hd, None));
    for x0 in [-hw - w, hw] {
        out.push(Prism {
            outline: vec![
                [x0, -hd, 0.0],
                [x0, hd, 0.0],
                [x0, hd, s.height],
                [x0, -hd, s.height],
            ],
            holes: Vec::new(),
            extrude: [w, 0.0, 0.0],
            color: GREY,
        });
    }
    for boss in &s.bosses {
        out.push(Prism {
            outline: circle(*boss, s.boss_diameter, 0.0),
            holes: if s.hole_diameter > 0.0 && s.hole_diameter < s.boss_diameter {
                vec![circle(*boss, s.hole_diameter, 0.0)]
            } else {
                Vec::new()
            },
            extrude: [0.0, 0.0, s.standoff],
            color: GREY,
        });
    }
    out
}

/// The prisms of the lid: a plate on the walls.
#[must_use]
pub fn lid_prisms(s: &BoxSpec) -> Vec<Prism> {
    let (hw, hd, w) = (s.width / 2.0, s.depth / 2.0, s.wall);
    vec![Prism {
        outline: rect(-hw - w, -hd - w, hw + w, hd + w, s.height),
        holes: Vec::new(),
        extrude: [0.0, 0.0, w],
        color: LID,
    }]
}

/// The bottom shell and the lid as STEP files.
#[must_use]
pub fn step_files(s: &BoxSpec, version: &str) -> (String, String) {
    (
        file("Enclosure bottom", &bottom_prisms(s), version),
        file("Enclosure lid", &lid_prisms(s), version),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::step::tessellate::planar_mesh;

    #[test]
    fn writes_closed_shells() {
        let spec = BoxSpec::default();
        let (bottom, lid) = step_files(&spec, "test");
        for text in [&bottom, &lid] {
            let mesh = planar_mesh(text).unwrap_or_else(|e| panic!("{e}"));
            // Every edge is shared by exactly two triangles in opposite
            // directions inside each solid: the signed volume is positive.
            let mut volume = 0.0;
            for t in mesh.indices.chunks_exact(3) {
                let p = |i: u32| {
                    let q = mesh.positions[i as usize];
                    [f64::from(q[0]), f64::from(q[1]), f64::from(q[2])]
                };
                let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
                volume += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                    + a[2] * (b[0] * c[1] - b[1] * c[0]))
                    / 6.0;
            }
            assert!(volume > 0.0, "{volume}");
        }
        let mesh = planar_mesh(&bottom).unwrap_or_else(|e| panic!("{e}"));
        let b = mesh.bounds().unwrap_or([0.0; 6]);
        assert_eq!(b, [-22.5, -17.5, -2.0, 22.5, 17.5, 16.0]);
    }
}
