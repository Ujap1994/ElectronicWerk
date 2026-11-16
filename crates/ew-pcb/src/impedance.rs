//! Characteristic impedance of tracks from the board's stack-up.
//!
//! An outer layer carries **microstrip** lines over the plane of the next
//! copper layer; an inner layer carries **stripline** between the planes
//! above and below it. The formulas are the usual closed forms that board
//! calculators use, accurate to a few per cent:
//!
//! - microstrip: Hammerstad and Jensen (1980), with their correction for
//!   the copper thickness;
//! - stripline: Wheeler (1978), with the thickness; an off-centre line as
//!   the parallel combination of two centred ones (Wadell);
//! - edge-coupled pairs: the IPC-2141 coupling factors.
//!
//! Solder mask and the etch profile are not taken into account; board
//! makers check impedance-controlled boards with a field solver.

use std::f64::consts::{E, PI};

use ew_model::layer::CopperLayer;
use ew_model::pcb::{Stackup, StackupLayer};
use serde::{Deserialize, Serialize};

/// The impedance of free space (Ω).
const ETA0: f64 = 376.730_313;

/// How a track sits between its planes (lengths in nm).
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "kind"
)]
pub enum Structure {
    /// On an outer layer, `height` above the plane under it.
    Microstrip {
        /// Dielectric between the track and its plane.
        height: f64,
        /// Relative permittivity of that dielectric.
        epsilon_r: f64,
        /// Copper thickness.
        thickness: f64,
    },
    /// On an inner layer, between two planes.
    Stripline {
        /// Dielectric to the plane above.
        above: f64,
        /// Dielectric to the plane below.
        below: f64,
        /// Relative permittivity (thickness-weighted mean of both sides).
        epsilon_r: f64,
        /// Copper thickness.
        thickness: f64,
    },
}

/// A single track or an edge-coupled pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    /// One track against its planes.
    #[default]
    Single,
    /// Two coupled tracks (the differential impedance).
    Differential,
}

/// The structure of a copper layer, or `None` without a plane next to it
/// (a single-sided board).
#[must_use]
pub fn structure(stackup: &Stackup, layer: CopperLayer) -> Option<Structure> {
    let layers = &stackup.layers;
    let at = layers
        .iter()
        .position(|l| matches!(l, StackupLayer::Copper { layer: own, .. } if *own == layer))?;
    let thickness = layers[at].thickness() as f64;
    // The dielectrics up to the next copper layer on one side.
    let side = |range: &mut dyn Iterator<Item = &StackupLayer>| -> Option<(f64, f64)> {
        let mut height = 0.0;
        let mut weighted = 0.0;
        for l in range {
            match l {
                StackupLayer::Dielectric {
                    thickness,
                    epsilon_r,
                    ..
                } => {
                    let t = *thickness as f64;
                    height += t;
                    weighted += t * epsilon_r;
                }
                StackupLayer::Copper { .. } => {
                    return (height > 0.0).then(|| (height, weighted / height));
                }
                _ => {}
            }
        }
        None
    };
    let above = side(&mut layers[..at].iter().rev());
    let below = side(&mut layers[at + 1..].iter());
    match (above, below) {
        (Some((a, ea)), Some((b, eb))) => Some(Structure::Stripline {
            above: a,
            below: b,
            epsilon_r: (a * ea + b * eb) / (a + b),
            thickness,
        }),
        (Some((height, epsilon_r)), None) | (None, Some((height, epsilon_r))) => {
            Some(Structure::Microstrip {
                height,
                epsilon_r,
                thickness,
            })
        }
        (None, None) => None,
    }
}

/// The impedance of a microstrip line of width `w` in free space for a
/// width-to-height ratio `u` (Hammerstad and Jensen).
fn z_air(u: f64) -> f64 {
    let f = 6.0 + (2.0 * PI - 6.0) * (-(30.666 / u).powf(0.7528)).exp();
    ETA0 / (2.0 * PI) * (f / u + (1.0 + 4.0 / (u * u)).sqrt()).ln()
}

