//! The `libedit.*` methods: the symbol, footprint and component editors of
//! project libraries. The logic lives in `ew-libedit`; this module finds the
//! items, checks names and turns edits into change requests.

use std::collections::BTreeSet;

use ew_libedit::check::{self, Finding, Severity};
use ew_libedit::footprint::FootprintEdit;
use ew_libedit::refs::{self, LibraryKind};
use ew_libedit::symbol::{Body, SymbolEdit};
use ew_libedit::wizard::{self, Wizard};
use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::library::{ComponentDef, FootprintDef, Library, SymbolDef, SymbolUnit};
use ew_model::object::Object;
use ew_model::project::Project;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::change::{Request, merge_patch};
use crate::error::{EngineError, ErrorCode};

/// History labels of the library editors.
pub mod labels {
    /// `libedit.create`.
    pub const CREATE: &str = "create-library-item";
    /// `libedit.delete`.
    pub const DELETE: &str = "delete-library-item";
    /// `libedit.rename`.
    pub const RENAME: &str = "rename-library-item";
    /// `libedit.setProps`.
    pub const PROPS: &str = "edit-library-item";
    /// `libedit.symbol`.
    pub const SYMBOL: &str = "edit-symbol";
    /// `libedit.footprint`.
    pub const FOOTPRINT: &str = "edit-footprint";
}

/// Grid for centring turned symbol items.
const SYMBOL_GRID: i64 = 1_270_000;
/// Grid for centring turned footprint items.
const FOOTPRINT_GRID: i64 = 50_000;

/// What a method does.
pub enum Outcome {
    /// A reply without changes.
    Read(Value),
    /// Requests to commit as one undo step, and the reply to add to the
    /// commit result.
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
    let value = if params.is_null() {
        Value::Object(Map::new())
    } else {
        params.clone()
    };
    serde_json::from_value(value).map_err(|error| EngineError::invalid_params(error.to_string()))
}

fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, EngineError> {
    serde_json::to_value(value).map_err(EngineError::internal)
}

fn library(project: &Project, doc: Id) -> Result<&Library, EngineError> {
    match project.document(doc) {
        Some(Document::Library(library)) => Ok(library),
        Some(_) => Err(EngineError::invalid_params(format!(
            "{doc} is not a library"
        ))),
        None => Err(EngineError::not_found(format!("no document {doc}"))),
    }
}

/// Symbols and footprints of every project library (first one wins).
struct Defs<'a> {
    symbols: std::collections::BTreeMap<&'a str, &'a SymbolDef>,
    footprints: std::collections::BTreeMap<&'a str, &'a FootprintDef>,
}

fn defs(project: &Project) -> Defs<'_> {
    let mut out = Defs {
        symbols: std::collections::BTreeMap::new(),
        footprints: std::collections::BTreeMap::new(),
    };
    for entry in &project.manifest.documents {
        if let Some(Document::Library(library)) = project.document(entry.id) {
            for def in &library.symbols {
                out.symbols.entry(def.name.as_str()).or_insert(def);
            }
            for def in &library.footprints {
                out.footprints.entry(def.name.as_str()).or_insert(def);
            }
        }
    }
    out
}

fn counts(findings: &[Finding]) -> Value {
    let count = |severity: Severity| findings.iter().filter(|f| f.severity == severity).count();
    json!({"errors": count(Severity::Error), "warnings": count(Severity::Warning), "infos": count(Severity::Info)})
}

