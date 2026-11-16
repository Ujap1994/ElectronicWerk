//! Routed lengths and length tuning.
//!
//! The length of a net is the length of its tracks (arcs along the arc).
//! Two kinds of rules compare lengths: the **skew** of a differential pair
//! (the length difference of its two nets, at most the class's `maxSkew`)
//! and **length groups** (every net of a group at most `tolerance` shorter
//! than its longest net).
//!
//! A net that is too short is made longer with **meanders**: a straight
//! track is replaced by the same track with bumps to one side. The bumps
//! have 45° corners and parallel legs `spacing` apart (centre to centre);
//! their height is chosen so the net gets exactly the length it needs.

use std::collections::{BTreeSet, HashMap};

use ew_model::id::Id;
use ew_model::pcb::{Pcb, Track};
use ew_model::rules::matches_pattern;
use serde::{Deserialize, Serialize};

use crate::geom::{V, arc_through};
use crate::items::ItemRef;
use crate::route::{Corners, Request, Router};
use crate::rules::RuleSet;

/// The length of a track (an arc along the arc).
#[must_use]
pub fn track_length(track: &Track) -> f64 {
    let (a, b) = (V::of(track.start), V::of(track.end));
    track
        .mid
        .and_then(|mid| arc_through(a, V::of(mid), b))
        .map_or_else(|| a.dist(b), |(_, radius, _, sweep)| radius * sweep.abs())
}

/// The routed length of a net: the length of its tracks. Vias are not
/// counted.
#[must_use]
pub fn net_length(pcb: &Pcb, net: Id) -> f64 {
    pcb.tracks
        .iter()
        .filter(|t| t.net == Some(net))
        .map(track_length)
        .sum()
}

/// The length of one net.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetLength {
    /// The net.
    pub net: Id,
    /// Its name.
    pub name: String,
    /// Routed length (nm).
    pub length: f64,
}

/// The lengths of a differential pair.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairLength {
    /// The common part of the names.
    pub name: String,
    /// The positive net.
    pub positive: NetLength,
    /// The negative net.
    pub negative: NetLength,
    /// Their length difference (nm).
    pub skew: f64,
    /// The largest allowed skew of the class (0: not checked).
    pub max_skew: f64,
    /// Within the allowed skew (or not checked).
    pub ok: bool,
}

/// The lengths of a length group.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupLength {
    /// Name of the group.
    pub name: String,
    /// How much shorter than the longest net a net may be.
    pub tolerance: f64,
    /// The longest net's length.
    pub longest: f64,
    /// Its nets, longest first.
    pub nets: Vec<NetLength>,
    /// Every net within the tolerance.
    pub ok: bool,
}

/// The lengths of a board: every routed net, the differential pairs and
/// the length groups.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Every net with tracks, by name.
    pub nets: Vec<NetLength>,
    /// The differential pairs.
    pub pairs: Vec<PairLength>,
    /// The length groups.
    pub groups: Vec<GroupLength>,
}

/// The nets of a length group, by their names.
#[must_use]
pub fn group_nets(pcb: &Pcb, group: &ew_model::rules::LengthGroup) -> Vec<Id> {
    pcb.nets
        .iter()
        .filter(|net| group.patterns().any(|p| matches_pattern(p, &net.name)))
        .map(|net| net.id)
        .collect()
}

