//! Schematic logic of ElectronicWerk (M5): connectivity across sheets and
//! the hierarchy, the electrical rules check, editing operations,
//! annotation and exports. Everything works on the data model and returns
//! data or change requests; the engine applies them.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "coordinates are limited to ±10 m by the data model"
)]

pub mod annotate;
pub mod connectivity;
pub mod edit;
pub mod erc;
pub mod export;
pub mod find;
pub mod geometry;
pub mod starter;

pub use connectivity::{Connectivity, analyze};
pub use erc::{ErcReport, Issue, check};
