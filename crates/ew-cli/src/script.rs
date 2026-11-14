//! `ewerk script`: engine calls from a file, in one engine session.
//!
//! A script is JSON (an array of steps, or `{"project", "steps"}`) or JSON
//! Lines (one step per line; empty lines and lines starting with `#` or
//! `//` are skipped). A step is `{"method": "…", "params": …}`.

use std::io::{self, Read as _, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Args;
use ew_core::Engine;
use serde_json::{Value, json};

use crate::commands::{Failure, Outcome, print_json};

#[derive(Debug, Args)]
pub struct ScriptArgs {
    /// The script (`.json` or `.jsonl`; `-` reads standard input).
    pub file: PathBuf,
    /// Open this project first and save it at the end.
    #[arg(long)]
    pub project: Option<PathBuf>,
    /// Run the remaining steps after a step failed.
    #[arg(long)]
    pub keep_going: bool,
    /// Do not save the project given with `--project`.
    #[arg(long)]
    pub dry_run: bool,
    /// Print every step's result as JSON.
    #[arg(long)]
    pub json: bool,
}

/// One step of a script.
#[derive(Debug)]
pub struct Step {
    /// Line or index, for messages.
    pub line: usize,
    pub method: String,
    pub params: Value,
}

/// Reads the steps (and an optional project) of a script.
///
/// # Errors
///
/// Fails if the text is neither a JSON script nor JSON Lines.
pub fn parse(text: &str) -> Result<(Option<String>, Vec<Step>), Failure> {
    let trimmed = text.trim_start();
    if trimmed.starts_with('[') || trimmed.starts_with('{') {
        // A whole JSON document?
        if let Ok(value) = serde_json::from_str::<Value>(text) {
            let (project, steps) = match value {
                Value::Array(steps) => (None, steps),
                Value::Object(mut object) if object.contains_key("steps") => {
                    let project = object
                        .get("project")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    let Some(Value::Array(steps)) = object.remove("steps") else {
                        return Err(Failure("`steps` must be a list".to_owned()));
                    };
                    (project, steps)
                }
                Value::Object(_) => (None, vec![value]),
                _ => return Err(Failure("a script is a list of steps".to_owned())),
            };
            let steps = steps
                .into_iter()
                .enumerate()
                .map(|(index, step)| step_of(index + 1, step))
                .collect::<Result<_, _>>()?;
            return Ok((project, steps));
        }
    }
    let mut steps = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let value: Value = serde_json::from_str(line)
            .map_err(|error| Failure(format!("line {}: {error}", index + 1)))?;
        steps.push(step_of(index + 1, value)?);
    }
    Ok((None, steps))
}

fn step_of(line: usize, value: Value) -> Result<Step, Failure> {
    let Value::Object(mut object) = value else {
        return Err(Failure(format!("step {line}: a step is an object")));
    };
    let Some(Value::String(method)) = object.remove("method") else {
        return Err(Failure(format!("step {line}: `method` is missing")));
    };
    let params = object.remove("params").unwrap_or(Value::Null);
    Ok(Step {
        line,
        method,
        params,
    })
}

fn read(file: &Path) -> Result<String, Failure> {
    if file.as_os_str() == "-" {
        let mut text = String::new();
        io::stdin().lock().read_to_string(&mut text)?;
        Ok(text)
    } else {
        std::fs::read_to_string(file)
            .map_err(|error| Failure(format!("cannot read {}: {error}", file.display())))
    }
}

/// A short description of a step's result for the text output.
fn describe(method: &str, result: &Value) -> String {
    let text = |key: &str| result[key].as_str().unwrap_or("").to_owned();
    match method {
        "design.place" => format!("{} {}", text("designator"), text("value")),
        "design.connect" => format!("{} → {}", text("net"), list(&result["connected"])),
        "design.route" => format!(
            "{} routed, {} unrouted",
            result["routed"], result["unrouted"]
        ),
        "project.create" | "project.open" => text("name"),
        _ => String::new(),
    }
}

fn list(value: &Value) -> String {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Runs a script; exit code 1 if a step failed.
pub fn run(args: &ScriptArgs, out: &mut impl Write) -> Outcome {
    let text = read(&args.file)?;
    let (script_project, steps) = parse(&text)?;
    // A relative project of the script file is relative to the script.
    let base = args
        .file
        .parent()
        .filter(|_| args.file.as_os_str() != "-")
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let project = args
        .project
        .clone()
        .or_else(|| script_project.map(|p| base.join(p)));
    let mut engine = Engine::without_journal();
    if let Some(project) = &project {
        let reply = dispatch(&mut engine, "project.open", &json!({ "path": project }));
        if let Some(error) = reply.get("error") {
            return Err(Failure(format!(
                "cannot open {}: {}",
                project.display(),
                error["message"].as_str().unwrap_or("")
            )));
        }
    }
    let mut results = Vec::new();
    let mut failed = 0;
    for step in &steps {
        let reply = dispatch(&mut engine, &step.method, &step.params);
        let ok = reply.get("error").is_none();
        if args.json {
            results.push(json!({
                "line": step.line,
                "method": step.method,
                "result": reply.get("result"),
                "error": reply.get("error"),
            }));
        } else if ok {
            writeln!(
                out,
                "ok    {:<24} {}",
                step.method,
                describe(&step.method, &reply["result"])
            )?;
        } else {
            writeln!(
                out,
                "error {:<24} line {}: {}",
                step.method,
                step.line,
                reply["error"]["message"].as_str().unwrap_or("")
            )?;
        }
        if !ok {
            failed += 1;
            if !args.keep_going {
                break;
            }
        }
    }
    let mut saved = false;
    if project.is_some() && !args.dry_run && (failed == 0 || args.keep_going) {
        let info = dispatch(&mut engine, "project.info", &Value::Null);
        if info["result"]["history"]["dirty"] == true {
            let reply = dispatch(&mut engine, "project.save", &Value::Null);
            if let Some(error) = reply.get("error") {
                return Err(Failure(format!(
                    "cannot save: {}",
                    error["message"].as_str().unwrap_or("")
                )));
            }
            saved = true;
        }
    }
    if args.json {
        print_json(
            out,
            &json!({ "steps": results, "failed": failed, "saved": saved }),
        )?;
    } else {
        writeln!(
            out,
            "{} steps, {failed} failed{}",
            steps.len(),
            if saved { ", project saved" } else { "" }
        )?;
    }
    Ok(if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn dispatch(engine: &mut Engine, method: &str, params: &Value) -> Value {
    let request = json!({ "method": method, "params": params });
    serde_json::from_str(&engine.dispatch(&request.to_string())).unwrap_or_else(
        |error| json!({"error": {"code": "internal", "message": error.to_string()}}),
    )
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn reads_json_and_json_lines() {
        let (project, steps) =
            parse(r#"{"project": "Blinker", "steps": [{"method": "design.nets"}]}"#).unwrap();
        assert_eq!(project.as_deref(), Some("Blinker"));
        assert_eq!(steps[0].method, "design.nets");
        let (_, steps) = parse(
            "# a comment\n{\"method\": \"design.place\", \"params\": {\"symbol\": \"R\"}}\n\n// another\n{\"method\": \"design.route\"}\n",
        )
        .unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].line, 2);
        assert_eq!(steps[1].line, 5);
        assert_eq!(steps[0].params["symbol"], "R");
        assert!(parse("{\"params\": 1}").is_err());
        assert!(parse("not json").is_err());
    }
}
