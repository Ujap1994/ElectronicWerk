//! VRML 2.0 (VRML97) export: the board meshes and one shared group per
//! model, placed by `Transform` nodes (translation, rotation, scale).
//! The file is y up, as VRML viewers expect, in the chosen unit.

use std::fmt::Write as _;

use crate::glb::Material;
use crate::place::Matrix;

/// A mesh to write (millimetres, z up).
#[derive(Debug, Clone, Copy)]
pub struct Shape<'a> {
    /// Material.
    pub material: Material,
    /// Positions.
    pub positions: &'a [[f32; 3]],
    /// Normals (same count, or empty).
    pub normals: &'a [[f32; 3]],
    /// Triangles.
    pub indices: &'a [u32],
}

/// A placed model group.
#[derive(Debug, Clone)]
pub struct Instance {
    /// Name (the designator).
    pub name: String,
    /// Index of the model group.
    pub group: usize,
    /// Placement (mm, z up).
    pub matrix: Matrix,
}

/// Length units of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Units {
    /// Millimetres.
    #[default]
    Mm,
    /// Metres (the VRML standard unit).
    M,
    /// Inches.
    Inch,
}

impl Units {
    /// Scale from millimetres.
    #[must_use]
    pub const fn scale(self) -> f64 {
        match self {
            Self::Mm => 1.0,
            Self::M => 0.001,
            Self::Inch => 1.0 / 25.4,
        }
    }
}

fn num(value: f64) -> String {
    let value = if value.abs() < 1e-9 { 0.0 } else { value };
    let mut text = format!("{value:.6}");
    while text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    if text == "-0" {
        text = "0".into();
    }
    text
}

