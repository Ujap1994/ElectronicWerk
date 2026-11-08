//! Reading library sources: KiCad symbol, footprint and 3D model folders,
//! with meshes (GLB) from the tessellator.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use ew_kicad::ImportReport;
use rayon::prelude::*;
use sha2::{Digest, Sha256};

use crate::entry::{Entry, footprint_entry, model_entry, symbol_entry};
use crate::fingerprint::hex16;
use crate::index::SourceInfo;
use crate::mesh;

/// A library source: KiCad folders (each optional).
#[derive(Debug, Clone, Default)]
pub struct Source {
    /// Name, version and license.
    pub info: SourceInfo,
    /// Folder with `.kicad_sym` files and `.kicad_symdir` folders.
    pub symbols: Option<PathBuf>,
    /// Folder with `.pretty` footprint libraries.
    pub footprints: Option<PathBuf>,
    /// Folder with `.3dshapes` model libraries.
    pub models: Option<PathBuf>,
    /// Folder with the tessellated models: `<Library>.3dshapes/<name>.glb`.
    pub meshes: Option<PathBuf>,
}

/// What a source contained.
#[derive(Debug, Default)]
pub struct Ingested {
    /// The entries of every source.
    pub entries: Vec<Entry>,
    /// Problems while reading.
    pub report: ImportReport,
    /// GLB files by content hash.
    pub meshes: BTreeMap<String, PathBuf>,
}

/// Progress of an ingest: stage, files done, files in total.
pub type Progress<'a> = &'a (dyn Fn(&str, usize, usize) + Sync);

fn listing(dir: &Path, wanted: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| wanted(path))
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out
}

