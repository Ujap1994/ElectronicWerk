//! The base library in the engine: the `library.*` methods (ADR-0005).
//!
//! The library lives in its own folder (by default
//! `<config>/ElectronicWerk/library`, or `EW_LIBRARY_DIR`): the index
//! (`library.sqlite`), the review decisions, the reports and the mesh cache
//! (`meshes/<hash>.glb`). Builds run in a background thread; they write to
//! `staging/` and replace the index when they are done, so searches keep
//! working meanwhile.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use ew_library::dedup::{Class, Decision};
use ew_library::index::{Library, Query};
use ew_library::{Built, Kind, LibraryError, Source};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::{EngineError, ErrorCode};

/// Folder of a build in progress, inside the library folder.
const STAGING: &str = "staging";

/// The library folder of this system: `EW_LIBRARY_DIR`, or
/// `ElectronicWerk/library` in the user's configuration folder (the same
/// folder the desktop app uses for its settings).
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("EW_LIBRARY_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let config = if cfg!(target_os = "macos") {
        home.map(|h| h.join("Library").join("Application Support"))
    } else if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(|| home.map(|h| h.join(".config")))
    };
    config.map(|c| c.join("ElectronicWerk").join("library"))
}

fn library_error(error: LibraryError) -> EngineError {
    match error {
        LibraryError::NotFound(message) => EngineError::not_found(message),
        LibraryError::Io { path, source } => EngineError::io(&path.display().to_string(), &source),
        LibraryError::Format(message) => EngineError::new(ErrorCode::FormatError, message),
        LibraryError::Sqlite(message) => EngineError::internal(message),
    }
}

/// Shared event queue (filled by the build thread).
type Events = Arc<Mutex<Vec<Value>>>;

struct Job {
    handle: JoinHandle<Result<Built, LibraryError>>,
    staging: PathBuf,
    started: Instant,
    progress: Arc<Mutex<Value>>,
}

/// The library state of an engine.
#[derive(Default)]
pub struct LibraryState {
    dir: Option<PathBuf>,
    index: Option<Library>,
    job: Option<Job>,
    events: Events,
}

