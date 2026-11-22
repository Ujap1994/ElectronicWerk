//! Outputs of a system (M18): the bill of materials of all boards and
//! cables, the connection report, a wiring list per cable and its harness
//! drawing (PDF).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ew_model::id::Id;
use ew_model::system::{Cable, ConnectorRef, System};
use ew_output::canvas::{Color, Doc, Page, Paint, Path};
use ew_output::font::TextSpec;
use ew_pcb::geom::V;
use serde::Serialize;

use crate::system::{Checked, Loaded, connector_of, short_name};

/// One line of the system's bill of materials.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BomLine {
    /// `part`, `board`, `cable`, `housing` or `wire`.
    pub kind: &'static str,
    /// How many.
    pub quantity: u32,
    /// Value (or the board's, cable's, housing's name).
    pub value: String,
    /// Footprint (parts).
    pub footprint: String,
    /// Designators (`Carrier: R1, R2`).
    pub designators: String,
    /// Length in mm (wires).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<f64>,
}

/// The bill of materials: every board's parts times the board's quantity,
/// the boards themselves, and per cable its type, housings and wires.
#[must_use]
pub fn bom(system: &System, loaded: &[Loaded]) -> Vec<BomLine> {
    let mut out = Vec::new();
    let mut parts: BTreeMap<(String, String), (u32, Vec<String>)> = BTreeMap::new();
    for l in loaded {
        let Some(pcb) = &l.pcb else { continue };
        let q = l.entry.quantity.max(1);
        out.push(BomLine {
            kind: "board",
            quantity: q,
            value: format!("Board {}", l.entry.name),
            footprint: String::new(),
            designators: String::new(),
            length: None,
        });
        for fp in pcb
            .footprints
            .iter()
            .filter(|f| !f.attributes.dnp && !f.attributes.exclude_from_bom)
        {
            let entry = parts
                .entry((fp.value.clone(), fp.footprint.clone()))
                .or_insert((0, Vec::new()));
            entry.0 += q;
            entry.1.push(format!("{}: {}", l.entry.name, fp.designator));
        }
    }
    for ((value, footprint), (quantity, mut designators)) in parts {
        designators.sort_by(|a, b| ew_board3d::place::natural(a, b));
        out.push(BomLine {
            kind: "part",
            quantity,
            value,
            footprint,
            designators: designators.join(", "),
            length: None,
        });
    }
    let mut housings: BTreeMap<String, u32> = BTreeMap::new();
    let mut wires: BTreeMap<(String, String), f64> = BTreeMap::new();
    for cable in &system.cables {
        out.push(BomLine {
            kind: "cable",
            quantity: 1,
            value: if cable.part.is_empty() {
                cable.name.clone()
            } else {
                format!("{} ({})", cable.name, cable.part)
            },
            footprint: String::new(),
            designators: cable.name.clone(),
            #[allow(clippy::cast_precision_loss, reason = "lengths")]
            length: (cable.length > 0).then(|| cable.length as f64 / 1e6),
        });
        for end in cable.ends.iter().filter(|e| !e.housing.is_empty()) {
            *housings.entry(end.housing.clone()).or_default() += 1;
        }
        for wire in &cable.wires {
            #[allow(clippy::cast_precision_loss, reason = "lengths")]
            let length = wire.length.unwrap_or(cable.length) as f64 / 1e6;
            *wires
                .entry((wire.gauge.clone(), wire.color.clone()))
                .or_default() += length;
        }
    }
    for (housing, quantity) in housings {
        out.push(BomLine {
            kind: "housing",
            quantity,
            value: housing,
            footprint: String::new(),
            designators: String::new(),
            length: None,
        });
    }
    for ((gauge, color), length) in wires {
        let value = [gauge.as_str(), color.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
        out.push(BomLine {
            kind: "wire",
            quantity: 1,
            value: if value.is_empty() {
                "Wire".to_owned()
            } else {
                format!("Wire {value}")
            },
            footprint: String::new(),
            designators: String::new(),
            length: Some((length * 10.0).round() / 10.0),
        });
    }
    out
}

/// A field for CSV: quoted when needed.
fn csv(field: &str) -> String {
    if field.contains([',', '"', '\n']) {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_owned()
    }
}

/// The bill of materials as CSV.
#[must_use]
pub fn bom_csv(lines: &[BomLine]) -> String {
    let mut out = String::from("Kind,Quantity,Value,Footprint,Designators,Length (mm)\n");
    for l in lines {
        let _ = writeln!(
            out,
            "{},{},{},{},{},{}",
            l.kind,
            l.quantity,
            csv(&l.value),
            csv(&l.footprint),
            csv(&l.designators),
            l.length.map(|v| format!("{v}")).unwrap_or_default()
        );
    }
    out
}

/// The connection report as CSV: every pin pair of every connection.
#[must_use]
pub fn connections_csv(checked: &Checked) -> String {
    let mut out = String::from("Connection,Kind,Pin A,Signal A,Pin B,Signal B,Status\n");
    for c in &checked.connections {
        for p in &c.pairs {
            let _ = writeln!(
                out,
                "{},{},{},{},{},{},{}",
                csv(&c.name),
                c.kind,
                csv(p.a.as_deref().unwrap_or("")),
                csv(p.a_net.as_deref().map_or("", short_name)),
                csv(p.b.as_deref().unwrap_or("")),
                csv(p.b_net.as_deref().map_or("", short_name)),
                p.status
            );
        }
    }
    out
}

fn html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The connection report as a self-contained HTML page.
#[must_use]
pub fn connections_html(system: &System, checked: &Checked, version: &str) -> String {
    let mut out = String::new();
    let _ = write!(
        out,
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><title>{} – connections</title>\n<style>body{{font:14px system-ui,sans-serif;margin:24px;color:#1b1f24}}table{{border-collapse:collapse;margin:8px 0 24px}}td,th{{border:1px solid #d9dde3;padding:3px 8px;text-align:left}}th{{background:#eef0f3}}.ok{{color:#1e9e5a}}.error{{color:#d23c3c}}.warning{{color:#c98a00}}tr.bad td{{background:#fdf0f0}}</style></head><body>\n",
        html(&system.name)
    );
    let _ = writeln!(
        out,
        "<h1>{} – connections</h1><p>{} errors, {} warnings. Made by ElectronicWerk {}.</p>",
        html(&system.name),
        checked.errors,
        checked.warnings,
        html(version)
    );
    if !checked.findings.is_empty() {
        out.push_str("<h2>Findings</h2><ul>");
        for f in checked.findings.iter().filter(|f| !f.excluded) {
            let _ = write!(
                out,
                "<li class=\"{}\">{} <code>{}</code></li>",
                f.severity,
                html(&f.message),
                f.code
            );
        }
        out.push_str("</ul>\n");
    }
    for c in &checked.connections {
        let _ = write!(
            out,
            "<h2>{}</h2><table><tr><th>Pin A</th><th>Signal A</th><th>Pin B</th><th>Signal B</th><th>Status</th></tr>",
            html(&c.name)
        );
        for p in &c.pairs {
            let bad = !matches!(p.status.as_str(), "ok" | "unused");
            let _ = write!(
                out,
                "<tr{}><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                if bad { " class=\"bad\"" } else { "" },
                html(p.a.as_deref().unwrap_or("")),
                html(p.a_net.as_deref().map_or("", short_name)),
                html(p.b.as_deref().unwrap_or("")),
                html(p.b_net.as_deref().map_or("", short_name)),
                p.status
            );
        }
        out.push_str("</table>\n");
    }
    out.push_str("</body></html>\n");
    out
}

/// One wire of a cable with the signals on both ends.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireRow {
    /// Wire number (1-based).
    pub number: usize,
    /// The wire's id.
    pub id: Id,
    /// From: end label.
    pub from: String,
    /// From: pin.
    pub from_pin: String,
    /// To: end label.
    pub to: String,
    /// To: pin.
    pub to_pin: String,
    /// The signal (the wire's own, else the boards').
    pub signal: String,
    /// Colour.
    pub color: String,
    /// Gauge.
    pub gauge: String,
    /// Length (mm).
    pub length: f64,
}

/// The signal on a pin of a connector.
fn pin_signal(loaded: &[Loaded], r: Option<&ConnectorRef>, pin: &str) -> Option<String> {
    let r = r?;
    let l = loaded.iter().find(|l| l.entry.id == r.board)?;
    let pcb = l.pcb.as_ref()?;
    let fp = pcb
        .footprints
        .iter()
        .find(|f| f.designator == r.designator)?;
    let c = connector_of(pcb, fp);
    c.pins
        .into_iter()
        .find(|p| p.number == pin)
        .and_then(|p| p.net)
        .map(|n| short_name(&n).to_owned())
}

/// The wiring list of a cable.
#[must_use]
pub fn wiring(loaded: &[Loaded], cable: &Cable) -> Vec<WireRow> {
    let end = |id: Id| cable.ends.iter().find(|e| e.id == id);
    cable
        .wires
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let (from, to) = (end(w.from.end), end(w.to.end));
            let signal = if w.signal.is_empty() {
                pin_signal(loaded, from.and_then(|e| e.connector.as_ref()), &w.from.pin)
                    .or_else(|| {
                        pin_signal(loaded, to.and_then(|e| e.connector.as_ref()), &w.to.pin)
                    })
                    .unwrap_or_default()
            } else {
                w.signal.clone()
            };
            #[allow(clippy::cast_precision_loss, reason = "lengths")]
            let length = w.length.unwrap_or(cable.length) as f64 / 1e6;
            WireRow {
                number: i + 1,
                id: w.id,
                from: from.map_or_else(String::new, |e| e.label.clone()),
                from_pin: w.from.pin.clone(),
                to: to.map_or_else(String::new, |e| e.label.clone()),
                to_pin: w.to.pin.clone(),
                signal,
                color: w.color.clone(),
                gauge: w.gauge.clone(),
                length,
            }
        })
        .collect()
}

