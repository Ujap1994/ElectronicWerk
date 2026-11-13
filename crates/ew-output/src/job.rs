//! Output jobs: one run generates every output of a job (Gerber, drill,
//! bill of materials, placement, prints, netlists, a ZIP package for the
//! board maker), and presets fill a job with what a board house expects.
//!
//! Running is pure: the files come back in memory with paths relative to
//! the job's output folder, and the caller writes them.

use std::collections::BTreeMap;

use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::layer::Layer;
use ew_model::outputs::{OutputJob, OutputKind, OutputSpec};
use ew_model::pcb::Pcb;
use ew_model::project::Project;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::OutputFile;
use crate::art::{ArtOptions, BoardArt};
use crate::bom::{self, BomOptions, Heading};
use crate::print::{self, AssemblyPrint, BoardPrint, ColorMode, Paper, SchematicPrint};

/// Inputs of a run besides the job.
pub struct RunContext<'a> {
    /// The project.
    pub project: &'a Project,
    /// Application version.
    pub version: String,
    /// ISO 8601 time of the run (empty for reproducible files).
    pub created: String,
    /// Expands text variables for a document.
    pub resolve: &'a dyn Fn(&str, Option<Id>) -> String,
    /// Title block labels of prints.
    pub labels: ew_scene::TitleLabels,
}

impl std::fmt::Debug for RunContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunContext")
            .field("project", &self.project.manifest.name)
            .finish_non_exhaustive()
    }
}

/// Result of one output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// Files were generated.
    Done,
    /// Not generated (switched off, or not available yet).
    Skipped,
    /// Failed; see the message.
    Failed,
}

/// What one output produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputResult {
    /// Output id.
    pub id: Id,
    /// Output name.
    pub name: String,
    /// Kind.
    pub kind: OutputKind,
    /// Status.
    pub status: Status,
    /// Why it was skipped or failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Generated files (relative paths).
    pub files: Vec<String>,
}

/// The result of a run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunResult {
    /// Generated files.
    pub files: Vec<OutputFile>,
    /// Per output.
    pub results: Vec<OutputResult>,
}

/// A file name part made of safe characters.
#[must_use]
pub fn sanitize(name: &str) -> String {
    let out: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '+') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() {
        "board".to_owned()
    } else {
        out
    }
}

fn options<T: DeserializeOwned + Default>(spec: &OutputSpec) -> Result<T, String> {
    if spec.options.is_empty() {
        return Ok(T::default());
    }
    let value = Value::Object(spec.options.clone().into_iter().collect());
    serde_json::from_value(value).map_err(|e| format!("invalid options: {e}"))
}

fn folder(path: &str, name: &str) -> String {
    let path = path.trim_matches('/');
    if path.is_empty() {
        name.to_owned()
    } else {
        format!("{path}/{name}")
    }
}

fn parse_layers(names: &[String]) -> Result<Vec<Layer>, String> {
    names
        .iter()
        .map(|n| n.parse::<Layer>().map_err(|e| e.to_string()))
        .collect()
}

/// Options of the Gerber output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[allow(clippy::struct_excessive_bools, reason = "independent switches")]
pub struct GerberJob {
    /// Layers (empty: the fabrication layers).
    pub layers: Vec<String>,
    /// Protel file extensions.
    pub protel_extensions: bool,
    /// X2 attributes.
    pub x2: bool,
    /// Gerber job file.
    pub job_file: bool,
    /// Vias covered by solder mask.
    pub tent_vias: bool,
    /// Values on the silkscreen.
    pub silk_values: bool,
    /// Subfolder.
    pub folder: String,
}

impl Default for GerberJob {
    fn default() -> Self {
        Self {
            layers: Vec::new(),
            protel_extensions: true,
            x2: true,
            job_file: true,
            tent_vias: true,
            silk_values: true,
            folder: "gerber".to_owned(),
        }
    }
}

/// Options of the drill output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DrillJob {
    /// Plated and non-plated holes in one file.
    pub merge: bool,
    /// Subfolder.
    pub folder: String,
}

impl Default for DrillJob {
    fn default() -> Self {
        Self {
            merge: false,
            folder: "gerber".to_owned(),
        }
    }
}

