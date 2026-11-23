//! Panels: several copies of a board on one sheet, with rails, tabs with
//! mouse bites or V-score lines, fiducials and tooling holes. The panel is
//! an ordinary board in memory, so every board output (Gerber, drill, pick
//! and place) can be made for it.

use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, LayerSpec, Side};
use ew_model::outputs::{PanelSettings, Rails, Separation};
use ew_model::pcb::{
    BoardShape, FootprintAttributes, FootprintInstance, Mounting, Pad, PadKind, PadShape, Pcb,
};
use ew_model::shape::Geometry;
use ew_model::units::{Angle, Nm, Point, Size};

use crate::geom::{BBox, V};

/// Layer of the V-score lines (Gerber file `User_1`).
pub const VSCORE_LAYER: Layer = Layer::User(1);
/// Mouse bite holes.
const BITE_DRILL: Nm = 500_000;
const BITE_PITCH: Nm = 800_000;
/// Fiducial copper dot and its solder mask opening.
const FIDUCIAL: Nm = 1_000_000;
const FIDUCIAL_MASK: Nm = 500_000;
/// Tooling holes.
const TOOLING_DRILL: Nm = 2_000_000;
/// Distance of fiducials and tooling holes from the panel corners.
const CORNER: Nm = 5_000_000;
/// Line width of the outline and V-score lines.
const LINE: Nm = 100_000;

/// Why a board cannot be panelized.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PanelError {
    /// The board has no closed outline.
    #[error("the board has no closed outline")]
    NoOutline,
    /// V-score needs a rectangular board.
    #[error("V-score needs a rectangular board; use mouse bites for this outline")]
    NotRectangular,
    /// Settings out of range.
    #[error("{0}")]
    Invalid(String),
}

impl PanelError {
    /// Stable code of the problem (for translated messages).
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::NoOutline => "noOutline",
            Self::NotRectangular => "notRectangular",
            Self::Invalid(_) => "invalid",
        }
    }
}

/// Something to know about a panel that was made anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
pub enum PanelWarning {
    /// Tooling holes were asked for, but the rails are missing or too narrow.
    #[error("tooling holes need rails at least 3 mm wide")]
    ToolingHolesNeedRails,
    /// Fiducials were asked for, but the rails are missing or too narrow.
    #[error("fiducials need rails at least 3 mm wide")]
    FiducialsNeedRails,
    /// The board is not a rectangle: tabs sit on its bounding box.
    #[error("the tabs are placed on the board's bounding box")]
    TabsOnBoundingBox,
}

impl PanelWarning {
    /// Stable code of the warning (for translated messages).
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::ToolingHolesNeedRails => "toolingHolesNeedRails",
            Self::FiducialsNeedRails => "fiducialsNeedRails",
            Self::TabsOnBoundingBox => "tabsOnBoundingBox",
        }
    }
}

/// A panel and what to know about it.
#[derive(Debug, Clone)]
pub struct Panel {
    /// The panel as a board.
    pub pcb: Pcb,
    /// Width and height.
    pub size: (Nm, Nm),
    /// Hints (for example fiducials without rails).
    pub warnings: Vec<PanelWarning>,
}

#[allow(clippy::cast_possible_truncation, reason = "board coordinates")]
fn nm(value: f64) -> Nm {
    value.round() as Nm
}

