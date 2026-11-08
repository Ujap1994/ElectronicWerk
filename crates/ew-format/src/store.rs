//! Project folders: finding, loading, saving and creating projects.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::project::{DocumentRef, Project};

use crate::codec::{self, Decoded};
use crate::error::FormatError;
use crate::fsio::{self, StagedWrites};

/// File extension of the project manifest.
pub const MANIFEST_EXTENSION: &str = "ewproj";
/// Folder for local state (autosave journal, caches). Never committed.
pub const STATE_DIR: &str = ".ewerk";
/// Content of the `.gitignore` written into new projects.
pub const GITIGNORE: &str =
    "# ElectronicWerk: local state and generated files\n.ewerk/\noutputs/generated/\n";

/// Locates the manifest: `path` may be the `.ewproj` file or its folder.
///
/// # Errors
///
/// Fails if the path does not exist, or a folder has no or several manifests.
pub fn find_manifest(path: &Path) -> Result<PathBuf, FormatError> {
    let absolute = std::path::absolute(path).map_err(|error| FormatError::io(path, error))?;
    let path = absolute.as_path();
    let metadata = fs::metadata(path).map_err(|error| FormatError::io(path, error))?;
    if metadata.is_file() {
        return if has_manifest_extension(path) {
            Ok(path.to_path_buf())
        } else {
            Err(FormatError::NoManifest(path.to_path_buf()))
        };
    }
    let mut found = Vec::new();
    for entry in fs::read_dir(path).map_err(|error| FormatError::io(path, error))? {
        let entry = entry.map_err(|error| FormatError::io(path, error))?;
        let candidate = entry.path();
        if candidate.is_file() && has_manifest_extension(&candidate) {
            found.push(candidate);
        }
    }
    match found.len() {
        0 => Err(FormatError::NoManifest(path.to_path_buf())),
        1 => Ok(found.remove(0)),
        _ => Err(FormatError::AmbiguousManifest(path.to_path_buf())),
    }
}

fn has_manifest_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case(MANIFEST_EXTENSION))
}

/// Checks a document path from the manifest: relative, `/`-separated, inside
/// the project, without `..`, with the right extension.
///
/// # Errors
///
/// Returns a description of the problem.
pub fn check_document_path(path: &str, kind: DocumentKind) -> Result<(), String> {
    if path.is_empty() || path.len() > 255 {
        return Err("the path is empty or too long".to_owned());
    }
    if path.starts_with('/') || path.contains('\\') || path.contains(':') {
        return Err(format!(
            "`{path}` must be a relative path with `/` separators"
        ));
    }
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(format!("`{path}` contains an empty, `.` or `..` segment"));
        }
        if segment
            .chars()
            .any(|c| c.is_control() || matches!(c, '<' | '>' | '"' | '|' | '?' | '*'))
        {
            return Err(format!(
                "`{path}` contains characters that are not allowed in file names"
            ));
        }
    }
    if path.split('/').next() == Some(STATE_DIR) {
        return Err(format!("`{path}` points into the local state folder"));
    }
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case(kind.extension()) {
        return Err(format!(
            "`{path}` should have the extension .{} for a {kind} document",
            kind.extension()
        ));
    }
    Ok(())
}

/// Turns a user-provided name into a safe file or folder name.
#[must_use]
pub fn sanitize_file_name(name: &str) -> String {
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .take(100)
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if trimmed.is_empty() {
        return "project".to_owned();
    }
    let stem = trimmed.split('.').next().unwrap_or(trimmed);
    if RESERVED
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(stem))
    {
        return format!("_{trimmed}");
    }
    trimmed.to_owned()
}

/// File name of the manifest for a project name, for example `MyBoard.ewproj`.
#[must_use]
pub fn manifest_file_name(project_name: &str) -> String {
    format!("{}.{MANIFEST_EXTENSION}", sanitize_file_name(project_name))
}

