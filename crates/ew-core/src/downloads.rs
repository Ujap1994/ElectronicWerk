//! Parts downloaded from online part libraries (`SnapEDA`, Ultra Librarian):
//! a ZIP file in KiCad format, or a single KiCad library file, previewed and
//! imported into a project library (M10).
//!
//! The download is unpacked into a temporary folder, with limits against
//! malformed archives (no paths outside the folder, at most 2,000 entries
//! and 512 MB). KiCad 6+ symbol libraries (`.kicad_sym`) and footprints
//! (`.kicad_mod`) are read wherever they are in the archive; 3D models
//! (STEP, VRML) are copied into the project's `3d/<library>` folder and the
//! footprints point at them with `${KIPRJMOD}`. Every symbol and footprint
//! is checked against the base library: a footprint the base library already
//! has can be replaced by the library's.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use ew_model::id::Id;
use ew_model::library::{FootprintDef, SymbolDef};
use ew_model::pcb::{Model3d, Scale3};
use ew_model::units::Vec3;
use serde::{Deserialize, Serialize};

use crate::error::{EngineError, ErrorCode};
use crate::library::{Duplicate, LibraryState};

/// Most entries an archive may have.
const MAX_ENTRIES: usize = 2_000;
/// Most bytes an archive may unpack to.
const MAX_BYTES: u64 = 512 * 1024 * 1024;

/// An unpacked download in a temporary folder (removed when dropped).
#[derive(Debug)]
pub struct Unpacked {
    dir: PathBuf,
    /// Unpacked files, relative to the folder.
    pub files: Vec<PathBuf>,
    /// The download's file name.
    pub name: String,
}

impl Drop for Unpacked {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Unpacked {
    /// Absolute path of an unpacked file.
    #[must_use]
    pub fn path(&self, file: &Path) -> PathBuf {
        self.dir.join(file)
    }
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

fn format_error(message: impl Into<String>) -> EngineError {
    EngineError::new(ErrorCode::FormatError, message)
}

/// Unpacks a download (a ZIP file or a single KiCad library or 3D file).
///
/// # Errors
///
/// Fails if the file cannot be read, is not a ZIP file or a KiCad file, or
/// breaks the limits.
pub fn unpack(path: &Path) -> Result<Unpacked, EngineError> {
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let dir = std::env::temp_dir().join(format!("ew-download-{}", Id::random()));
    std::fs::create_dir_all(&dir)
        .map_err(|e| EngineError::io("creating a temporary folder", &e))?;
    let mut unpacked = Unpacked {
        dir,
        files: Vec::new(),
        name,
    };
    match extension(path).as_str() {
        "zip" => {
            let file =
                File::open(path).map_err(|e| EngineError::io(&path.display().to_string(), &e))?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| format_error(format!("not a ZIP file: {e}")))?;
            if archive.len() > MAX_ENTRIES {
                return Err(format_error(format!(
                    "the archive has {} entries (at most {MAX_ENTRIES})",
                    archive.len()
                )));
            }
            let mut budget = MAX_BYTES;
            for index in 0..archive.len() {
                let mut entry = archive
                    .by_index(index)
                    .map_err(|e| format_error(format!("a broken ZIP entry: {e}")))?;
                if entry.is_dir() {
                    continue;
                }
                // Never outside the folder (`..`, absolute paths).
                let Some(relative) = entry.enclosed_name() else {
                    continue;
                };
                let target = unpacked.dir.join(&relative);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| EngineError::io("unpacking the download", &e))?;
                }
                let mut out = File::create(&target)
                    .map_err(|e| EngineError::io("unpacking the download", &e))?;
                let mut limited = (&mut entry).take(budget + 1);
                let written = std::io::copy(&mut limited, &mut out)
                    .map_err(|e| format_error(format!("a broken ZIP entry: {e}")))?;
                if written > budget {
                    return Err(format_error("the archive unpacks to more than 512 MB"));
                }
                budget -= written;
                out.flush()
                    .map_err(|e| EngineError::io("unpacking the download", &e))?;
                unpacked.files.push(relative);
            }
        }
        "kicad_sym" | "kicad_mod" | "step" | "stp" | "wrl" | "wrz" => {
            let relative = PathBuf::from(&unpacked.name);
            std::fs::copy(path, unpacked.dir.join(&relative))
                .map_err(|e| EngineError::io(&path.display().to_string(), &e))?;
            unpacked.files.push(relative);
        }
        _ => {
            return Err(EngineError::invalid_params(
                "a download must be a ZIP file or a KiCad library or 3D model file",
            ));
        }
    }
    unpacked.files.sort();
    Ok(unpacked)
}