#[allow(clippy::cast_precision_loss, reason = "board coordinates")]
const fn fl(value: Nm) -> f64 {
    value as f64
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<V> {
    vec![
        V::new(x0, y0),
        V::new(x1, y0),
        V::new(x1, y1),
        V::new(x0, y1),
    ]
}

fn shape(layer: Layer, geometry: Geometry) -> BoardShape {
    BoardShape {
        id: Id::random(),
        layer,
        geometry,
        width: LINE,
        filled: false,
    }
}

fn special(designator: &str, value: &str, at: Point, pad: Pad) -> FootprintInstance {
    FootprintInstance {
        id: Id::random(),
        component: None,
        footprint: value.to_owned(),
        designator: designator.to_owned(),
        value: value.to_owned(),
        at,
        rotation: Angle::ZERO,
        side: Side::Top,
        locked: true,
        pads: vec![pad],
        graphics: Vec::new(),
        texts: Vec::new(),
        models: Vec::new(),
        properties: std::collections::BTreeMap::new(),
        attributes: FootprintAttributes {
            mounting: Mounting::Other,
            exclude_from_bom: true,
            exclude_from_placement: true,
            dnp: false,
        },
    }
}

fn pad(kind: PadKind, size: Nm, drill: Option<Nm>, layers: Vec<LayerSpec>) -> Pad {
    Pad {
        id: Id::random(),
        number: "1".to_owned(),
        net: None,
        kind,
        shape: PadShape::Circle,
        at: Point::ORIGIN,
        rotation: Angle::ZERO,
        size: Size { w: size, h: size },
        drill: drill.map(|diameter| ew_model::pcb::Drill {
            diameter,
            slot_length: None,
            offset: Point::ORIGIN,
        }),
        layers,
        clearance: None,
        mask_margin: None,
        paste_margin: None,
        stack: Vec::new(),
    }
}

fn fiducial(number: usize, at: Point) -> FootprintInstance {
    let mut dot = pad(
        PadKind::Smd,
        FIDUCIAL,
        None,
        vec![
            LayerSpec::Layer(Layer::Copper(CopperLayer::Top)),
            LayerSpec::Layer(Layer::SolderMask(Side::Top)),
        ],
    );
    dot.mask_margin = Some(FIDUCIAL_MASK);
    special(&format!("FID{number}"), "Fiducial", at, dot)
}

fn tooling_hole(number: usize, at: Point) -> FootprintInstance {
    special(
        &format!("TH{number}"),
        "Tooling hole",
        at,
        pad(
            PadKind::NpThroughHole,
            TOOLING_DRILL,
            Some(TOOLING_DRILL),
            vec![LayerSpec::AllCopper, LayerSpec::AllMask],
        ),
    )
}

fn mouse_bite(number: usize, at: Point) -> FootprintInstance {
    special(
        &format!("MB{number}"),
        "Mouse bite",
        at,
        pad(
            PadKind::NpThroughHole,
            BITE_DRILL,
            Some(BITE_DRILL),
            Vec::new(),
        ),
    )
}

/// Holes along a board edge across a tab: `along` x (horizontal edge) or y.
fn bites(from: f64, to: f64, edge: f64, horizontal: bool) -> Vec<Point> {
    let width = to - from;
    let pitch = fl(BITE_PITCH);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a few holes"
    )]
    let count = ((width / pitch).floor() as usize).max(1) + 1;
    let start = from + (width - pitch * fl(i64::try_from(count - 1).unwrap_or(0))) / 2.0;
    (0..count)
        .map(|i| {
            let along = nm(start + pitch * fl(i64::try_from(i).unwrap_or(0)));
            if horizontal {
                Point::new(along, nm(edge))
            } else {
                Point::new(nm(edge), along)
            }
        })
        .collect()
}