/// The lengths of a board.
#[must_use]
#[allow(clippy::cast_precision_loss, reason = "board lengths fit f64")]
pub fn report(pcb: &Pcb, rules: &RuleSet) -> Report {
    // Every net's length in one pass over the tracks.
    let mut lengths: HashMap<Id, f64> = HashMap::new();
    for track in &pcb.tracks {
        if let Some(net) = track.net {
            *lengths.entry(net).or_default() += track_length(track);
        }
    }
    let entry = |net: Id| NetLength {
        net,
        name: pcb
            .nets
            .get(net)
            .map_or_else(String::new, |n| n.name.clone()),
        length: lengths.get(&net).copied().unwrap_or(0.0),
    };
    let routed: BTreeSet<Id> = lengths.keys().copied().collect();
    let mut nets: Vec<NetLength> = routed.into_iter().map(entry).collect();
    nets.sort_by(|a, b| a.name.cmp(&b.name));
    let pairs = crate::pairs::pairs(pcb)
        .into_iter()
        .map(|pair| {
            let (positive, negative) = (entry(pair.positive), entry(pair.negative));
            let class = pcb
                .nets
                .get(pair.positive)
                .map(|n| rules.class_name(&n.name, &n.class))
                .unwrap_or_default();
            let max_skew = rules.class(&class).max_skew as f64;
            let skew = (positive.length - negative.length).abs();
            PairLength {
                name: pair.name,
                ok: max_skew <= 0.0 || skew <= max_skew + 0.5,
                skew,
                max_skew,
                positive,
                negative,
            }
        })
        .collect();
    let groups = rules
        .length_groups
        .iter()
        .map(|group| {
            let mut members: Vec<NetLength> =
                group_nets(pcb, group).into_iter().map(entry).collect();
            members.sort_by(|a, b| b.length.total_cmp(&a.length));
            let longest = members.first().map_or(0.0, |n| n.length);
            let tolerance = group.tolerance as f64;
            GroupLength {
                name: group.name.clone(),
                tolerance,
                longest,
                ok: members
                    .iter()
                    .all(|n| longest - n.length <= tolerance + 0.5),
                nets: members,
            }
        })
        .collect();
    Report {
        nets,
        pairs,
        groups,
    }
}

/// `true` if the rules check lengths on this board at all: a length
/// group, or a differential pair whose class limits its skew.
#[must_use]
pub fn has_rules(pcb: &Pcb, rules: &RuleSet) -> bool {
    !rules.length_groups.is_empty()
        || crate::pairs::pairs(pcb).iter().any(|pair| {
            pcb.nets
                .get(pair.positive)
                .is_some_and(|n| rules.class(&rules.class_name(&n.name, &n.class)).max_skew > 0)
        })
}

/// To which side of a track meanders go (seen along the track).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MeanderSide {
    /// Whichever side has room.
    #[default]
    Auto,
    /// To the left.
    Left,
    /// To the right.
    Right,
}

/// The shape of meanders.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeanderOptions {
    /// Distance between neighbouring legs, centre to centre (nm).
    pub spacing: f64,
    /// The highest bump (nm).
    pub max_amplitude: f64,
    /// Side of the track.
    pub side: MeanderSide,
}

impl MeanderOptions {
    /// Meanders for a track: legs a track width and twice the clearance
    /// apart (at least three widths), bumps up to three spacings high.
    #[must_use]
    pub fn for_track(width: f64, clearance: f64) -> Self {
        let spacing = (3.0 * width).max(width + 2.0 * clearance);
        Self {
            spacing,
            max_amplitude: 3.0 * spacing,
            side: MeanderSide::Auto,
        }
    }
}

/// The extra length of one bump of amplitude `a` with legs `s` apart.
fn bump_extra(a: f64, s: f64) -> f64 {
    let c = corner(a, s);
    2.0 * a - c * (8.0 - 4.0 * std::f64::consts::SQRT_2)
}

/// The 45° corner of a bump.
fn corner(a: f64, s: f64) -> f64 {
    a.min(s) / 4.0
}

/// The amplitude that gives `extra` with `k` bumps.
#[allow(clippy::cast_precision_loss, reason = "few bumps")]
fn amplitude_for(extra: f64, k: usize, s: f64) -> f64 {
    let per = extra / k as f64;
    let k45 = 8.0 - 4.0 * std::f64::consts::SQRT_2;
    // With the corner a quarter of the spacing (high bumps) …
    let high = f64::midpoint(per, s / 4.0 * k45);
    if high >= s {
        return high;
    }
    // … or a quarter of the amplitude (low bumps).
    per / (2.0 - k45 / 4.0)
}

/// The points of one bump starting at `x` along the track (relative
/// coordinates: along, across).
fn bump(x: f64, a: f64, s: f64) -> [(f64, f64); 8] {
    let c = corner(a, s);
    [
        (x, 0.0),
        (x + c, c),
        (x + c, a - c),
        (x + 2.0 * c, a),
        (x + s, a),
        (x + s + c, a - c),
        (x + s + c, c),
        (x + s + 2.0 * c, 0.0),
    ]
}

/// A track made longer with meanders.
#[derive(Debug, Clone, PartialEq)]
pub struct Tuned {
    /// The track that is replaced.
    pub track: Id,
    /// The tracks that replace it.
    pub tracks: Vec<Track>,
    /// How much longer they are (nm).
    pub extra: f64,
}