/// Vector file format of drawings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DrawingFormat {
    /// PDF.
    #[default]
    Pdf,
    /// SVG (one file per page).
    Svg,
}

/// Options of the drill map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DrillMapJob {
    /// Format.
    pub format: DrawingFormat,
    /// Subfolder.
    pub folder: String,
}

impl Default for DrillMapJob {
    fn default() -> Self {
        Self {
            format: DrawingFormat::Pdf,
            folder: "gerber".to_owned(),
        }
    }
}

/// File format of the bill of materials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BomFormat {
    /// CSV.
    #[default]
    Csv,
    /// Excel workbook.
    Xlsx,
    /// HTML page.
    Html,
}

/// Options of the bill of materials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BomJob {
    /// Table options.
    #[serde(flatten)]
    pub table: BomOptions,
    /// Format.
    pub format: BomFormat,
    /// Subfolder.
    pub folder: String,
}

impl Default for BomJob {
    fn default() -> Self {
        Self {
            table: BomOptions::default(),
            format: BomFormat::Csv,
            folder: "assembly".to_owned(),
        }
    }
}

/// Options of the placement file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlacementJob {
    /// File options.
    #[serde(flatten)]
    pub file: crate::placement::PlacementOptions,
    /// Subfolder.
    pub folder: String,
}

impl Default for PlacementJob {
    fn default() -> Self {
        Self {
            file: crate::placement::PlacementOptions::default(),
            folder: "assembly".to_owned(),
        }
    }
}

/// What a PDF or SVG print shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PrintContent {
    /// Every schematic sheet.
    #[default]
    Schematic,
    /// Board layers.
    Board,
    /// Assembly drawings.
    Assembly,
}

/// Options of PDF and SVG prints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[allow(clippy::struct_excessive_bools, reason = "independent switches")]
pub struct PrintJob {
    /// Content.
    pub content: PrintContent,
    /// Board layers (board prints).
    pub layers: Vec<String>,
    /// One page per layer (board prints).
    pub page_per_layer: bool,
    /// Bottom view (board prints).
    pub mirror: bool,
    /// Colours.
    pub color: ColorMode,
    /// Paper.
    pub paper: Paper,
    /// Values under the designators (assembly drawings).
    pub values: bool,
    /// Subfolder.
    pub folder: String,
}

impl Default for PrintJob {
    fn default() -> Self {
        Self {
            content: PrintContent::Schematic,
            layers: vec!["cu.top".to_owned(), "silk.top".to_owned()],
            page_per_layer: false,
            mirror: false,
            color: ColorMode::Color,
            paper: Paper::Fit,
            values: false,
            folder: "documentation".to_owned(),
        }
    }
}

/// Options of netlist outputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NetlistJob {
    /// `kicad` or `csv` (schematic netlists).
    pub format: String,
    /// Vias covered by solder mask (IPC-D-356).
    pub tent_vias: bool,
    /// Subfolder.
    pub folder: String,
}

impl Default for NetlistJob {
    fn default() -> Self {
        Self {
            format: "kicad".to_owned(),
            tent_vias: true,
            folder: "netlists".to_owned(),
        }
    }
}

/// Options of the ZIP package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ArchiveJob {
    /// Output kinds whose files go into the package.
    pub include: Vec<OutputKind>,
    /// File name; `{board}` is replaced.
    pub file: String,
}

impl Default for ArchiveJob {
    fn default() -> Self {
        Self {
            include: vec![OutputKind::Gerber, OutputKind::Drill],
            file: "{board}-fabrication.zip".to_owned(),
        }
    }
}

/// The board an output uses: its source, else the first board.
fn board(project: &Project, source: Option<Id>) -> Result<&Pcb, String> {
    if let Some(id) = source {
        return match project.document(id) {
            Some(Document::Pcb(pcb)) => Ok(pcb),
            _ => Err("the source is not a board".to_owned()),
        };
    }
    project
        .manifest
        .documents
        .iter()
        .find_map(|entry| match project.document(entry.id) {
            Some(Document::Pcb(pcb)) => Some(pcb),
            _ => None,
        })
        .ok_or_else(|| "the project has no board".to_owned())
}

fn has_schematic(project: &Project) -> bool {
    project
        .documents
        .values()
        .any(|d| matches!(d, Document::Schematic(_)))
}

