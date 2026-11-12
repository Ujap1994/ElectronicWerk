//! The board methods through the engine API: design rules check (full and
//! by region), accepted findings, zone filling with undo, the design rules
//! and the rule syntax check.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "test helpers may panic and take owned JSON for brevity; board coordinates fit f64"
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

/// A new project from a template; returns the engine and the board.
fn project(dir: &std::path::Path, template: &str) -> (Engine, String) {
    let mut engine = Engine::without_journal();
    let snapshot = call(
        &mut engine,
        "project.create",
        json!({"path": dir.join("Board"), "name": "Board", "template": template}),
    );
    let board = snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "pcb")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    (engine, board)
}

const MM: i64 = 1_000_000;

fn codes(report: &Value) -> Vec<String> {
    report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["code"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn drc_findings_regions_and_exclusions() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, board) = project(dir.path(), "demo");
    let report = call(&mut engine, "pcb.drc", json!({"doc": board}));
    assert!(report["counts"]["errors"].is_u64());
    let before = report["findings"].as_array().unwrap().len();

    // A via of no net in the middle of nowhere dangles.
    let via = json!({
        "type": "via", "id": "00000000-0000-4000-8000-0000000000aa",
        "at": [-200 * MM, -200 * MM], "diameter": 600_000, "drill": 300_000
    });
    call(
        &mut engine,
        "edit.apply",
        json!({"changes": [{"op": "insert", "doc": board, "object": via}]}),
    );
    let region = [-201 * MM, -201 * MM, -199 * MM, -199 * MM];
    let online = call(
        &mut engine,
        "pcb.drc",
        json!({"doc": board, "region": region}),
    );
    let found: Vec<&Value> = online["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["targets"][0]["id"] == "00000000-0000-4000-8000-0000000000aa")
        .collect();
    let found_codes: Vec<&str> = found.iter().map(|f| f["code"].as_str().unwrap()).collect();
    assert!(found_codes.contains(&"dangling-via"), "{found_codes:?}");
    assert!(found_codes.contains(&"outside-board"), "{found_codes:?}");
    assert_eq!(
        online["findings"].as_array().unwrap().len(),
        before + found.len()
    );

    // Accept the dangling via.
    let key = found.iter().find(|f| f["code"] == "dangling-via").unwrap()["key"].clone();
    call(
        &mut engine,
        "pcb.exclude",
        json!({"doc": board, "keys": [key]}),
    );
    let full = call(&mut engine, "pcb.drc", json!({"doc": board}));
    let accepted = full["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["key"] == key)
        .unwrap();
    assert_eq!(accepted["excluded"], true);
    assert_eq!(full["counts"]["excluded"], 1);
    call(&mut engine, "edit.undo", Value::Null);
    let again = call(&mut engine, "pcb.drc", json!({"doc": board}));
    assert_eq!(again["counts"]["excluded"], 0);
}

#[test]
fn zones_fill_and_unfill_with_undo() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, board) = project(dir.path(), "board-2l");
    let zone = json!({
        "type": "zone", "id": "00000000-0000-4000-8000-0000000000bb",
        "name": "GND", "layers": ["cu.bottom"],
        "outline": [[-100 * MM, -100 * MM], [100 * MM, -100 * MM], [100 * MM, 100 * MM], [-100 * MM, 100 * MM]]
    });
    call(
        &mut engine,
        "edit.apply",
        json!({"changes": [{"op": "insert", "doc": board, "object": zone}]}),
    );
    let report = call(&mut engine, "pcb.drc", json!({"doc": board}));
    assert!(codes(&report).contains(&"zone-unfilled".to_owned()));
    let filled = call(&mut engine, "pcb.fillZones", json!({"doc": board}));
    assert_eq!(filled["zones"], 1);
    assert!(filled["areas"].as_u64().unwrap() >= 1);
    let object = call(&mut engine, "document.get", json!({"doc": board}));
    let zones = object["zones"].as_array().unwrap();
    let filled_zone = zones.iter().find(|z| z["name"] == "GND").unwrap();
    // A zone without a net keeps its islands and fills the board inside
    // its edge clearance.
    assert_ne!(filled_zone["filled"].as_array().unwrap().len(), 0);
    let report = call(&mut engine, "pcb.drc", json!({"doc": board}));
    assert!(!codes(&report).contains(&"zone-unfilled".to_owned()));
    // Filling again changes nothing.
    let again = call(&mut engine, "pcb.fillZones", json!({"doc": board}));
    assert_eq!(again["zones"], 0);
    call(&mut engine, "pcb.unfillZones", json!({"doc": board}));
    call(&mut engine, "edit.undo", Value::Null);
    let object = call(&mut engine, "document.get", json!({"doc": board}));
    let zone = &object["zones"].as_array().unwrap()[0];
    assert_ne!(
        zone["filled"].as_array().unwrap().len(),
        0,
        "undo restores the fill"
    );
}

