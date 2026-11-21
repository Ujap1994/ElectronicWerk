//! Parametric 3D models of packages (the part generator, ADR-0017): the
//! body as a box and the terminals by the kind of package — gull-wing
//! leads, terminals under the edge, balls, through-hole pins, chip end
//! caps and tabs — placed at the pads of the generated footprint. Every
//! piece is an exact prism, so the model is a set of solids for mechanical
//! CAD and the enclosure check, not a mesh.
//!
//! Coordinates are millimetres with the footprint's origin, x and y as in
//! the footprint and z up from the board surface.

use crate::step::solids::{Prism, file};

/// How the terminals of a package look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Terminals {
    /// Gull-wing leads out of the body sides (SOIC, QFP, SOT).
    GullWing,
    /// Flat terminals under the body edge (QFN, DFN).
    NoLead,
    /// Solder balls (BGA).
    Balls,
    /// Through-hole pins in two rows out of the body sides (DIP).
    Dip,
    /// Through-hole pins under a standing body (TO-92, TO-220 with `tab`).
    Inline {
        /// A metal tab above the body (TO-220).
        tab: bool,
    },
    /// Metal end caps of a two-terminal chip.
    Chip,
}

/// A pad of the footprint (millimetres).
#[derive(Debug, Clone, PartialEq)]
pub struct PadPlace {
    /// Pad number.
    pub number: String,
    /// Centre.
    pub at: [f64; 2],
    /// Size (x, y).
    pub size: [f64; 2],
}

/// What a package model is made from.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageSpec {
    /// Model name.
    pub name: String,
    /// Kind of terminals.
    pub terminals: Terminals,
    /// Body size (x, y).
    pub body: [f64; 2],
    /// Body top above the board (`A`).
    pub height: f64,
    /// Body bottom above the board (`A1`).
    pub standoff: f64,
    /// Lead span across x (`E`) and across y (`D` of QFPs).
    pub span: [f64; 2],
    /// Lead or terminal width (`b`).
    pub lead_width: f64,
    /// Lead thickness (`c`).
    pub lead_thickness: f64,
    /// Foot or terminal length (`L`).
    pub foot: f64,
    /// Pads of the footprint.
    pub pads: Vec<PadPlace>,
    /// Number of the exposed pad or tab, if any, and its size.
    pub exposed: Option<(String, [f64; 2])>,
}

const BODY: [f32; 3] = [0.12, 0.12, 0.13];
const METAL: [f32; 3] = [0.78, 0.78, 0.8];
const MARK: [f32; 3] = [0.85, 0.85, 0.85];
const CERAMIC: [f32; 3] = [0.45, 0.33, 0.2];

/// A box from `min` to `max`.
fn cuboid(min: [f64; 3], max: [f64; 3], color: [f32; 3]) -> Prism {
    Prism {
        outline: vec![
            [min[0], min[1], min[2]],
            [max[0], min[1], min[2]],
            [max[0], max[1], min[2]],
            [min[0], max[1], min[2]],
        ],
        holes: Vec::new(),
        extrude: [0.0, 0.0, max[2] - min[2]],
        color,
    }
}

/// An upright polygonal cylinder.
fn column(center: [f64; 2], diameter: f64, z0: f64, z1: f64, color: [f32; 3]) -> Prism {
    let r = diameter / 2.0;
    let outline = (0..12)
        .map(|i| {
            let a = f64::from(i) * std::f64::consts::TAU / 12.0;
            [center[0] + r * a.cos(), center[1] + r * a.sin(), z0]
        })
        .collect();
    Prism {
        outline,
        holes: Vec::new(),
        extrude: [0.0, 0.0, z1 - z0],
        color,
    }
}

/// A gull-wing lead: its side profile in the plane of `outward` and z,
/// extruded across by the lead width. `from` is the body edge and `tip`
/// the lead tip (distances along `outward` from the origin); `lateral` the
/// position across.
fn gull_wing(
    outward: [f64; 2],
    lateral: f64,
    from: f64,
    tip: f64,
    s: &PackageSpec,
    width: f64,
) -> Prism {
    let c = s.lead_thickness.max(0.08);
    let h = c / 2.0;
    let mid = (s.standoff + (s.height - s.standoff) * 0.45).max(c);
    let knee_out = (tip - s.foot).max(from + 0.05);
    let knee_in = (knee_out - mid * 0.35).max(from + 0.02);
    let profile = [
        (from - 0.05, mid + h),
        (knee_in + h, mid + h),
        (knee_out + h, c),
        (tip, c),
        (tip, 0.0),
        (knee_out - h, 0.0),
        (knee_in - h, mid - h),
        (from - 0.05, mid - h),
    ];
    // Across the lead: perpendicular to `outward` in the board plane.
    let across = [-outward[1], outward[0]];
    let start = lateral - width / 2.0;
    let outline = profile
        .iter()
        .map(|(u, z)| {
            [
                outward[0] * u + across[0] * start,
                outward[1] * u + across[1] * start,
                *z,
            ]
        })
        .collect();
    Prism {
        outline,
        holes: Vec::new(),
        extrude: [across[0] * width, across[1] * width, 0.0],
        color: METAL,
    }
}

