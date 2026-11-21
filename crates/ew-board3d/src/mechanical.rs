//! The 3D assembly of a board with its mechanical bodies (enclosure parts,
//! panels, screws, carrier boards) and the enclosure check (M17).
//!
//! Every body that is checked is compared with the board body and with
//! every populated part: an **overlap** is a collision, surfaces closer than
//! the minimum clearance are reported with their distance. The board may
//! touch a body (it rests on bosses, screws clamp it); a part touching a
//! body is reported as a clearance of 0.
//!
//! Parts are checked with their 3D models. A part without a model, or whose
//! model has no mesh, stands in as a box: its courtyard (or the box of its
//! pads) with its height — the footprint property `Height` in millimetres,
//! else a default.

use std::sync::Arc;

use ew_model::id::Id;
use ew_model::layer::Layer;
use ew_model::pcb::{FootprintInstance, MechanicalBody, Pcb};
use rayon::prelude::*;
use serde::Serialize;

use crate::collide::{self, Bounds, Meeting, P, Solid, Tri};
use crate::mesh::Mesh;
use crate::mm;
use crate::place::{self, Matrix, natural};
use crate::stack::Levels;

/// Where a body's model goes: its position, then the rotations about x,
/// y and z (in that order).
#[must_use]
pub fn body_matrix(body: &MechanicalBody) -> Matrix {
    let p = body.position;
    let [rx, ry, rz] = body.rotation.map(|a| a.degrees().to_radians());
    #[allow(clippy::cast_precision_loss, reason = "board coordinates fit f64")]
    Matrix::translation(mm(p.x as f64), mm(p.y as f64), mm(p.z as f64))
        .mul(&Matrix::rotation(2, rz))
        .mul(&Matrix::rotation(1, ry))
        .mul(&Matrix::rotation(0, rx))
}

/// The height of a part without a model: the footprint property `Height`
/// (`1.2`, `1.2 mm`, `1,2mm`), if any.
#[must_use]
pub fn part_height(fp: &FootprintInstance) -> Option<f64> {
    let text = fp
        .properties
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("height"))
        .map(|(_, v)| v.trim().to_ascii_lowercase())?;
    let number = text.trim_end_matches("mm").trim().replace(',', ".");
    number
        .parse::<f64>()
        .ok()
        .filter(|h| *h > 0.0 && *h < 1000.0)
}

/// The courtyard box of a footprint in its own coordinates (mm), else the
/// box of its pads.
fn footprint_box(fp: &FootprintInstance, pads: [f64; 4]) -> [f64; 4] {
    let mut bounds: Option<ew_model::units::Rect> = None;
    for graphic in &fp.graphics {
        if matches!(graphic.layer, Layer::Courtyard(_))
            && let Some(b) = graphic.geometry.bounds()
        {
            bounds = Some(bounds.map_or(b, |a| a.union(b)));
        }
    }
    #[allow(clippy::cast_precision_loss, reason = "board coordinates fit f64")]
    bounds.map_or(pads, |b| {
        [
            mm(b.min.x as f64),
            mm(b.min.y as f64),
            mm(b.max.x as f64),
            mm(b.max.y as f64),
        ]
    })
}

/// How a part is checked.
#[derive(Debug, Clone)]
pub enum Shape {
    /// A model mesh (model coordinates) with its placement.
    Mesh(Arc<Mesh>, Matrix),
    /// A stand-in box (triangles in board coordinates).
    Box(Vec<Tri>),
}

impl Shape {
    fn bounds(&self) -> Bounds {
        match self {
            Self::Mesh(mesh, matrix) => Bounds::of_mesh(mesh, matrix),
            Self::Box(tris) => {
                let mut b = Bounds::EMPTY;
                for t in tris {
                    for p in t {
                        b.add(*p);
                    }
                }
                b
            }
        }
    }

    fn meet(&self, solid: &Solid, limit: f64) -> Option<Meeting> {
        let bounds = self.bounds();
        match self {
            Self::Mesh(mesh, matrix) => {
                collide::meet(collide::triangles(mesh, matrix), &bounds, solid, limit)
            }
            Self::Box(tris) => collide::meet(tris.iter().copied(), &bounds, solid, limit),
        }
    }
}

/// A part as the check sees it.
#[derive(Debug, Clone)]
pub struct PartShape {
    /// Footprint id.
    pub id: Id,
    /// Reference designator.
    pub designator: String,
    /// Its shapes.
    pub shapes: Vec<Shape>,
    /// Checked as a box (no model, or a model without a mesh).
    pub stand_in: bool,
}

