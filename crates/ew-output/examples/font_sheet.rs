//! Writes an SVG with every glyph of the stroke font (for reviewing the
//! design): `cargo run -p ew-output --example font_sheet > font.svg`.

#![allow(
    clippy::print_stdout,
    clippy::cast_precision_loss,
    reason = "example output"
)]

use std::fmt::Write as _;

use ew_output::font::{self, TextSpec};

fn main() {
    let rows = [
        ":;<=>?@[\\]^_`",
        "{|}~ °±²³µ·×÷",
        "ÄÖÜäöüß ΩéèÉ€",
        "Il1 O0 4k7 ±5%",
        "#$%&'()*+,-./",
    ];
    let size = 10.0;
    let mut svg = String::new();
    let height = rows.len() as f64 * 18.0 + 10.0;
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 {height}" width="720" height="{}"><rect width="100%" height="100%" fill="white"/>"#,
        height * 6.0
    );
    for (i, row) in rows.iter().enumerate() {
        let y = 15.0 + i as f64 * 18.0;
        let spec = TextSpec {
            text: row,
            at: [5.0, 0.0],
            size,
            rotation: 0.0,
            h: -1,
            v: -1,
            mirror: false,
            italic: false,
        };
        for stroke in font::strokes(&spec) {
            let points: Vec<String> = stroke
                .iter()
                .map(|p| format!("{:.3},{:.3}", p[0], y - p[1]))
                .collect();
            let _ = write!(
                svg,
                r#"<polyline points="{}" fill="none" stroke="black" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                points.join(" "),
                font::default_thickness(size, false)
            );
        }
    }
    svg.push_str("</svg>");
    println!("{svg}");
}
