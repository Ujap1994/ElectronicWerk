//! Drill files in the Excellon format.
//!
//! Plated holes (through-hole pads and vias) and non-plated holes go into
//! separate files, as board makers expect; vias that do not go through the
//! whole board get one file per layer pair. Slots are written with `G85`.
//! The files carry the X2 attributes as KiCad writes them (`; #@! …`
//! comments), so CAM tools can tell vias from component holes.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ew_model::layer::CopperLayer;
use ew_model::pcb::{PadKind, Pcb};
use ew_pcb::geom::{Core, V};
use ew_pcb::items::{footprint_xform, pad_hole};

use crate::OutputFile;

/// What a hole is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HoleKind {
    /// A via.
    Via,
    /// A plated component hole.
    Component,
    /// A non-plated (mechanical) hole.
    Mechanical,
}

impl HoleKind {
    const fn attribute(self) -> &'static str {
        match self {
            Self::Via => "Plated,PTH,ViaDrill",
            Self::Component => "Plated,PTH,ComponentDrill",
            Self::Mechanical => "NonPlated,NPTH,ComponentDrill",
        }
    }
}

/// One drilled hole.
#[derive(Debug, Clone, PartialEq)]
pub struct Hole {
    /// Centre (round holes) or start of the slot.
    pub start: V,
    /// End of the slot; `None` for round holes.
    pub end: Option<V>,
    /// Diameter (slot width).
    pub diameter: f64,
    /// Plated.
    pub plated: bool,
    /// Function.
    pub kind: HoleKind,
    /// First and last copper layer (indices in the stack, top = 0).
    pub span: (usize, usize),
}

/// Every hole of a board.
#[must_use]
pub fn holes(pcb: &Pcb) -> Vec<Hole> {
    let stack: Vec<CopperLayer> = pcb.stackup.copper_layers().collect();
    let last = stack.len().saturating_sub(1);
    let index = |layer: CopperLayer| stack.iter().position(|own| *own == layer);
    let mut out = Vec::new();
    for fp in &pcb.footprints {
        let xf = footprint_xform(fp);
        for pad in &fp.pads {
            let Some(shape) = pad_hole(pad, &xf) else {
                continue;
            };
            let plated = pad.kind != PadKind::NpThroughHole;
            let (start, end) = match shape.core {
                Core::Point(p) => (p, None),
                Core::Segment(a, b) => (a, Some(b)),
                Core::Poly(_) => continue,
            };
            out.push(Hole {
                start,
                end,
                diameter: shape.radius * 2.0,
                plated,
                kind: if plated {
                    HoleKind::Component
                } else {
                    HoleKind::Mechanical
                },
                span: (0, last),
            });
        }
    }
    for via in &pcb.vias {
        let (Some(a), Some(b)) = (index(via.from), index(via.to)) else {
            continue;
        };
        out.push(Hole {
            start: V::of(via.at),
            end: None,
            diameter: via.drill as f64,
            plated: true,
            kind: HoleKind::Via,
            span: (a.min(b), a.max(b)),
        });
    }
    out
}

/// Options of the drill output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DrillOptions {
    /// Plated and non-plated holes in one file.
    pub merge: bool,
}

/// A coordinate in millimetres, at most four decimals.
fn coord(value: f64) -> String {
    let mut text = format!("{:.4}", value / 1e6);
    while text.ends_with('0') && !text.ends_with(".0") {
        text.pop();
    }
    if text == "-0.0" {
        "0.0".clone_into(&mut text);
    }
    text
}

/// One drill file of a group of holes.
fn file(holes: &[&Hole], function: &str, version: &str, created: &str) -> String {
    let mut out = String::from("M48\n");
    let _ = writeln!(out, "; DRILL file ElectronicWerk {version}");
    out.push_str("; FORMAT={-:-/ absolute / metric / decimal}\n");
    if !created.is_empty() {
        let _ = writeln!(out, "; #@! TF.CreationDate,{created}");
    }
    let _ = writeln!(
        out,
        "; #@! TF.GenerationSoftware,ElectronicWerk,ElectronicWerk,{version}"
    );
    let _ = writeln!(out, "; #@! TF.FileFunction,{function}");
    out.push_str("FMAT,2\nMETRIC\n");
    // Tools by diameter (rounded to 0.1 µm) and function.
    let mut tools: BTreeMap<(i64, HoleKind), Vec<&Hole>> = BTreeMap::new();
    for hole in holes {
        tools
            .entry(((hole.diameter / 100.0).round() as i64, hole.kind))
            .or_default()
            .push(hole);
    }
    for (number, ((diameter, kind), _)) in tools.iter().enumerate() {
        let _ = writeln!(out, "; #@! TA.AperFunction,{}", kind.attribute());
        let _ = writeln!(out, "T{}C{:.3}", number + 1, *diameter as f64 * 100.0 / 1e6);
    }
    out.push_str("%\nG90\nG05\n");
    for (number, list) in tools.values().enumerate() {
        let _ = writeln!(out, "T{}", number + 1);
        let mut list = list.clone();
        list.sort_by(|a, b| {
            a.start
                .x
                .total_cmp(&b.start.x)
                .then(a.start.y.total_cmp(&b.start.y))
        });
        for hole in list {
            match hole.end {
                Some(end) => {
                    let _ = writeln!(
                        out,
                        "X{}Y{}G85X{}Y{}",
                        coord(hole.start.x),
                        coord(hole.start.y),
                        coord(end.x),
                        coord(end.y)
                    );
                }
                None => {
                    let _ = writeln!(out, "X{}Y{}", coord(hole.start.x), coord(hole.start.y));
                }
            }
        }
    }
    out.push_str("M30\n");
    out
}

