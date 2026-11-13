//! The drill map: the board outline with every hole marked by a symbol of
//! its drill size, and a legend with the size, count and plating of every
//! drill. Board makers use it to check the drill files.

use std::collections::BTreeMap;

use ew_model::layer::Layer;
use ew_model::pcb::Pcb;
use ew_pcb::geom::V;

use crate::art::BoardArt;
use crate::canvas::{Color, Doc, Page, Paint, Path, View};
use crate::drill::{Hole, HoleKind, holes};
use crate::font::TextSpec;

const MM: f64 = 1e6;
/// Margin around the drawing.
const MARGIN: f64 = 10.0 * MM;
/// Text height of the legend.
const TEXT: f64 = 2.0 * MM;
/// Line pitch of the legend.
const ROW: f64 = 4.5 * MM;
/// Pen width of symbols and outline.
const PEN: f64 = 0.15 * MM;

/// One drill of the legend.
#[derive(Debug, Clone, PartialEq)]
pub struct Tool {
    /// Diameter (nm).
    pub diameter: f64,
    /// Plated.
    pub plated: bool,
    /// Via, component or mechanical hole.
    pub kind: HoleKind,
    /// Number of holes.
    pub count: usize,
    /// Number of slots among them.
    pub slots: usize,
}

/// The drills of a board, by size.
#[must_use]
pub fn tools(holes: &[Hole]) -> Vec<Tool> {
    let mut map: BTreeMap<(i64, bool, HoleKind), Tool> = BTreeMap::new();
    for hole in holes {
        let key = (
            (hole.diameter / 100.0).round() as i64,
            hole.plated,
            hole.kind,
        );
        let tool = map.entry(key).or_insert(Tool {
            diameter: hole.diameter,
            plated: hole.plated,
            kind: hole.kind,
            count: 0,
            slots: 0,
        });
        tool.count += 1;
        tool.slots += usize::from(hole.end.is_some());
    }
    map.into_values().collect()
}

/// Draws symbol `index` of size `s` around `c`.
fn symbol(index: usize, c: V, s: f64) -> Path {
    let r = s / 2.0;
    let p = |x: f64, y: f64| V::new(c.x + x * r, c.y + y * r);
    let mut path = Path::default();
    let polygon = |path: &mut Path, n: usize, start: f64| {
        let points: Vec<V> = (0..n)
            .map(|i| {
                let a = start + std::f64::consts::TAU * i as f64 / n as f64;
                p(a.cos(), a.sin())
            })
            .collect();
        path.add_polyline(&points, true);
    };
    let cross = |path: &mut Path| {
        path.add_polyline(&[p(-1.0, -1.0), p(1.0, 1.0)], false);
        path.add_polyline(&[p(-1.0, 1.0), p(1.0, -1.0)], false);
    };
    let plus = |path: &mut Path| {
        path.add_polyline(&[p(-1.0, 0.0), p(1.0, 0.0)], false);
        path.add_polyline(&[p(0.0, -1.0), p(0.0, 1.0)], false);
    };
    let square = |path: &mut Path| {
        path.add_polyline(
            &[p(-1.0, -1.0), p(1.0, -1.0), p(1.0, 1.0), p(-1.0, 1.0)],
            true,
        );
    };
    match index % 12 {
        0 => {
            path.add_arc(c, r, 0.0, std::f64::consts::TAU, true);
            plus(&mut path);
        }
        1 => cross(&mut path),
        2 => square(&mut path),
        3 => polygon(&mut path, 3, std::f64::consts::FRAC_PI_2),
        4 => polygon(&mut path, 4, 0.0),
        5 => path.add_arc(c, r, 0.0, std::f64::consts::TAU, true),
        6 => plus(&mut path),
        7 => {
            square(&mut path);
            cross(&mut path);
        }
        8 => polygon(&mut path, 6, 0.0),
        9 => {
            plus(&mut path);
            cross(&mut path);
        }
        10 => {
            path.add_arc(c, r, 0.0, std::f64::consts::TAU, true);
            cross(&mut path);
        }
        _ => {
            square(&mut path);
            plus(&mut path);
        }
    }
    path
}

