//! JSON-based RPC: the engine API used by the desktop app, the CLI and
//! tests.
//!
//! A request is a JSON object `{"method": "...", "params": ...}`. The reply
//! is either `{"result": ...}` or `{"error": {"code": "...", "message":
//! "..."}}`. Request ids and transport are the caller's business.
//!
//! The [`Engine`] keeps the open project between calls. After every call,
//! [`Engine::take_events`] returns the change sets and project events that
//! the call produced, so every view can follow the edits.

use std::path::{Path, PathBuf};

use ew_format::{archive, store, validate};
use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::object::Object;
use ew_model::project::Units;
use ew_model::schematic::{DEFAULT_SHEET_SYMBOL_SIZE, SheetSymbol};
use ew_model::units::{Point, mm};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::change::Request;
use crate::error::{EngineError, ErrorCode};
use crate::import;
use crate::info::EngineInfo;
use crate::library::{LibraryState, PreviewParams};
use crate::session::{ChangeSet, Session};
use crate::summary;
use crate::templates::{self, BoardOptions, DocumentNames, NewProject};

/// A single engine request.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RpcRequest {
    /// Fully qualified method name, for example `engine.info`.
    pub method: String,
    /// Method parameters; `null` when omitted.
    #[serde(default)]
    pub params: Value,
}

/// Error returned to the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RpcError {
    /// Error category.
    pub code: ErrorCode,
    /// Human-readable description (English, for logs and developers).
    pub message: String,
}

impl From<EngineError> for RpcError {
    fn from(error: EngineError) -> Self {
        Self {
            code: error.code,
            message: error.message,
        }
    }
}

/// Reply to a request.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Response {
    /// The method succeeded.
    Result(Value),
    /// The method failed.
    Error(RpcError),
}

/// All methods the engine understands, in alphabetical order.
pub const METHODS: &[&str] = &[
    "document.add",
    "document.get",
    "document.properties",
    "document.remove",
    "document.rename",
    "document.setProperties",
    "edit.apply",
    "edit.history",
    "edit.redo",
    "edit.undo",
    "engine.info",
    "engine.methods",
    "engine.ping",
    "import.kicad",
    "import.kicadProbe",
    "import.library",
    "import.netlist",
    "libedit.check",
    "libedit.create",
    "libedit.delete",
    "libedit.footprint",
    "libedit.item",
    "libedit.list",
    "libedit.presets",
    "libedit.rename",
    "libedit.setProps",
    "libedit.symbol",
    "libedit.wizard",
    "library.add",
    "library.build",
    "library.decide",
    "library.detect",
    "library.facets",
    "library.get",
    "library.groups",
    "library.install",
    "library.mesh",
    "library.open",
    "library.preview",
    "library.report",
    "library.search",
    "library.status",
    "project.archive",
    "project.close",
    "project.create",
    "project.discardRecovery",
    "project.files",
    "project.info",
    "project.open",
    "project.probe",
    "project.recover",
    "project.resolveText",
    "project.save",
    "project.saveAs",
    "project.saveCopy",
    "project.setProperties",
    "project.templates",
    "project.validate",
    "schematic.add",
    "schematic.annotate",
    "schematic.bom",
    "schematic.components",
    "schematic.copy",
    "schematic.delete",
    "schematic.dragInfo",
    "schematic.erc",
    "schematic.ercSettings",
    "schematic.find",
    "schematic.move",
    "schematic.net",
    "schematic.netlist",
    "schematic.orient",
    "schematic.paste",
    "schematic.place",
    "schematic.replace",
    "schematic.setErcSettings",
    "schematic.snapToGrid",
    "schematic.symbol",
    "schematic.symbols",
    "schematic.syncSheetPins",
    "schematic.wire",
    "view.item",
    "view.object",
    "view.scene",
];

/// Transaction labels used by the built-in commands. Clients translate
/// them for undo/redo menus; scripts may use their own labels.
pub mod labels {
    /// `document.add`.
    pub const ADD_DOCUMENT: &str = "add-document";
    /// `document.add` with a parent sheet.
    pub const ADD_SHEET: &str = "add-sheet";
    /// `document.remove`.
    pub const REMOVE_DOCUMENT: &str = "remove-document";
    /// `document.rename`.
    pub const RENAME_DOCUMENT: &str = "rename-document";
    /// `document.setProperties`.
    pub const SET_DOCUMENT_PROPERTIES: &str = "set-document-properties";
    /// `project.setProperties`.
    pub const SET_PROJECT_PROPERTIES: &str = "set-project-properties";
    /// `import.library`.
    pub const IMPORT_LIBRARY: &str = "import-library";
    /// `import.netlist` with `apply`.
    pub const IMPORT_NETLIST: &str = "import-netlist";
    /// `library.add`.
    pub const ADD_FROM_LIBRARY: &str = "add-from-library";
    /// `edit.apply` without a label.
    pub const APPLY: &str = "apply";
}

