//! Systems of several boards through the engine API: the synthetic
//! "Station" (carrier, shield, front panel), the connection check with its
//! mistakes, aliases, signals across boards, a board of another project,
//! cables, saving and opening again.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
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
    reply["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

fn codes(checked: &Value) -> Vec<String> {
    checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["excluded"] == false)
        .map(|f| {
            let variant = f["args"]["variant"].as_str().unwrap_or("");
            if variant.is_empty() {
                f["code"].as_str().unwrap().to_owned()
            } else {
                format!("{}/{variant}", f["code"].as_str().unwrap())
            }
        })
        .collect()
}

fn board_id(info: &Value, name: &str) -> String {
    info["boards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["name"] == name)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn checks_the_connections_of_a_system() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Station");
    ew_format::store::create_project(&root, &ew_synth::system::generate(1)).unwrap();
    let mut engine = Engine::without_journal();
    let snapshot = call(&mut engine, "project.open", json!({"path": root}));
    let doc = snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "system")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let validation = call(&mut engine, "project.validate", Value::Null);
    assert_eq!(validation["errors"], 0, "{validation}");

    let info = call(&mut engine, "system.info", json!({"doc": doc}));
    assert_eq!(info["boards"].as_array().unwrap().len(), 3);
    assert!(
        info["boards"]
            .as_array()
            .unwrap()
            .iter()
            .all(|b| b["loaded"] == true && b["local"] == true)
    );
    let carrier = board_id(&info, "Carrier");
    let shield = board_id(&info, "Shield");
    let panel = board_id(&info, "Front panel");

    // Connectors and pins.
    let connectors = call(
        &mut engine,
        "system.connectors",
        json!({"doc": doc, "board": carrier}),
    );
    let names: Vec<&str> = connectors["connectors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["designator"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["J1", "J2"]);
    let pins = call(
        &mut engine,
        "system.pins",
        json!({"doc": doc, "board": carrier, "designator": "J1"}),
    );
    assert_eq!(pins["pins"].as_array().unwrap().len(), 16);
    assert_eq!(pins["pins"][0]["net"], "+5V");
    assert_eq!(pins["pins"][0]["power"], true);
    assert_eq!(pins["pins"][4]["power"], false);

    // The check: SDA and SCL swapped, A0 meets an unconnected pin, BUTTON
    // meets BTN through the cable.
    let checked = call(&mut engine, "system.check", json!({"doc": doc}));
    let found = codes(&checked);
    assert!(
        found.contains(&"system-mismatch/swapped".to_owned()),
        "{found:?}"
    );
    assert!(found.contains(&"system-missing".to_owned()), "{found:?}");
    assert!(
        found.contains(&"system-mismatch/pin".to_owned()),
        "{found:?}"
    );
    assert_eq!(checked["errors"], 2, "{checked}");
    assert_eq!(checked["warnings"], 1, "{checked}");
    let link = &checked["connections"][0];
    assert_eq!(link["kind"], "link");
    assert_eq!(link["pairs"].as_array().unwrap().len(), 16);
    assert_eq!(link["pairs"][4]["status"], "swapped");
    assert_eq!(link["pairs"][10]["status"], "missing");
    assert_eq!(link["pairs"][0]["status"], "ok");
    let cable = &checked["connections"][1];
    assert_eq!(cable["kind"], "cable");
    assert_eq!(cable["pairs"][3]["status"], "mismatch");
    let swap = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["args"]["variant"] == "swapped")
        .unwrap();
    assert_eq!(
        swap["message"],
        "SDA and SCL are swapped: Carrier J1 pins 5 and 6 meet Shield P1 pins 6 and 5"
    );

    // An alias makes BUTTON and BTN the same signal.
    call(
        &mut engine,
        "system.setAliases",
        json!({"doc": doc, "aliases": [["BUTTON", "BTN"]]}),
    );
    let checked = call(&mut engine, "system.check", json!({"doc": doc}));
    assert_eq!(checked["errors"], 1, "{checked}");

    // Accepting the missing A0.
    let missing = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["code"] == "system-missing")
        .unwrap()["key"]
        .clone();
    call(
        &mut engine,
        "system.exclude",
        json!({"doc": doc, "keys": [missing], "excluded": true}),
    );
    let checked = call(&mut engine, "system.check", json!({"doc": doc}));
    assert_eq!(checked["warnings"], 0, "{checked}");

    // Where the carrier's SDA goes: the shield's SCL (the swap).
    let parts = call(
        &mut engine,
        "system.counterparts",
        json!({"doc": doc, "board": carrier, "net": "SDA"}),
    );
    let first = &parts["counterparts"][0];
    assert_eq!(first["boardName"], "Shield");
    assert_eq!(first["designator"], "P1");
    assert_eq!(first["net"], "SCL");

    // Renaming LED on the carrier renames it on the front panel too.
    let renamed = call(
        &mut engine,
        "system.renameSignal",
        json!({"doc": doc, "board": carrier, "net": "LED", "to": "STATUS"}),
    );
    assert_eq!(renamed["renamed"], 2, "{renamed}");
    assert_eq!(renamed["others"], json!([]));
    let pins = call(
        &mut engine,
        "system.pins",
        json!({"doc": doc, "board": panel, "designator": "J1"}),
    );
    assert_eq!(pins["pins"][2]["net"], "STATUS");
    call(&mut engine, "edit.undo", Value::Null);
    let pins = call(
        &mut engine,
        "system.pins",
        json!({"doc": doc, "board": panel, "designator": "J1"}),
    );
    assert_eq!(pins["pins"][2]["net"], "LED");
    assert!(
        error(
            &mut engine,
            "system.renameSignal",
            json!({"doc": doc, "board": carrier, "net": "LED", "to": "a/b"})
        )
        .contains("valid")
    );

    // Pairing by number instead of by position meets the wrong rows.
    let link_id = checked["connections"][0]["id"].clone();
    call(
        &mut engine,
        "system.updateLink",
        json!({"doc": doc, "id": link_id, "pairing": "number"}),
    );
    let checked = call(&mut engine, "system.check", json!({"doc": doc}));
    assert!(checked["errors"].as_u64().unwrap() > 1, "{checked}");
    call(&mut engine, "edit.undo", Value::Null);

    // A board of another project.
    let other = dir.path().join("Sensor");
    ew_format::store::create_project(&other, &ew_synth::system::generate(2)).unwrap();
    let added = call(
        &mut engine,
        "system.addBoard",
        json!({"doc": doc, "source": {"kind": "project", "path": other, "board": "Shield"}, "name": "Spare shield"}),
    );
    let info = call(&mut engine, "system.info", json!({"doc": doc}));
    let spare = info["boards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == added["id"])
        .unwrap()
        .clone();
    assert_eq!(spare["local"], false);
    assert_eq!(spare["loaded"], true);
    assert_eq!(spare["board"], "Shield");
    assert!(
        spare["source"]["path"]
            .as_str()
            .unwrap()
            .starts_with("../Sensor/")
    );
    // Linked to the carrier's socket like the first shield (the same
    // mistakes); renaming on it is left to its own project.
    call(
        &mut engine,
        "system.addLink",
        json!({"doc": doc, "a": {"board": carrier, "designator": "J1"}, "b": {"board": added["id"], "designator": "P1"}}),
    );
    let renamed = call(
        &mut engine,
        "system.renameSignal",
        json!({"doc": doc, "board": carrier, "net": "D2", "to": "PWM"}),
    );
    assert_eq!(renamed["renamed"].as_u64().unwrap(), 2, "{renamed}");
    assert_eq!(renamed["others"][0]["board"], "Spare shield");
    call(&mut engine, "edit.undo", Value::Null);
    assert!(
        error(
            &mut engine,
            "system.addLink",
            json!({"doc": doc, "a": {"board": carrier, "designator": "J1"}, "b": {"board": carrier, "designator": "J2"}})
        )
        .contains("two different boards")
    );

    // A reference board from its open design files.
    let blinky = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../ew-kicad/tests/fixtures/blinky/blinky.kicad_pro");
    let reference = call(
        &mut engine,
        "system.importReference",
        json!({"doc": doc, "source": blinky, "name": "Blinky"}),
    );
    assert_eq!(reference["path"], "systems/references/Blinky/Blinky.ewproj");
    assert!(
        root.join("systems/references/Blinky/Blinky.ewproj")
            .is_file()
    );
    let info = call(&mut engine, "system.info", json!({"doc": doc}));
    let blinky_board = info["boards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == reference["id"])
        .unwrap()
        .clone();
    assert_eq!(blinky_board["loaded"], true);
    assert_eq!(blinky_board["local"], false);
    let connectors = call(
        &mut engine,
        "system.connectors",
        json!({"doc": doc, "board": reference["id"]}),
    );
    assert_eq!(connectors["connectors"][0]["designator"], "J1");
    assert!(
        error(
            &mut engine,
            "system.importReference",
            json!({"doc": doc, "source": blinky, "name": "Blinky"})
        )
        .contains("already exists")
    );
    call(
        &mut engine,
        "system.removeBoard",
        json!({"doc": doc, "id": reference["id"]}),
    );

    // A cable from scratch, wired one to one.
    let cable = call(
        &mut engine,
        "system.addCable",
        json!({"doc": doc, "ends": [
            {"connector": {"board": carrier, "designator": "J2"}},
            {"connector": {"board": panel, "designator": "J1"}}
        ]}),
    );
    let wired = call(
        &mut engine,
        "system.wireCable",
        json!({"doc": doc, "cable": cable["id"], "from": cable["ends"][0], "to": cable["ends"][1], "color": "grey"}),
    );
    assert_eq!(wired["wires"], 4);
    let info = call(&mut engine, "system.info", json!({"doc": doc}));
    let added_cable = info["cables"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == cable["id"])
        .unwrap()
        .clone();
    assert_eq!(added_cable["name"], "W2");
    assert_eq!(added_cable["ends"][1]["label"], "X2");
    call(
        &mut engine,
        "system.removeCable",
        json!({"doc": doc, "id": cable["id"]}),
    );

    // Removing a board removes its links.
    call(
        &mut engine,
        "system.removeBoard",
        json!({"doc": doc, "id": added["id"]}),
    );
    let info = call(&mut engine, "system.info", json!({"doc": doc}));
    assert_eq!(info["links"].as_array().unwrap().len(), 1);

    // A board's place in the system.
    call(
        &mut engine,
        "system.updateBoard",
        json!({"doc": doc, "id": shield, "quantity": 2, "position": [0, 0, 12_000_000]}),
    );

    // Saved and opened again.
    call(&mut engine, "project.save", Value::Null);
    assert!(root.join("systems/station.ewsys").is_file());
    let mut later = Engine::without_journal();
    call(&mut later, "project.open", json!({"path": root}));
    let info = call(&mut later, "system.info", json!({"doc": doc}));
    let shield_entry = info["boards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["name"] == "Shield")
        .unwrap()
        .clone();
    assert_eq!(shield_entry["quantity"], 2);
    assert_eq!(shield_entry["position"], json!([0, 0, 12_000_000]));
    assert_eq!(info["aliases"], json!([["BUTTON", "BTN"]]));
}

#[test]
fn the_system_in_3d() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Station");
    ew_format::store::create_project(&root, &ew_synth::system::generate(1)).unwrap();
    let mut engine = Engine::without_journal();
    let snapshot = call(&mut engine, "project.open", json!({"path": root}));
    let doc = snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "system")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let info = call(&mut engine, "system.info", json!({"doc": doc}));
    let shield = board_id(&info, "Shield");

    // One scene: three boards' meshes and their six parts.
    let scene = call(&mut engine, "system.scene", json!({"doc": doc}));
    assert_eq!(scene["boards"].as_array().unwrap().len(), 3);
    assert_eq!(scene["parts"].as_array().unwrap().len(), 6);
    assert!(scene["data"].as_str().unwrap().len() > 1000);
    let bounds = scene["board"]["bounds"].as_array().unwrap();
    // From the front panel (y −40) to the shield's top (z above 10 mm).
    assert!(bounds[1].as_f64().unwrap() < -39.0);
    assert!(bounds[5].as_f64().unwrap() > 11.0);

    // No collisions as placed.
    let checked = call(&mut engine, "system.check", json!({"doc": doc}));
    assert!(
        !codes(&checked)
            .iter()
            .any(|c| c == "system-collision" || c == "system-clearance"),
        "{checked}"
    );

    // The shield 7 mm lower meets the carrier's controller.
    call(
        &mut engine,
        "system.updateBoard",
        json!({"doc": doc, "id": shield, "position": [12_540_000, 10_000_000, 3_100_000]}),
    );
    let checked = call(&mut engine, "system.check", json!({"doc": doc}));
    let found = codes(&checked);
    assert!(found.contains(&"system-collision".to_owned()), "{found:?}");
    let collision = checked["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["code"] == "system-collision")
        .unwrap()
        .clone();
    assert!(
        collision["message"]
            .as_str()
            .unwrap()
            .contains("Shield (board)"),
        "{collision}"
    );

    // Stacked onto the socket again: back where it was.
    let link = info["links"][0]["id"].clone();
    let stacked = call(
        &mut engine,
        "system.stack",
        json!({"doc": doc, "link": link}),
    );
    assert_eq!(stacked["gap"], 8.5);
    assert_eq!(
        stacked["position"],
        json!([12_540_000, 10_000_000, 10_080_000])
    );
    assert_eq!(stacked["rotation"], json!([0, 0, 0]));
    let checked = call(&mut engine, "system.check", json!({"doc": doc}));
    assert!(!codes(&checked).contains(&"system-collision".to_owned()));
    // A given gap.
    let stacked = call(
        &mut engine,
        "system.stack",
        json!({"doc": doc, "link": link, "gap": 11.0}),
    );
    assert_eq!(stacked["position"][2], 12_580_000);
    assert!(
        error(
            &mut engine,
            "system.stack",
            json!({"doc": doc, "link": link, "gap": -1.0})
        )
        .contains("gap")
    );

    // The assembly as one STEP file.
    let path = dir.path().join("station.step");
    let exported = call(
        &mut engine,
        "system.export",
        json!({"doc": doc, "path": path}),
    );
    assert!(exported["bytes"].as_u64().unwrap() > 1000);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("ElectronicWerk system assembly"));
    for name in ["'Carrier'", "'Shield'", "'Front panel'"] {
        assert!(text.contains(name), "{name}");
    }
}

