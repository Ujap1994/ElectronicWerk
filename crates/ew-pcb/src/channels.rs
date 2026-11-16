//! Channels: a schematic sheet used more than once (four identical
//! amplifiers, eight LED drivers) gives the board one group of parts per
//! use. The layout of one channel (where its parts are and how they are
//! routed) can be copied to the others: each part of a target channel goes
//! where the same part of the source channel is, moved by the offset
//! between the channels, and the source's tracks and vias are copied with
//! the target's nets.
//!
//! Parts correspond by their symbol on the shared sheet; nets by the pads
//! they reach (a source net that reaches pads of two different target nets
//! is not copied).

use std::collections::{BTreeMap, HashMap};

use ew_model::id::Id;
use ew_model::pcb::{FootprintInstance, Pcb, Track, Via};
use ew_model::project::Project;
use ew_model::units::Point;
use ew_schematic::connectivity::Connectivity;
use serde::Serialize;

use crate::geom::{BBox, V};

/// One use of a repeated sheet.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelInstance {
    /// The sheet instance key (`/…/`).
    pub key: String,
    /// The readable instance path (`/Channel 1/`).
    pub name: String,
    /// Its footprints, by the part's first symbol on the sheet.
    #[serde(skip)]
    pub footprints: BTreeMap<Id, Id>,
    /// The ids of its footprints.
    pub parts: Vec<Id>,
    /// The designators of its footprints.
    pub designators: Vec<String>,
}

/// A sheet used more than once.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    /// The schematic document.
    pub sheet: Id,
    /// Its name.
    pub name: String,
    /// Its uses, in the order of the schematic.
    pub instances: Vec<ChannelInstance>,
}

/// The repeated sheets of a project with the footprints of each use.
#[must_use]
pub fn channels(project: &Project, connectivity: &Connectivity, pcb: &Pcb) -> Vec<Channel> {
    let (parts, _) = crate::sync::parts(project, connectivity);
    let by_component: HashMap<Id, &FootprintInstance> = pcb
        .footprints
        .iter()
        .filter_map(|fp| fp.component.map(|c| (c, fp)))
        .collect();
    let mut out: Vec<Channel> = Vec::new();
    for inst in &connectivity.instances {
        let uses = connectivity
            .instances
            .iter()
            .filter(|i| i.sheet == inst.sheet)
            .count();
        if uses < 2 {
            continue;
        }
        let mut footprints = BTreeMap::new();
        let mut designators = Vec::new();
        for part in parts.iter().filter(|p| p.instance == inst.key) {
            let (Some(first), Some(fp)) = (
                part.units.iter().map(|(_, symbol)| *symbol).min(),
                by_component.get(&part.key),
            ) else {
                continue;
            };
            footprints.insert(first, fp.id);
            designators.push(fp.designator.clone());
        }
        let instance = ChannelInstance {
            key: inst.key.clone(),
            name: inst.name.clone(),
            parts: footprints.values().copied().collect(),
            footprints,
            designators,
        };
        if let Some(channel) = out.iter_mut().find(|c| c.sheet == inst.sheet) {
            channel.instances.push(instance);
        } else {
            out.push(Channel {
                sheet: inst.sheet,
                name: match project.document(inst.sheet) {
                    Some(ew_model::document::Document::Schematic(sheet)) => sheet.name.clone(),
                    _ => String::new(),
                },
                instances: vec![instance],
            });
        }
    }
    for channel in &mut out {
        channel
            .instances
            .sort_by(|a, b| ew_schematic::connectivity::natural_cmp(&a.name, &b.name));
    }
    out
}

/// What copying a channel's layout changes.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Copied {
    /// Footprints with their new place.
    pub footprints: Vec<FootprintInstance>,
    /// Tracks and vias of the targets that the copies replace.
    pub removed: Vec<Id>,
    /// The copied tracks.
    pub tracks: Vec<Track>,
    /// The copied vias.
    pub vias: Vec<Via>,
    /// Parts of the targets that were left as they are (another footprint,
    /// or locked).
    pub skipped: Vec<String>,
}