impl std::fmt::Debug for LibraryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryState")
            .field("dir", &self.dir)
            .field("open", &self.index.is_some())
            .field("building", &self.job.is_some())
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DirParams {
    dir: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdParams {
    id: i64,
}

#[derive(Deserialize)]
struct ModelParams {
    path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KindParams {
    kind: Kind,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
struct GroupParams {
    kind: Option<Kind>,
    class: Option<String>,
    decision: Option<String>,
    offset: u32,
    limit: Option<u32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecideParams {
    key: String,
    decision: Decision,
    #[serde(default)]
    canonical: Option<i64>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase", default)]
struct SourceParams {
    name: Option<String>,
    version: Option<String>,
    license: Option<String>,
    url: Option<String>,
    /// A KiCad installation or folder with library checkouts.
    kicad: Option<PathBuf>,
    symbols: Option<PathBuf>,
    footprints: Option<PathBuf>,
    models: Option<PathBuf>,
    meshes: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildParams {
    sources: Vec<SourceParams>,
    #[serde(default)]
    license: Option<String>,
    /// Build in the request (scripts) instead of in the background.
    #[serde(default)]
    wait: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InstallParams {
    file: PathBuf,
    /// The library release of the pack (`library-…`).
    #[serde(default)]
    release: Option<String>,
    /// Delete the pack after a successful installation.
    #[serde(default)]
    remove: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportParams {
    #[serde(default)]
    format: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MeshParams {
    hash: String,
}

/// Parameters of `library.preview`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreviewParams {
    pub id: i64,
    #[serde(default)]
    pub unit: Option<u16>,
    #[serde(default)]
    pub labels: Option<ew_scene::TitleLabels>,
    #[serde(default)]
    pub encoding: Option<String>,
}

fn parse<T: serde::de::DeserializeOwned>(params: &Value) -> Result<T, EngineError> {
    let value = if params.is_null() {
        json!({})
    } else {
        params.clone()
    };
    serde_json::from_value(value).map_err(|error| EngineError::invalid_params(error.to_string()))
}

fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, EngineError> {
    serde_json::to_value(value).map_err(EngineError::internal)
}

fn source_of(params: SourceParams) -> Result<Source, EngineError> {
    let mut source = match &params.kicad {
        Some(path) => ew_library::ingest::detect_kicad(path).ok_or_else(|| {
            EngineError::not_found(format!("no KiCad libraries in {}", path.display()))
        })?,
        None => Source::default(),
    };
    let info = &mut source.info;
    if let Some(name) = params.name {
        info.name = name;
    } else if info.name.is_empty() {
        "Local".clone_into(&mut info.name);
    }
    if let Some(version) = params.version {
        info.version = version;
    }
    if let Some(license) = params.license {
        info.license = license;
    }
    if let Some(url) = params.url {
        info.url = url;
    }
    source.symbols = params.symbols.or(source.symbols);
    source.footprints = params.footprints.or(source.footprints);
    source.models = params.models.or(source.models);
    source.meshes = params.meshes.or(source.meshes);
    if source.symbols.is_none() && source.footprints.is_none() && source.models.is_none() {
        return Err(EngineError::invalid_params(
            "a source needs symbols, footprints or models",
        ));
    }
    Ok(source)
}

/// Moves the files of a finished build into the library folder.
fn promote(staging: &Path, dir: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(staging)? {
        let path = entry?.path();
        let Some(name) = path.file_name().map(ToOwned::to_owned) else {
            continue;
        };
        let target = dir.join(&name);
        if path.is_dir() {
            std::fs::create_dir_all(&target)?;
            for mesh in std::fs::read_dir(&path)? {
                let mesh = mesh?.path();
                if let Some(file) = mesh.file_name() {
                    let to = target.join(file);
                    if !to.exists() {
                        std::fs::rename(&mesh, &to)?;
                    }
                }
            }
        } else if name != ew_library::dedup::Decisions::FILE {
            if target.exists() {
                std::fs::remove_file(&target)?;
            }
            std::fs::rename(&path, &target)?;
        }
    }
    std::fs::remove_dir_all(staging)
}

impl LibraryState {
    /// The library folder (resolved on first use).
    fn dir(&mut self) -> Result<PathBuf, EngineError> {
        if self.dir.is_none() {
            self.dir = default_dir();
        }
        self.dir
            .clone()
            .ok_or_else(|| EngineError::not_found("no library folder (set EW_LIBRARY_DIR)"))
    }

    /// The open index, opening it on first use.
    fn index(&mut self) -> Result<&Library, EngineError> {
        if self.index.is_none() {
            let dir = self.dir()?;
            match Library::open(&dir) {
                Ok(library) => self.index = Some(library),
                Err(LibraryError::NotFound(_)) => {
                    return Err(EngineError::not_found(
                        "no library is installed yet; download it or build it from KiCad libraries",
                    ));
                }
                Err(error) => return Err(library_error(error)),
            }
        }
        self.index
            .as_ref()
            .ok_or_else(|| EngineError::not_found("no library"))
    }

    /// Moves events of the build thread to `out` and finishes a build that
    /// is done.
    pub fn poll(&mut self, out: &mut Vec<Value>) {
        out.append(&mut self.events.lock().unwrap_or_else(PoisonError::into_inner));
        let done = self
            .job
            .as_ref()
            .is_some_and(|job| job.handle.is_finished());
        if !done {
            return;
        }
        let Some(job) = self.job.take() else {
            return;
        };
        let seconds = job.started.elapsed().as_secs_f64();
        let result = job
            .handle
            .join()
            .unwrap_or_else(|_| Err(LibraryError::Format("the library build crashed".to_owned())));
        match result {
            Ok(built) => {
                let problems = built.problems.clone();
                drop(built);
                self.index = None;
                let dir = self.dir.clone().unwrap_or_default();
                if let Err(error) = promote(&job.staging, &dir) {
                    out.push(json!({"type": "library", "action": "failed", "message": error.to_string()}));
                    return;
                }
                let status = self.status_value();
                out.push(json!({
                    "type": "library", "action": "built", "seconds": seconds,
                    "status": status, "problems": problems
                }));
            }
            Err(error) => {
                let _ = std::fs::remove_dir_all(&job.staging);
                out.push(
                    json!({"type": "library", "action": "failed", "message": error.to_string()}),
                );
            }
        }
    }

    fn status_value(&mut self) -> Value {
        let dir = self.dir().ok();
        let building = self.job.as_ref().map(|job| {
            job.progress
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()
        });
        let status = self.index().ok().and_then(|index| index.status().ok());
        json!({
            "dir": dir.map(|d| d.display().to_string()),
            "installed": status.is_some(),
            "building": building,
            "status": status,
        })
    }

    /// Executes a `library.*` method (except `library.add` and
    /// `library.preview`, which need the engine).
    ///
    /// # Errors
    ///
    /// Fails like the method.
    #[allow(clippy::too_many_lines, reason = "one short arm per method")]
    pub fn call(&mut self, method: &str, params: &Value) -> Result<Value, EngineError> {
        match method {
            "library.status" => Ok(self.status_value()),
            "library.open" => {
                let params: DirParams = parse(params)?;
                if self.job.is_some() {
                    return Err(EngineError::conflict("a library build is running"));
                }
                self.index = None;
                self.dir = Some(params.dir);
                Ok(self.status_value())
            }
            "library.detect" => {
                let found: Vec<Value> = ew_library::ingest::kicad_installations()
                    .into_iter()
                    .filter_map(|path| {
                        let source = ew_library::ingest::detect_kicad(&path)?;
                        Some(json!({
                            "path": path.display().to_string(),
                            "symbols": source.symbols.map(|p| p.display().to_string()),
                            "footprints": source.footprints.map(|p| p.display().to_string()),
                            "models": source.models.map(|p| p.display().to_string()),
                        }))
                    })
                    .collect();
                Ok(Value::Array(found))
            }
            "library.search" => {
                let query: Query = parse(params)?;
                to_value(&self.index()?.search(&query).map_err(library_error)?)
            }
            "library.facets" => {
                let params: KindParams = parse(params)?;
                to_value(&self.index()?.facets(params.kind).map_err(library_error)?)
            }
            "library.get" => {
                let params: IdParams = parse(params)?;
                to_value(&self.index()?.get(params.id).map_err(library_error)?)
            }
            "library.model" => {
                // The base library's entry of a footprint's model path (for
                // the 3D preview of the footprint editor).
                let params: ModelParams = parse(params)?;
                let key = ew_library::entry::model_key(&params.path);
                let id = match (&key, self.index()) {
                    (Some(key), Ok(index)) => index.model_id(key).map_err(library_error)?,
                    _ => None,
                };
                Ok(json!({"key": key, "id": id}))
            }
            "library.groups" => {
                let params: GroupParams = parse(params)?;
                let class = params
                    .class
                    .as_deref()
                    .map(|c| {
                        Class::parse(c).ok_or_else(|| EngineError::invalid_params("unknown class"))
                    })
                    .transpose()?;
                let decision = params
                    .decision
                    .as_deref()
                    .map(|d| {
                        Decision::parse(d)
                            .ok_or_else(|| EngineError::invalid_params("unknown decision"))
                    })
                    .transpose()?;
                let page = self
                    .index()?
                    .groups(
                        params.kind,
                        class,
                        decision,
                        params.offset,
                        params.limit.unwrap_or(100),
                    )
                    .map_err(library_error)?;
                to_value(&page)
            }
            "library.decide" => {
                let params: DecideParams = parse(params)?;
                let group = self
                    .index()?
                    .decide(&params.key, params.decision, params.canonical)
                    .map_err(library_error)?;
                self.events
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(json!({"type": "library", "action": "changed"}));
                to_value(&group)
            }
            "library.report" => {
                let params: ReportParams = parse(params)?;
                let report = self.index()?.report().map_err(library_error)?;
                Ok(Value::String(match params.format.as_deref() {
                    Some("markdown") => report.to_markdown(),
                    _ => report.to_json(),
                }))
            }
            "library.mesh" => {
                let params: MeshParams = parse(params)?;
                if !params.hash.chars().all(|c| c.is_ascii_hexdigit()) || params.hash.len() != 32 {
                    return Err(EngineError::invalid_params("not a mesh hash"));
                }
                let path = ew_library::pack::mesh_path(&self.dir()?, &params.hash);
                Ok(json!({"path": path.display().to_string(), "exists": path.is_file()}))
            }
            "library.install" => {
                let params: InstallParams = parse(params)?;
                if self.job.is_some() {
                    return Err(EngineError::conflict("a library build is running"));
                }
                let dir = self.dir()?;
                // Close the index first: Windows cannot replace an open file.
                self.index = None;
                let installed =
                    ew_library::pack::install(&params.file, &dir).map_err(library_error)?;
                if installed.index {
                    if let Some(release) = &params.release {
                        self.index()?
                            .set_meta("release", release)
                            .map_err(library_error)?;
                    }
                }
                if params.remove {
                    let _ = std::fs::remove_file(&params.file);
                }
                self.events
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(json!({"type": "library", "action": "changed"}));
                Ok(json!({"installed": installed, "status": self.status_value()}))
            }
            "library.build" => self.build(parse(params)?),
            other => Err(EngineError::new(
                ErrorCode::MethodNotFound,
                format!("unknown method `{other}`"),
            )),
        }
    }

    fn build(&mut self, params: BuildParams) -> Result<Value, EngineError> {
        if self.job.is_some() {
            return Err(EngineError::conflict("a library build is already running"));
        }
        let sources: Vec<Source> = params
            .sources
            .into_iter()
            .map(source_of)
            .collect::<Result<_, _>>()?;
        if sources.is_empty() {
            return Err(EngineError::invalid_params("no sources"));
        }
        let dir = self.dir()?;
        let staging = dir.join(STAGING);
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::create_dir_all(&staging)
            .map_err(|error| EngineError::io(&staging.display().to_string(), &error))?;
        let decisions = dir.join(ew_library::dedup::Decisions::FILE);
        if decisions.is_file() {
            let _ = std::fs::copy(&decisions, staging.join(ew_library::dedup::Decisions::FILE));
        }
        let license = params.license;
        if params.wait {
            let built = ew_library::build(&staging, &sources, license.as_deref(), &|_, _, _| {})
                .map_err(library_error)?;
            let problems = built.problems.clone();
            drop(built);
            self.index = None;
            promote(&staging, &dir)
                .map_err(|error| EngineError::io(&dir.display().to_string(), &error))?;
            return Ok(json!({"status": self.status_value(), "problems": problems}));
        }
        let events = Arc::clone(&self.events);
        let progress = Arc::new(Mutex::new(json!({"stage": "start", "done": 0, "total": 0})));
        let shared = Arc::clone(&progress);
        let target = staging.clone();
        let handle = std::thread::Builder::new()
            .name("library-build".to_owned())
            .spawn(move || {
                let last = Mutex::new(
                    Instant::now()
                        .checked_sub(Duration::from_secs(1))
                        .unwrap_or_else(Instant::now),
                );
                let report = |stage: &str, done: usize, total: usize| {
                    let value = json!({"stage": stage, "done": done, "total": total});
                    *shared.lock().unwrap_or_else(PoisonError::into_inner) = value.clone();
                    let mut last = last.lock().unwrap_or_else(PoisonError::into_inner);
                    if done == total || last.elapsed() >= Duration::from_millis(200) {
                        *last = Instant::now();
                        let mut event = value;
                        event["type"] = json!("library");
                        event["action"] = json!("progress");
                        events
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .push(event);
                    }
                };
                ew_library::build(&target, &sources, license.as_deref(), &report)
            })
            .map_err(EngineError::internal)?;
        self.job = Some(Job {
            handle,
            staging,
            started: Instant::now(),
            progress,
        });
        Ok(json!({"started": true}))
    }

    /// The definition of an entry and its kind (for `library.preview` and
    /// `library.add`).
    pub(crate) fn definition(&mut self, id: i64) -> Result<(Kind, Value, String), EngineError> {
        let details = self.index()?.get(id).map_err(library_error)?;
        let definition = details.definition.ok_or_else(|| {
            EngineError::invalid_params("this entry has no drawing (3D models have none)")
        })?;
        Ok((
            details.summary.kind,
            definition,
            format!("{}:{}", details.summary.library, details.summary.name),
        ))
    }

    /// The id of `library:name` of a kind.
    ///
    /// # Errors
    ///
    /// Fails if no library is installed.
    pub(crate) fn find(&mut self, kind: Kind, name: &str) -> Result<Option<i64>, EngineError> {
        self.index()?.find(kind, name).map_err(library_error)
    }
}
