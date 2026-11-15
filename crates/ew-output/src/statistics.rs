//! Design statistics: what a design consists of, in numbers — for the
//! project details, the documentation generator and `ewerk project stats`.

use std::collections::BTreeMap;

use ew_model::document::{Document, DocumentKind};
use ew_model::layer::Side;
use ew_model::pcb::{Mounting, PadKind, Pcb, Track};
use ew_model::project::Project;
use ew_model::units::Nm;
use serde_json::{Value, json};

#[allow(
    clippy::cast_precision_loss,
    reason = "lengths are reported in millimetres"
)]
fn mm(value: Nm) -> f64 {
    (value as f64 / 1e6 * 1000.0).round() / 1000.0
}

fn round(value: f64, places: i32) -> f64 {
    let factor = 10_f64.powi(places);
    (value * factor).round() / factor
}

/// Length of a track in nanometres (arcs through their mid point).
#[allow(clippy::cast_precision_loss, reason = "lengths in floating point")]
fn track_length(track: &Track) -> f64 {
    let (ax, ay) = (track.start.x as f64, track.start.y as f64);
    let (bx, by) = (track.end.x as f64, track.end.y as f64);
    let Some(mid) = track.mid else {
        return (bx - ax).hypot(by - ay);
    };
    let (mx, my) = (mid.x as f64, mid.y as f64);
    // The circle through the three points.
    let d = 2.0 * (ax * (my - by) + mx * (by - ay) + bx * (ay - my));
    if d.abs() < 1e-6 {
        return (mx - ax).hypot(my - ay) + (bx - mx).hypot(by - my);
    }
    let ux = ((ax * ax + ay * ay) * (my - by)
        + (mx * mx + my * my) * (by - ay)
        + (bx * bx + by * by) * (ay - my))
        / d;
    let uy = ((ax * ax + ay * ay) * (bx - mx)
        + (mx * mx + my * my) * (ax - bx)
        + (bx * bx + by * by) * (mx - ax))
        / d;
    let radius = (ax - ux).hypot(ay - uy);
    let angle = |x: f64, y: f64| (y - uy).atan2(x - ux);
    let (a0, a1, a2) = (angle(ax, ay), angle(mx, my), angle(bx, by));
    let tau = std::f64::consts::TAU;
    let sweep = |from: f64, to: f64| (to - from).rem_euclid(tau);
    // The sweep from start to end that passes the mid point.
    let total = if sweep(a0, a1) <= sweep(a0, a2) {
        sweep(a0, a2)
    } else {
        tau - sweep(a0, a2)
    };
    radius * total
}

fn schematic(project: &Project) -> Value {
    let connectivity = ew_schematic::analyze(project);
    let sheets = project.documents_of(DocumentKind::Schematic).count();
    let parts: std::collections::BTreeSet<&str> = connectivity
        .pins
        .iter()
        .filter(|pin| !pin.power_symbol)
        .map(|pin| pin.designator.as_str())
        .collect();
    let pins = connectivity.pins.iter().filter(|p| !p.power_symbol).count();
    let nets = connectivity
        .nets
        .iter()
        .filter(|n| !n.pins.is_empty())
        .count();
    let named = connectivity
        .nets
        .iter()
        .filter(|n| n.named && !n.pins.is_empty())
        .count();
    let single = connectivity
        .nets
        .iter()
        .filter(|n| {
            n.pins
                .iter()
                .filter(|i| connectivity.pins.get(**i).is_some_and(|p| !p.power_symbol))
                .count()
                == 1
        })
        .count();
    json!({
        "sheets": sheets,
        "parts": parts.len(),
        "pins": pins,
        "nets": nets,
        "namedNets": named,
        "singlePinNets": single,
    })
}