/// The engine: holds the open project and serves requests.
#[derive(Debug)]
pub struct Engine {
    session: Option<Session>,
    events: Vec<Value>,
    journaling: bool,
    library: LibraryState,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

fn parse<T: DeserializeOwned>(params: &Value) -> Result<T, EngineError> {
    let value = if params.is_null() {
        Value::Object(Map::new())
    } else {
        params.clone()
    };
    serde_json::from_value(value).map_err(|error| EngineError::invalid_params(error.to_string()))
}

fn to_value<T: Serialize>(value: &T) -> Result<Value, EngineError> {
    serde_json::to_value(value).map_err(EngineError::internal)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathParams {
    path: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DocParams {
    doc: Id,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CreateParams {
    path: PathBuf,
    name: String,
    #[serde(default)]
    template: Option<String>,
    #[serde(default)]
    board: Option<BoardOptions>,
    #[serde(default)]
    units: Option<Units>,
    #[serde(default)]
    names: Option<DocumentNames>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddDocumentParams {
    kind: DocumentKind,
    name: String,
    #[serde(default)]
    parent: Option<Id>,
    /// Top-left corner of the sheet symbol (with `parent`).
    #[serde(default)]
    at: Option<Point>,
    /// Size of the sheet symbol (with `parent`).
    #[serde(default)]
    size: Option<ew_model::units::Size>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenameParams {
    doc: Id,
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetPropsParams {
    doc: Id,
    props: Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyParams {
    #[serde(default)]
    label: Option<String>,
    changes: Vec<Value>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct SaveParams {
    #[serde(default)]
    all: bool,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct CloseParams {
    #[serde(default)]
    discard: bool,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ValidateParams {
    #[serde(default)]
    path: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ImportKicadParams {
    source: PathBuf,
    path: PathBuf,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    names: Option<DocumentNames>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceParams {
    source: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportLibraryParams {
    source: PathBuf,
    #[serde(default)]
    doc: Option<Id>,
    #[serde(default)]
    nickname: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddParams {
    ids: Vec<i64>,
    #[serde(default)]
    doc: Option<Id>,
    /// Also add the default footprints of symbols.
    #[serde(default = "yes")]
    footprints: bool,
}

const fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportNetlistParams {
    source: PathBuf,
    #[serde(default)]
    doc: Option<Id>,
    #[serde(default)]
    apply: bool,
}

/// How `view.scene` returns the binary buffer to JSON clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum SceneEncoding {
    /// `data` is a base64 string (default).
    Base64,
    /// `data` is `null` (only the metadata is wanted).
    None,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneParams {
    doc: Id,
    #[serde(default)]
    encoding: Option<SceneEncoding>,
    #[serde(default)]
    labels: Option<ew_scene::TitleLabels>,
}

/// `view.item`: a symbol or footprint of a project library in its editor.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ItemSceneParams {
    doc: Id,
    kind: ew_libedit::refs::LibraryKind,
    name: String,
    #[serde(default)]
    unit: Option<u16>,
    #[serde(default)]
    alternate: bool,
    #[serde(default)]
    encoding: Option<SceneEncoding>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveParams {
    text: String,
    #[serde(default)]
    doc: Option<Id>,
}

impl Engine {
    /// An engine with the autosave journal enabled.
    #[must_use]
    pub fn new() -> Self {
        Self {
            session: None,
            events: Vec::new(),
            journaling: true,
            library: LibraryState::default(),
        }
    }

    /// An engine without autosave journal (one-shot command-line calls).
    #[must_use]
    pub fn without_journal() -> Self {
        Self {
            session: None,
            events: Vec::new(),
            journaling: false,
            library: LibraryState::default(),
        }
    }

    /// The open project, if any.
    #[must_use]
    pub const fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    /// Mutable access to the open project (for embedding and tests).
    pub const fn session_mut(&mut self) -> Option<&mut Session> {
        self.session.as_mut()
    }

    /// Takes the events produced since the last call.
    pub fn take_events(&mut self) -> Vec<Value> {
        self.library.poll(&mut self.events);
        std::mem::take(&mut self.events)
    }

    /// Parses a JSON request, executes it and returns the JSON reply. Never
    /// panics on bad input.
    pub fn dispatch(&mut self, request_json: &str) -> String {
        let response = match serde_json::from_str::<RpcRequest>(request_json) {
            Ok(request) => self.handle(&request),
            Err(err) => Response::Error(RpcError {
                code: ErrorCode::InvalidRequest,
                message: format!("malformed request: {err}"),
            }),
        };
        encode(&response)
    }

    /// Like [`Engine::dispatch`], but `view.scene` returns its binary buffer
    /// separately instead of base64 inside the JSON (used by the desktop
    /// app, which hands the buffer to the GPU as it is).
    pub fn dispatch_binary(&mut self, request_json: &str) -> (String, Option<Vec<u8>>) {
        let Ok(request) = serde_json::from_str::<RpcRequest>(request_json) else {
            return (self.dispatch(request_json), None);
        };
        let result = match request.method.as_str() {
            "view.scene" => {
                parse::<SceneParams>(&request.params).and_then(|params| self.scene(&params))
            }
            "library.preview" => parse::<PreviewParams>(&request.params)
                .and_then(|params| self.library_preview(&params)),
            "view.item" => parse::<ItemSceneParams>(&request.params)
                .and_then(|params| self.item_scene(&params)),
            _ => return (encode(&self.handle(&request)), None),
        };
        match result {
            Ok((meta, data)) => (encode(&Response::Result(meta)), Some(data)),
            Err(error) => (encode(&Response::Error(error.into())), None),
        }
    }

    /// Builds the scene of a document: metadata and binary buffer.
    fn scene(&self, params: &SceneParams) -> Result<(Value, Vec<u8>), EngineError> {
        let session = self.open()?;
        let doc = params.doc;
        let resolve = |text: &str| session.resolve_text(text, Some(doc));
        let options = ew_scene::Options {
            resolve: &resolve,
            labels: params.labels.clone().unwrap_or_default(),
        };
        let scene =
            ew_scene::build(session.project(), doc, &options).map_err(|error| match error {
                ew_scene::SceneError::NotFound(_) => EngineError::not_found(error.to_string()),
                ew_scene::SceneError::NotViewable => EngineError::invalid_params(error.to_string()),
            })?;
        Ok((to_value(&scene.meta)?, scene.data))
    }

    /// Builds the editor scene of a library symbol or footprint.
    fn item_scene(&self, params: &ItemSceneParams) -> Result<(Value, Vec<u8>), EngineError> {
        use ew_libedit::refs::LibraryKind;
        let session = self.open()?;
        let Some(Document::Library(library)) = session.project().document(params.doc) else {
            return Err(EngineError::not_found(format!("no library {}", params.doc)));
        };
        let resolve = |text: &str| text.to_owned();
        let options = ew_scene::Options {
            resolve: &resolve,
            labels: ew_scene::TitleLabels::default(),
        };
        let scene = match params.kind {
            LibraryKind::Symbol => {
                let def = library.symbol(&params.name).ok_or_else(|| {
                    EngineError::not_found(format!("no symbol `{}`", params.name))
                })?;
                ew_scene::symbol_editor(def, params.unit.unwrap_or(1), params.alternate, &options)
            }
            LibraryKind::Footprint => {
                let def = library.footprint(&params.name).ok_or_else(|| {
                    EngineError::not_found(format!("no footprint `{}`", params.name))
                })?;
                ew_scene::footprint_editor(def, &options)
            }
            LibraryKind::Component => {
                return Err(EngineError::invalid_params("components have no drawing"));
            }
        };
        Ok((to_value(&scene.meta)?, scene.data))
    }

    /// Executes a parsed request.
    pub fn handle(&mut self, request: &RpcRequest) -> Response {
        match self.call(&request.method, &request.params) {
            Ok(value) => Response::Result(value),
            Err(error) => Response::Error(error.into()),
        }
    }

    fn open(&self) -> Result<&Session, EngineError> {
        self.session.as_ref().ok_or_else(EngineError::no_project)
    }

    fn open_mut(&mut self) -> Result<&mut Session, EngineError> {
        self.session.as_mut().ok_or_else(EngineError::no_project)
    }

    fn snapshot(&self) -> Result<Value, EngineError> {
        to_value(&summary::snapshot(self.open()?))
    }

    fn project_event(&mut self, action: &str) {
        let path = self
            .session
            .as_ref()
            .map(|session| session.manifest_path().to_string_lossy().into_owned());
        self.events
            .push(json!({"type": "project", "action": action, "path": path}));
    }

    fn change_event(&mut self, change_set: &ChangeSet) -> Result<(), EngineError> {
        self.events
            .push(json!({"type": "changed", "changeSet": to_value(change_set)?}));
        Ok(())
    }

    fn ensure_closed(&self) -> Result<(), EngineError> {
        if self.session.is_some() {
            return Err(EngineError::new(
                ErrorCode::ProjectOpen,
                "another project is open; close it first",
            ));
        }
        Ok(())
    }

    fn commit(&mut self, label: &str, requests: Vec<Request>) -> Result<ChangeSet, EngineError> {
        let change_set = self.open_mut()?.commit(label, requests)?;
        self.change_event(&change_set)?;
        Ok(change_set)
    }

    fn call(&mut self, method: &str, params: &Value) -> Result<Value, EngineError> {
        match method {
            "engine.info" => to_value(&EngineInfo::current()),
            "engine.methods" => to_value(&METHODS),
            "engine.ping" => Ok(json!({"pong": true, "echo": params})),
            _ if method.starts_with("project.") => self.call_project(method, params),
            _ if method.starts_with("document.") => self.call_document(method, params),
            _ if method.starts_with("edit.") => self.call_edit(method, params),
            _ if method.starts_with("import.") => self.call_import(method, params),
            _ if method.starts_with("schematic.") => self.call_schematic(method, params),
            _ if method.starts_with("libedit.") => self.call_libedit(method, params),
            "library.preview" => {
                let params: PreviewParams = parse(params)?;
                let (mut meta, data) = self.library_preview(&params)?;
                if let Some(object) = meta.as_object_mut() {
                    let encoded = if params.encoding.as_deref() == Some("none") {
                        Value::Null
                    } else {
                        Value::String(ew_scene::base64(&data))
                    };
                    object.insert("data".to_owned(), encoded);
                }
                Ok(meta)
            }
            "library.add" => self.library_add(&parse(params)?),
            _ if method.starts_with("library.") => self.library.call(method, params),
            "view.scene" => {
                let params: SceneParams = parse(params)?;
                let (mut meta, data) = self.scene(&params)?;
                if let Some(object) = meta.as_object_mut() {
                    let encoded = if params.encoding == Some(SceneEncoding::None) {
                        Value::Null
                    } else {
                        Value::String(ew_scene::base64(&data))
                    };
                    object.insert("data".to_owned(), encoded);
                }
                Ok(meta)
            }
            "view.item" => {
                let params: ItemSceneParams = parse(params)?;
                let (mut meta, data) = self.item_scene(&params)?;
                if let Some(object) = meta.as_object_mut() {
                    let encoded = if params.encoding == Some(SceneEncoding::None) {
                        Value::Null
                    } else {
                        Value::String(ew_scene::base64(&data))
                    };
                    object.insert("data".to_owned(), encoded);
                }
                Ok(meta)
            }
            "view.object" => {
                let target: ew_scene::ObjectRef = parse(params)?;
                let session = self.open()?;
                let info = ew_scene::describe(session.project(), &target).ok_or_else(|| {
                    EngineError::not_found(format!("no object with id {}", target.id))
                })?;
                to_value(&info)
            }
            other => Err(EngineError::new(
                ErrorCode::MethodNotFound,
                format!("unknown method `{other}`"),
            )),
        }
    }

    fn call_project(&mut self, method: &str, params: &Value) -> Result<Value, EngineError> {
        match method {
            "project.save"
            | "project.saveAs"
            | "project.saveCopy"
            | "project.archive"
            | "project.files"
            | "project.validate"
            | "project.resolveText"
            | "project.setProperties" => self.call_project_data(method, params),
            "project.templates" => to_value(&templates::TEMPLATES),
            "project.create" => {
                self.ensure_closed()?;
                let params: CreateParams = parse(params)?;
                let project = templates::build(&NewProject {
                    name: params.name,
                    template: params.template.unwrap_or_else(|| "empty".to_owned()),
                    board: params.board,
                    units: params.units.unwrap_or_default(),
                    names: params.names.unwrap_or_default(),
                })?;
                self.session = Some(Session::create(&params.path, &project, self.journaling)?);
                self.project_event("opened");
                self.snapshot()
            }
            "project.open" => {
                self.ensure_closed()?;
                let params: PathParams = parse(params)?;
                self.session = Some(Session::open(&params.path, self.journaling)?);
                self.project_event("opened");
                self.snapshot()
            }
            "project.probe" => {
                let params: PathParams = parse(params)?;
                Ok(probe(&params.path))
            }
            "project.info" => self.snapshot(),
            "project.recover" => {
                let change_set = self.open_mut()?.recover()?;
                self.change_event(&change_set)?;
                self.project_event("recovered");
                self.snapshot()
            }
            "project.discardRecovery" => {
                self.open_mut()?.discard_recovery()?;
                self.project_event("recoveryDiscarded");
                self.snapshot()
            }
            "project.close" => {
                let params: CloseParams = parse(params)?;
                self.open_mut()?.close(params.discard)?;
                self.project_event("closed");
                self.session = None;
                Ok(json!({"closed": true}))
            }
            other => Err(EngineError::new(
                ErrorCode::MethodNotFound,
                format!("unknown method `{other}`"),
            )),
        }
    }

    fn call_project_data(&mut self, method: &str, params: &Value) -> Result<Value, EngineError> {
        match method {
            "project.save" => {
                let params: SaveParams = parse(params)?;
                let report = self.open_mut()?.save(params.all)?;
                self.project_event("saved");
                Ok(json!({
                    "written": report.written,
                    "deleted": report.deleted,
                    "history": self.open()?.history_state(),
                }))
            }
            "project.saveAs" => {
                let params: PathParams = parse(params)?;
                self.open_mut()?.save_as(&params.path)?;
                self.project_event("saved");
                self.snapshot()
            }
            "project.saveCopy" => {
                let params: PathParams = parse(params)?;
                let manifest = self.open()?.save_copy(&params.path)?;
                Ok(json!({"manifestPath": manifest.to_string_lossy()}))
            }
            "project.archive" => {
                let params: PathParams = parse(params)?;
                let session = self.open()?;
                let report = archive::archive_project(
                    session.root(),
                    session.manifest_file(),
                    session.project(),
                    &session.generated_dirs(),
                    &params.path,
                )?;
                to_value(&report)
            }
            "project.setProperties" => {
                let props = params
                    .as_object()
                    .cloned()
                    .ok_or_else(|| EngineError::invalid_params("params must be an object"))?;
                let change_set = self.commit(
                    labels::SET_PROJECT_PROPERTIES,
                    vec![Request::SetProject { props }],
                )?;
                to_value(&change_set.history)
            }
            "project.files" => {
                let session = self.open()?;
                to_value(&store::list_files(
                    session.root(),
                    session.manifest_file(),
                    session.project(),
                    &session.generated_dirs(),
                )?)
            }
            "project.validate" => {
                let params: ValidateParams = parse(params)?;
                let report = match params.path {
                    Some(path) => validate::validate_dir(&path)?,
                    None => validate::validate_project(self.open()?.project()),
                };
                to_value(&report)
            }
            "project.resolveText" => {
                let params: ResolveParams = parse(params)?;
                let text = self.open()?.resolve_text(&params.text, params.doc);
                Ok(json!({"text": text}))
            }
            other => Err(EngineError::new(
                ErrorCode::MethodNotFound,
                format!("unknown method `{other}`"),
            )),
        }
    }

    fn call_document(&mut self, method: &str, params: &Value) -> Result<Value, EngineError> {
        match method {
            "document.add" => {
                let params: AddDocumentParams = parse(params)?;
                let (requests, id, label) = self.add_document_requests(&params)?;
                self.commit(label, requests)?;
                let path = self
                    .open()?
                    .project()
                    .reference(id)
                    .map(|entry| entry.path.clone());
                Ok(json!({"id": id, "path": path}))
            }
            "document.remove" => {
                let params: DocParams = parse(params)?;
                let change_set = self.commit(
                    labels::REMOVE_DOCUMENT,
                    vec![Request::RemoveDocument { doc: params.doc }],
                )?;
                to_value(&change_set.history)
            }
            "document.rename" => {
                let params: RenameParams = parse(params)?;
                let mut props = Map::new();
                props.insert(
                    "name".to_owned(),
                    Value::String(params.name.trim().to_owned()),
                );
                let change_set = self.commit(
                    labels::RENAME_DOCUMENT,
                    vec![Request::SetProps {
                        doc: params.doc,
                        props,
                    }],
                )?;
                to_value(&change_set.history)
            }
            "document.setProperties" => {
                let params: SetPropsParams = parse(params)?;
                let change_set = self.commit(
                    labels::SET_DOCUMENT_PROPERTIES,
                    vec![Request::SetProps {
                        doc: params.doc,
                        props: params.props,
                    }],
                )?;
                to_value(&change_set.history)
            }
            "document.get" | "document.properties" => {
                let params: DocParams = parse(params)?;
                let session = self.open()?;
                let document = session.project().document(params.doc).ok_or_else(|| {
                    EngineError::not_found(format!("no document with id {}", params.doc))
                })?;
                let path = session
                    .project()
                    .reference(params.doc)
                    .map(|entry| entry.path.clone());
                let mut value = if method == "document.get" {
                    to_value(document)?
                } else {
                    let mut props = document.props().to_json().map_err(EngineError::internal)?;
                    if let Some(object) = props.as_object_mut() {
                        object.insert("docKind".to_owned(), to_value(&document.kind())?);
                    }
                    props
                };
                if let Some(object) = value.as_object_mut() {
                    object.insert("path".to_owned(), to_value(&path)?);
                }
                Ok(value)
            }
            other => Err(EngineError::new(
                ErrorCode::MethodNotFound,
                format!("unknown method `{other}`"),
            )),
        }
    }

    fn add_document_requests(
        &self,
        params: &AddDocumentParams,
    ) -> Result<(Vec<Request>, Id, &'static str), EngineError> {
        let session = self.open()?;
        let project = session.project();
        let id = Id::random();
        let name = params.name.trim().to_owned();
        let mut requests = vec![Request::AddDocument {
            kind: params.kind,
            name: name.clone(),
            path: None,
            id: Some(id),
        }];
        let mut label = labels::ADD_DOCUMENT;
        if let Some(parent) = params.parent {
            let Some(Document::Schematic(sheet)) = project.document(parent) else {
                return Err(EngineError::invalid_params(
                    "the parent must be a schematic",
                ));
            };
            if params.kind != DocumentKind::Schematic {
                return Err(EngineError::invalid_params(
                    "only schematics can be placed as sheets",
                ));
            }
            let index = i64::try_from(sheet.sheets.len()).unwrap_or(0);
            let page = sheet.page.dimensions();
            let symbol = SheetSymbol {
                id: Id::random(),
                name: name.clone(),
                schematic: id,
                at: params.at.unwrap_or_else(|| {
                    Point::new(
                        mm(25) + (index % 6) * mm(40),
                        page.h - mm(40) - (index / 6) * mm(30),
                    )
                }),
                size: params
                    .size
                    .filter(|size| size.w > 0 && size.h > 0)
                    .unwrap_or(DEFAULT_SHEET_SYMBOL_SIZE),
                pins: Vec::new(),
            };
            requests.push(Request::Insert {
                doc: parent,
                object: Object::Sheet(symbol),
            });
            label = labels::ADD_SHEET;
        }
        if params.kind == DocumentKind::Schematic && project.manifest.root_schematic.is_none() {
            let mut props = Map::new();
            props.insert("rootSchematic".to_owned(), Value::String(id.to_string()));
            requests.push(Request::SetProject { props });
        }
        Ok((requests, id, label))
    }

    /// Builds the preview scene of a library symbol or footprint.
    fn library_preview(&mut self, params: &PreviewParams) -> Result<(Value, Vec<u8>), EngineError> {
        let (kind, definition, _) = self.library.definition(params.id)?;
        let resolve = |text: &str| text.to_owned();
        let options = ew_scene::Options {
            resolve: &resolve,
            labels: params.labels.clone().unwrap_or_default(),
        };
        let invalid = |error: serde_json::Error| EngineError::internal(error);
        let scene = match kind {
            ew_library::Kind::Symbol => {
                let def: ew_model::library::SymbolDef =
                    serde_json::from_value(definition).map_err(invalid)?;
                ew_scene::symbol_preview(&def, params.unit.unwrap_or(1), &options)
            }
            ew_library::Kind::Footprint => {
                let def: ew_model::library::FootprintDef =
                    serde_json::from_value(definition).map_err(invalid)?;
                ew_scene::footprint_preview(&def, &options)
            }
            ew_library::Kind::Model => {
                return Err(EngineError::invalid_params("3D models have no 2D preview"));
            }
        };
        Ok((to_value(&scene.meta)?, scene.data))
    }

    /// Copies library symbols and footprints into a project library.
    fn library_add(&mut self, params: &AddParams) -> Result<Value, EngineError> {
        let mut imported = ew_kicad::library::Library::default();
        let mut footprints = Vec::new();
        for id in &params.ids {
            let (kind, definition, _) = self.library.definition(*id)?;
            let invalid = |error: serde_json::Error| EngineError::internal(error);
            match kind {
                ew_library::Kind::Symbol => {
                    let def: ew_model::library::SymbolDef =
                        serde_json::from_value(definition).map_err(invalid)?;
                    if params.footprints {
                        footprints.extend(def.footprint.clone());
                    }
                    imported.symbols.push(def);
                }
                ew_library::Kind::Footprint => imported
                    .footprints
                    .push(serde_json::from_value(definition).map_err(invalid)?),
                ew_library::Kind::Model => {}
            }
        }
        // The default footprints of the symbols, if the library has them.
        for name in footprints {
            if imported.footprints.iter().any(|fp| fp.name == name) {
                continue;
            }
            if let Some(id) = self.library.find(ew_library::Kind::Footprint, &name)? {
                let (_, definition, _) = self.library.definition(id)?;
                if let Ok(def) = serde_json::from_value(definition) {
                    imported.footprints.push(def);
                }
            }
        }
        let session = self.open()?;
        let project = session.project();
        let doc = match params.doc {
            Some(doc) => doc,
            None => project
                .documents_of(DocumentKind::Library)
                .next()
                .map(|entry| entry.id)
                .ok_or_else(|| EngineError::not_found("the project has no library"))?,
        };
        if !matches!(project.document(doc), Some(Document::Library(_))) {
            return Err(EngineError::invalid_params("the target must be a library"));
        }
        let mut report = ew_kicad::ImportReport::new("library");
        let (symbols, footprints, requests) =
            import::library_changes(project, doc, imported, &mut report);
        let skipped: usize = report
            .findings
            .iter()
            .filter(|f| f.code == "already-in-library")
            .map(|f| f.count)
            .sum();
        let history = if requests.is_empty() {
            self.open()?.history_state()
        } else {
            self.commit(labels::ADD_FROM_LIBRARY, requests)?.history
        };
        Ok(json!({
            "symbols": symbols,
            "footprints": footprints,
            "skipped": skipped,
            "history": to_value(&history)?,
        }))
    }

    fn call_import(&mut self, method: &str, params: &Value) -> Result<Value, EngineError> {
        match method {
            "import.kicadProbe" => {
                let params: SourceParams = parse(params)?;
                to_value(&import::probe(&params.source)?)
            }
            "import.kicad" => {
                self.ensure_closed()?;
                let params: ImportKicadParams = parse(params)?;
                let mut imported = ew_kicad::import_project(&params.source, params.name.as_deref())
                    .map_err(|error| import::engine_error(&error))?;
                if let Some(names) = &params.names {
                    templates::name_imported_documents(&mut imported.project, names);
                }
                crate::change::check_name(&imported.project.manifest.name)?;
                let session = Session::create(&params.path, &imported.project, self.journaling)?;
                let markdown = imported
                    .report
                    .to_markdown(&format!("KiCad import: {}", imported.project.manifest.name));
                // The report is a plain file in the project folder; failing to
                // write it does not fail the import.
                let _ = std::fs::write(session.root().join(import::REPORT_FILE), markdown);
                self.session = Some(session);
                self.project_event("opened");
                Ok(json!({
                    "project": self.snapshot()?,
                    "report": to_value(&imported.report)?,
                    "reportFile": import::REPORT_FILE,
                }))
            }
            "import.library" => {
                let params: ImportLibraryParams = parse(params)?;
                let session = self.open()?;
                let project = session.project();
                let doc = match params.doc {
                    Some(doc) => doc,
                    None => project
                        .documents_of(DocumentKind::Library)
                        .next()
                        .map(|entry| entry.id)
                        .ok_or_else(|| EngineError::not_found("the project has no library"))?,
                };
                if !matches!(project.document(doc), Some(Document::Library(_))) {
                    return Err(EngineError::invalid_params("the target must be a library"));
                }
                let mut report = ew_kicad::ImportReport::new(params.source.display().to_string());
                let imported = ew_kicad::library::import(
                    &params.source,
                    params.nickname.as_deref(),
                    &mut report,
                )
                .map_err(|error| import::engine_error(&error))?;
                let (symbols, footprints, requests) =
                    import::library_changes(project, doc, imported, &mut report);
                report.finish();
                let history = if requests.is_empty() {
                    self.open()?.history_state()
                } else {
                    self.commit(labels::IMPORT_LIBRARY, requests)?.history
                };
                Ok(json!({
                    "symbols": symbols,
                    "footprints": footprints,
                    "report": to_value(&report)?,
                    "history": to_value(&history)?,
                }))
            }
            "import.netlist" => {
                let params: ImportNetlistParams = parse(params)?;
                let (list, format) = import::read_netlist(&params.source)?;
                let session = self.open()?;
                let pcb = import::board(session.project(), params.doc)?;
                let (mut result, requests) = import::netlist_changes(pcb, &list, format);
                if params.apply && !requests.is_empty() {
                    self.commit(labels::IMPORT_NETLIST, requests)?;
                    result.applied = true;
                }
                to_value(&result)
            }
            other => Err(EngineError::new(
                ErrorCode::MethodNotFound,
                format!("unknown method `{other}`"),
            )),
        }
    }

    fn call_libedit(&mut self, method: &str, params: &Value) -> Result<Value, EngineError> {
        match crate::libedit::call(self.open()?.project(), method, params)? {
            crate::libedit::Outcome::Read(value) => Ok(value),
            crate::libedit::Outcome::Commit {
                label,
                requests,
                reply,
            } => {
                let change_set = self.commit(label, requests)?;
                let mut out = json!({"seq": change_set.seq, "history": change_set.history});
                if let (Some(out), Value::Object(extra)) = (out.as_object_mut(), reply) {
                    out.extend(extra);
                }
                Ok(out)
            }
        }
    }

    fn call_schematic(&mut self, method: &str, params: &Value) -> Result<Value, EngineError> {
        let outcome = crate::schematic::call(self.open()?.project(), method, params)?;
        match outcome {
            crate::schematic::Outcome::Read(value) => Ok(value),
            crate::schematic::Outcome::Requests { label, requests } => {
                let change_set = self.commit(label, requests)?;
                Ok(
                    json!({"seq": change_set.seq, "select": [], "changes": 1, "history": change_set.history}),
                )
            }
            crate::schematic::Outcome::Edit { label, edit } => {
                if edit.changes.is_empty() {
                    let history = self.open()?.history_state();
                    return Ok(json!({
                        "seq": null,
                        "select": edit.select,
                        "changes": 0,
                        "history": history,
                    }));
                }
                let change_set = self.commit(label, crate::schematic::requests(&edit))?;
                Ok(json!({
                    "seq": change_set.seq,
                    "select": edit.select,
                    "changes": edit.changes.len(),
                    "history": change_set.history,
                }))
            }
        }
    }

    fn call_edit(&mut self, method: &str, params: &Value) -> Result<Value, EngineError> {
        match method {
            "edit.apply" => {
                let params: ApplyParams = parse(params)?;
                let requests = params
                    .changes
                    .iter()
                    .map(Request::parse)
                    .collect::<Result<Vec<_>, _>>()?;
                let inserted: Vec<Id> = requests
                    .iter()
                    .filter_map(|request| match request {
                        Request::Insert { object, .. } => Some(object.id()),
                        Request::AddDocument { id, .. } => *id,
                        _ => None,
                    })
                    .collect();
                let label = params.label.unwrap_or_else(|| labels::APPLY.to_owned());
                let change_set = self.commit(&label, requests)?;
                Ok(json!({
                    "seq": change_set.seq,
                    "inserted": inserted,
                    "history": change_set.history,
                }))
            }
            "edit.undo" | "edit.redo" => {
                let session = self.open_mut()?;
                let change_set = if method == "edit.undo" {
                    session.undo()?
                } else {
                    session.redo()?
                };
                let history = self.open()?.history_state();
                match change_set {
                    Some(change_set) => {
                        self.change_event(&change_set)?;
                        Ok(json!({"applied": true, "label": change_set.label, "history": history}))
                    }
                    None => Ok(json!({"applied": false, "history": history})),
                }
            }
            "edit.history" => {
                let session = self.open()?;
                let history = session.history();
                Ok(json!({
                    "state": session.history_state(),
                    "undo": history.undo_labels().collect::<Vec<_>>(),
                    "redo": history.redo_labels().collect::<Vec<_>>(),
                    "bytes": history.bytes(),
                }))
            }
            other => Err(EngineError::new(
                ErrorCode::MethodNotFound,
                format!("unknown method `{other}`"),
            )),
        }
    }
}

/// Cheap check of a project path for the "recent projects" list.
fn probe(path: &Path) -> Value {
    let Ok(manifest_path) = store::find_manifest(path) else {
        return json!({"exists": false});
    };
    let root = manifest_path.parent().unwrap_or(Path::new("."));
    let Ok(text) = std::fs::read_to_string(&manifest_path) else {
        return json!({"exists": false});
    };
    let Ok(decoded) = ew_format::codec::decode_manifest(&manifest_path, &text) else {
        return json!({"exists": true, "valid": false, "path": root.to_string_lossy()});
    };
    let has_recovery =
        crate::journal::read_recovery(root, decoded.value.id, &std::collections::BTreeMap::new())
            .ok()
            .flatten()
            .is_some();
    json!({
        "exists": true,
        "valid": true,
        "name": decoded.value.name,
        "path": root.to_string_lossy(),
        "manifestPath": manifest_path.to_string_lossy(),
        "hasRecovery": has_recovery,
    })
}

/// Executes one request on a fresh engine without journal (stateless use,
/// for example `ewerk call`).
#[must_use]
pub fn dispatch(request_json: &str) -> String {
    Engine::without_journal().dispatch(request_json)
}

fn encode(response: &Response) -> String {
    // Serializing these plain types cannot fail; fall back to a static error
    // instead of panicking so the host process always gets valid JSON.
    serde_json::to_string(response).unwrap_or_else(|_| {
        r#"{"error":{"code":"internal","message":"could not encode response"}}"#.to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(raw: &str) -> Value {
        serde_json::from_str(&dispatch(raw)).unwrap()
    }

    #[test]
    fn info_returns_version() {
        let reply = call(r#"{"method":"engine.info"}"#);
        assert_eq!(reply["result"]["version"], crate::VERSION);
    }

    #[test]
    fn ping_echoes_params() {
        let reply = call(r#"{"method":"engine.ping","params":{"n":42}}"#);
        assert_eq!(reply["result"], json!({"pong": true, "echo": {"n": 42}}));
        let reply = call(r#"{"method":"engine.ping"}"#);
        assert_eq!(reply["result"]["echo"], Value::Null);
    }

    #[test]
    fn methods_lists_every_method_sorted() {
        let reply = call(r#"{"method":"engine.methods"}"#);
        let listed: Vec<String> = serde_json::from_value(reply["result"].clone()).unwrap();
        let mut sorted = listed.clone();
        sorted.sort();
        assert_eq!(listed, sorted);
        assert_eq!(listed, METHODS);
    }

    #[test]
    fn errors_are_reported() {
        let reply = call(r#"{"method":"does.not.exist"}"#);
        assert_eq!(reply["error"]["code"], "methodNotFound");
        assert!(
            reply["error"]["message"]
                .as_str()
                .unwrap()
                .contains("does.not.exist")
        );
        assert_eq!(call("{not json")["error"]["code"], "invalidRequest");
        assert_eq!(
            call(r#"{"method":"engine.info","extra":1}"#)["error"]["code"],
            "invalidRequest"
        );
        assert_eq!(
            call(r#"{"method":"project.info"}"#)["error"]["code"],
            "noProject"
        );
        assert_eq!(
            call(r#"{"method":"project.open","params":{"path":42}}"#)["error"]["code"],
            "invalidParams"
        );
        assert_eq!(
            call(r#"{"method":"project.open","params":{"path":"/definitely/not/here"}}"#)["error"]
                ["code"],
            "notFound"
        );
    }

    #[test]
    fn every_listed_method_is_routed() {
        let mut engine = Engine::without_journal();
        for method in METHODS {
            let reply = engine.handle(&RpcRequest {
                method: (*method).to_owned(),
                params: Value::Null,
            });
            if let Response::Error(error) = reply {
                assert_ne!(
                    error.code,
                    ErrorCode::MethodNotFound,
                    "{method} is not routed"
                );
            }
        }
    }
}