/// The side of the body a pad is on: the outward direction.
fn side(pad: &PadPlace, s: &PackageSpec) -> [f64; 2] {
    let (x, y) = (pad.at[0], pad.at[1]);
    // Relative to the body: the larger overhang decides.
    let over_x = x.abs() - s.body[0] / 2.0;
    let over_y = y.abs() - s.body[1] / 2.0;
    if over_x >= over_y || pad.size[0] > pad.size[1] && over_x > -0.5 {
        [x.signum(), 0.0]
    } else {
        [0.0, y.signum()]
    }
}

fn is_exposed(pad: &PadPlace, s: &PackageSpec) -> bool {
    s.exposed.as_ref().is_some_and(|(n, _)| *n == pad.number)
}

/// The solids of a package.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one block per kind of terminal")]
pub fn prisms(s: &PackageSpec) -> Vec<Prism> {
    let (bx, by) = (s.body[0] / 2.0, s.body[1] / 2.0);
    let mut out = Vec::new();
    match s.terminals {
        Terminals::Inline { tab } => {
            // A standing body above the leads.
            let base = s.standoff.max(1.0);
            out.push(cuboid([-bx, -by, base], [bx, by, base + s.height], BODY));
            if tab {
                let t = 1.3_f64.min(by);
                out.push(cuboid(
                    [-bx, by - t, base + s.height],
                    [bx, by, base + s.height + 6.3],
                    METAL,
                ));
            }
            for pad in &s.pads {
                let w = s.lead_width.max(0.4);
                out.push(cuboid(
                    [pad.at[0] - w / 2.0, pad.at[1] - 0.25, -3.0],
                    [pad.at[0] + w / 2.0, pad.at[1] + 0.25, base + 0.5],
                    METAL,
                ));
            }
            return out;
        }
        Terminals::Chip => {
            let cap = s.foot.max(0.1).min(bx);
            out.push(cuboid(
                [-bx + cap, -by, 0.0],
                [bx - cap, by, s.height],
                CERAMIC,
            ));
            out.push(cuboid([-bx, -by, 0.0], [-bx + cap, by, s.height], METAL));
            out.push(cuboid([bx - cap, -by, 0.0], [bx, by, s.height], METAL));
            return out;
        }
        _ => {}
    }
    let bottom = if s.terminals == Terminals::Dip {
        s.standoff.max(0.38)
    } else {
        s.standoff.max(0.0)
    };
    out.push(cuboid(
        [-bx, -by, bottom],
        [bx, by, s.height.max(bottom + 0.2)],
        BODY,
    ));
    // Pin 1 mark on top, near the top-left corner.
    let mark = (bx.min(by) * 0.25).clamp(0.2, 1.0);
    out.push(cuboid(
        [
            -bx + mark * 0.6,
            by - mark * 1.6,
            s.height.max(bottom + 0.2),
        ],
        [
            -bx + mark * 1.6,
            by - mark * 0.6,
            s.height.max(bottom + 0.2) + 0.01,
        ],
        MARK,
    ));
    for pad in &s.pads {
        if is_exposed(pad, s) {
            let size = s.exposed.as_ref().map_or(pad.size, |(_, size)| *size);
            match s.terminals {
                Terminals::GullWing => {
                    // A tab: a wide gull-wing lead (SOT-223, DPAK) or a pad
                    // under the body (HTSSOP).
                    if pad.at[0].abs() > bx {
                        let dir = side(pad, s);
                        let tip = s.span[0] / 2.0;
                        out.push(gull_wing(dir, pad.at[1], bx, tip, s, size[1]));
                    } else {
                        out.push(cuboid(
                            [pad.at[0] - size[0] / 2.0, pad.at[1] - size[1] / 2.0, 0.0],
                            [
                                pad.at[0] + size[0] / 2.0,
                                pad.at[1] + size[1] / 2.0,
                                bottom.max(0.05),
                            ],
                            METAL,
                        ));
                    }
                }
                _ => out.push(cuboid(
                    [pad.at[0] - size[0] / 2.0, pad.at[1] - size[1] / 2.0, 0.0],
                    [pad.at[0] + size[0] / 2.0, pad.at[1] + size[1] / 2.0, 0.05],
                    METAL,
                )),
            }
            continue;
        }
        match s.terminals {
            Terminals::GullWing => {
                let dir = side(pad, s);
                let (from, tip, lateral) = if dir[0] == 0.0 {
                    (by, s.span[1] / 2.0, pad.at[0])
                } else {
                    (bx, s.span[0] / 2.0, pad.at[1])
                };
                out.push(gull_wing(dir, lateral, from, tip, s, s.lead_width));
            }
            Terminals::NoLead => {
                let dir = side(pad, s);
                let w = s.lead_width / 2.0;
                let l = s.foot;
                let (min, max) = if dir[0] == 0.0 {
                    let y0 = if dir[1] > 0.0 { by - l } else { -by };
                    (
                        [pad.at[0] - w, y0, 0.0],
                        [pad.at[0] + w, y0 + l, 0.2_f64.min(s.height * 0.3)],
                    )
                } else {
                    let x0 = if dir[0] > 0.0 { bx - l } else { -bx };
                    (
                        [x0, pad.at[1] - w, 0.0],
                        [x0 + l, pad.at[1] + w, 0.2_f64.min(s.height * 0.3)],
                    )
                };
                out.push(cuboid(min, max, METAL));
            }
            Terminals::Balls => {
                out.push(column(pad.at, s.lead_width, 0.0, bottom.max(0.2), METAL));
            }
            Terminals::Dip => {
                let dir = [pad.at[0].signum(), 0.0];
                let shoulder = 1.5_f64.min(s.lead_width * 3.0);
                let z = bottom + 0.6;
                let t = s.lead_thickness.max(0.25);
                // Out of the body side, then down through the board.
                let x0 = bx.min(pad.at[0].abs());
                let x1 = pad.at[0].abs() + t / 2.0;
                let (lx, hx) = if dir[0] > 0.0 { (x0, x1) } else { (-x1, -x0) };
                out.push(cuboid(
                    [lx, pad.at[1] - shoulder / 2.0, z],
                    [hx, pad.at[1] + shoulder / 2.0, z + t],
                    METAL,
                ));
                let px = pad.at[0] - dir[0] * t / 2.0;
                out.push(cuboid(
                    [px - t / 2.0, pad.at[1] - s.lead_width / 2.0, -3.0],
                    [px + t / 2.0, pad.at[1] + s.lead_width / 2.0, z],
                    METAL,
                ));
                out.push(cuboid(
                    [px - t / 2.0, pad.at[1] - shoulder / 2.0, 0.0],
                    [px + t / 2.0, pad.at[1] + shoulder / 2.0, z],
                    METAL,
                ));
            }
            Terminals::Inline { .. } | Terminals::Chip => {}
        }
    }
    out
}

