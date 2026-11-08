//! Implementation of the `ewerk` commands.

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use ew_core::Engine;
use ew_core::info::EngineInfo;
use ew_synth::DesignSize;
use serde_json::{Value, json};

use crate::{Command, NewArgs, ProjectCommand};

/// A command failure with a message for standard error.
#[derive(Debug)]
pub struct Failure(String);

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self {
        Self(error.to_string())
    }
}

impl From<serde_json::Error> for Failure {
    fn from(error: serde_json::Error) -> Self {
        Self(error.to_string())
    }
}

type Outcome = Result<ExitCode, Failure>;

/// Calls one engine method and turns an error reply into a failure.
fn call(engine: &mut Engine, method: &str, params: Value) -> Result<Value, Failure> {
    let mut request = serde_json::Map::new();
    request.insert("method".to_owned(), Value::String(method.to_owned()));
    request.insert("params".to_owned(), params);
    let request = Value::Object(request).to_string();
    let reply: Value = serde_json::from_str(&engine.dispatch(&request))?;
    match reply.get("result") {
        Some(result) => Ok(result.clone()),
        None => Err(Failure(
            reply["error"]["message"]
                .as_str()
                .unwrap_or("unknown engine error")
                .to_owned(),
        )),
    }
}

fn print_json(out: &mut impl Write, value: &Value) -> io::Result<()> {
    writeln!(
        out,
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    )
}

#[allow(
    clippy::cast_precision_loss,
    reason = "lengths are displayed with three decimals"
)]
fn format_mm(nm: &Value) -> String {
    nm.as_i64().map_or_else(
        || "?".to_owned(),
        |value| format!("{:.3} mm", value as f64 / 1e6),
    )
}

