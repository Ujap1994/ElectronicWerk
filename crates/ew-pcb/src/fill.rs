//! The zone filler.
//!
//! Zones fill in order of priority (higher first). For every layer of a
//! zone, the filled copper is the zone area on the board (kept away from
//! the edge), minus all copper of other nets and the fills of
//! higher-priority zones grown by the clearance, minus non-plated holes and
//! keep-out areas. Pads of the zone's net connect solidly or with thermal
//! reliefs (a gap around the pad and spokes across it). Necks thinner than
//! the minimum width are removed, and islands without a connection to the
//! zone's net are dropped.

use std::collections::{HashMap, HashSet};

use ew_model::id::Id;
use ew_model::layer::CopperLayer;
use ew_model::pcb::{FilledArea, IslandRemoval, PadConnection, Pcb, Zone};
use ew_model::rules::Rules;
use rayon::prelude::*;

use crate::geom::{BBox, Shape, V};
use crate::items::{Board, CopperItem, ItemRef, footprint_xform};
use crate::poly::{self, Polygons};
use crate::rules::{Props, RuleSet, ZoneTest};

/// Chord error of the filled outlines (5 µm).
const TOLERANCE: f64 = 5_000.0;

struct NoZones;

impl ZoneTest for NoZones {
    fn inside(&self, _name: &str, _p: V) -> bool {
        false
    }
}

/// The new fill of one zone.
#[derive(Debug, Clone, PartialEq)]
pub struct Filled {
    /// The zone.
    pub zone: Id,
    /// Its filled areas.
    pub areas: Vec<FilledArea>,
}

/// Fills the zones of a board (all of them, or only `only`), returning the
/// new fills in priority order.
#[must_use]
pub fn fill(pcb: &Pcb, rules: Option<&Rules>, only: Option<&[Id]>) -> Vec<Filled> {
    let board = Board::new(pcb);
    let mut set = RuleSet::new(rules);
    set.cache_classes(pcb.nets.iter().map(|n| (n.name.clone(), n.class.clone())));
    let wanted: HashSet<Id> = only.map_or_else(
        || pcb.zones.iter().map(|z| z.id).collect(),
        |ids| ids.iter().copied().collect(),
    );
    let mut zones: Vec<&Zone> = pcb
        .zones
        .iter()
        .filter(|z| z.keepout.is_none() && z.outline.len() >= 3 && wanted.contains(&z.id))
        .collect();
    zones.sort_by(|a, b| b.priority.cmp(&a.priority).then(a.id.cmp(&b.id)));
    let nets: HashMap<Id, (&str, String)> = pcb
        .nets
        .iter()
        .map(|n| (n.id, (n.name.as_str(), set.class_name(&n.name, &n.class))))
        .collect();
    let filler = Filler {
        board: &board,
        rules: &set,
        nets: &nets,
        refilled: zones.iter().map(|z| z.id).collect(),
    };
    // Layers fill independently (in parallel); on one layer, zones fill in
    // priority order because higher fills knock out lower ones.
    let layers: Vec<CopperLayer> = board
        .stack
        .iter()
        .copied()
        .filter(|layer| zones.iter().any(|z| z.layers.contains(layer)))
        .collect();
    let per_layer: Vec<Vec<(Id, CopperLayer, Polygons)>> = layers
        .par_iter()
        .map(|layer| {
            let mut made: Vec<(Id, Option<Id>, u32, CopperLayer, Polygons)> = Vec::new();
            for zone in zones.iter().filter(|z| z.layers.contains(layer)) {
                let polygons = filler.layer(zone, *layer, &made);
                made.push((zone.id, zone.net, zone.priority, *layer, polygons));
            }
            made.into_iter()
                .map(|(id, _, _, layer, polygons)| (id, layer, polygons))
                .collect()
        })
        .collect();
    let mut by_zone: HashMap<Id, Vec<FilledArea>> = HashMap::new();
    for (id, layer, polygons) in per_layer.into_iter().flatten() {
        let areas = by_zone.entry(id).or_default();
        for polygon in &polygons {
            let mut rings = polygon
                .iter()
                .map(|ring| ring.iter().map(|p| p.point()).collect());
            let Some(points) = rings.next() else {
                continue;
            };
            areas.push(FilledArea {
                layer,
                points,
                holes: rings.collect(),
            });
        }
    }
    zones
        .iter()
        .map(|zone| Filled {
            zone: zone.id,
            areas: by_zone.remove(&zone.id).unwrap_or_default(),
        })
        .collect()
}