/// A free path for a new document, for example `schematics/power-supply.ewsch`.
#[must_use]
pub fn unique_document_path(project: &Project, kind: DocumentKind, name: &str) -> String {
    let slug: String = sanitize_file_name(name)
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_whitespace() || c == '.' {
                '-'
            } else {
                c
            }
        })
        .collect();
    let slug = if slug.is_empty() {
        kind.name().to_lowercase()
    } else {
        slug
    };
    let taken: BTreeSet<String> = project
        .manifest
        .documents
        .iter()
        .map(|entry| entry.path.to_lowercase())
        .collect();
    let mut counter = 1;
    loop {
        let candidate = if counter == 1 {
            format!("{}/{slug}.{}", kind.folder(), kind.extension())
        } else {
            format!("{}/{slug}-{counter}.{}", kind.folder(), kind.extension())
        };
        if !taken.contains(&candidate.to_lowercase()) {
            return candidate;
        }
        counter += 1;
    }
}

/// Joins a manifest-relative path onto the project folder.
#[must_use]
pub fn resolve(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

/// A file upgraded from an older schema version while loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigratedFile {
    /// Project-relative path.
    pub path: String,
    /// Original schema version.
    pub from: u32,
    /// Original bytes (kept for the backup written on the next save).
    pub original: Vec<u8>,
}

/// A project read from disk.
#[derive(Debug, Clone)]
pub struct LoadedProject {
    /// The project.
    pub project: Project,
    /// Project folder.
    pub root: PathBuf,
    /// Manifest file name inside the folder.
    pub manifest_file: String,
    /// FNV-1a hash of every file as read (key: project-relative path).
    pub hashes: BTreeMap<String, u64>,
    /// Files upgraded from an older schema version.
    pub migrated: Vec<MigratedFile>,
}

fn read_text(path: &Path) -> Result<(String, u64), FormatError> {
    let bytes = fs::read(path).map_err(|error| FormatError::io(path, error))?;
    let hash = fsio::fnv1a(&bytes);
    let text = String::from_utf8(bytes).map_err(|_| FormatError::Parse {
        path: path.to_path_buf(),
        message: "the file is not valid UTF-8".to_owned(),
    })?;
    Ok((text, hash))
}

/// Checks the manifest's document list (paths, unique ids and paths).
///
/// # Errors
///
/// Fails on the first problem.
pub fn check_manifest(manifest_path: &Path, references: &[DocumentRef]) -> Result<(), FormatError> {
    let invalid = |message: String| FormatError::Invalid {
        path: manifest_path.to_path_buf(),
        message,
    };
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for entry in references {
        check_document_path(&entry.path, entry.kind).map_err(invalid)?;
        if !ids.insert(entry.id) {
            return Err(invalid(format!("document id {} is listed twice", entry.id)));
        }
        if !paths.insert(entry.path.to_lowercase()) {
            return Err(invalid(format!("`{}` is listed twice", entry.path)));
        }
    }
    Ok(())
}

/// Loads a project (`path` is the folder or the `.ewproj` file). Documents
/// are parsed in parallel.
///
/// # Errors
///
/// Fails if any file is missing, unreadable or invalid.
pub fn load_project(path: &Path) -> Result<LoadedProject, FormatError> {
    let manifest_path = find_manifest(path)?;
    let root = manifest_path
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let manifest_file = manifest_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let (text, hash) = read_text(&manifest_path)?;
    let Decoded {
        value: manifest,
        migrated_from,
        ..
    } = codec::decode_manifest(&manifest_path, &text)?;
    check_manifest(&manifest_path, &manifest.documents)?;

    let mut hashes = BTreeMap::new();
    hashes.insert(manifest_file.clone(), hash);
    let mut migrated = Vec::new();
    if let Some(from) = migrated_from {
        migrated.push(MigratedFile {
            path: manifest_file.clone(),
            from,
            original: text.into_bytes(),
        });
    }

    let results: Vec<DocumentResult> = std::thread::scope(|scope| {
        let handles: Vec<_> = manifest
            .documents
            .iter()
            .map(|entry| scope.spawn(|| load_document(&root, entry)))
            .collect();
        handles
            .into_iter()
            .map(|handle| {
                handle.join().unwrap_or_else(|_| {
                    Err(FormatError::Invalid {
                        path: root.clone(),
                        message: "a document could not be read".to_owned(),
                    })
                })
            })
            .collect()
    });

    let mut documents = BTreeMap::new();
    for (entry, result) in manifest.documents.iter().zip(results) {
        let (document, hash, migration) = result?;
        hashes.insert(entry.path.clone(), hash);
        migrated.extend(migration);
        documents.insert(entry.id, document);
    }

    Ok(LoadedProject {
        project: Project {
            manifest,
            documents,
        },
        root,
        manifest_file,
        hashes,
        migrated,
    })
}

