//! The assembly assistant: in which order to place the parts of a board by
//! hand, and how far a build is. The progress is saved at once in
//! `assembly.json` in the project folder (not in the documents: ticking off
//! a part is no design change and needs no undo or saving).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::layer::Side;
use ew_model::pcb::{FootprintInstance, Mounting, Pcb};
use ew_model::project::Project;
use ew_output::bom::Part;
use ew_schematic::connectivity::natural_cmp;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::error::EngineError;

/// The progress file in the project folder.
pub const FILE: &str = "assembly.json";
const FORMAT: &str = "electronicwerk.assembly";

/// Methods of this module (for `engine.methods`).
pub const METHODS: &[&str] = &["assembly.mark", "assembly.plan", "assembly.reset"];

/// The progress of one board.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardProgress {
    /// Designators placed in the current build.
    #[serde(default)]
    pub placed: BTreeSet<String>,
    /// Builds finished before the current one.
    #[serde(default)]
    pub completed: u32,
}

/// The content of `assembly.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressFile {
    /// `electronicwerk.assembly`.
    pub format: String,
    /// Version of this file's schema.
    pub schema_version: u32,
    /// By board document id.
    #[serde(default)]
    pub boards: BTreeMap<Id, BoardProgress>,
}

impl Default for ProgressFile {
    fn default() -> Self {
        Self {
            format: FORMAT.to_owned(),
            schema_version: 1,
            boards: BTreeMap::new(),
        }
    }
}

/// Reads the progress (empty if there is none or it cannot be read).
#[must_use]
pub fn read(root: &Path) -> ProgressFile {
    std::fs::read_to_string(root.join(FILE))
        .ok()
        .and_then(|text| serde_json::from_str::<ProgressFile>(&text).ok())
        .filter(|file| file.format == FORMAT)
        .unwrap_or_default()
}

fn write(root: &Path, file: &ProgressFile) -> Result<(), EngineError> {
    let mut text = serde_json::to_string_pretty(file).map_err(EngineError::internal)?;
    text.push('\n');
    ew_format::fsio::write_atomic(&root.join(FILE), text.as_bytes())?;
    Ok(())
}

/// The order of the groups on each side.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Order {
    /// Small parts first (they are hard to reach between tall ones).
    #[default]
    Size,
    /// By value.
    Value,
    /// The most parts first.
    Count,
    /// By the first designator.
    Designator,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PlanParams {
    #[serde(default)]
    board: Option<String>,
    #[serde(default)]
    order: Order,
    /// The side assembled first.
    #[serde(default)]
    first_side: Side,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MarkParams {
    #[serde(default)]
    board: Option<String>,
    designators: Vec<String>,
    placed: bool,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ResetParams {
    #[serde(default)]
    board: Option<String>,
    /// The build is finished: count it before starting the next.
    #[serde(default)]
    completed: bool,
}

fn parse<T: serde::de::DeserializeOwned + Default>(params: &Value) -> Result<T, EngineError> {
    if params.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(params.clone()).map_err(|e| EngineError::invalid_params(e.to_string()))
}

fn board<'a>(project: &'a Project, wanted: Option<&str>) -> Result<(Id, &'a Pcb), EngineError> {
    let mut boards = project.documents_of(DocumentKind::Pcb).filter_map(|entry| {
        match project.document(entry.id) {
            Some(Document::Pcb(pcb)) => Some((entry.id, pcb)),
            _ => None,
        }
    });
    match wanted {
        None => boards
            .next()
            .ok_or_else(|| EngineError::not_found("the project has no board")),
        Some(wanted) => boards
            .find(|(id, pcb)| pcb.name == wanted || id.to_string() == wanted)
            .ok_or_else(|| EngineError::not_found(format!("no board `{wanted}`"))),
    }
}

const fn side_name(side: Side) -> &'static str {
    match side {
        Side::Top => "top",
        Side::Bottom => "bottom",
    }
}

const fn mounting_name(mounting: Mounting) -> &'static str {
    match mounting {
        Mounting::Smd => "smd",
        Mounting::ThroughHole => "throughHole",
        Mounting::Other => "other",
    }
}

const fn mounting_rank(mounting: Mounting) -> u8 {
    match mounting {
        Mounting::Smd => 0,
        Mounting::ThroughHole => 1,
        Mounting::Other => 2,
    }
}

/// A group of parts placed together.
struct Group<'a> {
    side: Side,
    mounting: Mounting,
    part: &'a Part,
    footprints: Vec<&'a FootprintInstance>,
    /// Box area of the footprint in mm².
    area: f64,
}

fn property<'a>(part: &'a Part, keys: &[&str]) -> &'a str {
    keys.iter()
        .find_map(|key| {
            part.properties
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(key))
                .map(|(_, value)| value.as_str())
        })
        .unwrap_or("")
}

