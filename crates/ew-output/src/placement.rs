//! Pick-and-place (component placement, CPL) files: the position, rotation
//! and side of every part for the assembly machine.

use std::fmt::Write as _;

use ew_model::layer::Side;
use ew_model::pcb::{FootprintInstance, Pcb};
use ew_model::units::Point;
use ew_schematic::connectivity::natural_cmp;
use serde::{Deserialize, Serialize};

use crate::OutputFile;
use crate::bom::csv_field;

/// Column layout of the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlacementStyle {
    /// `Designator,Value,Footprint,X,Y,Rotation,Side` in plain numbers.
    #[default]
    Generic,
    /// `Designator,Mid X,Mid Y,Layer,Rotation` with `mm` units (JLCPCB).
    Jlcpcb,
    /// `Ref,Val,Package,PosX,PosY,Rot,Side` (KiCad's CSV columns).
    Kicad,
}

/// Length unit of the coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlacementUnit {
    /// Millimetres.
    #[default]
    Mm,
    /// Inches.
    Inch,
}

/// Options of the placement file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[allow(clippy::struct_excessive_bools, reason = "independent switches")]
pub struct PlacementOptions {
    /// Column layout.
    pub style: PlacementStyle,
    /// Unit.
    pub unit: PlacementUnit,
    /// Coordinates relative to the board origin (otherwise to the design
    /// origin).
    pub board_origin: bool,
    /// List parts that are not placed.
    pub include_dnp: bool,
    /// One file per side.
    pub split_sides: bool,
    /// Bottom parts: mirror x (as seen from the bottom).
    pub mirror_bottom_x: bool,
}

impl Default for PlacementOptions {
    fn default() -> Self {
        Self {
            style: PlacementStyle::Generic,
            unit: PlacementUnit::Mm,
            board_origin: true,
            include_dnp: false,
            split_sides: false,
            mirror_bottom_x: false,
        }
    }
}

/// One placed part.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    /// Designator.
    pub designator: String,
    /// Value.
    pub value: String,
    /// Footprint.
    pub footprint: String,
    /// Position (mm or inch).
    pub x: f64,
    /// Position (mm or inch).
    pub y: f64,
    /// Rotation in degrees, 0 ≤ r < 360.
    pub rotation: f64,
    /// Side.
    pub side: Side,
}

fn included(fp: &FootprintInstance, options: PlacementOptions) -> bool {
    !fp.attributes.exclude_from_placement && (options.include_dnp || !fp.attributes.dnp)
}

/// The placements of a board, sorted by designator.
#[must_use]
pub fn placements(pcb: &Pcb, options: &PlacementOptions) -> Vec<Placement> {
    let origin = if options.board_origin {
        pcb.origin
    } else {
        Point::ORIGIN
    };
    let scale = match options.unit {
        PlacementUnit::Mm => 1e6,
        PlacementUnit::Inch => 25.4e6,
    };
    let mut out: Vec<Placement> = pcb
        .footprints
        .iter()
        .filter(|fp| included(fp, *options))
        .map(|fp| {
            let mut x = (fp.at.x - origin.x) as f64 / scale;
            if options.mirror_bottom_x && fp.side == Side::Bottom {
                x = -x;
            }
            let rotation = fp.rotation.degrees().rem_euclid(360.0);
            Placement {
                designator: fp.designator.clone(),
                value: fp.value.clone(),
                footprint: fp.footprint.clone(),
                x,
                y: (fp.at.y - origin.y) as f64 / scale,
                rotation: if rotation >= 360.0 - 1e-9 {
                    0.0
                } else {
                    rotation
                },
                side: fp.side,
            }
        })
        .collect();
    out.sort_by(|a, b| natural_cmp(&a.designator, &b.designator));
    out
}

fn number(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    if text.starts_with('-')
        && text
            .trim_start_matches('-')
            .chars()
            .all(|c| c == '0' || c == '.')
    {
        text[1..].to_owned()
    } else {
        text
    }
}