/// The wiring list as CSV.
#[must_use]
pub fn wiring_csv(rows: &[WireRow]) -> String {
    let mut out = String::from("Wire,From,From pin,To,To pin,Signal,Colour,Gauge,Length (mm)\n");
    for r in rows {
        let _ = writeln!(
            out,
            "{},{},{},{},{},{},{},{},{}",
            r.number,
            csv(&r.from),
            csv(&r.from_pin),
            csv(&r.to),
            csv(&r.to_pin),
            csv(&r.signal),
            csv(&r.color),
            csv(&r.gauge),
            r.length
        );
    }
    out
}

/// The colour of a wire on the drawing (names, IEC 60757 codes or
/// `#rrggbb`); black when unknown.
#[must_use]
pub fn wire_color(name: &str) -> Color {
    let lower = name.trim().to_ascii_lowercase();
    if let Some(c) = Color::hex(&lower) {
        return c;
    }
    match lower.as_str() {
        "red" | "rd" | "rot" => Color::rgb(210, 40, 40),
        "black" | "bk" | "schwarz" => Color::rgb(30, 30, 30),
        "yellow" | "ye" | "gelb" => Color::rgb(220, 170, 0),
        "green" | "gn" | "grün" | "gruen" => Color::rgb(40, 150, 60),
        "blue" | "bu" | "blau" => Color::rgb(40, 90, 200),
        "white" | "wh" | "weiß" | "weiss" => Color::rgb(170, 170, 170),
        "brown" | "bn" | "braun" => Color::rgb(130, 80, 40),
        "orange" | "og" => Color::rgb(240, 130, 20),
        "grey" | "gray" | "gy" | "grau" => Color::rgb(120, 120, 120),
        "violet" | "purple" | "vt" | "violett" => Color::rgb(130, 60, 170),
        "pink" | "pk" | "rosa" => Color::rgb(230, 110, 160),
        _ => Color::BLACK,
    }
}

