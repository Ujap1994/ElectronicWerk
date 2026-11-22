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
        "SDA and SCL are swapped: Carrier J1 pins 5 and 6 meet Shield P1 pins 5 and 6"
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

    // Links: the shield turned over pairs the rows the other way.
    let link_id = checked["connections"][0]["id"].clone();
    call(
        &mut engine,
        "system.updateLink",
        json!({"doc": doc, "id": link_id, "pairing": "mirrored"}),
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