impl Router<'_> {
    /// Replaces a straight, unlocked track by meanders that make it up to
    /// `extra` longer: the full `extra` where the bumps fit, otherwise as
    /// much as fits (`None` when nothing fits).
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "bump counts are small"
    )]
    pub fn tune(&self, track: &Track, extra: f64, options: &MeanderOptions) -> Option<Tuned> {
        if track.locked || track.mid.is_some() || extra <= 0.5 {
            return None;
        }
        let (a, b) = (V::of(track.start), V::of(track.end));
        let length = a.dist(b);
        let s = options.spacing;
        let width = track.width as f64;
        // The bumps keep a spacing away from both ends.
        let room = length - 2.0 * s;
        let slots = ((room + s / 2.0) / (2.0 * s)).floor().max(0.0) as usize;
        if slots == 0 {
            return None;
        }
        let dir = b.sub(a).unit();
        let left = V::new(-dir.y, dir.x);
        let request = Request {
            layer: track.layer,
            net: track.net,
            partner: None,
            width,
            start: a,
            target: b,
            diagonal_first: false,
            corners: Corners::Deg45,
            previous: None,
        };
        let me = ItemRef::Track { id: track.id };
        let clear = |points: &[V]| {
            points
                .windows(2)
                .all(|w| self.clear_skipping(&request, w[0], w[1], &|item| item == me))
        };
        let at = |side: f64, (x, y): (f64, f64)| a.add(dir.scale(x)).add(left.scale(side * y));
        // Slot positions spread over the track.
        let used = slots as f64 * 2.0 * s - s / 2.0;
        let first = s + (room - used).max(0.0) / 2.0;
        let slot_x = |i: usize| first + i as f64 * 2.0 * s;
        let sides: &[f64] = match options.side {
            MeanderSide::Auto => &[1.0, -1.0],
            MeanderSide::Left => &[1.0],
            MeanderSide::Right => &[-1.0],
        };
        let full = bump_extra(options.max_amplitude, s);
        let needed = ((extra / full).ceil() as usize).max(1);
        let mut best: Option<(f64, f64, f64, Vec<usize>)> = None;
        for &side in sides {
            for k in needed.min(slots)..=slots {
                let amp = amplitude_for(extra, k, s).min(options.max_amplitude);
                if amp < 1_000.0 {
                    continue;
                }
                let free: Vec<usize> = (0..slots)
                    .filter(|&i| {
                        let points: Vec<V> = bump(slot_x(i), amp, s)
                            .iter()
                            .map(|&p| at(side, p))
                            .collect();
                        clear(&points)
                    })
                    .collect();
                let count = free.len().min(k);
                let gained = count as f64 * bump_extra(amp, s);
                if best.as_ref().is_none_or(|(g, ..)| gained > *g + 0.5) && count > 0 {
                    // Spread the chosen bumps over the free slots.
                    let chosen: Vec<usize> =
                        (0..count).map(|j| free[j * free.len() / count]).collect();
                    best = Some((gained, side, amp, chosen));
                }
                if gained + 0.5 >= extra {
                    break;
                }
            }
            if best.as_ref().is_some_and(|(g, ..)| g + 0.5 >= extra) {
                break;
            }
        }
        let (gained, side, amp, chosen) = best?;
        let mut points = vec![a];
        for i in chosen {
            points.extend(bump(slot_x(i), amp, s).iter().map(|&p| at(side, p)));
        }
        points.push(b);
        let points: Vec<_> = points.into_iter().map(V::point).collect();
        let tracks = points
            .windows(2)
            .filter(|w| w[0] != w[1])
            .map(|w| Track {
                id: Id::random(),
                layer: track.layer,
                net: track.net,
                width: track.width,
                start: w[0],
                end: w[1],
                mid: None,
                locked: false,
            })
            .collect();
        Some(Tuned {
            track: track.id,
            tracks,
            extra: gained,
        })
    }
}

/// Changes that make nets longer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Lengthened {
    /// Tracks of the board that are replaced.
    pub removed: Vec<Id>,
    /// The tracks that replace them.
    pub added: Vec<Track>,
    /// Per net: the length it should gain and the length it gained (nm).
    pub nets: Vec<(Id, f64, f64)>,
}