/// Runs one output; returns its files (relative paths).
#[allow(clippy::too_many_lines, reason = "one arm per output kind")]
fn run_one(
    spec: &OutputSpec,
    ctx: &RunContext<'_>,
    done: &[(OutputKind, Vec<OutputFile>)],
) -> Result<Vec<OutputFile>, String> {
    let project = ctx.project;
    let title = |pcb: &Pcb| format!("{} – {}", project.manifest.name, pcb.name);
    match spec.kind {
        OutputKind::Gerber => {
            let o: GerberJob = options(spec)?;
            let pcb = board(project, spec.source)?;
            let resolve = |t: &str| (ctx.resolve)(t, Some(pcb.id));
            let art = BoardArt::new(
                pcb,
                ArtOptions {
                    tent_vias: o.tent_vias,
                    silk_values: o.silk_values,
                    ..ArtOptions::default()
                },
                &resolve,
            );
            let files = crate::gerber::gerbers(
                &art,
                &crate::gerber::GerberOptions {
                    layers: parse_layers(&o.layers)?,
                    protel_extensions: o.protel_extensions,
                    attributes: o.x2,
                    job_file: o.job_file,
                },
                &crate::gerber::Context {
                    version: ctx.version.clone(),
                    created: ctx.created.clone(),
                    project: project.manifest.name.clone(),
                    guid: project.manifest.id.to_string(),
                    revision: project.manifest.revision.clone(),
                    stem: sanitize(&pcb.name),
                },
            );
            Ok(files
                .into_iter()
                .map(|f| OutputFile::binary(folder(&o.folder, &f.name), f.data))
                .collect())
        }
        OutputKind::Drill => {
            let o: DrillJob = options(spec)?;
            let pcb = board(project, spec.source)?;
            Ok(crate::drill::drill_files(
                pcb,
                crate::drill::DrillOptions { merge: o.merge },
                &sanitize(&pcb.name),
                &ctx.version,
                &ctx.created,
            )
            .into_iter()
            .map(|f| OutputFile::binary(folder(&o.folder, &f.name), f.data))
            .collect())
        }
        OutputKind::DrillMap => {
            let o: DrillMapJob = options(spec)?;
            let pcb = board(project, spec.source)?;
            let resolve = |t: &str| (ctx.resolve)(t, Some(pcb.id));
            let art = BoardArt::new(pcb, ArtOptions::default(), &resolve);
            let doc = crate::drillmap::drill_map_doc(
                &art,
                &crate::drillmap::Labels::default(),
                &ctx.version,
                &ctx.created,
            );
            let stem = sanitize(&pcb.name);
            Ok(drawing(
                &doc,
                o.format,
                &o.folder,
                &format!("{stem}-drill-map"),
            ))
        }
        OutputKind::Bom => {
            let o: BomJob = options(spec)?;
            let pcb = board(project, spec.source).ok();
            if pcb.is_none() && !has_schematic(project) {
                return Err("the project has no schematic and no board".to_owned());
            }
            let parts = bom::parts(project, pcb);
            let table = bom::build(&parts, &o.table);
            let stem = sanitize(&project.manifest.name);
            let file = match o.format {
                BomFormat::Csv => OutputFile::text(
                    folder(&o.folder, &format!("{stem}-bom.csv")),
                    bom::csv(&table, o.table.separator),
                ),
                BomFormat::Xlsx => OutputFile::binary(
                    folder(&o.folder, &format!("{stem}-bom.xlsx")),
                    bom::workbook(&table, "BOM"),
                ),
                BomFormat::Html => OutputFile::text(
                    folder(&o.folder, &format!("{stem}-bom.html")),
                    bom::html(
                        &table,
                        &Heading {
                            title: format!("{} – Bill of materials", project.manifest.name),
                            details: [
                                ("Revision", project.manifest.revision.clone()),
                                ("Generated", ctx.created.clone()),
                            ]
                            .into_iter()
                            .filter(|(_, v)| !v.is_empty())
                            .map(|(k, v)| (k.to_owned(), v))
                            .collect(),
                        },
                    ),
                ),
            };
            Ok(vec![file])
        }
        OutputKind::PickAndPlace => {
            let o: PlacementJob = options(spec)?;
            let pcb = board(project, spec.source)?;
            Ok(
                crate::placement::placement_files(pcb, &o.file, &sanitize(&pcb.name))
                    .into_iter()
                    .map(|f| OutputFile::binary(folder(&o.folder, &f.name), f.data))
                    .collect(),
            )
        }
        OutputKind::Pdf | OutputKind::Svg => {
            let o: PrintJob = options(spec)?;
            let format = if spec.kind == OutputKind::Pdf {
                DrawingFormat::Pdf
            } else {
                DrawingFormat::Svg
            };
            match o.content {
                PrintContent::Schematic => {
                    if !has_schematic(project) {
                        return Err("the project has no schematic".to_owned());
                    }
                    let pages = print::schematic_pages(
                        project,
                        &ew_scene::Options {
                            resolve: &|t: &str| (ctx.resolve)(t, None),
                            labels: ctx.labels.clone(),
                        },
                        SchematicPrint {
                            color: o.color,
                            paper: o.paper,
                        },
                    );
                    let stem = sanitize(&project.manifest.name);
                    let doc = print::document(
                        &format!("{} – Schematic", project.manifest.name),
                        pages,
                        &ctx.version,
                        &ctx.created,
                    );
                    Ok(drawing(
                        &doc,
                        format,
                        &o.folder,
                        &format!("{stem}-schematic"),
                    ))
                }
                PrintContent::Board => {
                    let pcb = board(project, spec.source)?;
                    let resolve = |t: &str| (ctx.resolve)(t, Some(pcb.id));
                    let art = BoardArt::new(pcb, ArtOptions::default(), &resolve);
                    let pages = print::board_pages(
                        &art,
                        &BoardPrint {
                            layers: parse_layers(&o.layers)?,
                            page_per_layer: o.page_per_layer,
                            mirror: o.mirror,
                            color: o.color,
                            paper: o.paper,
                            outline: true,
                        },
                        &title(pcb),
                    );
                    let doc = print::document(&title(pcb), pages, &ctx.version, &ctx.created);
                    let stem = sanitize(&pcb.name);
                    Ok(drawing(&doc, format, &o.folder, &format!("{stem}-layers")))
                }
                PrintContent::Assembly => {
                    let pcb = board(project, spec.source)?;
                    let resolve = |t: &str| (ctx.resolve)(t, Some(pcb.id));
                    let art = BoardArt::new(pcb, ArtOptions::default(), &resolve);
                    let pages = print::assembly_pages(
                        &art,
                        &AssemblyPrint {
                            values: o.values,
                            paper: if o.paper == Paper::Fit {
                                Paper::A4
                            } else {
                                o.paper
                            },
                            ..AssemblyPrint::default()
                        },
                        &title(pcb),
                    );
                    let doc = print::document(&title(pcb), pages, &ctx.version, &ctx.created);
                    let stem = sanitize(&pcb.name);
                    Ok(drawing(
                        &doc,
                        format,
                        &o.folder,
                        &format!("{stem}-assembly"),
                    ))
                }
            }
        }
        OutputKind::Ipc356 => {
            let o: NetlistJob = options(spec)?;
            let pcb = board(project, spec.source)?;
            Ok(vec![OutputFile::text(
                folder(&o.folder, &format!("{}.d356", sanitize(&pcb.name))),
                crate::ipc356::netlist(pcb, o.tent_vias, &ctx.version),
            )])
        }
        OutputKind::Ipc2581 => {
            let o: NetlistJob = options(spec)?;
            let pcb = board(project, spec.source)?;
            let resolve = |t: &str| (ctx.resolve)(t, Some(pcb.id));
            let art = BoardArt::new(pcb, ArtOptions::default(), &resolve);
            let folder_name = if spec.options.contains_key("folder") {
                o.folder
            } else {
                String::new()
            };
            Ok(vec![OutputFile::text(
                folder(
                    &folder_name,
                    &format!("{}-ipc2581.xml", sanitize(&pcb.name)),
                ),
                crate::ipc2581::write(
                    &art,
                    &crate::ipc2581::Context {
                        version: ctx.version.clone(),
                        created: ctx.created.clone(),
                    },
                ),
            )])
        }
        OutputKind::Netlist => {
            let o: NetlistJob = options(spec)?;
            if !has_schematic(project) {
                return Err("the project has no schematic".to_owned());
            }
            let connectivity = ew_schematic::analyze(project);
            let stem = sanitize(&project.manifest.name);
            Ok(vec![if o.format == "csv" {
                OutputFile::text(
                    folder(&o.folder, &format!("{stem}-netlist.csv")),
                    ew_schematic::export::csv_netlist(&connectivity),
                )
            } else {
                OutputFile::text(
                    folder(&o.folder, &format!("{stem}.net")),
                    ew_schematic::export::kicad_netlist(project, &connectivity),
                )
            }])
        }
        OutputKind::Archive => {
            let o: ArchiveJob = options(spec)?;
            let entries: Vec<OutputFile> = done
                .iter()
                .filter(|(kind, _)| o.include.contains(kind))
                .flat_map(|(_, files)| files.iter())
                .map(|f| {
                    let name = f.name.rsplit('/').next().unwrap_or(&f.name).to_owned();
                    OutputFile::binary(name, f.data.clone())
                })
                .collect();
            if entries.is_empty() {
                return Err("no files to pack (run the included outputs first)".to_owned());
            }
            let stem = board(project, spec.source).map_or_else(
                |_| sanitize(&project.manifest.name),
                |pcb| sanitize(&pcb.name),
            );
            Ok(vec![OutputFile::binary(
                o.file.replace("{board}", &stem),
                crate::archive::zip(&entries),
            )])
        }
        OutputKind::Step => Err("SKIP:3D export (STEP) comes with the 3D view".to_owned()),
    }
}

