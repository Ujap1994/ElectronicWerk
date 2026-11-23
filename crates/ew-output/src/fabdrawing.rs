//! The fabrication drawing: what a board maker needs besides the Gerber and
//! drill files, on one page — the board outline with its overall size and
//! the dimensions of the board, every hole marked by a symbol of its drill,
//! the drill table, the layer stack-up and numbered notes (material,
//! thickness, copper, finish, colours, smallest track and hole, tolerances,
//! IPC class) followed by the notes of the job.

use ew_model::dimension::{Dimension, DimensionKind, draw, new_dimension};
use ew_model::id::Id;
use ew_model::layer::{Layer, Side};
use ew_model::pcb::{DielectricKind, Pcb, StackupLayer};
use ew_model::project::Units;
use ew_model::text::{HAlign, VAlign};
use ew_model::units::Point;
use ew_pcb::geom::V;

use crate::art::BoardArt;
use crate::canvas::{Color, Doc, Page, Paint, Path, View};
use crate::drill::{HoleKind, holes};
use crate::drillmap::{symbol, thin, tools};
use crate::font::{TextSpec, extent};

const MM: f64 = 1e6;
/// Margin around everything.
const MARGIN: f64 = 10.0 * MM;
/// Room for the overall dimensions next to the board.
const DIM_ROOM: f64 = 16.0 * MM;
/// Text height of tables and notes.
const TEXT: f64 = 2.5 * MM;
/// Line pitch.
const ROW: f64 = 5.0 * MM;
/// Pen of outlines and symbols.
const PEN: f64 = 0.15 * MM;
/// Width of the column with the tables and notes.
const COLUMN: f64 = 130.0 * MM;
/// Largest board side drawn at 1:1.
const FULL_SIZE: f64 = 200.0 * MM;

/// Texts of the drawing (English: the language of board makers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Labels {
    /// Title; `{board}` is replaced.
    pub title: String,
    /// `{scale}` is replaced by `1:1`, `1:2` …
    pub scale: String,
    /// Heading of the drill table.
    pub drills: String,
    /// Heading of the stack-up.
    pub stackup: String,
    /// Columns of the stack-up: layer, material, thickness.
    pub stackup_columns: [String; 3],
    /// Silkscreen, solder mask, copper, core, prepreg.
    pub layer_kinds: [String; 5],
    /// The row with the total thickness.
    pub total: String,
    /// Heading of the notes.
    pub notes: String,
}

impl Default for Labels {
    fn default() -> Self {
        Self {
            title: "Fabrication drawing – {board}".to_owned(),
            scale: "Scale {scale}".to_owned(),
            drills: "Drills".to_owned(),
            stackup: "Layer stack-up (top to bottom)".to_owned(),
            stackup_columns: [
                "Layer".to_owned(),
                "Material".to_owned(),
                "Thickness".to_owned(),
            ],
            layer_kinds: [
                "Silkscreen".to_owned(),
                "Solder mask".to_owned(),
                "Copper".to_owned(),
                "Core".to_owned(),
                "Prepreg".to_owned(),
            ],
            total: "Total".to_owned(),
            notes: "Notes".to_owned(),
        }
    }
}

/// What the drawing shows besides the board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Notes made from the board data come first.
    pub generated_notes: bool,
    /// IPC-6012 / IPC-A-600 performance class (1, 2 or 3; 0: not named).
    pub ipc_class: u8,
    /// Further notes of the job, in order.
    pub notes: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            generated_notes: true,
            ipc_class: 2,
            notes: Vec::new(),
        }
    }
}

