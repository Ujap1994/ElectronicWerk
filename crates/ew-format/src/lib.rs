//! ElectronicWerk native file format.
//!
//! - [`json`]: the deterministic, diff-friendly JSON layout.
//! - [`codec`]: file headers, schema versions and migrations.
//! - [`fsio`]: crash-safe (atomic) writes.
//! - [`store`]: loading, saving and creating project folders.
//! - [`validate`]: structural and referential checks.
//! - [`archive`]: ZIP archives of a project.

pub mod archive;
pub mod codec;
pub mod error;
pub mod fsio;
pub mod json;
pub mod store;
pub mod time;
pub mod validate;

pub use codec::SCHEMA_VERSION;
pub use error::FormatError;
pub use store::{LoadedProject, SavePlan, SaveReport};
