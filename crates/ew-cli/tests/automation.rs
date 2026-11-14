//! Automation through the real `ewerk` binary: a board designed with the
//! `schematic` and `board` commands, a script, a JSON-RPC 2.0 session and an
//! MCP session, as CI pipelines and AI agents use them.

#![allow(
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "test code"
)]

use std::io::{BufRead as _, BufReader, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::{Value, json};

fn ewerk(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ewerk"))
        .args(args)
        // No base library: everything comes from the project and the wizard.
        .env("EW_LIBRARY_DIR", std::env::temp_dir().join("ew-no-library"))
        .output()
        .unwrap()
}

fn ok(args: &[&str]) -> String {
    let output = ewerk(args);
    assert!(
        output.status.success(),
        "ewerk {args:?} failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn json_of(args: &[&str]) -> Value {
    serde_json::from_str(&ok(args)).unwrap()
}

fn new_project(dir: &Path) -> PathBuf {
    let project = dir.join("Blinker");
    let path = project.to_str().unwrap();
    ok(&["project", "new", path, "--width", "40", "--height", "30"]);
    project
}

fn files_in(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for entry in walk(dir) {
        out.push(entry.file_name().unwrap().to_string_lossy().into_owned());
    }
    out
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(walk(&path));
            } else {
                out.push(path);
            }
        }
    }
    out
}

#[test]
fn designs_a_board_from_the_command_line() {
    let dir = tempfile::tempdir().unwrap();
    let project = new_project(dir.path());
    let p = project.to_str().unwrap();

    let placed = ok(&[
        "schematic",
        "place",
        p,
        "R",
        "--value",
        "330",
        "--footprint",
        "0805",
        "--at",
        "50,120",
    ]);
    assert!(
        placed.starts_with("Placed R1 (330) at [49.53,119.83] mm"),
        "{placed}"
    );
    ok(&[
        "schematic",
        "place",
        p,
        "LED",
        "--footprint",
        "0805",
        "--rotate",
        "90",
    ]);
    let c7 = json_of(&[
        "schematic",
        "place",
        p,
        "C",
        "--ref",
        "C7",
        "--footprint",
        "0603",
        "--json",
    ]);
    assert_eq!(c7["designator"], "C7");

    assert!(
        ok(&[
            "schematic",
            "connect",
            p,
            "VCC",
            "R1.1",
            "C7.1",
            "--style",
            "power"
        ])
        .contains("Connected R1.1, C7.1 to VCC")
    );
    ok(&["schematic", "connect", p, "LED_A", "R1.2", "D1.A"]);
    ok(&[
        "schematic",
        "connect",
        p,
        "GND",
        "D1.K",
        "C7.2",
        "--style",
        "power",
    ]);

    // Mistakes: exit code 3 with the reason.
    let wrong = ewerk(&["schematic", "connect", p, "X", "R1.9"]);
    assert_eq!(wrong.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&wrong.stderr).contains("R1 has no pin 9; its pins are 1, 2"));
    let usage = ewerk(&["schematic", "place", p, "R", "--at", "fifty"]);
    assert_eq!(usage.status.code(), Some(2));

    let parts = json_of(&["schematic", "list", p, "--json"]);
    assert_eq!(parts["components"].as_array().unwrap().len(), 3);
    let nets = ok(&["schematic", "nets", p]);
    assert!(nets.contains("/LED_A"), "{nets}");
    assert!(ok(&["schematic", "erc", p]).contains("ERC: 0 errors"));
    let netlist = ok(&["schematic", "netlist", p, "--format", "csv"]);
    assert!(netlist.contains("VCC"), "{netlist}");

    ok(&[
        "schematic",
        "set",
        p,
        "R1",
        "--value",
        "470",
        "--property",
        "MPN=RC0805FR-07470RL",
    ]);
    assert!(ok(&["schematic", "list", p]).contains("R1       470"));

    assert!(ok(&["board", "update", p]).contains("Applied"));
    let arranged = ok(&["board", "arrange", p]);
    assert!(arranged.contains("Placed C7, D1, R1"), "{arranged}");
    ok(&["board", "place", p, "R1", "--at", "30,15", "--rotate", "90"]);
    let routed = json_of(&["board", "route", p, "--json"]);
    assert_eq!(routed["unrouted"], 0, "{routed}");
    let info = json_of(&["board", "info", p, "--json"]);
    assert_eq!(info["unrouted"], 0);
    assert!(info["tracks"].as_u64().unwrap() >= 3);
    let drc = ewerk(&["board", "drc", p]);
    let text = String::from_utf8_lossy(&drc.stdout);
    assert_eq!(drc.status.code(), Some(0), "{text}");
    assert!(text.contains("DRC: 0 errors"), "{text}");

    ok(&["outputs", "run", p]);
    let written = files_in(&project.join("outputs"));
    assert!(
        written.iter().any(|f| {
            Path::new(f)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("gbr") || e.eq_ignore_ascii_case("gtl"))
        }),
        "{written:?}"
    );

    // The methods describe themselves.
    let methods = json_of(&["methods", "--json"]);
    let route = methods["methods"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "design.route")
        .unwrap();
    assert!(route["summary"].as_str().unwrap().contains("45°"));
}

