//! Library packs: ZIP archives that distribute a built library.
//!
//! - The **index pack** holds `library.sqlite`, the deduplication report and
//!   the license of the library data.
//! - One **mesh pack** per 3D library (`Resistor_SMD`) holds its meshes as
//!   `meshes/<hash>.glb`. They are downloaded on demand, so nobody needs the
//!   whole 3D library (more than 3 GB as STEP files) to start.
//!
//! `manifest.json` lists the packs with their size and SHA-256 checksum.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;

use crate::error::LibraryError;
use crate::fingerprint::hex16;
use crate::index::{INDEX_FILE, Library, SourceInfo};

/// Version of the pack layout.
pub const PACK_FORMAT: u32 = 1;

/// Folder of the mesh cache inside a library folder.
pub const MESH_DIR: &str = "meshes";

/// Reports written next to the index.
pub const REPORT_JSON: &str = "dedup-report.json";
/// Reports written next to the index.
pub const REPORT_MARKDOWN: &str = "dedup-report.md";
/// The license of the library data.
pub const LICENSE_FILE: &str = "LICENSE.md";

/// The list of packs of a library release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Pack layout version.
    pub format: u32,
    /// Name of the library release (`KiCad 10.0.0`).
    pub name: String,
    /// Version of the library release.
    pub version: String,
    /// Sources of the library data.
    pub sources: Vec<SourceInfo>,
    /// Entries by kind (`symbol`, `footprint`, `model`).
    pub counts: BTreeMap<String, u64>,
    /// The packs.
    pub packs: Vec<PackFile>,
}

/// One pack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    /// File name.
    pub file: String,
    /// `index` or `meshes`.
    pub kind: String,
    /// The 3D library of a mesh pack.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library: Option<String>,
    /// Size in bytes.
    pub size: u64,
    /// SHA-256 of the file (hex).
    pub sha256: String,
    /// Files in the pack.
    pub entries: u64,
}

/// What an installation added.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    /// The index was replaced.
    pub index: bool,
    /// Meshes added.
    pub meshes: u64,
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> LibraryError + '_ {
    move |source| LibraryError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn zip_error(path: &Path) -> impl FnOnce(zip::result::ZipError) -> LibraryError + '_ {
    move |error| LibraryError::Format(format!("{}: {error}", path.display()))
}

/// The path of a cached mesh.
#[must_use]
pub fn mesh_path(dir: &Path, hash: &str) -> PathBuf {
    dir.join(MESH_DIR).join(format!("{hash}.glb"))
}

/// The SHA-256 of a file as hex.
///
/// # Errors
///
/// Fails if the file cannot be read.
pub fn sha256_file(path: &Path) -> Result<String, LibraryError> {
    let mut file = File::open(path).map_err(io(path))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1 << 16];
    loop {
        let n = file.read(&mut buffer).map_err(io(path))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(hex)
}

/// Copies meshes into the mesh cache of `dir` (named by content hash).
///
/// # Errors
///
/// Fails if a file cannot be copied.
pub fn store_meshes(dir: &Path, meshes: &BTreeMap<String, PathBuf>) -> Result<u64, LibraryError> {
    let target = dir.join(MESH_DIR);
    std::fs::create_dir_all(&target).map_err(io(&target))?;
    let mut added = 0;
    for (hash, source) in meshes {
        let path = mesh_path(dir, hash);
        if !path.exists() {
            std::fs::copy(source, &path).map_err(io(&path))?;
            added += 1;
        }
    }
    Ok(added)
}

fn options() -> SimpleFileOptions {
    // A fixed time stamp keeps packs reproducible.
    SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .last_modified_time(zip::DateTime::default())
}

fn add_file(
    zip: &mut zip::ZipWriter<File>,
    name: &str,
    path: &Path,
    out: &Path,
) -> Result<(), LibraryError> {
    zip.start_file(name, options()).map_err(zip_error(out))?;
    let mut file = File::open(path).map_err(io(path))?;
    std::io::copy(&mut file, zip).map_err(io(out))?;
    Ok(())
}

