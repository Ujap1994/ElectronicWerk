//! Imports a whole KiCad project as an ElectronicWerk project.

use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::library::Library;
use ew_model::outputs::OutputJob;
use ew_model::project::{DocumentRef, Project};
use ew_model::rules::Rules;
use ew_model::schematic::Schematic;

use crate::common::{Ids, derived_id};
use crate::error::KicadError;
use crate::library::{self, parse_file, read};
use crate::pcb;
use crate::project::{self, LibraryEntry};
use crate::report::ImportReport;
use crate::schematic::{self, SymbolTable};

/// An imported project and its report.
#[derive(Debug)]
pub struct Imported {
    /// The project, ready to be written with `ew_format::store`.
    pub project: Project,
    /// What was imported and what was not.
    pub report: ImportReport,
}

/// Finds the `.kicad_pro` (or the design files) behind `source`: a project
/// file, a schematic or board, or a folder with exactly one project.
///
/// # Errors
///
/// Fails if there is no KiCad project.
pub fn locate(source: &Path) -> Result<(PathBuf, String), KicadError> {
    let unsupported = |message: &str| KicadError::Unsupported {
        path: source.to_path_buf(),
        message: message.to_owned(),
    };
    if source.is_dir() {
        let mut projects: Vec<PathBuf> = fs::read_dir(source)
            .map_err(|error| KicadError::Io {
                path: source.to_path_buf(),
                source: error,
            })?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|e| e == "kicad_pro"))
            .collect();
        projects.sort();
        return match projects.as_slice() {
            [single] => locate(single),
            [] => Err(unsupported(
                "the folder contains no KiCad project (.kicad_pro)",
            )),
            _ => Err(unsupported(
                "the folder contains several KiCad projects; choose one",
            )),
        };
    }
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    if !matches!(extension, "kicad_pro" | "kicad_sch" | "kicad_pcb") {
        return Err(unsupported("not a KiCad project, schematic or board"));
    }
    let folder = source
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| unsupported("invalid file name"))?
        .to_owned();
    if !folder.join(format!("{stem}.kicad_sch")).exists()
        && !folder.join(format!("{stem}.kicad_pcb")).exists()
    {
        return Err(unsupported(
            "the project has neither a schematic nor a board",
        ));
    }
    Ok((folder, stem))
}

fn add(project: &mut Project, document: Document, path: String) {
    let reference = DocumentRef {
        id: document.id(),
        kind: document.kind(),
        path,
    };
    // Paths are made unique below, so this cannot fail.
    let _ = project.add_document(reference, usize::MAX, document);
}

fn unique(used: &mut Vec<String>, folder: &str, stem: &str, extension: &str) -> String {
    let clean: String = stem
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let mut candidate = format!("{folder}/{clean}.{extension}");
    let mut n = 2;
    while used.contains(&candidate) {
        candidate = format!("{folder}/{clean}-{n}.{extension}");
        n += 1;
    }
    used.push(candidate.clone());
    candidate
}

/// Resolves `${KIPRJMOD}` in a library table entry; other paths stay
/// outside the project.
fn project_library(folder: &Path, entry: &LibraryEntry) -> Option<PathBuf> {
    let rest = entry.uri.strip_prefix("${KIPRJMOD}")?;
    Some(folder.join(rest.trim_start_matches(['/', '\\'])))
}

fn import_tables(folder: &Path, library: &mut Library, report: &mut ImportReport) {
    for (table, kind) in [("sym-lib-table", "symbol"), ("fp-lib-table", "footprint")] {
        let path = folder.join(table);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(root) = crate::sexpr::parse(&text) else {
            report.warn(
                "unreadable-file",
                Some(table),
                "the library table could not be read",
            );
            continue;
        };
        for entry in project::library_table(&root) {
            let Some(lib_path) = project_library(folder, &entry) else {
                report.info(
                    "external-library",
                    Some(table),
                    format!(
                        "the {kind} library `{}` is outside the project; the {kind}s the design uses were imported from the design itself",
                        entry.name
                    ),
                );
                continue;
            };
            match library::import(&lib_path, Some(&entry.name), report) {
                Ok(imported) => {
                    for symbol in imported.symbols {
                        if library.symbol(&symbol.name).is_none() {
                            library.symbols.insert(symbol);
                        }
                    }
                    for footprint in imported.footprints {
                        if library.footprint(&footprint.name).is_none() {
                            library.footprints.insert(footprint);
                        }
                    }
                }
                Err(error) => report.error("unreadable-file", Some(table), error.to_string()),
            }
        }
    }
}