/// A drawing as one PDF or one SVG per page.
fn drawing(
    doc: &crate::canvas::Doc,
    format: DrawingFormat,
    path: &str,
    stem: &str,
) -> Vec<OutputFile> {
    match format {
        DrawingFormat::Pdf => vec![OutputFile::binary(
            folder(path, &format!("{stem}.pdf")),
            crate::pdf::write(doc),
        )],
        DrawingFormat::Svg => {
            let single = doc.pages.len() == 1;
            doc.pages
                .iter()
                .enumerate()
                .map(|(i, page)| {
                    let name = if single {
                        format!("{stem}.svg")
                    } else {
                        format!("{stem}-{}.svg", i + 1)
                    };
                    OutputFile::text(folder(path, &name), crate::svg::write(page, &doc.title))
                })
                .collect()
        }
    }
}

/// Runs a job (or some of its outputs). Outputs run in order; one that
/// fails does not stop the others.
#[must_use]
pub fn run(job: &OutputJob, only: Option<&[Id]>, ctx: &RunContext<'_>) -> RunResult {
    let mut result = RunResult::default();
    let mut done: Vec<(OutputKind, Vec<OutputFile>)> = Vec::new();
    for spec in &job.outputs {
        let selected = only.is_none_or(|ids| ids.contains(&spec.id));
        let mut entry = OutputResult {
            id: spec.id,
            name: spec.name.clone(),
            kind: spec.kind,
            status: Status::Skipped,
            message: None,
            files: Vec::new(),
        };
        if !selected || !spec.enabled {
            entry.message = Some(
                if spec.enabled {
                    "not selected"
                } else {
                    "switched off"
                }
                .to_owned(),
            );
            result.results.push(entry);
            continue;
        }
        match run_one(spec, ctx, &done) {
            Ok(files) => {
                entry.status = Status::Done;
                entry.files = files.iter().map(|f| f.name.clone()).collect();
                done.push((spec.kind, files.clone()));
                result.files.extend(files);
            }
            Err(message) => {
                if let Some(reason) = message.strip_prefix("SKIP:") {
                    entry.message = Some(reason.to_owned());
                } else {
                    entry.status = Status::Failed;
                    entry.message = Some(message);
                }
            }
        }
        result.results.push(entry);
    }
    result
}

