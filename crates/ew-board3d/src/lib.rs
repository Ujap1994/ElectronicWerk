//! The 3D model of a board (M9, ADR-0010).
//!
//! - [`stack`]: the heights of the body, copper, mask and silkscreen from the
//!   stack-up, and the colours of mask, silkscreen and surface finish.
//! - [`mesh`]: triangle meshes and their construction from polygons.
//! - [`board`]: the board body (outline minus holes, extruded), the copper,
//!   mask, silkscreen and paste of each side and the plated barrels.
//! - [`place`]: where the 3D models of the footprints go (KiCad's
//!   convention), as 4×4 matrices.
//! - [`scene`]: the board as one binary buffer plus metadata for the 3D view.
//! - [`glb`]: reads the GLB files of the mesh cache and writes GLB files.
//! - [`step`]: STEP AP214 export of the assembly: the board body as an exact
//!   B-rep, component STEP files merged in, mesh-only models as faceted
//!   solids.
//! - [`vrml`]: VRML 2.0 export.
//!
//! Units: the 3D model uses millimetres with z up; the board's bottom face
//! lies at z = 0 and its top face at the height of the dielectric stack, so
//! the outer copper sits on the faces (KiCad's STEP convention).

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "board coordinates are limited to ±10 m, well inside f64 precision; geometry code uses short names"
)]

pub mod board;
pub mod mesh;
pub mod place;
pub mod scene;
pub mod stack;

/// Nanometres per millimetre.
pub const NM_PER_MM: f64 = 1e6;

/// Millimetres of a length in nanometres.
#[must_use]
pub fn mm(nm: f64) -> f64 {
    nm / NM_PER_MM
}
