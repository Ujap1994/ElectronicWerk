//! The `outputs.*` and `gerber.*` methods: output jobs (presets, running
//! them into the project folder), the bill of materials preview and the
//! Gerber viewer. The logic lives in `ew-output`.

use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::outputs::OutputJob;
use ew_output::bom::{self, BomOptions, Cell};
use std::cell::RefCell;

use ew_output::job::{self, Model3dFile, Model3dJob, RunContext};

use crate::board3d::ExportParams;
use crate::library::LibraryState;
use ew_output::viewer::{self, ViewerFile};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::change::Request;
use crate::error::EngineError;
use crate::session::Session;

/// History labels of the output methods.
pub mod labels {
    /// `outputs.create`.
    pub const CREATE: &str = "add-output-job";
    /// `outputs.applyPreset`.
    pub const PRESET: &str = "output-preset";
}

/// What a method does.
pub enum Outcome {
    /// A reply without changes.
    Read(Value),
    /// Requests to commit as one undo step, and extra reply fields.
    Commit {
        /// History label.
        label: &'static str,
        /// Requests.
        requests: Vec<Request>,
        /// Extra reply fields.
        reply: Value,
    },
}

fn parse<T: DeserializeOwned>(params: &Value) -> Result<T, EngineError> {
    serde_json::from_value(params.clone())
        .map_err(|error| EngineError::invalid_params(error.to_string()))
}

fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, EngineError> {
    serde_json::to_value(value).map_err(|error| EngineError::invalid_params(error.to_string()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateParams {
    name: String,
    #[serde(default)]
    preset: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PresetParams {
    job: Id,
    preset: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RunParams {
    job: Id,
    #[serde(default)]
    outputs: Option<Vec<Id>>,
    #[serde(default)]
    labels: Option<ew_scene::TitleLabels>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
struct BomParams {
    source: Option<Id>,
    options: Option<BomOptions>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct JobParams {
    job: Id,
}

/// Parameters of `gerber.scene`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GerberSceneParams {
    /// Files to show (absolute, or relative to the project folder).
    pub paths: Vec<String>,
    /// Name of the view.
    #[serde(default)]
    pub name: Option<String>,
    /// `none` leaves the binary data out of the JSON reply.
    #[serde(default)]
    pub encoding: Option<String>,
}

fn output_job(session: &Session, id: Id) -> Result<&OutputJob, EngineError> {
    match session.project().document(id) {
        Some(Document::OutputJob(job)) => Ok(job),
        Some(_) => Err(EngineError::invalid_params(
            "the document is not an output job",
        )),
        None => Err(EngineError::not_found(format!("no document with id {id}"))),
    }
}

fn preset_outputs(name: &str) -> Result<Value, EngineError> {
    let preset = job::presets()
        .into_iter()
        .find(|p| p.id == name)
        .ok_or_else(|| EngineError::invalid_params(format!("unknown preset `{name}`")))?;
    to_value(&job::preset_outputs(&preset, Id::random))
}

/// A relative path inside the project folder, or an error.
fn inside(root: &Path, relative: &str) -> Result<PathBuf, EngineError> {
    let path = Path::new(relative);
    if path.is_absolute()
        || path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
    {
        return Err(EngineError::invalid_params(format!(
            "`{relative}` is not inside the project folder"
        )));
    }
    Ok(root.join(path))
}

/// The output folder of a job.
fn job_folder(session: &Session, job: &OutputJob) -> Result<PathBuf, EngineError> {
    let directory = job.directory.trim().trim_matches('/');
    let directory = if directory.is_empty() {
        ew_model::outputs::DEFAULT_OUTPUT_DIRECTORY
    } else {
        directory
    };
    inside(session.root(), directory)
}

fn run(
    session: &Session,
    library: &mut LibraryState,
    params: &RunParams,
    version: &str,
) -> Result<Value, EngineError> {
    let job = output_job(session, params.job)?;
    let folder = job_folder(session, job)?;
    let started = Instant::now();
    let created = ew_format::time::DateTime::now().iso_string();
    let resolve = |text: &str, doc: Option<Id>| session.resolve_text(text, doc);
    let library = RefCell::new(library);
    let model3d = |pcb: &ew_model::pcb::Pcb, o: &Model3dJob| -> Result<Model3dFile, String> {
        let params: ExportParams = serde_json::from_value(json!({
            "format": if o.format == "wrl" { "vrml" } else { o.format.as_str() },
            "origin": o.origin,
            "includeDnp": o.include_dnp,
            "units": o.units,
        }))
        .map_err(|error| format!("invalid options: {error}"))?;
        let mut library = library.borrow_mut();
        let (data, report) =
            crate::board3d::export_board(session, &mut library, pcb, &params, version, &created);
        Ok(Model3dFile {
            data,
            note: crate::board3d::summary(&report),
        })
    };
    let ctx = RunContext {
        project: session.project(),
        version: version.to_owned(),
        created: created.clone(),
        resolve: &resolve,
        labels: params.labels.clone().unwrap_or_default(),
        model3d: Some(&model3d),
    };
    let result = job::run(job, params.outputs.as_deref(), &ctx);
    let mut files = Vec::new();
    for file in &result.files {
        let target = inside(&folder, &file.name)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| EngineError::io("creating the output folder", &error))?;
        }
        ew_format::fsio::write_atomic(&target, &file.data)
            .map_err(|error| EngineError::invalid_params(error.to_string()))?;
        files.push(json!({"path": file.name, "bytes": file.data.len()}));
    }
    Ok(json!({
        "directory": folder.to_string_lossy(),
        "results": result.results,
        "files": files,
        "elapsedMs": started.elapsed().as_millis(),
    }))
}

fn bom_preview(session: &Session, params: &BomParams) -> Result<Value, EngineError> {
    let project = session.project();
    let board =
        match params.source {
            Some(id) => match project.document(id) {
                Some(Document::Pcb(pcb)) => Some(pcb),
                _ => return Err(EngineError::invalid_params("the source is not a board")),
            },
            None => project.manifest.documents.iter().find_map(|entry| {
                match project.document(entry.id) {
                    Some(Document::Pcb(pcb)) => Some(pcb),
                    _ => None,
                }
            }),
        };
    let parts = bom::parts(project, board);
    let options = params.options.clone().unwrap_or_default();
    let table = bom::build(&parts, &options);
    let rows: Vec<Vec<Value>> = table
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| match cell {
                    Cell::Number(n) => json!(n),
                    Cell::Text(t) => json!(t),
                })
                .collect()
        })
        .collect();
    Ok(json!({
        "headers": table.headers,
        "rows": rows,
        "parts": table.parts,
        "fields": fields(&parts),
    }))
}

/// The fields a bill of materials can show: the built-in ones and every
/// field name of the parts.
fn fields(parts: &[bom::Part]) -> Vec<String> {
    let mut out: Vec<String> = [
        "item",
        "quantity",
        "designators",
        "value",
        "footprint",
        "symbol",
        "description",
        "side",
        "mounting",
        "dnp",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let mut extra: Vec<String> = parts
        .iter()
        .flat_map(|p| p.properties.keys().cloned())
        .collect();
    extra.sort_by_key(|k| k.to_lowercase());
    extra.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    for key in ["Manufacturer", "MPN", "LCSC"] {
        if !extra.iter().any(|k| k.eq_ignore_ascii_case(key)) {
            extra.push(key.to_owned());
        }
    }
    out.extend(extra);
    out
}

/// Gerber and drill files in a job's output folder (recursively).
fn files_of_job(session: &Session, id: Id) -> Result<Value, EngineError> {
    let job = output_job(session, id)?;
    let folder = job_folder(session, job)?;
    let mut found = Vec::new();
    let mut stack = vec![folder.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let known = matches!(
                ext.as_str(),
                "gbr"
                    | "gtl"
                    | "gbl"
                    | "gts"
                    | "gbs"
                    | "gto"
                    | "gbo"
                    | "gtp"
                    | "gbp"
                    | "gta"
                    | "gba"
                    | "gm1"
                    | "gko"
                    | "drl"
                    | "xln"
            ) || (ext.starts_with('g') && ext[1..].parse::<u32>().is_ok());
            if known {
                found.push(path.to_string_lossy().into_owned());
            }
        }
    }
    found.sort();
    Ok(json!({"directory": folder.to_string_lossy(), "files": found}))
}

/// Builds the scene of Gerber and drill files.
///
/// # Errors
///
/// Fails if no file can be read.
pub fn gerber_scene(
    root: Option<&Path>,
    params: &GerberSceneParams,
) -> Result<(Value, Vec<u8>), EngineError> {
    let mut files = Vec::new();
    let mut problems = Vec::new();
    for path in &params.paths {
        let full = match root {
            Some(root) if Path::new(path).is_relative() => root.join(path),
            _ => PathBuf::from(path),
        };
        let name = full
            .file_name()
            .map_or_else(|| path.clone(), |n| n.to_string_lossy().into_owned());
        let Ok(bytes) = std::fs::read(&full) else {
            problems.push(json!({"file": name, "message": "cannot be read"}));
            continue;
        };
        let text = String::from_utf8_lossy(&bytes);
        match ew_output::read::read(&text) {
            Some(image) => {
                for warning in &image.warnings {
                    problems.push(json!({"file": name, "message": warning}));
                }
                files.push(ViewerFile { name, image });
            }
            None => problems.push(json!({"file": name, "message": "not a Gerber or drill file"})),
        }
    }
    if files.is_empty() {
        return Err(EngineError::invalid_params(
            "none of the files is a Gerber or drill file",
        ));
    }
    let key: Vec<&str> = params.paths.iter().map(String::as_str).collect();
    let doc = Id::derived(&key);
    let name = params.name.clone().unwrap_or_else(|| "Gerber".to_owned());
    let functions: Vec<Value> = files
        .iter()
        .map(|f| json!({"file": f.name, "function": viewer::function_of(f)}))
        .collect();
    let scene = viewer::scene(&files, doc, &name);
    let mut meta = to_value(&scene.meta)?;
    if let Some(object) = meta.as_object_mut() {
        object.insert("files".to_owned(), Value::Array(functions));
        object.insert("problems".to_owned(), Value::Array(problems));
    }
    Ok((meta, scene.data))
}

/// Runs an `outputs.*` method.
///
/// # Errors
///
/// Fails on bad parameters, unknown documents or write errors.
pub fn call(
    session: &Session,
    library: &mut LibraryState,
    method: &str,
    params: &Value,
    version: &str,
) -> Result<Outcome, EngineError> {
    match method {
        "outputs.presets" => Ok(Outcome::Read(to_value(&job::presets())?)),
        "outputs.create" => {
            let params: CreateParams = parse(params)?;
            crate::change::check_name(&params.name)?;
            let id = Id::random();
            let outputs = preset_outputs(params.preset.as_deref().unwrap_or("generic"))?;
            let mut props = Map::new();
            props.insert("outputs".to_owned(), outputs);
            Ok(Outcome::Commit {
                label: labels::CREATE,
                requests: vec![
                    Request::AddDocument {
                        kind: DocumentKind::OutputJob,
                        name: params.name.trim().to_owned(),
                        path: None,
                        id: Some(id),
                    },
                    Request::SetProps { doc: id, props },
                ],
                reply: json!({"id": id}),
            })
        }
        "outputs.applyPreset" => {
            let params: PresetParams = parse(params)?;
            output_job(session, params.job)?;
            let mut props = Map::new();
            props.insert("outputs".to_owned(), preset_outputs(&params.preset)?);
            Ok(Outcome::Commit {
                label: labels::PRESET,
                requests: vec![Request::SetProps {
                    doc: params.job,
                    props,
                }],
                reply: json!({}),
            })
        }
        "outputs.run" => Ok(Outcome::Read(run(
            session,
            library,
            &parse(params)?,
            version,
        )?)),
        "outputs.bom" => Ok(Outcome::Read(bom_preview(session, &parse(params)?)?)),
        "outputs.files" => {
            let params: JobParams = parse(params)?;
            Ok(Outcome::Read(files_of_job(session, params.job)?))
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
    fn paths_stay_inside_the_project() {
        let root = Path::new("/p");
        assert!(inside(root, "outputs/generated").is_ok());
        assert!(inside(root, "../x").is_err());
        assert!(inside(root, "/etc").is_err());
        assert!(inside(root, "a/../../b").is_err());
    }
}
