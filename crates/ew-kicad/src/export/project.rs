//! A whole project as a KiCad project: `.kicad_pro`, one `.kicad_sch` per
//! sheet, the board, the project libraries and their library tables.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::library::{FootprintDef, SymbolDef};
use ew_model::pcb::Pcb;
use ew_model::project::Project;
use ew_model::rules::Rules;
use ew_model::schematic::Schematic;
use serde_json::{Map, Value, json};

use super::board::{board_file, footprint_file, natural};
use super::schematic::{SheetContext, library_file, power_symbols, sheet_file};
use super::{ExportReport, Title};

/// A written file, relative to the export folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFile {
    /// Path with `/` separators.
    pub path: String,
    /// Content.
    pub data: Vec<u8>,
}

/// The files of a KiCad project and what could not be carried over.
#[derive(Debug, Clone, Default)]
pub struct KicadProject {
    /// The files to write.
    pub files: Vec<ExportFile>,
    /// What was left out or changed.
    pub report: ExportReport,
}

/// A name that is safe as a KiCad file name and library nickname.
#[must_use]
pub fn file_stem(name: &str) -> String {
    let stem: String = name
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
    if stem.is_empty() {
        "project".to_owned()
    } else {
        stem
    }
}

fn mm(value: i64) -> Value {
    json!(value as f64 / 1e6)
}

/// The `.kicad_pro` file: net classes, their patterns, the board minimums
/// and the text variables.
fn project_json(project: &Project, rules: Option<&Rules>, stem: &str) -> String {
    let mut root = Map::new();
    root.insert(
        "meta".to_owned(),
        json!({"filename": format!("{stem}.kicad_pro"), "version": 1}),
    );
    if let Some(rules) = rules {
        let classes: Vec<Value> = rules
            .net_classes
            .iter()
            .map(|class| {
                json!({
                    "name": class.name,
                    "description": class.description,
                    "clearance": mm(class.clearance),
                    "track_width": mm(class.track_width),
                    "via_diameter": mm(class.via_diameter),
                    "via_drill": mm(class.via_drill),
                    "microvia_diameter": mm(class.micro_via_diameter),
                    "microvia_drill": mm(class.micro_via_drill),
                    "diff_pair_width": mm(class.diff_pair_width),
                    "diff_pair_gap": mm(class.diff_pair_gap),
                    "diff_pair_via_gap": 0.25,
                    "wire_width": 6,
                    "bus_width": 12,
                    "line_style": 0,
                    "pcb_color": "rgba(0, 0, 0, 0.000)",
                    "schematic_color": "rgba(0, 0, 0, 0.000)",
                    "priority": 2_147_483_647,
                })
            })
            .collect();
        let patterns: Vec<Value> = rules
            .assignments
            .iter()
            .map(|a| json!({"netclass": a.class, "pattern": a.pattern}))
            .collect();
        root.insert(
            "net_settings".to_owned(),
            json!({"classes": classes, "meta": {"version": 3}, "netclass_patterns": patterns}),
        );
        let c = &rules.constraints;
        root.insert(
            "board".to_owned(),
            json!({"design_settings": {"rules": {
                "min_clearance": mm(c.min_clearance),
                "min_track_width": mm(c.min_track_width),
                "min_via_diameter": mm(c.min_via_diameter),
                "min_through_hole_diameter": mm(c.min_hole),
                "min_via_annular_width": mm(c.min_annular_ring),
                "min_hole_to_hole": mm(c.min_hole_to_hole),
                "min_copper_edge_clearance": mm(c.min_copper_to_edge),
                "min_text_thickness": mm(c.min_silk_width),
                "allow_blind_buried_vias": c.allow_blind_buried_vias,
                "allow_microvias": c.allow_micro_vias,
                "min_microvia_diameter": mm(c.min_micro_via_diameter),
                "min_microvia_drill": mm(c.min_micro_via_drill),
            }}}),
        );
    }
    root.insert(
        "text_variables".to_owned(),
        Value::Object(
            project
                .manifest
                .variables
                .iter()
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect(),
        ),
    );
    let mut text = serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_default();
    text.push('\n');
    text
}

/// One use of a sheet: its KiCad path, page number and the sheet symbol
/// ids after the root.
type Instance = (String, String, Vec<Id>);

/// The uses of each sheet document.
type Uses = BTreeMap<Id, Vec<Instance>>;
/// The parent path and page number of each sheet symbol.
type Pages = BTreeMap<Id, Vec<(String, String)>>;