/// The effective permittivity of a microstrip line (Hammerstad and Jensen).
fn epsilon_eff(u: f64, er: f64) -> f64 {
    let a = 1.0 + (u.powi(4) + (u / 52.0).powi(2)).ln() / 49.0 - (u.powi(4) + 0.432).ln() / 49.0
        + (1.0 + (u / 18.1).powi(3)).ln() / 18.7;
    let b = 0.564 * ((er - 0.9) / (er + 3.0)).powf(0.053);
    f64::midpoint(er, 1.0) + (er - 1.0) / 2.0 * (1.0 + 10.0 / u).powf(-a * b)
}

/// A microstrip line of width `w`.
fn microstrip(w: f64, h: f64, er: f64, t: f64) -> f64 {
    let u = w / h;
    // The copper thickness widens the line a little (Hammerstad and
    // Jensen); the corrected permittivity cancels out of the impedance.
    let ur = if t > 0.0 {
        let th = t / h;
        let coth = 1.0 / (6.517 * u).sqrt().tanh();
        let du1 = th / PI * (1.0 + 4.0 * E / (th * coth * coth)).ln();
        u + f64::midpoint(1.0, 1.0 / (er - 1.0).sqrt().cosh()) * du1
    } else {
        u
    };
    z_air(ur) / epsilon_eff(ur, er).sqrt()
}

/// A centred stripline of width `w` between planes `b` apart (Wheeler).
#[allow(clippy::many_single_char_names, reason = "the formula's symbols")]
fn stripline_centred(w: f64, b: f64, er: f64, t: f64) -> f64 {
    let x = t / b;
    let bt = b - t;
    let w_eff = if t > 0.0 {
        let m = 6.0 / (3.0 + 2.0 * t / bt);
        let inner = (x / (2.0 - x)).powi(2) + (0.0796 * x / (w / b + 1.1 * x)).powf(m);
        w + bt * x / (PI * (1.0 - x)) * (1.0 - 0.5 * inner.ln())
    } else {
        w
    };
    let r = 4.0 / PI * bt / w_eff;
    ETA0 / (4.0 * PI * er.sqrt()) * (1.0 + r * (2.0 * r + ((2.0 * r).powi(2) + 6.27).sqrt())).ln()
}

/// The impedance of a single track of width `width` (nm).
#[must_use]
pub fn single(structure: &Structure, width: f64) -> f64 {
    match *structure {
        Structure::Microstrip {
            height,
            epsilon_r,
            thickness,
        } => microstrip(width, height, epsilon_r, thickness),
        Structure::Stripline {
            above,
            below,
            epsilon_r,
            thickness,
        } => {
            // Off centre: two centred striplines in parallel.
            let za = stripline_centred(width, 2.0 * above + thickness, epsilon_r, thickness);
            let zb = stripline_centred(width, 2.0 * below + thickness, epsilon_r, thickness);
            2.0 * za * zb / (za + zb)
        }
    }
}

/// The differential impedance of two tracks of width `width` with `gap`
/// between them (nm).
#[must_use]
pub fn differential(structure: &Structure, width: f64, gap: f64) -> f64 {
    let z0 = single(structure, width);
    match *structure {
        Structure::Microstrip { height, .. } => {
            2.0 * z0 * (1.0 - 0.48 * (-0.96 * gap / height).exp())
        }
        Structure::Stripline {
            above,
            below,
            thickness,
            ..
        } => {
            let b = above + below + thickness;
            2.0 * z0 * (1.0 - 0.347 * (-2.9 * gap / b).exp())
        }
    }
}

/// The impedance of a track, or of a pair with `gap`.
#[must_use]
pub fn impedance(structure: &Structure, kind: Kind, width: f64, gap: f64) -> f64 {
    match kind {
        Kind::Single => single(structure, width),
        Kind::Differential => differential(structure, width, gap),
    }
}