/// The shapes of the populated parts. `meshes` gives the mesh of a model
/// path (model coordinates, mm); `height` is the height of stand-in boxes
/// without a `Height` property (mm).
pub fn part_shapes(
    pcb: &Pcb,
    levels: &Levels,
    meshes: &dyn Fn(&str) -> Option<Arc<Mesh>>,
    height: f64,
) -> Vec<PartShape> {
    let mut out = Vec::new();
    let mut footprints: Vec<&FootprintInstance> = pcb
        .footprints
        .iter()
        .filter(|fp| !fp.attributes.dnp)
        .collect();
    footprints.sort_by(|a, b| natural(&a.designator, &b.designator).then(a.id.cmp(&b.id)));
    for fp in footprints {
        let part = place::part(fp, levels);
        let mut shapes = Vec::new();
        let mut complete = !part.models.is_empty();
        for model in &part.models {
            match meshes(&model.path) {
                Some(mesh) if !mesh.is_empty() => shapes.push(Shape::Mesh(mesh, model.matrix)),
                _ => complete = false,
            }
        }
        if !complete {
            shapes.clear();
            let [x0, y0, x1, y1] = footprint_box(fp, part.pads);
            let h = part_height(fp).unwrap_or(height);
            shapes.push(Shape::Box(collide::box_triangles(
                [x0, y0, 0.0],
                [x1, y1, h],
                &part.matrix,
            )));
        }
        out.push(PartShape {
            id: fp.id,
            designator: fp.designator.clone(),
            shapes,
            stand_in: !complete,
        });
    }
    out
}

/// What a body meets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Other {
    /// The board body.
    Board,
    /// A part.
    Part {
        /// Footprint id.
        id: Id,
    },
}

/// One finding of the enclosure check.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    /// The mechanical body.
    pub body: Id,
    /// What it meets.
    pub other: Other,
    /// `true` for an overlap (a collision).
    pub overlap: bool,
    /// The distance of the surfaces (mm; 0 for overlaps and touching).
    pub distance: f64,
    /// Where (mm, board coordinates).
    pub point: P,
    /// The closest point on the other object (the same as `point` for
    /// overlaps).
    pub other_point: P,
}

/// The input of a check.
#[derive(Debug)]
pub struct Input<'a> {
    /// The board body (board coordinates), if the board has an outline.
    pub board: Option<&'a Mesh>,
    /// The checked bodies with their solids (board coordinates).
    pub bodies: Vec<(Id, Arc<Solid>)>,
    /// The parts.
    pub parts: &'a [PartShape],
    /// Minimum clearance (mm).
    pub clearance: f64,
}

fn hit(body: Id, other: Other, meeting: Meeting) -> Hit {
    match meeting {
        Meeting::Overlap { at } => Hit {
            body,
            other,
            overlap: true,
            distance: 0.0,
            point: at,
            other_point: at,
        },
        Meeting::Gap { distance, a, b } => Hit {
            body,
            other,
            overlap: false,
            distance: if distance < collide::TOUCH {
                0.0
            } else {
                distance
            },
            point: b,
            other_point: a,
        },
    }
}

/// Checks every body against the board and the parts.
#[must_use]
pub fn check(input: &Input<'_>) -> Vec<Hit> {
    let limit = input.clearance.max(0.0);
    let mut hits = Vec::new();
    for (body, solid) in &input.bodies {
        if let Some(board) = input.board {
            let bounds = Bounds::of_mesh(board, &Matrix::IDENTITY);
            let found = collide::meet(
                collide::triangles(board, &Matrix::IDENTITY),
                &bounds,
                solid,
                limit,
            );
            // The board may rest on a body.
            if let Some(found) = found.filter(|m| match m {
                Meeting::Overlap { .. } => true,
                Meeting::Gap { distance, .. } => {
                    *distance >= collide::TOUCH && *distance < limit - 1e-6
                }
            }) {
                hits.push(hit(*body, Other::Board, found));
            }
        }
        let found: Vec<Hit> = input
            .parts
            .par_iter()
            .filter_map(|part| {
                let mut worst: Option<Meeting> = None;
                for shape in &part.shapes {
                    let Some(m) = shape.meet(solid, limit) else {
                        continue;
                    };
                    worst = Some(match (worst, m) {
                        (Some(w @ Meeting::Overlap { .. }), _) => w,
                        (_, Meeting::Overlap { .. }) | (None, _) => m,
                        (
                            Some(w @ Meeting::Gap { distance: dw, .. }),
                            Meeting::Gap { distance: dm, .. },
                        ) => {
                            if dm < dw {
                                m
                            } else {
                                w
                            }
                        }
                    });
                    if matches!(worst, Some(Meeting::Overlap { .. })) {
                        break;
                    }
                }
                worst
                    .filter(|m| match m {
                        Meeting::Overlap { .. } => true,
                        Meeting::Gap { distance, .. } => *distance < limit - 1e-6,
                    })
                    .map(|m| hit(*body, Other::Part { id: part.id }, m))
            })
            .collect();
        hits.extend(found);
    }
    hits
}
