//! Eagle import (ADR-0015): Eagle XML schematics (`.sch`), boards (`.brd`)
//! and libraries (`.lbr`) — the format of Eagle 6 to 9 and of Fusion
//! electronics exports — become a native ElectronicWerk project with an
//! import report. Libraries go through the same duplicate check as KiCad
//! libraries.
//!
//! Eagle files are in millimetres with y pointing up, like ElectronicWerk.
//! Orientations (`MR90`) turn first and then mirror; mirrored parts on a
//! board sit on the bottom.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "Eagle stores millimetres as decimals; values are limited by the file format"
)]

pub mod board;
pub mod library;
pub mod schematic;
pub mod util;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ew_kicad::{ImportReport, Imported, KicadError};
use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::library::Library;
use ew_model::outputs::OutputJob;
use ew_model::project::{DocumentRef, Project};
use ew_model::schematic::{Schematic, SheetSymbol};
use ew_model::units::{Point, Size};

use crate::library::EagleLibrary;
use crate::util::{child, children, path};

/// Parses an Eagle XML file.
///
/// # Errors
///
/// Fails if the text is not XML or not an Eagle file.
pub fn parse<'a>(file: &Path, text: &'a str) -> Result<roxmltree::Document<'a>, KicadError> {
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..roxmltree::ParsingOptions::default()
    };
    let document = roxmltree::Document::parse_with_options(text, options).map_err(|error| {
        KicadError::Unsupported {
            path: file.to_path_buf(),
            message: format!("not an Eagle XML file: {error}"),
        }
    })?;
    if !document.root_element().has_tag_name("eagle") {
        return Err(KicadError::Unsupported {
            path: file.to_path_buf(),
            message: "not an Eagle file (no <eagle> element); Eagle files before version 6 are binary and cannot be read".to_owned(),
        });
    }
    Ok(document)
}

/// `true` if the text looks like an Eagle XML file.
#[must_use]
pub fn is_eagle(text: &str) -> bool {
    let head: String = text.chars().take(2048).collect();
    head.contains("<eagle")
}

/// The schematic and board of an Eagle project: `source` is either file
/// (or the folder with exactly one of them); the other one is the file of
/// the same name next to it.
///
/// # Errors
///
/// Fails if there is neither a schematic nor a board.
pub fn locate(source: &Path) -> Result<(Option<PathBuf>, Option<PathBuf>, String), KicadError> {
    let unsupported = |message: &str| KicadError::Unsupported {
        path: source.to_path_buf(),
        message: message.to_owned(),
    };
    let (folder, stem) = if source.is_dir() {
        let mut stems: Vec<String> = std::fs::read_dir(source)
            .map_err(|error| KicadError::Io {
                path: source.to_path_buf(),
                source: error,
            })?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| matches!(p.extension().and_then(|e| e.to_str()), Some("sch" | "brd")))
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect();
        stems.sort();
        stems.dedup();
        match stems.as_slice() {
            [stem] => (source.to_path_buf(), stem.clone()),
            [] => return Err(unsupported("the folder has no Eagle schematic or board")),
            _ => {
                return Err(unsupported(
                    "the folder has several Eagle projects; choose one file",
                ));
            }
        }
    } else {
        let folder = source
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        let stem = source
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .ok_or_else(|| unsupported("not an Eagle file"))?;
        (folder, stem)
    };
    let existing = |ext: &str| {
        let path = folder.join(format!("{stem}.{ext}"));
        path.exists().then_some(path)
    };
    let (sch, brd) = (existing("sch"), existing("brd"));
    if sch.is_none() && brd.is_none() {
        return Err(unsupported(
            "there is no Eagle schematic (.sch) or board (.brd)",
        ));
    }
    Ok((sch, brd, stem))
}