/// Every use of every sheet, from the root down (breadth first, as KiCad
/// numbers its pages), and the pages of each sheet symbol.
fn instances(root: &Schematic, sheets: &BTreeMap<Id, &Schematic>) -> (Uses, Pages) {
    let mut uses: BTreeMap<Id, Vec<Instance>> = BTreeMap::new();
    let mut child_pages: BTreeMap<Id, Vec<(String, String)>> = BTreeMap::new();
    let mut queue: VecDeque<(Id, String, Vec<Id>)> = VecDeque::new();
    let mut page = 1;
    uses.entry(root.id)
        .or_default()
        .push((format!("/{}", root.id), "1".to_owned(), Vec::new()));
    queue.push_back((root.id, format!("/{}", root.id), Vec::new()));
    let mut guard = 0;
    while let Some((doc, path, chain)) = queue.pop_front() {
        guard += 1;
        if guard > 10_000 {
            break;
        }
        let Some(sheet) = sheets.get(&doc) else {
            continue;
        };
        let mut children: Vec<_> = sheet.sheets.iter().collect();
        children.sort_by(|a, b| natural(&a.name, &b.name));
        for child in children {
            if chain.contains(&child.id) {
                continue;
            }
            page += 1;
            let child_path = format!("{path}/{}", child.id);
            let mut child_chain = chain.clone();
            child_chain.push(child.id);
            child_pages
                .entry(child.id)
                .or_default()
                .push((path.clone(), page.to_string()));
            uses.entry(child.schematic).or_default().push((
                child_path.clone(),
                page.to_string(),
                child_chain.clone(),
            ));
            queue.push_back((child.schematic, child_path, child_chain));
        }
    }
    (uses, child_pages)
}