/// The online library a download probably comes from.
#[must_use]
pub fn provider_of(unpacked: &Unpacked) -> Option<&'static str> {
    let lower = unpacked.name.to_ascii_lowercase();
    let paths: Vec<String> = unpacked
        .files
        .iter()
        .map(|f| f.to_string_lossy().to_ascii_lowercase())
        .collect();
    if lower.starts_with("ul_")
        || paths
            .iter()
            .any(|p| p.contains("kicadv6/") || p.contains("ultra librarian"))
    {
        Some("UltraLibrarian")
    } else if lower.contains("snapeda")
        || paths.iter().any(|p| {
            p.ends_with("how-to-import.htm") || p.contains("snapeda") || p.contains("snapmagic")
        })
    {
        Some("SnapEDA")
    } else {
        None
    }
}

/// A library nickname from free text: letters, digits, `_`, `-` and `.`.
fn nickname_of(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "Download".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// What a download contains, read and resolved.
#[derive(Debug)]
pub struct Parts {
    /// Symbols (names with the nickname).
    pub symbols: Vec<SymbolDef>,
    /// Footprints (names with the nickname; models already point into the
    /// project).
    pub footprints: Vec<FootprintDef>,
    /// 3D model files in the download, by file name: (relative path, bytes,
    /// used by a footprint).
    pub models: BTreeMap<String, (PathBuf, u64, bool)>,
    /// Problems for the report.
    pub problems: Vec<String>,
    /// The nickname used.
    pub nickname: String,
    /// The online library, if known.
    pub provider: Option<&'static str>,
}

fn is_model(path: &Path) -> bool {
    matches!(extension(path).as_str(), "step" | "stp" | "wrl" | "wrz")
}

/// The folder (relative to the project) the models of a nickname go to.
#[must_use]
pub fn model_folder(nickname: &str) -> String {
    format!("3d/{nickname}")
}

/// Reads the KiCad files of a download.
#[must_use]
pub fn read(unpacked: &Unpacked, nickname: Option<&str>) -> Parts {
    let provider = provider_of(unpacked);
    let stem = unpacked
        .name
        .rsplit_once('.')
        .map_or(unpacked.name.as_str(), |(stem, _)| stem);
    let nickname = nickname_of(nickname.unwrap_or_else(|| provider.unwrap_or(stem)));
    let mut report = ew_kicad::ImportReport::new(unpacked.name.clone());
    let mut symbols: Vec<SymbolDef> = Vec::new();
    let mut footprints: Vec<FootprintDef> = Vec::new();
    let mut problems = Vec::new();
    let mut models: BTreeMap<String, (PathBuf, u64, bool)> = BTreeMap::new();
    let mut legacy = false;
    for file in &unpacked.files {
        let path = unpacked.path(file);
        match extension(file).as_str() {
            "kicad_sym" | "kicad_mod" => {
                match ew_kicad::library::import(&path, Some(&nickname), &mut report) {
                    Ok(library) => {
                        for symbol in library.symbols {
                            if !symbols.iter().any(|s| s.name == symbol.name) {
                                symbols.push(symbol);
                            }
                        }
                        for footprint in library.footprints {
                            if !footprints.iter().any(|f| f.name == footprint.name) {
                                footprints.push(footprint);
                            }
                        }
                    }
                    Err(error) => problems.push(format!("{}: {error}", file.display())),
                }
            }
            "lib" | "dcm" | "mod" => legacy = true,
            _ if is_model(file) => {
                let name = file
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                let bytes = std::fs::metadata(&path).map_or(0, |m| m.len());
                models.entry(name).or_insert((file.clone(), bytes, false));
            }
            _ => {}
        }
    }
    for finding in report
        .findings
        .iter()
        .filter(|f| f.severity == ew_kicad::report::Severity::Error)
    {
        problems.push(finding.message.clone());
    }
    if legacy && symbols.is_empty() {
        problems.push(
            "the download has only KiCad 5 libraries (.lib, .mod); download the KiCad 6 (or newer) format"
                .to_owned(),
        );
    }
    if symbols.is_empty() && footprints.is_empty() && models.is_empty() {
        problems.push("the download contains no KiCad symbols, footprints or 3D models".to_owned());
    }

    link_footprints(&mut symbols, &footprints);
    link_models(&mut footprints, &mut models, &nickname);
    Parts {
        symbols,
        footprints,
        models,
        problems,
        nickname,
        provider,
    }
}

/// Symbols point at the downloaded footprints by their new names.
fn link_footprints(symbols: &mut [SymbolDef], footprints: &[FootprintDef]) {
    for symbol in symbols {
        let Some(wanted) = symbol.footprint.clone() else {
            continue;
        };
        let base = wanted.rsplit(':').next().unwrap_or(&wanted).to_owned();
        if let Some(own) = footprints
            .iter()
            .find(|f| f.name.rsplit(':').next() == Some(base.as_str()))
        {
            symbol.footprint = Some(own.name.clone());
        }
    }
}

/// Footprints point at the downloaded models in the project folder: a model
/// of the same file name, a STEP file of the same name for a VRML one, or
/// the only model for the only footprint.
fn link_models(
    footprints: &mut [FootprintDef],
    models: &mut BTreeMap<String, (PathBuf, u64, bool)>,
    nickname: &str,
) {
    let folder = model_folder(nickname);
    let single = models.len() == 1 && footprints.len() == 1;
    for footprint in footprints.iter_mut() {
        for model in &mut footprint.models {
            let file = model
                .path
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(&model.path)
                .to_owned();
            let stem = file
                .rsplit_once('.')
                .map_or(file.as_str(), |(s, _)| s)
                .to_ascii_lowercase();
            let found = models
                .keys()
                .find(|name| name.eq_ignore_ascii_case(&file))
                .or_else(|| {
                    models.keys().find(|name| {
                        let lower = name.to_ascii_lowercase();
                        lower
                            .rsplit_once('.')
                            .is_some_and(|(s, e)| s == stem && matches!(e, "step" | "stp"))
                    })
                })
                .cloned();
            if let Some(name) = found {
                model.path = format!("${{KIPRJMOD}}/{folder}/{name}");
                if let Some(entry) = models.get_mut(&name) {
                    entry.2 = true;
                }
            }
        }
        if footprint.models.is_empty()
            && single
            && let Some((name, entry)) = models.iter_mut().next()
        {
            footprint.models.push(Model3d {
                path: format!("${{KIPRJMOD}}/{folder}/{name}"),
                offset: Vec3::default(),
                rotation: Default::default(),
                scale: Scale3::default(),
                hidden: false,
            });
            entry.2 = true;
        }
    }
}

/// Replaces downloaded footprints (full names) by their duplicates in the
/// base library; symbols follow. Returns how many were replaced.
///
/// # Errors
///
/// Fails if a library entry cannot be read.
pub fn use_library(
    parts: &mut Parts,
    names: &[String],
    library: &mut LibraryState,
) -> Result<usize, EngineError> {
    let mut replaced = 0;
    for name in names {
        let Some(index) = parts.footprints.iter().position(|f| &f.name == name) else {
            continue;
        };
        let Some(duplicate) = library.footprint_duplicate(&parts.footprints[index]) else {
            continue;
        };
        let (_, definition, full_name) = library.definition(duplicate.id)?;
        let def: FootprintDef =
            serde_json::from_value(definition).map_err(EngineError::internal)?;
        parts.footprints.remove(index);
        for symbol in &mut parts.symbols {
            if symbol.footprint.as_deref() == Some(name.as_str()) {
                symbol.footprint = Some(full_name.clone());
            }
        }
        if !parts.footprints.iter().any(|f| f.name == def.name) {
            parts.footprints.push(def);
        }
        replaced += 1;
    }
    // Only models a remaining footprint uses are copied.
    let used: Vec<String> = parts
        .footprints
        .iter()
        .flat_map(|f| f.models.iter())
        .filter_map(|m| m.path.rsplit('/').next().map(str::to_owned))
        .collect();
    for (file, entry) in &mut parts.models {
        entry.2 = used.contains(file);
    }
    Ok(replaced)
}

/// One symbol or footprint of a preview.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemPreview {
    /// Full name (`nickname:name`).
    pub name: String,
    /// Description.
    pub description: String,
    /// Symbols: their footprint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footprint: Option<String>,
    /// Footprints: their 3D models (project paths).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
    /// Pins (symbols) or pads (footprints).
    pub pins: usize,
    /// Already in the target library (it is skipped).
    pub exists: bool,
    /// The base library's match.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duplicate: Option<Duplicate>,
}

