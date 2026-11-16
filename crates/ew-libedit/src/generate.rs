//! Symbols made from a pin list (the part generator, ADR-0017): a box with
//! supply pins on top, ground and unconnected pins at the bottom, inputs on
//! the left and outputs on the right. Pins of one group (`PA0`…`PA7`,
//! `D0`…`D7`) stay together; bidirectional and passive groups go to the
//! side with fewer pins. Large parts are split into units: one for the
//! supply pins, then units of whole groups.

use std::collections::BTreeMap;

use ew_model::library::{Pin, PinDirection, PinKind, PinShape, SymbolGraphic, SymbolUnit};
use ew_model::shape::{Fill, Geometry, Stroke};
use ew_model::units::{Nm, Point};

/// Schematic grid (100 mil).
const GRID: Nm = 2_540_000;
/// Estimated width of one character of a pin name (1.27 mm text).
const CHAR: Nm = 850_000;

/// One pin to place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinSpec {
    /// Pad number.
    pub number: String,
    /// Name.
    pub name: String,
    /// Electrical type.
    pub kind: PinKind,
}

/// How the symbol is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolLayout {
    /// Split into units above this many pins (0: never).
    pub split_above: usize,
    /// Largest number of pins of a unit after splitting.
    pub unit_size: usize,
    /// Pin length.
    pub pin_length: Nm,
}