/// The placement file as CSV.
#[must_use]
pub fn csv(rows: &[Placement], options: &PlacementOptions) -> String {
    let decimals = match options.unit {
        PlacementUnit::Mm => 4,
        PlacementUnit::Inch => 5,
    };
    let side = |s: Side| match s {
        Side::Top => "Top",
        Side::Bottom => "Bottom",
    };
    let mut out = String::new();
    match options.style {
        PlacementStyle::Generic => {
            out.push_str("Designator,Value,Footprint,X,Y,Rotation,Side\r\n");
            for r in rows {
                let _ = write!(
                    out,
                    "{},{},{},{},{},{},{}\r\n",
                    csv_field(&r.designator, ','),
                    csv_field(&r.value, ','),
                    csv_field(&r.footprint, ','),
                    number(r.x, decimals),
                    number(r.y, decimals),
                    number(r.rotation, 2),
                    side(r.side)
                );
            }
        }
        PlacementStyle::Jlcpcb => {
            let unit = match options.unit {
                PlacementUnit::Mm => "mm",
                PlacementUnit::Inch => "in",
            };
            out.push_str("Designator,Mid X,Mid Y,Layer,Rotation\r\n");
            for r in rows {
                let _ = write!(
                    out,
                    "{},{}{unit},{}{unit},{},{}\r\n",
                    csv_field(&r.designator, ','),
                    number(r.x, decimals),
                    number(r.y, decimals),
                    side(r.side),
                    number(r.rotation, 0)
                );
            }
        }
        PlacementStyle::Kicad => {
            out.push_str("Ref,Val,Package,PosX,PosY,Rot,Side\r\n");
            for r in rows {
                let _ = write!(
                    out,
                    "\"{}\",\"{}\",\"{}\",{},{},{},{}\r\n",
                    r.designator.replace('"', "\"\""),
                    r.value.replace('"', "\"\""),
                    r.footprint.replace('"', "\"\""),
                    number(r.x, 6),
                    number(r.y, 6),
                    number(r.rotation, 6),
                    side(r.side).to_lowercase()
                );
            }
        }
    }
    out
}

/// The placement files of a board (one, or one per side).
#[must_use]
pub fn placement_files(pcb: &Pcb, options: &PlacementOptions, stem: &str) -> Vec<OutputFile> {
    let rows = placements(pcb, options);
    if options.split_sides {
        [(Side::Top, "top"), (Side::Bottom, "bottom")]
            .into_iter()
            .map(|(side, name)| {
                let part: Vec<Placement> =
                    rows.iter().filter(|r| r.side == side).cloned().collect();
                OutputFile::text(format!("{stem}-placement-{name}.csv"), csv(&part, options))
            })
            .collect()
    } else {
        vec![OutputFile::text(
            format!("{stem}-placement.csv"),
            csv(&rows, options),
        )]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_model::id::Id;
    use ew_model::pcb::Stackup;
    use ew_model::units::mm;

    fn board() -> Pcb {
        let mut pcb = Pcb::new(Id::random(), "b", Stackup::standard(2, mm(1)));
        pcb.origin = Point::mm(100, 50);
        for (designator, x, rotation, side, dnp) in [
            ("R10", 110, 90, Side::Top, false),
            ("R2", 120, 270, Side::Bottom, false),
            ("C1", 130, 0, Side::Top, true),
        ] {
            let fp: FootprintInstance = serde_json::from_value(serde_json::json!({
                "id": Id::random(),
                "footprint": "R_0603",
                "designator": designator,
                "value": "10k",
                "at": [mm(x), mm(60)],
                "rotation": rotation,
                "side": side,
                "attributes": { "dnp": dnp },
            }))
            .unwrap();
            pcb.footprints.insert(fp);
        }
        pcb
    }

    #[test]
    fn rows_are_sorted_relative_to_the_origin() {
        let rows = placements(&board(), &PlacementOptions::default());
        let names: Vec<&str> = rows.iter().map(|r| r.designator.as_str()).collect();
        assert_eq!(names, ["R2", "R10"]);
        assert!((rows[1].x - 10.0).abs() < 1e-9 && (rows[1].y - 10.0).abs() < 1e-9);
        assert!((rows[0].rotation - 270.0).abs() < 1e-9);
    }

    #[test]
    fn styles() {
        let pcb = board();
        let generic = csv(
            &placements(&pcb, &PlacementOptions::default()),
            &PlacementOptions::default(),
        );
        assert!(
            generic.contains("R10,10k,R_0603,10.0000,10.0000,90.00,Top\r\n"),
            "{generic}"
        );
        let jlc = PlacementOptions {
            style: PlacementStyle::Jlcpcb,
            include_dnp: true,
            ..PlacementOptions::default()
        };
        let text = csv(&placements(&pcb, &jlc), &jlc);
        assert!(text.starts_with("Designator,Mid X,Mid Y,Layer,Rotation\r\n"));
        assert!(
            text.contains("R2,20.0000mm,10.0000mm,Bottom,270\r\n"),
            "{text}"
        );
        assert!(text.contains("C1,"));
        let split = placement_files(
            &pcb,
            &PlacementOptions {
                split_sides: true,
                ..PlacementOptions::default()
            },
            "demo",
        );
        assert_eq!(split.len(), 2);
        assert_eq!(split[1].name, "demo-placement-bottom.csv");
    }
}
