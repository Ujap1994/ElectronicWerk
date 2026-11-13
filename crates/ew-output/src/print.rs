//! Prints: schematics as the viewer draws them, board layers and assembly
//! drawings, as PDF pages or SVG files.

use std::collections::BTreeSet;
use std::f64::consts::TAU;

use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, Side};
use ew_model::project::Project;
use ew_pcb::geom::V;
use ew_scene::plot::{Plot, PlotLayer};
use ew_schematic::connectivity::natural_cmp;
use serde::{Deserialize, Serialize};

use crate::art::{BoardArt, Feature};
use crate::canvas::{Color, Doc, Page, Paint, Path, View, shape_path};
use crate::font::{self, TextSpec};

const MM: f64 = 1e6;

/// Paper sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Paper {
    /// The size of the drawing plus a margin, at scale 1:1.
    #[default]
    Fit,
    /// ISO A4.
    A4,
    /// ISO A3.
    A3,
    /// US Letter.
    Letter,
}

impl Paper {
    /// Width and height in landscape orientation.
    #[must_use]
    pub const fn landscape(self) -> Option<(f64, f64)> {
        match self {
            Self::Fit => None,
            Self::A4 => Some((297.0 * MM, 210.0 * MM)),
            Self::A3 => Some((420.0 * MM, 297.0 * MM)),
            Self::Letter => Some((279.4 * MM, 215.9 * MM)),
        }
    }
}

/// Colours of a print.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ColorMode {
    /// The colours of the layers.
    #[default]
    Color,
    /// Everything black (fills light grey).
    Monochrome,
}

/// Print colour of a schematic layer (the viewer's light theme).
#[must_use]
pub fn schematic_color(key: &str) -> Option<Color> {
    let hex = match key {
        "sch.frame" => "#8c3c3c",
        "sch.wires" | "sch.junctions" => "#008400",
        "sch.buses" | "sch.noConnects" => "#0000c8",
        "sch.labels" | "sch.power" => "#8c1515",
        "sch.symbols" | "sch.pins" => "#8c0000",
        "sch.symbolFill" => "#ffffc2",
        "sch.pinText" => "#007878",
        "sch.fields" => "#006464",
        "sch.sheets" => "#840084",
        "sch.texts" | "sch.shapes" => "#1d1d6e",
        _ => return None,
    };
    Color::hex(hex)
}

/// Print colour of a board layer (darker than the viewer's colours, for
/// white paper).
#[must_use]
pub fn board_color(layer: Layer) -> Color {
    let hex = match layer {
        Layer::Copper(CopperLayer::Top) => "#c83434",
        Layer::Copper(CopperLayer::Bottom) => "#3c64b4",
        Layer::Copper(CopperLayer::Inner(n)) => [
            "#a0a000", "#a000a0", "#00a0a0", "#c06000", "#5050d0", "#00a050",
        ][usize::from(n.saturating_sub(1)) % 6],
        Layer::Silkscreen(_) => "#202020",
        Layer::SolderMask(Side::Top) => "#a050c8",
        Layer::SolderMask(Side::Bottom) => "#00a0a0",
        Layer::Paste(_) => "#808080",
        Layer::Adhesive(_) => "#c05050",
        Layer::Fabrication(_) => "#505050",
        Layer::Courtyard(_) => "#c020b0",
        Layer::Outline => "#000000",
        Layer::Mechanical(_) | Layer::User(_) => "#008060",
    };
    Color::hex(hex).unwrap_or(Color::BLACK)
}

/// Stroke width of texts drawn from a plot.
fn text_width(size: f64, bold: bool) -> f64 {
    font::default_thickness(size, bold)
}