/// Writes the drill files of a board.
#[must_use]
pub fn drill_files(
    pcb: &Pcb,
    options: DrillOptions,
    stem: &str,
    version: &str,
    created: &str,
) -> Vec<OutputFile> {
    let all = holes(pcb);
    let copper = usize::from(pcb.stackup.copper_count().max(1));
    let last = copper - 1;
    let mut files = Vec::new();
    let through = |h: &&Hole| h.span == (0, last);
    let plated: Vec<&Hole> = all.iter().filter(|h| h.plated).filter(through).collect();
    let unplated: Vec<&Hole> = all.iter().filter(|h| !h.plated).collect();
    if options.merge {
        let merged: Vec<&Hole> = plated.iter().chain(unplated.iter()).copied().collect();
        files.push(OutputFile::text(
            format!("{stem}.drl"),
            file(
                &merged,
                &format!("MixedPlating,1,{copper}"),
                version,
                created,
            ),
        ));
    } else {
        files.push(OutputFile::text(
            format!("{stem}-PTH.drl"),
            file(&plated, &format!("Plated,1,{copper},PTH"), version, created),
        ));
        files.push(OutputFile::text(
            format!("{stem}-NPTH.drl"),
            file(
                &unplated,
                &format!("NonPlated,1,{copper},NPTH"),
                version,
                created,
            ),
        ));
    }
    // Blind and buried vias: one file per layer pair.
    let mut pairs: BTreeMap<(usize, usize), Vec<&Hole>> = BTreeMap::new();
    for hole in all.iter().filter(|h| h.plated && h.span != (0, last)) {
        pairs.entry(hole.span).or_default().push(hole);
    }
    for ((a, b), list) in pairs {
        let kind = if a == 0 || b == last {
            "Blind"
        } else {
            "Buried"
        };
        files.push(OutputFile::text(
            format!("{stem}-L{}-L{}.drl", a + 1, b + 1),
            file(
                &list,
                &format!("Plated,{},{},{kind}", a + 1, b + 1),
                version,
                created,
            ),
        ));
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinates_have_few_decimals() {
        assert_eq!(coord(101_460_000.0), "101.46");
        assert_eq!(coord(-126_000_000.0), "-126.0");
        assert_eq!(coord(-0.2), "0.0");
        assert_eq!(coord(1_234_567.0), "1.2346");
    }

    #[test]
    fn tools_group_holes_by_size_and_function() {
        let hole = |x: f64, d: f64, kind: HoleKind| Hole {
            start: V::new(x, 0.0),
            end: None,
            diameter: d,
            plated: kind != HoleKind::Mechanical,
            kind,
            span: (0, 1),
        };
        let a = hole(2e6, 4e5, HoleKind::Via);
        let b = hole(1e6, 4e5, HoleKind::Via);
        let c = hole(0.0, 1e6, HoleKind::Component);
        let slot = Hole {
            end: Some(V::new(0.0, 2e6)),
            ..hole(5e6, 1e6, HoleKind::Component)
        };
        let out = file(&[&a, &b, &c, &slot], "Plated,1,2,PTH", "1.0", "");
        assert!(out.contains("T1C0.400\n"), "{out}");
        assert!(out.contains("T2C1.000\n"));
        assert!(out.contains("T1\nX1.0Y0.0\nX2.0Y0.0\n"));
        assert!(out.contains("X5.0Y0.0G85X0.0Y2.0"));
        assert!(out.contains("; #@! TA.AperFunction,Plated,PTH,ViaDrill\nT1C"));
        assert!(out.ends_with("M30\n"));
    }
}
