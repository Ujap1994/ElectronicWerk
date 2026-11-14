//! `ewerk schematic …` and `ewerk board …`: editing a design from the
//! terminal with designators, pin names and millimetres (the engine's
//! `design.*` methods). Each command opens the project, makes its change,
//! saves and closes it.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Args, Subcommand, ValueEnum};
use ew_core::Engine;
use serde_json::{Map, Value, json};

use crate::commands::{Failure, Outcome, call, print_json};

/// The project and the output format of every design command.
#[derive(Debug, Args)]
pub struct Target {
    /// Project folder or `.ewproj` file.
    pub project: PathBuf,
    /// Print machine-readable JSON.
    #[arg(long, global = true)]
    pub json: bool,
}

#[derive(Debug, Subcommand)]
pub enum SchematicCommand {
    /// List the parts with their pins and nets.
    List {
        #[command(flatten)]
        target: Target,
    },
    /// List the nets with their pins.
    Nets {
        #[command(flatten)]
        target: Target,
    },
    /// Place a symbol (from the project library or the base library).
    Place(PlaceArgs),
    /// Change the value, footprint, designator or properties of a part.
    Set(SetArgs),
    /// Connect pins to a net (`R1.2`, `U1.VCC`): a short wire and a label.
    Connect(ConnectArgs),
    /// Mark pins as intentionally unconnected.
    NoConnect {
        #[command(flatten)]
        target: Target,
        /// Pins (`U1.7`, `U1.NC`).
        #[arg(required = true)]
        pins: Vec<String>,
    },
    /// Number the parts that have no number yet (`R?` → `R4`).
    Annotate {
        #[command(flatten)]
        target: Target,
        /// Number every part again.
        #[arg(long)]
        all: bool,
    },
    /// Run the electrical rules check (exit code 1 if there are errors).
    Erc {
        #[command(flatten)]
        target: Target,
    },
    /// Write the netlist (KiCad or CSV).
    Netlist {
        #[command(flatten)]
        target: Target,
        /// `kicad` or `csv`.
        #[arg(long, default_value = "kicad")]
        format: String,
        /// Output file (default: standard output).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Args)]
pub struct PlaceArgs {
    #[command(flatten)]
    target: Target,
    /// Symbol name (`R`, `Device:R`) or, with `--component`, a component of
    /// the project library.
    symbol: String,
    /// The name is a component of the project library.
    #[arg(long)]
    component: bool,
    /// Designator (default: the next free one, for example `R4`).
    #[arg(long = "ref")]
    designator: Option<String>,
    /// Value (for example `10k`).
    #[arg(long)]
    value: Option<String>,
    /// Footprint: a project footprint, a wizard preset (`0805`, `SOIC-8`)
    /// or a base library footprint.
    #[arg(long)]
    footprint: Option<String>,
    /// Sheet name (default: the top sheet).
    #[arg(long)]
    sheet: Option<String>,
    /// Position in millimetres, `x,y` (default: a free place).
    #[arg(long, value_parser = parse_xy)]
    at: Option<[f64; 2]>,
    /// Rotation in degrees (counter-clockwise).
    #[arg(long, default_value_t = 0.0, allow_negative_numbers = true)]
    rotate: f64,
    /// Mirror the symbol.
    #[arg(long)]
    mirror: bool,
    /// Unit of a multi-unit symbol (1 = A).
    #[arg(long, default_value_t = 1)]
    unit: u16,
}

#[derive(Debug, Args)]
pub struct SetArgs {
    #[command(flatten)]
    target: Target,
    /// The part (`R1`).
    designator: String,
    /// New value.
    #[arg(long)]
    value: Option<String>,
    /// New footprint (project footprint, wizard preset or base library).
    #[arg(long)]
    footprint: Option<String>,
    /// New designator.
    #[arg(long)]
    rename: Option<String>,
    /// A property, `KEY=VALUE` (an empty value removes it); repeatable.
    #[arg(long = "property", value_parser = parse_property)]
    properties: Vec<(String, String)>,
}

/// How a net is drawn.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum NetStyle {
    /// A local label, or a global label when the net spans sheets.
    Auto,
    /// A local label.
    Label,
    /// A global label.
    Global,
    /// A power port (GND gets the ground symbol).
    Power,
}

#[derive(Debug, Args)]
pub struct ConnectArgs {
    #[command(flatten)]
    target: Target,
    /// Net name.
    net: String,
    /// Pins (`R1.2`, `U1.VCC`).
    #[arg(required = true)]
    pins: Vec<String>,
    /// How the net is drawn.
    #[arg(long, value_enum, default_value_t = NetStyle::Auto)]
    style: NetStyle,
}

#[derive(Debug, Subcommand)]
pub enum BoardCommand {
    /// Show the footprints, layers and unrouted connections.
    Info {
        #[command(flatten)]
        target: Target,
        /// Board name (default: the first board).
        #[arg(long)]
        board: Option<String>,
    },
    /// Bring the board up to date with the schematic (new parts, nets).
    Update {
        #[command(flatten)]
        target: Target,
        #[arg(long)]
        board: Option<String>,
    },
    /// Put a footprint at a position (millimetres).
    Place {
        #[command(flatten)]
        target: Target,
        /// The part (`R1`).
        designator: String,
        /// Position `x,y` in millimetres.
        #[arg(long, value_parser = parse_xy)]
        at: [f64; 2],
        /// Rotation in degrees (counter-clockwise).
        #[arg(long, allow_negative_numbers = true)]
        rotate: Option<f64>,
        /// `top` or `bottom`.
        #[arg(long)]
        side: Option<String>,
        #[arg(long)]
        board: Option<String>,
    },
    /// Put footprints in rows inside the board outline (default: those that
    /// are not on the board yet).
    Arrange {
        #[command(flatten)]
        target: Target,
        /// Only these parts.
        designators: Vec<String>,
        /// Gap between footprints in millimetres.
        #[arg(long, default_value_t = 2.0)]
        gap: f64,
        #[arg(long)]
        board: Option<String>,
    },
    /// Route the unrouted connections (walkaround router, 45° bends).
    Route {
        #[command(flatten)]
        target: Target,
        /// Only these nets (repeatable).
        #[arg(long = "net")]
        nets: Vec<String>,
        /// Copper layers in order of preference (repeatable; default: top,
        /// bottom).
        #[arg(long = "layer")]
        layers: Vec<String>,
        /// Track width in millimetres (default: the design rules).
        #[arg(long)]
        width: Option<f64>,
        #[arg(long)]
        board: Option<String>,
    },
    /// Fill the copper zones.
    FillZones {
        #[command(flatten)]
        target: Target,
        #[arg(long)]
        board: Option<String>,
    },
    /// Run the design rules check (exit code 1 if there are errors).
    Drc {
        #[command(flatten)]
        target: Target,
        #[arg(long)]
        board: Option<String>,
    },
}

fn parse_xy(text: &str) -> Result<[f64; 2], String> {
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    match parts.as_slice() {
        [x, y] => {
            let x: f64 = x.parse().map_err(|_| format!("`{x}` is not a number"))?;
            let y: f64 = y.parse().map_err(|_| format!("`{y}` is not a number"))?;
            if x.is_finite() && y.is_finite() {
                Ok([x, y])
            } else {
                Err("the position must be finite".to_owned())
            }
        }
        _ => Err("write the position as x,y in millimetres, for example 50,40".to_owned()),
    }
}

fn parse_property(text: &str) -> Result<(String, String), String> {
    text.split_once('=')
        .map(|(key, value)| (key.trim().to_owned(), value.to_owned()))
        .filter(|(key, _)| !key.is_empty())
        .ok_or_else(|| "write a property as KEY=VALUE".to_owned())
}

/// An engine with the project open.
fn open(project: &Path) -> Result<Engine, Failure> {
    let mut engine = Engine::without_journal();
    call(&mut engine, "project.open", json!({ "path": project }))?;
    Ok(engine)
}

/// Saves the project if the call changed it.
fn save_if_changed(engine: &mut Engine, result: &Value) -> Result<(), Failure> {
    if result.get("seq").is_some_and(|seq| !seq.is_null()) {
        call(engine, "project.save", Value::Null)?;
    }
    Ok(())
}

/// Runs one changing method and saves.
fn change(project: &Path, method: &str, params: Value) -> Result<Value, Failure> {
    let mut engine = open(project)?;
    let result = call(&mut engine, method, params)?;
    save_if_changed(&mut engine, &result)?;
    Ok(result)
}

fn board_param(board: Option<String>) -> Map<String, Value> {
    let mut params = Map::new();
    if let Some(board) = board {
        params.insert("board".into(), Value::String(board));
    }
    params
}

/// `1 change`, `2 changes`.
fn count(n: u64, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn pins_text(value: &Value) -> String {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

#[allow(clippy::too_many_lines, reason = "one arm per command")]
pub fn schematic(command: SchematicCommand, out: &mut impl Write) -> Outcome {
    match command {
        SchematicCommand::List { target } => {
            let mut engine = open(&target.project)?;
            let result = call(&mut engine, "design.components", Value::Null)?;
            if target.json {
                print_json(out, &result)?;
            } else {
                for part in result["components"].as_array().into_iter().flatten() {
                    let pins: Vec<String> = part["pins"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|pin| {
                            format!(
                                "{}={}",
                                pin["number"].as_str().unwrap_or(""),
                                pin["net"].as_str().unwrap_or("-")
                            )
                        })
                        .collect();
                    writeln!(
                        out,
                        "{:<8} {:<12} {:<28} {}",
                        part["designator"].as_str().unwrap_or(""),
                        part["value"].as_str().unwrap_or(""),
                        part["footprint"].as_str().unwrap_or("(no footprint)"),
                        pins.join(" ")
                    )?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        SchematicCommand::Nets { target } => {
            let mut engine = open(&target.project)?;
            let result = call(&mut engine, "design.nets", Value::Null)?;
            if target.json {
                print_json(out, &result)?;
            } else {
                for net in result["nets"].as_array().into_iter().flatten() {
                    writeln!(
                        out,
                        "{:<20} {}",
                        net["name"].as_str().unwrap_or(""),
                        pins_text(&net["pins"])
                    )?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        SchematicCommand::Place(args) => {
            let mut params = Map::new();
            let key = if args.component {
                "component"
            } else {
                "symbol"
            };
            params.insert(key.into(), json!(args.symbol));
            for (key, value) in [
                ("designator", args.designator),
                ("value", args.value),
                ("footprint", args.footprint),
                ("sheet", args.sheet),
            ] {
                if let Some(value) = value {
                    params.insert(key.into(), json!(value));
                }
            }
            if let Some(at) = args.at {
                params.insert("at".into(), json!(at));
            }
            params.insert("rotation".into(), json!(args.rotate));
            params.insert("mirror".into(), json!(args.mirror));
            params.insert("unit".into(), json!(args.unit));
            let result = change(&args.target.project, "design.place", Value::Object(params))?;
            if args.target.json {
                print_json(out, &result)?;
            } else {
                writeln!(
                    out,
                    "Placed {} ({}) at {} mm on sheet {}",
                    result["designator"].as_str().unwrap_or(""),
                    result["value"].as_str().unwrap_or(""),
                    result["at"],
                    result["sheet"].as_str().unwrap_or("")
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        SchematicCommand::Set(args) => {
            let mut params = Map::new();
            params.insert("designator".into(), json!(args.designator));
            for (key, value) in [
                ("value", args.value),
                ("footprint", args.footprint),
                ("rename", args.rename),
            ] {
                if let Some(value) = value {
                    params.insert(key.into(), json!(value));
                }
            }
            if !args.properties.is_empty() {
                let properties: Map<String, Value> = args
                    .properties
                    .into_iter()
                    .map(|(key, value)| (key, Value::String(value)))
                    .collect();
                params.insert("properties".into(), Value::Object(properties));
            }
            let result = change(&args.target.project, "design.set", Value::Object(params))?;
            if args.target.json {
                print_json(out, &result)?;
            } else {
                writeln!(
                    out,
                    "Changed {}",
                    result["designator"].as_str().unwrap_or("")
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        SchematicCommand::Connect(args) => {
            let style = match args.style {
                NetStyle::Auto => "auto",
                NetStyle::Label => "label",
                NetStyle::Global => "global",
                NetStyle::Power => "power",
            };
            let result = change(
                &args.target.project,
                "design.connect",
                json!({ "net": args.net, "pins": args.pins, "style": style }),
            )?;
            if args.target.json {
                print_json(out, &result)?;
            } else {
                let connected = pins_text(&result["connected"]);
                if !connected.is_empty() {
                    writeln!(out, "Connected {connected} to {}", args.net)?;
                }
                let already = pins_text(&result["already"]);
                if !already.is_empty() {
                    writeln!(out, "Already on {}: {already}", args.net)?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        SchematicCommand::NoConnect { target, pins } => {
            let result = change(&target.project, "design.noConnect", json!({ "pins": pins }))?;
            if target.json {
                print_json(out, &result)?;
            } else {
                writeln!(
                    out,
                    "Marked as unconnected: {}",
                    pins_text(&result["marked"])
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        SchematicCommand::Annotate { target, all } => {
            let result = change(
                &target.project,
                "schematic.annotate",
                json!({ "reset": all }),
            )?;
            if target.json {
                print_json(out, &result)?;
            } else {
                writeln!(
                    out,
                    "Annotated {} parts",
                    result["changes"].as_u64().unwrap_or(0)
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        SchematicCommand::Erc { target } => {
            let mut engine = open(&target.project)?;
            let report = call(&mut engine, "schematic.erc", Value::Null)?;
            let issues: Vec<&Value> = report["issues"].as_array().into_iter().flatten().collect();
            let errors = issues.iter().filter(|i| i["severity"] == "error").count();
            if target.json {
                print_json(out, &report)?;
            } else {
                for issue in &issues {
                    writeln!(
                        out,
                        "{:<7} {:<24} {}",
                        issue["severity"].as_str().unwrap_or(""),
                        issue["code"].as_str().unwrap_or(""),
                        issue["message"].as_str().unwrap_or("")
                    )?;
                }
                let warnings = issues.iter().filter(|i| i["severity"] == "warning").count();
                writeln!(out, "ERC: {errors} errors, {warnings} warnings")?;
            }
            Ok(if errors == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        SchematicCommand::Netlist {
            target,
            format,
            output,
        } => {
            let mut engine = open(&target.project)?;
            let result = call(
                &mut engine,
                "schematic.netlist",
                json!({ "format": format }),
            )?;
            let text = result["text"].as_str().unwrap_or("");
            match output {
                Some(path) => {
                    std::fs::write(&path, text)?;
                    if !target.json {
                        writeln!(out, "Wrote {}", path.display())?;
                    }
                }
                None => write!(out, "{text}")?,
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

#[allow(clippy::too_many_lines, reason = "one arm per command")]
pub fn board(command: BoardCommand, out: &mut impl Write) -> Outcome {
    match command {
        BoardCommand::Info { target, board } => {
            let mut engine = open(&target.project)?;
            let result = call(
                &mut engine,
                "design.board",
                Value::Object(board_param(board)),
            )?;
            if target.json {
                print_json(out, &result)?;
            } else {
                writeln!(
                    out,
                    "{}: {} footprints, {} tracks, {} vias, {} unrouted",
                    result["board"].as_str().unwrap_or(""),
                    result["footprints"].as_array().map_or(0, Vec::len),
                    result["tracks"],
                    result["vias"],
                    result["unrouted"]
                )?;
                for fp in result["footprints"].as_array().into_iter().flatten() {
                    writeln!(
                        out,
                        "{:<8} {:<28} at {} {:>5}° {}{}",
                        fp["designator"].as_str().unwrap_or(""),
                        fp["footprint"].as_str().unwrap_or(""),
                        fp["at"],
                        fp["rotation"],
                        fp["side"].as_str().unwrap_or(""),
                        if fp["onBoard"] == true {
                            ""
                        } else {
                            " (off the board)"
                        }
                    )?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        BoardCommand::Update { target, board } => {
            let result = change(
                &target.project,
                "design.updateBoard",
                Value::Object(board_param(board)),
            )?;
            if target.json {
                print_json(out, &result)?;
            } else {
                let n = result["changes"].as_array().map_or(0, Vec::len) as u64;
                writeln!(
                    out,
                    "Applied {} from the schematic",
                    count(n, "change", "changes")
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        BoardCommand::Place {
            target,
            designator,
            at,
            rotate,
            side,
            board,
        } => {
            let mut params = board_param(board);
            params.insert("designator".into(), json!(designator));
            params.insert("at".into(), json!(at));
            if let Some(rotate) = rotate {
                params.insert("rotation".into(), json!(rotate));
            }
            if let Some(side) = side {
                if side != "top" && side != "bottom" {
                    return Err(Failure("the side is `top` or `bottom`".to_owned()));
                }
                params.insert("side".into(), json!(side));
            }
            let result = change(
                &target.project,
                "design.placeFootprint",
                Value::Object(params),
            )?;
            if target.json {
                print_json(out, &result)?;
            } else {
                writeln!(
                    out,
                    "Placed {} at {} mm, {}°, {}",
                    result["designator"].as_str().unwrap_or(""),
                    result["at"],
                    result["rotation"],
                    result["side"].as_str().unwrap_or("")
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        BoardCommand::Arrange {
            target,
            designators,
            gap,
            board,
        } => {
            let mut params = board_param(board);
            if !designators.is_empty() {
                params.insert("designators".into(), json!(designators));
            }
            params.insert("gap".into(), json!(gap));
            let result = change(&target.project, "design.arrange", Value::Object(params))?;
            if target.json {
                print_json(out, &result)?;
            } else {
                writeln!(out, "Placed {}", pins_text(&result["placed"]))?;
                let no_room = pins_text(&result["noRoom"]);
                if !no_room.is_empty() {
                    writeln!(out, "No room on the board for {no_room}")?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        BoardCommand::Route {
            target,
            nets,
            layers,
            width,
            board,
        } => {
            let mut params = board_param(board);
            if !nets.is_empty() {
                params.insert("nets".into(), json!(nets));
            }
            if !layers.is_empty() {
                params.insert("layers".into(), json!(layers));
            }
            if let Some(width) = width {
                params.insert("width".into(), json!(width));
            }
            let result = change(&target.project, "design.route", Value::Object(params))?;
            if target.json {
                print_json(out, &result)?;
            } else {
                writeln!(
                    out,
                    "Routed {}; {} still unrouted",
                    count(
                        result["routed"].as_u64().unwrap_or(0),
                        "connection",
                        "connections"
                    ),
                    result["unrouted"]
                )?;
                for failed in result["failed"].as_array().into_iter().flatten() {
                    writeln!(
                        out,
                        "  no path for {} from {} to {}",
                        failed["net"].as_str().unwrap_or(""),
                        failed["from"],
                        failed["to"]
                    )?;
                }
            }
            Ok(ExitCode::SUCCESS)
        }
        BoardCommand::FillZones { target, board } => {
            let mut engine = open(&target.project)?;
            let info = call(
                &mut engine,
                "design.board",
                Value::Object(board_param(board)),
            )?;
            let result = call(&mut engine, "pcb.fillZones", json!({ "doc": info["id"] }))?;
            save_if_changed(&mut engine, &result)?;
            if target.json {
                print_json(out, &result)?;
            } else {
                writeln!(
                    out,
                    "Filled the zones ({} areas)",
                    result["areas"].as_u64().unwrap_or(0)
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
        BoardCommand::Drc { target, board } => {
            let mut engine = open(&target.project)?;
            let info = call(
                &mut engine,
                "design.board",
                Value::Object(board_param(board)),
            )?;
            let report = call(&mut engine, "pcb.drc", json!({ "doc": info["id"] }))?;
            let findings: Vec<&Value> = report["findings"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|f| f["excluded"] != true)
                .collect();
            let errors = findings.iter().filter(|f| f["severity"] == "error").count();
            if target.json {
                print_json(out, &report)?;
            } else {
                for finding in &findings {
                    writeln!(
                        out,
                        "{:<7} {:<16} {}",
                        finding["severity"].as_str().unwrap_or(""),
                        finding["code"].as_str().unwrap_or(""),
                        finding["message"].as_str().unwrap_or("")
                    )?;
                }
                let warnings = findings
                    .iter()
                    .filter(|f| f["severity"] == "warning")
                    .count();
                writeln!(
                    out,
                    "DRC: {errors} errors, {warnings} warnings, {} unrouted",
                    report["airwires"].as_array().map_or(0, Vec::len)
                )?;
            }
            Ok(if errors == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
    }
}