fn has_extension(path: &Path, extensions: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| extensions.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// Merges the findings of `from` into `into`.
fn merge(into: &mut ImportReport, from: ImportReport) {
    for finding in from.findings {
        match into.findings.iter_mut().find(|f| {
            f.severity == finding.severity
                && f.code == finding.code
                && f.file == finding.file
                && f.message == finding.message
        }) {
            Some(existing) => existing.count += finding.count,
            None => into.findings.push(finding),
        }
    }
    for (kind, version) in from.versions {
        into.versions.entry(kind).or_insert(version);
    }
}

/// Reads every source. The order of the entries does not depend on threads.
#[must_use]
pub fn ingest(sources: &[Source], progress: Progress<'_>) -> Ingested {
    let mut out = Ingested::default();
    for (index, source) in sources.iter().enumerate() {
        if let Some(dir) = &source.symbols {
            let files = listing(dir, |p| has_extension(p, &["kicad_sym", "kicad_symdir"]));
            let (entries, report) = libraries(&files, "symbols", progress, |path, report| {
                ew_kicad::library::import(path, None, report).map(|lib| {
                    lib.symbols
                        .iter()
                        .map(|def| symbol_entry(def, index))
                        .collect()
                })
            });
            out.entries.extend(entries);
            merge(&mut out.report, report);
        }
        if let Some(dir) = &source.footprints {
            let files = listing(dir, |p| p.is_dir() && has_extension(p, &["pretty"]));
            let (entries, report) = libraries(&files, "footprints", progress, |path, report| {
                ew_kicad::library::import(path, None, report).map(|lib| {
                    lib.footprints
                        .iter()
                        .map(|def| footprint_entry(def, index))
                        .collect()
                })
            });
            out.entries.extend(entries);
            merge(&mut out.report, report);
        }
        if let Some(dir) = &source.models {
            let (entries, meshes, report) = models(dir, source.meshes.as_deref(), index, progress);
            out.entries.extend(entries);
            out.meshes.extend(meshes);
            merge(&mut out.report, report);
        }
    }
    out.report.finish();
    out
}

fn libraries(
    files: &[PathBuf],
    stage: &str,
    progress: Progress<'_>,
    read: impl Fn(&Path, &mut ImportReport) -> Result<Vec<Entry>, ew_kicad::KicadError> + Sync,
) -> (Vec<Entry>, ImportReport) {
    let done = AtomicUsize::new(0);
    let parts: Vec<(Vec<Entry>, ImportReport)> = files
        .par_iter()
        .map(|path| {
            let mut report = ImportReport::default();
            let entries = read(path, &mut report).unwrap_or_else(|error| {
                report.error(
                    "unreadable-file",
                    Some(&path.display().to_string()),
                    error.to_string(),
                );
                Vec::new()
            });
            progress(stage, done.fetch_add(1, Ordering::Relaxed) + 1, files.len());
            (entries, report)
        })
        .collect();
    let mut entries = Vec::new();
    let mut report = ImportReport::default();
    for (list, part) in parts {
        entries.extend(list);
        merge(&mut report, part);
    }
    (entries, report)
}

/// The model files of a `.3dshapes` folder, one per name (STEP preferred).
fn model_files(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut by_name: BTreeMap<String, PathBuf> = BTreeMap::new();
    for file in listing(dir, |p| has_extension(p, &["step", "stp", "wrl", "wrz"])) {
        let Some(stem) = file.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let is_step = has_extension(&file, &["step", "stp"]);
        match by_name.get(stem) {
            Some(existing) if has_extension(existing, &["step", "stp"]) || !is_step => {}
            _ => {
                by_name.insert(stem.to_owned(), file);
            }
        }
    }
    by_name.into_iter().collect()
}

/// A model entry, its mesh file (hash, path) and a problem while reading it.
type ModelResult = (Entry, Option<(String, PathBuf)>, Option<String>);

fn models(
    dir: &Path,
    meshes: Option<&Path>,
    source: usize,
    progress: Progress<'_>,
) -> (Vec<Entry>, BTreeMap<String, PathBuf>, ImportReport) {
    let libraries = listing(dir, |p| p.is_dir() && has_extension(p, &["3dshapes"]));
    let mut jobs = Vec::new();
    for library in &libraries {
        let folder = library
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        let name = folder
            .strip_suffix(".3dshapes")
            .unwrap_or(folder)
            .to_owned();
        for (stem, file) in model_files(library) {
            let relative = format!(
                "{folder}/{}",
                file.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default()
            );
            let glb = meshes.map(|m| m.join(folder).join(format!("{stem}.glb")));
            jobs.push((name.clone(), stem, relative, glb));
        }
    }
    let done = AtomicUsize::new(0);
    let results: Vec<ModelResult> = jobs
        .par_iter()
        .map(|(library, stem, relative, glb)| {
            let mut problem = None;
            let mesh = glb.as_ref().filter(|path| path.is_file()).and_then(|path| {
                let bytes = std::fs::read(path).ok()?;
                match mesh::read_glb(&bytes) {
                    Ok(print) => Some((hex16(&Sha256::digest(&bytes)), print, path.clone())),
                    Err(error) => {
                        problem = Some(format!("{}: {error}", path.display()));
                        None
                    }
                }
            });
            progress(
                "models",
                done.fetch_add(1, Ordering::Relaxed) + 1,
                jobs.len(),
            );
            let file = mesh
                .as_ref()
                .map(|(hash, _, path)| (hash.clone(), path.clone()));
            let entry = model_entry(
                library,
                stem,
                relative,
                mesh.map(|(hash, print, _)| (hash, print)),
                source,
            );
            (entry, file, problem)
        })
        .collect();
    let mut entries = Vec::with_capacity(results.len());
    let mut files = BTreeMap::new();
    let mut report = ImportReport::default();
    let mut missing = 0;
    for (entry, file, problem) in results {
        if entry.mesh.is_none() && meshes.is_some() {
            missing += 1;
        }
        entries.push(entry);
        if let Some((hash, path)) = file {
            files.insert(hash, path);
        }
        if let Some(problem) = problem {
            report.warn("unreadable-mesh", None, problem);
        }
    }
    if missing > 0 {
        report.info(
            "missing-mesh",
            None,
            format!("{missing} models have no tessellated mesh yet"),
        );
    }
    (entries, files, report)
}

/// Finds KiCad libraries in `path`: a KiCad installation (`symbols`,
/// `footprints`, `3dmodels`) or checkouts of the library repositories
/// (`kicad-symbols…`, `kicad-footprints…`, `kicad-packages3D…`).
#[must_use]
pub fn detect_kicad(path: &Path) -> Option<Source> {
    let sub = |names: &[&str], prefix: &str| -> Option<PathBuf> {
        names
            .iter()
            .map(|name| path.join(name))
            .find(|p| p.is_dir())
            .or_else(|| {
                listing(path, |p| {
                    p.is_dir()
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with(prefix) && !n.contains("-source"))
                })
                .into_iter()
                .next()
            })
    };
    let source = Source {
        info: SourceInfo {
            name: "KiCad".to_owned(),
            version: String::new(),
            license: "CC-BY-SA-4.0 WITH KiCad-libraries-exception".to_owned(),
            url: "https://www.kicad.org/libraries/".to_owned(),
        },
        symbols: sub(&["symbols"], "kicad-symbols"),
        footprints: sub(&["footprints"], "kicad-footprints"),
        models: sub(&["3dmodels"], "kicad-packages3D"),
        meshes: None,
    };
    (source.symbols.is_some() || source.footprints.is_some()).then_some(source)
}

/// Where KiCad keeps its libraries on this system (existing folders only).
#[must_use]
pub fn kicad_installations() -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        candidates.push("/Applications/KiCad/KiCad.app/Contents/SharedSupport".into());
    } else if cfg!(windows) {
        for base in ["ProgramFiles", "ProgramW6432"] {
            if let Some(dir) = std::env::var_os(base) {
                let kicad = PathBuf::from(dir).join("KiCad");
                let mut versions = listing(&kicad, Path::is_dir);
                versions.reverse();
                candidates.extend(versions.into_iter().map(|v| v.join("share").join("kicad")));
            }
        }
    } else {
        candidates.extend([
            "/usr/share/kicad".into(),
            "/usr/local/share/kicad".into(),
            "/var/lib/flatpak/app/org.kicad.KiCad/current/active/files/share/kicad".into(),
        ]);
    }
    candidates.retain(|path| detect_kicad(path).is_some());
    candidates.dedup();
    candidates
}
