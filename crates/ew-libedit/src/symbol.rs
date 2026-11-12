//! Editing a symbol: pins (one by one or many at once), graphics, texts,
//! units and alternate bodies. An edit works on one body (a unit, normal or
//! alternate) and returns the new symbol and the items to select.

use ew_model::library::{
    Pin, PinDirection, PinKind, PinShape, SymbolBody, SymbolDef, SymbolGraphic, SymbolText,
    SymbolUnit,
};
use ew_model::units::{Nm, Point};
use serde::Deserialize;

use crate::items::{
    ItemKind, ItemRef, Orientation, center_of, geometry_points, next_number, remove_indices,
    transform_geometry,
};

/// The body an edit works on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Body {
    /// Unit (1-based).
    pub unit: u16,
    /// The alternate body.
    pub alternate: bool,
}

/// Fields to set on several pins at once (missing fields stay).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PinPatch {
    /// Name.
    pub name: Option<String>,
    /// Number.
    pub number: Option<String>,
    /// Electrical type.
    pub kind: Option<PinKind>,
    /// Graphic style.
    pub shape: Option<PinShape>,
    /// Direction towards the body.
    pub direction: Option<PinDirection>,
    /// Length.
    pub length: Option<Nm>,
    /// Hidden.
    pub hidden: Option<bool>,
    /// Position.
    pub at: Option<Point>,
}

/// One symbol edit.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "op")]
pub enum SymbolEdit {
    /// Moves items.
    Move {
        /// Items.
        items: Vec<ItemRef>,
        /// Offset.
        delta: Point,
    },
    /// Copies items, moved by `offset`.
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
    /// Turns or mirrors items about `center` (default: their centre on the grid).
    Orient {
        /// Items.
        items: Vec<ItemRef>,
        /// How.
        orientation: Orientation,
        /// Centre.
        #[serde(default)]
        center: Option<Point>,
    },
    /// Adds a pin; an empty number gets the next free one.
    AddPin {
        /// The pin.
        pin: Pin,
    },
    /// Adds several pins at once (a pasted pin list); empty numbers get
    /// the next free ones.
    AddPins {
        /// The pins.
        pins: Vec<Pin>,
    },
    /// Adds a graphic.
    AddGraphic {
        /// The graphic.
        graphic: SymbolGraphic,
    },
    /// Adds a text.
    AddText {
        /// The text.
        text: SymbolText,
    },
    /// Replaces a pin.
    UpdatePin {
        /// Index.
        index: usize,
        /// The new pin.
        pin: Pin,
    },
    /// Replaces a graphic.
    UpdateGraphic {
        /// Index.
        index: usize,
        /// The new graphic.
        graphic: SymbolGraphic,
    },
    /// Replaces a text.
    UpdateText {
        /// Index.
        index: usize,
        /// The new text.
        text: SymbolText,
    },
    /// Sets fields of several pins (the pin table's bulk edit).
    SetPins {
        /// Pin indices.
        indices: Vec<usize>,
        /// Fields.
        patch: PinPatch,
    },
    /// Numbers pins one after the other (`start`, then by `step`), in the
    /// given order.
    Renumber {
        /// Pin indices, in numbering order.
        indices: Vec<usize>,
        /// First number (`1`, `A1`).
        start: String,
        /// Step.
        #[serde(default = "one")]
        step: i64,
    },
    /// Adds a unit: a copy of the current one or an empty one.
    AddUnit {
        /// Copy the current unit (pins get new numbers).
        #[serde(default)]
        copy: bool,
    },
    /// Removes the current unit (not the last one).
    RemoveUnit,
    /// Names the current unit.
    NameUnit {
        /// Name (`A`, `Power`).
        name: String,
    },
    /// Adds an alternate body to the current unit (a copy of the normal one).
    AddAlternate,
    /// Removes the alternate body of the current unit.
    RemoveAlternate,
}

const fn one() -> i64 {
    1
}

/// Why an edit is not possible.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct SymbolError(pub String);

fn invalid(message: impl Into<String>) -> SymbolError {
    SymbolError(message.into())
}