/// A preset of outputs for a board house.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    /// Id.
    pub id: &'static str,
    /// Name.
    pub name: &'static str,
    /// What it is for.
    pub description: &'static str,
    /// Outputs (name, kind, options).
    pub outputs: Vec<(String, OutputKind, Value)>,
}

fn jlc_bom() -> Value {
    json!({
        "columns": [
            {"field": "value", "label": "Comment"},
            {"field": "designators", "label": "Designator"},
            {"field": "footprint", "label": "Footprint"},
            {"field": "LCSC", "label": "LCSC Part #"}
        ],
        "groupBy": ["value", "footprint", "LCSC"],
        "includeDnp": false,
        "format": "xlsx"
    })
}

/// The presets for common board makers.
#[must_use]
#[allow(clippy::too_many_lines, reason = "a table of presets")]
pub fn presets() -> Vec<Preset> {
    let o = |name: &str, kind: OutputKind, options: Value| (name.to_owned(), kind, options);
    let assembly_docs = |outputs: &mut Vec<(String, OutputKind, Value)>| {
        outputs.push(o("Schematic PDF", OutputKind::Pdf, json!({})));
        outputs.push(o(
            "Assembly drawing",
            OutputKind::Pdf,
            json!({"content": "assembly", "paper": "a4"}),
        ));
    };
    let mut generic = vec![
        o("Gerber", OutputKind::Gerber, json!({})),
        o("Drill", OutputKind::Drill, json!({})),
        o("Drill map", OutputKind::DrillMap, json!({})),
        o(
            "Bill of materials",
            OutputKind::Bom,
            json!({"format": "xlsx"}),
        ),
        o("Pick and place", OutputKind::PickAndPlace, json!({})),
        o("IPC-D-356 netlist", OutputKind::Ipc356, json!({})),
    ];
    assembly_docs(&mut generic);
    generic.push(o("Fabrication package", OutputKind::Archive, json!({})));
    let mut jlc = vec![
        o(
            "Gerber",
            OutputKind::Gerber,
            json!({"x2": false, "jobFile": false}),
        ),
        o("Drill", OutputKind::Drill, json!({})),
        o("BOM (JLCPCB)", OutputKind::Bom, jlc_bom()),
        o(
            "CPL (JLCPCB)",
            OutputKind::PickAndPlace,
            json!({"style": "jlcpcb"}),
        ),
    ];
    jlc.push(o(
        "Gerber ZIP",
        OutputKind::Archive,
        json!({"file": "{board}-jlcpcb.zip"}),
    ));
    let mut pcbway = vec![
        o("Gerber", OutputKind::Gerber, json!({"x2": false})),
        o("Drill", OutputKind::Drill, json!({})),
        o(
            "Bill of materials",
            OutputKind::Bom,
            json!({
                "columns": [
                    {"field": "item"}, {"field": "designators"}, {"field": "quantity"},
                    {"field": "value"}, {"field": "footprint"}, {"field": "Manufacturer"},
                    {"field": "MPN"}, {"field": "description"}
                ],
                "format": "xlsx"
            }),
        ),
        o("Pick and place", OutputKind::PickAndPlace, json!({})),
    ];
    assembly_docs(&mut pcbway);
    pcbway.push(o(
        "Gerber ZIP",
        OutputKind::Archive,
        json!({"file": "{board}-pcbway.zip"}),
    ));
    let mut aisler = vec![
        o("Gerber X2", OutputKind::Gerber, json!({})),
        o("Drill", OutputKind::Drill, json!({})),
        o(
            "Bill of materials",
            OutputKind::Bom,
            json!({"format": "csv"}),
        ),
        o("Pick and place", OutputKind::PickAndPlace, json!({})),
    ];
    aisler.push(o(
        "Package",
        OutputKind::Archive,
        json!({"file": "{board}-aisler.zip"}),
    ));
    let mut eurocircuits = vec![
        o("Gerber X2", OutputKind::Gerber, json!({})),
        o("Drill", OutputKind::Drill, json!({})),
        o("IPC-D-356 netlist", OutputKind::Ipc356, json!({})),
        o("IPC-2581", OutputKind::Ipc2581, json!({})),
        o(
            "Bill of materials",
            OutputKind::Bom,
            json!({"format": "xlsx"}),
        ),
        o("Pick and place", OutputKind::PickAndPlace, json!({})),
    ];
    eurocircuits.push(o(
        "Package",
        OutputKind::Archive,
        json!({"include": ["gerber", "drill", "ipc356"], "file": "{board}-eurocircuits.zip"}),
    ));
    let oshpark = vec![
        o(
            "Gerber",
            OutputKind::Gerber,
            json!({"x2": false, "jobFile": false}),
        ),
        o("Drill", OutputKind::Drill, json!({"merge": true})),
        o(
            "Gerber ZIP",
            OutputKind::Archive,
            json!({"file": "{board}-oshpark.zip"}),
        ),
    ];
    vec![
        Preset {
            id: "generic",
            name: "Generic",
            description: "Gerber X2, drill files and map, BOM, placement, IPC-D-356, PDFs and a ZIP package",
            outputs: generic,
        },
        Preset {
            id: "jlcpcb",
            name: "JLCPCB",
            description: "Gerber with Protel extensions, drill files, BOM and CPL in JLCPCB's columns",
            outputs: jlc,
        },
        Preset {
            id: "pcbway",
            name: "PCBWay",
            description: "Gerber RS-274X, drill files, BOM workbook, placement and assembly drawings",
            outputs: pcbway,
        },
        Preset {
            id: "aisler",
            name: "AISLER",
            description: "Gerber X2 with job file, drill files, BOM and placement",
            outputs: aisler,
        },
        Preset {
            id: "eurocircuits",
            name: "Eurocircuits",
            description: "Gerber X2, drill files, IPC-D-356 netlist, IPC-2581, BOM and placement",
            outputs: eurocircuits,
        },
        Preset {
            id: "oshpark",
            name: "OSH Park",
            description: "Gerber and one drill file in a ZIP package",
            outputs: oshpark,
        },
    ]
}