/// The STEP file of a package model.
#[must_use]
pub fn step(spec: &PackageSpec, version: &str) -> String {
    file(&spec.name, &prisms(spec), version)
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn soic_spec() -> PackageSpec {
        soic()
    }

    fn soic() -> PackageSpec {
        let pads = (0..8)
            .map(|i| {
                let left = i < 4;
                let row = if left { i } else { 7 - i };
                PadPlace {
                    number: (i + 1).to_string(),
                    at: [if left { -2.7 } else { 2.7 }, 1.905 - f64::from(row) * 1.27],
                    size: [1.55, 0.6],
                }
            })
            .collect();
        PackageSpec {
            name: "SOIC-8".into(),
            terminals: Terminals::GullWing,
            body: [3.9, 4.9],
            height: 1.55,
            standoff: 0.15,
            span: [6.0, 4.9],
            lead_width: 0.41,
            lead_thickness: 0.2,
            foot: 0.8,
            pads,
            exposed: None,
        }
    }

    #[test]
    fn makes_a_body_a_mark_and_one_lead_per_pad() {
        let parts = prisms(&soic());
        assert_eq!(parts.len(), 2 + 8);
        // Leads reach the lead span and the board.
        let lead = &parts[2];
        let xs: Vec<f64> = lead.outline.iter().map(|p| p[0]).collect();
        let min_x = xs.iter().copied().fold(f64::INFINITY, f64::min);
        assert!((min_x + 3.0).abs() < 1e-9);
        assert!(lead.outline.iter().any(|p| p[2].abs() < 1e-9));
    }

    #[test]
    fn writes_a_step_file_of_solids() {
        let text = step(&soic(), "0.0.0");
        assert!(text.starts_with("ISO-10303-21;"));
        assert_eq!(text.matches("MANIFOLD_SOLID_BREP").count(), 10);
        assert_eq!(text.matches("CLOSED_SHELL").count(), 10);
        // A box has 6 faces, a lead profile of 8 points 10.
        assert_eq!(text.matches("ADVANCED_FACE").count(), 2 * 6 + 8 * 10);
    }
}

#[cfg(test)]
mod files {
    /// Writes a model for a look in a STEP viewer:
    /// `EW_PACKAGE_OUT=dir cargo test -p ew-board3d package::files -- --ignored`.
    #[test]
    #[ignore = "writes a file for a manual look"]
    fn write_file() {
        if let Some(dir) = std::env::var_os("EW_PACKAGE_OUT") {
            let spec = super::tests::soic_spec();
            let path = std::path::Path::new(&dir).join("soic8.step");
            std::fs::write(path, super::step(&spec, "test")).unwrap_or_default();
        }
    }
}