/// One 3D model file of a preview.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPreview {
    /// File name.
    pub file: String,
    /// Size in bytes.
    pub bytes: u64,
    /// A footprint uses it (only used models are copied).
    pub used: bool,
}

/// What an import would bring.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    /// The download's file name.
    pub file: String,
    /// `SnapEDA`, `UltraLibrarian` or `null`.
    pub provider: Option<&'static str>,
    /// The library nickname (prefix of the names).
    pub nickname: String,
    /// Where the 3D models go (relative to the project).
    pub model_folder: String,
    /// Symbols.
    pub symbols: Vec<ItemPreview>,
    /// Footprints.
    pub footprints: Vec<ItemPreview>,
    /// 3D models.
    pub models: Vec<ModelPreview>,
    /// Problems.
    pub problems: Vec<String>,
}

/// Previews a download for a target library (`None`: nothing exists yet).
pub fn preview(
    parts: &Parts,
    name: &str,
    target: Option<&ew_model::library::Library>,
    library: &mut LibraryState,
) -> Preview {
    let symbols = parts
        .symbols
        .iter()
        .map(|symbol| ItemPreview {
            name: symbol.name.clone(),
            description: symbol.description.clone(),
            footprint: symbol.footprint.clone(),
            models: Vec::new(),
            pins: symbol.units.iter().map(|u| u.pins.len()).sum(),
            exists: target.is_some_and(|t| t.symbol(&symbol.name).is_some()),
            duplicate: library.symbol_duplicate(symbol),
        })
        .collect();
    let footprints = parts
        .footprints
        .iter()
        .map(|footprint| ItemPreview {
            name: footprint.name.clone(),
            description: footprint.description.clone(),
            footprint: None,
            models: footprint.models.iter().map(|m| m.path.clone()).collect(),
            pins: footprint.pads.len(),
            exists: target.is_some_and(|t| t.footprint(&footprint.name).is_some()),
            duplicate: library.footprint_duplicate(footprint),
        })
        .collect();
    Preview {
        file: name.to_owned(),
        provider: parts.provider,
        nickname: parts.nickname.clone(),
        model_folder: model_folder(&parts.nickname),
        symbols,
        footprints,
        models: parts
            .models
            .iter()
            .map(|(file, (_, bytes, used))| ModelPreview {
                file: file.clone(),
                bytes: *bytes,
                used: *used,
            })
            .collect(),
        problems: parts.problems.clone(),
    }
}

