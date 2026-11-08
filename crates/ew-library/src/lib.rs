//! The ElectronicWerk base library (ADR-0005): a search index over symbols,
//! footprints and 3D models from any source, with a deduplication engine.
//!
//! - [`ingest`] reads sources (KiCad folders and tessellated meshes) into
//!   source-independent [`Entry`] values with fingerprints.
//! - [`dedup`] groups exact duplicates, near duplicates and aliases.
//! - [`Library`] is the SQLite index with FTS5 full-text search.
//! - [`pack`] writes and installs the library packs that the app downloads.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    reason = "coordinates are limited by the data model; ids and counts fit into i64"
)]

pub mod dedup;
pub mod entry;
mod error;
pub mod fingerprint;
pub mod index;
pub mod ingest;
pub mod mesh;
pub mod pack;

use std::path::Path;

pub use dedup::{Class, Decision, Report};
pub use entry::{Entry, Kind};
pub use error::LibraryError;
pub use ew_kicad::ImportReport;
pub use index::{Library, Query, SourceInfo};
pub use ingest::Source;

/// The result of [`build`].
pub struct Built {
    /// The new index.
    pub library: Library,
    /// The deduplication report.
    pub report: Report,
    /// Problems while reading the sources.
    pub problems: ImportReport,
}

/// Builds a library in `dir` from `sources`: reads them, finds duplicates,
/// writes the index, copies the meshes into the mesh cache and writes the
/// reports (`dedup-report.json`, `dedup-report.md`) and the license.
///
/// # Errors
///
/// Fails if the index or a report cannot be written.
pub fn build(
    dir: &Path,
    sources: &[Source],
    license: Option<&str>,
    progress: ingest::Progress<'_>,
) -> Result<Built, LibraryError> {
    let ingested = ingest::ingest(sources, progress);
    progress("index", 0, 1);
    let infos: Vec<SourceInfo> = sources.iter().map(|s| s.info.clone()).collect();
    let (library, report) =
        Library::build(dir, &infos, ingested.entries, fingerprint::TOLERANCE_UM)?;
    pack::store_meshes(dir, &ingested.meshes)?;
    for (index, source) in sources.iter().enumerate() {
        if let Some(models) = &source.models {
            library.set_meta(&format!("models:{index}"), &models.display().to_string())?;
        }
    }
    let write = |name: &str, text: &str| {
        let path = dir.join(name);
        std::fs::write(&path, text).map_err(|source| LibraryError::Io { path, source })
    };
    write(pack::REPORT_JSON, &report.to_json())?;
    write(pack::REPORT_MARKDOWN, &report.to_markdown())?;
    if let Some(license) = license {
        write(pack::LICENSE_FILE, license)?;
    }
    progress("index", 1, 1);
    Ok(Built {
        library,
        report,
        problems: ingested.report,
    })
}