pub fn run(command: Command, out: &mut impl Write) -> Outcome {
    match command {
        Command::Info { json } => info(json, out),
        Command::Call { method, params } => call_once(&method, &params, out),
        Command::Methods => {
            for method in ew_core::rpc::METHODS {
                writeln!(out, "{method}")?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Rpc { events, no_journal } => {
            let stdin = io::stdin().lock();
            rpc(stdin, out, events, !no_journal)
        }
        Command::Project(command) => project(command, out),
        Command::Synth {
            size,
            dir,
            seed,
            json,
        } => synth(&size, &dir, seed, json, out),
        Command::Bench { size, json } => bench(&size, json, out),
    }
}

fn info(json: bool, out: &mut impl Write) -> Outcome {
    let info = EngineInfo::current();
    if json {
        writeln!(out, "{}", serde_json::to_string_pretty(&info)?)?;
    } else {
        writeln!(out, "{} {}", info.name, info.version)?;
        writeln!(out, "Target:  {}-{}", info.os, info.arch)?;
        writeln!(out, "Threads: {}", info.threads)?;
        writeln!(
            out,
            "Build:   {}",
            if info.debug { "debug" } else { "release" }
        )?;
    }
    Ok(ExitCode::SUCCESS)
}

fn call_once(method: &str, params: &str, out: &mut impl Write) -> Outcome {
    let params: Value = serde_json::from_str(params)
        .map_err(|err| Failure(format!("params are not valid JSON: {err}")))?;
    let request = json!({ "method": method, "params": params });
    let reply = ew_core::rpc::dispatch(&request.to_string());
    writeln!(out, "{reply}")?;
    let failed =
        serde_json::from_str::<Value>(&reply).map_or(true, |value| value.get("error").is_some());
    Ok(if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

/// Line-based JSON-RPC session: `{"id": 1, "method": …, "params": …}` in,
/// `{"id": 1, "result": …}` out.
pub fn rpc(
    input: impl io::BufRead,
    out: &mut impl Write,
    events: bool,
    journaling: bool,
) -> Outcome {
    let mut engine = if journaling {
        Engine::new()
    } else {
        Engine::without_journal()
    };
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let parsed: Result<Value, _> = serde_json::from_str(&line);
        let (id, reply) = match parsed {
            Ok(mut value) => {
                let id = value
                    .as_object_mut()
                    .and_then(|object| object.remove("id"))
                    .unwrap_or(Value::Null);
                let reply: Value = serde_json::from_str(&engine.dispatch(&value.to_string()))?;
                (id, reply)
            }
            Err(err) => (
                Value::Null,
                json!({"error": {"code": "invalidRequest", "message": format!("malformed request: {err}")}}),
            ),
        };
        let mut message = json!({"id": id});
        if let (Some(target), Some(source)) = (message.as_object_mut(), reply.as_object()) {
            target.extend(source.clone());
        }
        writeln!(out, "{message}")?;
        for event in engine.take_events() {
            if events {
                writeln!(out, "{}", json!({"event": event}))?;
            }
        }
        out.flush()?;
    }
    Ok(ExitCode::SUCCESS)
}

fn project(command: ProjectCommand, out: &mut impl Write) -> Outcome {
    let mut engine = Engine::without_journal();
    match command {
        ProjectCommand::New(args) => project_new(&mut engine, args, out),
        ProjectCommand::Info { path, json } => {
            let snapshot = call(&mut engine, "project.open", json!({"path": path}))?;
            if json {
                print_json(out, &snapshot)?;
            } else {
                print_summary(&snapshot, out)?;
            }
            Ok(ExitCode::SUCCESS)
        }
        ProjectCommand::Validate { path, json } => {
            let report = call(&mut engine, "project.validate", json!({"path": path}))?;
            if json {
                print_json(out, &report)?;
            } else {
                print_report(&report, out)?;
            }
            let errors = report["errors"].as_u64().unwrap_or(0);
            Ok(if errors == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        ProjectCommand::Archive { path, output } => {
            let snapshot = call(&mut engine, "project.open", json!({"path": path}))?;
            let output = output.unwrap_or_else(|| {
                let root = Path::new(snapshot["path"].as_str().unwrap_or("."));
                let name = ew_format::store::sanitize_file_name(
                    snapshot["name"].as_str().unwrap_or("project"),
                );
                root.parent().unwrap_or(root).join(format!("{name}.zip"))
            });
            let report = call(&mut engine, "project.archive", json!({"path": output}))?;
            writeln!(
                out,
                "Archived {} files ({} bytes) to {}",
                report["files"],
                report["bytes"],
                output.display()
            )?;
            Ok(ExitCode::SUCCESS)
        }
        ProjectCommand::Upgrade { path } => {
            let snapshot = call(&mut engine, "project.open", json!({"path": path}))?;
            let needed = snapshot["needsUpgrade"].as_bool().unwrap_or(false);
            let report = call(&mut engine, "project.save", json!({"all": true}))?;
            let written = report["written"].as_array().map_or(0, Vec::len);
            writeln!(
                out,
                "{} {written} files in schema version {}.",
                if needed {
                    "Upgraded and wrote"
                } else {
                    "Already current; rewrote"
                },
                ew_format::SCHEMA_VERSION
            )?;
            Ok(ExitCode::SUCCESS)
        }
        ProjectCommand::Templates => {
            let templates = call(&mut engine, "project.templates", Value::Null)?;
            for template in templates.as_array().into_iter().flatten() {
                let board = &template["board"];
                let detail = if board.is_null() {
                    String::from("no board")
                } else {
                    format!(
                        "{} × {}, {} layers",
                        format_mm(&board["width"]),
                        format_mm(&board["height"]),
                        board["layers"]
                    )
                };
                writeln!(
                    out,
                    "{:<16} {detail}",
                    template["id"].as_str().unwrap_or_default()
                )?;
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn project_new(engine: &mut Engine, args: NewArgs, out: &mut impl Write) -> Outcome {
    let name = args.name.unwrap_or_else(|| {
        args.dir.file_name().map_or_else(
            || "Project".to_owned(),
            |name| name.to_string_lossy().into_owned(),
        )
    });
    let templates = call(engine, "project.templates", Value::Null)?;
    let defaults = templates
        .as_array()
        .and_then(|list| {
            list.iter()
                .find(|template| template["id"] == args.template.as_str())
        })
        .map_or(Value::Null, |template| template["board"].clone());
    let mut params = json!({
        "path": args.dir,
        "name": name,
        "template": args.template,
        "units": args.units,
    });
    if defaults.is_object() {
        let mut board = defaults;
        let to_nm = ew_model::units::from_mm;
        if let Some(width) = args.width {
            board["width"] = json!(to_nm(width));
        }
        if let Some(height) = args.height {
            board["height"] = json!(to_nm(height));
        }
        if let Some(layers) = args.layers {
            board["layers"] = json!(layers);
        }
        if let Some(thickness) = args.thickness {
            board["thickness"] = json!(to_nm(thickness));
        }
        params["board"] = board;
    }
    let snapshot = call(engine, "project.create", params)?;
    if args.json {
        print_json(out, &snapshot)?;
    } else {
        writeln!(
            out,
            "Created {}",
            snapshot["manifestPath"].as_str().unwrap_or_default()
        )?;
        print_summary(&snapshot, out)?;
    }
    Ok(ExitCode::SUCCESS)
}

fn print_summary(snapshot: &Value, out: &mut impl Write) -> io::Result<()> {
    writeln!(
        out,
        "Project:   {}",
        snapshot["name"].as_str().unwrap_or_default()
    )?;
    let revision = snapshot["revision"].as_str().unwrap_or_default();
    if !revision.is_empty() {
        writeln!(out, "Revision:  {revision}")?;
    }
    writeln!(
        out,
        "Folder:    {}",
        snapshot["path"].as_str().unwrap_or_default()
    )?;
    if let Some(variables) = snapshot["variables"].as_object()
        && !variables.is_empty()
    {
        writeln!(out, "Variables:")?;
        for (key, value) in variables {
            writeln!(out, "  {key} = {}", value.as_str().unwrap_or_default())?;
        }
    }
    writeln!(out, "Documents:")?;
    for document in snapshot["documents"].as_array().into_iter().flatten() {
        let stats = &document["stats"];
        let detail = match document["kind"].as_str().unwrap_or_default() {
            "schematic" => format!("{} symbols, {} wires", stats["symbols"], stats["wires"]),
            "pcb" => {
                let size = &stats["boardSize"];
                let outline = if size.is_array() {
                    format!("{} × {}, ", format_mm(&size[0]), format_mm(&size[1]))
                } else {
                    String::new()
                };
                format!(
                    "{outline}{} layers, {} footprints, {} tracks, {} vias",
                    stats["copperLayers"], stats["footprints"], stats["tracks"], stats["vias"]
                )
            }
            "library" => format!(
                "{} symbols, {} footprints",
                stats["symbols"], stats["footprints"]
            ),
            "rules" => format!("{} net classes", stats["netClasses"]),
            "outputJob" => format!("{} outputs", stats["outputs"]),
            _ => String::new(),
        };
        writeln!(
            out,
            "  {:<10} {:<34} {detail}",
            document["kind"].as_str().unwrap_or_default(),
            document["path"].as_str().unwrap_or_default()
        )?;
    }
    if snapshot["recovery"].is_object() {
        writeln!(
            out,
            "Note: the project has unsaved changes from a session that ended unexpectedly. Open it in ElectronicWerk to restore them."
        )?;
    }
    Ok(())
}

fn print_report(report: &Value, out: &mut impl Write) -> io::Result<()> {
    for issue in report["issues"].as_array().into_iter().flatten() {
        let severity = issue["severity"].as_str().unwrap_or("info");
        let path = issue["path"]
            .as_str()
            .map(|path| format!("{path}: "))
            .unwrap_or_default();
        writeln!(
            out,
            "{severity:<7} {path}{} [{}]",
            issue["message"].as_str().unwrap_or_default(),
            issue["code"].as_str().unwrap_or_default()
        )?;
    }
    writeln!(
        out,
        "{} documents, {} objects checked: {} errors, {} warnings.",
        report["documents"], report["objects"], report["errors"], report["warnings"]
    )?;
    Ok(())
}

fn parse_size(size: &str) -> Result<DesignSize, Failure> {
    size.parse().map_err(Failure)
}

fn synth(size: &str, dir: &Path, seed: u64, json: bool, out: &mut impl Write) -> Outcome {
    let size = parse_size(size)?;
    let started = Instant::now();
    let project = ew_synth::generate(size, seed);
    let stats = ew_synth::stats(&project);
    let created =
        ew_format::store::create_project(dir, &project).map_err(|err| Failure(err.to_string()))?;
    let elapsed = started.elapsed();
    let manifest = created.root.join(&created.manifest_file);
    if json {
        print_json(
            out,
            &json!({
                "manifestPath": manifest.to_string_lossy(),
                "size": size.name(),
                "seed": seed,
                "symbols": stats.symbols,
                "sheets": stats.sheets,
                "footprints": stats.footprints,
                "pads": stats.pads,
                "nets": stats.nets,
                "tracks": stats.tracks,
                "vias": stats.vias,
                "zones": stats.zones,
                "copperLayers": stats.copper_layers,
                "milliseconds": elapsed.as_millis(),
            }),
        )?;
    } else {
        writeln!(
            out,
            "Created synthetic {size} design: {}",
            manifest.display()
        )?;
        writeln!(
            out,
            "{} symbols on {} sheets, {} footprints ({} pads), {} nets, {} tracks, {} vias, {} zones, {} copper layers ({} ms)",
            stats.symbols,
            stats.sheets,
            stats.footprints,
            stats.pads,
            stats.nets,
            stats.tracks,
            stats.vias,
            stats.zones,
            stats.copper_layers,
            elapsed.as_millis()
        )?;
    }
    Ok(ExitCode::SUCCESS)
}

fn bench(size: &str, json: bool, out: &mut impl Write) -> Outcome {
    let size = parse_size(size)?;
    let dir = std::env::temp_dir().join(format!(
        "ewerk-bench-{}-{}",
        size.name(),
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let project = ew_synth::generate(size, ew_synth::DEFAULT_SEED);
    let created =
        ew_format::store::create_project(&dir, &project).map_err(|err| Failure(err.to_string()))?;
    let mut open_ms = f64::MAX;
    let mut save_ms = f64::MAX;
    let mut scene_ms = f64::MAX;
    let mut primitives = 0;
    for _ in 0..3 {
        let started = Instant::now();
        let loaded =
            ew_format::store::load_project(&dir).map_err(|err| Failure(err.to_string()))?;
        open_ms = open_ms.min(started.elapsed().as_secs_f64() * 1e3);
        let board = loaded
            .project
            .documents_of(ew_model::document::DocumentKind::Pcb)
            .next()
            .map(|entry| entry.id);
        if let Some(board) = board {
            let options = ew_scene::Options {
                resolve: &str::to_owned,
                labels: ew_scene::TitleLabels::default(),
            };
            let started = Instant::now();
            let scene = ew_scene::build(&loaded.project, board, &options)
                .map_err(|err| Failure(err.to_string()))?;
            scene_ms = scene_ms.min(started.elapsed().as_secs_f64() * 1e3);
            let stats = scene.meta.stats;
            primitives =
                stats.segments + stats.arcs + stats.circles + stats.rects + stats.triangles;
        }
        let started = Instant::now();
        ew_format::store::save_project(
            &dir,
            &created.manifest_file,
            &loaded.project,
            &ew_format::SavePlan::everything(&loaded.project),
        )
        .map_err(|err| Failure(err.to_string()))?;
        save_ms = save_ms.min(started.elapsed().as_secs_f64() * 1e3);
    }
    let _ = std::fs::remove_dir_all(&dir);
    if json {
        print_json(
            out,
            &json!({
                "size": size.name(),
                "openMs": open_ms,
                "saveMs": save_ms,
                "sceneMs": scene_ms,
                "primitives": primitives,
            }),
        )?;
    } else {
        writeln!(
            out,
            "{size} design: open {open_ms:.0} ms, save {save_ms:.0} ms, board scene {scene_ms:.0} ms ({primitives} primitives) (best of 3)"
        )?;
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_to_string(command: Command) -> (ExitCode, String) {
        let mut buffer = Vec::new();
        let code = run(command, &mut buffer).unwrap();
        (code, String::from_utf8(buffer).unwrap())
    }

    #[test]
    fn info_text_and_json() {
        let (code, output) = run_to_string(Command::Info { json: false });
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(output.contains(ew_core::VERSION));
        let (_, output) = run_to_string(Command::Info { json: true });
        let value: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(value["version"], ew_core::VERSION);
    }

    #[test]
    fn call_and_methods() {
        let (code, output) = run_to_string(Command::Call {
            method: "engine.ping".into(),
            params: "[1,2]".into(),
        });
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(output.contains(r#""echo":[1,2]"#));
        let (code, output) = run_to_string(Command::Call {
            method: "nope".into(),
            params: "null".into(),
        });
        assert_eq!(code, ExitCode::FAILURE);
        assert!(output.contains("methodNotFound"));
        let mut buffer = Vec::new();
        assert!(
            run(
                Command::Call {
                    method: "engine.ping".into(),
                    params: "{".into(),
                },
                &mut buffer
            )
            .is_err()
        );
        let (_, output) = run_to_string(Command::Methods);
        assert_eq!(output.lines().count(), ew_core::rpc::METHODS.len());
    }

    #[test]
    fn project_commands() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("Blinky");
        let (code, output) = run_to_string(Command::Project(ProjectCommand::New(NewArgs {
            dir: target.clone(),
            name: None,
            template: "board-4l".into(),
            width: Some(50.0),
            height: Some(40.5),
            layers: None,
            thickness: None,
            units: "mm".into(),
            json: false,
        })));
        assert_eq!(code, ExitCode::SUCCESS, "{output}");
        assert!(output.contains("Blinky.ewproj"), "{output}");
        assert!(
            output.contains("50.000 mm × 40.500 mm, 4 layers"),
            "{output}"
        );

        let (code, output) = run_to_string(Command::Project(ProjectCommand::Info {
            path: target.clone(),
            json: false,
        }));
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(output.contains("Project:   Blinky"));

        let (code, output) = run_to_string(Command::Project(ProjectCommand::Validate {
            path: target.clone(),
            json: false,
        }));
        assert_eq!(code, ExitCode::SUCCESS, "{output}");
        assert!(output.contains("0 errors"));

        std::fs::write(target.join("rules/design.ewrules"), "{").unwrap();
        let (code, output) = run_to_string(Command::Project(ProjectCommand::Validate {
            path: target.clone(),
            json: true,
        }));
        assert_eq!(code, ExitCode::FAILURE);
        assert!(output.contains("unreadable-document"));

        let (_, output) = run_to_string(Command::Project(ProjectCommand::Templates));
        assert!(output.contains("arduino-shield"));
    }

    #[test]
    fn archive_and_upgrade() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("Arch");
        run_to_string(Command::Project(ProjectCommand::New(NewArgs {
            dir: target.clone(),
            name: Some("Arch".into()),
            template: "empty".into(),
            width: None,
            height: None,
            layers: None,
            thickness: None,
            units: "mil".into(),
            json: true,
        })));
        let (code, output) = run_to_string(Command::Project(ProjectCommand::Archive {
            path: target.clone(),
            output: None,
        }));
        assert_eq!(code, ExitCode::SUCCESS, "{output}");
        assert!(dir.path().join("Arch.zip").is_file());
        let (code, output) =
            run_to_string(Command::Project(ProjectCommand::Upgrade { path: target }));
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(output.contains("Already current"), "{output}");
    }

    #[test]
    fn synth_and_bench() {
        let dir = tempfile::tempdir().unwrap();
        let (code, output) = run_to_string(Command::Synth {
            size: "s".into(),
            dir: dir.path().join("s"),
            seed: 1,
            json: true,
        });
        assert_eq!(code, ExitCode::SUCCESS);
        let value: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(value["symbols"], 50);
        let mut buffer = Vec::new();
        assert!(
            run(
                Command::Synth {
                    size: "huge".into(),
                    dir: dir.path().join("x"),
                    seed: 1,
                    json: false,
                },
                &mut buffer
            )
            .is_err()
        );
        let (code, output) = run_to_string(Command::Bench {
            size: "S".into(),
            json: true,
        });
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(output.contains("openMs"));
    }

    #[test]
    fn rpc_session_keeps_state() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p");
        let input = format!(
            "{}\n\n{}\nnot json\n{}\n",
            json!({"id": 1, "method": "project.create", "params": {"path": path, "name": "P"}}),
            json!({"id": 2, "method": "project.setProperties", "params": {"revision": "C"}}),
            json!({"id": "x", "method": "project.info"}),
        );
        let mut buffer = Vec::new();
        rpc(input.as_bytes(), &mut buffer, true, false).unwrap();
        let lines: Vec<Value> = String::from_utf8(buffer)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let replies: Vec<&Value> = lines
            .iter()
            .filter(|line| line.get("event").is_none())
            .collect();
        assert_eq!(replies.len(), 4);
        assert_eq!(replies[0]["id"], 1);
        assert!(replies[1]["result"]["dirty"].as_bool().unwrap());
        assert_eq!(replies[2]["error"]["code"], "invalidRequest");
        assert_eq!(replies[3]["id"], "x");
        assert_eq!(replies[3]["result"]["revision"], "C");
        assert!(lines.iter().any(|line| line["event"]["type"] == "changed"));
    }
}
