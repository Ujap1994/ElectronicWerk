//! `ewerk mechanical`: the mechanical bodies of a board's 3D assembly
//! (enclosure parts, panels, screws) and the enclosure check (M17).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Subcommand;
use ew_core::Engine;
use serde_json::{Value, json};

use crate::commands::{Failure, Outcome, call, print_json};

#[derive(Debug, Subcommand)]
pub enum MechanicalCommand {
    /// List the bodies of a board.
    List {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// Board name (default: the first board).
        #[arg(long)]
        board: Option<String>,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Add a STEP file as a body (copied into the project's mechanical/
    /// folder).
    Add {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// The STEP file.
        file: PathBuf,
        /// Board name (default: the first board).
        #[arg(long)]
        board: Option<String>,
        /// Name (default: the file name).
        #[arg(long)]
        name: Option<String>,
        /// enclosure, lid, panel, screw, carrier or other (default: guessed
        /// from the name).
        #[arg(long)]
        kind: Option<String>,
        /// Position `x,y,z` in mm (default: the board's centre, z 0 at the
        /// board's bottom face).
        #[arg(long, value_name = "X,Y,Z")]
        at: Option<String>,
        /// Rotation about x, y and z in degrees.
        #[arg(long, value_name = "X,Y,Z")]
        rotate: Option<String>,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Move or turn a body.
    Move {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// The body (name or id).
        body: String,
        /// Board name (default: the first board).
        #[arg(long)]
        board: Option<String>,
        /// Position `x,y,z` in mm.
        #[arg(long, value_name = "X,Y,Z")]
        at: Option<String>,
        /// Rotation about x, y and z in degrees.
        #[arg(long, value_name = "X,Y,Z")]
        rotate: Option<String>,
    },
    /// Remove a body (its file stays in mechanical/).
    Remove {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// The body (name or id).
        body: String,
        /// Board name (default: the first board).
        #[arg(long)]
        board: Option<String>,
    },
    /// Check the board and its parts against the bodies: collisions and
    /// clearances. Exits with 1 if there are errors.
    Check {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// Board name (default: the first board).
        #[arg(long)]
        board: Option<String>,
        /// Minimum clearance in mm (default: the design rules').
        #[arg(long)]
        clearance: Option<f64>,
        /// Height of parts without a 3D model in mm (default 3).
        #[arg(long)]
        default_height: Option<f64>,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// The cross-section of the bodies at a height (mm above the board's
    /// bottom face; default the middle of the board).
    Section {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// Board name (default: the first board).
        #[arg(long)]
        board: Option<String>,
        /// Height of the plane in mm.
        #[arg(long)]
        z: Option<f64>,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Take the board outline from the room inside the enclosure at the
    /// board plane, less the clearance (cut-outs stay).
    Outline {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// Board name (default: the first board).
        #[arg(long)]
        board: Option<String>,
        /// Gap to the enclosure in mm (default: the design rules' minimum).
        #[arg(long)]
        clearance: Option<f64>,
        /// Only print the outline; do not change the board.
        #[arg(long)]
        dry_run: bool,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Add keep-out areas where the enclosure crosses the board and height
    /// areas from the room under the lid and above the floor.
    Areas {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// Board name (default: the first board).
        #[arg(long)]
        board: Option<String>,
        /// Grid of the height areas in mm (default 1).
        #[arg(long)]
        step: Option<f64>,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
    /// Make a simple box enclosure that fits the board (bottom shell with
    /// bosses under the mounting holes, flat lid) and add it.
    Enclosure {
        /// Project folder or `.ewproj` file.
        project: PathBuf,
        /// Board name (default: the first board).
        #[arg(long)]
        board: Option<String>,
        /// Gap between the board edge and the walls in mm (default 0.5).
        #[arg(long)]
        margin: Option<f64>,
        /// Print machine-readable JSON.
        #[arg(long)]
        json: bool,
    },
}

fn open(project: &Path) -> Result<Engine, Failure> {
    let mut engine = Engine::without_journal();
    call(&mut engine, "project.open", json!({ "path": project }))?;
    Ok(engine)
}

fn board_id(engine: &mut Engine, board: Option<&str>) -> Result<Value, Failure> {
    let info = call(engine, "project.info", Value::Null)?;
    let boards: Vec<&Value> = info["documents"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["kind"] == "pcb")
        .collect();
    let found = match board {
        Some(wanted) => boards
            .iter()
            .find(|d| d["name"] == wanted || d["id"] == wanted),
        None => boards.first(),
    };
    found
        .map(|d| d["id"].clone())
        .ok_or_else(|| Failure("the project has no such board".to_owned()))
}

fn body_id(engine: &mut Engine, doc: &Value, body: &str) -> Result<Value, Failure> {
    let list = call(engine, "mechanical.list", json!({ "doc": doc }))?;
    list["bodies"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|b| b["name"] == body || b["id"] == body)
        .map(|b| b["id"].clone())
        .ok_or_else(|| Failure(format!("the board has no body `{body}`")))
}

fn triple(text: &str, what: &str) -> Result<[f64; 3], Failure> {
    let values: Vec<f64> = text
        .split(',')
        .map(|v| v.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| Failure(format!("{what}: expected three numbers `x,y,z`")))?;
    <[f64; 3]>::try_from(values)
        .map_err(|_| Failure(format!("{what}: expected three numbers `x,y,z`")))
}

#[allow(clippy::cast_possible_truncation, reason = "millimetres to nanometres")]
fn position(text: &str) -> Result<Value, Failure> {
    let [x, y, z] = triple(text, "--at")?;
    Ok(json!([
        (x * 1e6).round() as i64,
        (y * 1e6).round() as i64,
        (z * 1e6).round() as i64
    ]))
}

fn rotation(text: &str) -> Result<Value, Failure> {
    let r = triple(text, "--rotate")?;
    Ok(json!(r))
}

fn save(engine: &mut Engine) -> Result<(), Failure> {
    call(engine, "project.save", Value::Null).map(|_| ())
}

#[allow(clippy::cast_precision_loss, reason = "positions for display")]
fn mm3(v: &Value) -> String {
    let at = |i: usize| v[i].as_i64().unwrap_or(0) as f64 / 1e6;
    format!("{}, {}, {}", at(0), at(1), at(2))
}

/// Runs a mechanical command.
///
/// # Errors
///
/// Unreadable projects, unknown boards or bodies, bad values.
#[allow(clippy::too_many_lines, reason = "one arm per command")]
pub fn run(command: MechanicalCommand, out: &mut impl Write) -> Outcome {
    match command {
        MechanicalCommand::List {
            project,
            board,
            json,
        } => {
            let mut engine = open(&project)?;
            let doc = board_id(&mut engine, board.as_deref())?;
            let list = call(&mut engine, "mechanical.list", json!({ "doc": doc }))?;
            if json {
                print_json(out, &list)?;
            } else {
                let bodies = list["bodies"].as_array().cloned().unwrap_or_default();
                if bodies.is_empty() {
                    writeln!(out, "No mechanical bodies.")?;
                }
                for b in &bodies {
                    let flags = [
                        (b["hidden"] == true).then_some("hidden"),
                        (b["check"] == false).then_some("not checked"),
                        (b["exists"] == false).then_some("file missing"),
                    ]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(", ");
                    writeln!(
                        out,
                        "{:<24} {:<10} at {} mm, turned {}°  {}{}",
                        b["name"].as_str().unwrap_or(""),
                        b["kind"].as_str().unwrap_or(""),
                        mm3(&b["position"]),
                        b["rotation"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|v| v.as_f64().unwrap_or(0.0).to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                        b["path"].as_str().unwrap_or(""),
                        if flags.is_empty() {
                            String::new()
                        } else {
                            format!(" ({flags})")
                        }
                    )?;
                }
                writeln!(
                    out,
                    "Minimum clearance: {} mm",
                    list["clearance"].as_f64().unwrap_or(0.0)
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        MechanicalCommand::Add {
            project,
            file,
            board,
            name,
            kind,
            at,
            rotate,
            json,
        } => {
            let mut engine = open(&project)?;
            let doc = board_id(&mut engine, board.as_deref())?;
            let file = std::path::absolute(&file).unwrap_or(file);
            let mut params = json!({ "doc": doc, "file": file });
            if let Some(name) = name {
                params["name"] = json!(name);
            }
            if let Some(kind) = kind {
                params["kind"] = json!(kind);
            }
            if let Some(at) = at {
                params["position"] = position(&at)?;
            }
            if let Some(rotate) = rotate {
                params["rotation"] = rotation(&rotate)?;
            }
            let added = call(&mut engine, "mechanical.add", params)?;
            save(&mut engine)?;
            if json {
                print_json(out, &added)?;
            } else {
                writeln!(
                    out,
                    "Added {} as a mechanical body.",
                    added["path"].as_str().unwrap_or("")
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        MechanicalCommand::Move {
            project,
            body,
            board,
            at,
            rotate,
        } => {
            let mut engine = open(&project)?;
            let doc = board_id(&mut engine, board.as_deref())?;
            let id = body_id(&mut engine, &doc, &body)?;
            let mut params = json!({ "doc": doc, "id": id });
            if let Some(at) = at {
                params["position"] = position(&at)?;
            }
            if let Some(rotate) = rotate {
                params["rotation"] = rotation(&rotate)?;
            }
            call(&mut engine, "mechanical.update", params)?;
            save(&mut engine)?;
            writeln!(out, "Moved {body}.")?;
            Ok(ExitCode::SUCCESS)
        }
        MechanicalCommand::Remove {
            project,
            body,
            board,
        } => {
            let mut engine = open(&project)?;
            let doc = board_id(&mut engine, board.as_deref())?;
            let id = body_id(&mut engine, &doc, &body)?;
            call(
                &mut engine,
                "mechanical.remove",
                json!({ "doc": doc, "ids": [id] }),
            )?;
            save(&mut engine)?;
            writeln!(out, "Removed {body}.")?;
            Ok(ExitCode::SUCCESS)
        }
        MechanicalCommand::Check {
            project,
            board,
            clearance,
            default_height,
            json,
        } => {
            let mut engine = open(&project)?;
            let doc = board_id(&mut engine, board.as_deref())?;
            let mut params = json!({ "doc": doc });
            if let Some(clearance) = clearance {
                params["clearance"] = json!(clearance);
            }
            if let Some(height) = default_height {
                params["defaultHeight"] = json!(height);
            }
            let report = call(&mut engine, "mechanical.check", params)?;
            let errors = report["errors"].as_u64().unwrap_or(0);
            if json {
                print_json(out, &report)?;
            } else {
                for finding in report["findings"].as_array().into_iter().flatten() {
                    let p = &finding["point"];
                    writeln!(
                        out,
                        "{:<8}{} [{}] at {:.2}, {:.2}, {:.2} mm{}",
                        finding["severity"].as_str().unwrap_or(""),
                        finding["message"].as_str().unwrap_or(""),
                        finding["code"].as_str().unwrap_or(""),
                        p[0].as_f64().unwrap_or(0.0),
                        p[1].as_f64().unwrap_or(0.0),
                        p[2].as_f64().unwrap_or(0.0),
                        if finding["excluded"] == true {
                            " (accepted)"
                        } else {
                            ""
                        }
                    )?;
                }
                for file in report["tessellate"].as_array().into_iter().flatten() {
                    writeln!(
                        out,
                        "Note: {} has curved faces and no mesh yet; open the board's 3D view in the app once.",
                        file.as_str().unwrap_or("")
                    )?;
                }
                let stand_ins = report["standIns"].as_array().map_or(0, Vec::len);
                if stand_ins > 0 {
                    writeln!(
                        out,
                        "Note: {stand_ins} parts without a 3D model were checked as boxes."
                    )?;
                }
                writeln!(
                    out,
                    "Enclosure check: {errors} errors, {} warnings ({} ms)",
                    report["warnings"].as_u64().unwrap_or(0),
                    report["milliseconds"].as_u64().unwrap_or(0)
                )?;
            }
            Ok(if errors == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        MechanicalCommand::Section {
            project,
            board,
            z,
            json,
        } => {
            let mut engine = open(&project)?;
            let doc = board_id(&mut engine, board.as_deref())?;
            let mut params = json!({ "doc": doc });
            if let Some(z) = z {
                params["z"] = json!(z);
            }
            let section = call(&mut engine, "mechanical.section", params)?;
            if json {
                print_json(out, &section)?;
            } else {
                let polygons = section["polygons"].as_array().cloned().unwrap_or_default();
                writeln!(
                    out,
                    "Section at z = {} mm: {} areas",
                    section["z"].as_f64().unwrap_or(0.0),
                    polygons.len()
                )?;
                for polygon in &polygons {
                    let rings = polygon.as_array().cloned().unwrap_or_default();
                    let outline = rings.first().and_then(Value::as_array).map_or(0, Vec::len);
                    writeln!(
                        out,
                        "  outline of {outline} points, {} holes",
                        rings.len().saturating_sub(1)
                    )?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        MechanicalCommand::Outline {
            project,
            board,
            clearance,
            dry_run,
            json,
        } => {
            let mut engine = open(&project)?;
            let doc = board_id(&mut engine, board.as_deref())?;
            let mut params = json!({ "doc": doc, "apply": !dry_run });
            if let Some(clearance) = clearance {
                params["clearance"] = json!(clearance);
            }
            let reply = call(&mut engine, "mechanical.outline", params)?;
            if !dry_run {
                save(&mut engine)?;
            }
            if json {
                print_json(out, &reply)?;
            } else {
                let points = reply["outline"].as_array().cloned().unwrap_or_default();
                writeln!(
                    out,
                    "{} the board outline with {} corners from the enclosure:",
                    if dry_run { "Would set" } else { "Set" },
                    points.len()
                )?;
                for p in &points {
                    writeln!(
                        out,
                        "  {:.3}, {:.3} mm",
                        p[0].as_f64().unwrap_or(0.0),
                        p[1].as_f64().unwrap_or(0.0)
                    )?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        MechanicalCommand::Areas {
            project,
            board,
            step,
            json,
        } => {
            let mut engine = open(&project)?;
            let doc = board_id(&mut engine, board.as_deref())?;
            let mut params = json!({ "doc": doc });
            if let Some(step) = step {
                params["step"] = json!(step);
            }
            let reply = call(&mut engine, "mechanical.areas", params)?;
            save(&mut engine)?;
            if json {
                print_json(out, &reply)?;
            } else {
                writeln!(
                    out,
                    "Added {} keep-out areas and height areas:",
                    reply["keepouts"].as_u64().unwrap_or(0)
                )?;
                for h in reply["heights"].as_array().into_iter().flatten() {
                    writeln!(
                        out,
                        "  {}: parts up to {} mm",
                        h["side"].as_str().unwrap_or(""),
                        h["height"].as_f64().unwrap_or(0.0)
                    )?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        MechanicalCommand::Enclosure {
            project,
            board,
            margin,
            json,
        } => {
            let mut engine = open(&project)?;
            let doc = board_id(&mut engine, board.as_deref())?;
            let mut params = json!({ "doc": doc });
            if let Some(margin) = margin {
                params["margin"] = json!(margin);
            }
            let made = call(&mut engine, "mechanical.enclosure", params)?;
            save(&mut engine)?;
            if json {
                print_json(out, &made)?;
            } else {
                let s = &made["spec"];
                writeln!(
                    out,
                    "Added a box enclosure, inside {} × {} × {} mm, with {} bosses: mechanical/enclosure-bottom.step and mechanical/enclosure-lid.step",
                    s["width"].as_f64().unwrap_or(0.0),
                    s["depth"].as_f64().unwrap_or(0.0),
                    s["height"].as_f64().unwrap_or(0.0),
                    s["bosses"].as_array().map_or(0, Vec::len)
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}