fn bom(project: &Project, board: Option<&Pcb>) -> Value {
    let parts = crate::bom::parts(project, board);
    let mut lines: BTreeMap<(String, String, String), usize> = BTreeMap::new();
    let mut dnp = 0;
    let mut no_footprint = 0;
    let mut no_mpn = 0;
    for part in &parts {
        if part.dnp {
            dnp += 1;
            continue;
        }
        let mpn = part
            .properties
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("MPN"))
            .map(|(_, value)| value.clone())
            .unwrap_or_default();
        if part.footprint.is_empty() {
            no_footprint += 1;
        }
        if mpn.is_empty() {
            no_mpn += 1;
        }
        *lines
            .entry((part.value.clone(), part.footprint.clone(), mpn))
            .or_default() += 1;
    }
    json!({
        "lines": lines.len(),
        "parts": parts.len() - dnp,
        "dnp": dnp,
        "withoutFootprint": no_footprint,
        "withoutPartNumber": no_mpn,
    })
}

#[allow(clippy::too_many_lines, reason = "one pass over the board")]
#[allow(clippy::cast_precision_loss, reason = "areas and densities")]
fn board(id: ew_model::id::Id, pcb: &Pcb) -> Value {
    let outline = ew_pcb::items::outline(pcb);
    let size = outline.rings.first().map(|ring| {
        let bbox = ew_pcb::geom::BBox::of_points(ring);
        (bbox.max.x - bbox.min.x, bbox.max.y - bbox.min.y)
    });
    // Area of the board itself (cut-outs subtracted).
    let area = if outline.rings.is_empty() {
        None
    } else {
        let mut total = 0.0;
        for (index, ring) in outline.rings.iter().enumerate() {
            let a = ew_pcb::geom::signed_area2(ring).abs() / 2.0;
            total += if index == 0 { a } else { -a };
        }
        Some(total / 1e14) // nm² → cm²
    };
    let mut footprints = BTreeMap::from([
        ("total", 0usize),
        ("top", 0),
        ("bottom", 0),
        ("smd", 0),
        ("throughHole", 0),
        ("other", 0),
    ]);
    let (mut pads, mut smd_pads, mut tht_pads) = (0usize, 0usize, 0usize);
    let mut drills: BTreeMap<Nm, (usize, usize)> = BTreeMap::new();
    let mut smallest_hole: Option<Nm> = None;
    for fp in &pcb.footprints {
        *footprints.entry("total").or_default() += 1;
        *footprints
            .entry(if fp.side == Side::Bottom {
                "bottom"
            } else {
                "top"
            })
            .or_default() += 1;
        *footprints
            .entry(match fp.attributes.mounting {
                Mounting::Smd => "smd",
                Mounting::ThroughHole => "throughHole",
                Mounting::Other => "other",
            })
            .or_default() += 1;
        for pad in &fp.pads {
            pads += 1;
            match pad.kind {
                PadKind::Smd | PadKind::Connector => smd_pads += 1,
                PadKind::ThroughHole | PadKind::NpThroughHole => tht_pads += 1,
            }
            if let Some(drill) = &pad.drill {
                let entry = drills.entry(drill.diameter).or_default();
                if pad.kind == PadKind::NpThroughHole {
                    entry.1 += 1;
                } else {
                    entry.0 += 1;
                }
                smallest_hole =
                    Some(smallest_hole.map_or(drill.diameter, |s| s.min(drill.diameter)));
            }
        }
    }
    let mut smallest_via: Option<Nm> = None;
    for via in &pcb.vias {
        drills.entry(via.drill).or_default().0 += 1;
        smallest_via = Some(smallest_via.map_or(via.drill, |s| s.min(via.drill)));
        smallest_hole = Some(smallest_hole.map_or(via.drill, |s| s.min(via.drill)));
    }
    let mut lengths: BTreeMap<String, f64> = BTreeMap::new();
    let mut smallest_track: Option<Nm> = None;
    let mut total_length = 0.0;
    for track in &pcb.tracks {
        let length = track_length(track);
        total_length += length;
        *lengths.entry(track.layer.to_string()).or_default() += length;
        smallest_track = Some(smallest_track.map_or(track.width, |s| s.min(track.width)));
    }
    let order: Vec<String> = pcb.stackup.copper_layers().map(|l| l.to_string()).collect();
    let layers: Vec<Value> = order
        .iter()
        .map(|layer| {
            json!({
                "layer": layer,
                "length": round(lengths.get(layer).copied().unwrap_or(0.0) / 1e6, 1),
            })
        })
        .collect();
    let plated: usize = drills.values().map(|(p, _)| p).sum();
    let non_plated: usize = drills.values().map(|(_, n)| n).sum();
    // Connections: every pad of a net but one per net needs a connection.
    let mut net_pads: BTreeMap<ew_model::id::Id, usize> = BTreeMap::new();
    for fp in &pcb.footprints {
        for pad in &fp.pads {
            if let Some(net) = pad.net {
                *net_pads.entry(net).or_default() += 1;
            }
        }
    }
    let connections: usize = net_pads.values().map(|n| n.saturating_sub(1)).sum();
    let items = ew_pcb::items::Board::new(pcb);
    let unrouted = ew_pcb::connectivity::analyze(&items).airwires.len();
    let completion = if connections == 0 {
        1.0
    } else {
        1.0 - (unrouted.min(connections) as f64 / connections as f64)
    };
    json!({
        "board": pcb.name,
        "id": id,
        "size": size.map(|(w, h)| [round(w / 1e6, 2), round(h / 1e6, 2)]),
        "area": area.map(|a| round(a, 2)),
        "copperLayers": pcb.stackup.copper_count(),
        "thickness": mm(pcb.stackup.thickness()),
        "footprints": footprints,
        "pads": {"total": pads, "smd": smd_pads, "throughHole": tht_pads},
        "holes": {
            "plated": plated,
            "nonPlated": non_plated,
            "drills": drills
                .iter()
                .map(|(diameter, (p, n))| json!({"diameter": mm(*diameter), "plated": p, "nonPlated": n}))
                .collect::<Vec<_>>(),
        },
        "vias": pcb.vias.len(),
        "zones": pcb.zones.len(),
        "tracks": {
            "segments": pcb.tracks.len(),
            "length": round(total_length / 1e6, 1),
            "layers": layers,
        },
        "smallest": {
            "track": smallest_track.map(mm),
            "via": smallest_via.map(mm),
            "hole": smallest_hole.map(mm),
        },
        "density": area.filter(|a| *a > 0.0).map(|a| json!({
            "parts": round(footprints["total"] as f64 / a, 2),
            "pads": round(pads as f64 / a, 2),
        })),
        "routing": {
            "connections": connections,
            "unrouted": unrouted,
            "completion": round(completion, 3),
        },
    })
}

