//! `ewerk library`: build, search and distribute the base library.

use std::io::Write;
use std::process::ExitCode;

use ew_core::Engine;
use serde_json::{Value, json};

use crate::commands::{Failure, Outcome, call, print_json};
use crate::{LibraryCommand, LibraryDir};

fn engine(target: &LibraryDir) -> Result<Engine, Failure> {
    let mut engine = Engine::without_journal();
    if let Some(dir) = &target.dir {
        call(&mut engine, "library.open", json!({"dir": dir}))?;
    }
    Ok(engine)
}

fn print_status(status: &Value, out: &mut impl Write) -> std::io::Result<()> {
    let Some(index) = status.get("status").filter(|s| !s.is_null()) else {
        return writeln!(
            out,
            "No library in {}",
            status["dir"].as_str().unwrap_or("?")
        );
    };
    writeln!(out, "Library:  {}", index["path"].as_str().unwrap_or("?"))?;
    for source in index["sources"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "Source:   {} {} ({})",
            source["name"].as_str().unwrap_or(""),
            source["version"].as_str().unwrap_or(""),
            source["license"].as_str().unwrap_or("")
        )?;
    }
    for kind in ["symbol", "footprint", "model"] {
        writeln!(
            out,
            "{:<10}{} ({} shown, the others merged into duplicates)",
            format!("{kind}s:"),
            index["counts"][kind],
            index["visible"][kind]
        )?;
    }
    writeln!(
        out,
        "Groups:   {} exact, {} near ({} to review), {} alias",
        index["groups"]["exact"],
        index["groups"]["near"],
        index["pending"],
        index["groups"]["alias"]
    )?;
    writeln!(out, "Meshes:   {}", index["meshes"])
}

#[allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    reason = "one arm per command; sizes are printed in MB"
)]
pub fn run(command: LibraryCommand, out: &mut impl Write) -> Outcome {
    match command {
        LibraryCommand::Build {
            kicad,
            symbols,
            footprints,
            models,
            meshes,
            version,
            license_file,
            target,
        } => {
            let mut engine = engine(&target)?;
            let license = match license_file {
                Some(path) => Some(
                    std::fs::read_to_string(&path)
                        .map_err(|error| Failure(format!("{}: {error}", path.display())))?,
                ),
                None => None,
            };
            let mut source = json!({
                "kicad": kicad, "symbols": symbols, "footprints": footprints,
                "models": models, "meshes": meshes, "version": version,
            });
            if let Some(object) = source.as_object_mut() {
                object.retain(|_, value| !value.is_null() && value != "");
            }
            let started = std::time::Instant::now();
            let result = call(
                &mut engine,
                "library.build",
                json!({"sources": [source], "license": license, "wait": true}),
            )?;
            if target.json {
                print_json(out, &result)?;
            } else {
                writeln!(out, "Built in {:.1} s", started.elapsed().as_secs_f64())?;
                print_status(&result["status"], out)?;
                // Findings by severity and code (the files are in --json).
                let mut totals: std::collections::BTreeMap<(String, String), (u64, String)> =
                    std::collections::BTreeMap::new();
                for finding in result["problems"]["findings"]
                    .as_array()
                    .into_iter()
                    .flatten()
                {
                    let key = (
                        finding["severity"].as_str().unwrap_or("").to_owned(),
                        finding["code"].as_str().unwrap_or("").to_owned(),
                    );
                    let entry = totals
                        .entry(key)
                        .or_insert((0, finding["message"].as_str().unwrap_or("").to_owned()));
                    entry.0 += finding["count"].as_u64().unwrap_or(1);
                }
                for ((severity, code), (count, message)) in totals {
                    writeln!(out, "{severity:<7} {count}× {message} [{code}]")?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        LibraryCommand::Info { target } => {
            let mut engine = engine(&target)?;
            let status = call(&mut engine, "library.status", Value::Null)?;
            if target.json {
                print_json(out, &status)?;
            } else {
                print_status(&status, out)?;
            }
            Ok(if status["installed"] == true {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        LibraryCommand::Search {
            text,
            kind,
            library,
            limit,
            target,
        } => {
            let mut engine = engine(&target)?;
            let mut query = json!({"text": text, "limit": limit});
            if let Some(kind) = kind {
                query["kind"] = Value::String(kind);
            }
            if let Some(library) = library {
                query["libraries"] = json!([library]);
            }
            let result = call(&mut engine, "library.search", query)?;
            if target.json {
                print_json(out, &result)?;
            } else {
                for item in result["items"].as_array().into_iter().flatten() {
                    writeln!(
                        out,
                        "{:<9} {}:{}  {}",
                        item["kind"].as_str().unwrap_or(""),
                        item["library"].as_str().unwrap_or(""),
                        item["name"].as_str().unwrap_or(""),
                        item["description"].as_str().unwrap_or("")
                    )?;
                }
                writeln!(
                    out,
                    "{} matches in {:.1} ms",
                    result["total"],
                    result["elapsedMs"].as_f64().unwrap_or(0.0)
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        LibraryCommand::Duplicates { markdown, target } => {
            let mut engine = engine(&target)?;
            let format = if markdown { "markdown" } else { "json" };
            let report = call(&mut engine, "library.report", json!({"format": format}))?;
            write!(out, "{}", report.as_str().unwrap_or(""))?;
            Ok(ExitCode::SUCCESS)
        }
        LibraryCommand::Pack {
            out: folder,
            prefix,
            name,
            version,
            target,
        } => {
            let dir = match target.dir {
                Some(dir) => dir,
                None => ew_core::library::default_dir()
                    .ok_or_else(|| Failure("no library folder".to_owned()))?,
            };
            let manifest = ew_library::pack::write_packs(&dir, &folder, &prefix, &name, &version)
                .map_err(|error| Failure(error.to_string()))?;
            if target.json {
                print_json(out, &serde_json::to_value(&manifest)?)?;
            } else {
                let total: u64 = manifest.packs.iter().map(|p| p.size).sum();
                writeln!(
                    out,
                    "Wrote {} packs ({:.1} MB) and manifest.json to {}",
                    manifest.packs.len(),
                    total as f64 / 1e6,
                    folder.display()
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        LibraryCommand::Install { pack, target } => {
            let mut engine = engine(&target)?;
            let result = call(&mut engine, "library.install", json!({"file": pack}))?;
            if target.json {
                print_json(out, &result)?;
            } else {
                writeln!(
                    out,
                    "Installed: index {}, {} meshes",
                    if result["installed"]["index"] == true {
                        "replaced"
                    } else {
                        "unchanged"
                    },
                    result["installed"]["meshes"]
                )?;
                print_status(&result["status"], out)?;
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}
