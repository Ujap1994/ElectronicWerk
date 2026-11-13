//! Manufacturing outputs (M8, ADR-0009).
//!
//! - [`font`]: the stroke font of plotted texts.
//! - [`art`]: board layers as plotted features (flashes, lines, arcs,
//!   regions).
//! - [`gerber`]: Gerber X2 files and the Gerber job file.
//! - [`drill`]: Excellon drill files; [`drillmap`]: the drill map drawing.
//! - [`bom`]: bills of materials (CSV, XLSX via [`xlsx`], HTML).
//! - [`placement`]: pick-and-place files.
//! - [`ipc356`]: IPC-D-356A netlists for the electrical test; [`ipc2581`]:
//!   IPC-2581 board data.
//! - [`canvas`]: vector pages, written by [`pdf`] and [`svg`]; [`print`]:
//!   schematic prints, board prints and assembly drawings.
//! - [`archive`]: ZIP packages.
//! - [`job`]: output jobs and presets for board makers.
//! - [`viewer`]: Gerber and drill files as a scene for the viewer.
//! - [`fracture`]: polygons with holes as one contour.
//! - [`read`]: reads Gerber and Excellon files; [`raster`] draws them into
//!   bitmaps for comparisons.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::many_single_char_names,
    clippy::similar_names,
    reason = "board coordinates are limited to ±10 m, well inside f64 precision; geometry code uses short names"
)]

pub mod archive;
pub mod art;
pub mod bom;
pub mod canvas;
pub mod drill;
pub mod drillmap;
pub mod font;
pub mod fracture;
pub mod gerber;
pub mod ipc2581;
pub mod ipc356;
pub mod job;
pub mod pdf;
pub mod placement;
pub mod print;
pub mod raster;
pub mod read;
pub mod svg;
pub mod viewer;
pub mod xlsx;

/// A generated file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFile {
    /// File name (relative to the output folder; may contain `/`).
    pub name: String,
    /// Content.
    pub data: Vec<u8>,
}

impl OutputFile {
    /// A text file.
    #[must_use]
    pub fn text(name: impl Into<String>, content: String) -> Self {
        Self {
            name: name.into(),
            data: content.into_bytes(),
        }
    }

    /// A binary file.
    #[must_use]
    pub fn binary(name: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }
}