impl Lengthened {
    /// Applies the changes to a board.
    pub fn apply(&self, pcb: &mut Pcb) {
        for id in &self.removed {
            pcb.tracks.remove(*id);
        }
        for track in &self.added {
            pcb.tracks.insert(track.clone());
        }
    }
}

/// Makes nets longer with meanders on their longest straight tracks, one
/// net after the other (later meanders keep their distance to earlier
/// ones). `options` overrides the meander shape of the net classes.
#[must_use]
#[allow(clippy::cast_precision_loss, reason = "board lengths fit f64")]
pub fn lengthen(
    pcb: &Pcb,
    rules: Option<&ew_model::rules::Rules>,
    wanted: &[(Id, f64)],
    options: Option<MeanderOptions>,
) -> Lengthened {
    let mut board = pcb.clone();
    let mut out = Lengthened::default();
    for &(net, extra) in wanted {
        let mut remaining = extra;
        for _ in 0..16 {
            if remaining <= 0.5 {
                break;
            }
            let tuned = {
                let router = Router::new(&board, rules);
                let class = router.rules.class(&router.net(Some(net)).class).clone();
                let mut tracks: Vec<&Track> = board
                    .tracks
                    .iter()
                    .filter(|t| t.net == Some(net) && !t.locked && t.mid.is_none())
                    .collect();
                tracks.sort_by(|a, b| track_length(b).total_cmp(&track_length(a)));
                tracks.into_iter().find_map(|track| {
                    let shape = options.unwrap_or_else(|| {
                        MeanderOptions::for_track(track.width as f64, class.clearance as f64)
                    });
                    router.tune(track, remaining, &shape)
                })
            };
            let Some(tuned) = tuned else {
                break;
            };
            remaining -= tuned.extra;
            board.tracks.remove(tuned.track);
            // A track added before and tuned again is simply replaced.
            if let Some(index) = out.added.iter().position(|t| t.id == tuned.track) {
                out.added.remove(index);
            } else {
                out.removed.push(tuned.track);
            }
            for track in tuned.tracks {
                board.tracks.insert(track.clone());
                out.added.push(track);
            }
        }
        out.nets.push((net, extra, extra - remaining.max(0.0)));
    }
    out
}

/// What the length rules ask for: per net that is too short, the length it
/// should gain (to the length of its partner, or of the longest net of its
/// group).
#[must_use]
pub fn shortfalls(report: &Report) -> Vec<(Id, f64)> {
    let mut wanted: Vec<(Id, f64)> = Vec::new();
    let mut want = |net: Id, extra: f64| {
        if let Some(entry) = wanted.iter_mut().find(|(n, _)| *n == net) {
            entry.1 = entry.1.max(extra);
        } else {
            wanted.push((net, extra));
        }
    };
    for pair in report.pairs.iter().filter(|p| !p.ok) {
        let shorter = if pair.positive.length < pair.negative.length {
            &pair.positive
        } else {
            &pair.negative
        };
        want(shorter.net, pair.skew);
    }
    for group in report.groups.iter().filter(|g| !g.ok) {
        for net in &group.nets {
            if group.longest - net.length > group.tolerance + 0.5 {
                want(net.net, group.longest - net.length);
            }
        }
    }
    wanted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bumps_add_what_they_promise() {
        let s = 600.0;
        for a in [200.0, 600.0, 1_500.0] {
            let points = bump(0.0, a, s);
            let path: f64 = points
                .windows(2)
                .map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1))
                .sum();
            let straight = points[7].0 - points[0].0;
            assert!((path - straight - bump_extra(a, s)).abs() < 1e-6, "{a}");
            // Every bend is 45°.
            for w in points.windows(3) {
                let (u, v) = (
                    (w[1].0 - w[0].0, w[1].1 - w[0].1),
                    (w[2].0 - w[1].0, w[2].1 - w[1].1),
                );
                let cos = (u.0 * v.0 + u.1 * v.1) / (u.0.hypot(u.1) * v.0.hypot(v.1));
                assert!(
                    (cos - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-9
                        || (cos - 1.0).abs() < 1e-9
                );
            }
        }
        for extra in [300.0, 2_000.0, 9_000.0] {
            for k in 1..4 {
                let a = amplitude_for(extra, k, s);
                #[allow(clippy::cast_precision_loss, reason = "small")]
                let got = k as f64 * bump_extra(a, s);
                assert!((got - extra).abs() < 1e-6, "{extra} {k}");
            }
        }
    }
}
