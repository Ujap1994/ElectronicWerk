//! The import report: what was imported and what could not be mapped.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use serde::Serialize;

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    /// Something was converted in a slightly different way.
    Info,
    /// Something was approximated or left out.
    Warning,
    /// A file or object could not be read.
    Error,
}

/// One finding, counted when it repeats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Severity.
    pub severity: Severity,
    /// Stable code, for example `unsupported-item`.
    pub code: String,
    /// English description.
    pub message: String,
    /// The KiCad file it concerns (relative to the project folder).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// How often it occurred.
    pub count: usize,
}

/// The report of one import.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    /// What was imported (the KiCad project or library path).
    pub source: String,
    /// KiCad file format versions that were read, per file kind.
    pub versions: BTreeMap<String, String>,
    /// Imported objects by kind (`symbols`, `footprints`, `tracks`, …).
    pub counts: BTreeMap<String, usize>,
    /// Findings, most serious first after [`ImportReport::finish`].
    pub findings: Vec<Finding>,
}

impl ImportReport {
    /// An empty report for `source`.
    #[must_use]
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            ..Self::default()
        }
    }

    /// Adds a finding (or counts a repetition of the same one).
    pub fn add(
        &mut self,
        severity: Severity,
        code: &str,
        file: Option<&str>,
        message: impl Into<String>,
    ) {
        let message = message.into();
        if let Some(existing) = self.findings.iter_mut().find(|finding| {
            finding.severity == severity
                && finding.code == code
                && finding.file.as_deref() == file
                && finding.message == message
        }) {
            existing.count += 1;
            return;
        }
        self.findings.push(Finding {
            severity,
            code: code.to_owned(),
            message,
            file: file.map(str::to_owned),
            count: 1,
        });
    }

    /// Adds a warning.
    pub fn warn(&mut self, code: &str, file: Option<&str>, message: impl Into<String>) {
        self.add(Severity::Warning, code, file, message);
    }

    /// Adds an informational note.
    pub fn info(&mut self, code: &str, file: Option<&str>, message: impl Into<String>) {
        self.add(Severity::Info, code, file, message);
    }

    /// Adds an error.
    pub fn error(&mut self, code: &str, file: Option<&str>, message: impl Into<String>) {
        self.add(Severity::Error, code, file, message);
    }

    /// Counts imported objects.
    pub fn count(&mut self, kind: &str, n: usize) {
        if n > 0 {
            *self.counts.entry(kind.to_owned()).or_default() += n;
        }
    }

    /// Number of findings of a severity (repetitions included).
    #[must_use]
    pub fn total(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == severity)
            .map(|finding| finding.count)
            .sum()
    }

    /// Sorts the findings: errors first, then warnings, then notes.
    pub fn finish(&mut self) {
        self.findings.sort_by(|a, b| {
            b.severity
                .cmp(&a.severity)
                .then_with(|| a.code.cmp(&b.code))
        });
    }

    /// The report as Markdown (saved next to the imported project).
    #[must_use]
    pub fn to_markdown(&self, title: &str) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "# {title}\n");
        let _ = writeln!(out, "Source: `{}`\n", self.source);
        if !self.versions.is_empty() {
            out.push_str("## KiCad file versions\n\n| File | Version |\n| --- | --- |\n");
            for (kind, version) in &self.versions {
                let _ = writeln!(out, "| {kind} | {version} |");
            }
            out.push('\n');
        }
        out.push_str("## Imported\n\n| Kind | Count |\n| --- | ---: |\n");
        for (kind, count) in &self.counts {
            let _ = writeln!(out, "| {kind} | {count} |");
        }
        out.push('\n');
        let _ = writeln!(
            out,
            "## Findings\n\n{} errors, {} warnings, {} notes.\n",
            self.total(Severity::Error),
            self.total(Severity::Warning),
            self.total(Severity::Info)
        );
        if !self.findings.is_empty() {
            out.push_str(
                "| Severity | Code | File | Message | Count |\n| --- | --- | --- | --- | ---: |\n",
            );
            for finding in &self.findings {
                let severity = match finding.severity {
                    Severity::Error => "error",
                    Severity::Warning => "warning",
                    Severity::Info => "note",
                };
                let _ = writeln!(
                    out,
                    "| {severity} | `{}` | {} | {} | {} |",
                    finding.code,
                    finding.file.as_deref().unwrap_or(""),
                    finding.message.replace('|', "\\|"),
                    finding.count
                );
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_repetitions_and_sorts_by_severity() {
        let mut report = ImportReport::new("demo.kicad_pro");
        report.info("note", None, "a note");
        report.warn("unsupported-item", Some("a.kicad_pcb"), "dimension");
        report.warn("unsupported-item", Some("a.kicad_pcb"), "dimension");
        report.error("unreadable-file", Some("b.kicad_sch"), "broken");
        report.count("tracks", 3);
        report.count("tracks", 2);
        report.count("vias", 0);
        report.finish();
        assert_eq!(report.findings[0].severity, Severity::Error);
        assert_eq!(report.findings[1].count, 2);
        assert_eq!(report.total(Severity::Warning), 2);
        assert_eq!(report.counts.get("tracks"), Some(&5));
        assert!(!report.counts.contains_key("vias"));
        let markdown = report.to_markdown("KiCad import");
        assert!(markdown.contains("| tracks | 5 |"));
        assert!(markdown.contains("1 errors, 2 warnings, 1 notes."));
    }
}