/// Parameters of `import.archivePreview`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewParams {
    /// The downloaded file.
    pub path: PathBuf,
    /// Library nickname (default: the provider or the file name).
    #[serde(default)]
    pub nickname: Option<String>,
    /// Target library (default: the project's first library).
    #[serde(default)]
    pub doc: Option<Id>,
}

/// Parameters of `import.archive`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportParams {
    /// The downloaded file.
    pub path: PathBuf,
    /// Library nickname.
    #[serde(default)]
    pub nickname: Option<String>,
    /// Target library.
    #[serde(default)]
    pub doc: Option<Id>,
    /// Downloaded footprints (full names) to replace by their duplicate in
    /// the base library.
    #[serde(default)]
    pub use_library: Vec<String>,
}

/// Copies the used models of a download into the project folder; returns
/// how many were copied. An existing file with other content is replaced.
///
/// # Errors
///
/// Fails if a file cannot be written.
pub fn copy_models(unpacked: &Unpacked, parts: &Parts, root: &Path) -> Result<usize, EngineError> {
    let folder = root.join(model_folder(&parts.nickname));
    let mut copied = 0;
    for (name, (relative, _, used)) in &parts.models {
        if !used {
            continue;
        }
        std::fs::create_dir_all(&folder)
            .map_err(|e| EngineError::io("creating the 3D folder", &e))?;
        let source = unpacked.path(relative);
        let target = folder.join(name);
        let same = match (std::fs::read(&source), std::fs::read(&target)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        };
        if !same {
            std::fs::copy(&source, &target)
                .map_err(|e| EngineError::io(&target.display().to_string(), &e))?;
        }
        copied += 1;
    }
    Ok(copied)
}

/// Reads a whole file (small text files of a download, for tests).
#[must_use]
pub fn read_text(unpacked: &Unpacked, file: &Path) -> Option<String> {
    let mut text = String::new();
    File::open(unpacked.path(file))
        .ok()?
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}