/// Draws one plot layer onto a page.
fn draw_layer(page: &mut Page, layer: &PlotLayer, view: &View, color: Color, min_width: f64) {
    let p = |q: [f64; 2]| view.point(V::new(q[0], q[1]));
    let width = |w: f64| view.length(w).max(min_width);
    for polygon in &layer.polygons {
        let mut path = Path::default();
        let ring = |r: &[[f64; 2]]| r.iter().map(|q| p(*q)).collect::<Vec<V>>();
        path.add_polyline(&ring(&polygon.outline), true);
        for hole in &polygon.holes {
            path.add_polyline(&ring(hole), true);
        }
        if polygon.hatched {
            page.add(path, Paint::stroke(color, min_width));
        } else {
            page.add(path, Paint::fill(color));
        }
    }
    for rect in &layer.rects {
        let c = V::new(rect.center[0], rect.center[1]);
        let r = rect.corner.min(rect.half_w).min(rect.half_h);
        let (hw, hh) = (rect.half_w - r, rect.half_h - r);
        let mut points = Vec::new();
        for (k, (sx, sy)) in [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)]
            .into_iter()
            .enumerate()
        {
            let corner = V::new(sx * hw, sy * hh);
            let steps = if r > 0.0 { 6 } else { 0 };
            for i in 0..=steps {
                let a = (k as f64 + f64::from(i) / f64::from(steps.max(1))) * TAU / 4.0;
                let q = corner.add(V::new(r * a.cos(), r * a.sin()));
                points.push(p([
                    c.x + q.x * rect.angle.cos() - q.y * rect.angle.sin(),
                    c.y + q.x * rect.angle.sin() + q.y * rect.angle.cos(),
                ]));
                if r <= 0.0 {
                    break;
                }
            }
        }
        page.add(Path::polyline(&points, true), Paint::fill(color));
    }
    for circle in &layer.circles {
        let c = p(circle.center);
        let mut path = Path::circle(c, view.length(circle.outer));
        if circle.inner > 0.0 {
            path.0.extend(Path::circle(c, view.length(circle.inner)).0);
        }
        page.add(path, Paint::fill(color));
    }
    for segment in &layer.segments {
        page.add(
            Path::polyline(&[p(segment.a), p(segment.b)], false),
            Paint::stroke(color, width(segment.width)),
        );
    }
    for arc in &layer.arcs {
        let mut path = Path::default();
        let start = if view.mirror {
            std::f64::consts::PI - arc.start
        } else {
            arc.start
        };
        let sweep = if view.mirror { -arc.sweep } else { arc.sweep };
        path.add_arc(p(arc.center), view.length(arc.radius), start, sweep, true);
        page.add(path, Paint::stroke(color, width(arc.width)));
    }
    for text in &layer.texts {
        let at = p(text.at);
        let size = view.length(text.size);
        let spec = TextSpec {
            text: &text.text,
            at: [at.x, at.y],
            size,
            rotation: text.rotation,
            h: text.h,
            v: text.v,
            mirror: text.mirror != view.mirror,
            italic: text.italic,
        };
        page.text(&spec, color, text_width(size, text.bold).max(min_width));
    }
}

/// The schematic sheets of a project in reading order: the top sheet, then
/// its sub-sheets depth first, then sheets that are not used.
#[must_use]
pub fn sheet_order(project: &Project) -> Vec<Id> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut stack: Vec<Id> = project.manifest.root_schematic.into_iter().collect();
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some(Document::Schematic(sheet)) = project.document(id) else {
            continue;
        };
        out.push(id);
        let mut children: Vec<_> = sheet.sheets.iter().collect();
        // Top to bottom, left to right.
        children.sort_by(|a, b| b.at.y.cmp(&a.at.y).then(a.at.x.cmp(&b.at.x)));
        for child in children.into_iter().rev() {
            stack.push(child.schematic);
        }
    }
    let mut rest: Vec<(&str, Id)> = project
        .documents
        .values()
        .filter_map(|d| match d {
            Document::Schematic(s) if !seen.contains(&s.id) => Some((s.name.as_str(), s.id)),
            _ => None,
        })
        .collect();
    rest.sort_by(|a, b| natural_cmp(a.0, b.0));
    out.extend(rest.into_iter().map(|(_, id)| id));
    out
}

/// One printed page of a plot (sheet or board as the viewer draws it).
#[must_use]
pub fn plot_page(plot: &Plot, mode: ColorMode, paper: Paper) -> Page {
    let (page_w, page_h) = plot
        .page
        .map(|[w, h]| (w as f64, h as f64))
        .or_else(|| {
            plot.bounds
                .map(|b| (b[2] - b[0] + 20.0 * MM, b[3] - b[1] + 20.0 * MM))
        })
        .unwrap_or((297.0 * MM, 210.0 * MM));
    let model = plot.page.map_or_else(
        || plot.bounds.unwrap_or([0.0, 0.0, page_w, page_h]),
        |[w, h]| [0.0, 0.0, w as f64, h as f64],
    );
    let (width, height) = match paper.landscape() {
        None => (page_w, page_h),
        Some((w, h)) if page_w >= page_h => (w, h),
        Some((w, h)) => (h, w),
    };
    let margin = if plot.page.is_some() && paper == Paper::Fit {
        0.0
    } else {
        10.0 * MM
    };
    let view = View::fit(
        model,
        [margin, margin, width - 2.0 * margin, height - 2.0 * margin],
        f64::MAX,
        false,
        width,
    );
    let mut page = Page::new(plot.name.clone(), width, height);
    let mut layers: Vec<&PlotLayer> = plot
        .layers
        .iter()
        .filter(|l| l.visible && l.key != "sch.page")
        .collect();
    layers.sort_by_key(|l| l.z);
    for layer in layers {
        let color = match mode {
            ColorMode::Color => schematic_color(&layer.key).unwrap_or(Color::BLACK),
            ColorMode::Monochrome if layer.key == "sch.symbolFill" => Color::rgb(240, 240, 240),
            ColorMode::Monochrome => Color::BLACK,
        };
        draw_layer(&mut page, layer, &view, color, 0.05 * MM);
    }
    page
}