fn text(text: &str, x: f64, y: f64, size: f64) -> TextSpec<'_> {
    TextSpec {
        text,
        at: [x, y],
        size,
        rotation: 0.0,
        h: -1,
        v: -1,
        mirror: false,
        italic: false,
    }
}

/// Labels of the legend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labels {
    /// Title, `{board}` is replaced.
    pub title: String,
    /// Column headings: symbol, diameter, count, type.
    pub columns: [String; 4],
    /// Plated, non-plated, via.
    pub kinds: [String; 3],
    /// "slot" and "slots".
    pub slots: [String; 2],
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            title: "Drill map – {board}".to_owned(),
            columns: [
                "Symbol".to_owned(),
                "Diameter".to_owned(),
                "Count".to_owned(),
                "Type".to_owned(),
            ],
            kinds: [
                "plated".to_owned(),
                "non-plated".to_owned(),
                "via".to_owned(),
            ],
            slots: ["slot".to_owned(), "slots".to_owned()],
        }
    }
}

/// The drill map of a board as one page.
#[must_use]
#[allow(clippy::too_many_lines, reason = "drawing and legend")]
pub fn drill_map(art: &BoardArt<'_>, labels: &Labels) -> Page {
    let pcb: &Pcb = art.pcb();
    let all = holes(pcb);
    let list = tools(&all);
    let outline = art.features(Layer::Outline);
    let mut bounds = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let mut grow = |p: V, r: f64| {
        bounds = [
            bounds[0].min(p.x - r),
            bounds[1].min(p.y - r),
            bounds[2].max(p.x + r),
            bounds[3].max(p.y + r),
        ];
    };
    if let Some(r) = pcb.outline_bounds() {
        grow(V::of(r.min), 0.0);
        grow(V::of(r.max), 0.0);
    }
    for hole in &all {
        grow(hole.start, hole.diameter);
        if let Some(end) = hole.end {
            grow(end, hole.diameter);
        }
    }
    if bounds[0] > bounds[2] {
        bounds = [0.0, 0.0, 10.0 * MM, 10.0 * MM];
    }
    let (bw, bh) = (bounds[2] - bounds[0], bounds[3] - bounds[1]);
    let legend_height = ROW * (list.len() as f64 + 3.0);
    let legend_width = 120.0 * MM;
    let width = bw.max(legend_width) + 2.0 * MARGIN;
    let height = bh + legend_height + 3.0 * MARGIN;
    let mut page = Page::new("Drill map", width, height);
    let view = View {
        scale: 1.0,
        offset: V::new(MARGIN - bounds[0], legend_height + 2.0 * MARGIN - bounds[1]),
        mirror: false,
        width,
    };
    for feature in &outline {
        // Outlines in a fine, even pen.
        let thin = match feature.clone() {
            crate::art::Feature::Line {
                a,
                b,
                function,
                attributes,
                ..
            } => crate::art::Feature::Line {
                a,
                b,
                width: PEN,
                function,
                attributes,
            },
            crate::art::Feature::Arc {
                start,
                end,
                center,
                ccw,
                function,
                attributes,
                ..
            } => crate::art::Feature::Arc {
                start,
                end,
                center,
                ccw,
                width: PEN,
                function,
                attributes,
            },
            other => other,
        };
        page.feature(&thin, &view, Color::BLACK);
    }
    let symbol_size = |d: f64| (d * 1.2).clamp(0.8 * MM, 2.5 * MM);
    let index_of = |hole: &Hole| {
        list.iter().position(|t| {
            (t.diameter - hole.diameter).abs() < 100.0
                && t.plated == hole.plated
                && t.kind == hole.kind
        })
    };
    let blue = Color::rgb(20, 60, 160);
    for hole in &all {
        let Some(index) = index_of(hole) else {
            continue;
        };
        let at = view.point(hole.start);
        if let Some(end) = hole.end {
            let shape = ew_pcb::geom::Shape::stadium(hole.start, end, hole.diameter / 2.0);
            page.add(
                crate::canvas::shape_path(&shape, &view),
                Paint::stroke(blue, PEN),
            );
            let mid = view.point(V::new(
                f64::midpoint(hole.start.x, end.x),
                f64::midpoint(hole.start.y, end.y),
            ));
            page.add(
                symbol(index, mid, symbol_size(hole.diameter)),
                Paint::stroke(Color::BLACK, PEN),
            );
        } else {
            page.add(
                symbol(index, at, symbol_size(hole.diameter)),
                Paint::stroke(Color::BLACK, PEN),
            );
        }
    }
    // Legend.
    let title = labels.title.replace("{board}", &pcb.name);
    let top = legend_height + MARGIN;
    page.text(
        &text(&title, MARGIN, top - ROW, TEXT * 1.4),
        Color::BLACK,
        TEXT * 0.16,
    );
    let columns = [
        MARGIN,
        MARGIN + 22.0 * MM,
        MARGIN + 62.0 * MM,
        MARGIN + 82.0 * MM,
    ];
    let mut y = top - 2.6 * ROW;
    for (heading, x) in labels.columns.iter().zip(columns) {
        page.text(&text(heading, x, y, TEXT), Color::BLACK, TEXT * 0.16);
    }
    for (index, tool) in list.iter().enumerate() {
        y -= ROW;
        page.add(
            symbol(
                index,
                V::new(columns[0] + 4.0 * MM, y + TEXT / 2.0),
                2.5 * MM,
            ),
            Paint::stroke(Color::BLACK, PEN),
        );
        let diameter = format!(
            "{:.2} mm ({:.3}\")",
            tool.diameter / MM,
            tool.diameter / 25.4e6
        );
        page.text(
            &text(&diameter, columns[1], y, TEXT),
            Color::BLACK,
            TEXT * 0.125,
        );
        let count = tool.count.to_string();
        page.text(
            &text(&count, columns[2], y, TEXT),
            Color::BLACK,
            TEXT * 0.125,
        );
        let mut kind = match tool.kind {
            HoleKind::Via => labels.kinds[2].clone(),
            HoleKind::Component => labels.kinds[0].clone(),
            HoleKind::Mechanical => labels.kinds[1].clone(),
        };
        if tool.slots > 0 {
            let word = &labels.slots[usize::from(tool.slots != 1)];
            kind = format!("{kind}, {} {word}", tool.slots);
        }
        page.text(
            &text(&kind, columns[3], y, TEXT),
            Color::BLACK,
            TEXT * 0.125,
        );
    }
    page
}

