//! Engine side of the 3D view (M9): `board3d.scene` (the board meshes as a
//! binary buffer plus the parts), `board3d.parts` (only the parts, for live
//! updates while parts move) and the resolution of 3D model paths.
//!
//! A footprint names its models like KiCad does
//! (`${KICAD10_3DMODEL_DIR}/Package_SO.3dshapes/SOIC-8.step`,
//! `${KIPRJMOD}/models/part.step`, relative paths). A model is looked for:
//!
//! 1. as a file: `${KIPRJMOD}` and relative paths in the project folder,
//!    other variables from the environment, the KiCad model variables also
//!    in the folders of a KiCad installation; a `.wrl` path also finds a
//!    `.step` file of the same name;
//! 2. in the base library, by its library and name (the mesh of a downloaded
//!    library, or the STEP file of a library built from local folders).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use ew_board3d::board::{self, BoardOptions};
use ew_board3d::place;
use ew_board3d::scene;
use ew_board3d::stack::Levels;
use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::pcb::Pcb;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::EngineError;
use crate::library::LibraryState;
use crate::session::Session;

/// Where a model was found.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelSource {
    /// The path as the footprint names it.
    pub path: String,
    /// A model file on this computer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// `step` or `vrml` for files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<&'static str>,
    /// The base library's model entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<i64>,
    /// Hash of the library model's mesh in the mesh cache.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh: Option<String>,
}

impl ModelSource {
    /// `true` if the model can be shown.
    #[must_use]
    pub const fn found(&self) -> bool {
        self.file.is_some() || self.entry.is_some()
    }
}

/// Folders of the 3D models of KiCad installations on this system.
#[must_use]
pub fn kicad_model_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        dirs.push("/Applications/KiCad/KiCad.app/Contents/SharedSupport/3dmodels".into());
    } else if cfg!(windows) {
        let base = std::env::var_os("ProgramFiles")
            .map_or_else(|| PathBuf::from(r"C:\Program Files"), PathBuf::from);
        for version in ["10.0", "9.0", "8.0", "7.0"] {
            dirs.push(
                base.join("KiCad")
                    .join(version)
                    .join("share")
                    .join("kicad")
                    .join("3dmodels"),
            );
        }
    } else {
        dirs.extend([
            "/usr/share/kicad/3dmodels".into(),
            "/usr/local/share/kicad/3dmodels".into(),
            "/var/lib/flatpak/app/org.kicad.KiCad/current/active/files/share/kicad/3dmodels".into(),
        ]);
    }
    dirs.retain(|dir| dir.is_dir());
    dirs
}

/// `true` for KiCad's model folder variables (`KICAD10_3DMODEL_DIR`, …).
fn is_kicad_model_var(name: &str) -> bool {
    name.starts_with("KICAD") && name.ends_with("3DMODEL_DIR")
}

/// The candidate files of a model path (variables expanded).
#[must_use]
pub fn candidates(path: &str, root: &Path, kicad_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let text = path.trim().replace('\\', "/");
    // `${NAME}` or `$(NAME)` at the start.
    let (variable, rest) = if let Some(after) = text.strip_prefix("${") {
        after
            .split_once('}')
            .map_or((None, text.as_str()), |(name, rest)| (Some(name), rest))
    } else if let Some(after) = text.strip_prefix("$(") {
        after
            .split_once(')')
            .map_or((None, text.as_str()), |(name, rest)| (Some(name), rest))
    } else {
        (None, text.as_str())
    };
    let rest = rest.trim_start_matches('/');
    let mut bases: Vec<PathBuf> = Vec::new();
    match variable {
        Some("KIPRJMOD" | "PROJECT_DIR") => bases.push(root.to_path_buf()),
        Some(name) => {
            if let Some(value) = std::env::var_os(name).filter(|v| !v.is_empty()) {
                bases.push(PathBuf::from(value));
            }
            if is_kicad_model_var(name) {
                bases.extend(kicad_dirs.iter().cloned());
            }
        }
        None => {}
    }
    let mut files: Vec<PathBuf> = if variable.is_some() {
        bases.iter().map(|base| base.join(rest)).collect()
    } else {
        let plain = PathBuf::from(&text);
        if plain.is_absolute() {
            vec![plain]
        } else {
            vec![root.join(&text)]
        }
    };
    // KiCad substitutes a STEP model for a VRML one of the same name.
    let mut out = Vec::new();
    for file in files.drain(..) {
        let ext = file
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        if matches!(ext.as_deref(), Some("wrl" | "wrz")) {
            out.push(file.with_extension("step"));
            out.push(file.with_extension("stp"));
        }
        out.push(file);
    }
    out
}

/// The format of a model file by its extension.
fn format_of(file: &Path) -> Option<&'static str> {
    match file
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("step" | "stp") => Some("step"),
        Some("wrl" | "wrz") => Some("vrml"),
        _ => None,
    }
}