/// Options of a schematic print.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SchematicPrint {
    /// Colours.
    pub color: ColorMode,
    /// Paper; `fit` uses the page size of each sheet.
    pub paper: Paper,
}

/// Every sheet of a project, one page each.
#[must_use]
pub fn schematic_pages(
    project: &Project,
    options: &ew_scene::Options<'_>,
    print: SchematicPrint,
) -> Vec<Page> {
    sheet_order(project)
        .into_iter()
        .filter_map(|id| ew_scene::plot(project, id, options).ok())
        .map(|plot| plot_page(&plot, print.color, print.paper))
        .collect()
}

/// Options of a board print.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BoardPrint {
    /// Layers, drawn in this order (later on top).
    pub layers: Vec<Layer>,
    /// One page per layer (otherwise all layers on one page).
    pub page_per_layer: bool,
    /// Seen from the bottom (mirrored).
    pub mirror: bool,
    /// Colours.
    pub color: ColorMode,
    /// Paper.
    pub paper: Paper,
    /// Always draw the board outline.
    pub outline: bool,
}

impl Default for BoardPrint {
    fn default() -> Self {
        Self {
            layers: vec![
                Layer::Copper(CopperLayer::Top),
                Layer::Silkscreen(Side::Top),
            ],
            page_per_layer: false,
            mirror: false,
            color: ColorMode::Color,
            paper: Paper::Fit,
            outline: true,
        }
    }
}

/// The box of the board (outline, or everything on it).
fn board_box(art: &BoardArt<'_>) -> [f64; 4] {
    if let Some(r) = art.pcb().outline_bounds() {
        return [
            r.min.x as f64,
            r.min.y as f64,
            r.max.x as f64,
            r.max.y as f64,
        ];
    }
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for fp in &art.pcb().footprints {
        b = [
            b[0].min(fp.at.x as f64),
            b[1].min(fp.at.y as f64),
            b[2].max(fp.at.x as f64),
            b[3].max(fp.at.y as f64),
        ];
    }
    if b[0] > b[2] {
        [0.0, 0.0, 100.0 * MM, 100.0 * MM]
    } else {
        [
            b[0] - 5.0 * MM,
            b[1] - 5.0 * MM,
            b[2] + 5.0 * MM,
            b[3] + 5.0 * MM,
        ]
    }
}

/// A page for a board drawing with a caption line at the bottom.
fn board_page(art: &BoardArt<'_>, name: String, paper: Paper, mirror: bool) -> (Page, View) {
    let model = board_box(art);
    let caption = 8.0 * MM;
    let margin = 10.0 * MM;
    let (mw, mh) = (model[2] - model[0], model[3] - model[1]);
    let (width, height) = match paper.landscape() {
        None => (mw + 2.0 * margin, mh + 2.0 * margin + caption),
        Some((w, h)) if mw >= mh => (w, h),
        Some((w, h)) => (h, w),
    };
    let max_scale = if paper == Paper::Fit { 1.0 } else { 10.0 };
    let view = View::fit(
        model,
        [
            margin,
            margin + caption,
            width - 2.0 * margin,
            height - 2.0 * margin - caption,
        ],
        max_scale,
        mirror,
        width,
    );
    (Page::new(name, width, height), view)
}

/// The English name of a board layer (captions of prints).
#[must_use]
pub fn layer_name(layer: Layer) -> String {
    let side = |s: Side| if s == Side::Top { "Top" } else { "Bottom" };
    match layer {
        Layer::Copper(CopperLayer::Top) => "Top copper".to_owned(),
        Layer::Copper(CopperLayer::Bottom) => "Bottom copper".to_owned(),
        Layer::Copper(CopperLayer::Inner(n)) => format!("Inner copper {n}"),
        Layer::Silkscreen(s) => format!("{} silkscreen", side(s)),
        Layer::SolderMask(s) => format!("{} solder mask", side(s)),
        Layer::Paste(s) => format!("{} paste", side(s)),
        Layer::Adhesive(s) => format!("{} adhesive", side(s)),
        Layer::Fabrication(s) => format!("{} fabrication", side(s)),
        Layer::Courtyard(s) => format!("{} courtyard", side(s)),
        Layer::Outline => "Board outline".to_owned(),
        Layer::Mechanical(n) => format!("Mechanical {n}"),
        Layer::User(n) => format!("User {n}"),
    }
}