/// Builds the panel of a board.
///
/// # Errors
///
/// Fails without a closed outline, for V-score with a non-rectangular
/// board, or for settings out of range.
#[allow(clippy::too_many_lines, reason = "the panel step by step")]
pub fn panelize(pcb: &Pcb, settings: &PanelSettings) -> Result<Panel, PanelError> {
    if !(1..=50).contains(&settings.columns) || !(1..=50).contains(&settings.rows) {
        return Err(PanelError::Invalid(
            "a panel has 1 to 50 columns and rows".to_owned(),
        ));
    }
    if settings.rail_width < 0 || settings.gap < 0 || settings.tab_width <= 0 {
        return Err(PanelError::Invalid("widths must be positive".to_owned()));
    }
    let outline = crate::items::outline(pcb);
    let ring = outline.rings.first().ok_or(PanelError::NoOutline)?;
    let bbox = BBox::of_points(ring);
    let (w, h) = (bbox.max.x - bbox.min.x, bbox.max.y - bbox.min.y);
    let rectangular = (crate::geom::signed_area2(ring).abs() / 2.0 - w * h).abs() < w * h * 0.002;
    let vscore = settings.separation == Separation::VScore;
    if vscore && !rectangular {
        return Err(PanelError::NotRectangular);
    }
    let gap = if vscore { 0.0 } else { fl(settings.gap) };
    let rail = fl(settings.rail_width);
    let (rails_x, rails_y) = match settings.rails {
        Rails::None => (false, false),
        Rails::TopBottom => (false, true),
        Rails::LeftRight => (true, false),
        Rails::All => (true, true),
    };
    let (cols, rows) = (settings.columns, settings.rows);
    let cols_f = fl(i64::from(cols));
    let rows_f = fl(i64::from(rows));
    // Boards start after a rail and its gap.
    let x0 = if rails_x { rail + gap } else { 0.0 };
    let y0 = if rails_y { rail + gap } else { 0.0 };
    let width = 2.0 * x0 + cols_f * w + (cols_f - 1.0) * gap;
    let height = 2.0 * y0 + rows_f * h + (rows_f - 1.0) * gap;

    let mut panel = pcb.clone();
    panel.name = format!("{} panel", pcb.name);
    panel.origin = Point::ORIGIN;
    panel.footprints = ew_model::collection::Collection::default();
    panel.tracks = ew_model::collection::Collection::default();
    panel.vias = ew_model::collection::Collection::default();
    panel.zones = ew_model::collection::Collection::default();
    panel.texts = ew_model::collection::Collection::default();
    panel.shapes = ew_model::collection::Collection::default();
    panel.drc_exclusions.clear();

    // The copies (without their outline: the panel has its own).
    let mut substrate: Vec<Vec<Vec<V>>> = Vec::new();
    let mut cutouts: Vec<Vec<Vec<V>>> = Vec::new();
    let mut origins = Vec::new();
    for row in 0..rows {
        for col in 0..cols {
            let ox = x0 + fl(i64::from(col)) * (w + gap);
            let oy = y0 + fl(i64::from(row)) * (h + gap);
            origins.push((ox, oy));
            let offset = Point::new(nm(ox - bbox.min.x), nm(oy - bbox.min.y));
            let copy = crate::edit::translated(pcb, offset);
            for mut fp in copy.footprints.iter().cloned() {
                fp.id = Id::random();
                for pad in &mut fp.pads {
                    pad.id = Id::random();
                }
                panel.footprints.insert(fp);
            }
            for mut track in copy.tracks.iter().cloned() {
                track.id = Id::random();
                panel.tracks.insert(track);
            }
            for mut via in copy.vias.iter().cloned() {
                via.id = Id::random();
                panel.vias.insert(via);
            }
            for mut zone in copy.zones.iter().cloned() {
                zone.id = Id::random();
                panel.zones.insert(zone);
            }
            for mut text in copy.texts.iter().cloned() {
                text.id = Id::random();
                panel.texts.insert(text);
            }
            for mut shape in copy.shapes.iter().cloned() {
                if shape.layer == Layer::Outline {
                    continue;
                }
                shape.id = Id::random();
                panel.shapes.insert(shape);
            }
            let shift = V::new(ox - bbox.min.x, oy - bbox.min.y);
            let moved = |r: &Vec<V>| {
                r.iter()
                    .map(|p| V::new(p.x + shift.x, p.y + shift.y))
                    .collect()
            };
            substrate.push(vec![moved(ring)]);
            for hole in outline.rings.iter().skip(1) {
                cutouts.push(vec![moved(hole)]);
            }
        }
    }

    // Rails.
    if rails_y {
        substrate.push(vec![rect(0.0, 0.0, width, rail)]);
        substrate.push(vec![rect(0.0, height - rail, width, height)]);
    }
    if rails_x {
        substrate.push(vec![rect(0.0, 0.0, rail, height)]);
        substrate.push(vec![rect(width - rail, 0.0, width, height)]);
    }

    // Tabs across the gaps, with mouse bites on the board edges.
    let mut holes: Vec<Point> = Vec::new();
    if !vscore && gap > 0.0 {
        let tabs = settings.tabs.max(1);
        let tab = fl(settings.tab_width);
        let spots = |length: f64| -> Vec<f64> {
            (1..=tabs)
                .map(|k| length * fl(i64::from(k)) / fl(i64::from(tabs + 1)))
                .collect()
        };
        for &(ox, oy) in &origins {
            // Right edge: to the next board or the right rail.
            let right = ox + w;
            if right + gap < width - 1.0 {
                for at in spots(h) {
                    let (ty0, ty1) = (oy + at - tab / 2.0, oy + at + tab / 2.0);
                    substrate.push(vec![rect(right - 1.0, ty0, right + gap + 1.0, ty1)]);
                    holes.extend(bites(ty0, ty1, right, false));
                    if right + gap + w <= width - x0 + 1.0 {
                        holes.extend(bites(ty0, ty1, right + gap, false));
                    }
                }
            }
            // Left edge next to the left rail.
            if rails_x && (ox - x0).abs() < 1.0 {
                for at in spots(h) {
                    let (ty0, ty1) = (oy + at - tab / 2.0, oy + at + tab / 2.0);
                    substrate.push(vec![rect(ox - gap - 1.0, ty0, ox + 1.0, ty1)]);
                    holes.extend(bites(ty0, ty1, ox, false));
                }
            }
            // Top edge: to the next row or the top rail.
            let top = oy + h;
            if top + gap < height - 1.0 {
                for at in spots(w) {
                    let (tx0, tx1) = (ox + at - tab / 2.0, ox + at + tab / 2.0);
                    substrate.push(vec![rect(tx0, top - 1.0, tx1, top + gap + 1.0)]);
                    holes.extend(bites(tx0, tx1, top, true));
                    if top + gap + h <= height - y0 + 1.0 {
                        holes.extend(bites(tx0, tx1, top + gap, true));
                    }
                }
            }
            // Bottom edge next to the bottom rail.
            if rails_y && (oy - y0).abs() < 1.0 {
                for at in spots(w) {
                    let (tx0, tx1) = (ox + at - tab / 2.0, ox + at + tab / 2.0);
                    substrate.push(vec![rect(tx0, oy - gap - 1.0, tx1, oy + 1.0)]);
                    holes.extend(bites(tx0, tx1, oy, true));
                }
            }
        }
    }

    // The outline: the union of boards, rails and tabs, without cut-outs.
    let mut body = crate::poly::union(&substrate);
    if !cutouts.is_empty() {
        body = crate::poly::difference(&body, &cutouts);
    }
    for polygon in &body {
        for ring in polygon {
            let points: Vec<Point> = ring.iter().map(|p| Point::new(nm(p.x), nm(p.y))).collect();
            if points.len() >= 3 {
                panel
                    .shapes
                    .insert(shape(Layer::Outline, Geometry::Polygon { points }));
            }
        }
    }

    // V-score lines across the whole panel, along every board edge inside it.
    if vscore {
        let mut xs: Vec<f64> = (0..=cols).map(|c| x0 + fl(i64::from(c)) * w).collect();
        let mut ys: Vec<f64> = (0..=rows).map(|r| y0 + fl(i64::from(r)) * h).collect();
        xs.retain(|x| *x > 1.0 && *x < width - 1.0);
        ys.retain(|y| *y > 1.0 && *y < height - 1.0);
        for x in xs {
            panel.shapes.insert(shape(
                VSCORE_LAYER,
                Geometry::Line {
                    start: Point::new(nm(x), 0),
                    end: Point::new(nm(x), nm(height)),
                },
            ));
        }
        for y in ys {
            panel.shapes.insert(shape(
                VSCORE_LAYER,
                Geometry::Line {
                    start: Point::new(0, nm(y)),
                    end: Point::new(nm(width), nm(y)),
                },
            ));
        }
    }

    for (number, at) in holes.into_iter().enumerate() {
        panel.footprints.insert(mouse_bite(number + 1, at));
    }

    // Fiducials and tooling holes on the rails.
    let mut warnings = Vec::new();
    let corners = |inset: f64| -> Vec<Point> {
        if rails_y {
            let (y_low, y_high) = (rail / 2.0, height - rail / 2.0);
            vec![
                Point::new(nm(inset), nm(y_low)),
                Point::new(nm(width - inset), nm(y_low)),
                Point::new(nm(inset), nm(y_high)),
                Point::new(nm(width - inset), nm(y_high)),
            ]
        } else if rails_x {
            let (x_low, x_high) = (rail / 2.0, width - rail / 2.0);
            vec![
                Point::new(nm(x_low), nm(inset)),
                Point::new(nm(x_high), nm(inset)),
                Point::new(nm(x_low), nm(height - inset)),
                Point::new(nm(x_high), nm(height - inset)),
            ]
        } else {
            Vec::new()
        }
    };
    let has_rails = rails_x || rails_y;
    if settings.tooling_holes {
        if has_rails && rail >= fl(TOOLING_DRILL) + 1e6 {
            for (number, at) in corners(fl(CORNER)).into_iter().enumerate() {
                panel.footprints.insert(tooling_hole(number + 1, at));
            }
        } else {
            warnings.push(PanelWarning::ToolingHolesNeedRails);
        }
    }
    if settings.fiducials {
        if has_rails && rail >= fl(FIDUCIAL + 2 * FIDUCIAL_MASK) + 1e6 {
            // Three of the four corners, so the panel's orientation is clear.
            let spots = corners(fl(CORNER) * 2.0);
            for (number, at) in spots.into_iter().take(3).enumerate() {
                panel.footprints.insert(fiducial(number + 1, at));
            }
        } else {
            warnings.push(PanelWarning::FiducialsNeedRails);
        }
    }
    if !vscore && !rectangular {
        warnings.push(PanelWarning::TabsOnBoundingBox);
    }
    Ok(Panel {
        pcb: panel,
        size: (nm(width), nm(height)),
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::outline;

    fn board() -> Pcb {
        let project = ew_synth::generate(ew_synth::DesignSize::S, ew_synth::DEFAULT_SEED);
        project
            .documents
            .values()
            .find_map(|d| match d {
                ew_model::document::Document::Pcb(pcb) => Some(pcb.clone()),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn mouse_bites_with_rails_fiducials_and_tooling_holes() {
        let pcb = board();
        let settings = PanelSettings::default();
        let panel = panelize(&pcb, &settings).unwrap();
        // 80 × 60 mm boards, 2 × 2, 2 mm gaps, 5 mm rails top and bottom.
        assert_eq!(panel.size, (162_000_000, 136_000_000));
        let copies = pcb.footprints.len() * 4;
        let count = |prefix: &str| {
            panel
                .pcb
                .footprints
                .iter()
                .filter(|fp| {
                    fp.designator.starts_with(prefix) && fp.value != "Mouse bite"
                        || prefix == "MB" && fp.value == "Mouse bite"
                })
                .count()
        };
        assert_eq!(count("FID"), 3);
        assert_eq!(count("TH"), 4);
        assert!(count("MB") > 20);
        let parts = panel
            .pcb
            .footprints
            .iter()
            .filter(|fp| fp.attributes.mounting != Mounting::Other)
            .count();
        assert_eq!(parts, copies);
        assert_eq!(panel.pcb.tracks.len(), pcb.tracks.len() * 4);
        assert_eq!(panel.pcb.vias.len(), pcb.vias.len() * 4);
        // One outline: the panel with slots between the boards.
        let rings = outline(&panel.pcb).rings;
        assert!(rings.len() > 1, "the milled slots are holes in the panel");
        let outer = BBox::of_points(&rings[0]);
        assert!((outer.max.x - outer.min.x - 162e6).abs() < 1.0);
        assert!(panel.warnings.is_empty(), "{:?}", panel.warnings);
    }

    #[test]
    fn v_score_without_rails() {
        let pcb = board();
        let panel = panelize(
            &pcb,
            &PanelSettings {
                columns: 3,
                rows: 1,
                separation: Separation::VScore,
                rails: Rails::None,
                fiducials: false,
                tooling_holes: false,
                ..PanelSettings::default()
            },
        )
        .unwrap();
        assert_eq!(panel.size, (240_000_000, 60_000_000));
        let lines = panel
            .pcb
            .shapes
            .iter()
            .filter(|s| s.layer == VSCORE_LAYER)
            .count();
        assert_eq!(lines, 2, "between the three boards");
        let rings = outline(&panel.pcb).rings;
        assert_eq!(rings.len(), 1, "the boards touch: one rectangle");
        assert!(
            panel
                .pcb
                .footprints
                .iter()
                .all(|fp| fp.value != "Mouse bite")
        );
        assert_eq!(
            panelize(
                &pcb,
                &PanelSettings {
                    columns: 0,
                    ..PanelSettings::default()
                }
            )
            .unwrap_err(),
            PanelError::Invalid("a panel has 1 to 50 columns and rows".to_owned())
        );
    }
}