const MM: f64 = 1e6;

fn rect(x: f64, y: f64, w: f64, h: f64) -> Path {
    Path::polyline(
        &[
            V::new(x, y),
            V::new(x + w, y),
            V::new(x + w, y + h),
            V::new(x, y + h),
        ],
        true,
    )
}

fn label(page: &mut Page, text: &str, at: [f64; 2], size: f64, h: i8, color: Color) {
    page.text(
        &TextSpec {
            text,
            at,
            size: size * MM,
            rotation: 0.0,
            h,
            v: 0,
            mirror: false,
            italic: false,
        },
        color,
        size * MM / 7.0,
    );
}

/// The harness drawing of a cable as a PDF file.
#[must_use]
pub fn harness_pdf(
    system: &System,
    loaded: &[Loaded],
    cable: &Cable,
    version: &str,
    created: &str,
) -> Vec<u8> {
    let doc = Doc {
        title: format!("{} – {}", system.name, cable.name),
        version: version.to_owned(),
        created: created.to_owned(),
        pages: vec![harness_page(system, loaded, cable, version, created)],
    };
    ew_output::pdf::write(&doc)
}

/// The harness drawing of a cable as an SVG file.
#[must_use]
pub fn harness_svg(
    system: &System,
    loaded: &[Loaded],
    cable: &Cable,
    version: &str,
    created: &str,
) -> String {
    let page = harness_page(system, loaded, cable, version, created);
    ew_output::svg::write(&page, &format!("{} – {}", system.name, cable.name))
}

