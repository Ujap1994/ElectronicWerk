//! `ewerk system`: systems of several boards (M18) — boards, connections,
//! the connection check, stacking, outputs and the 3D assembly.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Subcommand;
use ew_core::Engine;
use serde_json::{Value, json};

use crate::commands::{Failure, Outcome, call, print_json};

#[derive(Debug, Subcommand)]
pub enum SystemCommand {
    /// Create a system in a project.
    New {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// Name of the system.
        name: String,
    },
    /// List the systems of a project with their boards and connections.
    List {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Add a board: one of this project (`--board`) or of another project
    /// (`--other`).
    AddBoard {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// The system (default: the first).
        #[arg(long)]
        system: Option<String>,
        /// A board of this project (name).
        #[arg(long, conflicts_with = "other")]
        board: Option<String>,
        /// Another project (folder or `.ewproj`).
        #[arg(long)]
        other: Option<PathBuf>,
        /// The board of the other project (default: its first).
        #[arg(long, requires = "other")]
        other_board: Option<String>,
        /// Name in the system.
        #[arg(long)]
        name: Option<String>,
        /// How many of it the system has.
        #[arg(long)]
        quantity: Option<u32>,
    },
    /// Two connectors that mate directly: `Carrier:J1 Shield:P1`.
    Link {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// One connector (`board:designator`).
        a: String,
        /// The connector it mates with (`board:designator`).
        b: String,
        /// The system (default: the first).
        #[arg(long)]
        system: Option<String>,
        /// `number`, `mirrored` or `position` (default number).
        #[arg(long)]
        pairing: Option<String>,
    },
    /// Place a board on the connector it mates with.
    Stack {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// The board to move (its name in the system).
        board: String,
        /// The system (default: the first).
        #[arg(long)]
        system: Option<String>,
        /// Distance between the connectors' boards in mm (default: the
        /// lower connector's height).
        #[arg(long)]
        gap: Option<f64>,
        /// Turn the board over (a mezzanine board facing down).
        #[arg(long)]
        flip: bool,
    },
    /// The connection check across the boards. Exits with 1 if there are
    /// errors.
    Check {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// The system (default: the first).
        #[arg(long)]
        system: Option<String>,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Write the bill of materials, the connection report, the wiring
    /// lists, the harness drawings and the 3D assembly.
    Outputs {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// The system (default: the first).
        #[arg(long)]
        system: Option<String>,
        /// Folder (default: outputs/<system>/ in the project).
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Export the system's 3D assembly as one STEP file.
    #[command(name = "export-3d")]
    Export3d {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// The system (default: the first).
        #[arg(long)]
        system: Option<String>,
        /// Output file (default: `<system>.step` in the current folder).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn open(project: &Path) -> Result<Engine, Failure> {
    let mut engine = Engine::without_journal();
    call(&mut engine, "project.open", json!({ "path": project }))?;
    Ok(engine)
}

fn save(engine: &mut Engine) -> Result<(), Failure> {
    call(engine, "project.save", Value::Null).map(|_| ())
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The system document: by name or id, else the first.
fn system_id(engine: &mut Engine, wanted: Option<&str>) -> Result<(Value, String), Failure> {
    let info = call(engine, "project.info", Value::Null)?;
    let systems: Vec<&Value> = info["documents"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["kind"] == "system")
        .collect();
    let found = match wanted {
        Some(w) => systems.iter().find(|d| d["name"] == w || d["id"] == w),
        None => systems.first(),
    };
    found
        .map(|d| {
            (
                d["id"].clone(),
                d["name"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .ok_or_else(|| Failure("the project has no such system".to_owned()))
}

fn board_of(info: &Value, name: &str) -> Result<Value, Failure> {
    info["boards"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|b| b["name"] == name || b["id"] == name)
        .map(|b| b["id"].clone())
        .ok_or_else(|| Failure(format!("the system has no board `{name}`")))
}

fn connector(info: &Value, text: &str) -> Result<Value, Failure> {
    let (board, designator) = text
        .rsplit_once(':')
        .ok_or_else(|| Failure(format!("`{text}`: write board:designator")))?;
    Ok(json!({ "board": board_of(info, board)?, "designator": designator }))
}

fn print_findings(out: &mut impl Write, checked: &Value) -> std::io::Result<()> {
    for f in checked["findings"].as_array().into_iter().flatten() {
        writeln!(
            out,
            "{:<8}{} [{}]{}",
            f["severity"].as_str().unwrap_or(""),
            f["message"].as_str().unwrap_or(""),
            f["code"].as_str().unwrap_or(""),
            if f["excluded"] == true {
                " (accepted)"
            } else {
                ""
            }
        )?;
    }
    Ok(())
}

/// Runs a system command.
///
/// # Errors
///
/// Unreadable projects, unknown systems, boards or connectors.
#[allow(clippy::too_many_lines, reason = "one arm per command")]
pub fn run(command: SystemCommand, out: &mut impl Write) -> Outcome {
    match command {
        SystemCommand::New { project, name } => {
            let mut engine = open(&project)?;
            let reply = call(
                &mut engine,
                "document.add",
                json!({ "kind": "system", "name": name }),
            )?;
            save(&mut engine)?;
            writeln!(
                out,
                "Created system {name} ({}).",
                reply["id"].as_str().unwrap_or("")
            )?;
            Ok(ExitCode::SUCCESS)
        }
        SystemCommand::List { project, json } => {
            let mut engine = open(&project)?;
            let info = call(&mut engine, "project.info", Value::Null)?;
            let mut systems = Vec::new();
            for d in info["documents"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|d| d["kind"] == "system")
            {
                systems.push(call(&mut engine, "system.info", json!({ "doc": d["id"] }))?);
            }
            if json {
                print_json(out, &json!({ "systems": systems }))?;
            } else if systems.is_empty() {
                writeln!(out, "No systems.")?;
            }
            if !json {
                for s in &systems {
                    writeln!(out, "{}", s["name"].as_str().unwrap_or(""))?;
                    for b in s["boards"].as_array().into_iter().flatten() {
                        writeln!(
                            out,
                            "  board  {:<16} {}{}",
                            b["name"].as_str().unwrap_or(""),
                            if b["local"] == true {
                                format!("this project: {}", b["board"].as_str().unwrap_or(""))
                            } else {
                                format!(
                                    "{}: {}",
                                    b["source"]["path"].as_str().unwrap_or(""),
                                    b["board"].as_str().unwrap_or("")
                                )
                            },
                            b["error"]
                                .as_str()
                                .map(|e| format!(" (cannot be read: {e})"))
                                .unwrap_or_default()
                        )?;
                    }
                    let name = |id: &Value| -> String {
                        s["boards"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .find(|b| &b["id"] == id)
                            .and_then(|b| b["name"].as_str())
                            .unwrap_or("?")
                            .to_owned()
                    };
                    for l in s["links"].as_array().into_iter().flatten() {
                        writeln!(
                            out,
                            "  link   {}:{} ↔ {}:{}",
                            name(&l["a"]["board"]),
                            l["a"]["designator"].as_str().unwrap_or(""),
                            name(&l["b"]["board"]),
                            l["b"]["designator"].as_str().unwrap_or("")
                        )?;
                    }
                    for c in s["cables"].as_array().into_iter().flatten() {
                        writeln!(
                            out,
                            "  cable  {} ({} wires)",
                            c["name"].as_str().unwrap_or(""),
                            c["wires"].as_array().map_or(0, Vec::len)
                        )?;
                    }
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        SystemCommand::AddBoard {
            project,
            system,
            board,
            other,
            other_board,
            name,
            quantity,
        } => {
            let mut engine = open(&project)?;
            let (doc, _) = system_id(&mut engine, system.as_deref())?;
            let source = if let Some(other) = other {
                json!({ "kind": "project", "path": absolute(&other), "board": other_board.unwrap_or_default() })
            } else {
                let wanted = board.ok_or_else(|| Failure("give --board or --other".to_owned()))?;
                let info = call(&mut engine, "project.info", Value::Null)?;
                let id = info["documents"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|d| d["kind"] == "pcb" && d["name"] == wanted.as_str())
                    .map(|d| d["id"].clone())
                    .ok_or_else(|| Failure(format!("the project has no board `{wanted}`")))?;
                json!({ "kind": "local", "doc": id })
            };
            let mut params = json!({ "doc": doc, "source": source });
            if let Some(name) = name {
                params["name"] = json!(name);
            }
            if let Some(q) = quantity {
                params["quantity"] = json!(q);
            }
            call(&mut engine, "system.addBoard", params)?;
            save(&mut engine)?;
            writeln!(out, "Added the board to the system.")?;
            Ok(ExitCode::SUCCESS)
        }
        SystemCommand::Link {
            project,
            a,
            b,
            system,
            pairing,
        } => {
            let mut engine = open(&project)?;
            let (doc, _) = system_id(&mut engine, system.as_deref())?;
            let info = call(&mut engine, "system.info", json!({ "doc": doc }))?;
            let mut params =
                json!({ "doc": doc, "a": connector(&info, &a)?, "b": connector(&info, &b)? });
            if let Some(p) = pairing {
                params["pairing"] = json!(p);
            }
            call(&mut engine, "system.addLink", params)?;
            save(&mut engine)?;
            writeln!(out, "Linked {a} and {b}.")?;
            Ok(ExitCode::SUCCESS)
        }
        SystemCommand::Stack {
            project,
            board,
            system,
            gap,
            flip,
        } => {
            let mut engine = open(&project)?;
            let (doc, _) = system_id(&mut engine, system.as_deref())?;
            let info = call(&mut engine, "system.info", json!({ "doc": doc }))?;
            let id = board_of(&info, &board)?;
            let link = info["links"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|l| l["b"]["board"] == id)
                .map(|l| l["id"].clone())
                .ok_or_else(|| {
                    Failure(format!("{board} is not the second board of a connection"))
                })?;
            let mut params = json!({ "doc": doc, "link": link, "flip": flip });
            if let Some(gap) = gap {
                params["gap"] = json!(gap);
            }
            let reply = call(&mut engine, "system.stack", params)?;
            save(&mut engine)?;
            let p = &reply["position"];
            #[allow(clippy::cast_precision_loss, reason = "positions for display")]
            let mm = |v: &Value| v.as_i64().unwrap_or(0) as f64 / 1e6;
            writeln!(
                out,
                "Placed {board} at {}, {}, {} mm, turned {}° ({} mm above the connector's board).",
                mm(&p[0]),
                mm(&p[1]),
                mm(&p[2]),
                reply["rotation"][2].as_f64().unwrap_or(0.0),
                reply["gap"].as_f64().unwrap_or(0.0)
            )?;
            Ok(ExitCode::SUCCESS)
        }
        SystemCommand::Check {
            project,
            system,
            json,
        } => {
            let mut engine = open(&project)?;
            let (doc, name) = system_id(&mut engine, system.as_deref())?;
            let checked = call(&mut engine, "system.check", json!({ "doc": doc }))?;
            let errors = checked["errors"].as_u64().unwrap_or(0);
            if json {
                print_json(out, &checked)?;
            } else {
                print_findings(out, &checked)?;
                writeln!(
                    out,
                    "System {name}: {errors} errors, {} warnings",
                    checked["warnings"].as_u64().unwrap_or(0)
                )?;
            }
            Ok(if errors == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        SystemCommand::Outputs {
            project,
            system,
            dir,
            json,
        } => {
            let mut engine = open(&project)?;
            let (doc, _) = system_id(&mut engine, system.as_deref())?;
            let mut params = json!({ "doc": doc });
            if let Some(dir) = dir {
                params["dir"] = json!(absolute(&dir));
            }
            let reply = call(&mut engine, "system.outputs", params)?;
            if json {
                print_json(out, &reply)?;
            } else {
                for f in reply["files"].as_array().into_iter().flatten() {
                    writeln!(out, "{}", f.as_str().unwrap_or(""))?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        SystemCommand::Export3d {
            project,
            system,
            output,
        } => {
            let mut engine = open(&project)?;
            let (doc, name) = system_id(&mut engine, system.as_deref())?;
            let path = absolute(&output.unwrap_or_else(|| PathBuf::from(format!("{name}.step"))));
            let reply = call(
                &mut engine,
                "system.export",
                json!({ "doc": doc, "path": path }),
            )?;
            writeln!(
                out,
                "{name} → {} ({} bytes)",
                path.display(),
                reply["bytes"]
            )?;
            for note in reply["notes"].as_array().into_iter().flatten() {
                writeln!(out, "  {}", note.as_str().unwrap_or(""))?;
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}