type DocumentResult = Result<(Document, u64, Option<MigratedFile>), FormatError>;

fn load_document(root: &Path, entry: &DocumentRef) -> DocumentResult {
    let path = resolve(root, &entry.path);
    let (text, hash) = read_text(&path)?;
    let decoded = codec::decode_document(&path, entry.kind, &text)?;
    if decoded.value.id() != entry.id {
        return Err(FormatError::Invalid {
            path,
            message: format!(
                "the document id {} does not match the id {} in the project file",
                decoded.value.id(),
                entry.id
            ),
        });
    }
    let migration = decoded.migrated_from.map(|from| MigratedFile {
        path: entry.path.clone(),
        from,
        original: text.into_bytes(),
    });
    Ok((decoded.value, hash, migration))
}

/// What to write in [`save_project`].
#[derive(Debug, Clone, Default)]
pub struct SavePlan {
    /// Write the manifest.
    pub manifest: bool,
    /// Documents to write.
    pub documents: Vec<Id>,
    /// Project-relative paths of files to delete (removed documents).
    pub delete: Vec<String>,
}

impl SavePlan {
    /// Writes everything.
    #[must_use]
    pub fn everything(project: &Project) -> Self {
        Self {
            manifest: true,
            documents: project
                .manifest
                .documents
                .iter()
                .map(|entry| entry.id)
                .collect(),
            delete: Vec::new(),
        }
    }
}

/// Result of a save.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveReport {
    /// Written files (project-relative).
    pub written: Vec<String>,
    /// Deleted files (project-relative).
    pub deleted: Vec<String>,
    /// Hashes of the written files.
    pub hashes: BTreeMap<String, u64>,
}

type StageResult = Result<(fsio::StagedFile, u64), FormatError>;