/// A caption at the bottom of a page, small enough to fit its width.
fn caption(page: &mut Page, text: &str) {
    let chars = text.chars().count().max(1) as f64;
    let size = ((page.width - 20.0 * MM) / (chars * font::ADVANCE)).clamp(0.8 * MM, 2.0 * MM);
    page.text(
        &TextSpec {
            text,
            at: [10.0 * MM, 5.0 * MM],
            size,
            rotation: 0.0,
            h: -1,
            v: -1,
            mirror: false,
            italic: false,
        },
        Color::BLACK,
        size * 0.125,
    );
}

/// Board layers as pages.
#[must_use]
pub fn board_pages(art: &BoardArt<'_>, print: &BoardPrint, title: &str) -> Vec<Page> {
    let groups: Vec<Vec<Layer>> = if print.page_per_layer {
        print.layers.iter().map(|l| vec![*l]).collect()
    } else {
        vec![print.layers.clone()]
    };
    let mut pages = Vec::new();
    for layers in groups {
        let names: Vec<String> = layers.iter().map(|l| layer_name(*l)).collect();
        let name = names.join(", ");
        let (mut page, view) = board_page(art, name.clone(), print.paper, print.mirror);
        let mut order = layers.clone();
        if print.outline && !order.contains(&Layer::Outline) {
            order.push(Layer::Outline);
        }
        for layer in order {
            let color = match print.color {
                ColorMode::Color => board_color(layer),
                ColorMode::Monochrome => Color::BLACK,
            };
            for feature in art.features(layer) {
                let feature = if layer == Layer::Outline {
                    thin(feature, 0.1 * MM / view.scale)
                } else {
                    feature
                };
                page.feature(&feature, &view, color);
            }
        }
        let view_label = if print.mirror { " (bottom view)" } else { "" };
        caption(&mut page, &format!("{title} – {name}{view_label}"));
        pages.push(page);
    }
    pages
}

/// The same feature drawn with another pen width.
fn thin(feature: Feature, width: f64) -> Feature {
    match feature {
        Feature::Line {
            a,
            b,
            function,
            attributes,
            ..
        } => Feature::Line {
            a,
            b,
            width,
            function,
            attributes,
        },
        Feature::Arc {
            start,
            end,
            center,
            ccw,
            function,
            attributes,
            ..
        } => Feature::Arc {
            start,
            end,
            center,
            ccw,
            width,
            function,
            attributes,
        },
        other => other,
    }
}

/// Options of an assembly drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[allow(clippy::struct_excessive_bools, reason = "independent switches")]
pub struct AssemblyPrint {
    /// Top side page.
    pub top: bool,
    /// Bottom side page (seen from the bottom).
    pub bottom: bool,
    /// Show values under the designators.
    pub values: bool,
    /// Draw pads.
    pub pads: bool,
    /// Paper.
    pub paper: Paper,
}

impl Default for AssemblyPrint {
    fn default() -> Self {
        Self {
            top: true,
            bottom: true,
            values: false,
            pads: true,
            paper: Paper::A4,
        }
    }
}

