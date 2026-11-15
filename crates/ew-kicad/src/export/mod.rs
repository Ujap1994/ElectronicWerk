//! KiCad export (ADR-0015): an ElectronicWerk project as a KiCad project —
//! `.kicad_pro`, schematics, boards and the project libraries — so a design
//! never stays locked in. Files are written in the KiCad 8 format, which
//! KiCad 8 and every later version open.

pub mod board;
pub mod project;
pub mod schematic;

pub use project::{ExportFile, KicadProject, export_project, file_stem};

use crate::write::Sx;

/// Something the export could not carry over.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ExportNote {
    /// Short code (`layer`, `bus`, …).
    pub code: String,
    /// What was left out or changed (English).
    pub message: String,
}

/// What the export could not carry over (each message once).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ExportReport {
    /// The notes, in the order they came up.
    pub notes: Vec<ExportNote>,
}

impl ExportReport {
    /// Adds a note unless the same one is there already.
    pub fn note(&mut self, code: &str, message: impl Into<String>) {
        let message = message.into();
        if !self
            .notes
            .iter()
            .any(|n| n.code == code && n.message == message)
        {
            self.notes.push(ExportNote {
                code: code.to_owned(),
                message,
            });
        }
    }
}

/// The title block and names shared by the files of a project.
#[derive(Debug, Clone)]
pub struct Title<'a> {
    /// Project name.
    pub project: &'a str,
    /// Revision.
    pub revision: &'a str,
    /// Date (ISO 8601 or free text).
    pub date: &'a str,
    /// Company.
    pub company: &'a str,
    /// Nickname of the project libraries (`Library:Name` references).
    pub library: &'a str,
    /// The program version that writes the files.
    pub version: &'a str,
}

impl Title<'_> {
    /// `(title_block …)`.
    #[must_use]
    pub fn block(&self) -> Sx {
        let mut node = Sx::list("title_block").child(Sx::named("title", self.project));
        if !self.date.is_empty() {
            node = node.child(Sx::named("date", self.date));
        }
        if !self.revision.is_empty() {
            node = node.child(Sx::named("rev", self.revision));
        }
        if !self.company.is_empty() {
            node = node.child(Sx::named("company", self.company));
        }
        node
    }
}