/// The outputs of a preset with new ids.
#[must_use]
pub fn preset_outputs(preset: &Preset, mut make_id: impl FnMut() -> Id) -> Vec<OutputSpec> {
    preset
        .outputs
        .iter()
        .map(|(name, kind, options)| OutputSpec {
            id: make_id(),
            name: name.clone(),
            kind: *kind,
            enabled: true,
            source: None,
            options: options
                .as_object()
                .map(|m| m.clone().into_iter().collect::<BTreeMap<_, _>>())
                .unwrap_or_default(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_sanitized() {
        assert_eq!(sanitize("My board/v2 (rev A)"), "My_board_v2__rev_A_");
        assert_eq!(sanitize("  "), "board");
        assert_eq!(sanitize("Platine-Ü1"), "Platine-Ü1");
    }

    #[test]
    fn presets_have_valid_options() {
        for preset in presets() {
            for spec in preset_outputs(&preset, Id::random) {
                let ok = match spec.kind {
                    OutputKind::Gerber => options::<GerberJob>(&spec).is_ok(),
                    OutputKind::Drill => options::<DrillJob>(&spec).is_ok(),
                    OutputKind::DrillMap => options::<DrillMapJob>(&spec).is_ok(),
                    OutputKind::Bom => options::<BomJob>(&spec).is_ok(),
                    OutputKind::PickAndPlace => options::<PlacementJob>(&spec).is_ok(),
                    OutputKind::Pdf | OutputKind::Svg => options::<PrintJob>(&spec).is_ok(),
                    OutputKind::Archive => options::<ArchiveJob>(&spec).is_ok(),
                    _ => options::<NetlistJob>(&spec).is_ok(),
                };
                assert!(ok, "{} {}", preset.id, spec.name);
            }
        }
        let jlc: BomJob = serde_json::from_value(jlc_bom()).unwrap();
        assert_eq!(jlc.format, BomFormat::Xlsx);
        assert_eq!(jlc.table.columns[3].heading(), "LCSC Part #");
    }
}

#[cfg(test)]
mod preset_sync {
    use super::*;

    #[test]
    fn the_default_job_is_the_generic_preset() {
        let job = OutputJob::fabrication(Id::random(), "x", Id::random);
        let preset = &presets()[0];
        let kinds: Vec<OutputKind> = job.outputs.iter().map(|o| o.kind).collect();
        let expected: Vec<OutputKind> = preset.outputs.iter().map(|o| o.1).collect();
        assert_eq!(kinds, expected);
        for (spec, (name, _, options)) in job.outputs.iter().zip(&preset.outputs) {
            assert_eq!(&spec.name, name);
            let options: BTreeMap<String, Value> = options
                .as_object()
                .map(|m| m.clone().into_iter().collect())
                .unwrap_or_default();
            assert_eq!(spec.options, options, "{name}");
        }
    }
}