fn mm(nm: f64) -> String {
    let text = format!("{:.3}", nm / MM);
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn spec(text: &str, x: f64, y: f64, size: f64) -> TextSpec<'_> {
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

/// Copper thickness as µm and ounces (35 µm ≈ 1 oz).
fn copper(thickness: f64) -> String {
    let um = thickness / 1_000.0;
    let oz = um / 35.0;
    let oz = if (oz - oz.round()).abs() < 0.05 {
        format!("{}", oz.round())
    } else {
        format!("{oz:.1}")
    };
    format!("{um:.0} µm ({oz} oz)")
}

/// The notes made from the board data.
#[must_use]
#[allow(clippy::cast_precision_loss, reason = "board lengths fit f64")]
pub fn generated_notes(pcb: &Pcb, ipc_class: u8) -> Vec<String> {
    let mut notes = Vec::new();
    let layers = pcb.stackup.copper_count();
    if let Some(r) = pcb.outline_bounds() {
        notes.push(format!(
            "Board: {} × {} mm, {} mm thick, {layers} copper layer{}.",
            mm((r.max.x - r.min.x) as f64),
            mm((r.max.y - r.min.y) as f64),
            mm(pcb.stackup.thickness() as f64),
            if layers == 1 { "" } else { "s" },
        ));
    }
    let mut materials: Vec<String> = Vec::new();
    let mut outer: Vec<f64> = Vec::new();
    let mut inner: Vec<f64> = Vec::new();
    let mut masks: Vec<String> = Vec::new();
    let mut silks: Vec<String> = Vec::new();
    for layer in &pcb.stackup.layers {
        match layer {
            StackupLayer::Dielectric { material, .. } => {
                if !material.is_empty() && !materials.contains(material) {
                    materials.push(material.clone());
                }
            }
            StackupLayer::Copper {
                layer, thickness, ..
            } => {
                if matches!(
                    layer,
                    ew_model::layer::CopperLayer::Top | ew_model::layer::CopperLayer::Bottom
                ) {
                    outer.push(*thickness as f64);
                } else {
                    inner.push(*thickness as f64);
                }
            }
            StackupLayer::SolderMask { side, color, .. } if !color.is_empty() => {
                masks.push(format!("{color} ({})", side_name(*side)));
            }
            StackupLayer::Silkscreen { side, color } if !color.is_empty() => {
                silks.push(format!("{color} ({})", side_name(*side)));
            }
            _ => {}
        }
    }
    if !materials.is_empty() {
        notes.push(format!("Base material: {}.", materials.join(", ")));
    }
    let copper_note = |list: &[f64]| list.first().map(|t| copper(*t));
    match (copper_note(&outer), copper_note(&inner)) {
        (Some(o), Some(i)) => notes.push(format!(
            "Finished copper: outer layers {o}, inner layers {i}."
        )),
        (Some(o), None) => notes.push(format!("Finished copper: {o}.")),
        _ => {}
    }
    if !pcb.stackup.finish.is_empty() {
        notes.push(format!("Surface finish: {}.", pcb.stackup.finish));
    }
    if !masks.is_empty() {
        notes.push(format!("Solder mask: {}.", masks.join(", ")));
    }
    if !silks.is_empty() {
        notes.push(format!("Silkscreen: {}.", silks.join(", ")));
    }
    let narrowest = pcb.tracks.iter().map(|t| t.width).filter(|w| *w > 0).min();
    let all = holes(pcb);
    let smallest = all
        .iter()
        .map(|h| h.diameter)
        .filter(|d| *d > 0.0)
        .fold(f64::INFINITY, f64::min);
    let mut small = Vec::new();
    if let Some(w) = narrowest {
        small.push(format!("narrowest track {} mm", mm(w as f64)));
    }
    if smallest.is_finite() {
        small.push(format!("smallest drill {} mm", mm(smallest)));
    }
    if !small.is_empty() {
        notes.push(format!("Smallest features: {}.", small.join(", ")));
    }
    notes.push("Hole sizes are finished sizes (after plating).".to_owned());
    notes.push(
        "Tolerances unless noted: outline ±0.1 mm, plated holes ±0.075 mm, \
         non-plated holes ±0.05 mm."
            .to_owned(),
    );
    if ipc_class > 0 {
        notes.push(format!(
            "Manufacture and inspect to IPC-6012 and IPC-A-600 class {ipc_class}."
        ));
    }
    notes.push("All dimensions in millimetres.".to_owned());
    notes
}

const fn side_name(side: Side) -> &'static str {
    match side {
        Side::Top => "top",
        Side::Bottom => "bottom",
    }
}

/// The drawing scale: 1:1 up to 200 mm, then 1:2, 1:2.5, 1:4, 1:5, 1:10 …
fn scale_for(size: f64) -> (f64, String) {
    if size <= FULL_SIZE {
        return (1.0, "1:1".to_owned());
    }
    for n in [2.0, 2.5, 4.0, 5.0, 10.0, 20.0, 50.0] {
        if size / n <= FULL_SIZE {
            return (1.0 / n, format!("1:{n}"));
        }
    }
    (FULL_SIZE / size, "1:100".to_owned())
}

