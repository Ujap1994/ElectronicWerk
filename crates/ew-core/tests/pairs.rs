//! Differential pairs through the engine API: `pcb.routeStart` with `pair`
//! finds the other net and where it begins, `pcb.routePair` routes both
//! tracks together around obstacles, `pcb.addTracks` puts both in one step
//! that keeps every clearance, and `pcb.pairs` lists the pair with its
//! lengths.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    reason = "test helpers may panic and take owned JSON for brevity"
)]

use std::path::PathBuf;

use ew_core::Engine;
use serde_json::{Value, json};

const MM: i64 = 1_000_000;

fn call(engine: &mut Engine, method: &str, params: Value) -> Value {
    let reply: Value = serde_json::from_str(
        &engine.dispatch(&json!({"method": method, "params": params}).to_string()),
    )
    .unwrap();
    assert!(reply.get("error").is_none(), "{method}: {reply}");
    reply["result"].clone()
}

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

#[allow(clippy::cast_possible_truncation, reason = "board coordinates")]
fn mm(x: f64, y: f64) -> Value {
    json!([(x * 1e6).round() as i64, (y * 1e6).round() as i64])
}

#[test]
#[allow(clippy::too_many_lines, reason = "one scenario from start to DRC")]
fn a_pair_is_routed_together_and_keeps_its_gap() {
    let dir = tempfile::tempdir().unwrap();
    let mut engine = Engine::without_journal();
    let result = call(
        &mut engine,
        "import.kicad",
        json!({
            "source": fixture("../ew-eagle/tests/fixtures/blinky/blinky.sch"),
            "path": dir.path().join("blinky"),
        }),
    );
    let board = result["project"]["documents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["kind"] == "pcb")
        .unwrap()["id"]
        .clone();

    // Two nets of a USB pair, each with a short track in an empty corner.
    let (dp, dn) = (
        "11111111-1111-4111-8111-111111111111",
        "22222222-2222-4222-8222-222222222222",
    );
    call(
        &mut engine,
        "edit.apply",
        json!({"label": "nets", "changes": [
            {"op": "insert", "doc": board, "object": {"type": "net", "id": dp, "name": "USB_DP"}},
            {"op": "insert", "doc": board, "object": {"type": "net", "id": dn, "name": "USB_DN"}},
        ]}),
    );
    // Short tracks of both nets at the start (x 46–48 mm) and at the end
    // (x 60–62 mm), 0.5 mm apart.
    for (net, y) in [(dp, 4.0), (dn, 4.5)] {
        for x in [46.0, 60.0] {
            call(
                &mut engine,
                "pcb.addTracks",
                json!({
                    "doc": board, "net": net, "width": 200_000,
                    "segments": [{"layer": "cu.top", "points": [mm(x, y), mm(x + 2.0, y)]}],
                }),
            );
        }
    }
    // A track of another net in the way.
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    let other = pcb["nets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["name"] == "N$1")
        .unwrap()["id"]
        .clone();
    call(
        &mut engine,
        "pcb.addTracks",
        json!({
            "doc": board, "net": other, "width": 250_000,
            "segments": [{"layer": "cu.top", "points": [mm(53.0, 3.5), mm(53.0, 5.0)]}],
        }),
    );

    // The partner and where it begins.
    let start = call(
        &mut engine,
        "pcb.routeStart",
        json!({"doc": board, "at": mm(48.0, 4.0), "layer": "cu.top", "pair": true}),
    );
    assert_eq!(start["netName"], "USB_DP");
    assert_eq!(start["partner"]["netName"], "USB_DN");
    assert_eq!(start["partner"]["at"], json!([48.0e6, 4.5e6]));
    // Without `pair` no partner is looked for.
    let single = call(
        &mut engine,
        "pcb.routeStart",
        json!({"doc": board, "at": mm(48.0, 4.0), "layer": "cu.top"}),
    );
    assert!(single.get("partner").is_none());

    // Both tracks routed together, around the other track.
    let request = json!({
        "doc": board, "layer": "cu.top", "net": dp, "partner": dn,
        "width": 200_000.0, "gap": 300_000.0,
        "start": mm(48.0, 4.0), "partnerStart": mm(48.0, 4.5),
        "target": mm(58.0, 4.25),
    });
    let route = call(&mut engine, "pcb.routePair", request.clone());
    assert_eq!(route["reached"], true, "{route}");
    assert_eq!(route["landed"], false);
    // Their ends lie half a pitch to both sides of the target.
    let end = |key: &str| route[key].as_array().unwrap().last().unwrap().clone();
    let (a, b) = (end("points"), end("partner"));
    let pitch = (a[0].as_f64().unwrap() - b[0].as_f64().unwrap())
        .hypot(a[1].as_f64().unwrap() - b[1].as_f64().unwrap());
    assert!((pitch - 500_000.0).abs() < 2.0, "{pitch}");

    // Aimed at the pad (here the track end) of one net at the far end, the
    // pair lands on both.
    let mut aimed = request;
    aimed["target"] = mm(60.0, 4.0);
    let route = call(&mut engine, "pcb.routePair", aimed);
    assert_eq!(route["landed"], true, "{route}");
    let points = |key: &str| -> Vec<[f64; 2]> {
        route[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()])
            .collect()
    };
    let (mine, theirs) = (points("points"), points("partner"));
    assert_eq!(mine[0], [48e6, 4e6]);
    assert_eq!(theirs[0], [48e6, 4.5e6]);
    assert!(mine.len() > 2, "the pair goes around: {mine:?}");
    assert_eq!(mine.last().unwrap(), &[60e6, 4e6]);
    assert_eq!(theirs.last().unwrap(), &[60e6, 4.5e6]);

    // Both go into the board in one step.
    #[allow(clippy::cast_possible_truncation, reason = "board coordinates")]
    let round = |track: &[[f64; 2]]| -> Value {
        json!(
            track
                .iter()
                .map(|p| [p[0].round() as i64, p[1].round() as i64])
                .collect::<Vec<_>>()
        )
    };
    call(
        &mut engine,
        "pcb.addTracks",
        json!({
            "doc": board, "net": dp, "width": 200_000,
            "segments": [{"layer": "cu.top", "points": round(&mine)}],
            "pair": {"net": dn, "segments": [{"layer": "cu.top", "points": round(&theirs)}]},
        }),
    );
    let report = call(&mut engine, "pcb.drc", json!({"doc": board}));
    let near: Vec<&Value> = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| ["clearance", "short", "pair-gap"].contains(&f["code"].as_str().unwrap_or("")))
        .filter(|f| f["at"][0].as_i64().is_some_and(|x| x > 45 * MM))
        .collect();
    assert_eq!(near.len(), 0, "{near:#?}");

    // The pair with the lengths of both nets.
    let pairs = call(&mut engine, "pcb.pairs", json!({"doc": board}));
    let pair = pairs["pairs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "USB_D")
        .unwrap();
    assert_eq!(pair["positive"]["name"], "USB_DP");
    assert_eq!(pair["negative"]["name"], "USB_DN");
    assert!(pair["positive"]["length"].as_f64().unwrap() > 16e6);

    // One undo removes both tracks.
    call(&mut engine, "edit.undo", Value::Null);
    let pcb = call(&mut engine, "document.get", json!({"doc": board}));
    let count = |net: &str| {
        pcb["tracks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|t| t["net"] == net)
            .count()
    };
    assert_eq!((count(dp), count(dn)), (2, 2));
}