#[test]
fn runs_scripts() {
    let dir = tempfile::tempdir().unwrap();
    let project = new_project(dir.path());
    let script = dir.path().join("build.jsonl");
    std::fs::write(
        &script,
        r#"# A blinking LED
{"method": "design.place", "params": {"symbol": "R", "footprint": "0805"}}
{"method": "design.place", "params": {"symbol": "LED", "footprint": "0805"}}
{"method": "design.connect", "params": {"net": "A", "pins": ["R1.2", "D1.A"]}}
"#,
    )
    .unwrap();
    let out = ok(&[
        "script",
        script.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
    ]);
    assert!(
        out.contains("ok    design.place             R1 10k"),
        "{out}"
    );
    assert!(out.contains("3 steps, 0 failed, project saved"), "{out}");
    // Saved: a new process sees the parts.
    assert!(ok(&["schematic", "list", project.to_str().unwrap()]).contains("D1"));

    // A failing step: exit code 1, the rest is skipped unless --keep-going.
    let failing = dir.path().join("failing.json");
    std::fs::write(
        &failing,
        json!({
            "project": "Blinker",
            "steps": [
                {"method": "design.connect", "params": {"net": "B", "pins": ["R9.1"]}},
                {"method": "design.place", "params": {"symbol": "C"}},
            ],
        })
        .to_string(),
    )
    .unwrap();
    let run = ewerk(&["script", failing.to_str().unwrap()]);
    assert_eq!(run.status.code(), Some(1));
    let text = String::from_utf8_lossy(&run.stdout);
    assert!(text.contains("error design.connect"), "{text}");
    assert!(
        text.contains("1 steps, 1 failed") || text.contains("2 steps, 1 failed"),
        "{text}"
    );
    assert!(!ok(&["schematic", "list", project.to_str().unwrap()]).contains("C1"));
    let run = ewerk(&[
        "script",
        failing.to_str().unwrap(),
        "--keep-going",
        "--json",
    ]);
    assert_eq!(run.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&run.stdout).unwrap();
    assert_eq!(report["failed"], 1);
    assert_eq!(report["saved"], true);
    assert_eq!(report["steps"][1]["result"]["designator"], "C1");
}

/// A process speaking line-delimited JSON.
struct Session {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
}

impl Session {
    fn start(args: &[&str]) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_ewerk"))
            .args(args)
            .env("EW_LIBRARY_DIR", std::env::temp_dir().join("ew-no-library"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            stdin,
            stdout,
        }
    }