/// Bisection of a falling function: the `x` in `[low, high]` where `f`
/// reaches `target`, or `None` if it does not.
fn solve_falling(f: &dyn Fn(f64) -> f64, target: f64, low: f64, high: f64) -> Option<f64> {
    let (mut lo, mut hi) = (low, high);
    if f(lo) < target || f(hi) > target {
        return None;
    }
    for _ in 0..80 {
        let mid = f64::midpoint(lo, hi);
        if f(mid) > target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(f64::midpoint(lo, hi))
}

/// The narrowest and widest track the solver tries (nm).
const WIDTH_RANGE: (f64, f64) = (10_000.0, 20_000_000.0);
/// The largest gap the solver tries (nm).
const GAP_MAX: f64 = 5_000_000.0;

/// The width (nm) of a single track with `target` ohms.
#[must_use]
pub fn solve_width(structure: &Structure, target: f64) -> Option<f64> {
    solve_falling(
        &|w| single(structure, w),
        target,
        WIDTH_RANGE.0,
        WIDTH_RANGE.1,
    )
}

/// Width and gap (nm) of a pair with `target` ohms: the gap as small as the
/// board allows (`min_gap`, tight coupling), the width from it; if that
/// width would be below `min_width`, the width stays at the minimum and the
/// gap grows instead.
#[must_use]
pub fn solve_pair(
    structure: &Structure,
    target: f64,
    min_width: f64,
    min_gap: f64,
) -> Option<(f64, f64)> {
    let gap = min_gap.max(1_000.0);
    if let Some(width) = solve_falling(
        &|w| differential(structure, w, gap),
        target,
        WIDTH_RANGE.0,
        WIDTH_RANGE.1,
    ) && width >= min_width
    {
        return Some((width, gap));
    }
    // The gap grows (the impedance with it) at the narrowest width.
    let width = min_width.max(WIDTH_RANGE.0);
    let rising = |s: f64| -differential(structure, width, s);
    solve_falling(&rising, -target, gap, GAP_MAX).map(|gap| (width, gap))
}

/// An interface whose tracks need a controlled impedance (the beginner
/// mode's choices).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    /// Identifier (`usb2`, …), stored in the profile.
    pub id: &'static str,
    /// English name.
    pub name: &'static str,
    /// A pair of tracks.
    pub differential: bool,
    /// Target impedance (Ω).
    pub target: u16,
    /// Allowed deviation (%).
    pub tolerance: u8,
}

/// The interfaces the beginner mode offers.
pub const PRESETS: &[Preset] = &[
    Preset {
        id: "usb2",
        name: "USB 2.0",
        differential: true,
        target: 90,
        tolerance: 10,
    },
    Preset {
        id: "usb3",
        name: "USB 3 and USB-C high speed",
        differential: true,
        target: 90,
        tolerance: 7,
    },
    Preset {
        id: "ethernet",
        name: "Ethernet",
        differential: true,
        target: 100,
        tolerance: 10,
    },
    Preset {
        id: "hdmi",
        name: "HDMI",
        differential: true,
        target: 100,
        tolerance: 10,
    },
    Preset {
        id: "lvds",
        name: "LVDS and MIPI",
        differential: true,
        target: 100,
        tolerance: 10,
    },
    Preset {
        id: "pcie",
        name: "PCI Express",
        differential: true,
        target: 85,
        tolerance: 10,
    },
    Preset {
        id: "can",
        name: "CAN and RS-485",
        differential: true,
        target: 120,
        tolerance: 10,
    },
    Preset {
        id: "rf50",
        name: "RF and antennas (50 Ω)",
        differential: false,
        target: 50,
        tolerance: 10,
    },
];

/// A profile on one layer: the width and gap, the impedance they give and
/// whether they were set by hand.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    /// Track width (nm).
    pub width: f64,
    /// Gap between the tracks of a pair (nm; 0 for single tracks).
    pub gap: f64,
    /// The impedance they give (Ω).
    pub impedance: f64,
    /// Set by hand (otherwise computed).
    pub manual: bool,
    /// Within the board minimums (track width, clearance).
    pub fits: bool,
}

/// Rounds a computed length to whole micrometres.
fn whole_um(value: f64) -> f64 {
    (value / 1_000.0).round() * 1_000.0
}

/// A profile on a layer: its width and gap set by hand, or computed from
/// the stack-up (the gap no smaller than `min_gap`, the width no smaller
/// than `min_width` where that can still reach the target). `None` on a
/// layer without a plane next to it, or when no width reaches the target.
#[must_use]
pub fn layout(
    profile: &ew_model::rules::ImpedanceProfile,
    stackup: &Stackup,
    layer: CopperLayer,
    min_width: f64,
    min_gap: f64,
) -> Option<Layout> {
    let structure = structure(stackup, layer)?;
    let kind = if profile.differential {
        Kind::Differential
    } else {
        Kind::Single
    };
    let (width, gap, manual) = if let Some(set) = profile.layers.get(&layer.to_string()) {
        (set.width as f64, set.gap as f64, true)
    } else if profile.differential {
        let (w, g) = solve_pair(&structure, f64::from(profile.target), min_width, min_gap)?;
        (whole_um(w), whole_um(g), false)
    } else {
        (
            whole_um(solve_width(&structure, f64::from(profile.target))?),
            0.0,
            false,
        )
    };
    Some(Layout {
        width,
        gap,
        impedance: impedance(&structure, kind, width, gap),
        manual,
        fits: width >= min_width - 0.5 && (!profile.differential || gap >= min_gap - 0.5),
    })
}

