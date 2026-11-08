//! Project archives (`.zip`).

use std::fs::{self, File};
use std::io::{self, Write as _};
use std::path::Path;

use ew_model::project::Project;
use zip::write::SimpleFileOptions;

use crate::codec;
use crate::error::FormatError;
use crate::fsio;
use crate::store::{self, FileCategory};
use crate::time::DateTime;

/// What [`archive_project`] wrote.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveReport {
    /// Archive file.
    pub path: String,
    /// Number of files in the archive.
    pub files: usize,
    /// Archive size in bytes.
    pub bytes: u64,
}

/// Writes the project into a ZIP archive with one top-level folder.
///
/// The manifest and documents come from `project` (the current state in
/// memory, including unsaved changes); other files in the folder are copied
/// from disk. The local state folder and generated outputs are left out.
///
/// # Errors
///
/// Fails if a file cannot be read or the archive cannot be written.
pub fn archive_project(
    root: &Path,
    manifest_file: &str,
    project: &Project,
    generated_dirs: &[String],
    destination: &Path,
) -> Result<ArchiveReport, FormatError> {
    let folder = store::sanitize_file_name(&project.manifest.name);
    let now = DateTime::now();
    let timestamp = zip::DateTime::from_date_and_time(
        u16::try_from(now.year.clamp(1980, 2107)).unwrap_or(1980),
        now.month,
        now.day,
        now.hour,
        now.minute,
        now.second,
    )
    .unwrap_or_default();
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(timestamp)
        .unix_permissions(0o644);

    let destination_name = destination
        .canonicalize()
        .ok()
        .or_else(|| Some(destination.to_path_buf()));
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    let encode_error = |error: serde_json::Error| FormatError::Invalid {
        path: root.to_path_buf(),
        message: error.to_string(),
    };
    entries.push((
        manifest_file.to_owned(),
        codec::encode_manifest(&project.manifest)
            .map_err(encode_error)?
            .into_bytes(),
    ));
    for entry in &project.manifest.documents {
        if let Some(document) = project.document(entry.id) {
            entries.push((
                entry.path.clone(),
                codec::encode_document(document)
                    .map_err(encode_error)?
                    .into_bytes(),
            ));
        }
    }
    for file in store::list_files(root, manifest_file, project, generated_dirs)? {
        if file.category != FileCategory::Other {
            continue;
        }
        let path = store::resolve(root, &file.path);
        if path.canonicalize().ok() == destination_name {
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| FormatError::io(&path, error))?;
        entries.push((file.path, bytes));
    }
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut buffer = io::Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buffer);
        let zip_error = |error: zip::result::ZipError| FormatError::Invalid {
            path: destination.to_path_buf(),
            message: error.to_string(),
        };
        for (name, bytes) in &entries {
            writer
                .start_file(format!("{folder}/{name}"), options)
                .map_err(zip_error)?;
            writer
                .write_all(bytes)
                .map_err(|error| FormatError::io(destination, error))?;
        }
        writer.finish().map_err(zip_error)?;
    }
    let bytes = buffer.into_inner();
    fsio::write_atomic(destination, &bytes)?;
    Ok(ArchiveReport {
        path: destination.to_string_lossy().into_owned(),
        files: entries.len(),
        bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    })
}

/// Lists the entry names of an archive (used by tests and the CLI).
///
/// # Errors
///
/// Fails if the file is not a readable ZIP archive.
pub fn list_archive(path: &Path) -> Result<Vec<String>, FormatError> {
    let file = File::open(path).map_err(|error| FormatError::io(path, error))?;
    let archive = zip::ZipArchive::new(file).map_err(|error| FormatError::Invalid {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut names = archive
        .file_names()
        .map(|name| name.map(std::borrow::Cow::into_owned))
        .collect::<Result<Vec<String>, _>>()
        .map_err(|error| FormatError::Invalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    names.sort();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_model::document::{Document, DocumentKind};
    use ew_model::id::Id;
    use ew_model::project::DocumentRef;
    use std::io::Read as _;

    #[test]
    fn archives_current_state_without_local_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut project = Project::new(Id::random(), "Arch");
        let id = Id::random();
        project
            .add_document(
                DocumentRef {
                    id,
                    kind: DocumentKind::Schematic,
                    path: "schematics/root.ewsch".into(),
                },
                0,
                Document::new(DocumentKind::Schematic, id, "Root"),
            )
            .unwrap();
        let created = store::create_project(dir.path(), &project).unwrap();
        fs::create_dir_all(dir.path().join(".ewerk")).unwrap();
        fs::write(dir.path().join(".ewerk/journal.jsonl"), "x").unwrap();
        fs::create_dir_all(dir.path().join("outputs/generated")).unwrap();
        fs::write(dir.path().join("outputs/generated/x.gbr"), "x").unwrap();
        fs::write(dir.path().join("README.md"), "# Arch").unwrap();

        // An unsaved change must end up in the archive.
        project.manifest.revision = "B".into();
        let destination = dir.path().join("Arch.zip");
        let report = archive_project(
            dir.path(),
            &created.manifest_file,
            &project,
            &["outputs/generated".to_owned()],
            &destination,
        )
        .unwrap();
        assert_eq!(report.files, 4);
        let names = list_archive(&destination).unwrap();
        assert_eq!(
            names,
            vec![
                "Arch/.gitignore",
                "Arch/Arch.ewproj",
                "Arch/README.md",
                "Arch/schematics/root.ewsch"
            ]
        );
        let mut archive = zip::ZipArchive::new(File::open(&destination).unwrap()).unwrap();
        let mut manifest = String::new();
        archive
            .by_name("Arch/Arch.ewproj")
            .unwrap()
            .read_to_string(&mut manifest)
            .unwrap();
        assert!(manifest.contains("\"revision\": \"B\""));

        // Archiving again into the project folder skips the archive itself.
        let again = archive_project(
            dir.path(),
            &created.manifest_file,
            &project,
            &[],
            &destination,
        )
        .unwrap();
        assert_eq!(again.files, 5);
        let is_zip = |name: &String| {
            Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
        };
        assert!(!list_archive(&destination).unwrap().iter().any(is_zip));
    }
}