/// Writes the packs of the library in `dir` to `out` and returns the
/// manifest (also written as `manifest.json`). `prefix` starts every file
/// name (`ewerk-library-kicad-10.0.0`).
///
/// # Errors
///
/// Fails if the index cannot be read or a pack cannot be written.
pub fn write_packs(
    dir: &Path,
    out: &Path,
    prefix: &str,
    name: &str,
    version: &str,
) -> Result<Manifest, LibraryError> {
    std::fs::create_dir_all(out).map_err(io(out))?;
    let library = Library::open(dir)?;
    let status = library.status()?;
    let mut packs = Vec::new();

    // The index pack.
    let file = format!("{prefix}-index.zip");
    let path = out.join(&file);
    let mut zip = zip::ZipWriter::new(File::create(&path).map_err(io(&path))?);
    let mut entries = 0;
    for name in [INDEX_FILE, REPORT_JSON, REPORT_MARKDOWN, LICENSE_FILE] {
        let source = dir.join(name);
        if source.is_file() {
            add_file(&mut zip, name, &source, &path)?;
            entries += 1;
        }
    }
    zip.finish().map_err(zip_error(&path))?;
    packs.push(PackFile {
        size: std::fs::metadata(&path).map_err(io(&path))?.len(),
        sha256: sha256_file(&path)?,
        file,
        kind: "index".to_owned(),
        library: None,
        entries,
    });

    // One mesh pack per 3D library.
    let mut by_library: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (library_name, hash) in library.meshes()? {
        by_library.entry(library_name).or_default().push(hash);
    }
    for (library_name, mut hashes) in by_library {
        hashes.sort();
        hashes.dedup();
        let file = format!("{prefix}-meshes-{library_name}.zip");
        let path = out.join(&file);
        let mut zip = zip::ZipWriter::new(File::create(&path).map_err(io(&path))?);
        let mut entries = 0;
        for hash in &hashes {
            let source = mesh_path(dir, hash);
            if source.is_file() {
                add_file(&mut zip, &format!("{MESH_DIR}/{hash}.glb"), &source, &path)?;
                entries += 1;
            }
        }
        zip.finish().map_err(zip_error(&path))?;
        packs.push(PackFile {
            size: std::fs::metadata(&path).map_err(io(&path))?.len(),
            sha256: sha256_file(&path)?,
            file,
            kind: "meshes".to_owned(),
            library: Some(library_name),
            entries,
        });
    }
    let manifest = Manifest {
        format: PACK_FORMAT,
        name: name.to_owned(),
        version: version.to_owned(),
        sources: status.sources,
        counts: status.counts,
        packs,
    };
    let path = out.join("manifest.json");
    let mut text = serde_json::to_string_pretty(&manifest).unwrap_or_default();
    text.push('\n');
    std::fs::write(&path, text).map_err(io(&path))?;
    Ok(manifest)
}

/// Installs a pack into the library folder `dir`: an index pack replaces
/// the index (decisions stay), a mesh pack adds meshes to the cache. Mesh
/// files are checked against their names.
///
/// # Errors
///
/// Fails if the pack is damaged; the old index then stays in place.
pub fn install(pack: &Path, dir: &Path) -> Result<Installed, LibraryError> {
    std::fs::create_dir_all(dir.join(MESH_DIR)).map_err(io(dir))?;
    let file = File::open(pack).map_err(io(pack))?;
    let mut zip = zip::ZipArchive::new(file).map_err(zip_error(pack))?;
    let mut installed = Installed::default();
    for index in 0..zip.len() {
        let mut item = zip.by_index(index).map_err(zip_error(pack))?;
        let name = item.name().map_err(zip_error(pack))?.into_owned();
        let mut bytes = Vec::with_capacity(usize::try_from(item.size()).unwrap_or(0));
        item.read_to_end(&mut bytes).map_err(io(pack))?;
        if let Some(mesh) = name.strip_prefix(&format!("{MESH_DIR}/")) {
            let Some(hash) = mesh.strip_suffix(".glb") else {
                continue;
            };
            if hash.contains(['/', '\\', '.']) || hex16(&Sha256::digest(&bytes)) != hash {
                return Err(LibraryError::Format(format!(
                    "{}: {name} does not match its checksum",
                    pack.display()
                )));
            }
            let path = mesh_path(dir, hash);
            if !path.exists() {
                write_atomic(&path, &bytes)?;
                installed.meshes += 1;
            }
        } else if [INDEX_FILE, REPORT_JSON, REPORT_MARKDOWN, LICENSE_FILE].contains(&name.as_str())
        {
            let target = dir.join(&name);
            if name == INDEX_FILE {
                // Check the new index before it replaces the old one.
                let check = dir.join("incoming");
                std::fs::create_dir_all(&check).map_err(io(&check))?;
                std::fs::write(check.join(INDEX_FILE), &bytes).map_err(io(&check))?;
                let valid = Library::open(&check).map(drop);
                let _ = std::fs::remove_dir_all(&check);
                valid?;
                installed.index = true;
            }
            write_atomic(&target, &bytes)?;
        }
    }
    Ok(installed)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), LibraryError> {
    let temp = path.with_extension("part");
    let mut file = File::create(&temp).map_err(io(&temp))?;
    file.write_all(bytes).map_err(io(&temp))?;
    file.sync_all().map_err(io(&temp))?;
    drop(file);
    std::fs::rename(&temp, path).map_err(io(path))
}