/// Finds a model.
pub fn resolve(
    path: &str,
    root: &Path,
    kicad_dirs: &[PathBuf],
    library: &mut LibraryState,
) -> ModelSource {
    let mut source = ModelSource {
        path: path.to_owned(),
        ..ModelSource::default()
    };
    if let Some(file) = candidates(path, root, kicad_dirs)
        .into_iter()
        .find(|file| file.is_file() && format_of(file).is_some())
    {
        source.format = format_of(&file);
        source.file = Some(file.to_string_lossy().into_owned());
    }
    if let Some(model) = library.model(path) {
        source.entry = Some(model.id);
        source.mesh = model.mesh;
        if source.file.is_none()
            && let Some(file) = model.file
        {
            source.format = format_of(&file);
            source.file = Some(file.to_string_lossy().into_owned());
        }
    }
    source
}

/// Resolves every model path of a board once.
pub fn sources(pcb: &Pcb, root: &Path, library: &mut LibraryState) -> Vec<ModelSource> {
    let kicad_dirs = kicad_model_dirs();
    let paths: BTreeSet<&str> = pcb
        .footprints
        .iter()
        .flat_map(|fp| fp.models.iter().filter(|m| !m.hidden))
        .map(|model| model.path.as_str())
        .collect();
    paths
        .into_iter()
        .map(|path| resolve(path, root, &kicad_dirs, library))
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SceneParams {
    doc: Id,
    #[serde(default)]
    inner: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PartsParams {
    doc: Id,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResolveParams {
    path: String,
}

fn board_of(session: &Session, doc: Id) -> Result<&Pcb, EngineError> {
    match session.project().document(doc) {
        Some(Document::Pcb(pcb)) => Ok(pcb),
        Some(_) => Err(EngineError::invalid_params("the document is not a board")),
        None => Err(EngineError::not_found(format!("no document {doc}"))),
    }
}

fn parse<T: for<'de> Deserialize<'de>>(params: &Value) -> Result<T, EngineError> {
    serde_json::from_value(params.clone())
        .map_err(|error| EngineError::invalid_params(error.to_string()))
}

fn to_value<T: Serialize>(value: &T) -> Result<Value, EngineError> {
    serde_json::to_value(value).map_err(|error| EngineError::internal(error.to_string()))
}

/// `board3d.scene`: the board meshes (binary) and the metadata with the
/// parts and the found models.
///
/// # Errors
///
/// Fails if the document is not a board.
pub fn scene(
    session: &Session,
    library: &mut LibraryState,
    params: &Value,
) -> Result<(Value, Vec<u8>), EngineError> {
    let params: SceneParams = parse(params)?;
    let pcb = board_of(session, params.doc)?;
    let doc = params.doc;
    let resolve = |text: &str| session.resolve_text(text, Some(doc));
    let options = BoardOptions {
        inner: params.inner,
        ..BoardOptions::default()
    };
    let built = board::build(pcb, &resolve, &options);
    let parts = place::parts(pcb, &built.levels);
    let (meta, data) = scene::encode(&pcb.name, &built, parts);
    let mut value = to_value(&meta)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "models".into(),
            to_value(&sources(pcb, session.root(), library))?,
        );
    }
    Ok((value, data))
}

/// Calls of `board3d.*` that return JSON only.
///
/// # Errors
///
/// Fails on bad parameters or if the document is not a board.
pub fn call(
    session: Option<&Session>,
    library: &mut LibraryState,
    method: &str,
    params: &Value,
) -> Result<Value, EngineError> {
    match method {
        "board3d.parts" => {
            let session = session.ok_or_else(EngineError::no_project)?;
            let params: PartsParams = parse(params)?;
            let pcb = board_of(session, params.doc)?;
            let levels = Levels::of(&pcb.stackup);
            Ok(serde_json::json!({
                "parts": to_value(&place::parts(pcb, &levels))?,
                "models": to_value(&sources(pcb, session.root(), library))?,
            }))
        }
        "board3d.resolve" => {
            // One model path (the footprint editor's 3D preview).
            let params: ResolveParams = parse(params)?;
            let root = session.map_or_else(PathBuf::new, |s| s.root().to_path_buf());
            to_value(&resolve(&params.path, &root, &kicad_model_dirs(), library))
        }
        other => Err(EngineError::new(
            crate::error::ErrorCode::MethodNotFound,
            format!("unknown method `{other}`"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_paths_expand_like_kicad() {
        let root = Path::new("/projects/blinky");
        let kicad = vec![PathBuf::from("/opt/kicad/3dmodels")];
        let found = candidates("${KIPRJMOD}/models/part.step", root, &kicad);
        assert_eq!(found, vec![root.join("models/part.step")]);
        let found = candidates(
            "${KICAD10_3DMODEL_DIR}/Resistor_SMD.3dshapes/R_0805.wrl",
            root,
            &kicad,
        );
        assert!(found.contains(&PathBuf::from(
            "/opt/kicad/3dmodels/Resistor_SMD.3dshapes/R_0805.step"
        )));
        assert!(found.contains(&PathBuf::from(
            "/opt/kicad/3dmodels/Resistor_SMD.3dshapes/R_0805.wrl"
        )));
        assert_eq!(
            candidates("models/x.stp", root, &kicad),
            vec![root.join("models/x.stp")]
        );
        assert_eq!(
            candidates("${UNSET_VARIABLE_FOR_TEST}/x.step", root, &kicad),
            Vec::<PathBuf>::new()
        );
        assert_eq!(format_of(Path::new("a.STP")), Some("step"));
        assert_eq!(format_of(Path::new("a.wrl")), Some("vrml"));
        assert_eq!(format_of(Path::new("a.igs")), None);
    }
}