/// `true` if an impedance is within the profile's tolerance.
#[must_use]
pub fn within(profile: &ew_model::rules::ImpedanceProfile, impedance: f64) -> bool {
    let target = f64::from(profile.target);
    (impedance - target).abs() <= target * f64::from(profile.tolerance) / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_model::pcb::STANDARD_THICKNESS;

    const MM: f64 = 1e6;

    fn close(value: f64, expected: f64, percent: f64) -> bool {
        (value - expected).abs() <= expected * percent / 100.0
    }

    #[test]
    fn microstrip_matches_the_textbook() {
        // FR4 (εr 4.4), 1.6 mm, no copper thickness: a 50 Ω line is about
        // 3.06 mm wide (Pozar, Microwave Engineering, example 3.7 style).
        let line = Structure::Microstrip {
            height: 1.6 * MM,
            epsilon_r: 4.4,
            thickness: 0.0,
        };
        let w = solve_width(&line, 50.0).unwrap();
        assert!(close(w, 3.06 * MM, 2.0), "{w}");
        // w/h = 1 on εr 4.3: about 72 Ω (Wheeler's narrow-line formula
        // gives 71.9 Ω).
        let line = Structure::Microstrip {
            height: 1.0 * MM,
            epsilon_r: 4.3,
            thickness: 0.0,
        };
        assert!(close(single(&line, 1.0 * MM), 71.9, 1.0));
    }

    #[test]
    fn stripline_matches_the_textbook() {
        // Centred stripline, b = 2 mm, εr 2.2, zero thickness: 50 Ω at
        // about w = 1.66 mm (Pozar, example 3.5).
        let line = Structure::Stripline {
            above: 1.0 * MM,
            below: 1.0 * MM,
            epsilon_r: 2.2,
            thickness: 0.0,
        };
        let w = solve_width(&line, 50.0).unwrap();
        assert!(close(w, 1.66 * MM, 3.0), "{w}");
    }

    #[test]
    fn copper_thickness_and_coupling_lower_the_impedance() {
        let thin = Structure::Microstrip {
            height: 0.2 * MM,
            epsilon_r: 4.5,
            thickness: 0.0,
        };
        let thick = Structure::Microstrip {
            height: 0.2 * MM,
            epsilon_r: 4.5,
            thickness: 0.035 * MM,
        };
        assert!(single(&thick, 0.3 * MM) < single(&thin, 0.3 * MM));
        let tight = differential(&thick, 0.2 * MM, 0.1 * MM);
        let loose = differential(&thick, 0.2 * MM, 1.0 * MM);
        assert!(tight < loose);
        assert!(loose < 2.0 * single(&thick, 0.2 * MM));
    }

    #[test]
    fn layers_of_a_standard_stack() {
        let two = Stackup::standard(2, STANDARD_THICKNESS);
        let top = structure(&two, CopperLayer::Top).unwrap();
        assert!(matches!(top, Structure::Microstrip { height, .. } if height > 1.4 * MM));
        let four = Stackup::standard(4, STANDARD_THICKNESS);
        assert!(matches!(
            structure(&four, CopperLayer::Inner(1)).unwrap(),
            Structure::Stripline { .. }
        ));
        assert!(structure(&Stackup::standard(1, STANDARD_THICKNESS), CopperLayer::Top).is_none());
        // USB on the top of a 4-layer board: a pair within 1 Ω of 90 Ω.
        let top = structure(&four, CopperLayer::Top).unwrap();
        let (w, s) = solve_pair(&top, 90.0, 0.1 * MM, 0.15 * MM).unwrap();
        assert!(close(differential(&top, w, s), 90.0, 1.0));
        assert!(w >= 0.1 * MM && s >= 0.15 * MM);
    }
}