/// Exports a project. `version` is the program version for the file
/// headers.
#[allow(clippy::too_many_lines, reason = "the steps of one export")]
#[must_use]
pub fn export_project(project: &Project, version: &str) -> KicadProject {
    let mut report = ExportReport::default();
    let stem = file_stem(&project.manifest.name);
    let mut files = Vec::new();

    // Libraries: symbols and footprints of every project library.
    let mut symbols: BTreeMap<String, SymbolDef> = BTreeMap::new();
    let mut footprints: Vec<FootprintDef> = Vec::new();
    let mut rules: Option<&Rules> = None;
    let mut schematics: BTreeMap<Id, &Schematic> = BTreeMap::new();
    let mut boards: Vec<&Pcb> = Vec::new();
    for document in project.documents.values() {
        match document {
            Document::Library(library) => {
                for def in &library.symbols {
                    symbols
                        .entry(def.name.clone())
                        .or_insert_with(|| def.clone());
                }
                footprints.extend(library.footprints.iter().cloned());
            }
            Document::Rules(r) => rules = rules.or(Some(r)),
            Document::Schematic(s) => {
                schematics.insert(s.id, s);
            }
            Document::Pcb(pcb) => boards.push(pcb),
            Document::OutputJob(_) => {}
        }
    }
    let lib_id = |name: &str| {
        if name.contains(':') {
            name.to_owned()
        } else {
            format!("{stem}:{name}")
        }
    };
    let lib_ids: BTreeMap<String, String> = symbols
        .keys()
        .map(|name| (name.clone(), lib_id(name)))
        .collect();
    let mut defs: BTreeMap<String, SymbolDef> = symbols
        .values()
        .map(|def| (lib_id(&def.name), def.clone()))
        .collect();

    // Sheets: the root first, then every sheet reached from it; sheets
    // nobody uses are written as their own pages too.
    let root = project
        .manifest
        .root_schematic
        .and_then(|id| schematics.get(&id).copied())
        .or_else(|| schematics.values().next().copied());
    let mut sheet_files: BTreeMap<Id, String> = BTreeMap::new();
    let mut used_names: BTreeSet<String> = BTreeSet::new();
    if let Some(root) = root {
        sheet_files.insert(root.id, format!("{stem}.kicad_sch"));
        used_names.insert(format!("{stem}.kicad_sch").to_lowercase());
    }
    let mut ordered: Vec<&Schematic> = schematics.values().copied().collect();
    ordered.sort_by(|a, b| natural(&a.name, &b.name));
    for sheet in &ordered {
        if sheet_files.contains_key(&sheet.id) {
            continue;
        }
        let base = file_stem(&sheet.name);
        let mut name = format!("{base}.kicad_sch");
        let mut n = 2;
        while used_names.contains(&name.to_lowercase()) {
            name = format!("{base}_{n}.kicad_sch");
            n += 1;
        }
        used_names.insert(name.to_lowercase());
        sheet_files.insert(sheet.id, name);
    }
    let (uses, child_pages) = match root {
        Some(root) => instances(root, &schematics),
        None => (BTreeMap::new(), BTreeMap::new()),
    };
    let all: Vec<&Schematic> = ordered.clone();
    let power = power_symbols(&all);
    for (name, def) in power.values() {
        defs.insert(name.clone(), def.clone());
    }
    let mut power_refs: BTreeMap<Id, String> = BTreeMap::new();
    for sheet in &ordered {
        for port in &sheet.power_ports {
            power_refs.insert(port.id, format!("#PWR{:02}", power_refs.len() + 1));
        }
    }

    // Board ↔ schematic: a footprint names the path of its symbol.
    let mut component_paths: BTreeMap<Id, String> = BTreeMap::new();
    for sheet in &ordered {
        let chain = uses
            .get(&sheet.id)
            .and_then(|list| list.first())
            .map(|(_, _, chain)| chain.clone())
            .unwrap_or_default();
        for symbol in &sheet.symbols {
            if let Some(component) = symbol.component {
                let mut path = String::new();
                for id in &chain {
                    path.push('/');
                    path.push_str(&id.to_string());
                }
                path.push('/');
                path.push_str(&symbol.id.to_string());
                component_paths.entry(component).or_insert(path);
            }
        }
    }

    for sheet in &ordered {
        let Some(file) = sheet_files.get(&sheet.id) else {
            continue;
        };
        let empty = Vec::new();
        let paths = uses.get(&sheet.id).unwrap_or(&empty);
        if paths.is_empty() && root.is_some_and(|r| r.id != sheet.id) {
            report.note(
                "sheet",
                format!(
                    "the sheet `{}` is not placed in the hierarchy; it was written as {file} but KiCad only shows it once it is placed",
                    sheet.name
                ),
            );
        }
        let text = sheet_file(
            sheet,
            &SheetContext {
                project: &stem,
                lib_ids: &lib_ids,
                defs: &defs,
                power: &power,
                paths,
                files: &sheet_files,
                child_pages: &child_pages,
                root: root.is_some_and(|r| r.id == sheet.id),
                power_refs: &power_refs,
            },
            &mut report,
        );
        files.push(ExportFile {
            path: file.clone(),
            data: text.into_bytes(),
        });
    }

    // Boards: the first is the project's board.
    let title = Title {
        project: &project.manifest.name,
        revision: &project.manifest.revision,
        date: "",
        company: "",
        library: &stem,
        version,
    };
    for (index, pcb) in boards.iter().enumerate() {
        let file = if index == 0 {
            format!("{stem}.kicad_pcb")
        } else {
            report.note(
                "board",
                format!(
                    "KiCad projects have one board: `{}` was written as its own file",
                    pcb.name
                ),
            );
            format!("{stem}-{}.kicad_pcb", file_stem(&pcb.name))
        };
        let text = board_file(
            pcb,
            &title,
            &|component| component_paths.get(&component).cloned(),
            &mut report,
        );
        files.push(ExportFile {
            path: file,
            data: text.into_bytes(),
        });
    }

    // The project's own libraries (symbols and footprints without a
    // library nickname) and their tables.
    let own_symbols: Vec<(String, &SymbolDef)> = symbols
        .values()
        .filter(|def| !def.name.contains(':'))
        .map(|def| (def.name.clone(), def))
        .collect();
    let own_footprints: Vec<&FootprintDef> = footprints
        .iter()
        .filter(|def| !def.name.contains(':'))
        .collect();
    if !own_symbols.is_empty() {
        files.push(ExportFile {
            path: format!("{stem}.kicad_sym"),
            data: library_file(&own_symbols).into_bytes(),
        });
        files.push(ExportFile {
            path: "sym-lib-table".to_owned(),
            data: lib_table("sym_lib_table", &stem, &format!("{stem}.kicad_sym")).into_bytes(),
        });
    }
    if !own_footprints.is_empty() {
        let mut names = BTreeSet::new();
        for def in own_footprints {
            let base = file_stem(&def.name);
            if !names.insert(base.to_lowercase()) {
                continue;
            }
            files.push(ExportFile {
                path: format!("{stem}.pretty/{base}.kicad_mod"),
                data: footprint_file(def, &mut report).into_bytes(),
            });
        }
        files.push(ExportFile {
            path: "fp-lib-table".to_owned(),
            data: lib_table("fp_lib_table", &stem, &format!("{stem}.pretty")).into_bytes(),
        });
    }
    files.push(ExportFile {
        path: format!("{stem}.kicad_pro"),
        data: project_json(project, rules, &stem).into_bytes(),
    });
    KicadProject { files, report }
}

fn lib_table(kind: &str, name: &str, file: &str) -> String {
    format!(
        "({kind}\n\t(version 7)\n\t(lib (name \"{name}\") (type \"KiCad\") (uri \"${{KIPRJMOD}}/{file}\") (options \"\") (descr \"Project library (ElectronicWerk)\"))\n)\n"
    )
}
