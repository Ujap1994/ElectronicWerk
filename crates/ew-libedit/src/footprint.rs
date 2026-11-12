//! Editing a footprint: pads (one by one, as arrays, many at once),
//! graphics on any layer, texts, 3D models and the courtyard.

use ew_model::id::Id;
use ew_model::layer::{Layer, LayerSpec, Side};
use ew_model::library::FootprintDef;
use ew_model::pcb::{Drill, FootprintText, Graphic, Model3d, Pad, PadKind, PadShape};
use ew_model::shape::Geometry;
use ew_model::units::{Angle, Nm, Point, Size};
use serde::Deserialize;

use crate::items::{
    ItemKind, ItemRef, Orientation, center_of, geometry_points, next_number, remove_indices,
    transform_geometry,
};

/// Fields to set on several pads at once (missing fields stay).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PadPatch {
    /// Number.
    pub number: Option<String>,
    /// Kind.
    pub kind: Option<PadKind>,
    /// Shape.
    pub shape: Option<PadShape>,
    /// Size.
    pub size: Option<Size>,
    /// Drill (`null` inside `Some` removes it).
    pub drill: Option<Option<Drill>>,
    /// Layers.
    pub layers: Option<Vec<LayerSpec>>,
    /// Rotation.
    pub rotation: Option<Angle>,
    /// Position.
    pub at: Option<Point>,
    /// Solder mask margin.
    pub mask_margin: Option<Option<Nm>>,
    /// Paste margin.
    pub paste_margin: Option<Option<Nm>>,
}

/// One footprint edit.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", tag = "op")]
pub enum FootprintEdit {
    /// Moves items.
    Move {
        /// Items.
        items: Vec<ItemRef>,
        /// Offset.
        delta: Point,
    },
    /// Copies items, moved by `offset` (pads get the next numbers).
    Duplicate {
        /// Items.
        items: Vec<ItemRef>,
        /// Offset of the copies.
        offset: Point,
    },
    /// Deletes items.
    Delete {
        /// Items.
        items: Vec<ItemRef>,
    },
    /// Turns or mirrors items about `center` (default: their centre).
    Orient {
        /// Items.
        items: Vec<ItemRef>,
        /// How.
        orientation: Orientation,
        /// Centre.
        #[serde(default)]
        center: Option<Point>,
    },
    /// Adds a pad; an empty number gets the next free one.
    AddPad {
        /// The pad (its id is replaced).
        pad: Pad,
    },
    /// Adds a grid of pads, numbered row by row from `start`.
    AddPadArray {
        /// The first pad.
        pad: Pad,
        /// Columns.
        columns: u32,
        /// Rows.
        rows: u32,
        /// Offset between columns.
        pitch_x: Nm,
        /// Offset between rows (positive: rows go down).
        pitch_y: Nm,
        /// First number.
        start: String,
    },
    /// Adds a graphic.
    AddGraphic {
        /// The graphic.
        graphic: Graphic,
    },
    /// Adds a text.
    AddText {
        /// The text.
        text: FootprintText,
    },
    /// Replaces a pad.
    UpdatePad {
        /// Index.
        index: usize,
        /// The new pad.
        pad: Pad,
    },
    /// Replaces a graphic.
    UpdateGraphic {
        /// Index.
        index: usize,
        /// The new graphic.
        graphic: Graphic,
    },
    /// Replaces a text.
    UpdateText {
        /// Index.
        index: usize,
        /// The new text.
        text: FootprintText,
    },
    /// Sets fields of several pads (the pad table's bulk edit).
    SetPads {
        /// Pad indices.
        indices: Vec<usize>,
        /// Fields.
        patch: PadPatch,
    },
    /// Numbers pads one after the other, in the given order.
    Renumber {
        /// Pad indices, in numbering order.
        indices: Vec<usize>,
        /// First number.
        start: String,
        /// Step.
        #[serde(default = "one")]
        step: i64,
    },
    /// Adds a 3D model.
    AddModel {
        /// The model.
        model: Model3d,
    },
    /// Replaces a 3D model (path, offset, rotation, scale).
    UpdateModel {
        /// Index.
        index: usize,
        /// The new model.
        model: Model3d,
    },
    /// Replaces the courtyard by a rectangle around the pads and the
    /// fabrication outline, `margin` away.
    Courtyard {
        /// Distance.
        margin: Nm,
    },
}