fn name(text: &str) -> String {
    // VRML node names: no spaces, no leading digit.
    let mut out: String = text
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

fn shape(out: &mut String, s: &Shape<'_>, indent: &str) {
    let m = s.material;
    let _ = writeln!(out, "{indent}Shape {{");
    let _ = writeln!(
        out,
        "{indent}  appearance Appearance {{ material Material {{"
    );
    let _ = writeln!(
        out,
        "{indent}    diffuseColor {} {} {}",
        num(f64::from(m.color[0])),
        num(f64::from(m.color[1])),
        num(f64::from(m.color[2]))
    );
    if m.color[3] < 1.0 {
        let _ = writeln!(
            out,
            "{indent}    transparency {}",
            num(f64::from(1.0 - m.color[3]))
        );
    }
    let shine = f64::from((1.0 - m.roughness).clamp(0.0, 1.0));
    let _ = writeln!(out, "{indent}    shininess {}", num(shine * 0.6));
    if m.metallic > 0.3 {
        let _ = writeln!(
            out,
            "{indent}    specularColor {} {} {}",
            num(f64::from(m.color[0])),
            num(f64::from(m.color[1])),
            num(f64::from(m.color[2]))
        );
    }
    let _ = writeln!(out, "{indent}  }} }}");
    let _ = writeln!(out, "{indent}  geometry IndexedFaceSet {{");
    let _ = writeln!(out, "{indent}    solid TRUE");
    let _ = write!(out, "{indent}    coord Coordinate {{ point [");
    for (i, p) in s.positions.iter().enumerate() {
        if i % 8 == 0 {
            let _ = write!(out, "\n{indent}      ");
        }
        let _ = write!(
            out,
            "{} {} {}, ",
            num(f64::from(p[0])),
            num(f64::from(p[1])),
            num(f64::from(p[2]))
        );
    }
    let _ = writeln!(out, "\n{indent}    ] }}");
    let _ = write!(out, "{indent}    coordIndex [");
    for (i, tri) in s.indices.chunks_exact(3).enumerate() {
        if i % 10 == 0 {
            let _ = write!(out, "\n{indent}      ");
        }
        let _ = write!(out, "{},{},{},-1, ", tri[0], tri[1], tri[2]);
    }
    let _ = writeln!(out, "\n{indent}    ]");
    if s.normals.len() == s.positions.len() && !s.normals.is_empty() {
        let _ = write!(out, "{indent}    normal Normal {{ vector [");
        for (i, n) in s.normals.iter().enumerate() {
            if i % 8 == 0 {
                let _ = write!(out, "\n{indent}      ");
            }
            let _ = write!(
                out,
                "{} {} {}, ",
                num(f64::from(n[0])),
                num(f64::from(n[1])),
                num(f64::from(n[2]))
            );
        }
        let _ = writeln!(out, "\n{indent}    ] }}");
        let _ = writeln!(out, "{indent}    normalPerVertex TRUE");
    } else {
        let _ = writeln!(out, "{indent}    creaseAngle 0.5");
    }
    let _ = writeln!(out, "{indent}  }}");
    let _ = writeln!(out, "{indent}}}");
}

/// Translation, rotation (axis and angle) and scale of a matrix that
/// rotates and scales along its own axes.
#[must_use]
pub fn decompose(m: &Matrix) -> ([f64; 3], [f64; 4], [f64; 3]) {
    let column = |c: usize| [m.at(0, c), m.at(1, c), m.at(2, c)];
    let length = |v: [f64; 3]| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let scale = [length(column(0)), length(column(1)), length(column(2))];
    let r = |row: usize, col: usize| m.at(row, col) / scale[col].max(1e-12);
    let trace = r(0, 0) + r(1, 1) + r(2, 2);
    let angle = ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos();
    let axis = if angle.abs() < 1e-9 {
        [0.0, 0.0, 1.0]
    } else if (angle - std::f64::consts::PI).abs() < 1e-6 {
        // Half turn: the axis from the diagonal.
        let x = f64::midpoint(r(0, 0), 1.0).max(0.0).sqrt();
        let y = f64::midpoint(r(1, 1), 1.0).max(0.0).sqrt();
        let z = f64::midpoint(r(2, 2), 1.0).max(0.0).sqrt();
        // Signs from the off-diagonal elements.
        let (y, z) = if x > 1e-6 {
            (y.copysign(r(0, 1)), z.copysign(r(0, 2)))
        } else if y > 1e-6 {
            (y, z.copysign(r(1, 2)))
        } else {
            (y, z)
        };
        [x, y, z]
    } else {
        let s = 2.0 * angle.sin();
        [
            (r(2, 1) - r(1, 2)) / s,
            (r(0, 2) - r(2, 0)) / s,
            (r(1, 0) - r(0, 1)) / s,
        ]
    };
    (
        [m.at(0, 3), m.at(1, 3), m.at(2, 3)],
        [axis[0], axis[1], axis[2], angle],
        scale,
    )
}

/// Writes a VRML file: `board` shapes in board coordinates, `groups` of
/// model shapes in model coordinates, `instances` placing the groups;
/// everything shifted by `origin` (mm).
#[must_use]
pub fn write(
    board: &[Shape<'_>],
    groups: &[Vec<Shape<'_>>],
    instances: &[Instance],
    origin: [f64; 2],
    units: Units,
    title: &str,
) -> String {
    let mut out = String::from("#VRML V2.0 utf8\n");
    let _ = writeln!(
        out,
        "WorldInfo {{ title \"{}\" info [ \"Generated by ElectronicWerk\" ] }}",
        title.replace('"', "'")
    );
    let s = units.scale();
    // z up (board) → y up (VRML): a quarter turn about x.
    let _ = writeln!(
        out,
        "Transform {{\n  rotation 1 0 0 -1.570796\n  scale {} {} {}\n  children [\n  Transform {{\n  translation {} {} 0\n  children [",
        num(s),
        num(s),
        num(s),
        num(origin[0]),
        num(origin[1])
    );
    for item in board {
        shape(&mut out, item, "    ");
    }
    let mut defined = vec![false; groups.len()];
    for instance in instances {
        let Some(group) = groups.get(instance.group) else {
            continue;
        };
        let (t, r, k) = decompose(&instance.matrix);
        let _ = writeln!(
            out,
            "    DEF {} Transform {{\n      translation {} {} {}\n      rotation {} {} {} {}\n      scale {} {} {}\n      children [",
            name(&instance.name),
            num(t[0]),
            num(t[1]),
            num(t[2]),
            num(r[0]),
            num(r[1]),
            num(r[2]),
            num(r[3]),
            num(k[0]),
            num(k[1]),
            num(k[2])
        );
        let model = format!("M{}", instance.group);
        if defined[instance.group] {
            let _ = writeln!(out, "        USE {model}");
        } else {
            defined[instance.group] = true;
            let _ = writeln!(out, "        DEF {model} Group {{ children [");
            for item in group {
                shape(&mut out, item, "          ");
            }
            let _ = writeln!(out, "        ] }}");
        }
        let _ = writeln!(out, "      ]\n    }}");
    }
    let _ = writeln!(out, "  ]\n  }}\n  ]\n}}");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrices_decompose_into_vrml_transforms() {
        let m = Matrix::translation(1.0, 2.0, 3.0)
            .mul(&Matrix::rotation(2, std::f64::consts::FRAC_PI_2))
            .mul(&Matrix::scale(2.0, 2.0, 2.0));
        let (t, r, s) = decompose(&m);
        assert_eq!(t, [1.0, 2.0, 3.0]);
        assert!((r[3] - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
        assert!((r[2] - 1.0).abs() < 1e-9);
        assert!((s[0] - 2.0).abs() < 1e-9);
        // A half turn about y (bottom-side parts).
        let (_, r, _) = decompose(&Matrix::rotation(1, std::f64::consts::PI));
        assert!((r[1].abs() - 1.0).abs() < 1e-6, "{r:?}");
        assert!((r[3] - std::f64::consts::PI).abs() < 1e-6);
    }

    #[test]
    fn writes_shared_models() {
        let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let shape = Shape {
            material: Material::plain([0.5, 0.5, 0.5, 1.0]),
            positions: &positions,
            normals: &[],
            indices: &[0, 1, 2],
        };
        let instances = vec![
            Instance {
                name: "R1".into(),
                group: 0,
                matrix: Matrix::IDENTITY,
            },
            Instance {
                name: "R 2".into(),
                group: 0,
                matrix: Matrix::translation(5.0, 0.0, 0.0),
            },
        ];
        let text = write(
            &[shape],
            &[vec![shape]],
            &instances,
            [0.0, 0.0],
            Units::Mm,
            "t",
        );
        assert!(text.starts_with("#VRML V2.0 utf8"));
        assert!(text.contains("DEF M0 Group"));
        assert!(text.contains("USE M0"));
        assert!(text.contains("DEF R_2 Transform"));
        assert_eq!(text.matches("IndexedFaceSet").count(), 2);
    }
}
