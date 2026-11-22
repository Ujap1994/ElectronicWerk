//! "Select similar": the properties an object can be compared by, and the
//! objects of the same type that share the chosen ones (Altium's "Find
//! similar objects").

use ew_model::document::Document;
use ew_model::id::Id;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::error::EngineError;

/// Fields that identify or place an object; comparing them makes no sense.
const SKIPPED: &[&str] = &[
    "id",
    "designator",
    "component",
    "uuid",
    "at",
    "start",
    "end",
    "mid",
];

/// Lengths in nanometres (the UI shows them in its units).
const LENGTHS: &[&str] = &[
    "width",
    "diameter",
    "drill",
    "clearance",
    "size",
    "thickness",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SimilarParams {
    pub doc: Id,
    pub id: Id,
    /// The fields to compare; none: list the fields that can be compared.
    #[serde(default)]
    pub keys: Option<Vec<String>>,
}

/// The comparable top-level fields of an object: scalars only.
fn fields(object: &Value) -> Map<String, Value> {
    object
        .as_object()
        .map(|map| {
            map.iter()
                .filter(|(key, value)| {
                    !SKIPPED.contains(&key.as_str())
                        && (value.is_string() || value.is_number() || value.is_boolean())
                })
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn object_json(document: &Document, id: Id) -> Option<Value> {
    serde_json::to_value(document.object(id)?)
        .ok()
        .map(|value| {
            // `Object` is tagged: `{"type": …, …fields}` or `{"Track": {…}}`.
            match value {
                Value::Object(map) if map.len() == 1 && map.values().all(Value::is_object) => map
                    .into_iter()
                    .next()
                    .map_or(Value::Null, |(_, inner)| inner),
                other => other,
            }
        })
}

/// Lists the comparable fields, or finds the objects that match.
pub(crate) fn similar(document: &Document, params: &SimilarParams) -> Result<Value, EngineError> {
    let kind = document
        .object_type(params.id)
        .ok_or_else(|| EngineError::not_found(format!("no object with id {}", params.id)))?;
    let reference = object_json(document, params.id)
        .ok_or_else(|| EngineError::not_found(format!("no object with id {}", params.id)))?;
    let own = fields(&reference);
    let Some(keys) = &params.keys else {
        let net_name = |id: &Value| -> Option<String> {
            let Document::Pcb(pcb) = document else {
                return None;
            };
            let id: Id = serde_json::from_value(id.clone()).ok()?;
            pcb.nets.get(id).map(|net| net.name.clone())
        };
        let list: Vec<Value> = own
            .iter()
            .filter(|(key, _)| key.as_str() != "type")
            .map(|(key, value)| {
                let mut entry = json!({"key": key, "value": value});
                if LENGTHS.contains(&key.as_str()) {
                    entry["length"] = json!(true);
                }
                if key == "net" {
                    if let Some(name) = net_name(value) {
                        entry["text"] = json!(name);
                    }
                }
                entry
            })
            .collect();
        return Ok(json!({"type": kind, "fields": list}));
    };
    let wanted: Vec<(&String, Option<&Value>)> = keys.iter().map(|k| (k, own.get(k))).collect();
    let mut ids = Vec::new();
    for id in document.ids_of(kind) {
        let Some(object) = object_json(document, id) else {
            continue;
        };
        let theirs = fields(&object);
        if wanted.iter().all(|(key, value)| theirs.get(*key) == *value) {
            ids.push(id);
        }
    }
    Ok(json!({"type": kind, "ids": ids}))
}
