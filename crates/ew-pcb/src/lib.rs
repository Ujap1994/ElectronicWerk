//! Board logic for the layout editor (M7, ADR-0008).
//!
//! - [`geom`] and [`poly`]: copper shapes, distances and polygon operations.
//! - [`items`]: the objects of a board as shapes, with a spatial index.
//! - [`connectivity`]: which copper touches; unrouted connections
//!   (ratsnest).
//! - [`rules`]: net classes, constraints and custom rules with a small query
//!   language.
//! - [`drc`]: the design rules check.
//! - [`edit`]: moving, turning, flipping, aligning and dragging.
//! - [`fill`]: the zone filler.
//! - [`route`]: interactive routing around obstacles.
//! - [`presets`]: board presets; [`dxf`]: DXF import.
//! - [`sync`]: schematic ↔ board updates and cross-probing.

#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "board coordinates are limited to ±10 m, well inside f64 precision"
)]

pub mod connectivity;
pub mod drc;
pub mod dxf;
pub mod edit;
pub mod fill;
pub mod geom;
pub mod items;
pub mod poly;
pub mod presets;
pub mod route;
pub mod rules;
pub mod sync;