    fn send(&mut self, message: Value) {
        writeln!(self.stdin, "{message}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn receive(&mut self) -> Value {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    fn request(&mut self, message: Value) -> Value {
        self.send(message);
        self.receive()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn speaks_json_rpc_2() {
    let mut rpc = Session::start(&["rpc", "--no-journal"]);
    let reply = rpc
        .request(json!({"jsonrpc": "2.0", "id": 1, "method": "engine.ping", "params": {"a": 1}}));
    assert_eq!(
        reply,
        json!({"jsonrpc": "2.0", "id": 1, "result": {"pong": true, "echo": {"a": 1}}})
    );
    let reply = rpc.request(json!({"jsonrpc": "2.0", "id": "x", "method": "no.such"}));
    assert_eq!(reply["error"]["code"], -32601);
    assert_eq!(reply["error"]["data"]["code"], "methodNotFound");
    let reply = rpc.request(json!({"jsonrpc": "2.0", "id": 2, "method": "design.components"}));
    assert_eq!(reply["error"]["code"], -32000);
    assert_eq!(reply["error"]["data"]["code"], "noProject");
    // A notification gets no reply; the batch answers its requests only.
    rpc.send(json!({"jsonrpc": "2.0", "method": "engine.ping"}));
    let batch = rpc.request(json!([
        {"jsonrpc": "2.0", "id": 3, "method": "engine.ping"},
        {"jsonrpc": "2.0", "method": "engine.ping"},
        {"jsonrpc": "2.0", "id": 4, "method": "design.presets"},
    ]));
    let replies = batch.as_array().unwrap();
    assert_eq!(replies.len(), 2);
    assert_eq!(replies[0]["id"], 3);
    assert_eq!(replies[1]["id"], 4);
    // The short form keeps working.
    let reply = rpc.request(json!({"id": 5, "method": "engine.ping"}));
    assert_eq!(reply["result"]["pong"], true);
    assert!(reply.get("jsonrpc").is_none());
    let reply =
        rpc.request(json!({"jsonrpc": "2.0", "id": 6, "method": "engine.ping", "params": [1]}));
    assert_eq!(reply["result"]["echo"], json!([1]));
}

#[test]
fn an_agent_builds_a_board_over_mcp() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("Agent Board");
    let mut mcp = Session::start(&["mcp"]);
    let init = mcp.request(json!({
        "jsonrpc": "2.0", "id": 0, "method": "initialize",
        "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "test", "version": "1.0"}},
    }));
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    assert!(
        init["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("connect_pins")
    );
    mcp.send(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
    let mut id = 0;
    let mut tool = |name: &str, arguments: Value| -> Value {
        id += 1;
        let reply = mcp.request(json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": {"name": name, "arguments": arguments},
        }));
        let result = &reply["result"];
        assert_eq!(result["isError"], false, "{name}: {reply}");
        result["structuredContent"].clone()
    };
    tool(
        "create_project",
        json!({"path": project, "name": "Agent Board", "width": 40, "height": 30}),
    );
    assert_eq!(
        tool(
            "place_part",
            json!({"symbol": "R", "value": "1k", "footprint": "0805"})
        )["designator"],
        "R1"
    );
    tool("place_part", json!({"symbol": "LED", "footprint": "0805"}));
    tool("place_part", json!({"symbol": "C", "footprint": "0603"}));
    tool(
        "connect_pins",
        json!({"net": "VCC", "pins": ["R1.1", "C1.1"], "style": "power"}),
    );
    tool(
        "connect_pins",
        json!({"net": "LED_A", "pins": ["R1.2", "D1.A"]}),
    );
    tool(
        "connect_pins",
        json!({"net": "GND", "pins": ["D1.K", "C1.2"], "style": "power"}),
    );
    assert_eq!(tool("run_erc", json!({}))["errors"], 0);
    tool("update_board", json!({}));
    tool("arrange_footprints", json!({}));
    assert_eq!(tool("route_board", json!({}))["unrouted"], 0);
    let drc = tool("run_drc", json!({}));
    assert_eq!(drc["errors"], 0, "{drc}");
    assert_eq!(drc["unrouted"], 0);
    tool("generate_outputs", json!({}));

    // A tool error is a result, not a protocol error.
    id += 1;
    let reply = mcp.request(json!({
        "jsonrpc": "2.0", "id": id, "method": "tools/call",
        "params": {"name": "connect_pins", "arguments": {"net": "X", "pins": ["Q1.1"]}},
    }));
    assert_eq!(reply["result"]["isError"], true);
    assert!(
        reply["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("no part Q1")
    );
    drop(mcp);

    // Saved after every change: another process sees the routed board.
    let info = json_of(&["board", "info", project.to_str().unwrap(), "--json"]);
    assert_eq!(info["unrouted"], 0);
    assert_eq!(info["footprints"].as_array().unwrap().len(), 3);
}
