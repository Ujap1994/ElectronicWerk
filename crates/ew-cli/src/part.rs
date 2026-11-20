//! `ewerk part`: parts from PDF datasheets (M16, ADR-0017) — the text of a
//! datasheet, the recognised part draft, and the generated symbol,
//! footprint, 3D model and component in a project library.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Subcommand;
use ew_core::Engine;
use serde_json::{Value, json};

use crate::commands::{Failure, Outcome, call, print_json};

#[derive(Debug, Subcommand)]
pub enum PartCommand {
    /// Print the text of datasheet pages, line by line.
    Text {
        /// The PDF datasheet.
        datasheet: PathBuf,
        /// First page (1-based).
        #[arg(long)]
        from: Option<usize>,
        /// Last page.
        #[arg(long)]
        to: Option<usize>,
    },
    /// Recognise the part in a datasheet and write its draft (JSON).
    Extract {
        /// The PDF datasheet.
        datasheet: PathBuf,
        /// Write the draft to this file (default: print it with --json, or a
        /// summary).
        #[arg(long, short)]
        out: Option<PathBuf>,
        /// Print the draft as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Generate symbol, footprint, 3D model and component of one package
    /// of a draft into a project library.
    Generate {
        /// The draft (from `ewerk part extract`, corrected as needed).
        draft: PathBuf,
        /// The project folder or .ewproj file.
        #[arg(long)]
        project: PathBuf,
        /// The package name of the draft (`SOIC-8`).
        #[arg(long)]
        package: String,
        /// The library (default: the first project library).
        #[arg(long)]
        library: Option<String>,
        /// IPC-7351 density: least, nominal (default) or most.
        #[arg(long)]
        density: Option<String>,
        /// Name of the symbol and component.
        #[arg(long)]
        name: Option<String>,
        /// Do not write the 3D model.
        #[arg(long)]
        no_model: bool,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_owned())
}

/// Runs a part command.
///
/// # Errors
///
/// Unreadable datasheets or drafts, unknown packages or libraries.
#[allow(clippy::too_many_lines, reason = "one arm per command")]
pub fn run(command: PartCommand, out: &mut impl Write) -> Outcome {
    let mut engine = Engine::without_journal();
    match command {
        PartCommand::Text {
            datasheet,
            from,
            to,
        } => {
            let mut params = json!({ "path": absolute(&datasheet) });
            if let Some(from) = from {
                params["from"] = json!(from);
            }
            if let Some(to) = to {
                params["to"] = json!(to);
            }
            let text = call(&mut engine, "datasheet.text", params)?;
            for page in text["pages"].as_array().into_iter().flatten() {
                writeln!(out, "--- page {}", page["page"])?;
                writeln!(out, "{}", page["text"].as_str().unwrap_or(""))?;
            }
            Ok(ExitCode::SUCCESS)
        }
        PartCommand::Extract {
            datasheet,
            out: file,
            json,
        } => {
            let draft = call(
                &mut engine,
                "datasheet.extract",
                json!({ "path": absolute(&datasheet) }),
            )?;
            if let Some(file) = &file {
                call(
                    &mut engine,
                    "part.draftSave",
                    json!({ "draft": draft, "path": absolute(file) }),
                )?;
            }
            if json {
                print_json(out, &draft)?;
            } else {
                summary(&draft, out)?;
                if let Some(file) = &file {
                    writeln!(out, "Draft written to {}", file.display())?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        PartCommand::Generate {
            draft,
            project,
            package,
            library,
            density,
            name,
            no_model,
            json,
        } => {
            call(&mut engine, "project.open", json!({ "path": project }))?;
            let draft = call(
                &mut engine,
                "part.draftLoad",
                json!({ "path": absolute(&draft) }),
            )?;
            let mut options = json!({});
            if let Some(density) = density {
                options["density"] = json!(density);
            }
            if let Some(name) = name {
                options["name"] = json!(name);
            }
            let mut params = json!({
                "draft": draft,
                "package": package,
                "options": options,
                "model": !no_model,
            });
            if let Some(library) = library {
                let info = call(&mut engine, "project.info", Value::Null)?;
                let doc = info["documents"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|d| d["kind"] == "library" && d["name"] == library.as_str())
                    .map(|d| d["id"].clone())
                    .ok_or_else(|| Failure(format!("the project has no library {library}")))?;
                params["doc"] = doc;
            }
            let duplicates = call(&mut engine, "part.duplicates", params.clone())?;
            let made = call(&mut engine, "part.generate", params)?;
            call(&mut engine, "project.save", Value::Null)?;
            if json {
                print_json(out, &json!({ "generated": made, "duplicates": duplicates }))?;
            } else {
                writeln!(
                    out,
                    "Generated {}: symbol {}, footprint {}{}",
                    made["component"].as_str().unwrap_or(""),
                    made["symbol"].as_str().unwrap_or(""),
                    made["footprint"].as_str().unwrap_or(""),
                    made["model"]
                        .as_str()
                        .map(|m| format!(", 3D model {m}"))
                        .unwrap_or_default()
                )?;
                for warning in made["warnings"].as_array().into_iter().flatten() {
                    writeln!(out, "Note: {}", warning["message"].as_str().unwrap_or(""))?;
                }
                for m in duplicates["matches"].as_array().into_iter().flatten() {
                    writeln!(
                        out,
                        "Note: {} in the library {} is this part in nearly the same package.",
                        m["name"].as_str().unwrap_or(""),
                        m["library"].as_str().unwrap_or("")
                    )?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn summary(draft: &Value, out: &mut impl Write) -> std::io::Result<()> {
    writeln!(
        out,
        "{} ({})",
        draft["name"].as_str().unwrap_or("?"),
        draft["manufacturer"]
            .as_str()
            .unwrap_or("unknown manufacturer")
    )?;
    for p in draft["packages"].as_array().into_iter().flatten() {
        let dims = p["dimensions"].as_object();
        let read = dims.map_or(0, |d| {
            d.values().filter(|v| v.get("source").is_some()).count()
        });
        let total = dims.map_or(0, serde_json::Map::len);
        let pins = draft["pins"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|pin| {
                pin["numbers"]
                    .get(p["name"].as_str().unwrap_or(""))
                    .is_some()
            })
            .count();
        writeln!(
            out,
            "Package {}: {} pins, {} of {} dimensions from the datasheet",
            p["name"].as_str().unwrap_or("?"),
            pins,
            read,
            total
        )?;
    }
    for note in draft["notes"].as_array().into_iter().flatten() {
        writeln!(out, "Note: {}", note["message"].as_str().unwrap_or(""))?;
    }
    Ok(())
}
