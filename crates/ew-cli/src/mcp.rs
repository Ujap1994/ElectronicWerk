//! `ewerk mcp`: a Model Context Protocol server on standard input and
//! output, so AI agents (Claude, Cursor, VS Code, …) can design with
//! ElectronicWerk. JSON-RPC 2.0, one message per line.
//!
//! The tools wrap the engine's `design.*` methods and the checks; one tool
//! calls any engine method for everything else. The server keeps one
//! project open; changes are saved after every tool call unless the server
//! runs with `--no-autosave`.

use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use clap::Args;
use ew_core::Engine;
use serde_json::{Map, Value, json};

use crate::commands::Outcome;

/// Protocol versions this server speaks, newest first.
const VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

#[derive(Debug, Args)]
pub struct McpArgs {
    /// Open this project at the start.
    #[arg(long)]
    pub project: Option<std::path::PathBuf>,
    /// Keep changes in memory until the `save_project` tool runs.
    #[arg(long)]
    pub no_autosave: bool,
}

const INSTRUCTIONS: &str = "ElectronicWerk designs circuit boards. Work in this order: \
open_project or create_project; place_part for every part (symbols from the project library \
or the base library, e.g. Device:R; footprints as names or wizard presets such as 0805 or \
SOIC-8, see list_footprint_presets); connect_pins to make nets (pins as R1.2 or U1.VCC); \
run_erc; update_board; arrange_footprints or place_footprint; route_board; run_drc; \
generate_outputs. Lengths are millimetres, angles degrees, y points up. Read list_parts and \
board_info to see the state. Changes are saved after each tool unless autosave is off.";

/// A tool: name, title, description, input schema, read-only.
struct Tool {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    schema: Value,
    read_only: bool,
}