/// Imports the KiCad project at `source` (a `.kicad_pro`, `.kicad_sch` or
/// `.kicad_pcb` file, or a folder with one project). `name` overrides the
/// project name.
///
/// # Errors
///
/// Fails if there is no KiCad project or its main files cannot be parsed.
/// Problems with single objects or sub-sheets end up in the report.
#[allow(clippy::too_many_lines, reason = "the import steps in order")]
pub fn import_project(source: &Path, name: Option<&str>) -> Result<Imported, KicadError> {
    let (folder, stem) = locate(source)?;
    let project_name = name.map_or_else(|| stem.clone(), str::to_owned);
    let mut report = ImportReport::new(
        folder
            .join(format!("{stem}.kicad_pro"))
            .display()
            .to_string(),
    );
    let mut ids = Ids::default();
    let mut project = Project::new(derived_id(&["project", &stem]), project_name.clone());
    let mut used_paths: Vec<String> = Vec::new();

    // Project settings.
    let pro_path = folder.join(format!("{stem}.kicad_pro"));
    let settings = if let Ok(text) = fs::read_to_string(&pro_path) {
        serde_json::from_str::<serde_json::Value>(&text).map_or_else(
            |error| {
                report.error(
                    "unreadable-file",
                    Some(&format!("{stem}.kicad_pro")),
                    error.to_string(),
                );
                project::ProjectSettings::default()
            },
            |json| project::settings(&json),
        )
    } else {
        report.info(
            "no-project-file",
            None,
            "there is no .kicad_pro file; default settings are used",
        );
        project::ProjectSettings::default()
    };
    project.manifest.variables = settings.variables.clone();

    // Schematics, breadth first from the root sheet.
    let mut table = SymbolTable::default();
    let root_file = format!("{stem}.kicad_sch");
    let mut documents: BTreeMap<String, Id> = BTreeMap::new();
    let mut sheets: BTreeMap<String, Schematic> = BTreeMap::new();
    let mut order: Vec<String> = Vec::new();
    let mut links: Vec<(String, Id, String)> = Vec::new();
    if folder.join(&root_file).exists() {
        let root_path = folder.join(&root_file);
        let root_text = read(&root_path)?;
        let root = parse_file(&root_path, &root_text)?;
        let legacy = schematic::legacy_instances(&root);
        let mut queue: VecDeque<(String, String)> = VecDeque::new();
        queue.push_back((root_file.clone(), String::new()));
        while let Some((file, sheet_name)) = queue.pop_front() {
            if documents.contains_key(&file) {
                continue;
            }
            let id = derived_id(&["sheet-doc", &file]);
            documents.insert(file.clone(), id);
            let path = folder.join(&file);
            let text = match read(&path) {
                Ok(text) => text,
                Err(error) => {
                    report.error("missing-sheet", Some(&file), error.to_string());
                    let name = if sheet_name.is_empty() {
                        file.clone()
                    } else {
                        sheet_name
                    };
                    sheets.insert(file.clone(), Schematic::new(id, name));
                    order.push(file);
                    continue;
                }
            };
            let node = match crate::sexpr::parse(&text) {
                Ok(node) => node,
                Err(error) => {
                    report.error("unreadable-file", Some(&file), error.to_string());
                    sheets.insert(file.clone(), Schematic::new(id, sheet_name.clone()));
                    order.push(file);
                    continue;
                }
            };
            let title = node
                .get("title_block")
                .and_then(|tb| tb.value("title"))
                .unwrap_or_default()
                .to_owned();
            let display = if !sheet_name.is_empty() {
                sheet_name
            } else if !title.is_empty() {
                title
            } else {
                file.trim_end_matches(".kicad_sch").to_owned()
            };
            let (sheet, extras) = schematic::sheet(
                &node,
                id,
                &display,
                &mut ids,
                &mut table,
                &legacy,
                &mut report,
                &file,
            );
            for child in extras.children {
                links.push((file.clone(), child.symbol, child.file.clone()));
                queue.push_back((child.file, child.name));
            }
            sheets.insert(file.clone(), sheet);
            order.push(file);
        }
        report.count("sheets", sheets.len());
    } else {
        report.info("no-schematic", None, "the project has no schematic");
    }
    for (parent, symbol, child) in &links {
        let target = documents.get(child).copied();
        if let (Some(sheet), Some(target)) = (sheets.get_mut(parent), target) {
            if let Some(symbol) = sheet.sheets.get_mut(*symbol) {
                symbol.schematic = target;
            }
        }
    }
    for file in &order {
        if let Some(sheet) = sheets.remove(file) {
            if file == &root_file {
                project.manifest.root_schematic = Some(sheet.id);
                if !sheet.title_block.revision.contains("${") {
                    project
                        .manifest
                        .revision
                        .clone_from(&sheet.title_block.revision);
                }
            }
            let path = unique(
                &mut used_paths,
                "schematics",
                file.trim_end_matches(".kicad_sch"),
                "ewsch",
            );
            add(&mut project, Document::Schematic(sheet), path);
        }
    }

    // The board.
    let mut library = Library::new(derived_id(&["library", &stem]), "Project library");
    let board_file = format!("{stem}.kicad_pcb");
    let board_path = folder.join(&board_file);
    if board_path.exists() {
        let text = read(&board_path)?;
        let node = parse_file(&board_path, &text)?;
        let board = pcb::board(&node, &project_name, &mut ids, &mut report, &board_file);
        for def in board.footprints {
            library.footprints.insert(def);
        }
        let path = unique(&mut used_paths, "pcb", &stem, "ewpcb");
        add(&mut project, Document::Pcb(board.pcb), path);
    } else {
        report.info("no-board", None, "the project has no board");
    }

    // The project library: the symbols of every sheet, the footprints of the
    // board, and the project's own libraries.
    for def in table.defs.into_values() {
        library.symbols.insert(def);
    }
    import_tables(&folder, &mut library, &mut report);
    let library_path = unique(&mut used_paths, "library", "project", "ewlib");
    add(&mut project, Document::Library(library), library_path);

    // Rules and outputs.
    let mut rules = Rules::new(derived_id(&["rules", &stem]), "Design rules");
    if let Some(constraints) = settings.constraints {
        rules.constraints = constraints;
    }
    if !settings.net_classes.is_empty() {
        rules.net_classes = settings.net_classes;
    }
    rules.assignments = settings.assignments;
    if let Ok(text) = fs::read_to_string(folder.join(format!("{stem}.kicad_dru"))) {
        let count = project::custom_rule_count(&text);
        if count > 0 {
            report.warn(
                "custom-rules",
                Some(&format!("{stem}.kicad_dru")),
                format!("{count} custom design rules are not imported; the rule editor arrives with the layout editor"),
            );
        }
    }
    let rules_path = unique(&mut used_paths, "rules", "design", "ewrules");
    add(&mut project, Document::Rules(rules), rules_path);
    let mut counter = 0_u32;
    let job = OutputJob::fabrication(derived_id(&["outputs", &stem]), "Fabrication", || {
        counter += 1;
        derived_id(&["output", &stem, &counter.to_string()])
    });
    let job_path = unique(&mut used_paths, "outputs", "fabrication", "ewout");
    add(&mut project, Document::OutputJob(job), job_path);

    report.finish();
    Ok(Imported { project, report })
}
