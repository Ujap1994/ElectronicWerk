//! The `datasheet.*` and `part.*` methods (M16, ADR-0017): datasheets are
//! read and recognised by `ew-datasheet`, parts are generated from a
//! draft (footprint wizard, symbol, component) with a parametric 3D model
//! from `ew-board3d`, checked against the libraries for duplicates and
//! stored in a project library as one undo step.

#![allow(
    clippy::cast_precision_loss,
    reason = "lengths are limited to ±10 m by the data model"
)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use ew_board3d::package::{PackageSpec, PadPlace, Terminals};
use ew_datasheet::draft::{Dimension, Draft, Family};
use ew_datasheet::part::{Generated, PartOptions, generate};
use ew_datasheet::{Datasheet, PdfError, read};
use ew_libedit::check::{check_component, check_footprint, check_symbol};
use ew_libedit::wizard::Wizard;
use ew_library::{Kind, Query};
use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::library::{FootprintDef, Library};
use ew_model::object::Object;
use ew_model::pcb::Model3d;
use ew_model::project::Project;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::change::Request;
use crate::error::{EngineError, ErrorCode};
use crate::library::LibraryState;

/// History label of `part.generate`.
pub const GENERATE: &str = "generate-part";

/// Datasheets read recently (the app asks for pages one by one).
#[derive(Debug, Default)]
pub struct Datasheets {
    cache: Vec<Cached>,
}

#[derive(Debug)]
struct Cached {
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    sheet: Arc<Datasheet>,
}

const CACHED: usize = 3;

impl Datasheets {
    /// The read datasheet of a file.
    ///
    /// # Errors
    ///
    /// The file cannot be read or is not a readable PDF file.
    pub fn load(&mut self, path: &Path) -> Result<Arc<Datasheet>, EngineError> {
        let meta = std::fs::metadata(path)
            .map_err(|e| EngineError::not_found(format!("{}: {e}", path.display())))?;
        let modified = meta.modified().ok();
        if let Some(c) = self
            .cache
            .iter()
            .find(|c| c.path == path && c.size == meta.len() && c.modified == modified)
        {
            return Ok(Arc::clone(&c.sheet));
        }
        let data = std::fs::read(path)
            .map_err(|e| EngineError::not_found(format!("{}: {e}", path.display())))?;
        let sheet = Arc::new(read(data).map_err(|e| match e {
            PdfError::Crypt(_) => EngineError::new(
                ErrorCode::InvalidParams,
                "the datasheet is protected with a password",
            ),
            other => EngineError::invalid_params(format!("{}: {other}", path.display())),
        })?);
        self.cache.retain(|c| c.path != path);
        self.cache.insert(
            0,
            Cached {
                path: path.to_owned(),
                size: meta.len(),
                modified,
                sheet: Arc::clone(&sheet),
            },
        );
        self.cache.truncate(CACHED);
        Ok(sheet)
    }
}

/// What a method does.
pub enum Outcome {
    /// A reply without changes.
    Read(Value),
    /// Files to write, then requests to commit as one undo step.
    Commit {
        /// Files (path, content).
        files: Vec<(PathBuf, Vec<u8>)>,
        /// Requests.
        requests: Vec<Request>,
        /// Extra reply fields.
        reply: Value,
    },
}

fn parse<T: DeserializeOwned>(params: &Value) -> Result<T, EngineError> {
    serde_json::from_value(params.clone()).map_err(|e| EngineError::invalid_params(e.to_string()))
}