/// The drill map as a one-page document.
#[must_use]
pub fn drill_map_doc(art: &BoardArt<'_>, labels: &Labels, version: &str, created: &str) -> Doc {
    let page = drill_map(art, labels);
    Doc {
        title: format!("{} – {}", art.pcb().name, page.name),
        version: version.to_owned(),
        created: created.to_owned(),
        pages: vec![page],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_count_holes_and_slots() {
        let hole = |d: f64, slot: bool, kind: HoleKind| Hole {
            start: V::new(0.0, 0.0),
            end: slot.then(|| V::new(1e6, 0.0)),
            diameter: d,
            plated: kind != HoleKind::Mechanical,
            kind,
            span: (0, 1),
        };
        let list = tools(&[
            hole(4e5, false, HoleKind::Via),
            hole(4e5, false, HoleKind::Via),
            hole(1e6, true, HoleKind::Component),
            hole(1e6, false, HoleKind::Component),
            hole(3e6, false, HoleKind::Mechanical),
        ]);
        assert_eq!(list.len(), 3);
        assert_eq!((list[0].count, list[0].slots), (2, 0));
        assert_eq!((list[1].count, list[1].slots), (2, 1));
        assert!(!list[2].plated);
    }

    #[test]
    fn symbols_differ() {
        let paths: Vec<Path> = (0..12).map(|i| symbol(i, V::new(0.0, 0.0), 1.0)).collect();
        for i in 0..12 {
            for j in i + 1..12 {
                assert_ne!(paths[i], paths[j], "{i} {j}");
            }
        }
    }
}
