//! KiCad libraries: `.kicad_sym`, `.kicad_symdir`, `.kicad_mod` and
//! `.pretty`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ew_model::library::{FootprintDef, SymbolDef};

use crate::common::{Ids, derived_id};
use crate::error::KicadError;
use crate::footprint;
use crate::report::ImportReport;
use crate::sexpr::{self, Sexp};
use crate::symbol;

/// The symbols and footprints of one or more libraries.
#[derive(Debug, Default)]
pub struct Library {
    /// Symbols, named `nickname:name`.
    pub symbols: Vec<SymbolDef>,
    /// Footprints, named `nickname:name`.
    pub footprints: Vec<FootprintDef>,
}

pub(crate) fn read(path: &Path) -> Result<String, KicadError> {
    fs::read_to_string(path).map_err(|source| KicadError::Io {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn parse_file<'a>(path: &Path, text: &'a str) -> Result<Sexp<'a>, KicadError> {
    sexpr::parse(text).map_err(|source| KicadError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

/// Reads symbols from library roots, resolving `extends` (also across the
/// files of a `.kicad_symdir`).
fn symbols_of(
    roots: &[(PathBuf, Sexp<'_>)],
    nickname: &str,
    report: &mut ImportReport,
) -> Vec<SymbolDef> {
    let mut nodes: BTreeMap<String, (&Sexp<'_>, String)> = BTreeMap::new();
    for (path, root) in roots {
        if let Some(version) = root.value("version") {
            report
                .versions
                .entry("symbol library".to_owned())
                .or_insert_with(|| version.to_owned());
        }
        for node in root.all("symbol") {
            let name = node.arg(0).unwrap_or_default().to_owned();
            nodes.insert(name, (node, path.display().to_string()));
        }
    }
    let mut done: BTreeMap<String, SymbolDef> = BTreeMap::new();
    // Parents first; a chain of `extends` is resolved in as many rounds.
    for _ in 0..8 {
        let mut progressed = false;
        for (name, (node, file)) in &nodes {
            if done.contains_key(name) {
                continue;
            }
            let parent = node.value("extends");
            let base = match parent {
                Some(parent) => match done.get(parent) {
                    Some(base) => Some(base.clone()),
                    None if nodes.contains_key(parent) => continue,
                    None => {
                        report.warn(
                            "missing-parent",
                            Some(file),
                            format!("`{name}` extends `{parent}`, which is not in the library"),
                        );
                        None
                    }
                },
                None => None,
            };
            let full = format!("{nickname}:{name}");
            let id = derived_id(&["symbol-def", &full]);
            let def = symbol::symbol_def(node, id, &full, base.as_ref(), report, file);
            done.insert(name.clone(), def);
            progressed = true;
        }
        if !progressed {
            break;
        }
    }
    report.count("library symbols", done.len());
    done.into_values().collect()
}

fn footprint_def(
    path: &Path,
    nickname: &str,
    report: &mut ImportReport,
) -> Result<FootprintDef, KicadError> {
    let text = read(path)?;
    let root = parse_file(path, &text)?;
    let file = path.display().to_string();
    let mut ids = Ids::default();
    let fp = footprint::footprint(&root, 2, &mut ids, report, &file);
    if let Some(version) = root.value("version") {
        report
            .versions
            .entry("footprint".to_owned())
            .or_insert_with(|| version.to_owned());
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let base = if fp.name.is_empty() {
        stem
    } else {
        fp.name.rsplit(':').next().unwrap_or(stem)
    };
    let name = format!("{nickname}:{base}");
    Ok(FootprintDef {
        id: derived_id(&["footprint-def", &name]),
        pads: fp
            .pads
            .iter()
            .enumerate()
            .map(|(index, pad)| ew_model::pcb::Pad {
                id: derived_id(&["footprint-def", &name, "pad", &index.to_string()]),
                ..pad.clone()
            })
            .collect(),
        name,
        description: fp.description,
        keywords: fp.keywords,
        graphics: fp.graphics,
        texts: fp.texts,
        models: fp.models,
        attributes: fp.attributes,
    })
}

fn sorted_entries(dir: &Path, extension: &str) -> Result<Vec<PathBuf>, KicadError> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|source| KicadError::Io {
            path: dir.to_path_buf(),
            source,
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == extension))
        .collect();
    entries.sort();
    Ok(entries)
}

/// Imports a symbol library, a footprint or a footprint library.
/// `nickname` prefixes every name (default: the file name without
/// extension).
///
/// # Errors
///
/// Fails if the path is not a KiCad library or cannot be read.
pub fn import(
    path: &Path,
    nickname: Option<&str>,
    report: &mut ImportReport,
) -> Result<Library, KicadError> {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Library")
        .to_owned();
    let nickname = nickname.unwrap_or(&stem);
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    let mut out = Library::default();
    match extension {
        "kicad_sym" => {
            let text = read(path)?;
            let root = parse_file(path, &text)?;
            out.symbols = symbols_of(&[(path.to_path_buf(), root)], nickname, report);
        }
        "kicad_symdir" => {
            let files = sorted_entries(path, "kicad_sym")?;
            let texts: Vec<(PathBuf, String)> = files
                .iter()
                .map(|file| read(file).map(|text| (file.clone(), text)))
                .collect::<Result<_, _>>()?;
            let mut roots = Vec::new();
            for (file, text) in &texts {
                match sexpr::parse(text) {
                    Ok(root) => roots.push((file.clone(), root)),
                    Err(error) => report.error(
                        "unreadable-file",
                        Some(&file.display().to_string()),
                        error.to_string(),
                    ),
                }
            }
            out.symbols = symbols_of(&roots, nickname, report);
        }
        "kicad_mod" => out.footprints.push(footprint_def(path, nickname, report)?),
        "pretty" => {
            for file in sorted_entries(path, "kicad_mod")? {
                match footprint_def(&file, nickname, report) {
                    Ok(def) => out.footprints.push(def),
                    Err(error) => report.error(
                        "unreadable-file",
                        Some(&file.display().to_string()),
                        error.to_string(),
                    ),
                }
            }
        }
        _ => {
            return Err(KicadError::Unsupported {
                path: path.to_path_buf(),
                message: "not a KiCad library (.kicad_sym, .kicad_symdir, .kicad_mod or .pretty)"
                    .to_owned(),
            });
        }
    }
    report.count("library footprints", out.footprints.len());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_symbol_and_footprint_libraries() {
        let dir = tempfile::tempdir().unwrap();
        let sym = dir.path().join("Lab.kicad_sym");
        fs::write(
            &sym,
            r#"(kicad_symbol_lib (version 20251024) (generator "kicad_symbol_editor")
                (symbol "Small" (extends "Base") (property "Value" "Small" (at 0 0 0)))
                (symbol "Base" (property "Reference" "U" (at 0 0 0)) (property "Value" "Base" (at 0 0 0))
                    (symbol "Base_1_1" (pin input line (at 0 0 0) (length 2.54) (name "A") (number "1")))))"#,
        )
        .unwrap();
        let pretty = dir.path().join("Lab.pretty");
        fs::create_dir(&pretty).unwrap();
        fs::write(
            pretty.join("Pad.kicad_mod"),
            r#"(footprint "Pad" (version 20241229) (layer "F.Cu")
                (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu" "F.Mask")))"#,
        )
        .unwrap();
        fs::write(pretty.join("Broken.kicad_mod"), "(footprint").unwrap();
        let mut report = ImportReport::default();
        let symbols = import(&sym, None, &mut report).unwrap();
        assert_eq!(symbols.symbols.len(), 2);
        let small = symbols
            .symbols
            .iter()
            .find(|s| s.name == "Lab:Small")
            .unwrap();
        assert_eq!(small.value, "Small");
        assert_eq!(small.prefix, "U");
        assert_eq!(small.units[0].pins.len(), 1);
        let footprints = import(&pretty, Some("Mine"), &mut report).unwrap();
        assert_eq!(footprints.footprints.len(), 1);
        assert_eq!(footprints.footprints[0].name, "Mine:Pad");
        assert_eq!(report.total(crate::Severity::Error), 1);
        assert!(import(&dir.path().join("x.txt"), None, &mut report).is_err());
    }
}
