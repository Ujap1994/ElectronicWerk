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
use crate::info::EngineInfo;
use crate::session::{ChangeSet, Session};
use crate::summary;
use crate::templates::{self, BoardOptions, NewProject};

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
    /// `edit.apply` without a label.
    pub const APPLY: &str = "apply";
}

/// The engine: holds the open project and serves requests.
#[derive(Debug)]
pub struct Engine {
    session: Option<Session>,
    events: Vec<Value>,
    journaling: bool,
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
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AddDocumentParams {
    kind: DocumentKind,
    name: String,
    #[serde(default)]
    parent: Option<Id>,
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
#[serde(deny_unknown_fields)]
struct ResolveParams {
    text: String,
    #[serde(default)]
    doc: Option<Id>,
}

impl Engine {
    /// An engine with the autosave journal enabled.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            session: None,
            events: Vec::new(),
            journaling: true,
        }
    }

    /// An engine without autosave journal (one-shot command-line calls).
    #[must_use]
    pub const fn without_journal() -> Self {
        Self {
            session: None,
            events: Vec::new(),
            journaling: false,
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
                at: Point::new(
                    mm(25) + (index % 6) * mm(40),
                    page.h - mm(40) - (index / 6) * mm(30),
                ),
                size: DEFAULT_SHEET_SYMBOL_SIZE,
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