/// Breaks a note into lines that fit `width` at `size`.
fn wrap(text: &str, width: f64, size: f64) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if !line.is_empty() && extent(&candidate, size).0 > width {
            lines.push(std::mem::take(&mut line));
            word.clone_into(&mut line);
        } else {
            line = candidate;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Draws a dimension (board coordinates) through the view.
fn draw_dimension(page: &mut Page, dim: &Dimension, view: &View, color: Color) {
    let drawing = draw(dim, Units::Mm);
    #[allow(clippy::cast_precision_loss, reason = "line widths fit f64")]
    let pen = view.length(dim.width as f64).max(0.05 * MM);
    for (a, b) in &drawing.lines {
        page.add(
            Path::polyline(&[view.point(V::of(*a)), view.point(V::of(*b))], false),
            Paint::stroke(color, pen),
        );
    }
    let at = view.point(V::of(drawing.at));
    #[allow(clippy::cast_precision_loss, reason = "text sizes fit f64")]
    let size = view.length(drawing.style.size as f64);
    let h = match drawing.style.h_align {
        HAlign::Left => -1,
        HAlign::Center => 0,
        HAlign::Right => 1,
    };
    let v = match drawing.style.v_align {
        VAlign::Bottom => -1,
        VAlign::Center => 0,
        VAlign::Top => 1,
    };
    page.text(
        &TextSpec {
            text: &drawing.text,
            at: [at.x, at.y],
            size,
            rotation: drawing.rotation,
            h,
            v,
            mirror: false,
            italic: false,
        },
        color,
        size * 0.125,
    );
}

/// The fabrication drawing of a board as one page.
#[must_use]
#[allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    reason = "drawing, tables and notes on one page"
)]
pub fn fabrication_drawing(art: &BoardArt<'_>, labels: &Labels, options: &Options) -> Page {
    let pcb: &Pcb = art.pcb();
    let all = holes(pcb);
    let list = tools(&all);
    let bounds = pcb
        .outline_bounds()
        .map_or([0.0, 0.0, 50.0 * MM, 50.0 * MM], |r| {
            [
                r.min.x as f64,
                r.min.y as f64,
                r.max.x as f64,
                r.max.y as f64,
            ]
        });
    let (bw, bh) = (bounds[2] - bounds[0], bounds[3] - bounds[1]);
    let (scale, scale_text) = scale_for(bw.max(bh));

    // The column: drill table, stack-up, notes.
    let mut notes: Vec<String> = Vec::new();
    if options.generated_notes {
        notes.extend(generated_notes(pcb, options.ipc_class));
    }
    notes.extend(
        options
            .notes
            .iter()
            .filter(|n| !n.trim().is_empty())
            .cloned(),
    );
    let note_lines: Vec<(usize, Vec<String>)> = notes
        .iter()
        .enumerate()
        .map(|(i, note)| (i + 1, wrap(note, COLUMN - 8.0 * MM, TEXT)))
        .collect();
    let stack_rows = pcb.stackup.layers.len() + 1;
    let column_height = ROW * (list.len() as f64 + 2.5)
        + ROW * (stack_rows as f64 + 2.5)
        + ROW * (note_lines.iter().map(|(_, l)| l.len()).sum::<usize>() as f64 + 1.5);

    let drawing_width = bw * scale + 2.0 * DIM_ROOM;
    let drawing_height = bh * scale + 2.0 * DIM_ROOM;
    let title_room = 14.0 * MM;
    let width = MARGIN * 3.0 + drawing_width + COLUMN;
    let height = MARGIN * 2.0 + title_room + drawing_height.max(column_height);
    let mut page = Page::new("Fabrication drawing", width, height);
    let top = height - MARGIN;

    // Title and scale.
    let title = labels.title.replace("{board}", &pcb.name);
    page.text(
        &spec(&title, MARGIN, top - 5.0 * MM, 4.0 * MM),
        Color::BLACK,
        0.5 * MM,
    );
    let scale_label = labels.scale.replace("{scale}", &scale_text);
    page.text(
        &spec(&scale_label, MARGIN, top - 11.0 * MM, TEXT),
        Color::BLACK,
        TEXT * 0.125,
    );

    // The board: outline, holes, dimensions, overall size.
    let area_top = top - title_room;
    let view = View {
        scale,
        offset: V::new(
            MARGIN + DIM_ROOM - bounds[0] * scale,
            area_top - DIM_ROOM - bh * scale - bounds[1] * scale,
        ),
        mirror: false,
        width,
    };
    for feature in &art.features(Layer::Outline) {
        page.feature(&thin(feature, PEN / scale), &view, Color::BLACK);
    }
    let blue = Color::rgb(20, 60, 160);
    let index_of = |diameter: f64, plated: bool, kind: HoleKind| {
        list.iter().position(|t| {
            (t.diameter - diameter).abs() < 100.0 && t.plated == plated && t.kind == kind
        })
    };
    let symbol_size = |d: f64| (d * 1.2 * scale).clamp(0.8 * MM, 2.5 * MM);
    for hole in &all {
        let Some(index) = index_of(hole.diameter, hole.plated, hole.kind) else {
            continue;
        };
        let centre = hole.end.map_or(hole.start, |end| {
            V::new(
                f64::midpoint(hole.start.x, end.x),
                f64::midpoint(hole.start.y, end.y),
            )
        });
        if let Some(end) = hole.end {
            let shape = ew_pcb::geom::Shape::stadium(hole.start, end, hole.diameter / 2.0);
            page.add(
                crate::canvas::shape_path(&shape, &view),
                Paint::stroke(blue, PEN),
            );
        }
        page.add(
            symbol(index, view.point(centre), symbol_size(hole.diameter)),
            Paint::stroke(Color::BLACK, PEN),
        );
    }
    for dim in &pcb.dimensions {
        draw_dimension(&mut page, dim, &view, Color::BLACK);
    }
    // The overall size: below and left of the board, in page sizes.
    #[allow(clippy::cast_possible_truncation, reason = "board coordinates fit i64")]
    let corner = |x: f64, y: f64| Point::new(x.round() as i64, y.round() as i64);
    let overall = |kind: DimensionKind, end: Point| {
        let mut dim = new_dimension(
            Id::NIL,
            Layer::User(1),
            kind,
            corner(bounds[0], bounds[1]),
            end,
            0,
        );
        #[allow(clippy::cast_possible_truncation, reason = "page lengths fit i64")]
        {
            dim.offset = -((8.0 * MM / scale).round() as i64);
            dim.style.size = (TEXT / scale).round() as i64;
            dim.width = (PEN / scale).round() as i64;
        }
        dim
    };
    draw_dimension(
        &mut page,
        &overall(DimensionKind::Horizontal, corner(bounds[2], bounds[1])),
        &view,
        Color::BLACK,
    );
    draw_dimension(
        &mut page,
        &overall(DimensionKind::Vertical, corner(bounds[0], bounds[3])),
        &view,
        Color::BLACK,
    );

    // The column.
    let x0 = MARGIN * 2.0 + drawing_width;
    let mut y = area_top;
    let heading = |page: &mut Page, text: &str, y: f64| {
        page.text(&spec(text, x0, y, TEXT * 1.2), Color::BLACK, TEXT * 0.18);
    };
    let cell = |page: &mut Page, text: &str, x: f64, y: f64| {
        page.text(&spec(text, x, y, TEXT), Color::BLACK, TEXT * 0.125);
    };
    let rule = |page: &mut Page, y: f64| {
        page.add(
            Path::polyline(&[V::new(x0, y), V::new(x0 + COLUMN, y)], false),
            Paint::stroke(Color::BLACK, PEN),
        );
    };

    // Drill table.
    y -= ROW;
    heading(&mut page, &labels.drills, y);
    let drill_labels = crate::drillmap::Labels::default();
    let columns = [x0, x0 + 18.0 * MM, x0 + 62.0 * MM, x0 + 82.0 * MM];
    y -= ROW;
    for (text, x) in drill_labels.columns.iter().zip(columns) {
        cell(&mut page, text, x, y);
    }
    rule(&mut page, y - 1.2 * MM);
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
        cell(
            &mut page,
            &format!("{} mm ({:.3}\")", mm(tool.diameter), tool.diameter / 25.4e6),
            columns[1],
            y,
        );
        cell(&mut page, &tool.count.to_string(), columns[2], y);
        let mut kind = match tool.kind {
            HoleKind::Via => drill_labels.kinds[2].clone(),
            HoleKind::Component => drill_labels.kinds[0].clone(),
            HoleKind::Mechanical => drill_labels.kinds[1].clone(),
        };
        if tool.slots > 0 {
            let word = &drill_labels.slots[usize::from(tool.slots != 1)];
            kind = format!("{kind}, {} {word}", tool.slots);
        }
        cell(&mut page, &kind, columns[3], y);
    }

    // Stack-up.
    y -= ROW * 1.5;
    heading(&mut page, &labels.stackup, y);
    let columns = [x0, x0 + 52.0 * MM, x0 + 100.0 * MM];
    y -= ROW;
    for (text, x) in labels.stackup_columns.iter().zip(columns) {
        cell(&mut page, text, x, y);
    }
    rule(&mut page, y - 1.2 * MM);
    for layer in &pcb.stackup.layers {
        y -= ROW;
        let (name, material) = match layer {
            StackupLayer::Silkscreen { side, color } => (
                format!("{} ({})", labels.layer_kinds[0], side_name(*side)),
                color.clone(),
            ),
            StackupLayer::SolderMask { side, color, .. } => (
                format!("{} ({})", labels.layer_kinds[1], side_name(*side)),
                color.clone(),
            ),
            StackupLayer::Copper { layer, name, .. } => {
                let shown = if name.is_empty() {
                    match layer {
                        ew_model::layer::CopperLayer::Top => "top".to_owned(),
                        ew_model::layer::CopperLayer::Bottom => "bottom".to_owned(),
                        ew_model::layer::CopperLayer::Inner(n) => format!("inner {n}"),
                    }
                } else {
                    name.clone()
                };
                (format!("{} {shown}", labels.layer_kinds[2]), String::new())
            }
            StackupLayer::Dielectric {
                kind,
                material,
                epsilon_r,
                ..
            } => (
                match kind {
                    DielectricKind::Core => labels.layer_kinds[3].clone(),
                    DielectricKind::Prepreg => labels.layer_kinds[4].clone(),
                },
                if material.is_empty() {
                    String::new()
                } else {
                    format!("{material} (Dk {epsilon_r})")
                },
            ),
        };
        cell(&mut page, &name, columns[0], y);
        cell(&mut page, &material, columns[1], y);
        let thickness = layer.thickness();
        if thickness > 0 {
            cell(
                &mut page,
                &format!("{} mm", mm(thickness as f64)),
                columns[2],
                y,
            );
        }
    }
    rule(&mut page, y - 1.2 * MM);
    y -= ROW;
    cell(&mut page, &labels.total, columns[0], y);
    cell(
        &mut page,
        &format!("{} mm", mm(pcb.stackup.thickness() as f64)),
        columns[2],
        y,
    );

    // Notes.
    y -= ROW * 1.5;
    heading(&mut page, &labels.notes, y);
    for (number, lines) in &note_lines {
        for (i, line) in lines.iter().enumerate() {
            y -= ROW;
            if i == 0 {
                cell(&mut page, &format!("{number}."), x0, y);
            }
            cell(&mut page, line, x0 + 8.0 * MM, y);
        }
    }
    page
}