#[allow(clippy::too_many_lines, reason = "the plan in one pass")]
fn plan(project: &Project, root: &Path, params: &PlanParams) -> Result<Value, EngineError> {
    let (doc, pcb) = board(project, params.board.as_deref())?;
    let parts = ew_output::bom::parts(project, Some(pcb));
    let by_designator: BTreeMap<&str, &FootprintInstance> = pcb
        .footprints
        .iter()
        .map(|fp| (fp.designator.as_str(), fp))
        .collect();
    let mut groups: BTreeMap<(u8, u8, String, String, String), Group<'_>> = BTreeMap::new();
    let mut dnp = Vec::new();
    let mut missing = Vec::new();
    for part in &parts {
        if part.dnp {
            dnp.push(part.designator.clone());
            continue;
        }
        let Some(fp) = by_designator.get(part.designator.as_str()) else {
            missing.push(part.designator.clone());
            continue;
        };
        let side = fp.side;
        let mounting = fp.attributes.mounting;
        let side_rank = u8::from(side != params.first_side);
        let key = (
            side_rank,
            mounting_rank(mounting),
            part.value.clone(),
            part.footprint.clone(),
            property(part, &["MPN", "Manufacturer Part Number"]).to_owned(),
        );
        let bbox = ew_pcb::edit::footprint_box(fp);
        let area = ((bbox.max.x - bbox.min.x) / 1e6) * ((bbox.max.y - bbox.min.y) / 1e6);
        let group = groups.entry(key).or_insert_with(|| Group {
            side,
            mounting,
            part,
            footprints: Vec::new(),
            area,
        });
        group.footprints.push(fp);
    }
    let progress = read(root).boards.get(&doc).cloned().unwrap_or_default();
    let mut list: Vec<Group<'_>> = groups.into_values().collect();
    list.sort_by(|a, b| {
        // SMD parts on both sides first, then through-hole parts (they
        // keep the board from lying flat), each starting with the first side.
        let first = mounting_rank(a.mounting)
            .cmp(&mounting_rank(b.mounting))
            .then(
                u8::from(a.side != params.first_side).cmp(&u8::from(b.side != params.first_side)),
            );
        let designator = |g: &Group<'_>| g.footprints[0].designator.clone();
        first.then_with(|| match params.order {
            Order::Size => a
                .area
                .total_cmp(&b.area)
                .then_with(|| natural_cmp(&a.part.value, &b.part.value)),
            Order::Value => natural_cmp(&a.part.value, &b.part.value)
                .then_with(|| natural_cmp(&a.part.footprint, &b.part.footprint)),
            Order::Count => b.footprints.len().cmp(&a.footprints.len()),
            Order::Designator => natural_cmp(&designator(a), &designator(b)),
        })
    });
    let mut total = 0;
    let mut done = 0;
    let groups: Vec<Value> = list
        .iter_mut()
        .enumerate()
        .map(|(index, group)| {
            group
                .footprints
                .sort_by(|a, b| natural_cmp(&a.designator, &b.designator));
            let designators: Vec<&str> = group
                .footprints
                .iter()
                .map(|fp| fp.designator.as_str())
                .collect();
            let placed = designators
                .iter()
                .filter(|d| progress.placed.contains(**d))
                .count();
            total += designators.len();
            done += placed;
            json!({
                "index": index,
                "side": side_name(group.side),
                "mounting": mounting_name(group.mounting),
                "value": group.part.value,
                "footprint": group.part.footprint,
                "description": group.part.description,
                "mpn": property(group.part, &["MPN", "Manufacturer Part Number"]),
                "manufacturer": property(group.part, &["Manufacturer"]),
                "designators": designators,
                "ids": group.footprints.iter().map(|fp| fp.id).collect::<Vec<_>>(),
                "count": designators.len(),
                "placed": placed,
                "area": (group.area * 100.0).round() / 100.0,
            })
        })
        .collect();
    Ok(json!({
        "board": pcb.name,
        "boardId": doc,
        "groups": groups,
        "dnp": dnp,
        "missing": missing,
        "placed": progress.placed,
        "total": total,
        "done": done,
        "completed": progress.completed,
    }))
}

/// What a call returns, and the event it raises (progress changes).
pub struct Reply {
    /// The method's result.
    pub result: Value,
    /// An `assembly` event for every window.
    pub event: Option<Value>,
}

/// Executes an `assembly.*` method.
///
/// # Errors
///
/// Fails for unknown boards or parts, or if the progress cannot be saved.
pub fn call(
    project: &Project,
    root: &Path,
    method: &str,
    params: &Value,
) -> Result<Reply, EngineError> {
    match method {
        "assembly.plan" => Ok(Reply {
            result: plan(project, root, &parse(params)?)?,
            event: None,
        }),
        "assembly.mark" => {
            let params: MarkParams = serde_json::from_value(params.clone())
                .map_err(|e| EngineError::invalid_params(e.to_string()))?;
            let (doc, pcb) = board(project, params.board.as_deref())?;
            for designator in &params.designators {
                if !pcb.footprints.iter().any(|fp| &fp.designator == designator) {
                    return Err(EngineError::not_found(format!(
                        "no footprint {designator} on the board"
                    )));
                }
            }
            let mut file = read(root);
            let progress = file.boards.entry(doc).or_default();
            for designator in params.designators {
                if params.placed {
                    progress.placed.insert(designator);
                } else {
                    progress.placed.remove(&designator);
                }
            }
            let progress = progress.clone();
            write(root, &file)?;
            Ok(changed(doc, &progress))
        }
        "assembly.reset" => {
            let params: ResetParams = parse(params)?;
            let (doc, _) = board(project, params.board.as_deref())?;
            let mut file = read(root);
            let progress = file.boards.entry(doc).or_default();
            progress.placed.clear();
            if params.completed {
                progress.completed += 1;
            }
            let progress = progress.clone();
            write(root, &file)?;
            Ok(changed(doc, &progress))
        }
        other => Err(EngineError::new(
            crate::error::ErrorCode::MethodNotFound,
            format!("unknown method `{other}`"),
        )),
    }
}

fn changed(doc: Id, progress: &BoardProgress) -> Reply {
    let value = json!({
        "board": doc,
        "placed": progress.placed,
        "completed": progress.completed,
    });
    let mut event = value.clone();
    if let Some(map) = event.as_object_mut() {
        map.insert("type".to_owned(), json!("assembly"));
    }
    Reply {
        result: value,
        event: Some(event),
    }
}
