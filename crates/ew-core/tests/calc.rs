//! The calculators (`calc.*`) against published reference values.

#![allow(
    clippy::unwrap_used,
    clippy::needless_pass_by_value,
    clippy::float_cmp,
    reason = "test code"
)]

use ew_core::Engine;
use serde_json::{Value, json};

fn call(method: &str, params: Value) -> Value {
    let mut engine = Engine::without_journal();
    let reply: Value = serde_json::from_str(
        &engine.dispatch(&json!({"method": method, "params": params}).to_string()),
    )
    .unwrap();
    assert!(reply.get("error").is_none(), "{method}: {reply}");
    reply["result"].clone()
}

fn fails(method: &str, params: Value) -> String {
    let mut engine = Engine::without_journal();
    let reply: Value = serde_json::from_str(
        &engine.dispatch(&json!({"method": method, "params": params}).to_string()),
    )
    .unwrap();
    reply["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

fn f(value: &Value) -> f64 {
    value.as_f64().unwrap()
}

fn close(actual: f64, expected: f64, relative: f64) {
    assert!(
        ((actual - expected) / expected).abs() <= relative,
        "{actual} is not within {relative} of {expected}"
    );
}

/// Square mils in square millimetres.
const MIL2: f64 = 0.0254 * 0.0254;

#[test]
fn e_series_values() {
    let e24 = call("calc.eSeries", json!({"value": 4600}));
    assert_eq!(
        (f(&e24["nearest"]), f(&e24["below"]), f(&e24["above"])),
        (4700.0, 4300.0, 4700.0)
    );
    assert_eq!(f(&e24["tolerance"]), 5.0);
    let e96 = call("calc.eSeries", json!({"value": 4600, "series": "E96"}));
    assert_eq!((f(&e96["below"]), f(&e96["above"])), (4530.0, 4640.0));
    assert_eq!(f(&e96["nearest"]), 4640.0);
    // A value of the series is its own neighbour.
    let exact = call("calc.eSeries", json!({"value": 0.0022, "series": "E12"}));
    assert_eq!(
        (f(&exact["below"]), f(&exact["above"]), f(&exact["error"])),
        (0.0022, 0.0022, 0.0)
    );
    // The one E192 value that is off the formula.
    let e192 = call("calc.eSeries", json!({"value": 9.2, "series": "E192"}));
    assert_eq!(f(&e192["nearest"]), 9.2);
    // Across a decade.
    let top = call("calc.eSeries", json!({"value": 9.6, "series": "E12"}));
    assert_eq!((f(&top["below"]), f(&top["above"])), (8.2, 10.0));
    assert!(fails("calc.eSeries", json!({"value": -1})).contains("positive"));
    assert!(fails("calc.eSeries", json!({"value": 1, "series": "E7"})).contains("E7"));
}

#[test]
fn divider_pairs() {
    let reply = call(
        "calc.divider",
        json!({"vin": 12, "vout": 3.3, "series": "E24"}),
    );
    let pairs = reply["pairs"].as_array().unwrap();
    assert!(!pairs.is_empty() && pairs.len() <= 8);
    let mut last = 0.0;
    for pair in pairs {
        let (r1, r2) = (f(&pair["r1"]), f(&pair["r2"]));
        close(f(&pair["vout"]), 12.0 * r2 / (r1 + r2), 1e-12);
        assert!((1e3..=1e6).contains(&(r1 + r2)), "{pair}");
        let error = f(&pair["error"]).abs();
        assert!(error >= last - 1e-9, "sorted by error");
        last = error;
    }
    assert!(f(&pairs[0]["error"]).abs() < 0.5, "{}", pairs[0]);
    assert!(fails("calc.divider", json!({"vin": 3, "vout": 5})).contains("below"));
}

#[test]
fn regulator_resistors() {
    // LM317: 1.25 V reference, 50 µA out of the adjust pin.
    let reply = call(
        "calc.regulator",
        json!({"vref": 1.25, "vout": 5, "iadj": 50e-6, "series": "E96", "minLower": 100, "maxLower": 1000}),
    );
    let best = &reply["pairs"][0];
    let (upper, lower) = (f(&best["upper"]), f(&best["lower"]));
    close(
        f(&best["vout"]),
        1.25 * (1.0 + upper / lower) + 50e-6 * upper,
        1e-12,
    );
    assert!(f(&best["error"]).abs() < 0.2, "{best}");
    assert!((100.0..=1000.0).contains(&lower));
}

#[test]
fn track_current_ipc_2152_and_2221() {
    // The IPC-2152 fit: 10 A at 20 °C rise needs 513.1 mil² (IPC: 500).
    let wide = call("calc.track", json!({"current": 10, "rise": 20}));
    close(f(&wide["area"]), 513.1 * MIL2, 0.001);
    close(f(&wide["width"]), 513.1 * MIL2 / 0.035 * 1e6, 0.001);
    assert_eq!(wide["notes"], json!([]));
    // And back: that width carries 10 A.
    let back = call(
        "calc.track",
        json!({"width": f(&wide["width"]), "rise": 20}),
    );
    close(f(&back["current"]), 10.0, 1e-9);

    // IPC-2221: 100 mil² at 10 °C rise: 3.73 A outside, half inside.
    let outside = call(
        "calc.track",
        json!({"width": 2_540_000, "thickness": 25_400, "standard": "ipc2221"}),
    );
    close(f(&outside["current"]), 3.73, 0.005);
    let inside = call(
        "calc.track",
        json!({"width": 2_540_000, "thickness": 25_400, "standard": "ipc2221", "placement": "internal"}),
    );
    close(f(&inside["current"]) / f(&outside["current"]), 0.5, 1e-12);

    // Resistance of 100 mm of 1 mm × 35 µm at 35 °C: 49.3 mΩ … 51.7 mΩ.
    let r = call(
        "calc.track",
        json!({"width": 1_000_000, "length": 100_000_000}),
    );
    close(
        f(&r["resistance"]),
        1.724e-8 * 0.1 / 35e-9 * (1.0 + 0.00393 * 15.0),
        1e-9,
    );
    close(
        f(&r["voltageDrop"]),
        f(&r["current"]) * f(&r["resistance"]),
        1e-12,
    );

    // Outside the fitted range.
    let hot = call("calc.track", json!({"current": 40, "rise": 120}));
    assert_eq!(hot["notes"], json!(["riseOutsideFit", "currentOutsideFit"]));
    assert!(fails("calc.track", json!({"current": 1, "width": 1})).contains("either"));
}

#[test]
fn via_current() {
    // 0.3 mm hole, 25 µm plating: a ring of π · 0.325 · 0.025 mm².
    let via = call("calc.via", json!({"drill": 300_000}));
    let ring = std::f64::consts::PI * 0.325 * 0.025;
    close(f(&via["area"]), ring, 1e-9);
    // The same as a track of that cross-section.
    let track = call("calc.track", json!({"width": ring / 0.035 * 1e6}));
    close(f(&via["current"]), f(&track["current"]), 1e-9);
    assert!((1.5..2.0).contains(&f(&via["current"])), "{via}");
}

#[test]
fn fusing_current_onderdonk() {
    // 10 mil × 1 oz (1.378 mil), 1 s, 25 °C: 2.57 A.
    let track = call("calc.fusing", json!({"width": 254_000}));
    close(f(&track["current"]), 2.567, 0.002);
    // Four times the time: half the current.
    let long = call("calc.fusing", json!({"width": 254_000, "time": 4}));
    close(f(&long["current"]) / f(&track["current"]), 0.5, 1e-9);
    let via = call("calc.fusing", json!({"drill": 300_000}));
    assert!(f(&via["current"]) > 0.0);
    let slow = call("calc.fusing", json!({"width": 254_000, "time": 60}));
    assert_eq!(slow["notes"], json!(["longPulse"]));
}
