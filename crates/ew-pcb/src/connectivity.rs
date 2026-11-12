//! Board connectivity: which copper touches, clusters of connected copper
//! per net, shorts and the unrouted connections (ratsnest).

use std::collections::HashMap;

use ew_model::id::Id;
use rayon::prelude::*;
use serde::Serialize;

use crate::geom::V;
use crate::items::{Board, ItemRef};

/// A union-find over item indices.
struct Sets {
    parent: Vec<usize>,
}

impl Sets {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
        }
    }

    fn find(&mut self, mut i: usize) -> usize {
        while self.parent[i] != i {
            self.parent[i] = self.parent[self.parent[i]];
            i = self.parent[i];
        }
        i
    }

    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.parent[a.max(b)] = a.min(b);
        }
    }
}

/// An unrouted connection between two pieces of copper of one net.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Airwire {
    /// The net.
    pub net: Id,
    /// Start (on the first item).
    pub from: [f64; 2],
    /// End (on the second item).
    pub to: [f64; 2],
    /// The item at the start.
    pub from_item: ItemRef,
    /// The item at the end.
    pub to_item: ItemRef,
}

/// Two touching items of different nets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Short {
    /// First copper item (index).
    pub a: usize,
    /// Second copper item (index).
    pub b: usize,
    /// A shared copper layer bit.
    pub layer: u64,
}

/// The connectivity of a board.
#[derive(Debug, Clone, Default)]
pub struct Connectivity {
    /// Cluster of every copper item.
    pub cluster: Vec<usize>,
    /// For every copper item: the other items it touches.
    pub touching: Vec<Vec<usize>>,
    /// Touching items of different nets.
    pub shorts: Vec<Short>,
    /// Unrouted connections.
    pub airwires: Vec<Airwire>,
}

impl Connectivity {
    /// Number of unrouted connections of a net.
    #[must_use]
    pub fn unrouted(&self, net: Id) -> usize {
        self.airwires.iter().filter(|a| a.net == net).count()
    }
}

