//! KiCad import (ADR-0004): projects, schematics, boards, symbol and
//! footprint libraries and netlists become ElectronicWerk data, with a
//! report of everything that could not be mapped.
//!
//! KiCad 6 and later files (S-expression format) are supported; KiCad 10 is
//! the reference.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "KiCad stores millimetres as decimals; values are limited by the file format"
)]

pub mod common;
mod error;
pub mod footprint;
mod import;
pub mod library;
pub mod netlist;
pub mod pcb;
pub mod project;
pub mod report;
pub mod schematic;
pub mod sexpr;
pub mod symbol;

pub use error::KicadError;
pub use import::{Imported, import_project, locate};
pub use report::{Finding, ImportReport, Severity};