/// The harness drawing of a cable (A4 landscape): its ends as pin tables,
/// the wires between them in their colours, and the wiring list.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one drawing")]
pub fn harness_page(
    system: &System,
    loaded: &[Loaded],
    cable: &Cable,
    version: &str,
    created: &str,
) -> Page {
    let (width, height) = (297.0 * MM, 210.0 * MM);
    let mut page = Page::new(cable.name.clone(), width, height);
    let ink = Color::rgb(27, 31, 36);
    let muted = Color::rgb(91, 100, 112);
    // Frame and title.
    page.add(
        rect(10.0 * MM, 10.0 * MM, 277.0 * MM, 190.0 * MM),
        Paint::stroke(ink, 0.35 * MM),
    );
    let title = if cable.part.is_empty() {
        format!("Cable {}", cable.name)
    } else {
        format!("Cable {} – {}", cable.name, cable.part)
    };
    label(&mut page, &title, [18.0 * MM, 190.0 * MM], 5.0, -1, ink);
    #[allow(clippy::cast_precision_loss, reason = "lengths")]
    let length = cable.length as f64 / 1e6;
    label(
        &mut page,
        &format!(
            "System {} · length {length} mm · {} wires",
            system.name,
            cable.wires.len()
        ),
        [18.0 * MM, 182.0 * MM],
        3.0,
        -1,
        muted,
    );
    // The ends: columns with their pins.
    let rows = wiring(loaded, cable);
    let pins_of = |end: Id| -> Vec<String> {
        let mut pins: Vec<String> = Vec::new();
        for w in &cable.wires {
            if w.from.end == end && !pins.contains(&w.from.pin) {
                pins.push(w.from.pin.clone());
            }
            if w.to.end == end && !pins.contains(&w.to.pin) {
                pins.push(w.to.pin.clone());
            }
        }
        pins.sort_by(|a, b| ew_board3d::place::natural(a, b));
        pins
    };
    let row_h = 6.0 * MM;
    let top = 165.0 * MM;
    let box_w = 55.0 * MM;
    let count = cable.ends.len().max(1);
    #[allow(clippy::cast_precision_loss, reason = "a few ends")]
    let gap = if count > 1 {
        (257.0 * MM - box_w * count as f64) / (count as f64 - 1.0)
    } else {
        0.0
    };
    let mut pin_at: BTreeMap<(Id, String), (f64, f64, f64)> = BTreeMap::new(); // left x, right x, y
    for (i, end) in cable.ends.iter().enumerate() {
        #[allow(clippy::cast_precision_loss, reason = "a few ends")]
        let x = 20.0 * MM + i as f64 * (box_w + gap);
        let where_ = end.connector.as_ref().map_or_else(String::new, |r| {
            let board = loaded
                .iter()
                .find(|l| l.entry.id == r.board)
                .map_or_else(String::new, |l| l.entry.name.clone());
            format!("{board} {}", r.designator)
        });
        label(&mut page, &end.label, [x, top + 9.0 * MM], 4.0, -1, ink);
        if !where_.is_empty() {
            label(&mut page, &where_, [x, top + 4.0 * MM], 2.5, -1, muted);
        }
        let pins = pins_of(end.id);
        #[allow(clippy::cast_precision_loss, reason = "pin counts")]
        let h = row_h * pins.len().max(1) as f64;
        page.add(rect(x, top - h, box_w, h), Paint::stroke(ink, 0.3 * MM));
        for (j, pin) in pins.iter().enumerate() {
            #[allow(clippy::cast_precision_loss, reason = "pin counts")]
            let y = top - row_h * (j as f64 + 0.5);
            if j > 0 {
                #[allow(clippy::cast_precision_loss, reason = "pin counts")]
                let line_y = top - row_h * j as f64;
                page.add(
                    Path::polyline(&[V::new(x, line_y), V::new(x + box_w, line_y)], false),
                    Paint::stroke(muted, 0.15 * MM),
                );
            }
            let signal = pin_signal(loaded, end.connector.as_ref(), pin).unwrap_or_default();
            label(&mut page, pin, [x + 3.0 * MM, y], 2.5, -1, ink);
            label(&mut page, &signal, [x + box_w - 3.0 * MM, y], 2.5, 1, muted);
            pin_at.insert((end.id, pin.clone()), (x, x + box_w, y));
        }
        if !end.housing.is_empty() {
            label(
                &mut page,
                &end.housing,
                [x, top - h - 5.0 * MM],
                2.5,
                -1,
                muted,
            );
        }
    }
    // The wires.
    for (w, row) in cable.wires.iter().zip(&rows) {
        let (Some(a), Some(b)) = (
            pin_at.get(&(w.from.end, w.from.pin.clone())),
            pin_at.get(&(w.to.end, w.to.pin.clone())),
        ) else {
            continue;
        };
        let (start, end) = if a.0 < b.0 {
            (V::new(a.1, a.2), V::new(b.0, b.2))
        } else {
            (V::new(b.1, b.2), V::new(a.0, a.2))
        };
        let mid = f64::midpoint(start.x, end.x);
        page.add(
            Path::polyline(
                &[
                    start,
                    V::new(mid - 4.0 * MM, start.y),
                    V::new(mid + 4.0 * MM, end.y),
                    end,
                ],
                false,
            ),
            Paint::stroke(wire_color(&w.color), 0.7 * MM),
        );
        let text = [row.signal.as_str(), row.gauge.as_str(), row.color.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join(" · ");
        if !text.is_empty() {
            label(
                &mut page,
                &text,
                [mid, f64::midpoint(start.y, end.y) + 1.6 * MM],
                2.0,
                0,
                muted,
            );
        }
    }
    // The wiring list.
    let mut y = 60.0 * MM;
    label(
        &mut page,
        "Wiring list",
        [18.0 * MM, y + 6.0 * MM],
        3.5,
        -1,
        ink,
    );
    let columns = [18.0, 30.0, 52.0, 74.0, 96.0, 140.0, 175.0, 210.0];
    let header = [
        "Wire", "From", "Pin", "To", "Pin", "Signal", "Colour", "Gauge",
    ];
    for (x, text) in columns.iter().zip(header) {
        label(&mut page, text, [x * MM, y], 2.5, -1, muted);
    }
    for r in rows.iter().take(9) {
        y -= 4.5 * MM;
        let values = [
            r.number.to_string(),
            r.from.clone(),
            r.from_pin.clone(),
            r.to.clone(),
            r.to_pin.clone(),
            r.signal.clone(),
            r.color.clone(),
            r.gauge.clone(),
        ];
        for (x, text) in columns.iter().zip(&values) {
            label(&mut page, text, [x * MM, y], 2.5, -1, ink);
        }
    }
    if rows.len() > 9 {
        label(
            &mut page,
            &format!("… {} more wires in the wiring list (CSV)", rows.len() - 9),
            [18.0 * MM, y - 4.5 * MM],
            2.5,
            -1,
            muted,
        );
    }
    // Title block.
    page.add(
        rect(212.0 * MM, 10.0 * MM, 75.0 * MM, 18.0 * MM),
        Paint::stroke(ink, 0.3 * MM),
    );
    label(
        &mut page,
        &system.name,
        [215.0 * MM, 23.0 * MM],
        3.0,
        -1,
        ink,
    );
    label(
        &mut page,
        &format!("Cable {}", cable.name),
        [215.0 * MM, 17.5 * MM],
        2.5,
        -1,
        ink,
    );
    label(
        &mut page,
        &format!(
            "ElectronicWerk {version} · {}",
            created.get(..10).unwrap_or(created)
        ),
        [215.0 * MM, 12.5 * MM],
        2.0,
        -1,
        muted,
    );
    page
}

/// A file name part from a name (`Front panel` → `front-panel`).
#[must_use]
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_and_names() {
        assert_eq!(wire_color("RD"), Color::rgb(210, 40, 40));
        assert_eq!(wire_color("#00ff00"), Color::rgb(0, 255, 0));
        assert_eq!(wire_color("teal-ish"), Color::BLACK);
        assert_eq!(slug("Front panel / W1"), "front-panel-w1");
        assert_eq!(csv("a,b"), "\"a,b\"");
    }
}