fn read(path: &Path) -> Result<String, KicadError> {
    let bytes = std::fs::read(path).map_err(|source| KicadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn add(project: &mut Project, document: Document, path: String) {
    let reference = DocumentRef {
        id: document.id(),
        kind: document.kind(),
        path,
    };
    let _ = project.add_document(reference, usize::MAX, document);
}

fn libraries_of(
    node: roxmltree::Node<'_, '_>,
    report: &mut ImportReport,
    file: &str,
) -> BTreeMap<String, EagleLibrary> {
    child(node, "libraries")
        .map(|list| {
            children(list, "library")
                .map(|lib| {
                    let read = library::read(lib, report, file);
                    (read.name.clone(), read)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Imports an Eagle project (schematic, board or both) as a new project.
///
/// # Errors
///
/// Fails if the files cannot be read or are not Eagle XML files.
#[allow(clippy::too_many_lines, reason = "the steps of one import")]
pub fn import_project(source: &Path, name: Option<&str>) -> Result<Imported, KicadError> {
    let (sch_path, brd_path, stem) = locate(source)?;
    let project_name = name.map_or_else(|| stem.clone(), str::to_owned);
    let mut report = ImportReport::new(
        sch_path
            .as_ref()
            .or(brd_path.as_ref())
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    );
    let mut project = Project::new(Id::derived(&["eagle-project", &stem]), project_name.clone());
    let mut library = Library::new(
        Id::derived(&["eagle-project-library", &stem]),
        "Project library",
    );

    if let Some(path) = &sch_path {
        let file = format!("{stem}.sch");
        let text = read(path)?;
        let document = parse(path, &text)?;
        let root = document.root_element();
        if let Some(version) = root.attribute("version") {
            report
                .versions
                .insert("schematic".to_owned(), version.to_owned());
        }
        let schematic = path_or(root, &["drawing", "schematic"], path)?;
        let libraries = libraries_of(schematic, &mut report, &file);
        let parts = schematic::parts(schematic);
        let mut sheets = schematic::sheets(
            schematic,
            &stem,
            &schematic::Context {
                libraries: &libraries,
                parts: &parts,
                file: &file,
            },
            &mut report,
        );
        report.count("sheets", sheets.len());
        report.count("symbols", parts.len());
        for lib in libraries.values() {
            for device in lib.devices.values() {
                library.symbols.insert(device.def.clone());
            }
            for def in lib.packages.values() {
                if library.footprints.iter().all(|f| f.name != def.name) {
                    library.footprints.insert(def.clone());
                }
            }
        }
        // Several sheets: a root sheet with a sheet symbol for each (Eagle
        // sheets are flat; their nets meet through global labels).
        let root_id = if sheets.len() > 1 {
            let mut root_sheet =
                Schematic::new(Id::derived(&["eagle-root", &stem]), project_name.clone());
            root_sheet.title_block.title.clone_from(&project_name);
            for (index, sheet) in sheets.iter().enumerate() {
                let column = i64::try_from(index % 4).unwrap_or(0);
                let row = i64::try_from(index / 4).unwrap_or(0);
                root_sheet.sheets.insert(SheetSymbol {
                    id: Id::derived(&["eagle-sheet-symbol", &stem, &index.to_string()]),
                    name: sheet.name.clone(),
                    schematic: sheet.id,
                    at: Point::new(
                        25_400_000 + column * 66_040_000,
                        170_180_000 - row * 45_720_000,
                    ),
                    size: Size::new(50_800_000, 30_480_000),
                    pins: Vec::new(),
                });
            }
            let id = root_sheet.id;
            sheets.insert(0, root_sheet);
            id
        } else {
            sheets.first().map_or(Id::NIL, |s| s.id)
        };
        project.manifest.root_schematic = (root_id != Id::NIL).then_some(root_id);
        for (index, sheet) in sheets.into_iter().enumerate() {
            let file_name = if index == 0 {
                "root".to_owned()
            } else {
                format!("sheet-{index}")
            };
            add(
                &mut project,
                Document::Schematic(sheet),
                format!("schematics/{file_name}.ewsch"),
            );
        }
    } else {
        report.info("no-schematic", None, "the project has no schematic");
    }

    let mut rules = None;
    if let Some(path) = &brd_path {
        let file = format!("{stem}.brd");
        let text = read(path)?;
        let document = parse(path, &text)?;
        let root = document.root_element();
        if let Some(version) = root.attribute("version") {
            report
                .versions
                .insert("board".to_owned(), version.to_owned());
        }
        let board_node = path_or(root, &["drawing", "board"], path)?;
        let libraries = libraries_of(board_node, &mut report, &file);
        for lib in libraries.values() {
            for def in lib.packages.values() {
                if library.footprints.iter().all(|f| f.name != def.name) {
                    library.footprints.insert(def.clone());
                }
            }
        }
        let board = board::read(board_node, &project_name, &libraries, &mut report, &file);
        rules = Some(board.rules);
        add(
            &mut project,
            Document::Pcb(board.pcb),
            format!("pcb/{}.ewpcb", ew_safe(&stem)),
        );
    } else {
        report.info("no-board", None, "the project has no board");
    }

    add(
        &mut project,
        Document::Library(library),
        "library/project.ewlib".to_owned(),
    );
    if sch_path.is_some() && brd_path.is_some() {
        link_board(&mut project);
    }
    let rules = rules.unwrap_or_else(|| {
        ew_model::rules::Rules::new(Id::derived(&["eagle-rules", &stem]), "Design rules")
    });
    add(
        &mut project,
        Document::Rules(rules),
        "rules/design.ewrules".to_owned(),
    );
    let mut counter = 0_u32;
    let job = OutputJob::fabrication(
        Id::derived(&["eagle-outputs", &stem]),
        "Fabrication",
        || {
            counter += 1;
            Id::derived(&["eagle-output", &stem, &counter.to_string()])
        },
    );
    add(
        &mut project,
        Document::OutputJob(job),
        "outputs/fabrication.ewout".to_owned(),
    );
    report.finish();
    Ok(Imported { project, report })
}

/// Brings the board in line with the schematic the way the board update
/// sees it: every footprint carries the link of its part, and every board
/// net the schematic's name of the same pins.
fn link_board(project: &mut Project) {
    let connectivity = ew_schematic::connectivity::analyze(project);
    let (parts, _) = ew_pcb::sync::parts(project, &connectivity);
    let Some(pcb_id) = project
        .documents
        .iter()
        .find_map(|(id, doc)| matches!(doc, Document::Pcb(_)).then_some(*id))
    else {
        return;
    };
    let Some(Document::Pcb(pcb)) = project.documents.get_mut(&pcb_id) else {
        return;
    };
    // Board net → schematic name, from the pins each part's pads stand for.
    let mut names: BTreeMap<Id, BTreeMap<String, usize>> = BTreeMap::new();
    for part in &parts {
        let Some(fp) = pcb
            .footprints
            .iter_mut()
            .find(|f| f.designator == part.designator)
        else {
            continue;
        };
        fp.component = Some(part.key);
        for (pin, net) in &part.pins {
            let (Some(net), Some(pad)) = (net, fp.pads.iter().find(|p| &p.number == pin)) else {
                continue;
            };
            if let Some(board_net) = pad.net {
                *names
                    .entry(board_net)
                    .or_default()
                    .entry(net.clone())
                    .or_default() += 1;
            }
        }
    }
    let taken: Vec<String> = pcb.nets.iter().map(|n| n.name.clone()).collect();
    for net in &mut pcb.nets {
        let Some(votes) = names.get(&net.id) else {
            continue;
        };
        // The name most pins agree on, if no other board net has it.
        if let Some((name, _)) = votes.iter().max_by_key(|(_, count)| **count) {
            if *name != net.name && !taken.contains(name) {
                net.name.clone_from(name);
            }
        }
    }
}

fn path_or<'a, 'input>(
    node: roxmltree::Node<'a, 'input>,
    tags: &[&str],
    file: &Path,
) -> Result<roxmltree::Node<'a, 'input>, KicadError> {
    path(node, tags).ok_or_else(|| KicadError::Unsupported {
        path: file.to_path_buf(),
        message: format!("the file has no <{}>", tags.join("/")),
    })
}

/// A name usable in file names.
fn ew_safe(name: &str) -> String {
    let out: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    if out.is_empty() {
        "board".to_owned()
    } else {
        out
    }
}