#[test]
fn design_rules_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let (mut engine, board) = project(dir.path(), "demo");
    let info = call(&mut engine, "pcb.rules", json!({"doc": board}));
    assert!(info["doc"].is_string());
    assert!(
        info["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["code"] == "clearance" && c["severity"] == "error")
    );
    let default_nets = info["nets"]["Default"].as_array().unwrap().len();
    assert!(default_nets > 0);
    let all_nets: usize = info["nets"]
        .as_object()
        .unwrap()
        .values()
        .map(|nets| nets.as_array().unwrap().len())
        .sum();

    let ok = call(
        &mut engine,
        "pcb.validateRule",
        json!({"condition": "A.netclass == 'Power' && B.type == 'via'"}),
    );
    assert_eq!(ok["ok"], true);
    let bad = call(
        &mut engine,
        "pcb.validateRule",
        json!({"condition": "A.colour == 1"}),
    );
    assert_eq!(bad["ok"], false);
    assert_eq!(bad["position"], 2);

    let mut props = info["rules"].clone();
    props["custom"] = json!([{
        "name": "wide",
        "condition": "A.type == 'track'",
        "constraints": {"trackWidthMin": 5_000_000}
    }]);
    props["drc"] = json!({"severities": {"dangling-via": "ignore"}});
    props["assignments"] = json!([{"pattern": "*", "class": "Power"}]);
    call(&mut engine, "pcb.setRules", json!({"props": props}));
    let info = call(&mut engine, "pcb.rules", json!({"doc": board}));
    assert_eq!(info["rules"]["custom"][0]["name"], "wide");
    assert_eq!(info["nets"]["Default"].as_array().unwrap().len(), 0);
    assert_eq!(info["nets"]["Power"].as_array().unwrap().len(), all_nets);
    let report = call(&mut engine, "pcb.drc", json!({"doc": board}));
    let widths = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["code"] == "track-width")
        .count();
    assert!(widths > 0, "every track is narrower than 5 mm");
    assert!(
        report["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["args"]["source"] == "rule wide")
    );
}

