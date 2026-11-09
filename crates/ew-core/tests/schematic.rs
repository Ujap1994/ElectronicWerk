//! The schematic editor methods through the engine API, on the imported
//! KiCad demo project: ERC, editing with undo, net highlighting,
//! annotation, find and replace and exports.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    reason = "test helpers may panic and take owned JSON for brevity"
)]

use std::path::{Path, PathBuf};

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

fn call_err(engine: &mut Engine, method: &str, params: Value) -> String {
    let reply: Value = serde_json::from_str(
        &engine.dispatch(&json!({"method": method, "params": params}).to_string()),
    )
    .unwrap();
    reply["error"]["code"].as_str().unwrap_or("none").to_owned()
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../ew-kicad/tests/fixtures/blinky")
        .join(name)
}

/// Opens the demo project; returns the engine and the top sheet.
fn open(dir: &Path) -> (Engine, String) {
    let mut engine = Engine::without_journal();
    let result = call(
        &mut engine,
        "import.kicad",
        json!({"source": fixture("blinky.kicad_pro"), "path": dir.join("Blinky"), "name": "Blinky"}),
    );
    let root = result["project"]["rootSchematic"]
        .as_str()
        .unwrap()
        .to_owned();
    (engine, root)
}

fn sheet(engine: &mut Engine, doc: &str) -> Value {
    call(engine, "document.get", json!({"doc": doc}))
}

fn count(engine: &mut Engine, doc: &str, collection: &str) -> usize {
    sheet(engine, doc)[collection]
        .as_array()
        .map_or(0, Vec::len)
}

#[test]
fn erc_and_exports() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, _) = open(dir.path());
    let report = call(&mut engine, "schematic.erc", Value::Null);
    assert_eq!(report["errors"], 2);
    assert_eq!(report["issues"][0]["code"], "power-not-driven");
    assert_eq!(report["issues"][0]["path"], "/");
    let netlist = call(&mut engine, "schematic.netlist", json!({"format": "kicad"}));
    assert!(
        netlist["text"]
            .as_str()
            .unwrap()
            .starts_with("(export (version \"E\")")
    );
    let csv = call(&mut engine, "schematic.netlist", json!({"format": "csv"}));
    assert!(
        csv["text"]
            .as_str()
            .unwrap()
            .starts_with("Net,Designator,Pin")
    );
    let bom = call(&mut engine, "schematic.bom", Value::Null);
    assert!(bom["text"].as_str().unwrap().contains("R1"));
}

#[test]
#[allow(clippy::too_many_lines, reason = "one editing session")]
fn editing_with_undo() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, doc) = open(dir.path());
    let symbols = count(&mut engine, &doc, "symbols");
    let name = sheet(&mut engine, &doc)["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["designator"] == "R1")
        .unwrap()["symbol"]
        .as_str()
        .unwrap()
        .to_owned();

    // Place a resistor: it gets the next free number.
    let placed = call(
        &mut engine,
        "schematic.place",
        json!({"doc": doc, "symbol": name, "at": [25_400_000, 25_400_000], "annotate": true}),
    );
    let r = placed["select"][0].as_str().unwrap().to_owned();
    assert_eq!(placed["history"]["undoLabel"], "place-symbol");
    let placed_symbol = sheet(&mut engine, &doc)["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == r.as_str())
        .unwrap()
        .clone();
    assert_eq!(placed_symbol["designator"], "R4");
    assert_eq!(count(&mut engine, &doc, "symbols"), symbols + 1);
    assert_eq!(
        call_err(
            &mut engine,
            "schematic.place",
            json!({"doc": doc, "symbol": "Nope", "at": [0, 0]})
        ),
        "notFound"
    );

    // A wire from pin 1 (3.81 mm above the anchor) to nowhere, then its net.
    let wire = call(
        &mut engine,
        "schematic.wire",
        json!({"doc": doc, "points": [[25_400_000, 29_210_000], [25_400_000, 45_720_000], [35_560_000, 45_720_000]]}),
    );
    assert_eq!(wire["changes"], 2);
    let first = wire["select"][0].as_str().unwrap().to_owned();
    let net = call(
        &mut engine,
        "schematic.net",
        json!({"doc": doc, "id": first}),
    );
    assert_eq!(net["net"], "Net-(R4-Pad1)", "{net}");
    let members = &net["sheets"][&doc];
    assert_eq!(members["items"].as_array().unwrap().len(), 2);
    assert_eq!(members["pins"], json!([format!("{r}/pin/0")]));

    // Drag the resistor: the attached wire end follows.
    let moved = call(
        &mut engine,
        "schematic.move",
        json!({"doc": doc, "ids": [r], "delta": [2_540_000, 0], "drag": true}),
    );
    assert_eq!(moved["history"]["undoLabel"], "drag");
    assert!(moved["changes"].as_u64().unwrap() >= 2);
    call(
        &mut engine,
        "schematic.orient",
        json!({"doc": doc, "ids": [r], "orientation": "left"}),
    );
    let turned = sheet(&mut engine, &doc)["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == r.as_str())
        .unwrap()["rotation"]
        .clone();
    assert_eq!(turned, 90.0);

    // Labels, copy and paste, delete, undo.
    call(
        &mut engine,
        "schematic.add",
        json!({"doc": doc, "item": {"kind": "label", "text": "TEST"}, "at": [35_560_000, 45_720_000]}),
    );
    let copied = call(
        &mut engine,
        "schematic.copy",
        json!({"doc": doc, "ids": [r]}),
    );
    let pasted = call(
        &mut engine,
        "schematic.paste",
        json!({"doc": doc, "objects": copied["objects"], "offset": [10_160_000, 0]}),
    );
    assert_eq!(pasted["history"]["undoLabel"], "paste");
    assert_eq!(count(&mut engine, &doc, "symbols"), symbols + 2);
    let hits = call(
        &mut engine,
        "schematic.find",
        json!({"query": "TEST", "matchCase": true}),
    );
    assert_eq!(hits.as_array().unwrap().len(), 1);
    call(
        &mut engine,
        "schematic.replace",
        json!({"query": "TEST", "matchCase": true, "with": "PROBE"}),
    );
    assert_eq!(
        call(&mut engine, "schematic.find", json!({"query": "PROBE"}))
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let deleted = call(
        &mut engine,
        "schematic.delete",
        json!({"doc": doc, "ids": [r]}),
    );
    assert_eq!(deleted["history"]["undoLabel"], "delete");
    call(&mut engine, "edit.undo", Value::Null);
    assert_eq!(count(&mut engine, &doc, "symbols"), symbols + 2);

    // Annotation: nothing left unannotated, so nothing to do.
    let annotated = call(&mut engine, "schematic.annotate", json!({}));
    assert_eq!(annotated["changes"], 0);
    assert_eq!(annotated["seq"], Value::Null);
    let renumbered = call(&mut engine, "schematic.annotate", json!({"reset": true}));
    assert!(renumbered["changes"].as_u64().unwrap() > 0);
    assert_eq!(renumbered["history"]["undoLabel"], "annotate");
}