struct Filler<'a> {
    board: &'a Board<'a>,
    rules: &'a RuleSet,
    nets: &'a HashMap<Id, (&'a str, String)>,
    refilled: HashSet<Id>,
}

impl Filler<'_> {
    fn props(&self, net: Option<Id>, kind: &'static str, layer: CopperLayer) -> Props<'_> {
        let (name, class) = net
            .and_then(|id| self.nets.get(&id))
            .map_or(("", ew_model::rules::DEFAULT_NET_CLASS), |(n, c)| {
                (*n, c.as_str())
            });
        Props {
            kind,
            net: name,
            netclass: class,
            layer: Some(layer),
            ..Props::default()
        }
    }

    fn clearance(&self, zone: &Zone, layer: CopperLayer, item: &CopperItem) -> f64 {
        let a = self.props(zone.net, "zone", layer);
        let mut b = self.props(item.net, item.item.kind(), layer);
        b.at = item.shape.anchor();
        let rule = self.rules.clearance(&a, &b, &NoZones).value;
        let own = match item.item {
            ItemRef::Pad { footprint, index } => self
                .board
                .footprint(footprint)
                .and_then(|fp| fp.pads.get(index))
                .and_then(|pad| pad.clearance)
                .map_or(0.0, |c| c as f64),
            _ => 0.0,
        };
        rule.max(zone.fill.clearance as f64).max(own)
    }

    #[allow(clippy::too_many_lines, reason = "the fill steps read best in order")]
    fn layer(
        &self,
        zone: &Zone,
        layer: CopperLayer,
        made: &[(Id, Option<Id>, u32, CopperLayer, Polygons)],
    ) -> Polygons {
        let Some(bit) = self.board.layer_bit(layer) else {
            return Vec::new();
        };
        let outline: Vec<V> = zone.outline.iter().map(|p| V::of(*p)).collect();
        let holes: Vec<Vec<V>> = zone
            .holes
            .iter()
            .map(|h| h.iter().map(|p| V::of(*p)).collect())
            .collect();
        let mut area = poly::union(&[std::iter::once(outline).chain(holes).collect()]);
        // Stay on the board, away from its edge.
        if let Some(edge) = &self.board.outline.area {
            let rings: Vec<Vec<V>> = match &edge.core {
                crate::geom::Core::Poly(p) => p.rings().to_vec(),
                _ => Vec::new(),
            };
            let props = self.props(zone.net, "zone", layer);
            let gap = self.rules.edge_clearance(&props, &NoZones).value;
            let inside = poly::offset(&[rings], -gap, TOLERANCE);
            area = poly::intersection(&area, &inside);
        }
        if area.is_empty() {
            return Vec::new();
        }
        let bbox = area
            .iter()
            .flat_map(|p| p.iter().flatten())
            .fold(BBox::EMPTY, |b, p| b.with(*p));
        let reach = self.rules.max_clearance().max(zone.fill.clearance as f64)
            + zone.fill.thermal_gap as f64;
        let mut obstacles: Polygons = Vec::new();
        let mut thermal: Vec<usize> = Vec::new();
        let mut anchors: Vec<usize> = Vec::new();
        for index in self.board.copper_near(bbox.grow(reach)) {
            let item = &self.board.copper[index];
            if item.layers & bit == 0 {
                continue;
            }
            if let ItemRef::Zone { id, .. } = item.item {
                // Fills of zones filled in this run come from `made`.
                if self.refilled.contains(&id) || id == zone.id {
                    continue;
                }
            }
            let same_net = zone.net.is_some() && item.net == zone.net;
            if same_net {
                match item.item {
                    ItemRef::Pad { .. } => match zone.fill.pad_connection {
                        PadConnection::Solid => anchors.push(index),
                        PadConnection::Thermal => {
                            thermal.push(index);
                            anchors.push(index);
                        }
                        PadConnection::None => {
                            let clearance = self.clearance(zone, layer, item);
                            obstacles.extend(item.shape.outlines(clearance, TOLERANCE));
                        }
                    },
                    ItemRef::Track { .. } | ItemRef::Via { .. } => anchors.push(index),
                    ItemRef::Zone { .. } => {}
                }
                continue;
            }
            let clearance = self.clearance(zone, layer, item);
            obstacles.extend(item.shape.outlines(clearance, TOLERANCE));
        }
        // Fills of higher-priority zones of other nets.
        for (other, net, priority, own_layer, polygons) in made {
            if *other == zone.id || *own_layer != layer || *priority <= zone.priority {
                continue;
            }
            if zone.net.is_some() && *net == zone.net {
                continue;
            }
            let clearance = zone.fill.clearance as f64;
            obstacles.extend(poly::offset(polygons, clearance, TOLERANCE));
        }
        // Non-plated holes.
        for index in self.board.holes_near(bbox.grow(reach)) {
            let hole = &self.board.holes[index];
            if !hole.plated {
                obstacles.extend(hole.shape.outlines(zone.fill.clearance as f64, TOLERANCE));
            }
        }
        // Keep-out areas that forbid pours.
        for keepout in &self.board.keepouts {
            if keepout.rules.pours && keepout.layers & bit != 0 {
                obstacles.extend(keepout.shape.outlines(0.0, TOLERANCE));
            }
        }
        let base = poly::difference(&area, &obstacles);
        if base.is_empty() {
            return Vec::new();
        }
        // Thermal reliefs: a gap around the pad, then spokes across it.
        let gap = zone.fill.thermal_gap as f64;
        let knockouts: Polygons = thermal
            .iter()
            .flat_map(|&i| self.board.copper[i].shape.outlines(gap, TOLERANCE))
            .collect();
        let mut result = poly::difference(&base, &knockouts);
        let min_width = zone.fill.min_width as f64;
        if min_width > 1.0 {
            // Opening: shrink, then grow back. Shrinking by the chord error
            // more keeps the result inside the original despite the
            // approximated round corners.
            let shrunk = poly::offset(&result, -(min_width / 2.0 + TOLERANCE), TOLERANCE);
            result = poly::offset(&shrunk, min_width / 2.0, TOLERANCE);
        }
        if !thermal.is_empty() {
            let spokes: Polygons = thermal.iter().flat_map(|&i| self.spokes(zone, i)).collect();
            let spokes = poly::intersection(&spokes, &base);
            let mut all = result;
            all.extend(spokes);
            result = poly::union(&all);
        }
        if let Some((width, gap)) = zone.fill.hatch.map(|h| (h.w as f64, h.h as f64)) {
            result = hatch(&result, width, gap);
        }
        self.remove_islands(zone, &result, &anchors)
    }

    /// Four spokes from the centre of a pad across the thermal gap.
    fn spokes(&self, zone: &Zone, index: usize) -> Polygons {
        let item = &self.board.copper[index];
        let ItemRef::Pad {
            footprint,
            index: pad_index,
        } = item.item
        else {
            return Vec::new();
        };
        let Some(fp) = self.board.footprint(footprint) else {
            return Vec::new();
        };
        let Some(pad) = fp.pads.get(pad_index) else {
            return Vec::new();
        };
        let xf = footprint_xform(fp);
        let rotation = crate::geom::radians(pad.rotation);
        let at = V::of(pad.at);
        let center = xf.apply(at);
        let half = pad.size.w.max(pad.size.h) as f64 / 2.0;
        let reach = half + zone.fill.thermal_gap as f64 + zone.fill.min_width.max(1) as f64;
        let w = zone.fill.thermal_spoke_width as f64 / 2.0;
        let mut out = Vec::new();
        for d in [
            V::new(1.0, 0.0),
            V::new(-1.0, 0.0),
            V::new(0.0, 1.0),
            V::new(0.0, -1.0),
        ] {
            let dir = xf.apply(at.add(d.rotate(rotation))).sub(center).unit();
            let side = dir.perp().scale(w);
            let end = center.add(dir.scale(reach));
            out.push(vec![vec![
                center.add(side),
                end.add(side),
                end.sub(side),
                center.sub(side),
            ]]);
        }
        out
    }

    fn remove_islands(&self, zone: &Zone, polygons: &Polygons, anchors: &[usize]) -> Polygons {
        let rule = if zone.net.is_none() {
            IslandRemoval::Never
        } else {
            zone.fill.island_removal
        };
        if rule == IslandRemoval::Never {
            return polygons.clone();
        }
        polygons
            .iter()
            .filter(|polygon| {
                let shape = Shape::polygon(polygon[0].clone(), polygon[1..].to_vec(), 0.0);
                let connected = anchors
                    .iter()
                    .any(|&i| shape.touches(&self.board.copper[i].shape));
                if connected {
                    return true;
                }
                match rule {
                    IslandRemoval::BelowArea(limit) => {
                        poly::area(std::slice::from_ref(*polygon)) >= limit as f64
                    }
                    _ => false,
                }
            })
            .cloned()
            .collect()
    }
}