/// Analyses the connectivity of a board.
#[must_use]
pub fn analyze(board: &Board<'_>) -> Connectivity {
    let n = board.copper.len();
    // Touching pairs (i < j), found in parallel.
    let pairs: Vec<(usize, usize, u64, bool)> = (0..n)
        .into_par_iter()
        .flat_map_iter(|i| {
            let item = &board.copper[i];
            board
                .copper_near(item.bbox.grow(1.0))
                .filter_map(move |j| {
                    if j <= i {
                        return None;
                    }
                    let other = &board.copper[j];
                    let shared = item.layers & other.layers;
                    if shared == 0 {
                        return None;
                    }
                    if item.item == other.item {
                        // Pieces of one arc track.
                        return Some((i, j, shared, false));
                    }
                    item.shape
                        .touches(&other.shape)
                        .then_some((i, j, shared, true))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let mut sets = Sets::new(n);
    let mut touching = vec![Vec::new(); n];
    let mut shorts = Vec::new();
    for (i, j, shared, distinct) in pairs {
        if !distinct {
            sets.join(i, j);
            continue;
        }
        touching[i].push(j);
        touching[j].push(i);
        match (board.copper[i].net, board.copper[j].net) {
            (Some(a), Some(b)) if a == b => sets.join(i, j),
            (Some(_), Some(_)) => shorts.push(Short {
                a: i,
                b: j,
                layer: shared & shared.wrapping_neg(),
            }),
            _ => {}
        }
    }
    let cluster: Vec<usize> = (0..n).map(|i| sets.find(i)).collect();
    let airwires = ratsnest(board, &cluster);
    Connectivity {
        cluster,
        touching,
        shorts,
        airwires,
    }
}

/// The minimum spanning tree between the clusters of every net that
/// contain pads.
fn ratsnest(board: &Board<'_>, cluster: &[usize]) -> Vec<Airwire> {
    // net → cluster → anchors (point, item)
    let mut nets: HashMap<Id, HashMap<usize, Vec<(V, ItemRef)>>> = HashMap::new();
    let mut has_pad: HashMap<usize, bool> = HashMap::new();
    for (index, item) in board.copper.iter().enumerate() {
        let Some(net) = item.net else {
            continue;
        };
        let root = cluster[index];
        if matches!(item.item, ItemRef::Pad { .. }) {
            has_pad.insert(root, true);
        }
        let anchors = nets.entry(net).or_default().entry(root).or_default();
        anchors.extend(item.anchors.iter().map(|p| (*p, item.item)));
    }
    let mut airwires = Vec::new();
    let mut order: Vec<Id> = nets.keys().copied().collect();
    order.sort();
    for net in order {
        let clusters = &nets[&net];
        let mut groups: Vec<&Vec<(V, ItemRef)>> = clusters
            .iter()
            .filter(|(root, anchors)| has_pad.contains_key(root) && !anchors.is_empty())
            .map(|(_, anchors)| anchors)
            .collect();
        if groups.len() < 2 {
            continue;
        }
        groups.sort_by(|a, b| {
            let (pa, pb) = (a[0].0, b[0].0);
            pa.x.total_cmp(&pb.x).then(pa.y.total_cmp(&pb.y))
        });
        airwires.extend(spanning_tree(net, &groups));
    }
    airwires
}

/// Prim's algorithm over clusters: the anchors of the tree keep, for every
/// other anchor, the nearest tree anchor.
fn spanning_tree(net: Id, groups: &[&Vec<(V, ItemRef)>]) -> Vec<Airwire> {
    struct Anchor {
        group: usize,
        at: V,
        item: ItemRef,
        best: f64,
        from: usize,
    }
    let mut anchors: Vec<Anchor> = groups
        .iter()
        .enumerate()
        .flat_map(|(group, list)| {
            list.iter().map(move |(at, item)| Anchor {
                group,
                at: *at,
                item: *item,
                best: f64::INFINITY,
                from: usize::MAX,
            })
        })
        .collect();
    let mut in_tree = vec![false; groups.len()];
    let mut wires = Vec::new();
    in_tree[0] = true;
    let mut fresh: Vec<usize> = (0..anchors.len())
        .filter(|i| anchors[*i].group == 0)
        .collect();
    for _ in 1..groups.len() {
        // Update the distances to the anchors that just joined the tree.
        for &t in &fresh {
            let at = anchors[t].at;
            for (index, anchor) in anchors.iter_mut().enumerate() {
                if in_tree[anchor.group] || index == t {
                    continue;
                }
                let d = anchor.at.dist(at);
                if d < anchor.best {
                    anchor.best = d;
                    anchor.from = t;
                }
            }
        }
        let closest = anchors
            .iter()
            .enumerate()
            .filter(|(_, a)| !in_tree[a.group])
            .min_by(|(_, a), (_, b)| a.best.total_cmp(&b.best))
            .map(|(index, _)| index);
        let Some(chosen) = closest else {
            break;
        };
        let group = anchors[chosen].group;
        let from = &anchors[anchors[chosen].from];
        wires.push(Airwire {
            net,
            from: from.at.pair(),
            to: anchors[chosen].at.pair(),
            from_item: from.item,
            to_item: anchors[chosen].item,
        });
        in_tree[group] = true;
        fresh = (0..anchors.len())
            .filter(|i| anchors[*i].group == group)
            .collect();
    }
    wires
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::items::Board;
    use ew_model::layer::{CopperLayer, Layer, LayerSpec};
    use ew_model::pcb::{
        FootprintInstance, Net, Pad, PadKind, PadShape, Pcb, STANDARD_THICKNESS, Stackup, Track,
        Via,
    };
    use ew_model::units::{Angle, Point, Size, mm, um};

    pub(crate) fn pad(number: &str, at: Point, net: Option<Id>) -> Pad {
        Pad {
            id: Id::random(),
            number: number.into(),
            net,
            kind: PadKind::Smd,
            shape: PadShape::Rect,
            at,
            rotation: Angle::ZERO,
            size: Size::new(mm(1), mm(1)),
            drill: None,
            layers: vec![
                LayerSpec::Layer(Layer::TOP_COPPER),
                LayerSpec::Layer(Layer::SolderMask(ew_model::layer::Side::Top)),
            ],
            clearance: None,
            mask_margin: None,
            paste_margin: None,
        }
    }

    pub(crate) fn board_with(pads: Vec<(Point, Option<Id>)>) -> Pcb {
        let mut pcb = Pcb::new(Id::random(), "B", Stackup::standard(2, STANDARD_THICKNESS));
        let fp = FootprintInstance {
            id: Id::random(),
            component: None,
            footprint: "X".into(),
            designator: "U1".into(),
            value: String::new(),
            at: Point::ORIGIN,
            rotation: Angle::ZERO,
            side: ew_model::layer::Side::Top,
            locked: false,
            pads: pads
                .into_iter()
                .enumerate()
                .map(|(i, (at, net))| pad(&(i + 1).to_string(), at, net))
                .collect(),
            graphics: Vec::new(),
            texts: Vec::new(),
            models: Vec::new(),
            properties: std::collections::BTreeMap::default(),
            attributes: ew_model::pcb::FootprintAttributes::default(),
        };
        pcb.footprints.insert(fp);
        pcb
    }

    pub(crate) fn track(net: Id, a: Point, b: Point, layer: CopperLayer) -> Track {
        Track {
            id: Id::random(),
            layer,
            net: Some(net),
            width: um(250),
            start: a,
            end: b,
            mid: None,
            locked: false,
        }
    }

    #[test]
    fn unrouted_pads_get_a_spanning_tree() {
        let gnd = Id::random();
        let pcb = board_with(vec![
            (Point::mm(0, 0), Some(gnd)),
            (Point::mm(10, 0), Some(gnd)),
            (Point::mm(20, 0), Some(gnd)),
            (Point::mm(0, 10), None),
        ]);
        let board = Board::new(&pcb);
        let c = analyze(&board);
        assert_eq!(c.airwires.len(), 2);
        let total: f64 = c
            .airwires
            .iter()
            .map(|a| V::new(a.from[0], a.from[1]).dist(V::new(a.to[0], a.to[1])))
            .sum();
        assert!((total - 20e6).abs() < 1.0, "{total}");
    }

    #[test]
    fn tracks_and_vias_connect_pads() {
        let gnd = Id::random();
        let vcc = Id::random();
        let mut pcb = board_with(vec![
            (Point::mm(0, 0), Some(gnd)),
            (Point::mm(10, 0), Some(gnd)),
            (Point::mm(20, 0), Some(gnd)),
            (Point::mm(0, 5), Some(vcc)),
        ]);
        pcb.nets.insert(Net {
            id: gnd,
            name: "GND".into(),
            class: String::new(),
        });
        // Pad 1 → pad 2 on top.
        pcb.tracks.insert(track(
            gnd,
            Point::mm(0, 0),
            Point::mm(10, 0),
            CopperLayer::Top,
        ));
        // Pad 2 → via → bottom → via → pad 3 can't connect: pad 3 is SMD on
        // top, so the track on the bottom ends at a via next to it.
        pcb.tracks.insert(track(
            gnd,
            Point::mm(10, 0),
            Point::mm(15, 0),
            CopperLayer::Top,
        ));
        pcb.vias.insert(Via {
            id: Id::random(),
            at: Point::mm(15, 0),
            diameter: um(600),
            drill: um(300),
            net: Some(gnd),
            from: CopperLayer::Top,
            to: CopperLayer::Bottom,
            micro: false,
            locked: false,
        });
        pcb.tracks.insert(track(
            gnd,
            Point::mm(15, 0),
            Point::mm(20, 0),
            CopperLayer::Bottom,
        ));
        let board = Board::new(&pcb);
        let c = analyze(&board);
        // Pad 3 is still unrouted: the airwire runs from the nearest anchor
        // of the routed cluster (the bottom track's end) to the pad.
        assert_eq!(c.airwires.len(), 1);
        let wire = &c.airwires[0];
        assert!(V::new(wire.from[0], wire.from[1]).dist(V::new(20e6, 0.0)) < 1.0);
        assert_eq!(c.shorts.len(), 0);
        // A track of another net across pad 2 is a short.
        pcb.tracks.insert(track(
            vcc,
            Point::mm(10, -2),
            Point::mm(10, 5),
            CopperLayer::Top,
        ));
        let board = Board::new(&pcb);
        assert_eq!(analyze(&board).shorts.len(), 3, "pad 2 and both GND tracks");
    }
}
