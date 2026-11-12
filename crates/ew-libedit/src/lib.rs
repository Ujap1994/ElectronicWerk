//! Library editing of ElectronicWerk (M6): operations on symbols and
//! footprints, footprint wizards (IPC-7351 style) and library checks. Every
//! function works on a copy of a definition and returns the new one; the
//! engine stores it as one undo step.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "dimensions are limited to ±10 m by the data model"
)]

pub mod check;
pub mod wizard;