/// The statistics of a project: schematic, bill of materials and boards.
#[must_use]
pub fn statistics(project: &Project) -> Value {
    let boards: Vec<(ew_model::id::Id, &Pcb)> = project
        .documents_of(DocumentKind::Pcb)
        .filter_map(|entry| match project.document(entry.id) {
            Some(Document::Pcb(pcb)) => Some((entry.id, pcb)),
            _ => None,
        })
        .collect();
    json!({
        "schematic": schematic(project),
        "bom": bom(project, boards.first().map(|(_, pcb)| *pcb)),
        "boards": boards.iter().map(|(id, pcb)| board(*id, pcb)).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use ew_model::id::Id;
    use ew_model::layer::CopperLayer;
    use ew_model::units::Point;

    use super::*;

    fn track(start: Point, end: Point, mid: Option<Point>) -> Track {
        Track {
            id: Id::random(),
            layer: CopperLayer::Top,
            net: None,
            width: 250_000,
            start,
            end,
            mid,
            locked: false,
        }
    }

    #[test]
    fn measures_straight_and_arc_tracks() {
        let straight = track(Point::new(0, 0), Point::new(3_000_000, 4_000_000), None);
        assert!((track_length(&straight) - 5e6).abs() < 1.0);
        // A half circle of radius 1 mm: π mm.
        let arc = track(
            Point::new(-1_000_000, 0),
            Point::new(1_000_000, 0),
            Some(Point::new(0, 1_000_000)),
        );
        assert!((track_length(&arc) - std::f64::consts::PI * 1e6).abs() < 10.0);
        // A quarter circle the other way round.
        let quarter = track(
            Point::new(1_000_000, 0),
            Point::new(0, -1_000_000),
            Some(Point::new(707_107, -707_107)),
        );
        assert!((track_length(&quarter) - std::f64::consts::FRAC_PI_2 * 1e6).abs() < 10.0);
    }
}