fn doc_of(snapshot: &Value, kind: &str) -> String {
    snapshot["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == kind)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn kinds(preview: &Value) -> Vec<String> {
    preview["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["kind"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one round trip between schematic and board"
)]
fn schematic_and_board_stay_in_sync() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::without_journal();
    let snapshot = call(
        &mut engine,
        "project.create",
        json!({"path": dir.path().join("Sync"), "name": "Sync", "template": "board-2l"}),
    );
    let (lib, board) = (doc_of(&snapshot, "library"), doc_of(&snapshot, "pcb"));
    let root = snapshot["rootSchematic"].as_str().unwrap().to_owned();

    // A footprint and a resistor symbol that uses it.
    let presets = call(&mut engine, "libedit.presets", Value::Null);
    let soic = presets
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "SOIC-8")
        .unwrap()["wizard"]
        .clone();
    call(
        &mut engine,
        "libedit.create",
        json!({"doc": lib, "kind": "footprint", "name": "SOIC-8", "wizard": soic}),
    );
    call(
        &mut engine,
        "libedit.setProps",
        json!({"doc": lib, "kind": "symbol", "name": "R", "props": {"footprint": "SOIC-8"}}),
    );
    // R1 and R2 side by side, pin 1 to pin 1 and pin 2 to pin 2.
    let grid = 1_270_000;
    for x in [0, 8 * grid] {
        call(
            &mut engine,
            "schematic.place",
            json!({"doc": root, "symbol": "R", "at": [x, 40 * grid], "annotate": true}),
        );
    }
    for y in [37 * grid, 43 * grid] {
        call(
            &mut engine,
            "schematic.wire",
            json!({"doc": root, "points": [[0, y], [8 * grid, y]]}),
        );
    }

    // Forward annotation: two nets, two footprints.
    let preview = call(&mut engine, "pcb.syncPreview", json!({"doc": board}));
    let k = kinds(&preview);
    assert_eq!(k.iter().filter(|k| *k == "addNet").count(), 2, "{preview}");
    assert_eq!(
        k.iter().filter(|k| *k == "addFootprint").count(),
        2,
        "{preview}"
    );
    let report = call(&mut engine, "pcb.drc", json!({"doc": board}));
    assert!(codes(&report).contains(&"missing-footprint".to_owned()));
    let applied = call(&mut engine, "pcb.syncApply", json!({"doc": board}));
    assert_eq!(applied["select"].as_array().unwrap().len(), 2);
    assert_eq!(applied["history"]["undoLabel"], "update-board");
    let again = call(&mut engine, "pcb.syncPreview", json!({"doc": board}));
    assert_eq!(kinds(&again), Vec::<String>::new(), "{again}");
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    assert_eq!(pcb["nets"].as_array().unwrap().len(), 2);
    let footprints = pcb["footprints"].as_array().unwrap();
    assert_eq!(footprints.len(), 2);
    for fp in footprints {
        let pads = fp["pads"].as_array().unwrap();
        assert!(pads[0]["net"].is_string() && pads[1]["net"].is_string());
        assert!(pads[2].get("net").is_none(), "pad 3 has no pin");
    }
    // The two pad 1s share a net: one airwire per net.
    let ratsnest = call(&mut engine, "pcb.ratsnest", json!({"doc": board}));
    assert_eq!(ratsnest["unrouted"], 2);
    let report = call(&mut engine, "pcb.drc", json!({"doc": board}));
    assert!(!codes(&report).contains(&"missing-footprint".to_owned()));

    // Cross-probing both ways.
    let sheet = call(&mut engine, "document.get", json!({"doc": root}));
    let r1 = sheet["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["designator"] == "R1")
        .unwrap()["id"]
        .clone();
    let probe = call(
        &mut engine,
        "pcb.crossProbe",
        json!({"doc": board, "symbols": [r1]}),
    );
    let fp_r1 = probe["footprints"][0].clone();
    let fp = footprints.iter().find(|f| f["id"] == fp_r1).unwrap();
    assert_eq!(fp["designator"], "R1");
    let back = call(
        &mut engine,
        "pcb.crossProbe",
        json!({"doc": board, "footprints": [fp_r1]}),
    );
    assert_eq!(back["symbols"][0]["id"], r1);

    // Renaming on the board goes back to the schematic.
    let mut renamed = fp.clone();
    renamed["designator"] = json!("R10");
    renamed["value"] = json!("4k7");
    renamed["type"] = json!("footprint");
    call(
        &mut engine,
        "edit.apply",
        json!({"changes": [{"op": "update", "doc": board, "object": renamed}]}),
    );
    let back = call(&mut engine, "pcb.backPreview", json!({"doc": board}));
    assert_eq!(kinds(&back), vec!["setDesignator", "setValue"]);
    call(&mut engine, "pcb.backApply", json!({"doc": board}));
    let sheet = call(&mut engine, "document.get", json!({"doc": root}));
    assert!(
        sheet["symbols"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["designator"] == "R10" && s["value"] == "4k7")
    );
    // The automatic net names follow the lowest designator, so the nets
    // are renamed (tracks keep them).
    let preview = call(&mut engine, "pcb.syncPreview", json!({"doc": board}));
    assert_eq!(kinds(&preview), vec!["renameNet", "renameNet"], "{preview}");
    call(&mut engine, "pcb.syncApply", json!({"doc": board}));
    let preview = call(&mut engine, "pcb.syncPreview", json!({"doc": board}));
    assert_eq!(kinds(&preview), Vec::<String>::new(), "{preview}");

    // Deleting R2 in the schematic removes its footprint and nothing else.
    let r2 = sheet["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["designator"] == "R2")
        .unwrap()["id"]
        .clone();
    call(
        &mut engine,
        "schematic.delete",
        json!({"doc": root, "ids": [r2]}),
    );
    let preview = call(&mut engine, "pcb.syncPreview", json!({"doc": board}));
    let k = kinds(&preview);
    assert!(k.contains(&"removeFootprint".to_owned()), "{preview}");
    // Both nets now have a single pin: they disappear, and R10's pads lose
    // them.
    assert_eq!(
        k.iter().filter(|k| *k == "removeNet").count(),
        2,
        "{preview}"
    );
    let report = call(&mut engine, "pcb.drc", json!({"doc": board}));
    assert!(codes(&report).contains(&"extra-footprint".to_owned()));
    call(&mut engine, "pcb.syncApply", json!({"doc": board}));
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    assert_eq!(pcb["footprints"].as_array().unwrap().len(), 1);
    assert!(pcb.get("nets").is_none() || pcb["nets"].as_array().unwrap().is_empty());
}

#[test]
#[allow(clippy::too_many_lines, reason = "one layout session")]
fn placing_editing_routing_presets_and_dxf() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::without_journal();
    let snapshot = call(
        &mut engine,
        "project.create",
        json!({"path": dir.path().join("Layout"), "name": "Layout", "template": "board-2l"}),
    );
    let (lib, board) = (doc_of(&snapshot, "library"), doc_of(&snapshot, "pcb"));
    let presets = call(&mut engine, "libedit.presets", Value::Null);
    let soic = presets
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "SOIC-8")
        .unwrap()["wizard"]
        .clone();
    call(
        &mut engine,
        "libedit.create",
        json!({"doc": lib, "kind": "footprint", "name": "SOIC-8", "wizard": soic}),
    );
    // Two footprints, placed and turned.
    let placed = call(
        &mut engine,
        "pcb.place",
        json!({"doc": board, "footprint": "SOIC-8", "at": [20 * MM, 20 * MM]}),
    );
    assert_eq!(placed["designator"], "X1");
    assert_eq!(placed["history"]["undoLabel"], "place-footprint");
    let u1 = placed["select"][0].clone();
    let u2 = call(
        &mut engine,
        "pcb.place",
        json!({"doc": board, "footprint": "SOIC-8", "at": [40 * MM, 20 * MM], "designator": "U2"}),
    )["select"][0]
        .clone();
    let turned = call(
        &mut engine,
        "pcb.edit",
        json!({"doc": board, "op": "rotate", "ids": [u2], "angle": 90}),
    );
    assert_eq!(turned["history"]["undoLabel"], "rotate");
    call(
        &mut engine,
        "pcb.edit",
        json!({"doc": board, "op": "align", "ids": [u1, u2], "mode": "top"}),
    );
    call(&mut engine, "edit.undo", Value::Null);
    call(
        &mut engine,
        "pcb.edit",
        json!({"doc": board, "op": "move", "ids": [u2], "offset": [0, 5 * MM]}),
    );
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    let fp2 = pcb["footprints"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == u2)
        .unwrap();
    assert_eq!(fp2["at"], json!([40 * MM, 25 * MM]));
    assert_eq!(fp2["rotation"], 90);

    // A net on pad 1 of both, then a route from one to the other.
    let net = "00000000-0000-4000-8000-0000000000cc";
    let mut changes = vec![json!({"op": "insert", "doc": board,
        "object": {"type": "net", "id": net, "name": "SIG"}})];
    for fp in pcb["footprints"].as_array().unwrap() {
        let mut fp = fp.clone();
        fp["type"] = json!("footprint");
        fp["pads"][0]["net"] = json!(net);
        changes.push(json!({"op": "update", "doc": board, "object": fp}));
    }
    call(&mut engine, "edit.apply", json!({"changes": changes}));
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    let pad_center = |fp: &Value| -> [f64; 2] {
        // Pad 1 of the SOIC: its centre on the board (no rotation for X1).
        let at = &fp["at"];
        let pad = &fp["pads"][0]["at"];
        [
            (at[0].as_i64().unwrap() + pad[0].as_i64().unwrap()) as f64,
            (at[1].as_i64().unwrap() + pad[1].as_i64().unwrap()) as f64,
        ]
    };
    let fp1 = pcb["footprints"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["id"] == u1)
        .unwrap();
    let start = call(
        &mut engine,
        "pcb.routeStart",
        json!({"doc": board, "at": pad_center(fp1), "layer": "cu.top"}),
    );
    assert_eq!(start["net"], net);
    assert_eq!(start["width"], 250_000.0);
    let route = call(
        &mut engine,
        "pcb.route",
        json!({"doc": board, "layer": "cu.top", "net": net, "width": 250_000,
            "start": start["at"], "target": [30.0e6, 40.0e6], "via": 600_000}),
    );
    assert_eq!(route["reached"], true);
    assert_eq!(route["viaFits"], true);
    let points: Vec<Value> = route["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            json!([
                p[0].as_f64().unwrap().round() as i64,
                p[1].as_f64().unwrap().round() as i64
            ])
        })
        .collect();
    let end = points.last().unwrap().clone();
    let added = call(
        &mut engine,
        "pcb.addTracks",
        json!({"doc": board, "net": net, "width": 250_000,
            "segments": [{"layer": "cu.top", "points": points}],
            "vias": [{"at": end, "diameter": 600_000, "drill": 300_000}]}),
    );
    assert_eq!(added["history"]["undoLabel"], "route");
    assert!(added["select"].as_array().unwrap().len() >= 2);

    // Presets change the stack-up and the rules.
    let presets = call(&mut engine, "pcb.presets", Value::Null);
    assert!(
        presets
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["id"] == "standard-4l")
    );
    call(
        &mut engine,
        "pcb.applyPreset",
        json!({"doc": board, "preset": "standard-4l"}),
    );
    let props = call(&mut engine, "document.properties", json!({"doc": board}));
    let copper = props["stackup"]["layers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|l| l["type"] == "copper")
        .count();
    assert_eq!(copper, 4);
    let info = call(&mut engine, "pcb.rules", json!({"doc": board}));
    assert_eq!(info["rules"]["constraints"]["minClearance"], 127_000);

    // A DXF outline replaces the drawn one.
    let path = dir.path().join("outline.dxf");
    std::fs::write(
        &path,
        "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
0\nLWPOLYLINE\n8\nEDGE\n90\n4\n70\n1\n10\n0\n20\n0\n10\n60\n20\n0\n10\n60\n20\n40\n10\n0\n20\n40\n\
0\nCIRCLE\n8\nHOLES\n10\n5\n20\n5\n40\n1.6\n0\nENDSEC\n0\nEOF\n",
    )
    .unwrap();
    let preview = call(&mut engine, "pcb.dxfPreview", json!({"path": path}));
    assert_eq!(preview["count"], 2);
    assert_eq!(preview["layers"], json!(["EDGE", "HOLES"]));
    let imported = call(
        &mut engine,
        "pcb.importDxf",
        json!({"doc": board, "path": path, "layer": "outline", "layers": ["EDGE"],
            "replace": true, "origin": [0, 0]}),
    );
    assert_eq!(imported["shapes"], 1);
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    let outlines: Vec<&Value> = pcb["shapes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s["layer"] == "outline")
        .collect();
    assert_eq!(outlines.len(), 1);
    // The lower left corner lies at the origin: y runs from -40 mm to 0.
    let points = outlines[0]["geometry"]["polygon"]["points"]
        .as_array()
        .unwrap();
    assert!(points.contains(&json!([0, 0])));
    assert!(points.contains(&json!([60 * MM, -40 * MM])));
}
