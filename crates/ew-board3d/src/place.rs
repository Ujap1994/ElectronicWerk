//! Where the 3D models of the footprints go.
//!
//! A model is placed like KiCad does it: the footprint's position, its
//! rotation about z, for parts on the bottom a half turn about the
//! footprint's y axis (which mirrors x and z, as the bottom side mirrors the
//! footprint), then the model's own offset, its rotation by the negated
//! angles (z, then y, then x) and its scale. The model's z = 0 sits on the
//! outer copper of its side.

use ew_model::id::Id;
use ew_model::layer::Side;
use ew_model::pcb::{FootprintInstance, Model3d, Pcb};
use ew_pcb::geom::{BBox, V};
use ew_pcb::items::pad_shape;
use serde::Serialize;

use crate::mm;
use crate::stack::Levels;

/// A 4×4 matrix, column-major (as glTF and three.js store them).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix(pub [f64; 16]);

impl Serialize for Matrix {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Rounded, so the JSON stays short and stable.
        let rounded: Vec<f64> = self.0.iter().map(|v| (v * 1e9).round() / 1e9).collect();
        rounded.serialize(serializer)
    }
}

impl Matrix {
    /// The identity.
    pub const IDENTITY: Self = Self([
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);

    /// The element in `row`, `column`.
    #[must_use]
    pub const fn at(&self, row: usize, column: usize) -> f64 {
        self.0[column * 4 + row]
    }

    /// A translation.
    #[must_use]
    pub const fn translation(x: f64, y: f64, z: f64) -> Self {
        let mut m = Self::IDENTITY;
        m.0[12] = x;
        m.0[13] = y;
        m.0[14] = z;
        m
    }

    /// A rotation about the x (0), y (1) or z (2) axis.
    #[must_use]
    pub fn rotation(axis: usize, radians: f64) -> Self {
        let (s, c) = radians.sin_cos();
        // Snap the exact quarter turns, so 90° gives clean matrices.
        let snap = |v: f64| if v.abs() < 1e-15 { 0.0 } else { v };
        let (s, c) = (snap(s), snap(c));
        let mut m = Self::IDENTITY;
        let (i, j) = match axis {
            0 => (1, 2),
            1 => (2, 0),
            _ => (0, 1),
        };
        m.0[i * 4 + i] = c;
        m.0[j * 4 + j] = c;
        // Column i gets (c, s) in rows (i, j); column j gets (-s, c).
        m.0[i * 4 + j] = s;
        m.0[j * 4 + i] = -s;
        m
    }

    /// A scale.
    #[must_use]
    pub const fn scale(x: f64, y: f64, z: f64) -> Self {
        let mut m = Self::IDENTITY;
        m.0[0] = x;
        m.0[5] = y;
        m.0[10] = z;
        m
    }

    /// `self · other` (apply `other` first).
    #[must_use]
    pub fn mul(&self, other: &Self) -> Self {
        let mut out = [0.0; 16];
        for column in 0..4 {
            for row in 0..4 {
                out[column * 4 + row] = (0..4).map(|k| self.at(row, k) * other.at(k, column)).sum();
            }
        }
        Self(out)
    }

    /// Maps a point.
    #[must_use]
    pub fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        let mut out = [0.0; 3];
        for (row, value) in out.iter_mut().enumerate() {
            *value = self.at(row, 0) * p[0]
                + self.at(row, 1) * p[1]
                + self.at(row, 2) * p[2]
                + self.at(row, 3);
        }
        out
    }

    /// Maps a direction (no translation).
    #[must_use]
    pub fn apply_direction(&self, d: [f64; 3]) -> [f64; 3] {
        let mut out = [0.0; 3];
        for (row, value) in out.iter_mut().enumerate() {
            *value = self.at(row, 0) * d[0] + self.at(row, 1) * d[1] + self.at(row, 2) * d[2];
        }
        out
    }