/// Saves a project atomically: every file is encoded and staged first (in
/// parallel) and only then renamed into place.
///
/// # Errors
///
/// Fails if a file cannot be written. Files that were already renamed stay.
pub fn save_project(
    root: &Path,
    manifest_file: &str,
    project: &Project,
    plan: &SavePlan,
) -> Result<SaveReport, FormatError> {
    let manifest_path = root.join(manifest_file);
    check_manifest(&manifest_path, &project.manifest.documents)?;

    // Every job encodes one file and writes it to a temporary file.
    let mut jobs: Vec<(String, Option<Id>)> = Vec::new();
    if plan.manifest {
        jobs.push((manifest_file.to_owned(), None));
    }
    for id in &plan.documents {
        if let Some(entry) = project.reference(*id) {
            jobs.push((entry.path.clone(), Some(*id)));
        }
    }
    let run = |relative: &str, id: Option<Id>| -> StageResult {
        let path = resolve(root, relative);
        let text = match id {
            None => codec::encode_manifest(&project.manifest),
            Some(id) => match project.document(id) {
                Some(document) => codec::encode_document(document),
                None => {
                    return Err(FormatError::Invalid {
                        path,
                        message: "the document is missing".to_owned(),
                    });
                }
            },
        }
        .map_err(|error| FormatError::Invalid {
            path: path.clone(),
            message: error.to_string(),
        })?;
        let staged = fsio::stage_file(&path, text.as_bytes())?;
        Ok((staged, fsio::fnv1a(text.as_bytes())))
    };
    let threads = std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .clamp(1, 8)
        .min(jobs.len().max(1));
    let results: Vec<Vec<(usize, StageResult)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|worker| {
                let jobs = &jobs;
                let run = &run;
                scope.spawn(move || {
                    jobs.iter()
                        .enumerate()
                        .skip(worker)
                        .step_by(threads)
                        .map(|(index, (relative, id))| (index, run(relative, *id)))
                        .collect()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or_default())
            .collect()
    });
    let mut ordered: Vec<(usize, StageResult)> = results.into_iter().flatten().collect();
    ordered.sort_by_key(|(index, _)| *index);

    let mut writes = StagedWrites::new();
    let mut report = SaveReport::default();
    let mut first_error = None;
    for (index, result) in ordered {
        match result {
            Ok((staged, hash)) => {
                writes.adopt(staged);
                let relative = jobs[index].0.clone();
                report.hashes.insert(relative.clone(), hash);
                report.written.push(relative);
            }
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    if let Some(error) = first_error {
        // Dropping `writes` removes every temporary file.
        return Err(error);
    }
    if report.written.len() != jobs.len() {
        return Err(FormatError::Invalid {
            path: root.to_path_buf(),
            message: "a file could not be prepared for saving".to_owned(),
        });
    }
    writes.commit()?;

    for relative in &plan.delete {
        let path = resolve(root, relative);
        match fs::remove_file(&path) {
            Ok(()) => report.deleted.push(relative.clone()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(FormatError::io(path, error)),
        }
    }
    Ok(report)
}

/// Creates a new project folder (which must not exist or be empty) with
/// every document, a `.gitignore` and the output folder.
///
/// # Errors
///
/// Fails if the folder is not empty or a file cannot be written.
pub fn create_project(root: &Path, project: &Project) -> Result<LoadedProject, FormatError> {
    let absolute = std::path::absolute(root).map_err(|error| FormatError::io(root, error))?;
    let root = absolute.as_path();
    if root.exists() {
        let mut entries = fs::read_dir(root).map_err(|error| FormatError::io(root, error))?;
        if entries.next().is_some() {
            return Err(FormatError::NotEmpty(root.to_path_buf()));
        }
    }
    fs::create_dir_all(root).map_err(|error| FormatError::io(root, error))?;
    let manifest_file = manifest_file_name(&project.manifest.name);
    fsio::write_atomic(&root.join(".gitignore"), GITIGNORE.as_bytes())?;
    let report = save_project(
        root,
        &manifest_file,
        project,
        &SavePlan::everything(project),
    )?;
    Ok(LoadedProject {
        project: project.clone(),
        root: root.to_path_buf(),
        manifest_file,
        hashes: report.hashes,
        migrated: Vec::new(),
    })
}

/// Category of a file in the project folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FileCategory {
    /// The manifest.
    Manifest,
    /// A document listed in the manifest.
    Document,
    /// A generated output.
    Generated,
    /// Anything else.
    Other,
}

/// A file in the project folder.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    /// Project-relative path with `/` separators.
    pub path: String,
    /// Size in bytes.
    pub size: u64,
    /// Category.
    pub category: FileCategory,
}

/// Maximum number of files [`list_files`] returns.
pub const MAX_LISTED_FILES: usize = 2_000;

/// Lists the files of a project folder (without the local state folder and
/// temporary files), sorted by path.
///
/// # Errors
///
/// Fails if the project folder cannot be read.
pub fn list_files(
    root: &Path,
    manifest_file: &str,
    project: &Project,
    generated_dirs: &[String],
) -> Result<Vec<FileEntry>, FormatError> {
    let documents: BTreeSet<String> = project
        .manifest
        .documents
        .iter()
        .map(|entry| entry.path.to_lowercase())
        .collect();
    let mut files = Vec::new();
    let mut pending = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) if dir == root => return Err(FormatError::io(root, error)),
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let relative = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() || fsio::is_temp_file(&name) {
                continue;
            }
            if file_type.is_dir() {
                if relative != STATE_DIR && name != ".git" {
                    pending.push((entry.path(), relative));
                }
                continue;
            }
            let category = if relative == manifest_file {
                FileCategory::Manifest
            } else if documents.contains(&relative.to_lowercase()) {
                FileCategory::Document
            } else if generated_dirs
                .iter()
                .any(|dir| relative.starts_with(&format!("{dir}/")))
            {
                FileCategory::Generated
            } else {
                FileCategory::Other
            };
            let size = entry.metadata().map_or(0, |metadata| metadata.len());
            files.push(FileEntry {
                path: relative,
                size,
                category,
            });
            if files.len() >= MAX_LISTED_FILES {
                pending.clear();
                break;
            }
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

/// Copies the files that are not documents (notes, datasheets, …) to another
/// project folder. Generated outputs and local state are left out. Returns
/// the number of copied files.
///
/// # Errors
///
/// Fails if a file cannot be read or written.
pub fn copy_other_files(
    from: &Path,
    to: &Path,
    manifest_file: &str,
    project: &Project,
    generated_dirs: &[String],
) -> Result<usize, FormatError> {
    let mut copied = 0;
    for file in list_files(from, manifest_file, project, generated_dirs)? {
        if file.category != FileCategory::Other || file.path == ".gitignore" {
            continue;
        }
        let source = resolve(from, &file.path);
        let target = resolve(to, &file.path);
        let bytes = fs::read(&source).map_err(|error| FormatError::io(&source, error))?;
        fsio::write_atomic(&target, &bytes)?;
        copied += 1;
    }
    Ok(copied)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_model::document::Document;

    fn demo_project() -> Project {
        let mut project = Project::new(Id::random(), "My Board");
        for (kind, name) in [
            (DocumentKind::Schematic, "Root"),
            (DocumentKind::Pcb, "My Board"),
            (DocumentKind::Rules, "Design rules"),
        ] {
            let id = Id::random();
            let path = unique_document_path(&project, kind, name);
            project
                .add_document(
                    DocumentRef { id, kind, path },
                    usize::MAX,
                    Document::new(kind, id, name),
                )
                .unwrap();
        }
        project.manifest.root_schematic = Some(project.manifest.documents[0].id);
        project
    }

    #[test]
    fn create_load_and_save() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("My Board");
        let project = demo_project();
        let created = create_project(&root, &project).unwrap();
        assert_eq!(created.manifest_file, "My Board.ewproj");
        assert!(root.join(".gitignore").is_file());
        assert!(root.join("schematics/root.ewsch").is_file());
        assert!(root.join("pcb/my-board.ewpcb").is_file());

        let loaded = load_project(&root).unwrap();
        assert_eq!(loaded.project, project);
        assert_eq!(loaded.hashes, created.hashes);
        let by_file = load_project(&root.join("My Board.ewproj")).unwrap();
        assert_eq!(by_file.project, project);

        // Saving unchanged data writes identical bytes.
        let before = fs::read(root.join("pcb/my-board.ewpcb")).unwrap();
        save_project(
            &root,
            &loaded.manifest_file,
            &loaded.project,
            &SavePlan::everything(&project),
        )
        .unwrap();
        assert_eq!(fs::read(root.join("pcb/my-board.ewpcb")).unwrap(), before);

        assert!(matches!(
            create_project(&root, &project),
            Err(FormatError::NotEmpty(_))
        ));
    }

    #[test]
    fn deletes_removed_documents() {
        let dir = tempfile::tempdir().unwrap();
        let mut project = demo_project();
        let created = create_project(dir.path(), &project).unwrap();
        let rules = project.manifest.documents[2].id;
        let (entry, _, _) = project.remove_document(rules).unwrap();
        let plan = SavePlan {
            manifest: true,
            documents: Vec::new(),
            delete: vec![entry.path.clone(), "does/not/exist.ewsch".into()],
        };
        let report = save_project(dir.path(), &created.manifest_file, &project, &plan).unwrap();
        assert_eq!(report.deleted, vec![entry.path.clone()]);
        assert!(!resolve(dir.path(), &entry.path).exists());
        assert_eq!(load_project(dir.path()).unwrap().project, project);
    }

    #[test]
    fn roots_are_absolute() {
        let dir = tempfile::tempdir().unwrap();
        let previous = std::env::current_dir().unwrap();
        // Relative paths resolve against the working directory.
        let relative = Path::new("relative-project");
        let absolute = std::path::absolute(dir.path().join(relative)).unwrap();
        let created = create_project(&absolute, &demo_project()).unwrap();
        assert!(created.root.is_absolute());
        assert!(load_project(&absolute).unwrap().root.is_absolute());
        assert_eq!(std::env::current_dir().unwrap(), previous);
    }

    #[test]
    fn manifest_discovery() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            find_manifest(dir.path()),
            Err(FormatError::NoManifest(_))
        ));
        fs::write(dir.path().join("a.ewproj"), "{}").unwrap();
        assert!(find_manifest(dir.path()).is_ok());
        fs::write(dir.path().join("b.EWPROJ"), "{}").unwrap();
        assert!(matches!(
            find_manifest(dir.path()),
            Err(FormatError::AmbiguousManifest(_))
        ));
        assert_eq!(
            find_manifest(&dir.path().join("missing"))
                .unwrap_err()
                .code(),
            "notFound"
        );
    }

    #[test]
    fn detects_mismatched_document_ids() {
        let dir = tempfile::tempdir().unwrap();
        let project = demo_project();
        create_project(dir.path(), &project).unwrap();
        let other = Document::new(DocumentKind::Rules, Id::random(), "Other");
        let text = codec::encode_document(&other).unwrap();
        fs::write(dir.path().join("rules/design-rules.ewrules"), text).unwrap();
        let error = load_project(dir.path()).unwrap_err();
        assert!(error.to_string().contains("does not match"), "{error}");
    }

    #[test]
    fn document_paths() {
        assert!(check_document_path("schematics/a.ewsch", DocumentKind::Schematic).is_ok());
        for bad in [
            "",
            "/abs.ewsch",
            "a\\b.ewsch",
            "../x.ewsch",
            "a//b.ewsch",
            "c:/x.ewsch",
            "x.ewpcb",
            ".ewerk/x.ewsch",
            "a?.ewsch",
        ] {
            assert!(
                check_document_path(bad, DocumentKind::Schematic).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn file_names() {
        assert_eq!(sanitize_file_name("My Board"), "My Board");
        assert_eq!(sanitize_file_name("a/b:c"), "a_b_c");
        assert_eq!(sanitize_file_name("  ..  "), "project");
        assert_eq!(sanitize_file_name("con"), "_con");
        assert_eq!(sanitize_file_name("LPT1.txt"), "_LPT1.txt");
        assert_eq!(manifest_file_name("Demo"), "Demo.ewproj");
        let project = demo_project();
        assert_eq!(
            unique_document_path(&project, DocumentKind::Schematic, "Root"),
            "schematics/root-2.ewsch"
        );
        assert_eq!(
            unique_document_path(&project, DocumentKind::Schematic, "Power Supply 3.3V"),
            "schematics/power-supply-3-3v.ewsch"
        );
    }

    #[test]
    fn copies_other_files() {
        let dir = tempfile::tempdir().unwrap();
        let project = demo_project();
        let created = create_project(&dir.path().join("a"), &project).unwrap();
        fs::create_dir_all(dir.path().join("a/docs")).unwrap();
        fs::write(dir.path().join("a/docs/notes.md"), "n").unwrap();
        fs::create_dir_all(dir.path().join("a/outputs/generated")).unwrap();
        fs::write(dir.path().join("a/outputs/generated/x.gbr"), "g").unwrap();
        let copied = copy_other_files(
            &dir.path().join("a"),
            &dir.path().join("b"),
            &created.manifest_file,
            &project,
            &["outputs/generated".to_owned()],
        )
        .unwrap();
        assert_eq!(copied, 1);
        assert!(dir.path().join("b/docs/notes.md").is_file());
        assert!(!dir.path().join("b/outputs").exists());
    }

    #[test]
    fn lists_files_by_category() {
        let dir = tempfile::tempdir().unwrap();
        let project = demo_project();
        let created = create_project(dir.path(), &project).unwrap();
        fs::create_dir_all(dir.path().join("outputs/generated")).unwrap();
        fs::write(dir.path().join("outputs/generated/board.gbr"), "G04*").unwrap();
        fs::create_dir_all(dir.path().join(STATE_DIR)).unwrap();
        fs::write(dir.path().join(STATE_DIR).join("journal.jsonl"), "x").unwrap();
        fs::write(dir.path().join("notes.md"), "hi").unwrap();
        let files = list_files(
            dir.path(),
            &created.manifest_file,
            &project,
            &["outputs/generated".to_owned()],
        )
        .unwrap();
        let categories: BTreeMap<&str, FileCategory> = files
            .iter()
            .map(|file| (file.path.as_str(), file.category))
            .collect();
        assert_eq!(categories["My Board.ewproj"], FileCategory::Manifest);
        assert_eq!(categories["schematics/root.ewsch"], FileCategory::Document);
        assert_eq!(
            categories["outputs/generated/board.gbr"],
            FileCategory::Generated
        );
        assert_eq!(categories["notes.md"], FileCategory::Other);
        assert!(!categories.keys().any(|path| path.starts_with(STATE_DIR)));
    }
}