/// A hatched fill: square openings of `gap` at a pitch of `width + gap`,
/// with a solid border of `width`.
fn hatch(polygons: &Polygons, width: f64, gap: f64) -> Polygons {
    if width <= 0.0 || gap <= 0.0 || polygons.is_empty() {
        return polygons.clone();
    }
    let bbox = polygons
        .iter()
        .flat_map(|p| p.iter().flatten())
        .fold(BBox::EMPTY, |b, p| b.with(*p));
    let pitch = width + gap;
    let mut squares: Polygons = Vec::new();
    let mut y = (bbox.min.y / pitch).floor() * pitch + width;
    while y < bbox.max.y {
        let mut x = (bbox.min.x / pitch).floor() * pitch + width;
        while x < bbox.max.x {
            squares.push(vec![vec![
                V::new(x, y),
                V::new(x + gap, y),
                V::new(x + gap, y + gap),
                V::new(x, y + gap),
            ]]);
            x += pitch;
        }
        y += pitch;
    }
    let border = poly::difference(polygons, &poly::offset(polygons, -width, TOLERANCE));
    let mut all = poly::difference(polygons, &squares);
    all.extend(border);
    poly::union(&all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectivity::tests::{board_with, pad, track};
    use ew_model::layer::Layer;
    use ew_model::pcb::{BoardShape, Keepout, Net, ZoneFill};
    use ew_model::shape::Geometry;
    use ew_model::units::{Point, um};

    fn zone(net: Option<Id>, priority: u32) -> Zone {
        Zone {
            id: Id::random(),
            name: "GND".into(),
            net,
            layers: vec![CopperLayer::Top],
            outline: vec![
                Point::mm(-10, -10),
                Point::mm(30, -10),
                Point::mm(30, 20),
                Point::mm(-10, 20),
            ],
            holes: Vec::new(),
            priority,
            keepout: None,
            fill: ZoneFill::default(),
            filled: Vec::new(),
        }
    }

    fn setup() -> (Pcb, Id, Id) {
        let (gnd, vcc) = (Id::random(), Id::random());
        let mut pcb = board_with(Vec::new());
        for (id, name) in [(gnd, "GND"), (vcc, "VCC")] {
            pcb.nets.insert(Net {
                id,
                name: name.into(),
                class: String::new(),
            });
        }
        let fp = pcb.footprints.iter().next().unwrap().id;
        let mut fp_obj = pcb.footprints.get(fp).unwrap().clone();
        fp_obj.pads = vec![
            pad("1", Point::mm(0, 0), Some(gnd)),
            pad("2", Point::mm(10, 0), Some(vcc)),
        ];
        pcb.footprints.insert(fp_obj);
        pcb.shapes.insert(BoardShape {
            id: Id::random(),
            layer: Layer::Outline,
            geometry: Geometry::Rect {
                start: Point::mm(-5, -5),
                end: Point::mm(25, 15),
            },
            width: um(100),
            filled: false,
        });
        (pcb, gnd, vcc)
    }

    #[test]
    fn a_pour_keeps_clear_of_other_nets_and_passes_the_drc() {
        let (mut pcb, gnd, vcc) = setup();
        pcb.tracks.insert(track(
            vcc,
            Point::mm(10, 0),
            Point::mm(20, 0),
            CopperLayer::Top,
        ));
        let z = zone(Some(gnd), 0);
        let id = z.id;
        pcb.zones.insert(z);
        let fills = fill(&pcb, None, None);
        assert_eq!(fills.len(), 1);
        let areas = &fills[0].areas;
        assert_ne!(areas.len(), 0);
        let mut filled = pcb.zones.get(id).unwrap().clone();
        filled.filled.clone_from(areas);
        pcb.zones.insert(filled);
        let report = crate::drc::check(&pcb, None, &crate::drc::Options::default());
        let problems: Vec<&str> = report
            .findings
            .iter()
            .map(|f| f.code.as_str())
            .filter(|c| ["clearance", "short", "copper-to-edge"].contains(c))
            .collect();
        assert_eq!(problems, Vec::<&str>::new(), "{:#?}", report.findings);
        // The GND pad connects through the thermal spokes; the fill holds the
        // pad's net: no unrouted GND connection.
        assert!(!report.findings.iter().any(|f| f.code == "unrouted"));
        // The fill stays inside the board, 0.3 mm from the edge.
        let bbox = areas
            .iter()
            .flat_map(|a| a.points.iter())
            .fold(BBox::EMPTY, |b, p| b.with(V::of(*p)));
        assert!(bbox.min.x >= -4.71e6 && bbox.max.x <= 24.71e6, "{bbox:?}");
    }

    #[test]
    fn priorities_keepouts_and_islands() {
        let (mut pcb, gnd, vcc) = setup();
        let mut low = zone(Some(gnd), 0);
        low.fill.pad_connection = PadConnection::Solid;
        let mut high = zone(Some(vcc), 5);
        high.fill.island_removal = IslandRemoval::Never;
        high.outline = vec![
            Point::mm(15, -10),
            Point::mm(30, -10),
            Point::mm(30, 20),
            Point::mm(15, 20),
        ];
        let mut keepout = zone(None, 0);
        keepout.keepout = Some(Keepout {
            pours: true,
            ..Keepout::default()
        });
        keepout.outline = vec![
            Point::mm(-10, 5),
            Point::mm(30, 5),
            Point::mm(30, 6),
            Point::mm(-10, 6),
        ];
        let (low_id, high_id) = (low.id, high.id);
        pcb.zones.insert(low);
        pcb.zones.insert(high);
        pcb.zones.insert(keepout);
        let fills = fill(&pcb, None, None);
        assert_eq!(fills[0].zone, high_id, "higher priority fills first");
        let low_fill = &fills.iter().find(|f| f.zone == low_id).unwrap().areas;
        // The keep-out strip cuts the board in two; the upper part (y > 6 mm)
        // has no GND pad and is removed as an island.
        assert_eq!(low_fill.len(), 1);
        assert!(low_fill[0].points.iter().all(|p| p.y <= 5_000_000));
        // The low zone stays clear of the high zone's fill (x ≥ 15 mm).
        let max_x = low_fill[0].points.iter().map(|p| p.x).max().unwrap();
        assert!(max_x < 15_000_000 - 200_000, "{max_x}");
    }

    #[test]
    fn hatched_fills_have_openings() {
        let square = vec![vec![
            V::new(0.0, 0.0),
            V::new(10e6, 0.0),
            V::new(10e6, 10e6),
            V::new(0.0, 10e6),
        ]];
        let hatched = hatch(&vec![square], 0.5e6, 1e6);
        let area = poly::area(&hatched);
        assert!(area < 100e12 * 0.75 && area > 100e12 * 0.3, "{area}");
    }
}