impl Default for SymbolLayout {
    fn default() -> Self {
        Self {
            split_above: 64,
            unit_size: 40,
            pin_length: GRID,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Place {
    Top,
    Bottom,
    Left,
    Right,
    Either,
}

fn is_ground(name: &str) -> bool {
    let n = name.to_ascii_uppercase();
    [
        "GND", "VSS", "AGND", "DGND", "PGND", "SGND", "V-", "VEE", "AVSS", "DVSS", "GROUND",
    ]
    .iter()
    .any(|g| n.starts_with(g))
        || n == "EP"
        || n.starts_with("EPAD")
        || n.contains("THERMAL")
        || n.contains("POWERPAD")
}

fn place(pin: &PinSpec) -> Place {
    match pin.kind {
        PinKind::PowerIn | PinKind::Passive if is_ground(&pin.name) => Place::Bottom,
        PinKind::NoConnect | PinKind::Free => Place::Bottom,
        PinKind::PowerIn => Place::Top,
        PinKind::Input => Place::Left,
        PinKind::Output
        | PinKind::PowerOut
        | PinKind::OpenCollector
        | PinKind::OpenEmitter
        | PinKind::TriState => Place::Right,
        PinKind::Bidirectional | PinKind::Passive | PinKind::Unspecified => Place::Either,
    }
}

/// The group of a pin: its name without the trailing index and signs
/// (`PA7` → `PA`, `IN1+` → `IN`).
fn group(name: &str) -> String {
    name.trim_end_matches(|c: char| c.is_ascii_digit() || matches!(c, '+' | '-' | '_' | '#'))
        .to_owned()
}

/// The index in its group, for ordering (`PA10` after `PA9`).
fn index(name: &str) -> (u64, String) {
    let digits: String = name
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    (digits.parse().unwrap_or(0), name.to_owned())
}

fn natural(a: &str, b: &str) -> std::cmp::Ordering {
    let key = |s: &str| {
        let digits: String = s.chars().filter(char::is_ascii_digit).collect();
        (digits.len(), digits, s.to_owned())
    };
    key(a).cmp(&key(b))
}

/// Groups of pins in the order of their first pad.
fn groups(pins: &[PinSpec]) -> Vec<Vec<PinSpec>> {
    let mut order: Vec<String> = Vec::new();
    let mut by_group: BTreeMap<String, Vec<PinSpec>> = BTreeMap::new();
    let mut sorted: Vec<&PinSpec> = pins.iter().collect();
    sorted.sort_by(|a, b| natural(&a.number, &b.number));
    for pin in sorted {
        let g = group(&pin.name);
        if !by_group.contains_key(&g) {
            order.push(g.clone());
        }
        by_group.entry(g).or_default().push(pin.clone());
    }
    order
        .into_iter()
        .filter_map(|g| by_group.remove(&g))
        .map(|mut list| {
            list.sort_by_key(|p| index(&p.name));
            list
        })
        .collect()
}

/// Slots of a side: groups with one free slot between them.
fn slots(groups: &[Vec<PinSpec>]) -> usize {
    let pins: usize = groups.iter().map(Vec::len).sum();
    pins + groups.len().saturating_sub(1)
}

fn round_up(value: Nm, step: Nm) -> Nm {
    ((value + step - 1) / step) * step
}

fn name_width(pins: &[&PinSpec]) -> Nm {
    pins.iter()
        .map(|p| p.name.chars().count())
        .max()
        .map_or(0, |n| Nm::try_from(n).unwrap_or(0) * CHAR)
}

/// Lays out one unit.
#[allow(clippy::too_many_lines, reason = "sides, sizes and pins of one box")]
fn unit(pins: &[PinSpec], layout: &SymbolLayout) -> SymbolUnit {
    let mut sides: BTreeMap<Place, Vec<Vec<PinSpec>>> = BTreeMap::new();
    for g in groups(pins) {
        // A group goes where most of its pins belong.
        let mut votes: BTreeMap<Place, usize> = BTreeMap::new();
        for p in &g {
            *votes.entry(place(p)).or_default() += 1;
        }
        let side = votes
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
            .map_or(Place::Either, |(p, _)| p);
        sides.entry(side).or_default().push(g);
    }
    let mut left = sides.remove(&Place::Left).unwrap_or_default();
    let mut right = sides.remove(&Place::Right).unwrap_or_default();
    for g in sides.remove(&Place::Either).unwrap_or_default() {
        if slots(&left) <= slots(&right) {
            left.push(g);
        } else {
            right.push(g);
        }
    }
    let top: Vec<PinSpec> = sides
        .remove(&Place::Top)
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .collect();
    let mut bottom: Vec<PinSpec> = sides
        .remove(&Place::Bottom)
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .collect();
    // Ground first, unconnected pins after it.
    bottom.sort_by_key(|p| {
        (
            matches!(p.kind, PinKind::NoConnect | PinKind::Free),
            natural_key(&p.number),
        )
    });
    // Too many supply pins for the top and bottom: move to the sides.
    let rows = slots(&left).max(slots(&right)).max(1);
    let pin_names_left: Vec<&PinSpec> = left.iter().flatten().collect();
    let pin_names_right: Vec<&PinSpec> = right.iter().flatten().collect();
    let text = name_width(&pin_names_left) + name_width(&pin_names_right) + 2 * GRID;
    let across = |n: usize| Nm::try_from(n + 1).unwrap_or(1) * GRID;
    let width = round_up(
        text.max(across(top.len()))
            .max(across(bottom.len()))
            .max(4 * GRID),
        2 * GRID,
    );
    let vertical_name = |pins: &[PinSpec]| {
        let refs: Vec<&PinSpec> = pins.iter().collect();
        if pins.is_empty() {
            0
        } else {
            round_up(name_width(&refs) + GRID / 2, GRID)
        }
    };
    let top_band = vertical_name(&top);
    let bottom_band = vertical_name(&bottom);
    let band = Nm::try_from(rows).unwrap_or(1) * GRID;
    let height = round_up(band + GRID + top_band + bottom_band, 2 * GRID);
    let (half_w, half_h) = (width / 2, height / 2);
    let length = layout.pin_length;
    let mut out = Vec::new();
    let mut push = |p: &PinSpec, at: Point, direction: PinDirection| {
        out.push(Pin {
            number: p.number.clone(),
            name: p.name.clone(),
            kind: p.kind,
            shape: PinShape::Line,
            at,
            direction,
            length,
            hidden: false,
        });
    };
    // The side rows start below the top band, on the grid.
    let first_row = half_h - top_band - GRID;
    for (list, x, direction) in [
        (&left, -half_w - length, PinDirection::Right),
        (&right, half_w + length, PinDirection::Left),
    ] {
        let mut y = first_row;
        for (gi, g) in list.iter().enumerate() {
            if gi > 0 {
                y -= GRID;
            }
            for p in g {
                push(p, Point::new(x, y), direction);
                y -= GRID;
            }
        }
    }
    for (list, y, direction) in [
        (&top, half_h + length, PinDirection::Down),
        (&bottom, -half_h - length, PinDirection::Up),
    ] {
        let n = Nm::try_from(list.len()).unwrap_or(0);
        let start = -(n / 2) * GRID;
        for (i, p) in list.iter().enumerate() {
            push(
                p,
                Point::new(start + Nm::try_from(i).unwrap_or(0) * GRID, y),
                direction,
            );
        }
    }
    SymbolUnit {
        name: String::new(),
        pins: out,
        graphics: vec![SymbolGraphic {
            geometry: Geometry::Rect {
                start: Point::new(-half_w, half_h),
                end: Point::new(half_w, -half_h),
            },
            stroke: Stroke::default(),
            fill: Fill::Background,
        }],
        texts: Vec::new(),
        alternate: None,
    }
}

fn natural_key(s: &str) -> (usize, String) {
    let digits: String = s.chars().filter(char::is_ascii_digit).collect();
    (digits.len(), format!("{digits}{s}"))
}

/// The units of a symbol for `pins`.
#[must_use]
pub fn symbol_units(pins: &[PinSpec], layout: &SymbolLayout) -> Vec<SymbolUnit> {
    if pins.is_empty() {
        return vec![unit(&[], layout)];
    }
    if layout.split_above == 0 || pins.len() <= layout.split_above {
        return vec![unit(pins, layout)];
    }
    // A supply unit, then units of whole groups.
    let (supply, rest): (Vec<PinSpec>, Vec<PinSpec>) = pins
        .iter()
        .cloned()
        .partition(|p| matches!(place(p), Place::Top | Place::Bottom));
    let mut units: Vec<Vec<PinSpec>> = Vec::new();
    if !supply.is_empty() {
        units.push(supply);
    }
    let size = layout.unit_size.max(4);
    let mut current: Vec<PinSpec> = Vec::new();
    for g in groups(&rest) {
        for chunk in g.chunks(size) {
            if !current.is_empty() && current.len() + chunk.len() > size {
                units.push(std::mem::take(&mut current));
            }
            current.extend(chunk.iter().cloned());
        }
    }
    if !current.is_empty() {
        units.push(current);
    }
    units
        .iter()
        .enumerate()
        .map(|(i, list)| {
            let mut u = unit(list, layout);
            u.name = unit_name(i);
            u
        })
        .collect()
}

/// `A`, `B`, … `Z`, `AA`, ….
fn unit_name(index: usize) -> String {
    let letters = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    if index < 26 {
        char::from(letters[index]).to_string()
    } else {
        format!(
            "{}{}",
            char::from(letters[(index / 26 - 1) % 26]),
            char::from(letters[index % 26])
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(number: &str, name: &str, kind: PinKind) -> PinSpec {
        PinSpec {
            number: number.into(),
            name: name.into(),
            kind,
        }
    }

    fn comparator() -> Vec<PinSpec> {
        vec![
            spec("1", "OUT1", PinKind::Output),
            spec("2", "IN1-", PinKind::Input),
            spec("3", "IN1+", PinKind::Input),
            spec("4", "GND", PinKind::PowerIn),
            spec("5", "IN2+", PinKind::Input),
            spec("6", "IN2-", PinKind::Input),
            spec("7", "OUT2", PinKind::Output),
            spec("8", "VCC", PinKind::PowerIn),
        ]
    }

    fn pin<'a>(unit: &'a SymbolUnit, name: &str) -> &'a Pin {
        unit.pins
            .iter()
            .find(|p| p.name == name)
            .unwrap_or_else(|| panic!("no pin {name}"))
    }

    #[test]
    fn places_pins_by_their_function() {
        let units = symbol_units(&comparator(), &SymbolLayout::default());
        assert_eq!(units.len(), 1);
        let u = &units[0];
        assert_eq!(u.pins.len(), 8);
        assert_eq!(pin(u, "VCC").direction, PinDirection::Down);
        assert_eq!(pin(u, "GND").direction, PinDirection::Up);
        assert_eq!(pin(u, "IN1-").direction, PinDirection::Right);
        assert_eq!(pin(u, "OUT1").direction, PinDirection::Left);
        // Every connection point on the 100 mil grid.
        for p in &u.pins {
            assert_eq!(p.at.x % GRID, 0, "{} x", p.name);
            assert_eq!(p.at.y % GRID, 0, "{} y", p.name);
        }
        // Inputs of a group in order, top to bottom.
        assert!(
            pin(u, "IN1+").at.y < pin(u, "IN1-").at.y || pin(u, "IN1-").at.y < pin(u, "IN1+").at.y
        );
        assert!(pin(u, "OUT1").at.y > pin(u, "OUT2").at.y);
    }

    #[test]
    fn keeps_ports_together_and_balances_sides() {
        let mut pins = Vec::new();
        for i in 0..8 {
            pins.push(spec(
                &(i + 1).to_string(),
                &format!("PA{i}"),
                PinKind::Bidirectional,
            ));
            pins.push(spec(
                &(i + 9).to_string(),
                &format!("PB{i}"),
                PinKind::Bidirectional,
            ));
        }
        let u = &symbol_units(&pins, &SymbolLayout::default())[0];
        let side = |name: &str| pin(u, name).direction;
        assert!((0..8).all(|i| side(&format!("PA{i}")) == side("PA0")));
        assert!((0..8).all(|i| side(&format!("PB{i}")) == side("PB0")));
        assert_ne!(side("PA0"), side("PB0"));
        assert!(pin(u, "PA0").at.y > pin(u, "PA7").at.y);
    }

    #[test]
    fn splits_large_parts_into_units() {
        let mut pins = vec![
            spec("1", "VDD", PinKind::PowerIn),
            spec("2", "GND", PinKind::PowerIn),
        ];
        for port in ["PA", "PB", "PC", "PD", "PE", "PF", "PG", "PH", "PJ"] {
            for i in 0..8 {
                let n = pins.len() + 1;
                pins.push(spec(
                    &n.to_string(),
                    &format!("{port}{i}"),
                    PinKind::Bidirectional,
                ));
            }
        }
        let units = symbol_units(&pins, &SymbolLayout::default());
        assert!(units.len() >= 3);
        assert_eq!(units[0].name, "A");
        assert_eq!(units[0].pins.len(), 2);
        let total: usize = units.iter().map(|u| u.pins.len()).sum();
        assert_eq!(total, pins.len());
        assert!(units.iter().all(|u| u.pins.len() <= 40));
    }
}