    /// `true` if the matrix only rotates and translates (no scale, no
    /// mirror), within `epsilon`.
    #[must_use]
    pub fn is_rigid(&self, epsilon: f64) -> bool {
        let column = |c: usize| [self.at(0, c), self.at(1, c), self.at(2, c)];
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let (x, y, z) = (column(0), column(1), column(2));
        let cross = [
            x[1] * y[2] - x[2] * y[1],
            x[2] * y[0] - x[0] * y[2],
            x[0] * y[1] - x[1] * y[0],
        ];
        (dot(x, x) - 1.0).abs() < epsilon
            && (dot(y, y) - 1.0).abs() < epsilon
            && (dot(z, z) - 1.0).abs() < epsilon
            && dot(x, y).abs() < epsilon
            && dot(cross, z) > 1.0 - epsilon
    }
}

/// The placement of a footprint on the board (without its model's own
/// offset, rotation and scale).
#[must_use]
pub fn footprint_matrix(fp: &FootprintInstance, levels: &Levels) -> Matrix {
    let at = fp.at;
    let base = Matrix::translation(mm(at.x as f64), mm(at.y as f64), levels.mount(fp.side))
        .mul(&Matrix::rotation(2, fp.rotation.degrees().to_radians()));
    match fp.side {
        Side::Top => base,
        Side::Bottom => base.mul(&Matrix::rotation(1, std::f64::consts::PI)),
    }
}

/// The transformation of a model within its footprint.
#[must_use]
pub fn model_local(model: &Model3d) -> Matrix {
    let o = model.offset;
    let [rx, ry, rz] = model.rotation.map(|a| -a.degrees().to_radians());
    let [sx, sy, sz] = model.scale.0;
    Matrix::translation(mm(o.x as f64), mm(o.y as f64), mm(o.z as f64))
        .mul(&Matrix::rotation(2, rz))
        .mul(&Matrix::rotation(1, ry))
        .mul(&Matrix::rotation(0, rx))
        .mul(&Matrix::scale(sx, sy, sz))
}

/// One model of a placed part.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacedModel {
    /// The model path as the footprint names it.
    pub path: String,
    /// Model coordinates (mm, z up) → board coordinates (mm).
    pub matrix: Matrix,
}

/// A part on the board with its models.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    /// Footprint id.
    pub id: Id,
    /// Reference designator.
    pub designator: String,
    /// Value.
    pub value: String,
    /// Footprint name.
    pub footprint: String,
    /// Board side.
    pub side: Side,
    /// Not populated.
    pub dnp: bool,
    /// The footprint placement (for parts without a model).
    pub matrix: Matrix,
    /// The pads' box in footprint coordinates (mm) `[x0, y0, x1, y1]`: a
    /// stand-in for picking and for parts without a model.
    pub pads: [f64; 4],
    /// Shown models.
    pub models: Vec<PlacedModel>,
}

/// The parts of a board.
#[must_use]
pub fn parts(pcb: &Pcb, levels: &Levels) -> Vec<Part> {
    let mut out: Vec<Part> = pcb.footprints.iter().map(|fp| part(fp, levels)).collect();
    out.sort_by(|a, b| natural(&a.designator, &b.designator).then(a.id.cmp(&b.id)));
    out
}

/// One part.
#[must_use]
pub fn part(fp: &FootprintInstance, levels: &Levels) -> Part {
    let placement = footprint_matrix(fp, levels);
    let models = fp
        .models
        .iter()
        .filter(|model| !model.hidden && !model.path.trim().is_empty())
        .map(|model| PlacedModel {
            path: model.path.clone(),
            matrix: placement.mul(&model_local(model)),
        })
        .collect();
    Part {
        id: fp.id,
        designator: fp.designator.clone(),
        value: fp.value.clone(),
        footprint: fp.footprint.clone(),
        side: fp.side,
        dnp: fp.attributes.dnp,
        matrix: placement,
        pads: pad_box(fp),
        models,
    }
}