#[allow(clippy::needless_pass_by_value, reason = "called with json! literals")]
fn object(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

fn board_property() -> Value {
    json!({"type": "string", "description": "Board name (default: the first board)"})
}

#[allow(clippy::too_many_lines, reason = "one entry per tool")]
fn tools() -> Vec<Tool> {
    let xy = json!({
        "type": "array",
        "items": {"type": "number"},
        "minItems": 2,
        "maxItems": 2,
        "description": "[x, y] in millimetres (y up)",
    });
    vec![
        Tool {
            name: "open_project",
            title: "Open project",
            description: "Opens a project (its folder or .ewproj file); closes the open one.",
            schema: object(json!({"path": {"type": "string"}}), &["path"]),
            read_only: false,
        },
        Tool {
            name: "create_project",
            title: "Create project",
            description: "Creates a project in a new folder and opens it. Templates: empty, board-2l (default), board-4l, arduino-shield.",
            schema: object(
                json!({
                    "path": {"type": "string", "description": "New folder"},
                    "name": {"type": "string"},
                    "template": {"type": "string", "enum": ["empty", "board-2l", "board-4l", "arduino-shield", "demo"]},
                    "width": {"type": "number", "description": "Board width in mm"},
                    "height": {"type": "number", "description": "Board height in mm"},
                }),
                &["path", "name"],
            ),
            read_only: false,
        },
        Tool {
            name: "project_info",
            title: "Project info",
            description: "The open project: name, documents, units, unsaved changes.",
            schema: object(json!({}), &[]),
            read_only: true,
        },
        Tool {
            name: "save_project",
            title: "Save project",
            description: "Writes the changes to disk.",
            schema: object(json!({}), &[]),
            read_only: false,
        },
        Tool {
            name: "list_parts",
            title: "List parts",
            description: "The parts of the schematic: designator, value, footprint, sheet and every pin with number, name and net.",
            schema: object(json!({}), &[]),
            read_only: true,
        },
        Tool {
            name: "list_nets",
            title: "List nets",
            description: "The nets with their pins (designator.pin).",
            schema: object(json!({}), &[]),
            read_only: true,
        },
        Tool {
            name: "list_symbols",
            title: "List project symbols",
            description: "The symbols of the project library (name, description, pins, default footprint).",
            schema: object(json!({}), &[]),
            read_only: true,
        },
        Tool {
            name: "search_library",
            title: "Search the base library",
            description: "Full-text search in the base library (KiCad symbols and footprints, if installed). Use the full name (library:name) in place_part or as a footprint.",
            schema: object(
                json!({
                    "text": {"type": "string"},
                    "kind": {"type": "string", "enum": ["symbol", "footprint"]},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100},
                }),
                &["text"],
            ),
            read_only: true,
        },
        Tool {
            name: "list_footprint_presets",
            title: "Footprint presets",
            description: "Footprint wizard presets usable as a footprint without any library: 0402 … 2512 chips, SOIC, TSSOP, QFP, QFN, BGA, pin headers.",
            schema: object(json!({}), &[]),
            read_only: true,
        },
        Tool {
            name: "place_part",
            title: "Place part",
            description: "Places a symbol on a sheet. A symbol missing in the project is taken from the base library. Without `at`, a free place is used. Returns the designator and the pins.",
            schema: object(
                json!({
                    "symbol": {"type": "string", "description": "R, C, LED, … or a base library name such as Device:R"},
                    "component": {"type": "string", "description": "A component of the project library instead of a symbol"},
                    "designator": {"type": "string"},
                    "value": {"type": "string"},
                    "footprint": {"type": "string", "description": "Footprint name or wizard preset (0805, SOIC-8, …)"},
                    "sheet": {"type": "string"},
                    "at": xy.clone(),
                    "rotation": {"type": "number", "description": "Degrees, counter-clockwise"},
                    "mirror": {"type": "boolean"},
                    "unit": {"type": "integer", "minimum": 1},
                }),
                &[],
            ),
            read_only: false,
        },
        Tool {
            name: "edit_part",
            title: "Edit part",
            description: "Changes the value, footprint, designator or properties (MPN, Manufacturer, …) of a part.",
            schema: object(
                json!({
                    "designator": {"type": "string"},
                    "value": {"type": "string"},
                    "footprint": {"type": "string"},
                    "rename": {"type": "string"},
                    "properties": {"type": "object", "additionalProperties": {"type": "string"}},
                }),
                &["designator"],
            ),
            read_only: false,
        },
        Tool {
            name: "connect_pins",
            title: "Connect pins",
            description: "Connects pins to a net: a short wire and a net label (global across sheets) or a power port. Pins are designator.pin with the pin number or name (R1.2, U1.VCC).",
            schema: object(
                json!({
                    "net": {"type": "string"},
                    "pins": {"type": "array", "items": {"type": "string"}, "minItems": 1},
                    "style": {"type": "string", "enum": ["auto", "label", "global", "power"]},
                }),
                &["net", "pins"],
            ),
            read_only: false,
        },
        Tool {
            name: "mark_no_connect",
            title: "Mark pins unconnected",
            description: "Puts no-connect flags on pins that stay open on purpose.",
            schema: object(
                json!({"pins": {"type": "array", "items": {"type": "string"}, "minItems": 1}}),
                &["pins"],
            ),
            read_only: false,
        },
        Tool {
            name: "run_erc",
            title: "Electrical rules check",
            description: "Checks the schematic: unconnected pins, conflicting outputs, single-pin nets, … Returns the issues with severity.",
            schema: object(json!({}), &[]),
            read_only: true,
        },
        Tool {
            name: "update_board",
            title: "Update board from schematic",
            description: "Brings the board up to date with the schematic: new footprints (placed below the board), nets, values.",
            schema: object(json!({"board": board_property()}), &[]),
            read_only: false,
        },
        Tool {
            name: "board_info",
            title: "Board info",
            description: "The board outline (mm), copper layers, footprints with position, rotation and side, and the number of unrouted connections.",
            schema: object(json!({"board": board_property()}), &[]),
            read_only: true,
        },
        Tool {
            name: "place_footprint",
            title: "Place footprint",
            description: "Puts a footprint at a position (mm), with a rotation (degrees) and side.",
            schema: object(
                json!({
                    "designator": {"type": "string"},
                    "at": xy,
                    "rotation": {"type": "number"},
                    "side": {"type": "string", "enum": ["top", "bottom"]},
                    "board": board_property(),
                }),
                &["designator", "at"],
            ),
            read_only: false,
        },
        Tool {
            name: "arrange_footprints",
            title: "Arrange footprints",
            description: "Puts footprints in rows inside the board outline (default: all that are not on the board yet).",
            schema: object(
                json!({
                    "designators": {"type": "array", "items": {"type": "string"}},
                    "gap": {"type": "number", "description": "mm"},
                    "board": board_property(),
                }),
                &[],
            ),
            read_only: false,
        },
        Tool {
            name: "route_board",
            title: "Route board",
            description: "Routes the unrouted connections with 45° bends, one copper layer per connection. Reports what could not be routed.",
            schema: object(
                json!({
                    "nets": {"type": "array", "items": {"type": "string"}},
                    "layers": {"type": "array", "items": {"type": "string", "enum": ["top", "bottom"]}},
                    "width": {"type": "number", "description": "Track width in mm"},
                    "board": board_property(),
                }),
                &[],
            ),
            read_only: false,
        },
        Tool {
            name: "fill_zones",
            title: "Fill zones",
            description: "Fills the copper zones of the board.",
            schema: object(json!({"board": board_property()}), &[]),
            read_only: false,
        },
        Tool {
            name: "run_drc",
            title: "Design rules check",
            description: "Checks the board: clearances, track widths, unrouted connections, … Returns the findings.",
            schema: object(json!({"board": board_property()}), &[]),
            read_only: true,
        },
        Tool {
            name: "generate_outputs",
            title: "Generate manufacturing files",
            description: "Runs the project's output job: Gerber, drill, BOM, pick and place, PDF. Returns the written files.",
            schema: object(
                json!({"job": {"type": "string", "description": "Job name (default: the first)"}}),
                &[],
            ),
            read_only: false,
        },
        Tool {
            name: "undo",
            title: "Undo",
            description: "Undoes the last change (every tool call is one step).",
            schema: object(json!({}), &[]),
            read_only: false,
        },
        Tool {
            name: "redo",
            title: "Redo",
            description: "Redoes the last undone change.",
            schema: object(json!({}), &[]),
            read_only: false,
        },
        Tool {
            name: "list_engine_methods",
            title: "Engine methods",
            description: "Every method of the ElectronicWerk engine with a summary, for call_engine.",
            schema: object(json!({}), &[]),
            read_only: true,
        },
        Tool {
            name: "call_engine",
            title: "Call an engine method",
            description: "Calls any engine method (see list_engine_methods and the engine API reference). Lengths there are nanometres and objects are addressed by id.",
            schema: object(
                json!({"method": {"type": "string"}, "params": {}}),
                &["method"],
            ),
            read_only: false,
        },
    ]
}