#[test]
fn writes_the_system_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Station");
    ew_format::store::create_project(&root, &ew_synth::system::generate(1)).unwrap();
    let mut engine = Engine::without_journal();
    let snapshot = call(&mut engine, "project.open", json!({"path": root}));
    let doc = snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "system")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let info = call(&mut engine, "system.info", json!({"doc": doc}));

    // The bill of materials: three boards, the parts, the cable, two
    // housings and 600 mm of AWG 24 wire in four colours.
    let bom = call(&mut engine, "system.bom", json!({"doc": doc}));
    let lines = bom["lines"].as_array().unwrap();
    let of = |kind: &str| lines.iter().filter(|l| l["kind"] == kind).count();
    assert_eq!(of("board"), 3);
    assert_eq!(of("cable"), 1);
    assert_eq!(of("wire"), 4);
    let housing = lines.iter().find(|l| l["kind"] == "housing").unwrap();
    assert_eq!(housing["quantity"], 2);
    let mcu = lines.iter().find(|l| l["value"] == "MCU").unwrap();
    assert_eq!(mcu["designators"], "Carrier: U1");

    // The wiring list: signals from the boards.
    let cable = info["cables"][0]["id"].clone();
    let wiring = call(
        &mut engine,
        "system.wiring",
        json!({"doc": doc, "cable": cable}),
    );
    let rows = wiring["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0]["signal"], "+5V");
    assert_eq!(rows[0]["from"], "X1");
    assert_eq!(rows[0]["color"], "red");
    assert_eq!(rows[0]["length"], 150.0);

    // Every output in one folder.
    let written = call(&mut engine, "system.outputs", json!({"doc": doc}));
    let files: Vec<String> = written["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            std::path::Path::new(f.as_str().unwrap())
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(
        files,
        [
            "station-bom.csv",
            "station-connections.csv",
            "station-connections.html",
            "station-w1-wiring.csv",
            "station-w1-harness.pdf",
            "station-w1-harness.svg",
            "station.step"
        ]
    );
    let out = root.join("outputs/station");
    let pdf = std::fs::read(out.join("station-w1-harness.pdf")).unwrap();
    assert!(pdf.starts_with(b"%PDF-"));
    let report = std::fs::read_to_string(out.join("station-connections.html")).unwrap();
    assert!(report.contains("SDA and SCL are swapped"));
    let csv = std::fs::read_to_string(out.join("station-w1-wiring.csv")).unwrap();
    assert!(csv.starts_with("Wire,From,From pin,To,To pin,Signal"));
    // Only some.
    let some = call(
        &mut engine,
        "system.outputs",
        json!({"doc": doc, "dir": dir.path().join("only"), "kinds": ["bom"]}),
    );
    assert_eq!(some["files"].as_array().unwrap().len(), 1);
}