fn component_findings(def: &ComponentDef, all: &Defs<'_>) -> Vec<Finding> {
    let footprint = def
        .footprint
        .as_deref()
        .and_then(|name| all.footprints.get(name).copied());
    check::check_component(
        def,
        all.symbols.get(def.symbol.as_str()).copied(),
        footprint,
    )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocParams {
    doc: Id,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemParams {
    doc: Id,
    kind: LibraryKind,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateParams {
    doc: Id,
    kind: LibraryKind,
    name: String,
    /// Copy an existing item of this name.
    #[serde(default)]
    copy_of: Option<String>,
    /// A complete definition (from a wizard or an import).
    #[serde(default)]
    def: Option<Value>,
    /// Generate a footprint.
    #[serde(default)]
    wizard: Option<Wizard>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeleteParams {
    doc: Id,
    kind: LibraryKind,
    name: String,
    #[serde(default)]
    force: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RenameParams {
    doc: Id,
    kind: LibraryKind,
    name: String,
    new: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PropsParams {
    doc: Id,
    kind: LibraryKind,
    name: String,
    props: Map<String, Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SymbolParams {
    doc: Id,
    name: String,
    #[serde(default)]
    body: Body,
    edit: SymbolEdit,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FootprintParams {
    doc: Id,
    name: String,
    edit: FootprintEdit,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckParams {
    doc: Id,
    #[serde(default)]
    kind: Option<LibraryKind>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WizardParams {
    wizard: Wizard,
    #[serde(default)]
    name: Option<String>,
}

fn exists(library: &Library, kind: LibraryKind, name: &str) -> bool {
    match kind {
        LibraryKind::Symbol => library.symbol(name).is_some(),
        LibraryKind::Footprint => library.footprint(name).is_some(),
        LibraryKind::Component => library.component(name).is_some(),
    }
}

fn checked_name(library: &Library, kind: LibraryKind, name: &str) -> Result<String, EngineError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(EngineError::invalid_params("the name must not be empty"));
    }
    if exists(library, kind, name) {
        return Err(EngineError::new(
            ErrorCode::Conflict,
            format!("`{name}` already exists in the library"),
        ));
    }
    Ok(name.to_owned())
}

fn not_found(kind: LibraryKind, name: &str) -> EngineError {
    let what = match kind {
        LibraryKind::Symbol => "symbol",
        LibraryKind::Footprint => "footprint",
        LibraryKind::Component => "component",
    };
    EngineError::not_found(format!("no {what} `{name}` in the library"))
}

/// The object of an item.
fn object(library: &Library, kind: LibraryKind, name: &str) -> Result<Object, EngineError> {
    match kind {
        LibraryKind::Symbol => library.symbol(name).cloned().map(Object::SymbolDef),
        LibraryKind::Footprint => library.footprint(name).cloned().map(Object::FootprintDef),
        LibraryKind::Component => library.component(name).cloned().map(Object::ComponentDef),
    }
    .ok_or_else(|| not_found(kind, name))
}

/// A new, empty item.
fn empty(kind: LibraryKind, name: &str) -> Object {
    match kind {
        LibraryKind::Symbol => Object::SymbolDef(SymbolDef {
            id: Id::random(),
            name: name.to_owned(),
            description: String::new(),
            keywords: String::new(),
            prefix: "U".to_owned(),
            value: name.to_owned(),
            footprint: None,
            datasheet: String::new(),
            power: false,
            units: vec![SymbolUnit::default()],
            properties: std::collections::BTreeMap::new(),
            pin_text: ew_model::library::PinText::default(),
        }),
        LibraryKind::Footprint => Object::FootprintDef(FootprintDef {
            id: Id::random(),
            name: name.to_owned(),
            description: String::new(),
            keywords: String::new(),
            pads: Vec::new(),
            graphics: Vec::new(),
            texts: Vec::new(),
            models: Vec::new(),
            attributes: ew_model::pcb::FootprintAttributes::default(),
        }),
        LibraryKind::Component => Object::ComponentDef(ComponentDef {
            id: Id::random(),
            name: name.to_owned(),
            description: String::new(),
            keywords: String::new(),
            symbol: String::new(),
            footprint: None,
            value: name.to_owned(),
            manufacturer: String::new(),
            mpn: String::new(),
            datasheet: String::new(),
            properties: std::collections::BTreeMap::new(),
            suppliers: Vec::new(),
        }),
    }
}

/// Gives an object a new id and name.
fn renamed(object: &Object, name: &str) -> Result<Object, EngineError> {
    let mut value = to_value(object)?;
    if let Some(map) = value.as_object_mut() {
        map.insert("id".to_owned(), Value::String(Id::random().to_string()));
        map.insert("name".to_owned(), Value::String(name.to_owned()));
    }
    serde_json::from_value(value)
        .map_err(|error| EngineError::invalid_params(format!("definition: {error}")))
}

/// The list of a library with summaries and check counts.
fn list(project: &Project, library: &Library) -> Value {
    let all = defs(project);
    let symbols: Vec<Value> = library
        .symbols
        .iter()
        .map(|def| {
            json!({
                "name": def.name,
                "description": def.description,
                "units": def.units.len(),
                "pins": def.units.iter().map(|u| u.pins.len()).sum::<usize>(),
                "alternate": def.has_alternate(),
                "power": def.power,
                "checks": counts(&check::check_symbol(def)),
            })
        })
        .collect();
    let footprints: Vec<Value> = library
        .footprints
        .iter()
        .map(|def| {
            json!({
                "name": def.name,
                "description": def.description,
                "pads": def.pads.len(),
                "models": def.models.len(),
                "checks": counts(&check::check_footprint(def)),
            })
        })
        .collect();
    let components: Vec<Value> = library
        .components
        .iter()
        .map(|def| {
            json!({
                "name": def.name,
                "description": def.description,
                "symbol": def.symbol,
                "footprint": def.footprint,
                "mpn": def.mpn,
                "checks": counts(&component_findings(def, &all)),
            })
        })
        .collect();
    let sort = |mut list: Vec<Value>| {
        list.sort_by(|a, b| {
            ew_schematic::connectivity::natural_cmp(
                a["name"].as_str().unwrap_or_default(),
                b["name"].as_str().unwrap_or_default(),
            )
        });
        list
    };
    json!({
        "name": library.name,
        "symbols": sort(symbols),
        "footprints": sort(footprints),
        "components": sort(components),
    })
}

fn findings(project: &Project, kind: LibraryKind, object: &Object) -> Vec<Finding> {
    match (kind, object) {
        (LibraryKind::Symbol, Object::SymbolDef(def)) => check::check_symbol(def),
        (LibraryKind::Footprint, Object::FootprintDef(def)) => check::check_footprint(def),
        (LibraryKind::Component, Object::ComponentDef(def)) => {
            component_findings(def, &defs(project))
        }
        _ => Vec::new(),
    }
}

/// Runs a `libedit.*` method.
///
/// # Errors
///
/// Fails for unknown methods, bad parameters, missing items and names that
/// are taken.
#[allow(clippy::too_many_lines, reason = "one arm per method")]
pub fn call(project: &Project, method: &str, params: &Value) -> Result<Outcome, EngineError> {
    match method {
        "libedit.list" => {
            let params: DocParams = parse(params)?;
            Ok(Outcome::Read(list(project, library(project, params.doc)?)))
        }
        "libedit.item" => {
            let params: ItemParams = parse(params)?;
            let lib = library(project, params.doc)?;
            let object = object(lib, params.kind, &params.name)?;
            let found = findings(project, params.kind, &object);
            Ok(Outcome::Read(json!({
                "def": object,
                "usage": refs::usage(project, params.kind, &params.name),
                "findings": found,
            })))
        }
        "libedit.create" => {
            let params: CreateParams = parse(params)?;
            let lib = library(project, params.doc)?;
            let name = checked_name(lib, params.kind, &params.name)?;
            let object = if let Some(source) = &params.copy_of {
                renamed(&object(lib, params.kind, source)?, &name)?
            } else if let Some(def) = params.def {
                let mut value = def;
                if let Some(map) = value.as_object_mut() {
                    let tag = match params.kind {
                        LibraryKind::Symbol => "symbolDef",
                        LibraryKind::Footprint => "footprintDef",
                        LibraryKind::Component => "componentDef",
                    };
                    map.insert("type".to_owned(), Value::String(tag.to_owned()));
                    map.entry("id")
                        .or_insert_with(|| Value::String(Id::random().to_string()));
                }
                let parsed: Object = serde_json::from_value(value)
                    .map_err(|error| EngineError::invalid_params(format!("definition: {error}")))?;
                renamed(&parsed, &name)?
            } else if let Some(wizard) = &params.wizard {
                if params.kind != LibraryKind::Footprint {
                    return Err(EngineError::invalid_params("wizards make footprints"));
                }
                Object::FootprintDef(
                    wizard::generate(wizard, &name)
                        .map_err(|e| EngineError::invalid_params(e.to_string()))?,
                )
            } else {
                empty(params.kind, &name)
            };
            Ok(Outcome::Commit {
                label: labels::CREATE,
                requests: vec![Request::Insert {
                    doc: params.doc,
                    object,
                }],
                reply: json!({"name": name}),
            })
        }
        "libedit.delete" => {
            let params: DeleteParams = parse(params)?;
            let lib = library(project, params.doc)?;
            let object = object(lib, params.kind, &params.name)?;
            let usage = refs::usage(project, params.kind, &params.name);
            if !usage.is_unused() && !params.force {
                return Err(EngineError::new(
                    ErrorCode::Conflict,
                    format!("`{}` is still used", params.name),
                ));
            }
            Ok(Outcome::Commit {
                label: labels::DELETE,
                requests: vec![Request::Delete {
                    doc: params.doc,
                    id: object.id(),
                }],
                reply: json!({"usage": usage}),
            })
        }
        "libedit.rename" => {
            let params: RenameParams = parse(params)?;
            let lib = library(project, params.doc)?;
            object(lib, params.kind, &params.name)?;
            let new = params.new.trim();
            if new == params.name {
                return Ok(Outcome::Read(json!({"name": new, "changes": 0})));
            }
            let new = checked_name(lib, params.kind, new)?;
            let changes = refs::rename(project, params.doc, params.kind, &params.name, &new);
            let count = changes.len();
            Ok(Outcome::Commit {
                label: labels::RENAME,
                requests: changes
                    .into_iter()
                    .map(|(doc, object)| Request::Update { doc, object })
                    .collect(),
                reply: json!({"name": new, "changes": count}),
            })
        }
        "libedit.setProps" => {
            let params: PropsParams = parse(params)?;
            let lib = library(project, params.doc)?;
            let object = object(lib, params.kind, &params.name)?;
            // Names go through `libedit.rename`; the drawing through the editors.
            let fixed: BTreeSet<&str> = [
                "id", "type", "name", "units", "pads", "graphics", "texts", "models",
            ]
            .into_iter()
            .collect();
            if let Some(key) = params.props.keys().find(|k| fixed.contains(k.as_str())) {
                return Err(EngineError::invalid_params(format!(
                    "`{key}` cannot be set here"
                )));
            }
            let mut value = to_value(&object)?;
            merge_patch(&mut value, &Value::Object(params.props));
            let object: Object = serde_json::from_value(value)
                .map_err(|error| EngineError::invalid_params(format!("properties: {error}")))?;
            Ok(Outcome::Commit {
                label: labels::PROPS,
                requests: vec![Request::Update {
                    doc: params.doc,
                    object,
                }],
                reply: Value::Null,
            })
        }
        "libedit.symbol" => {
            let params: SymbolParams = parse(params)?;
            let lib = library(project, params.doc)?;
            let def = lib
                .symbol(&params.name)
                .ok_or_else(|| not_found(LibraryKind::Symbol, &params.name))?;
            let edited = ew_libedit::symbol::apply(def, params.body, &params.edit, SYMBOL_GRID)
                .map_err(|e| EngineError::invalid_params(e.to_string()))?;
            Ok(Outcome::Commit {
                label: labels::SYMBOL,
                requests: vec![Request::Update {
                    doc: params.doc,
                    object: Object::SymbolDef(edited.def),
                }],
                reply: json!({"body": {"unit": edited.body.unit, "alternate": edited.body.alternate}, "select": edited.select}),
            })
        }
        "libedit.footprint" => {
            let params: FootprintParams = parse(params)?;
            let lib = library(project, params.doc)?;
            let def = lib
                .footprint(&params.name)
                .ok_or_else(|| not_found(LibraryKind::Footprint, &params.name))?;
            let edited = ew_libedit::footprint::apply(def, &params.edit, FOOTPRINT_GRID)
                .map_err(|e| EngineError::invalid_params(e.to_string()))?;
            Ok(Outcome::Commit {
                label: labels::FOOTPRINT,
                requests: vec![Request::Update {
                    doc: params.doc,
                    object: Object::FootprintDef(edited.def),
                }],
                reply: json!({"select": edited.select}),
            })
        }
        "libedit.check" => {
            let params: CheckParams = parse(params)?;
            let lib = library(project, params.doc)?;
            let mut out: Vec<Value> = Vec::new();
            let mut add = |kind: LibraryKind, name: &str, object: Object| {
                let found = findings(project, kind, &object);
                if !found.is_empty() {
                    out.push(json!({"kind": kind, "name": name, "findings": found}));
                }
            };
            for def in &lib.symbols {
                if params.kind.is_none_or(|k| k == LibraryKind::Symbol)
                    && params.name.as_deref().is_none_or(|n| n == def.name)
                {
                    add(
                        LibraryKind::Symbol,
                        &def.name,
                        Object::SymbolDef(def.clone()),
                    );
                }
            }
            for def in &lib.footprints {
                if params.kind.is_none_or(|k| k == LibraryKind::Footprint)
                    && params.name.as_deref().is_none_or(|n| n == def.name)
                {
                    add(
                        LibraryKind::Footprint,
                        &def.name,
                        Object::FootprintDef(def.clone()),
                    );
                }
            }
            for def in &lib.components {
                if params.kind.is_none_or(|k| k == LibraryKind::Component)
                    && params.name.as_deref().is_none_or(|n| n == def.name)
                {
                    add(
                        LibraryKind::Component,
                        &def.name,
                        Object::ComponentDef(def.clone()),
                    );
                }
            }
            Ok(Outcome::Read(Value::Array(out)))
        }
        "libedit.wizard" => {
            let params: WizardParams = parse(params)?;
            let name = params
                .name
                .unwrap_or_else(|| wizard::suggest_name(&params.wizard));
            let def = wizard::generate(&params.wizard, &name)
                .map_err(|e| EngineError::invalid_params(e.to_string()))?;
            let found = check::check_footprint(&def);
            Ok(Outcome::Read(
                json!({"name": name, "def": def, "findings": found}),
            ))
        }
        "libedit.presets" => Ok(Outcome::Read(to_value(&wizard::presets())?)),
        other => Err(EngineError::new(
            ErrorCode::MethodNotFound,
            format!("unknown method `{other}`"),
        )),
    }
}