/// Assembly drawings: per side the outline, part outlines (fabrication
/// layer), pads and a designator in the middle of every part.
#[must_use]
pub fn assembly_pages(art: &BoardArt<'_>, print: &AssemblyPrint, title: &str) -> Vec<Page> {
    let pcb = art.pcb();
    let mut pages = Vec::new();
    for (side, wanted, label) in [
        (Side::Top, print.top, "top"),
        (Side::Bottom, print.bottom, "bottom"),
    ] {
        if !wanted || !pcb.footprints.iter().any(|f| f.side == side) && side == Side::Bottom {
            continue;
        }
        let mirror = side == Side::Bottom;
        let name = format!("Assembly {label}");
        let (mut page, view) = board_page(art, name.clone(), print.paper, mirror);
        if print.pads {
            let copper = if side == Side::Top {
                Layer::TOP_COPPER
            } else {
                Layer::BOTTOM_COPPER
            };
            for feature in art.features(copper) {
                if let Feature::Flash {
                    shape, attributes, ..
                } = &feature
                    && attributes.pin.is_some()
                {
                    page.add(
                        shape_path(shape, &view),
                        Paint::fill(Color::rgb(215, 215, 215)),
                    );
                }
            }
        }
        // Fabrication texts are replaced by the designators below.
        let plain = art.with_options(crate::art::ArtOptions {
            texts: false,
            ..art.options()
        });
        for feature in plain.features(Layer::Fabrication(side)) {
            page.feature(&feature, &view, Color::rgb(60, 60, 60));
        }
        for feature in art.features(Layer::Outline) {
            page.feature(&thin(feature, 0.15 * MM / view.scale), &view, Color::BLACK);
        }
        for fp in pcb.footprints.iter().filter(|f| f.side == side) {
            let box_ = footprint_box(fp);
            let (bw, bh) = (
                view.length(box_[2] - box_[0]),
                view.length(box_[3] - box_[1]),
            );
            let center = view.point(V::new(
                f64::midpoint(box_[0], box_[2]),
                f64::midpoint(box_[1], box_[3]),
            ));
            let chars = fp.designator.chars().count().max(1) as f64;
            let vertical = bh > bw * 1.3;
            let (long, short) = if vertical { (bh, bw) } else { (bw, bh) };
            let size = (long * 0.8 / (chars * font::ADVANCE))
                .min(short * 0.45)
                .clamp(0.6 * MM, 2.5 * MM);
            let lines = if print.values && !fp.value.is_empty() {
                format!("{}\n{}", fp.designator, fp.value)
            } else {
                fp.designator.clone()
            };
            page.text(
                &TextSpec {
                    text: &lines,
                    at: [center.x, center.y],
                    size,
                    rotation: if vertical { 90.0 } else { 0.0 },
                    h: 0,
                    v: 0,
                    mirror: false,
                    italic: false,
                },
                Color::rgb(20, 40, 140),
                size * 0.14,
            );
        }
        caption(
            &mut page,
            &format!(
                "{title} – {name}{}",
                if mirror {
                    " (seen from the bottom)"
                } else {
                    ""
                }
            ),
        );
        pages.push(page);
    }
    pages
}

/// The box of a footprint on the board: its courtyard, else its pads and
/// fabrication graphics.
#[must_use]
pub fn footprint_box(fp: &ew_model::pcb::FootprintInstance) -> [f64; 4] {
    let xf = ew_pcb::items::footprint_xform(fp);
    let mut points: Vec<V> = Vec::new();
    let mut courtyard = false;
    for graphic in &fp.graphics {
        if matches!(graphic.layer, Layer::Courtyard(_)) {
            courtyard = true;
        }
    }
    for graphic in &fp.graphics {
        let wanted = if courtyard {
            matches!(graphic.layer, Layer::Courtyard(_))
        } else {
            matches!(graphic.layer, Layer::Fabrication(_))
        };
        if !wanted {
            continue;
        }
        let mut out = Vec::new();
        crate::art::geometry(
            &graphic.geometry,
            &xf,
            graphic.width as f64,
            graphic.filled,
            crate::art::Function::Other,
            &crate::art::Attributes::default(),
            &mut out,
        );
        for f in out {
            match f {
                Feature::Line { a, b, .. } => points.extend([a, b]),
                Feature::Arc {
                    start, end, center, ..
                } => {
                    let r = center.dist(start);
                    points.extend([
                        start,
                        end,
                        center.add(V::new(r, r)),
                        center.sub(V::new(r, r)),
                    ]);
                }
                Feature::Region { outline, .. } => points.extend(outline),
                Feature::Flash { shape, .. } => {
                    let s = shape.bbox();
                    points.extend([V::new(s.min.x, s.min.y), V::new(s.max.x, s.max.y)]);
                }
            }
        }
    }
    if !courtyard || points.is_empty() {
        for pad in &fp.pads {
            let s = ew_pcb::items::pad_shape(pad, &xf, 0.0).bbox();
            points.extend([V::new(s.min.x, s.min.y), V::new(s.max.x, s.max.y)]);
        }
    }
    let mut b = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for p in points {
        b = [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)];
    }
    if b[0] > b[2] {
        let at = V::of(fp.at);
        [at.x - MM, at.y - MM, at.x + MM, at.y + MM]
    } else {
        b
    }
}

/// A document of pages.
#[must_use]
pub fn document(title: &str, pages: Vec<Page>, version: &str, created: &str) -> Doc {
    Doc {
        title: title.to_owned(),
        version: version.to_owned(),
        created: created.to_owned(),
        pages,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_sizes() {
        assert_eq!(Paper::A4.landscape(), Some((297.0 * MM, 210.0 * MM)));
        assert!(Paper::Fit.landscape().is_none());
        assert_eq!(schematic_color("sch.wires"), Color::hex("#008400"));
        assert_eq!(board_color(Layer::Outline), Color::BLACK);
    }
}