fn to_value<T: serde::Serialize>(value: &T) -> Result<Value, EngineError> {
    serde_json::to_value(value).map_err(EngineError::internal)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PathParams {
    path: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PageParams {
    path: PathBuf,
    page: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TextParams {
    path: PathBuf,
    #[serde(default)]
    from: Option<usize>,
    #[serde(default)]
    to: Option<usize>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PartParams {
    draft: Draft,
    package: String,
    #[serde(default)]
    options: PartOptions,
    /// Include the 3D model (`part.preview`: as STEP text; `part.generate`:
    /// as a file in the project).
    #[serde(default)]
    model: Option<bool>,
    /// Target library (`part.generate`; default: the first one).
    #[serde(default)]
    doc: Option<Id>,
}

/// The `datasheet.*` methods (no project needed).
///
/// # Errors
///
/// Unknown methods, bad parameters or unreadable files.
pub fn call_datasheet(
    state: &mut Datasheets,
    method: &str,
    params: &Value,
) -> Result<Value, EngineError> {
    match method {
        "datasheet.read" => {
            let p: PathParams = parse(params)?;
            let sheet = state.load(&p.path)?;
            let pages: Vec<Value> = sheet
                .pages
                .iter()
                .map(|page| {
                    json!({
                        "number": page.number,
                        "width": page.width,
                        "height": page.height,
                        "scanned": page.scanned,
                    })
                })
                .collect();
            Ok(json!({
                "path": p.path,
                "title": sheet.title,
                "author": sheet.author,
                "subject": sheet.subject,
                "encrypted": sheet.encrypted,
                "bookmarks": sheet.bookmarks,
                "pages": pages,
            }))
        }
        "datasheet.page" => {
            let p: PageParams = parse(params)?;
            let sheet = state.load(&p.path)?;
            let page = sheet
                .pages
                .get(p.page.wrapping_sub(1))
                .ok_or_else(|| EngineError::not_found(format!("no page {}", p.page)))?;
            to_value(page)
        }
        "datasheet.text" => {
            let p: TextParams = parse(params)?;
            let sheet = state.load(&p.path)?;
            let from = p.from.unwrap_or(1).max(1);
            let to = p.to.unwrap_or(sheet.pages.len()).min(sheet.pages.len());
            let pages: Vec<Value> = sheet
                .pages
                .iter()
                .filter(|page| page.number >= from && page.number <= to)
                .map(|page| {
                    let text = page
                        .lines
                        .iter()
                        .map(|l| l.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n");
                    json!({"page": page.number, "text": text, "scanned": page.scanned})
                })
                .collect();
            Ok(json!({"pages": pages}))
        }
        "datasheet.extract" => {
            let p: PathParams = parse(params)?;
            let sheet = state.load(&p.path)?;
            let name = p.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let mut draft = ew_datasheet::recognize::recognize_named(&sheet, name);
            draft.datasheet = p.path.to_string_lossy().into_owned();
            to_value(&draft)
        }
        other => Err(EngineError::new(
            ErrorCode::MethodNotFound,
            format!("unknown method `{other}`"),
        )),
    }
}

fn mm(nm: i64) -> f64 {
    nm as f64 / 1_000_000.0
}

fn value(d: Option<&Dimension>) -> Option<f64> {
    d.and_then(Dimension::value)
}

/// The 3D model of a generated part.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one mapping per package kind")]
pub fn model_spec(part: &Generated) -> PackageSpec {
    let p = &part.package;
    let dim = |key: &str| value(p.dimensions.get(key));
    let terminals = match part.wizard {
        Wizard::GullWing { .. } | Wizard::Sot { .. } | Wizard::Tab { .. } => Terminals::GullWing,
        Wizard::NoLead { .. } => Terminals::NoLead,
        Wizard::Bga { .. } => Terminals::Balls,
        Wizard::Dip { .. } | Wizard::Header { .. } => Terminals::Dip,
        Wizard::Inline { .. } => Terminals::Inline {
            tab: p.family == Family::To220,
        },
        Wizard::Chip { .. } => Terminals::Chip,
    };
    let (body, span) = match part.wizard {
        Wizard::GullWing {
            quad,
            body_width,
            body_length,
            lead_span,
            ..
        } => {
            let span_y = if quad {
                dim("D").unwrap_or(mm(lead_span))
            } else {
                mm(body_length)
            };
            ([mm(body_width), mm(body_length)], [mm(lead_span), span_y])
        }
        Wizard::Sot {
            body_width,
            body_length,
            lead_span,
            ..
        }
        | Wizard::Tab {
            body_width,
            body_length,
            lead_span,
            ..
        } => (
            [mm(body_width), mm(body_length)],
            [mm(lead_span), mm(body_length)],
        ),
        Wizard::NoLead {
            body_width,
            body_length,
            ..
        }
        | Wizard::Bga {
            body_width,
            body_length,
            ..
        } => (
            [mm(body_width), mm(body_length)],
            [mm(body_width), mm(body_length)],
        ),
        Wizard::Dip {
            body_width,
            body_length,
            row_spacing,
            ..
        } => (
            [mm(body_width), mm(body_length)],
            [mm(row_spacing), mm(body_length)],
        ),
        Wizard::Inline {
            body_width,
            body_depth,
            ..
        } => (
            [mm(body_width), mm(body_depth)],
            [mm(body_width), mm(body_depth)],
        ),
        Wizard::Chip {
            body_length,
            body_width,
            ..
        } => (
            [mm(body_length), mm(body_width)],
            [mm(body_length), mm(body_width)],
        ),
        Wizard::Header { .. } => ([2.54, 2.54], [2.54, 2.54]),
    };
    let height = match terminals {
        Terminals::Inline { tab: true } => dim("D").unwrap_or(9.0),
        Terminals::Inline { tab: false } => dim("A").unwrap_or(4.5),
        _ => dim("A").unwrap_or(1.2),
    };
    let pads: Vec<PadPlace> = part
        .footprint
        .pads
        .iter()
        .map(|pad| PadPlace {
            number: pad.number.clone(),
            at: [mm(pad.at.x), mm(pad.at.y)],
            size: [mm(pad.size.w), mm(pad.size.h)],
        })
        .collect();
    // The exposed pad or tab: the pad numbered after the pins.
    let exposed = part
        .footprint
        .pads
        .iter()
        .find(|pad| {
            pad.number.parse::<u32>().ok() == Some(p.pins + 1)
                || matches!(
                    part.wizard,
                    Wizard::Tab {
                        omit_middle: true,
                        ..
                    }
                ) && pad.size.h > pad.size.w * 2
        })
        .map(|pad| {
            let size = match part.wizard {
                Wizard::Tab { tab_width, .. } => [mm(pad.size.w), mm(tab_width)],
                _ => [
                    dim("E2").unwrap_or(mm(pad.size.w)),
                    dim("D2").unwrap_or(mm(pad.size.h)),
                ],
            };
            (pad.number.clone(), size)
        });
    let lead_width = match part.wizard {
        Wizard::Bga { ball, .. } => mm(ball),
        _ => dim("b").unwrap_or(0.4),
    };
    PackageSpec {
        name: part.footprint.name.clone(),
        terminals,
        body,
        height,
        standoff: dim("A1").unwrap_or(match terminals {
            Terminals::Balls => 0.3,
            Terminals::Dip => 0.5,
            _ => 0.05,
        }),
        span,
        lead_width,
        lead_thickness: dim("c").unwrap_or(0.2),
        foot: dim("L").unwrap_or(0.5),
        pads,
        exposed,
    }
}

fn findings(part: &Generated) -> Value {
    let mut out: Vec<Value> = Vec::new();
    for (kind, list) in [
        ("symbol", check_symbol(&part.symbol)),
        ("footprint", check_footprint(&part.footprint)),
        (
            "component",
            check_component(&part.component, Some(&part.symbol), Some(&part.footprint)),
        ),
    ] {
        for f in list {
            out.push(json!({"item": kind, "finding": f}));
        }
    }
    Value::Array(out)
}

fn make(params: &PartParams) -> Result<Generated, EngineError> {
    generate(&params.draft, &params.package, &params.options)
        .map_err(|e| EngineError::invalid_params(e.0))
}

/// A name not taken in the library: `name`, `name_2`, ….
fn free_name(taken: &BTreeSet<String>, name: &str) -> String {
    if !taken.contains(name) {
        return name.to_owned();
    }
    (2..=taken.len() + 2)
        .map(|i| format!("{name}_{i}"))
        .find(|n| !taken.contains(n))
        .unwrap_or_else(|| name.to_owned())
}

/// Normalised part numbers: letters and digits, upper case.
fn key(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Whether a library name or part number is the draft's part: the same,
/// or the same with an ordering suffix (`LM358` and `LM358DR`).
fn same_part(candidate: &str, part: &str) -> bool {
    let (c, p) = (key(candidate), key(part));
    !p.is_empty() && (c == p || c.starts_with(&p) && c.len() <= p.len() + 6)
}

/// Pads of two footprints nearly the same: the same numbers, centres
/// within 0.15 mm (the package geometry; land sizes depend on the IPC
/// density and are not compared).
fn pads_match(a: &FootprintDef, b: &FootprintDef) -> bool {
    if a.pads.len() != b.pads.len() {
        return false;
    }
    let centre = |f: &FootprintDef| {
        let n = f.pads.len().max(1) as f64;
        (
            f.pads.iter().map(|p| mm(p.at.x)).sum::<f64>() / n,
            f.pads.iter().map(|p| mm(p.at.y)).sum::<f64>() / n,
        )
    };
    let (ca, cb) = (centre(a), centre(b));
    a.pads.iter().all(|pa| {
        b.pads.iter().any(|pb| {
            pb.number == pa.number
                && ((mm(pa.at.x) - ca.0) - (mm(pb.at.x) - cb.0)).abs() <= 0.15
                && ((mm(pa.at.y) - ca.1) - (mm(pb.at.y) - cb.1)).abs() <= 0.15
        })
    })
}

/// Parts of the project libraries and the installed base library that are
/// the draft's part in nearly the same package.
fn duplicates(
    project: Option<&Project>,
    library: &mut LibraryState,
    params: &PartParams,
    part: &Generated,
) -> Value {
    let names = [params.draft.name.as_str(), params.draft.mpn.as_str()];
    let is_part = |text: &str| names.iter().any(|n| !n.is_empty() && same_part(text, n));
    let mut matches: Vec<Value> = Vec::new();
    let mut related: Vec<Value> = Vec::new();
    if let Some(project) = project {
        for entry in project.documents_of(DocumentKind::Library) {
            let Some(Document::Library(lib)) = project.document(entry.id) else {
                continue;
            };
            for c in &lib.components {
                if !(is_part(&c.name) || is_part(&c.mpn) || is_part(&c.value)) {
                    continue;
                }
                let fp = c.footprint.as_deref().and_then(|n| lib.footprint(n));
                let item = json!({
                    "source": "project",
                    "library": lib.name,
                    "doc": entry.id,
                    "kind": "component",
                    "name": c.name,
                    "footprint": c.footprint,
                });
                if fp.is_some_and(|f| pads_match(f, &part.footprint)) {
                    matches.push(item);
                } else {
                    related.push(item);
                }
            }
            for s in &lib.symbols {
                if !(is_part(&s.name) || is_part(&s.value)) {
                    continue;
                }
                if lib.components.iter().any(|c| c.symbol == s.name) {
                    continue;
                }
                let fp = s.footprint.as_deref().and_then(|n| lib.footprint(n));
                let item = json!({
                    "source": "project",
                    "library": lib.name,
                    "doc": entry.id,
                    "kind": "symbol",
                    "name": s.name,
                    "footprint": s.footprint,
                });
                if fp.is_some_and(|f| pads_match(f, &part.footprint)) {
                    matches.push(item);
                } else {
                    related.push(item);
                }
            }
        }
    }
    // The installed base library: symbols named like the part.
    let mut seen = BTreeSet::new();
    for name in names.iter().filter(|n| !n.is_empty()) {
        let query = Query {
            text: (*name).to_owned(),
            kind: Some(Kind::Symbol),
            limit: 60,
            ..Query::default()
        };
        let Ok(found) = library.search(&query) else {
            break;
        };
        for item in found.items {
            if !seen.insert(item.id) || !(is_part(&item.name) || is_part(&item.value)) {
                continue;
            }
            let footprint = library
                .find(Kind::Footprint, &item.package)
                .ok()
                .flatten()
                .and_then(|id| library.definition(id).ok())
                .and_then(|(_, def, _)| serde_json::from_value::<FootprintDef>(def).ok());
            let entry = json!({
                "source": "base",
                "library": item.library,
                "id": item.id,
                "kind": "symbol",
                "name": item.name,
                "footprint": item.package,
            });
            if footprint.is_some_and(|f| pads_match(&f, &part.footprint)) {
                matches.push(entry);
            } else {
                related.push(entry);
            }
        }
    }
    json!({"matches": matches, "related": related})
}

fn target_library(project: &Project, doc: Option<Id>) -> Result<(Id, &Library), EngineError> {
    let doc = match doc {
        Some(doc) => doc,
        None => project
            .documents_of(DocumentKind::Library)
            .next()
            .map(|entry| entry.id)
            .ok_or_else(|| EngineError::not_found("the project has no library"))?,
    };
    match project.document(doc) {
        Some(Document::Library(lib)) => Ok((doc, lib)),
        _ => Err(EngineError::invalid_params("the target must be a library")),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DraftSaveParams {
    draft: Draft,
    #[serde(default)]
    path: Option<PathBuf>,
}

/// Where a draft is saved by default: `parts/<name>.part.json` in the
/// project, else next to the datasheet.
fn draft_path(root: Option<&Path>, draft: &Draft) -> Result<PathBuf, EngineError> {
    let name = ew_datasheet::part::clean_name(&draft.name);
    let name = if name.is_empty() {
        "part".to_owned()
    } else {
        name
    };
    if let Some(root) = root {
        return Ok(root.join("parts").join(format!("{name}.part.json")));
    }
    let sheet = PathBuf::from(&draft.datasheet);
    if draft.datasheet.is_empty() || !sheet.is_absolute() {
        return Err(EngineError::invalid_params(
            "give a path: no project is open and the draft names no datasheet",
        ));
    }
    Ok(sheet.with_extension("part.json"))
}

/// The `part.*` methods.
///
/// # Errors
///
/// Unknown methods, bad drafts, a part that cannot be generated, or
/// `part.generate` without an open project.
#[allow(
    clippy::too_many_lines,
    reason = "the three methods with their replies"
)]
pub fn call_part(
    project: Option<(&Project, &Path)>,
    library: &mut LibraryState,
    method: &str,
    params: &Value,
) -> Result<Outcome, EngineError> {
    match method {
        "part.preview" => {
            let p: PartParams = parse(params)?;
            let part = make(&p)?;
            let mut reply = json!({
                "symbol": part.symbol,
                "footprint": part.footprint,
                "component": part.component,
                "wizard": part.wizard,
                "warnings": part.warnings,
                "findings": findings(&part),
            });
            if p.model.unwrap_or(false)
                && let Some(map) = reply.as_object_mut()
            {
                let spec = model_spec(&part);
                map.insert(
                    "step".into(),
                    Value::String(ew_board3d::package::step(&spec, env!("CARGO_PKG_VERSION"))),
                );
            }
            Ok(Outcome::Read(reply))
        }
        "part.draftSave" => {
            let p: DraftSaveParams = parse(params)?;
            let path = match p.path {
                Some(path) => path,
                None => draft_path(project.map(|(_, root)| root), &p.draft)?,
            };
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| EngineError::internal(format!("{}: {e}", dir.display())))?;
            }
            let text = serde_json::to_string_pretty(&p.draft).map_err(EngineError::internal)?;
            std::fs::write(&path, text + "\n")
                .map_err(|e| EngineError::internal(format!("{}: {e}", path.display())))?;
            Ok(Outcome::Read(json!({ "path": path })))
        }
        "part.draftLoad" => {
            let p: PathParams = parse(params)?;
            let text = std::fs::read_to_string(&p.path)
                .map_err(|e| EngineError::not_found(format!("{}: {e}", p.path.display())))?;
            let draft: Draft = serde_json::from_str(&text)
                .map_err(|e| EngineError::invalid_params(format!("{}: {e}", p.path.display())))?;
            Ok(Outcome::Read(to_value(&draft)?))
        }
        "part.duplicates" => {
            let p: PartParams = parse(params)?;
            let part = make(&p)?;
            Ok(Outcome::Read(duplicates(
                project.map(|(p, _)| p),
                library,
                &p,
                &part,
            )))
        }
        "part.generate" => {
            let (project, root) = project.ok_or_else(EngineError::no_project)?;
            let p: PartParams = parse(params)?;
            let mut part = make(&p)?;
            let (doc, lib) = target_library(project, p.doc)?;
            let taken = |names: Vec<String>| names.into_iter().collect::<BTreeSet<String>>();
            let footprint_name = free_name(
                &taken(lib.footprints.iter().map(|f| f.name.clone()).collect()),
                &part.footprint.name,
            );
            let symbol_name = free_name(
                &taken(lib.symbols.iter().map(|s| s.name.clone()).collect()),
                &part.symbol.name,
            );
            let component_name = free_name(
                &taken(lib.components.iter().map(|c| c.name.clone()).collect()),
                &part.component.name,
            );
            part.footprint.name.clone_from(&footprint_name);
            part.symbol.name.clone_from(&symbol_name);
            part.symbol.footprint = Some(footprint_name.clone());
            part.component.name.clone_from(&component_name);
            part.component.symbol.clone_from(&symbol_name);
            part.component.footprint = Some(footprint_name.clone());
            let mut files = Vec::new();
            let mut model = Value::Null;
            if p.model.unwrap_or(true) {
                let relative = format!("models/{footprint_name}.step");
                let spec = model_spec(&part);
                files.push((
                    root.join(&relative),
                    ew_board3d::package::step(&spec, env!("CARGO_PKG_VERSION")).into_bytes(),
                ));
                part.footprint.models.push(Model3d {
                    path: relative.clone(),
                    offset: ew_model::units::Vec3::default(),
                    rotation: Default::default(),
                    scale: ew_model::pcb::Scale3::default(),
                    hidden: false,
                });
                model = Value::String(relative);
            }
            let reply = json!({
                "symbol": symbol_name,
                "footprint": footprint_name,
                "component": component_name,
                "model": model,
                "warnings": part.warnings,
            });
            let requests = vec![
                Request::Insert {
                    doc,
                    object: Object::FootprintDef(part.footprint),
                },
                Request::Insert {
                    doc,
                    object: Object::SymbolDef(part.symbol),
                },
                Request::Insert {
                    doc,
                    object: Object::ComponentDef(part.component),
                },
            ];
            Ok(Outcome::Commit {
                files,
                requests,
                reply,
            })
        }
        other => Err(EngineError::new(
            ErrorCode::MethodNotFound,
            format!("unknown method `{other}`"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_part_numbers_loosely() {
        assert!(same_part("LM358DR", "LM358"));
        assert!(same_part("lm-358", "LM358"));
        assert!(!same_part("LM3580ABCDEFGH", "LM358"));
        assert!(!same_part("LM324", "LM358"));
    }

    #[test]
    fn names_avoid_taken_ones() {
        let taken: BTreeSet<String> = ["X".to_owned(), "X_2".to_owned()].into_iter().collect();
        assert_eq!(free_name(&taken, "X"), "X_3");
        assert_eq!(free_name(&taken, "Y"), "Y");
    }
}