/// The fabrication drawing as a one-page document.
#[must_use]
pub fn fabrication_doc(
    art: &BoardArt<'_>,
    labels: &Labels,
    options: &Options,
    version: &str,
    created: &str,
) -> Doc {
    let page = fabrication_drawing(art, labels, options);
    Doc {
        title: labels.title.replace("{board}", &art.pcb().name),
        version: version.to_owned(),
        created: created.to_owned(),
        pages: vec![page],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_wrap_and_scales_are_round() {
        let lines = wrap(
            "Tolerances unless noted: outline ±0.1 mm, plated holes ±0.075 mm, non-plated holes ±0.05 mm.",
            60.0 * MM,
            TEXT,
        );
        assert!(lines.len() > 1, "{lines:?}");
        assert!(lines.iter().all(|l| extent(l, TEXT).0 <= 60.0 * MM));
        assert_eq!(scale_for(150.0 * MM).1, "1:1");
        assert_eq!(scale_for(350.0 * MM).1, "1:2");
        assert_eq!(scale_for(450.0 * MM).1, "1:2.5");
        assert_eq!(copper(35_000.0), "35 µm (1 oz)");
        assert_eq!(copper(17_500.0), "18 µm (0.5 oz)");
        assert_eq!(mm(1_600_000.0), "1.6");
    }
}