/// The MCP server state.
struct Server {
    engine: Engine,
    autosave: bool,
    version: &'static str,
}

impl Server {
    #[allow(clippy::needless_pass_by_value, reason = "called with json! literals")]
    fn engine_call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let request = json!({ "method": method, "params": params });
        let reply: Value = serde_json::from_str(&self.engine.dispatch(&request.to_string()))
            .map_err(|error| error.to_string())?;
        match reply.get("result") {
            Some(result) => Ok(result.clone()),
            None => Err(reply["error"]["message"]
                .as_str()
                .unwrap_or("unknown engine error")
                .to_owned()),
        }
    }

    fn board_doc(&mut self, args: &Map<String, Value>) -> Result<Value, String> {
        let info = self.engine_call("design.board", pick(args, &["board"]))?;
        Ok(info["id"].clone())
    }

    #[allow(clippy::too_many_lines, reason = "one arm per tool")]
    fn tool(&mut self, name: &str, args: &Map<String, Value>) -> Result<Value, String> {
        let all = Value::Object(args.clone());
        match name {
            "open_project" => {
                let _ = self.engine_call("project.close", json!({ "discard": false }));
                self.engine_call("project.open", pick(args, &["path"]))
            }
            "create_project" => {
                let _ = self.engine_call("project.close", json!({ "discard": false }));
                let mut params = pick(args, &["path", "name", "template"]);
                if let (Some(w), Some(h)) = (
                    args.get("width").and_then(Value::as_f64),
                    args.get("height").and_then(Value::as_f64),
                ) {
                    #[allow(clippy::cast_possible_truncation, reason = "millimetres to nanometres")]
                    let board = json!({
                        "width": (w * 1e6).round() as i64,
                        "height": (h * 1e6).round() as i64,
                        "layers": 2,
                        "thickness": 1_600_000,
                    });
                    params["board"] = board;
                }
                if params.get("template").is_none() {
                    params["template"] = json!("board-2l");
                }
                self.engine_call("project.create", params)
            }
            "project_info" => self.engine_call("project.info", Value::Null),
            "save_project" => self.engine_call("project.save", Value::Null),
            "list_parts" => self.engine_call("design.components", Value::Null),
            "list_nets" => self.engine_call("design.nets", Value::Null),
            "list_symbols" => self.engine_call("schematic.symbols", Value::Null),
            "list_footprint_presets" => self.engine_call("design.presets", Value::Null),
            "search_library" => {
                let mut params = json!({ "text": args.get("text").cloned().unwrap_or_default() });
                if let Some(kind) = args.get("kind") {
                    params["kind"] = kind.clone();
                }
                params["limit"] = args.get("limit").cloned().unwrap_or(json!(20));
                let result = self.engine_call("library.search", params)?;
                let items: Vec<Value> = result["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|item| {
                        json!({
                            "name": format!(
                                "{}:{}",
                                item["library"].as_str().unwrap_or(""),
                                item["name"].as_str().unwrap_or("")
                            ),
                            "kind": item["kind"],
                            "description": item["description"],
                            "pins": item["pins"],
                        })
                    })
                    .collect();
                Ok(json!({ "total": result["total"], "items": items }))
            }
            "place_part" => self.engine_call("design.place", all),
            "edit_part" => self.engine_call("design.set", all),
            "connect_pins" => self.engine_call("design.connect", all),
            "mark_no_connect" => self.engine_call("design.noConnect", all),
            "run_erc" => self.engine_call("schematic.erc", Value::Null),
            "update_board" => self.engine_call("design.updateBoard", all),
            "board_info" => self.engine_call("design.board", all),
            "place_footprint" => self.engine_call("design.placeFootprint", all),
            "arrange_footprints" => self.engine_call("design.arrange", all),
            "route_board" => self.engine_call("design.route", all),
            "fill_zones" => {
                let doc = self.board_doc(args)?;
                self.engine_call("pcb.fillZones", json!({ "doc": doc }))
            }
            "run_drc" => {
                let doc = self.board_doc(args)?;
                let report = self.engine_call("pcb.drc", json!({ "doc": doc }))?;
                let findings: Vec<Value> = report["findings"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|f| f["excluded"] != true)
                    .map(|f| json!({"severity": f["severity"], "code": f["code"], "message": f["message"]}))
                    .collect();
                Ok(json!({
                    "errors": findings.iter().filter(|f| f["severity"] == "error").count(),
                    "warnings": findings.iter().filter(|f| f["severity"] == "warning").count(),
                    "unrouted": report["airwires"].as_array().map_or(0, Vec::len),
                    "findings": findings,
                }))
            }
            "generate_outputs" => {
                let info = self.engine_call("project.info", Value::Null)?;
                let wanted = args.get("job").and_then(Value::as_str);
                let job = info["documents"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|doc| doc["kind"] == "outputJob")
                    .find(|doc| wanted.is_none_or(|name| doc["name"] == name))
                    .map(|doc| doc["id"].clone())
                    .ok_or_else(|| "the project has no such output job".to_owned())?;
                self.engine_call("outputs.run", json!({ "job": job }))
            }
            "undo" => self.engine_call("edit.undo", Value::Null),
            "redo" => self.engine_call("edit.redo", Value::Null),
            "list_engine_methods" => Ok(ew_core::docs::methods()),
            "call_engine" => {
                let method = args
                    .get("method")
                    .and_then(Value::as_str)
                    .ok_or_else(|| "method is missing".to_owned())?
                    .to_owned();
                let params = args.get("params").cloned().unwrap_or(Value::Null);
                self.engine_call(&method, params)
            }
            other => Err(format!("unknown tool {other}")),
        }
    }

    /// Saves after a change (autosave).
    fn autosave(&mut self) {
        if !self.autosave {
            return;
        }
        if let Ok(info) = self.engine_call("project.info", Value::Null)
            && info["history"]["dirty"] == true
        {
            let _ = self.engine_call("project.save", Value::Null);
        }
    }

    fn call_tool(&mut self, params: &Value) -> Value {
        let name = params["name"].as_str().unwrap_or_default().to_owned();
        let args = params["arguments"].as_object().cloned().unwrap_or_default();
        let read_only = tools()
            .iter()
            .find(|tool| tool.name == name)
            .is_some_and(|tool| tool.read_only);
        let outcome = self.tool(&name, &args);
        if !read_only && outcome.is_ok() {
            self.autosave();
        }
        match outcome {
            Ok(value) => {
                let structured = if value.is_object() {
                    value.clone()
                } else {
                    json!({ "result": value })
                };
                let mut result = json!({
                    "content": [{"type": "text", "text": value.to_string()}],
                    "isError": false,
                });
                if self.version != "2024-11-05" && self.version != "2025-03-26" {
                    result["structuredContent"] = structured;
                }
                result
            }
            Err(message) => json!({
                "content": [{"type": "text", "text": message}],
                "isError": true,
            }),
        }
    }

    fn list_tools() -> Value {
        let list: Vec<Value> = tools()
            .into_iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "title": tool.title,
                    "description": tool.description,
                    "inputSchema": tool.schema,
                    "annotations": {
                        "title": tool.title,
                        "readOnlyHint": tool.read_only,
                        "destructiveHint": false,
                        "openWorldHint": false,
                    },
                })
            })
            .collect();
        json!({ "tools": list })
    }

    /// Handles one request; `None` for notifications.
    fn handle(&mut self, message: &Value) -> Option<Value> {
        let id = message.get("id").cloned();
        let method = message["method"].as_str().unwrap_or_default();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let result = match method {
            "initialize" => {
                let requested = params["protocolVersion"].as_str().unwrap_or_default();
                self.version = VERSIONS
                    .iter()
                    .find(|v| **v == requested)
                    .copied()
                    .unwrap_or(VERSIONS[0]);
                Ok(json!({
                    "protocolVersion": self.version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {
                        "name": "electronicwerk",
                        "title": "ElectronicWerk",
                        "version": ew_core::VERSION,
                    },
                    "instructions": INSTRUCTIONS,
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(Self::list_tools()),
            "tools/call" => Ok(self.call_tool(&params)),
            _ if method.starts_with("notifications/") => return None,
            other => Err((-32601, format!("method not found: {other}"))),
        };
        let id = id?;
        Some(match result {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err((code, message)) => {
                json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
            }
        })
    }
}

/// Picks some arguments.
fn pick(args: &Map<String, Value>, keys: &[&str]) -> Value {
    let mut out = Map::new();
    for key in keys {
        if let Some(value) = args.get(*key) {
            out.insert((*key).to_owned(), value.clone());
        }
    }
    Value::Object(out)
}

/// Serves MCP until standard input ends.
pub fn serve(input: impl BufRead, out: &mut impl Write, args: &McpArgs) -> Outcome {
    let mut server = Server {
        engine: Engine::without_journal(),
        autosave: !args.no_autosave,
        version: VERSIONS[0],
    };
    if let Some(project) = &args.project
        && let Err(message) = server.engine_call("project.open", json!({ "path": project }))
    {
        let _ = writeln!(
            io::stderr(),
            "ewerk mcp: cannot open {}: {message}",
            project.display()
        );
    }
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(batch)) => {
                let replies: Vec<Value> = batch.iter().filter_map(|m| server.handle(m)).collect();
                (!replies.is_empty()).then_some(Value::Array(replies))
            }
            Ok(message) => server.handle(&message),
            Err(error) => Some(json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {"code": -32700, "message": format!("parse error: {error}")},
            })),
        };
        if let Some(reply) = reply {
            writeln!(out, "{reply}")?;
            out.flush()?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::{McpArgs, serve, tools};
    use serde_json::{Value, json};

    fn session(messages: &[Value]) -> Vec<Value> {
        let input: String = messages.iter().map(|m| m.to_string() + "\n").collect();
        let mut out = Vec::new();
        let _ = serve(
            input.as_bytes(),
            &mut out,
            &McpArgs {
                project: None,
                no_autosave: false,
            },
        )
        .unwrap();
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn negotiates_and_lists_tools() {
        let replies = session(&[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "1"}}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "ping"}),
            json!({"jsonrpc": "2.0", "id": 4, "method": "resources/list"}),
            json!({"jsonrpc": "2.0", "id": 5, "method": "initialize", "params": {"protocolVersion": "1999-01-01"}}),
        ]);
        assert_eq!(replies.len(), 5, "the notification has no reply");
        assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(replies[0]["result"]["serverInfo"]["name"], "electronicwerk");
        let listed = replies[1]["result"]["tools"].as_array().unwrap();
        assert_eq!(listed.len(), tools().len());
        assert!(listed.iter().all(|t| t["inputSchema"]["type"] == "object"));
        assert_eq!(replies[2]["result"], json!({}));
        assert_eq!(replies[3]["error"]["code"], -32601);
        assert_eq!(replies[4]["result"]["protocolVersion"], "2025-11-25");
    }

    #[test]
    fn reports_tool_errors_in_the_result() {
        let replies = session(&[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-11-25"}}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "list_parts", "arguments": {}}}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "list_engine_methods", "arguments": {}}}),
        ]);
        assert_eq!(replies[1]["result"]["isError"], true);
        assert!(
            replies[1]["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("project")
        );
        assert_eq!(replies[2]["result"]["isError"], false);
        let methods = &replies[2]["result"]["structuredContent"]["methods"];
        assert!(
            methods
                .as_array()
                .unwrap()
                .iter()
                .any(|m| m["name"] == "design.route")
        );
    }
}