/// The result of an edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edited {
    /// The new symbol.
    pub def: SymbolDef,
    /// The body to show afterwards (a new unit, for example).
    pub body: Body,
    /// The items to select.
    pub select: Vec<ItemRef>,
}

/// The editable lists of one body.
struct Lists<'a> {
    pins: &'a mut Vec<Pin>,
    graphics: &'a mut Vec<SymbolGraphic>,
    texts: &'a mut Vec<SymbolText>,
}

fn lists(def: &mut SymbolDef, body: Body) -> Result<Lists<'_>, SymbolError> {
    let index = usize::from(body.unit.max(1) - 1);
    let unit = def
        .units
        .get_mut(index)
        .ok_or_else(|| invalid(format!("the symbol has no unit {}", body.unit)))?;
    if body.alternate {
        let alternate = unit
            .alternate
            .as_mut()
            .ok_or_else(|| invalid(format!("unit {} has no alternate body", body.unit)))?;
        Ok(Lists {
            pins: &mut alternate.pins,
            graphics: &mut alternate.graphics,
            texts: &mut alternate.texts,
        })
    } else {
        Ok(Lists {
            pins: &mut unit.pins,
            graphics: &mut unit.graphics,
            texts: &mut unit.texts,
        })
    }
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

const fn turn_direction(direction: PinDirection, orientation: Orientation) -> PinDirection {
    use PinDirection::{Down, Left, Right, Up};
    match (orientation, direction) {
        (Orientation::Left, Right) | (Orientation::Right, Left) | (Orientation::Vertical, Down) => {
            Up
        }
        (Orientation::Left, Up) | (Orientation::Right, Down) | (Orientation::Horizontal, Right) => {
            Left
        }
        (Orientation::Left, Left) | (Orientation::Right, Right) | (Orientation::Vertical, Up) => {
            Down
        }
        (Orientation::Left, Down) | (Orientation::Right, Up) | (Orientation::Horizontal, Left) => {
            Right
        }
        (Orientation::Horizontal | Orientation::Vertical, d) => d,
    }
}

/// Every pin number of the symbol (all units and bodies).
fn used_numbers(def: &SymbolDef) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    for unit in &def.units {
        out.extend(unit.pins.iter().map(|p| p.number.clone()));
        if let Some(alternate) = &unit.alternate {
            out.extend(alternate.pins.iter().map(|p| p.number.clone()));
        }
    }
    out
}

/// The smallest free numeric pin number.
fn free_number(def: &SymbolDef) -> String {
    let used = used_numbers(def);
    let mut n = 1;
    while used.contains(&n.to_string()) {
        n += 1;
    }
    n.to_string()
}

