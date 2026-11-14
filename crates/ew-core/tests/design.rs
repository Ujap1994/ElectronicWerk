//! The design methods (`design.*`): a small circuit from the first symbol to
//! a routed board without any ids or nanometres, as scripts and AI agents
//! use it.

#![allow(
    clippy::unwrap_used,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "test code"
)]

use ew_core::Engine;
use serde_json::{Value, json};

fn call(engine: &mut Engine, method: &str, params: Value) -> Value {
    let reply: Value = serde_json::from_str(
        &engine.dispatch(&json!({"method": method, "params": params}).to_string()),
    )
    .unwrap();
    assert!(reply.get("error").is_none(), "{method}: {reply}");
    reply["result"].clone()
}

fn error(engine: &mut Engine, method: &str, params: Value) -> String {
    let reply: Value = serde_json::from_str(
        &engine.dispatch(&json!({"method": method, "params": params}).to_string()),
    )
    .unwrap();
    assert!(
        reply.get("error").is_some(),
        "{method} should fail: {reply}"
    );
    reply["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

fn new_project(engine: &mut Engine, dir: &std::path::Path) {
    call(
        engine,
        "project.create",
        json!({
            "path": dir.join("Blinker"),
            "name": "Blinker",
            "template": "board-2l",
            "board": {"width": 40_000_000, "height": 30_000_000, "layers": 2, "thickness": 1_600_000},
        }),
    );
}

fn net_of<'a>(components: &'a Value, designator: &str, pin: &str) -> &'a str {
    components["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["designator"] == designator)
        .unwrap()["pins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["number"] == pin)
        .unwrap()["net"]
        .as_str()
        .unwrap_or("")
}

#[test]
fn builds_a_circuit_from_schematic_to_routed_board() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::without_journal();
    new_project(&mut engine, dir.path());

    // Parts: starter symbols with footprints from the wizard presets.
    let r1 = call(
        &mut engine,
        "design.place",
        json!({"symbol": "R", "value": "330", "footprint": "0805", "at": [50.8, 120.0]}),
    );
    assert_eq!(r1["designator"], "R1");
    assert_eq!(r1["value"], "330");
    assert_eq!(r1["footprint"], "Chip_0805_2013Metric");
    // Snapped to the sheet grid (1.27 mm from the top-left corner).
    assert_eq!(r1["at"], json!([50.8, 119.83]));
    let d1 = call(
        &mut engine,
        "design.place",
        json!({"symbol": "LED", "footprint": "0805", "rotation": 90}),
    );
    assert_eq!(d1["designator"], "D1");
    assert_eq!(
        d1["pins"],
        json!([{"number": "1", "name": "K"}, {"number": "2", "name": "A"}])
    );
    let c1 = call(
        &mut engine,
        "design.place",
        json!({"symbol": "C", "designator": "C7", "footprint": "0603"}),
    );
    assert_eq!(c1["designator"], "C7");
    let presets = call(&mut engine, "design.presets", Value::Null);
    assert!(presets["footprints"].as_array().unwrap().len() > 20);

    // Mistakes are explained.
    assert!(
        error(
            &mut engine,
            "design.place",
            json!({"symbol": "R", "designator": "R1"})
        )
        .contains("taken")
    );
    assert!(
        error(&mut engine, "design.place", json!({"symbol": "NoSuchPart"}))
            .contains("base library")
    );
    assert!(
        error(
            &mut engine,
            "design.connect",
            json!({"net": "X", "pins": ["R1.9"]})
        )
        .contains("its pins are 1, 2")
    );
    assert!(
        error(
            &mut engine,
            "design.connect",
            json!({"net": "X", "pins": ["R1"]})
        )
        .contains("designator.pin")
    );
    assert!(
        error(
            &mut engine,
            "design.connect",
            json!({"net": "X", "pins": ["Q5.1"]})
        )
        .contains("no part Q5")
    );

    // Nets: by pin number or pin name.
    let vcc = call(
        &mut engine,
        "design.connect",
        json!({"net": "VCC", "pins": ["R1.1", "C7.1"], "style": "power"}),
    );
    assert_eq!(vcc["connected"], json!(["R1.1", "C7.1"]));
    assert_eq!(vcc["style"], "power");
    call(
        &mut engine,
        "design.connect",
        json!({"net": "LED_A", "pins": ["R1.2", "D1.A"]}),
    );
    call(
        &mut engine,
        "design.connect",
        json!({"net": "GND", "pins": ["D1.K", "C7.2"], "style": "power"}),
    );
    // Again: nothing to do; another net: refused.
    let again = call(
        &mut engine,
        "design.connect",
        json!({"net": "GND", "pins": ["D1.1"]}),
    );
    assert_eq!(again["already"], json!(["D1.1"]));
    assert!(again.get("seq").is_none());
    assert!(
        error(
            &mut engine,
            "design.connect",
            json!({"net": "OTHER", "pins": ["D1.1"]})
        )
        .contains("on net GND")
    );

    let components = call(&mut engine, "design.components", Value::Null);
    assert_eq!(net_of(&components, "R1", "1"), "VCC");
    // Local labels name nets per sheet (`/` is the top sheet).
    assert_eq!(net_of(&components, "R1", "2"), "/LED_A");
    assert_eq!(net_of(&components, "D1", "2"), "/LED_A");
    assert_eq!(net_of(&components, "D1", "1"), "GND");
    assert_eq!(net_of(&components, "C7", "2"), "GND");
    let nets = call(&mut engine, "design.nets", Value::Null);
    let led = nets["nets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == "/LED_A")
        .unwrap();
    assert_eq!(led["pins"], json!(["D1.2", "R1.2"]));
    let erc = call(&mut engine, "schematic.erc", Value::Null);
    let errors = erc["issues"].as_array().map_or(0, |list| {
        list.iter().filter(|i| i["severity"] == "error").count()
    });
    assert_eq!(errors, 0, "{erc}");

    // Values and properties.
    call(
        &mut engine,
        "design.set",
        json!({"designator": "R1", "value": "470", "properties": {"MPN": "RC0805FR-07470RL"}}),
    );
    let components = call(&mut engine, "design.components", Value::Null);
    let r1 = components["components"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["designator"] == "R1")
        .unwrap();
    assert_eq!(r1["value"], "470");

    // The board.
    let update = call(&mut engine, "design.updateBoard", Value::Null);
    assert_eq!(
        update["changes"].as_array().unwrap().len(),
        3 + 3,
        "{update}"
    );
    let board = call(&mut engine, "design.board", Value::Null);
    assert_eq!(board["footprints"].as_array().unwrap().len(), 3);
    assert!(
        board["footprints"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["onBoard"] == false)
    );
    assert_eq!(
        board["outline"],
        json!({"min": [0.0, 0.0], "max": [40.0, 30.0]})
    );
    let arranged = call(&mut engine, "design.arrange", Value::Null);
    assert_eq!(arranged["placed"], json!(["C7", "D1", "R1"]));
    let board = call(&mut engine, "design.board", Value::Null);
    assert!(
        board["footprints"]
            .as_array()
            .unwrap()
            .iter()
            .all(|f| f["onBoard"] == true),
        "{board}"
    );
    assert_eq!(board["unrouted"], 3);

    // One part exactly.
    let moved = call(
        &mut engine,
        "design.placeFootprint",
        json!({"designator": "R1", "at": [30, 15], "rotation": 90}),
    );
    assert_eq!(moved["at"], json!([30.0, 15.0]));
    assert_eq!(moved["rotation"], 90.0);

    // Routing with 45° bends; one undo step.
    let routed = call(&mut engine, "design.route", Value::Null);
    assert_eq!(routed["unrouted"], 0, "{routed}");
    assert_eq!(routed["routed"], 3);
    let drc = call(&mut engine, "pcb.drc", json!({"doc": board["id"]}));
    let drc_errors: Vec<&Value> = drc["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["severity"] == "error")
        .collect();
    assert!(drc_errors.is_empty(), "{drc_errors:?}");
    assert_eq!(routed["history"]["undoLabel"], "auto-route");
    call(&mut engine, "edit.undo", Value::Null);
    let board = call(&mut engine, "design.board", Value::Null);
    assert_eq!(board["unrouted"], 3);
    assert_eq!(board["tracks"], 0);
}

#[test]
fn flips_footprints_and_reports_unknown_boards() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::without_journal();
    new_project(&mut engine, dir.path());
    call(
        &mut engine,
        "design.place",
        json!({"symbol": "R", "footprint": "1206"}),
    );
    call(&mut engine, "design.updateBoard", Value::Null);
    let flipped = call(
        &mut engine,
        "design.placeFootprint",
        json!({"designator": "R1", "at": [10, 10], "side": "bottom", "rotation": 180}),
    );
    assert_eq!(flipped["side"], "bottom");
    assert_eq!(flipped["at"], json!([10.0, 10.0]));
    assert_eq!(flipped["rotation"], 180.0);
    assert!(
        error(
            &mut engine,
            "design.placeFootprint",
            json!({"designator": "R9", "at": [0, 0]})
        )
        .contains("no footprint R9")
    );
    assert!(error(&mut engine, "design.board", json!({"board": "Nope"})).contains("no board"));
    assert!(error(&mut engine, "design.route", json!({"nets": ["NOPE"]})).contains("no net NOPE"));
    assert!(
        error(
            &mut engine,
            "design.footprint",
            json!({"name": "Unknown_Thing"})
        )
        .contains("not a wizard preset")
    );
    let header = call(
        &mut engine,
        "design.footprint",
        json!({"name": "pin header 1x04 2.54 mm"}),
    );
    assert_eq!(header["footprint"], "PinHeader_1x04_P2.54mm_Vertical");
    // Without a project.
    let mut empty = Engine::without_journal();
    assert!(error(&mut empty, "design.components", Value::Null).contains("project"));
}

#[test]
fn never_overwrites_files_another_program_changed() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = Engine::without_journal();
    new_project(&mut app, dir.path());
    // An agent (another engine) edits and saves the same project.
    let mut agent = Engine::without_journal();
    call(
        &mut agent,
        "project.open",
        json!({"path": dir.path().join("Blinker")}),
    );
    call(&mut agent, "design.place", json!({"symbol": "R"}));
    call(&mut agent, "project.save", Value::Null);
    // The app's own change to the same sheet must not overwrite it.
    call(&mut app, "design.place", json!({"symbol": "C"}));
    let message = error(&mut app, "project.save", Value::Null);
    assert!(
        message.contains("changed by another program: schematics/"),
        "{message}"
    );
    // Reopened, the agent's part is there and saving works again.
    call(&mut app, "project.close", json!({"discard": true}));
    call(
        &mut app,
        "project.open",
        json!({"path": dir.path().join("Blinker")}),
    );
    let parts = call(&mut app, "design.components", Value::Null);
    assert_eq!(parts["components"][0]["designator"], "R1");
    call(&mut app, "design.place", json!({"symbol": "C"}));
    call(&mut app, "project.save", Value::Null);
    // Unrelated files may change: only the files a save writes count.
    call(&mut agent, "project.close", json!({"discard": true}));
}