/// The box of the pads in the footprint's own coordinates (mm).
fn pad_box(fp: &FootprintInstance) -> [f64; 4] {
    let local = ew_pcb::geom::Xform::IDENTITY;
    let mut bbox = BBox::EMPTY;
    for pad in &fp.pads {
        bbox = bbox.union(pad_shape(pad, &local, 0.0).bbox());
    }
    if bbox.is_empty() {
        // Parts without pads (logos, mounting holes): a 1 mm square.
        bbox = BBox::of(V::new(-0.5e6, -0.5e6)).with(V::new(0.5e6, 0.5e6));
    }
    [
        mm(bbox.min.x),
        mm(bbox.min.y),
        mm(bbox.max.x),
        mm(bbox.max.y),
    ]
}

/// Orders designators naturally (`R2` before `R10`).
#[must_use]
pub fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let split = |s: &str| -> (String, u64, String) {
        let prefix: String = s.chars().take_while(|c| !c.is_ascii_digit()).collect();
        let rest = &s[prefix.len()..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        let number = digits.parse().unwrap_or(0);
        (prefix, number, rest[digits.len()..].to_owned())
    };
    split(a).cmp(&split(b)).then_with(|| a.cmp(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_model::pcb::{STANDARD_THICKNESS, Scale3, Stackup};
    use ew_model::units::{Angle, Point, Vec3};

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-9)
    }

    #[test]
    fn rotations_turn_counter_clockwise() {
        let z = Matrix::rotation(2, std::f64::consts::FRAC_PI_2);
        assert!(close(z.apply([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
        let x = Matrix::rotation(0, std::f64::consts::FRAC_PI_2);
        assert!(close(x.apply([0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]));
        let y = Matrix::rotation(1, std::f64::consts::FRAC_PI_2);
        assert!(close(y.apply([0.0, 0.0, 1.0]), [1.0, 0.0, 0.0]));
        let t = Matrix::translation(1.0, 2.0, 3.0).mul(&z);
        assert!(close(t.apply([1.0, 0.0, 0.0]), [1.0, 3.0, 3.0]));
        assert!(t.is_rigid(1e-9));
        assert!(!Matrix::scale(1.0, 1.0, 2.0).is_rigid(1e-9));
        assert!(!Matrix::scale(-1.0, 1.0, 1.0).is_rigid(1e-9));
    }

    #[test]
    fn bottom_parts_hang_under_the_board_mirrored() {
        let stack = Stackup::standard(2, STANDARD_THICKNESS);
        let levels = Levels::of(&stack);
        let mut fp = FootprintInstance {
            id: Id::random(),
            component: None,
            footprint: "R".into(),
            designator: "R1".into(),
            value: String::new(),
            at: Point::mm(10, 20),
            rotation: Angle::ZERO,
            side: Side::Top,
            locked: false,
            pads: Vec::new(),
            graphics: Vec::new(),
            texts: Vec::new(),
            models: vec![Model3d {
                path: "x.step".into(),
                offset: Vec3 {
                    x: 1_000_000,
                    y: 0,
                    z: 0,
                },
                rotation: [Angle::ZERO; 3],
                scale: Scale3::default(),
                hidden: false,
            }],
            properties: std::collections::BTreeMap::new(),
            attributes: ew_model::pcb::FootprintAttributes::default(),
        };
        let top = part(&fp, &levels);
        let m = top.models[0].matrix;
        // The model's +x offset and its top (z = 1) stay on +x and above.
        assert!(close(
            m.apply([0.0, 0.0, 1.0]),
            [11.0, 20.0, levels.mount(Side::Top) + 1.0]
        ));
        fp.side = Side::Bottom;
        let bottom = part(&fp, &levels);
        let m = bottom.models[0].matrix;
        // Mirrored in x like the pads, and hanging down.
        assert!(close(
            m.apply([0.0, 0.0, 1.0]),
            [9.0, 20.0, levels.mount(Side::Bottom) - 1.0]
        ));
        assert!(m.is_rigid(1e-9));
        assert_eq!(natural("R2", "R10"), std::cmp::Ordering::Less);
    }
}