/// The box around the pads of footprints, grown by `margin`.
fn region(pcb: &Pcb, ids: &[Id], margin: f64) -> BBox {
    let mut points = Vec::new();
    for fp in ids.iter().filter_map(|id| pcb.footprints.get(*id)) {
        points.push(V::of(fp.at));
        let xform = crate::items::footprint_xform(fp);
        for pad in &fp.pads {
            points.push(xform.apply(V::of(pad.at)));
        }
    }
    BBox::of_points(&points).grow(margin)
}

/// Copies the layout of the instance `from` of a channel to the instances
/// `to`: placement, and with `routing` the tracks and vias between its
/// parts.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    reason = "board offsets; placement, nets and routing of each target in one pass"
)]
pub fn copy_layout(
    pcb: &Pcb,
    channel: &Channel,
    from: &str,
    to: &[String],
    routing: bool,
) -> Copied {
    let mut out = Copied::default();
    let Some(source) = channel.instances.iter().find(|i| i.key == from) else {
        return out;
    };
    let margin = 1_000_000.0;
    let source_region = region(pcb, &source.parts, margin);
    let anchor = |inst: &ChannelInstance| {
        let points: Vec<V> = inst
            .parts
            .iter()
            .filter_map(|id| pcb.footprints.get(*id))
            .map(|fp| V::of(fp.at))
            .collect();
        BBox::of_points(&points).min
    };
    let source_anchor = anchor(source);
    for target in channel
        .instances
        .iter()
        .filter(|i| i.key != from && to.contains(&i.key))
    {
        let offset = anchor(target).sub(source_anchor);
        let shift =
            |p: Point| Point::new(p.x + offset.x.round() as i64, p.y + offset.y.round() as i64);
        // Nets: source net → target net, by the pads they reach.
        let mut nets: HashMap<Id, Option<Id>> = HashMap::new();
        for (symbol, source_fp) in &source.footprints {
            let (Some(src), Some(dst)) = (
                pcb.footprints.get(*source_fp),
                target
                    .footprints
                    .get(symbol)
                    .and_then(|id| pcb.footprints.get(*id)),
            ) else {
                continue;
            };
            // Placement.
            if dst.footprint != src.footprint || dst.locked {
                out.skipped.push(dst.designator.clone());
            } else {
                let mut moved = dst.clone();
                moved.at = shift(src.at);
                moved.rotation = src.rotation;
                moved.side = src.side;
                moved.texts.clone_from(&src.texts);
                out.footprints.push(moved);
            }
            for pad in &src.pads {
                let Some(net) = pad.net else {
                    continue;
                };
                let theirs = dst
                    .pads
                    .iter()
                    .find(|p| p.number == pad.number)
                    .and_then(|p| p.net);
                let entry = nets.entry(net).or_insert(theirs);
                if *entry != theirs {
                    *entry = None;
                }
            }
        }
        if !routing {
            continue;
        }
        let target_region = region(pcb, &target.parts, margin);
        let target_nets: Vec<Id> = nets.values().filter_map(|n| *n).collect();
        // The target's routing between its parts goes; the copy comes.
        for track in &pcb.tracks {
            let inside =
                target_region.contains(BBox::of(V::of(track.start)).with(V::of(track.end)));
            if inside && !track.locked && track.net.is_some_and(|n| target_nets.contains(&n)) {
                out.removed.push(track.id);
            }
        }
        for via in &pcb.vias {
            if target_region.contains(BBox::of(V::of(via.at)))
                && !via.locked
                && via.net.is_some_and(|n| target_nets.contains(&n))
            {
                out.removed.push(via.id);
            }
        }
        for track in &pcb.tracks {
            let inside =
                source_region.contains(BBox::of(V::of(track.start)).with(V::of(track.end)));
            let Some(Some(net)) = track.net.and_then(|n| nets.get(&n)).copied() else {
                continue;
            };
            if inside {
                out.tracks.push(Track {
                    id: Id::random(),
                    net: Some(net),
                    start: shift(track.start),
                    end: shift(track.end),
                    mid: track.mid.map(shift),
                    ..track.clone()
                });
            }
        }
        for via in &pcb.vias {
            let Some(Some(net)) = via.net.and_then(|n| nets.get(&n)).copied() else {
                continue;
            };
            if source_region.contains(BBox::of(V::of(via.at))) {
                out.vias.push(Via {
                    id: Id::random(),
                    net: Some(net),
                    at: shift(via.at),
                    ..via.clone()
                });
            }
        }
    }
    out
}