/// Applies an edit.
///
/// # Errors
///
/// Fails for a unit or body that does not exist, and for removing the last
/// unit.
#[allow(clippy::too_many_lines, reason = "one arm per edit")]
pub fn apply(
    def: &SymbolDef,
    body: Body,
    edit: &SymbolEdit,
    grid: Nm,
) -> Result<Edited, SymbolError> {
    let mut def = def.clone();
    let mut body = Body {
        unit: body.unit.max(1),
        ..body
    };
    let mut select = Vec::new();
    match edit {
        SymbolEdit::Move { items, delta } => {
            let l = lists(&mut def, body)?;
            for i in indices(items, ItemKind::Pin) {
                if let Some(pin) = l.pins.get_mut(i) {
                    pin.at = pin.at + *delta;
                }
            }
            for i in indices(items, ItemKind::Graphic) {
                if let Some(g) = l.graphics.get_mut(i) {
                    transform_geometry(&mut g.geometry, &|p| p + *delta);
                }
            }
            for i in indices(items, ItemKind::Text) {
                if let Some(t) = l.texts.get_mut(i) {
                    t.at = t.at + *delta;
                }
            }
            select.clone_from(items);
        }
        SymbolEdit::Duplicate { items, offset } => {
            let numbers = used_numbers(&def);
            let mut next = free_number(&def);
            let l = lists(&mut def, body)?;
            for i in indices(items, ItemKind::Pin) {
                if let Some(mut pin) = l.pins.get(i).cloned() {
                    pin.at = pin.at + *offset;
                    while numbers.contains(&next) {
                        next = next_number(&next, 1);
                    }
                    pin.number.clone_from(&next);
                    next = next_number(&next, 1);
                    l.pins.push(pin);
                    select.push(ItemRef::new(ItemKind::Pin, l.pins.len() - 1));
                }
            }
            for i in indices(items, ItemKind::Graphic) {
                if let Some(mut g) = l.graphics.get(i).cloned() {
                    transform_geometry(&mut g.geometry, &|p| p + *offset);
                    l.graphics.push(g);
                    select.push(ItemRef::new(ItemKind::Graphic, l.graphics.len() - 1));
                }
            }
            for i in indices(items, ItemKind::Text) {
                if let Some(mut t) = l.texts.get(i).cloned() {
                    t.at = t.at + *offset;
                    l.texts.push(t);
                    select.push(ItemRef::new(ItemKind::Text, l.texts.len() - 1));
                }
            }
        }
        SymbolEdit::Delete { items } => {
            let l = lists(&mut def, body)?;
            remove_indices(l.pins, &indices(items, ItemKind::Pin));
            remove_indices(l.graphics, &indices(items, ItemKind::Graphic));
            remove_indices(l.texts, &indices(items, ItemKind::Text));
        }
        SymbolEdit::Orient {
            items,
            orientation,
            center,
        } => {
            let l = lists(&mut def, body)?;
            let (pins, graphics, texts) = (
                indices(items, ItemKind::Pin),
                indices(items, ItemKind::Graphic),
                indices(items, ItemKind::Text),
            );
            let center = center.unwrap_or_else(|| {
                let mut points: Vec<Point> = pins
                    .iter()
                    .filter_map(|&i| l.pins.get(i))
                    .map(|p| p.at)
                    .collect();
                points.extend(
                    graphics
                        .iter()
                        .filter_map(|&i| l.graphics.get(i))
                        .flat_map(|g| geometry_points(&g.geometry)),
                );
                points.extend(texts.iter().filter_map(|&i| l.texts.get(i)).map(|t| t.at));
                center_of(&points, grid)
            });
            for i in &pins {
                if let Some(pin) = l.pins.get_mut(*i) {
                    pin.at = orientation.point(pin.at, center);
                    pin.direction = turn_direction(pin.direction, *orientation);
                }
            }
            for i in &graphics {
                if let Some(g) = l.graphics.get_mut(*i) {
                    transform_geometry(&mut g.geometry, &|p| orientation.point(p, center));
                }
            }
            for i in &texts {
                if let Some(t) = l.texts.get_mut(*i) {
                    t.at = orientation.point(t.at, center);
                    if !orientation.mirrors() {
                        t.rotation = orientation.angle(t.rotation);
                    }
                }
            }
            select.clone_from(items);
        }
        SymbolEdit::AddPin { pin } => {
            let mut pin = pin.clone();
            if pin.number.trim().is_empty() {
                pin.number = free_number(&def);
            }
            let l = lists(&mut def, body)?;
            l.pins.push(pin);
            select.push(ItemRef::new(ItemKind::Pin, l.pins.len() - 1));
        }
        SymbolEdit::AddPins { pins } => {
            let mut used = used_numbers(&def);
            let mut next = free_number(&def);
            let l = lists(&mut def, body)?;
            for pin in pins {
                let mut pin = pin.clone();
                if pin.number.trim().is_empty() {
                    while used.contains(&next) {
                        next = next_number(&next, 1);
                    }
                    pin.number.clone_from(&next);
                }
                used.insert(pin.number.clone());
                l.pins.push(pin);
                select.push(ItemRef::new(ItemKind::Pin, l.pins.len() - 1));
            }
        }
        SymbolEdit::AddGraphic { graphic } => {
            let l = lists(&mut def, body)?;
            l.graphics.push(graphic.clone());
            select.push(ItemRef::new(ItemKind::Graphic, l.graphics.len() - 1));
        }
        SymbolEdit::AddText { text } => {
            let l = lists(&mut def, body)?;
            l.texts.push(text.clone());
            select.push(ItemRef::new(ItemKind::Text, l.texts.len() - 1));
        }
        SymbolEdit::UpdatePin { index, pin } => {
            let l = lists(&mut def, body)?;
            *l.pins
                .get_mut(*index)
                .ok_or_else(|| invalid("no such pin"))? = pin.clone();
            select.push(ItemRef::new(ItemKind::Pin, *index));
        }
        SymbolEdit::UpdateGraphic { index, graphic } => {
            let l = lists(&mut def, body)?;
            *l.graphics
                .get_mut(*index)
                .ok_or_else(|| invalid("no such graphic"))? = graphic.clone();
            select.push(ItemRef::new(ItemKind::Graphic, *index));
        }
        SymbolEdit::UpdateText { index, text } => {
            let l = lists(&mut def, body)?;
            *l.texts
                .get_mut(*index)
                .ok_or_else(|| invalid("no such text"))? = text.clone();
            select.push(ItemRef::new(ItemKind::Text, *index));
        }
        SymbolEdit::SetPins {
            indices: list,
            patch,
        } => {
            let l = lists(&mut def, body)?;
            for &i in list {
                let Some(pin) = l.pins.get_mut(i) else {
                    continue;
                };
                if let Some(v) = &patch.name {
                    pin.name.clone_from(v);
                }
                if let Some(v) = &patch.number {
                    pin.number.clone_from(v);
                }
                if let Some(v) = patch.kind {
                    pin.kind = v;
                }
                if let Some(v) = patch.shape {
                    pin.shape = v;
                }
                if let Some(v) = patch.direction {
                    pin.direction = v;
                }
                if let Some(v) = patch.length {
                    pin.length = v.max(0);
                }
                if let Some(v) = patch.hidden {
                    pin.hidden = v;
                }
                if let Some(v) = patch.at {
                    pin.at = v;
                }
                select.push(ItemRef::new(ItemKind::Pin, i));
            }
        }
        SymbolEdit::Renumber {
            indices: list,
            start,
            step,
        } => {
            let l = lists(&mut def, body)?;
            let mut number = start.clone();
            for &i in list {
                if let Some(pin) = l.pins.get_mut(i) {
                    pin.number.clone_from(&number);
                    number = next_number(&number, *step);
                    select.push(ItemRef::new(ItemKind::Pin, i));
                }
            }
        }
        SymbolEdit::AddUnit { copy } => {
            let index = usize::from(body.unit - 1);
            let mut unit = if *copy {
                def.units.get(index).cloned().unwrap_or_default()
            } else {
                SymbolUnit::default()
            };
            if *copy {
                let mut used = used_numbers(&def);
                let mut next = free_number(&def);
                let renumber = |pins: &mut Vec<Pin>,
                                used: &mut std::collections::BTreeSet<String>,
                                next: &mut String| {
                    for pin in pins {
                        while used.contains(next.as_str()) {
                            *next = next_number(next, 1);
                        }
                        pin.number.clone_from(next);
                        used.insert(next.clone());
                    }
                };
                renumber(&mut unit.pins, &mut used, &mut next);
                if let Some(alternate) = &mut unit.alternate {
                    // The alternate body keeps the numbers of the normal one.
                    for (pin, normal) in alternate.pins.iter_mut().zip(&unit.pins) {
                        pin.number.clone_from(&normal.number);
                    }
                }
            }
            unit.name = String::new();
            def.units.push(unit);
            body = Body {
                unit: u16::try_from(def.units.len()).unwrap_or(u16::MAX),
                alternate: false,
            };
        }
        SymbolEdit::RemoveUnit => {
            if def.units.len() <= 1 {
                return Err(invalid("a symbol needs at least one unit"));
            }
            let index = usize::from(body.unit - 1);
            if index >= def.units.len() {
                return Err(invalid(format!("the symbol has no unit {}", body.unit)));
            }
            def.units.remove(index);
            body = Body {
                unit: body
                    .unit
                    .min(u16::try_from(def.units.len()).unwrap_or(u16::MAX)),
                alternate: false,
            };
        }
        SymbolEdit::NameUnit { name } => {
            let index = usize::from(body.unit - 1);
            let unit = def
                .units
                .get_mut(index)
                .ok_or_else(|| invalid("no such unit"))?;
            name.trim().clone_into(&mut unit.name);
        }
        SymbolEdit::AddAlternate => {
            let index = usize::from(body.unit - 1);
            let unit = def
                .units
                .get_mut(index)
                .ok_or_else(|| invalid("no such unit"))?;
            if unit.alternate.is_none() {
                unit.alternate = Some(SymbolBody {
                    pins: unit.pins.clone(),
                    graphics: unit.graphics.clone(),
                    texts: unit.texts.clone(),
                });
            }
            body.alternate = true;
        }
        SymbolEdit::RemoveAlternate => {
            let index = usize::from(body.unit - 1);
            let unit = def
                .units
                .get_mut(index)
                .ok_or_else(|| invalid("no such unit"))?;
            unit.alternate = None;
            body.alternate = false;
        }
    }
    Ok(Edited { def, body, select })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_model::id::Id;
    use ew_model::shape::{Fill, Geometry, Stroke};

    fn pin(number: &str, x: Nm, y: Nm) -> Pin {
        Pin {
            number: number.to_owned(),
            name: format!("P{number}"),
            kind: PinKind::Passive,
            shape: PinShape::Line,
            at: Point::new(x, y),
            direction: PinDirection::Right,
            length: 2_540_000,
            hidden: false,
        }
    }

    fn def() -> SymbolDef {
        SymbolDef {
            id: Id::random(),
            name: "X".to_owned(),
            description: String::new(),
            keywords: String::new(),
            prefix: "U".to_owned(),
            value: String::new(),
            footprint: None,
            datasheet: String::new(),
            power: false,
            units: vec![SymbolUnit {
                pins: vec![pin("1", 0, 0), pin("2", 0, -2_540_000)],
                graphics: vec![SymbolGraphic {
                    geometry: Geometry::Rect {
                        start: Point::new(2_540_000, 2_540_000),
                        end: Point::new(7_620_000, -5_080_000),
                    },
                    stroke: Stroke::default(),
                    fill: Fill::Background,
                }],
                ..SymbolUnit::default()
            }],
            properties: std::collections::BTreeMap::new(),
            pin_text: ew_model::library::PinText::default(),
        }
    }

    const GRID: Nm = 1_270_000;
    const BODY: Body = Body {
        unit: 1,
        alternate: false,
    };

    #[test]
    fn pins_move_turn_and_get_numbers() {
        let d = def();
        let moved = apply(
            &d,
            BODY,
            &SymbolEdit::Move {
                items: vec![ItemRef::new(ItemKind::Pin, 0)],
                delta: Point::new(0, GRID),
            },
            GRID,
        )
        .unwrap();
        assert_eq!(moved.def.units[0].pins[0].at, Point::new(0, GRID));
        let turned = apply(
            &d,
            BODY,
            &SymbolEdit::Orient {
                items: vec![ItemRef::new(ItemKind::Pin, 0)],
                orientation: Orientation::Left,
                center: Some(Point::ORIGIN),
            },
            GRID,
        )
        .unwrap();
        assert_eq!(turned.def.units[0].pins[0].direction, PinDirection::Up);
        let mirrored = apply(
            &d,
            BODY,
            &SymbolEdit::Orient {
                items: vec![ItemRef::new(ItemKind::Graphic, 0)],
                orientation: Orientation::Horizontal,
                center: Some(Point::ORIGIN),
            },
            GRID,
        )
        .unwrap();
        let Geometry::Rect { start, .. } = &mirrored.def.units[0].graphics[0].geometry else {
            unreachable!()
        };
        assert_eq!(start.x, -2_540_000);
        let added = apply(
            &d,
            BODY,
            &SymbolEdit::AddPin {
                pin: pin("", 0, -5_080_000),
            },
            GRID,
        )
        .unwrap();
        assert_eq!(added.def.units[0].pins[2].number, "3");
        assert_eq!(added.select, [ItemRef::new(ItemKind::Pin, 2)]);
        let copied = apply(
            &d,
            BODY,
            &SymbolEdit::Duplicate {
                items: vec![
                    ItemRef::new(ItemKind::Pin, 0),
                    ItemRef::new(ItemKind::Pin, 1),
                ],
                offset: Point::new(0, -5_080_000),
            },
            GRID,
        )
        .unwrap();
        let numbers: Vec<&str> = copied.def.units[0]
            .pins
            .iter()
            .map(|p| p.number.as_str())
            .collect();
        assert_eq!(numbers, ["1", "2", "3", "4"]);
    }

    #[test]
    fn pasted_pin_lists_get_free_numbers() {
        let d = def();
        let added = apply(
            &d,
            BODY,
            &SymbolEdit::AddPins {
                pins: vec![
                    pin("", 0, -5_080_000),
                    pin("9", 0, -7_620_000),
                    pin("", 0, -10_160_000),
                ],
            },
            GRID,
        )
        .unwrap();
        let numbers: Vec<&str> = added.def.units[0]
            .pins
            .iter()
            .map(|p| p.number.as_str())
            .collect();
        assert_eq!(numbers, ["1", "2", "3", "9", "4"]);
        assert_eq!(added.select.len(), 3);
    }

    #[test]
    fn bulk_edits_and_renumbering() {
        let d = def();
        let set = apply(
            &d,
            BODY,
            &SymbolEdit::SetPins {
                indices: vec![0, 1],
                patch: PinPatch {
                    kind: Some(PinKind::Input),
                    length: Some(3_810_000),
                    ..PinPatch::default()
                },
            },
            GRID,
        )
        .unwrap();
        assert!(
            set.def.units[0]
                .pins
                .iter()
                .all(|p| p.kind == PinKind::Input && p.length == 3_810_000)
        );
        let renumbered = apply(
            &d,
            BODY,
            &SymbolEdit::Renumber {
                indices: vec![1, 0],
                start: "A1".to_owned(),
                step: 1,
            },
            GRID,
        )
        .unwrap();
        assert_eq!(renumbered.def.units[0].pins[1].number, "A1");
        assert_eq!(renumbered.def.units[0].pins[0].number, "A2");
        let deleted = apply(
            &d,
            BODY,
            &SymbolEdit::Delete {
                items: vec![
                    ItemRef::new(ItemKind::Pin, 0),
                    ItemRef::new(ItemKind::Graphic, 0),
                ],
            },
            GRID,
        )
        .unwrap();
        assert_eq!(
            (
                deleted.def.units[0].pins.len(),
                deleted.def.units[0].graphics.len()
            ),
            (1, 0)
        );
    }

    #[test]
    fn units_and_alternate_bodies() {
        let d = def();
        let two = apply(&d, BODY, &SymbolEdit::AddUnit { copy: true }, GRID).unwrap();
        assert_eq!(two.def.units.len(), 2);
        assert_eq!(two.body.unit, 2);
        let numbers: Vec<&str> = two.def.units[1]
            .pins
            .iter()
            .map(|p| p.number.as_str())
            .collect();
        assert_eq!(numbers, ["3", "4"]);
        let alternate = apply(
            &two.def,
            Body {
                unit: 2,
                alternate: false,
            },
            &SymbolEdit::AddAlternate,
            GRID,
        )
        .unwrap();
        assert!(alternate.body.alternate);
        assert_eq!(
            alternate.def.units[1]
                .alternate
                .as_ref()
                .unwrap()
                .pins
                .len(),
            2
        );
        // Edits in the alternate body leave the normal one alone.
        let moved = apply(
            &alternate.def,
            alternate.body,
            &SymbolEdit::Move {
                items: vec![ItemRef::new(ItemKind::Pin, 0)],
                delta: Point::new(GRID, 0),
            },
            GRID,
        )
        .unwrap();
        assert_eq!(moved.def.units[1].pins[0].at, Point::new(0, 0));
        assert_eq!(
            moved.def.units[1].alternate.as_ref().unwrap().pins[0].at,
            Point::new(GRID, 0)
        );
        let one = apply(
            &moved.def,
            Body {
                unit: 2,
                alternate: false,
            },
            &SymbolEdit::RemoveUnit,
            GRID,
        )
        .unwrap();
        assert_eq!(one.def.units.len(), 1);
        assert!(apply(&one.def, BODY, &SymbolEdit::RemoveUnit, GRID).is_err());
        assert!(
            apply(
                &d,
                Body {
                    unit: 1,
                    alternate: true
                },
                &SymbolEdit::Delete { items: vec![] },
                GRID
            )
            .is_err()
        );
    }
}