const fn one() -> i64 {
    1
}

/// Why an edit is not possible.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct FootprintError(pub String);

fn invalid(message: impl Into<String>) -> FootprintError {
    FootprintError(message.into())
}

/// The result of an edit.
#[derive(Debug, Clone, PartialEq)]
pub struct Edited {
    /// The new footprint.
    pub def: FootprintDef,
    /// The items to select.
    pub select: Vec<ItemRef>,
}

fn indices(items: &[ItemRef], kind: ItemKind) -> Vec<usize> {
    let mut out: Vec<usize> = items
        .iter()
        .filter(|i| i.kind == kind)
        .map(|i| i.index)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The smallest free numeric pad number.
fn free_number(def: &FootprintDef) -> String {
    let used: std::collections::BTreeSet<&str> =
        def.pads.iter().map(|p| p.number.as_str()).collect();
    let mut n = 1;
    while used.contains(n.to_string().as_str()) {
        n += 1;
    }
    n.to_string()
}

/// The box of a pad `[min x, min y, max x, max y]` (quarter turns only).
fn pad_box(pad: &Pad) -> [Nm; 4] {
    let quarter = (pad.rotation.millidegrees() / 90_000) % 2 != 0;
    let (w, h) = if quarter {
        (pad.size.h, pad.size.w)
    } else {
        (pad.size.w, pad.size.h)
    };
    [
        pad.at.x - w / 2,
        pad.at.y - h / 2,
        pad.at.x + w / 2,
        pad.at.y + h / 2,
    ]
}

/// Applies an edit.
///
/// # Errors
///
/// Fails for items that do not exist.
#[allow(clippy::too_many_lines, reason = "one arm per edit")]
pub fn apply(def: &FootprintDef, edit: &FootprintEdit, grid: Nm) -> Result<Edited, FootprintError> {
    let mut def = def.clone();
    let mut select = Vec::new();
    match edit {
        FootprintEdit::Move { items, delta } => {
            for i in indices(items, ItemKind::Pad) {
                if let Some(pad) = def.pads.get_mut(i) {
                    pad.at = pad.at + *delta;
                }
            }
            for i in indices(items, ItemKind::Graphic) {
                if let Some(g) = def.graphics.get_mut(i) {
                    transform_geometry(&mut g.geometry, &|p| p + *delta);
                }
            }
            for i in indices(items, ItemKind::Text) {
                if let Some(t) = def.texts.get_mut(i) {
                    t.at = t.at + *delta;
                }
            }
            select.clone_from(items);
        }
        FootprintEdit::Duplicate { items, offset } => {
            for i in indices(items, ItemKind::Pad) {
                if let Some(mut pad) = def.pads.get(i).cloned() {
                    pad.id = Id::random();
                    pad.at = pad.at + *offset;
                    pad.number = free_number(&def);
                    def.pads.push(pad);
                    select.push(ItemRef::new(ItemKind::Pad, def.pads.len() - 1));
                }
            }
            for i in indices(items, ItemKind::Graphic) {
                if let Some(mut g) = def.graphics.get(i).cloned() {
                    transform_geometry(&mut g.geometry, &|p| p + *offset);
                    def.graphics.push(g);
                    select.push(ItemRef::new(ItemKind::Graphic, def.graphics.len() - 1));
                }
            }
            for i in indices(items, ItemKind::Text) {
                if let Some(mut t) = def.texts.get(i).cloned() {
                    t.at = t.at + *offset;
                    def.texts.push(t);
                    select.push(ItemRef::new(ItemKind::Text, def.texts.len() - 1));
                }
            }
        }
        FootprintEdit::Delete { items } => {
            remove_indices(&mut def.pads, &indices(items, ItemKind::Pad));
            remove_indices(&mut def.graphics, &indices(items, ItemKind::Graphic));
            remove_indices(&mut def.texts, &indices(items, ItemKind::Text));
            remove_indices(&mut def.models, &indices(items, ItemKind::Model));
        }
        FootprintEdit::Orient {
            items,
            orientation,
            center,
        } => {
            let (pads, graphics, texts) = (
                indices(items, ItemKind::Pad),
                indices(items, ItemKind::Graphic),
                indices(items, ItemKind::Text),
            );
            let center = center.unwrap_or_else(|| {
                let mut points: Vec<Point> = pads
                    .iter()
                    .filter_map(|&i| def.pads.get(i))
                    .map(|p| p.at)
                    .collect();
                points.extend(
                    graphics
                        .iter()
                        .filter_map(|&i| def.graphics.get(i))
                        .flat_map(|g| geometry_points(&g.geometry)),
                );
                points.extend(texts.iter().filter_map(|&i| def.texts.get(i)).map(|t| t.at));
                center_of(&points, grid)
            });
            for i in &pads {
                if let Some(pad) = def.pads.get_mut(*i) {
                    pad.at = orientation.point(pad.at, center);
                    pad.rotation = orientation.angle(pad.rotation);
                    if let Some(drill) = &mut pad.drill {
                        if orientation.mirrors() {
                            drill.offset = orientation.vector(drill.offset);
                        }
                    }
                    if let PadShape::Custom { points } = &mut pad.shape {
                        if orientation.mirrors() {
                            for p in points.iter_mut() {
                                *p = orientation.vector(*p);
                            }
                        }
                    }
                }
            }
            for i in &graphics {
                if let Some(g) = def.graphics.get_mut(*i) {
                    transform_geometry(&mut g.geometry, &|p| orientation.point(p, center));
                }
            }
            for i in &texts {
                if let Some(t) = def.texts.get_mut(*i) {
                    t.at = orientation.point(t.at, center);
                    if !orientation.mirrors() {
                        t.rotation = orientation.angle(t.rotation);
                    }
                }
            }
            select.clone_from(items);
        }
        FootprintEdit::AddPad { pad } => {
            let mut pad = pad.clone();
            pad.id = Id::random();
            if pad.number.trim().is_empty() && pad.kind != PadKind::NpThroughHole {
                pad.number = free_number(&def);
            }
            def.pads.push(pad);
            select.push(ItemRef::new(ItemKind::Pad, def.pads.len() - 1));
        }
        FootprintEdit::AddPadArray {
            pad,
            columns,
            rows,
            pitch_x,
            pitch_y,
            start,
        } => {
            if *columns == 0 || *rows == 0 || columns * rows > 10_000 {
                return Err(invalid("an array needs 1 to 10,000 pads"));
            }
            let mut number = if start.trim().is_empty() {
                free_number(&def)
            } else {
                start.clone()
            };
            for r in 0..*rows {
                for c in 0..*columns {
                    let mut copy = pad.clone();
                    copy.id = Id::random();
                    copy.at = Point::new(
                        pad.at.x + i64::from(c) * pitch_x,
                        pad.at.y - i64::from(r) * pitch_y,
                    );
                    copy.number.clone_from(&number);
                    number = next_number(&number, 1);
                    def.pads.push(copy);
                    select.push(ItemRef::new(ItemKind::Pad, def.pads.len() - 1));
                }
            }
        }
        FootprintEdit::AddGraphic { graphic } => {
            def.graphics.push(graphic.clone());
            select.push(ItemRef::new(ItemKind::Graphic, def.graphics.len() - 1));
        }
        FootprintEdit::AddText { text } => {
            def.texts.push(text.clone());
            select.push(ItemRef::new(ItemKind::Text, def.texts.len() - 1));
        }
        FootprintEdit::UpdatePad { index, pad } => {
            let slot = def
                .pads
                .get_mut(*index)
                .ok_or_else(|| invalid("no such pad"))?;
            let id = slot.id;
            *slot = Pad { id, ..pad.clone() };
            select.push(ItemRef::new(ItemKind::Pad, *index));
        }
        FootprintEdit::UpdateGraphic { index, graphic } => {
            *def.graphics
                .get_mut(*index)
                .ok_or_else(|| invalid("no such graphic"))? = graphic.clone();
            select.push(ItemRef::new(ItemKind::Graphic, *index));
        }
        FootprintEdit::UpdateText { index, text } => {
            *def.texts
                .get_mut(*index)
                .ok_or_else(|| invalid("no such text"))? = text.clone();
            select.push(ItemRef::new(ItemKind::Text, *index));
        }
        FootprintEdit::SetPads {
            indices: list,
            patch,
        } => {
            for &i in list {
                let Some(pad) = def.pads.get_mut(i) else {
                    continue;
                };
                if let Some(v) = &patch.number {
                    pad.number.clone_from(v);
                }
                if let Some(v) = patch.kind {
                    pad.kind = v;
                    // Keep the layers and the drill consistent with the kind.
                    match v {
                        PadKind::ThroughHole | PadKind::NpThroughHole => {
                            pad.layers = vec![LayerSpec::AllCopper, LayerSpec::AllMask];
                            if pad.drill.is_none() {
                                let diameter = (pad.size.w.min(pad.size.h) / 2).max(300_000);
                                pad.drill = Some(Drill {
                                    diameter,
                                    slot_length: None,
                                    offset: Point::ORIGIN,
                                });
                            }
                        }
                        PadKind::Smd | PadKind::Connector => {
                            pad.drill = None;
                            pad.layers = vec![
                                LayerSpec::Layer(Layer::Copper(ew_model::layer::CopperLayer::Top)),
                                LayerSpec::Layer(Layer::SolderMask(Side::Top)),
                            ];
                            if v == PadKind::Smd {
                                pad.layers
                                    .insert(1, LayerSpec::Layer(Layer::Paste(Side::Top)));
                            }
                        }
                    }
                }
                if let Some(v) = &patch.shape {
                    pad.shape = v.clone();
                }
                if let Some(v) = patch.size {
                    pad.size = Size::new(v.w.max(1), v.h.max(1));
                }
                if let Some(v) = patch.drill {
                    pad.drill = v;
                }
                if let Some(v) = &patch.layers {
                    pad.layers.clone_from(v);
                }
                if let Some(v) = patch.rotation {
                    pad.rotation = v;
                }
                if let Some(v) = patch.at {
                    pad.at = v;
                }
                if let Some(v) = patch.mask_margin {
                    pad.mask_margin = v;
                }
                if let Some(v) = patch.paste_margin {
                    pad.paste_margin = v;
                }
                select.push(ItemRef::new(ItemKind::Pad, i));
            }
        }
        FootprintEdit::Renumber {
            indices: list,
            start,
            step,
        } => {
            let mut number = start.clone();
            for &i in list {
                if let Some(pad) = def.pads.get_mut(i) {
                    pad.number.clone_from(&number);
                    number = next_number(&number, *step);
                    select.push(ItemRef::new(ItemKind::Pad, i));
                }
            }
        }
        FootprintEdit::AddModel { model } => {
            def.models.push(model.clone());
            select.push(ItemRef::new(ItemKind::Model, def.models.len() - 1));
        }
        FootprintEdit::UpdateModel { index, model } => {
            *def.models
                .get_mut(*index)
                .ok_or_else(|| invalid("no such 3D model"))? = model.clone();
            select.push(ItemRef::new(ItemKind::Model, *index));
        }
        FootprintEdit::Courtyard { margin } => {
            let mut b: Option<[Nm; 4]> = None;
            let mut include = |x: [Nm; 4]| {
                b = Some(b.map_or(x, |c| {
                    [
                        c[0].min(x[0]),
                        c[1].min(x[1]),
                        c[2].max(x[2]),
                        c[3].max(x[3]),
                    ]
                }));
            };
            for pad in &def.pads {
                include(pad_box(pad));
            }
            for g in def
                .graphics
                .iter()
                .filter(|g| matches!(g.layer, Layer::Fabrication(_)))
            {
                for p in geometry_points(&g.geometry) {
                    include([p.x, p.y, p.x, p.y]);
                }
            }
            let Some(b) = b else {
                return Err(invalid(
                    "the footprint has no pads or outline to put a courtyard around",
                ));
            };
            // Outwards to 0.01 mm.
            let step = 10_000;
            let down = |v: Nm| v.div_euclid(step) * step;
            let up = |v: Nm| -((-v).div_euclid(step) * step);
            let (x0, y0, x1, y1) = (
                down(b[0] - margin),
                down(b[1] - margin),
                up(b[2] + margin),
                up(b[3] + margin),
            );
            def.graphics
                .retain(|g| !matches!(g.layer, Layer::Courtyard(Side::Top)));
            def.graphics.push(Graphic {
                layer: Layer::Courtyard(Side::Top),
                geometry: Geometry::Rect {
                    start: Point::new(x0, y1),
                    end: Point::new(x1, y0),
                },
                width: 50_000,
                filled: false,
            });
            select.push(ItemRef::new(ItemKind::Graphic, def.graphics.len() - 1));
        }
    }
    Ok(Edited { def, select })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wizard::{generate, presets};

    fn chip() -> FootprintDef {
        let wizard = presets()
            .into_iter()
            .find(|p| p.name == "0603")
            .unwrap()
            .wizard;
        generate(&wizard, "C_0603").unwrap()
    }

    const GRID: Nm = 50_000;

    #[test]
    fn pads_move_turn_and_number() {
        let def = chip();
        let turned = apply(
            &def,
            &FootprintEdit::Orient {
                items: vec![ItemRef::new(ItemKind::Pad, 0)],
                orientation: Orientation::Left,
                center: Some(Point::ORIGIN),
            },
            GRID,
        )
        .unwrap();
        let pad = &turned.def.pads[0];
        assert_eq!(pad.at, Point::new(0, -825_000));
        assert_eq!(pad.rotation, Angle::DEG_90);
        let copy = apply(
            &def,
            &FootprintEdit::Duplicate {
                items: vec![ItemRef::new(ItemKind::Pad, 1)],
                offset: Point::new(0, 2_000_000),
            },
            GRID,
        )
        .unwrap();
        assert_eq!(copy.def.pads[2].number, "3");
        assert_ne!(copy.def.pads[2].id, def.pads[1].id);
    }

    #[test]
    fn pad_arrays_and_bulk_edits() {
        let def = chip();
        let mut pad = def.pads[0].clone();
        pad.at = Point::new(-5_000_000, 5_000_000);
        let grid = apply(
            &def,
            &FootprintEdit::AddPadArray {
                pad,
                columns: 3,
                rows: 2,
                pitch_x: 1_000_000,
                pitch_y: 1_000_000,
                start: "10".to_owned(),
            },
            GRID,
        )
        .unwrap();
        assert_eq!(grid.def.pads.len(), 8);
        assert_eq!(grid.def.pads[7].number, "15");
        assert_eq!(grid.def.pads[7].at, Point::new(-3_000_000, 4_000_000));
        let tht = apply(
            &def,
            &FootprintEdit::SetPads {
                indices: vec![0, 1],
                patch: PadPatch {
                    kind: Some(PadKind::ThroughHole),
                    ..PadPatch::default()
                },
            },
            GRID,
        )
        .unwrap();
        assert!(
            tht.def
                .pads
                .iter()
                .all(|p| p.drill.is_some() && p.layers.contains(&LayerSpec::AllCopper))
        );
        let renumbered = apply(
            &def,
            &FootprintEdit::Renumber {
                indices: vec![1, 0],
                start: "A".to_owned(),
                step: 1,
            },
            GRID,
        )
        .unwrap();
        assert_eq!(
            (
                renumbered.def.pads[1].number.as_str(),
                renumbered.def.pads[0].number.as_str()
            ),
            ("A", "A1")
        );
    }

    #[test]
    fn courtyard_is_drawn_around_pads() {
        let mut def = chip();
        def.graphics
            .retain(|g| !matches!(g.layer, Layer::Courtyard(_)));
        let edited = apply(&def, &FootprintEdit::Courtyard { margin: 250_000 }, GRID).unwrap();
        let courtyard = edited
            .def
            .graphics
            .iter()
            .find(|g| matches!(g.layer, Layer::Courtyard(_)))
            .unwrap();
        let Geometry::Rect { start, end } = courtyard.geometry else {
            unreachable!()
        };
        // Pads reach ±1.15 mm (0.825 + 0.325); plus 0.25 mm.
        assert_eq!((start.x, end.x), (-1_400_000, 1_400_000));
        assert!(
            crate::check::check_footprint(&edited.def)
                .iter()
                .all(|f| f.code != "no-courtyard")
        );
    }
}
