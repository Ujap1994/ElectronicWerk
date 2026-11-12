//! The design rules check.
//!
//! [`check`] runs every check over a board; with a region it only checks
//! the items in that region (online DRC after an edit), and [`merge`]
//! combines the result with the previous report.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Side};
use ew_model::pcb::{PadKind, Pcb};
use ew_model::rules::{ErcSeverity, Rules};
use ew_model::units::Point;
use rayon::prelude::*;
use serde::Serialize;

use crate::connectivity::{self, Airwire, Connectivity};
use crate::geom::{BBox, Shape, V};
use crate::items::{Board, CopperItem, ItemRef};
use crate::rules::{Limit, Props, RuleError, RuleSet, ZoneTest};

/// An object a finding is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    /// The board object (a pad's footprint).
    pub id: Id,
    /// The pad index, for pads.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pad: Option<usize>,
}

impl Target {
    const fn of(item: ItemRef) -> Self {
        match item {
            ItemRef::Pad { footprint, index } => Self {
                id: footprint,
                pad: Some(index),
            },
            _ => Self {
                id: item.object(),
                pad: None,
            },
        }
    }

    const fn object(id: Id) -> Self {
        Self { id, pad: None }
    }

    fn key(self) -> String {
        match self.pad {
            Some(pad) => format!("{}#{pad}", self.id),
            None => self.id.to_string(),
        }
    }
}

/// One finding of the design rules check.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Check code, for example `clearance`.
    pub code: String,
    /// Severity.
    pub severity: ErcSeverity,
    /// English description.
    pub message: String,
    /// Values for translated messages.
    pub args: BTreeMap<String, String>,
    /// Where to look.
    pub at: Point,
    /// The copper layer, if the finding is on one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    /// The objects concerned.
    pub targets: Vec<Target>,
    /// A stable key (for accepting a finding).
    pub key: String,
    /// Accepted by the user: kept, but not counted.
    pub excluded: bool,
}

/// Counts of a report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    /// Errors.
    pub errors: usize,
    /// Warnings.
    pub warnings: usize,
    /// Accepted findings.
    pub excluded: usize,
    /// Unrouted connections.
    pub unrouted: usize,
}

/// The result of a check.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// Findings, ordered by severity and code.
    pub findings: Vec<Finding>,
    /// Counts.
    pub counts: Counts,
    /// Unrouted connections (the ratsnest).
    pub airwires: Vec<Airwire>,
    /// Syntax errors in custom rules.
    pub rule_errors: Vec<RuleError>,
}

/// Codes of checks that look at items in one place; the others look at the
/// whole board and always run in full.
pub const LOCAL_CODES: &[&str] = &[
    "short",
    "clearance",
    "track-width",
    "via-diameter",
    "via-drill",
    "annular-ring",
    "hole-size",
    "hole-to-hole",
    "copper-to-edge",
    "outside-board",
    "keepout",
    "courtyard-overlap",
    "silk-over-pad",
    "silk-width",
    "dangling-track",
    "dangling-via",
];

/// Options of a check.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Only check items in this area (online DRC).
    pub region: Option<BBox>,
    /// Findings from other sources (schematic parity) to include.
    pub extra: Vec<Finding>,
}

/// Formats nanometres as millimetres.
#[must_use]
pub fn mm(value: f64) -> String {
    let text = format!("{:.3}", value / 1e6);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text} mm")
}

struct NetInfo {
    name: String,
    class: String,
}

struct Zones<'a> {
    pcb: &'a Pcb,
    shapes: HashMap<&'a str, Vec<Shape>>,
}

impl<'a> Zones<'a> {
    fn new(pcb: &'a Pcb) -> Self {
        let mut shapes: HashMap<&str, Vec<Shape>> = HashMap::new();
        for zone in pcb
            .zones
            .iter()
            .filter(|z| !z.name.is_empty() && z.outline.len() >= 3)
        {
            shapes
                .entry(zone.name.as_str())
                .or_default()
                .push(Shape::polygon(
                    zone.outline.iter().map(|p| V::of(*p)).collect(),
                    Vec::new(),
                    0.0,
                ));
        }
        Self { pcb, shapes }
    }
}

impl ZoneTest for Zones<'_> {
    fn inside(&self, name: &str, p: V) -> bool {
        let _ = self.pcb;
        self.shapes
            .get(name)
            .is_some_and(|shapes| shapes.iter().any(|s| s.contains(p)))
    }
}

struct Context<'a> {
    board: Board<'a>,
    rules: RuleSet,
    nets: HashMap<Id, NetInfo>,
    zones: Zones<'a>,
    settings: &'a ew_model::rules::DrcSettings,
    exclusions: HashSet<&'a str>,
    findings: HashMap<String, Finding>,
    no_net: NetInfo,
}

#[derive(Default)]
struct Detail {
    args: Vec<(&'static str, String)>,
    layer: Option<CopperLayer>,
}

impl Context<'_> {
    fn net(&self, net: Option<Id>) -> &NetInfo {
        net.and_then(|id| self.nets.get(&id))
            .unwrap_or(&self.no_net)
    }

    fn props(&self, item: &CopperItem, layer: Option<CopperLayer>) -> Props<'_> {
        let net = self.net(item.net);
        let mut props = Props {
            kind: item.item.kind(),
            net: &net.name,
            netclass: &net.class,
            layer,
            at: item.shape.anchor(),
            ..Props::default()
        };
        match item.item {
            ItemRef::Pad { footprint, index } => {
                if let Some(fp) = self.board.footprint(footprint) {
                    props.designator = &fp.designator;
                    props.footprint = &fp.footprint;
                    props.side = fp.side.as_str();
                    if let Some(pad) = fp.pads.get(index) {
                        props.pad = &pad.number;
                        props.diameter = Some(pad.size.w.min(pad.size.h) as f64);
                        props.drill = pad.drill.map(|d| d.diameter as f64);
                    }
                }
            }
            ItemRef::Track { id } => {
                props.width = self.board.pcb.tracks.get(id).map(|t| t.width as f64);
            }
            ItemRef::Via { id } => {
                if let Some(via) = self.board.pcb.vias.get(id) {
                    props.diameter = Some(via.diameter as f64);
                    props.drill = Some(via.drill as f64);
                }
            }
            ItemRef::Zone { .. } => {}
        }
        props
    }

    fn describe(&self, item: ItemRef) -> String {
        match item {
            ItemRef::Pad { footprint, index } => self.board.footprint(footprint).map_or_else(
                || "pad".to_owned(),
                |fp| {
                    let number = fp.pads.get(index).map_or("?", |p| p.number.as_str());
                    format!("pad {}-{number}", fp.designator)
                },
            ),
            ItemRef::Track { .. } => "track".to_owned(),
            ItemRef::Via { .. } => "via".to_owned(),
            ItemRef::Zone { id, .. } => {
                let name = self
                    .board
                    .pcb
                    .zones
                    .get(id)
                    .map(|z| z.name.as_str())
                    .unwrap_or_default();
                if name.is_empty() {
                    "zone".to_owned()
                } else {
                    format!("zone {name}")
                }
            }
        }
    }

    fn label(&self, index: usize) -> String {
        let item = &self.board.copper[index];
        let net = &self.net(item.net).name;
        let what = self.describe(item.item);
        if net.is_empty() {
            what
        } else {
            format!("{what} ({net})")
        }
    }

    fn report(&mut self, code: &str, message: String, at: V, targets: Vec<Target>, detail: Detail) {
        let severity = self.settings.severity(code);
        if severity == ErcSeverity::Ignore {
            return;
        }
        let mut sorted = targets.clone();
        sorted.sort();
        let mut key = format!("{code}:");
        for (i, target) in sorted.iter().enumerate() {
            if i > 0 {
                key.push('/');
            }
            key.push_str(&target.key());
        }
        if let Some(layer) = detail.layer {
            let _ = write!(key, ":{layer}");
        }
        let excluded = self.exclusions.contains(key.as_str());
        let finding = Finding {
            code: code.to_owned(),
            severity,
            message,
            args: detail
                .args
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v))
                .collect(),
            at: at.point(),
            layer: detail.layer.map(|l| l.to_string()),
            targets,
            key: key.clone(),
            excluded,
        };
        self.findings.entry(key).or_insert(finding);
    }

    fn first_layer(&self, bits: u64) -> Option<CopperLayer> {
        self.board.layers_of(bits).next()
    }
}

/// Measurements this much below a limit still pass (500 nm), so that
/// rounding in generated geometry (zone fills) never shows up as a
/// violation.
pub const EPSILON: f64 = 500.0;

type Region<'r> = dyn Fn(BBox) -> bool + Sync + 'r;

fn meeting_point(a: BBox, b: BBox) -> V {
    let min = V::new(a.min.x.max(b.min.x), a.min.y.max(b.min.y));
    let max = V::new(a.max.x.min(b.max.x), a.max.y.min(b.max.y));
    if min.x <= max.x && min.y <= max.y {
        BBox { min, max }.center()
    } else {
        a.center().add(b.center()).scale(0.5)
    }
}

/// Runs the design rules check on a board.
#[must_use]
pub fn check(pcb: &Pcb, rules: Option<&Rules>, options: &Options) -> Report {
    let defaults = ew_model::rules::DrcSettings::default();
    let mut set = RuleSet::new(rules);
    set.cache_classes(pcb.nets.iter().map(|n| (n.name.clone(), n.class.clone())));
    let nets = pcb
        .nets
        .iter()
        .map(|net| {
            (
                net.id,
                NetInfo {
                    name: net.name.clone(),
                    class: set.class_name(&net.name, &net.class),
                },
            )
        })
        .collect();
    let mut ctx = Context {
        board: Board::new(pcb),
        rules: set,
        nets,
        zones: Zones::new(pcb),
        settings: rules.map_or(&defaults, |r| &r.drc),
        exclusions: pcb.drc_exclusions.iter().map(String::as_str).collect(),
        findings: HashMap::new(),
        no_net: NetInfo {
            name: String::new(),
            class: ew_model::rules::DEFAULT_NET_CLASS.to_owned(),
        },
    };
    let region = options.region.map(|r| r.grow(ctx.rules.max_clearance()));
    let in_region = move |bbox: BBox| region.is_none_or(|r| r.intersects(bbox));
    let conn = connectivity::analyze(&ctx.board);

    copper_clearance(&mut ctx, &in_region);
    widths_and_holes(&mut ctx, &in_region);
    hole_spacing(&mut ctx, &in_region);
    edges_and_outline(&mut ctx, &in_region);
    keepouts(&mut ctx, &in_region);
    courtyards(&mut ctx, &in_region);
    silkscreen(&mut ctx, &in_region);
    dangling(&mut ctx, &conn, &in_region);
    board_checks(&mut ctx, &conn);

    let rule_errors = ctx.rules.errors.clone();
    for error in &rule_errors {
        ctx.report(
            "rule-error",
            format!(
                "Rule {}: {} (at character {})",
                error.rule, error.message, error.position
            ),
            V::default(),
            Vec::new(),
            Detail {
                args: vec![
                    ("rule", error.rule.clone()),
                    ("error", error.message.clone()),
                    ("position", error.position.to_string()),
                ],
                layer: None,
            },
        );
    }
    let mut findings: Vec<Finding> = ctx.findings.into_values().collect();
    for mut extra in options.extra.iter().cloned() {
        extra.excluded = pcb.drc_exclusions.contains(&extra.key);
        findings.push(extra);
    }
    sort(&mut findings);
    let counts = count(&findings, conn.airwires.len());
    Report {
        findings,
        counts,
        airwires: conn.airwires,
        rule_errors,
    }
}

fn sort(findings: &mut [Finding]) {
    findings.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.at.x.cmp(&b.at.x))
            .then_with(|| a.at.y.cmp(&b.at.y))
            .then_with(|| a.key.cmp(&b.key))
    });
}

fn count(findings: &[Finding], unrouted: usize) -> Counts {
    let mut counts = Counts {
        unrouted,
        ..Counts::default()
    };
    for finding in findings {
        if finding.excluded {
            counts.excluded += 1;
        } else if finding.severity == ErcSeverity::Error {
            counts.errors += 1;
        } else if finding.severity == ErcSeverity::Warning {
            counts.warnings += 1;
        }
    }
    counts
}

/// Combines a region check with the previous report: local findings in the
/// region are replaced, board-wide findings come from the new report.
#[must_use]
pub fn merge(previous: &Report, fresh: Report, region: BBox, max_clearance: f64) -> Report {
    let area = region.grow(max_clearance);
    let inside = |p: Point| area.intersects(BBox::of(V::of(p)));
    let mut findings: Vec<Finding> = previous
        .findings
        .iter()
        .filter(|f| LOCAL_CODES.contains(&f.code.as_str()) && !inside(f.at))
        .cloned()
        .collect();
    let mut seen: HashSet<String> = findings.iter().map(|f| f.key.clone()).collect();
    for finding in fresh.findings {
        if seen.insert(finding.key.clone()) {
            findings.push(finding);
        }
    }
    sort(&mut findings);
    let counts = count(&findings, fresh.airwires.len());
    Report {
        findings,
        counts,
        airwires: fresh.airwires,
        rule_errors: fresh.rule_errors,
    }
}

fn pad_clearance(ctx: &Context<'_>, item: &CopperItem) -> Option<f64> {
    match item.item {
        ItemRef::Pad { footprint, index } => ctx
            .board
            .footprint(footprint)
            .and_then(|fp| fp.pads.get(index))
            .and_then(|pad| pad.clearance)
            .map(|c| c as f64),
        ItemRef::Zone { id, .. } => ctx
            .board
            .pcb
            .zones
            .get(id)
            .map(|zone| zone.fill.clearance as f64),
        _ => None,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one pass over the candidate pairs with the rule lookup"
)]
fn copper_clearance(ctx: &mut Context<'_>, in_region: &Region<'_>) {
    let reach = ctx.rules.max_clearance()
        + ctx
            .board
            .pcb
            .footprints
            .iter()
            .flat_map(|fp| fp.pads.iter().filter_map(|p| p.clearance))
            .max()
            .unwrap_or(0) as f64;
    let shared_ctx: &Context<'_> = ctx;
    let hits: Vec<(usize, usize, f64, Limit, u64)> = (0..shared_ctx.board.copper.len())
        .into_par_iter()
        .flat_map_iter(|i| clearance_hits(shared_ctx, i, reach, in_region))
        .collect();
    report_clearance(ctx, hits);
}

#[allow(clippy::many_single_char_names, reason = "a, b, i, j: the pair")]
fn clearance_hits(
    ctx: &Context<'_>,
    i: usize,
    reach: f64,
    in_region: &Region<'_>,
) -> Vec<(usize, usize, f64, Limit, u64)> {
    let mut hits = Vec::new();
    let a = &ctx.board.copper[i];
    if !in_region(a.bbox) {
        return hits;
    }
    {
        for j in ctx.board.copper_near(a.bbox.grow(reach)) {
            let b = &ctx.board.copper[j];
            if j == i || (j < i && in_region(b.bbox)) {
                continue;
            }
            let shared = a.layers & b.layers;
            if shared == 0 || a.item == b.item {
                continue;
            }
            if matches!((a.net, b.net), (Some(x), Some(y)) if x == y) {
                continue;
            }
            if matches!((a.item, b.item), (ItemRef::Zone { id: x, .. }, ItemRef::Zone { id: y, .. }) if x == y)
            {
                continue;
            }
            let layers: Vec<CopperLayer> = if ctx.rules.has_layer_rules() {
                ctx.board.layers_of(shared).collect()
            } else {
                ctx.first_layer(shared).into_iter().collect()
            };
            for layer in layers {
                let (pa, pb) = (ctx.props(a, Some(layer)), ctx.props(b, Some(layer)));
                let mut limit = ctx.rules.clearance(&pa, &pb, &ctx.zones);
                for own in [pad_clearance(ctx, a), pad_clearance(ctx, b)]
                    .into_iter()
                    .flatten()
                {
                    if own > limit.value {
                        limit = Limit {
                            value: own,
                            source: if matches!(a.item, ItemRef::Zone { .. })
                                || matches!(b.item, ItemRef::Zone { .. })
                            {
                                crate::rules::Source::Zone
                            } else {
                                crate::rules::Source::Pad
                            },
                        };
                    }
                }
                if let Some(gap) = a.shape.gap(&b.shape, limit.value - EPSILON) {
                    let bit = ctx.board.layer_bit(layer).unwrap_or(shared);
                    hits.push((i, j, gap, limit, bit));
                    break;
                }
            }
        }
    }
    hits
}

fn report_clearance(ctx: &mut Context<'_>, hits: Vec<(usize, usize, f64, Limit, u64)>) {
    for (i, j, gap, limit, bit) in hits {
        let (a, b) = (&ctx.board.copper[i], &ctx.board.copper[j]);
        let short = gap <= 0.0 && a.net.is_some() && b.net.is_some();
        let at = meeting_point(a.bbox, b.bbox);
        let layer = ctx.first_layer(bit);
        let (la, lb) = (ctx.label(i), ctx.label(j));
        let targets = vec![Target::of(a.item), Target::of(b.item)];
        if short {
            ctx.report(
                "short",
                format!("{la} touches {lb}: the nets are shorted"),
                at,
                targets,
                Detail {
                    args: vec![("a", la), ("b", lb)],
                    layer,
                },
            );
        } else {
            let actual = gap.max(0.0);
            ctx.report(
                "clearance",
                format!(
                    "Clearance {} between {la} and {lb} is less than {} ({})",
                    mm(actual),
                    mm(limit.value),
                    limit.source
                ),
                at,
                targets,
                Detail {
                    args: vec![
                        ("a", la),
                        ("b", lb),
                        ("actual", mm(actual)),
                        ("required", mm(limit.value)),
                        ("source", limit.source.to_string()),
                    ],
                    layer,
                },
            );
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the size checks of tracks, vias and pads share one pass"
)]
fn widths_and_holes(ctx: &mut Context<'_>, in_region: &Region<'_>) {
    let mut seen: HashSet<ItemRef> = HashSet::new();
    let mut found: Vec<(&'static str, String, V, ItemRef, Detail)> = Vec::new();
    for item in &ctx.board.copper {
        if !in_region(item.bbox) || !seen.insert(item.item) {
            continue;
        }
        let layer = ctx.first_layer(item.layers);
        let props = ctx.props(item, layer);
        let at = item.shape.anchor();
        match item.item {
            ItemRef::Track { id } => {
                let Some(track) = ctx.board.pcb.tracks.get(id) else {
                    continue;
                };
                let width = track.width as f64;
                let limits = ctx.rules.track_width(&props, &ctx.zones);
                let center = V::of(track.start).add(V::of(track.end)).scale(0.5);
                if width < limits.min.value {
                    found.push((
                        "track-width",
                        format!(
                            "Track width {} is less than {} ({})",
                            mm(width),
                            mm(limits.min.value),
                            limits.min.source
                        ),
                        center,
                        item.item,
                        Detail {
                            args: vec![
                                ("actual", mm(width)),
                                ("required", mm(limits.min.value)),
                                ("source", limits.min.source.to_string()),
                            ],
                            layer,
                        },
                    ));
                } else if let Some(max) = limits.max.filter(|max| width > max.value) {
                    found.push((
                        "track-width",
                        format!(
                            "Track width {} is more than {} ({})",
                            mm(width),
                            mm(max.value),
                            max.source
                        ),
                        center,
                        item.item,
                        Detail {
                            args: vec![
                                ("actual", mm(width)),
                                ("maximum", mm(max.value)),
                                ("source", max.source.to_string()),
                            ],
                            layer,
                        },
                    ));
                }
            }
            ItemRef::Via { id } => {
                let Some(via) = ctx.board.pcb.vias.get(id) else {
                    continue;
                };
                let limits = ctx.rules.via(&props, &ctx.zones);
                let (d, drill) = (via.diameter as f64, via.drill as f64);
                let ring = (d - drill) / 2.0;
                let checks = [
                    ("via-diameter", "Via diameter", d, &limits.diameter),
                    ("via-drill", "Via drill", drill, &limits.drill),
                    ("annular-ring", "Annular ring", ring, &limits.annular),
                ];
                for (code, what, value, limit) in checks {
                    if value < limit.value {
                        found.push((
                            code,
                            format!(
                                "{what} {} is less than {} ({})",
                                mm(value),
                                mm(limit.value),
                                limit.source
                            ),
                            at,
                            item.item,
                            Detail {
                                args: vec![
                                    ("actual", mm(value)),
                                    ("required", mm(limit.value)),
                                    ("source", limit.source.to_string()),
                                ],
                                layer: None,
                            },
                        ));
                    }
                }
            }
            ItemRef::Pad { footprint, index } => {
                let Some(pad) = ctx
                    .board
                    .footprint(footprint)
                    .and_then(|fp| fp.pads.get(index))
                else {
                    continue;
                };
                let Some(drill) = pad.drill else {
                    continue;
                };
                let limits = ctx.rules.via(&props, &ctx.zones);
                let hole = drill.diameter as f64;
                if hole < limits.hole.value {
                    found.push((
                        "hole-size",
                        format!(
                            "Hole {} is less than {} ({})",
                            mm(hole),
                            mm(limits.hole.value),
                            limits.hole.source
                        ),
                        at,
                        item.item,
                        Detail {
                            args: vec![
                                ("actual", mm(hole)),
                                ("required", mm(limits.hole.value)),
                                ("source", limits.hole.source.to_string()),
                            ],
                            layer: None,
                        },
                    ));
                }
                if pad.kind == PadKind::ThroughHole {
                    let size = pad.size.w.min(pad.size.h) as f64;
                    let ring = (size - drill.slot_length.map_or(hole, |_| hole)) / 2.0;
                    if ring < limits.annular.value {
                        found.push((
                            "annular-ring",
                            format!(
                                "Annular ring {} is less than {} ({})",
                                mm(ring.max(0.0)),
                                mm(limits.annular.value),
                                limits.annular.source
                            ),
                            at,
                            item.item,
                            Detail {
                                args: vec![
                                    ("actual", mm(ring.max(0.0))),
                                    ("required", mm(limits.annular.value)),
                                    ("source", limits.annular.source.to_string()),
                                ],
                                layer: None,
                            },
                        ));
                    }
                }
            }
            ItemRef::Zone { .. } => {}
        }
    }
    for (code, message, at, item, detail) in found {
        ctx.report(code, message, at, vec![Target::of(item)], detail);
    }
}

fn hole_spacing(ctx: &mut Context<'_>, in_region: &Region<'_>) {
    let reach = ctx.rules.constraints.min_hole_to_hole as f64 * 4.0;
    let mut found = Vec::new();
    for (i, a) in ctx.board.holes.iter().enumerate() {
        let bbox = a.shape.bbox();
        if !in_region(bbox) {
            continue;
        }
        for j in ctx.board.holes_near(bbox.grow(reach)) {
            let b = &ctx.board.holes[j];
            if j == i || (j < i && in_region(b.shape.bbox())) || a.item == b.item {
                continue;
            }
            let pa = Props {
                kind: a.item.kind(),
                drill: Some(a.diameter),
                at: a.shape.anchor(),
                ..Props::default()
            };
            let pb = Props {
                kind: b.item.kind(),
                drill: Some(b.diameter),
                at: b.shape.anchor(),
                ..Props::default()
            };
            let limit = ctx.rules.hole_to_hole(&pa, &pb, &ctx.zones);
            if let Some(gap) = a.shape.gap(&b.shape, limit.value - EPSILON) {
                found.push((
                    a.item,
                    b.item,
                    gap.max(0.0),
                    limit,
                    meeting_point(bbox, b.shape.bbox()),
                ));
            }
        }
    }
    for (a, b, gap, limit, at) in found {
        let (la, lb) = (ctx.describe(a), ctx.describe(b));
        ctx.report(
            "hole-to-hole",
            format!(
                "Holes of {la} and {lb} are {} apart, less than {} ({})",
                mm(gap),
                mm(limit.value),
                limit.source
            ),
            at,
            vec![Target::of(a), Target::of(b)],
            Detail {
                args: vec![
                    ("a", la),
                    ("b", lb),
                    ("actual", mm(gap)),
                    ("required", mm(limit.value)),
                    ("source", limit.source.to_string()),
                ],
                layer: None,
            },
        );
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "edge, outside and footprint checks share the outline"
)]
fn edges_and_outline(ctx: &mut Context<'_>, in_region: &Region<'_>) {
    if ctx.board.outline.area.is_none() {
        return;
    }
    let mut found = Vec::new();
    let mut outside: Vec<(ItemRef, V)> = Vec::new();
    for (index, item) in ctx.board.copper.iter().enumerate() {
        if !in_region(item.bbox) {
            continue;
        }
        if let ItemRef::Pad {
            footprint,
            index: pad,
        } = item.item
        {
            let connector = ctx
                .board
                .footprint(footprint)
                .and_then(|fp| fp.pads.get(pad))
                .is_some_and(|p| p.kind == PadKind::Connector);
            if connector {
                continue;
            }
        }
        let layer = ctx.first_layer(item.layers);
        let props = ctx.props(item, layer);
        let limit = ctx.rules.edge_clearance(&props, &ctx.zones);
        if let Some(gap) = ctx
            .board
            .outline
            .edge_gap(&item.shape, limit.value - EPSILON)
        {
            found.push((index, gap.max(0.0), limit, layer));
        } else if matches!(item.item, ItemRef::Track { .. } | ItemRef::Via { .. })
            && !ctx.board.outline.contains(item.shape.anchor())
        {
            outside.push((item.item, item.shape.anchor()));
        }
    }
    let mut seen = HashSet::new();
    for (index, gap, limit, layer) in found {
        let item = ctx.board.copper[index].item;
        if !seen.insert(item) {
            continue;
        }
        let label = ctx.label(index);
        let at = ctx.board.copper[index].shape.anchor();
        ctx.report(
            "copper-to-edge",
            format!(
                "{label} is {} from the board edge, less than {} ({})",
                mm(gap),
                mm(limit.value),
                limit.source
            ),
            at,
            vec![Target::of(item)],
            Detail {
                args: vec![
                    ("item", label),
                    ("actual", mm(gap)),
                    ("required", mm(limit.value)),
                    ("source", limit.source.to_string()),
                ],
                layer,
            },
        );
    }
    let mut seen = HashSet::new();
    for (item, at) in outside {
        if seen.insert(item) {
            let label = ctx.describe(item);
            ctx.report(
                "outside-board",
                format!("{label} lies outside the board"),
                at,
                vec![Target::of(item)],
                Detail {
                    args: vec![("item", label)],
                    layer: None,
                },
            );
        }
    }
    let pcb = ctx.board.pcb;
    for fp in &pcb.footprints {
        let at = V::of(fp.at);
        if !in_region(BBox::of(at)) || fp.pads.is_empty() {
            continue;
        }
        let xf = crate::items::footprint_xform(fp);
        let all_outside = fp
            .pads
            .iter()
            .all(|pad| !ctx.board.outline.contains(xf.apply(V::of(pad.at))));
        if all_outside {
            ctx.report(
                "outside-board",
                format!("{} lies outside the board", fp.designator),
                at,
                vec![Target::object(fp.id)],
                Detail {
                    args: vec![("item", fp.designator.clone())],
                    layer: None,
                },
            );
        }
    }
}

fn keepouts(ctx: &mut Context<'_>, in_region: &Region<'_>) {
    let mut found = Vec::new();
    for area in &ctx.board.keepouts {
        let bbox = area.shape.bbox();
        if !in_region(bbox) {
            continue;
        }
        for j in ctx.board.copper_near(bbox) {
            let item = &ctx.board.copper[j];
            if item.layers & area.layers == 0 {
                continue;
            }
            let forbidden = match item.item {
                ItemRef::Track { .. } => area.rules.tracks,
                ItemRef::Via { .. } => area.rules.vias,
                ItemRef::Pad { .. } => area.rules.pads || area.rules.footprints,
                ItemRef::Zone { id, .. } => area.rules.pours && id != area.zone,
            };
            if forbidden && area.shape.gap(&item.shape, 0.0).is_some() {
                found.push((area.zone, area.name.clone(), j));
            }
        }
    }
    let mut seen = HashSet::new();
    for (zone, name, j) in found {
        let item = ctx.board.copper[j].item;
        if !seen.insert((zone, item)) {
            continue;
        }
        let label = ctx.label(j);
        let at = ctx.board.copper[j].shape.anchor();
        let area = if name.is_empty() {
            "a keep-out area".to_owned()
        } else {
            format!("keep-out area {name}")
        };
        ctx.report(
            "keepout",
            format!("{label} lies in {area}"),
            at,
            vec![Target::of(item), Target::object(zone)],
            Detail {
                args: vec![("item", label), ("area", area)],
                layer: ctx.first_layer(ctx.board.copper[j].layers),
            },
        );
    }
}

fn courtyards(ctx: &mut Context<'_>, in_region: &Region<'_>) {
    let boxes: Vec<BBox> = ctx
        .board
        .courtyards
        .iter()
        .map(|c| c.shapes.iter().fold(BBox::EMPTY, |b, s| b.union(s.bbox())))
        .collect();
    let mut found = Vec::new();
    for (i, a) in ctx.board.courtyards.iter().enumerate() {
        if !in_region(boxes[i]) {
            continue;
        }
        for (j, b) in ctx.board.courtyards.iter().enumerate().skip(i + 1) {
            if a.side != b.side || !boxes[i].intersects(boxes[j]) {
                continue;
            }
            let overlap = a.shapes.iter().any(|sa| {
                b.shapes
                    .iter()
                    .any(|sb| sa.gap(sb, 0.0).is_some() && overlap_area(sa, sb) > 1e8)
            });
            if overlap {
                found.push((a.footprint, b.footprint, meeting_point(boxes[i], boxes[j])));
            }
        }
    }
    for (a, b, at) in found {
        let name = |id: Id| {
            ctx.board
                .footprint(id)
                .map_or_else(String::new, |fp| fp.designator.clone())
        };
        let (na, nb) = (name(a), name(b));
        ctx.report(
            "courtyard-overlap",
            format!("The courtyards of {na} and {nb} overlap"),
            at,
            vec![Target::object(a), Target::object(b)],
            Detail {
                args: vec![("a", na), ("b", nb)],
                layer: None,
            },
        );
    }
}

fn overlap_area(a: &Shape, b: &Shape) -> f64 {
    let (pa, pb) = (a.outlines(0.0, 1_000.0), b.outlines(0.0, 1_000.0));
    crate::poly::area(&crate::poly::intersection(&pa, &pb))
}

fn silkscreen(ctx: &mut Context<'_>, in_region: &Region<'_>) {
    let exposed: HashSet<(Side, usize)> = ctx.board.exposed.iter().copied().collect();
    let min_width = ctx.rules.constraints.min_silk_width as f64;
    let mut over = Vec::new();
    let mut thin: HashMap<Id, (f64, V)> = HashMap::new();
    for silk in &ctx.board.silk {
        let bbox = silk.shape.bbox();
        if !in_region(bbox) {
            continue;
        }
        if silk.width > 0.0 && silk.width < min_width {
            thin.entry(silk.object)
                .or_insert((silk.width, silk.shape.anchor()));
        }
        for j in ctx.board.copper_near(bbox) {
            if !exposed.contains(&(silk.side, j)) {
                continue;
            }
            if silk.shape.gap(&ctx.board.copper[j].shape, 0.0).is_some() {
                over.push((
                    silk.object,
                    j,
                    meeting_point(bbox, ctx.board.copper[j].bbox),
                ));
            }
        }
    }
    let mut seen = HashSet::new();
    for (object, j, at) in over {
        let item = ctx.board.copper[j].item;
        if !seen.insert((object, item)) {
            continue;
        }
        let label = ctx.label(j);
        let owner = ctx.board.footprint(object).map_or_else(
            || "Silkscreen".to_owned(),
            |fp| format!("Silkscreen of {}", fp.designator),
        );
        ctx.report(
            "silk-over-pad",
            format!("{owner} lies on {label}"),
            at,
            vec![Target::object(object), Target::of(item)],
            Detail {
                args: vec![("owner", owner.clone()), ("item", label)],
                layer: None,
            },
        );
    }
    let mut thin: Vec<(Id, (f64, V))> = thin.into_iter().collect();
    thin.sort_by_key(|(id, _)| *id);
    for (object, (width, at)) in thin {
        ctx.report(
            "silk-width",
            format!(
                "Silkscreen line width {} is less than {} (board constraints)",
                mm(width),
                mm(min_width)
            ),
            at,
            vec![Target::object(object)],
            Detail {
                args: vec![("actual", mm(width)), ("required", mm(min_width))],
                layer: None,
            },
        );
    }
}

fn dangling(ctx: &mut Context<'_>, conn: &Connectivity, in_region: &Region<'_>) {
    let mut ends: Vec<(ItemRef, V, Option<CopperLayer>)> = Vec::new();
    let mut vias: Vec<(ItemRef, V)> = Vec::new();
    for (i, item) in ctx.board.copper.iter().enumerate() {
        if !in_region(item.bbox) {
            continue;
        }
        match item.item {
            ItemRef::Track { .. } => {
                for end in &item.anchors {
                    let probe = Shape::disc(*end, 0.0);
                    let connected = conn.touching[i].iter().any(|&j| {
                        let other = &ctx.board.copper[j];
                        other.item != item.item && other.shape.gap(&probe, 1.0).is_some()
                    });
                    if !connected {
                        ends.push((item.item, *end, ctx.first_layer(item.layers)));
                    }
                }
            }
            ItemRef::Via { .. } => {
                let others: HashSet<ItemRef> = conn.touching[i]
                    .iter()
                    .map(|&j| ctx.board.copper[j].item)
                    .collect();
                if others.len() < 2 {
                    vias.push((item.item, item.shape.anchor()));
                }
            }
            _ => {}
        }
    }
    for (item, at, layer) in ends {
        ctx.report(
            "dangling-track",
            "A track end is not connected".to_owned(),
            at,
            vec![Target::of(item)],
            Detail {
                args: Vec::new(),
                layer,
            },
        );
    }
    for (item, at) in vias {
        ctx.report(
            "dangling-via",
            "A via connects fewer than two items".to_owned(),
            at,
            vec![Target::of(item)],
            Detail::default(),
        );
    }
}

fn board_checks(ctx: &mut Context<'_>, conn: &Connectivity) {
    let pcb = ctx.board.pcb;
    let outline = ctx.board.outline.clone();
    if outline.rings.is_empty() && outline.open_ends.is_empty() {
        ctx.report(
            "no-outline",
            "The board has no outline".to_owned(),
            V::default(),
            Vec::new(),
            Detail::default(),
        );
    }
    for pair in outline.open_ends.chunks(2) {
        ctx.report(
            "outline-open",
            "The board outline is not closed here".to_owned(),
            pair[0],
            Vec::new(),
            Detail::default(),
        );
    }
    for wire in &conn.airwires {
        let (a, b) = (
            V::new(wire.from[0], wire.from[1]),
            V::new(wire.to[0], wire.to[1]),
        );
        let net = ctx
            .nets
            .get(&wire.net)
            .map(|n| n.name.clone())
            .unwrap_or_default();
        let (la, lb) = (ctx.describe(wire.from_item), ctx.describe(wire.to_item));
        ctx.report(
            "unrouted",
            format!("Net {net}: {la} is not connected to {lb}"),
            a.add(b).scale(0.5),
            vec![Target::of(wire.from_item), Target::of(wire.to_item)],
            Detail {
                args: vec![("net", net), ("a", la), ("b", lb)],
                layer: None,
            },
        );
    }
    for zone in &pcb.zones {
        if zone.keepout.is_none() && zone.filled.is_empty() && zone.outline.len() >= 3 {
            let at = V::of(zone.outline[0]);
            ctx.report(
                "zone-unfilled",
                "The zone is not filled".to_owned(),
                at,
                vec![Target::object(zone.id)],
                Detail::default(),
            );
        }
    }
    let mut designators: HashMap<&str, Vec<Id>> = HashMap::new();
    for fp in &pcb.footprints {
        if !fp.designator.is_empty() {
            designators
                .entry(fp.designator.as_str())
                .or_default()
                .push(fp.id);
        }
    }
    let mut duplicates: Vec<(&str, Vec<Id>)> = designators
        .into_iter()
        .filter(|(_, ids)| ids.len() > 1)
        .collect();
    duplicates.sort();
    for (name, ids) in duplicates {
        let at = pcb
            .footprints
            .get(ids[0])
            .map_or(V::default(), |fp| V::of(fp.at));
        ctx.report(
            "duplicate-designator",
            format!("{} footprints are called {name}", ids.len()),
            at,
            ids.into_iter().map(Target::object).collect(),
            Detail {
                args: vec![("designator", name.to_owned())],
                layer: None,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::connectivity::tests::{board_with, pad, track};
    use ew_model::layer::Layer;
    use ew_model::pcb::{BoardShape, Net, Via, Zone, ZoneFill};
    use ew_model::shape::Geometry;
    use ew_model::units::um;

    fn outline(pcb: &mut Pcb, w: i64, h: i64) {
        pcb.shapes.insert(BoardShape {
            id: Id::random(),
            layer: Layer::Outline,
            geometry: Geometry::Rect {
                start: Point::mm(-5, -5),
                end: Point::mm(w, h),
            },
            width: um(100),
            filled: false,
        });
    }

    fn codes(report: &Report) -> Vec<&str> {
        report.findings.iter().map(|f| f.code.as_str()).collect()
    }

    fn nets(pcb: &mut Pcb) -> (Id, Id) {
        let (gnd, vcc) = (Id::random(), Id::random());
        for (id, name) in [(gnd, "GND"), (vcc, "VCC")] {
            pcb.nets.insert(Net {
                id,
                name: name.into(),
                class: String::new(),
            });
        }
        (gnd, vcc)
    }

    #[test]
    fn a_routed_board_is_clean() {
        let mut pcb = board_with(Vec::new());
        let (gnd, _) = nets(&mut pcb);
        let fp = pcb.footprints.iter().next().unwrap().id;
        let mut fp_obj = pcb.footprints.get(fp).unwrap().clone();
        fp_obj.pads = vec![
            pad("1", Point::mm(0, 0), Some(gnd)),
            pad("2", Point::mm(10, 0), Some(gnd)),
        ];
        pcb.footprints.insert(fp_obj);
        outline(&mut pcb, 20, 10);
        pcb.tracks.insert(track(
            gnd,
            Point::mm(0, 0),
            Point::mm(10, 0),
            CopperLayer::Top,
        ));
        let report = check(&pcb, None, &Options::default());
        assert!(report.findings.is_empty(), "{:#?}", report.findings);
    }

    #[test]
    fn problems_are_found() {
        let mut pcb = board_with(Vec::new());
        let (gnd, vcc) = nets(&mut pcb);
        let fp = pcb.footprints.iter().next().unwrap().id;
        let mut fp_obj = pcb.footprints.get(fp).unwrap().clone();
        fp_obj.pads = vec![
            pad("1", Point::mm(0, 0), Some(gnd)),
            pad("2", Point::mm(10, 0), Some(gnd)),
            pad("3", Point::mm(0, 3), Some(vcc)),
            pad("4", Point::mm(10, 5), Some(vcc)),
        ];
        pcb.footprints.insert(fp_obj);
        outline(&mut pcb, 20, 10);
        // Too thin, and 0.1 mm away from the VCC pad (edge at y = 2.5 mm).
        let mut thin = track(gnd, Point::mm(0, 0), Point::mm(10, 0), CopperLayer::Top);
        thin.width = um(100);
        thin.start = Point::new(0, 0);
        thin.end = Point::new(10_000_000, 0);
        pcb.tracks.insert(thin);
        let mut close = track(gnd, Point::mm(10, 0), Point::mm(10, 0), CopperLayer::Top);
        close.start = Point::new(10_000_000, 0);
        close.end = Point::new(300_000, 2_200_000);
        pcb.tracks.insert(close);
        // A via too small, at the board edge, connected to nothing else.
        pcb.vias.insert(Via {
            id: Id::random(),
            at: Point::new(19_900_000, 5_000_000),
            diameter: um(400),
            drill: um(250),
            net: Some(vcc),
            from: CopperLayer::Top,
            to: CopperLayer::Bottom,
            micro: false,
            locked: false,
        });
        pcb.zones.insert(Zone {
            id: Id::random(),
            name: "GND".into(),
            net: Some(gnd),
            layers: vec![CopperLayer::Bottom],
            outline: vec![Point::mm(0, 0), Point::mm(5, 0), Point::mm(5, 5)],
            holes: Vec::new(),
            priority: 0,
            keepout: None,
            fill: ZoneFill::default(),
            filled: Vec::new(),
        });
        let report = check(&pcb, None, &Options::default());
        let codes = codes(&report);
        for code in [
            "track-width",
            "clearance",
            "via-diameter",
            "via-drill",
            "annular-ring",
            "copper-to-edge",
            "dangling-via",
            "zone-unfilled",
            "unrouted",
        ] {
            assert!(codes.contains(&code), "{code} missing in {codes:?}");
        }
        let clearance = report
            .findings
            .iter()
            .find(|f| f.code == "clearance")
            .unwrap();
        assert_eq!(clearance.args["required"], "0.2 mm");
        assert_eq!(clearance.layer.as_deref(), Some("cu.top"));
        // Accepting a finding keeps it but stops counting it.
        let errors = report.counts.errors;
        pcb.drc_exclusions.push(clearance.key.clone());
        let again = check(&pcb, None, &Options::default());
        assert_eq!(again.counts.errors, errors - 1);
        assert_eq!(again.counts.excluded, 1);
        // Severities follow the settings.
        let mut rules = Rules::new(Id::random(), "Rules");
        rules
            .drc
            .severities
            .insert("dangling-via".into(), ErcSeverity::Ignore);
        let quiet = check(&pcb, Some(&rules), &Options::default());
        assert!(!quiet.findings.iter().any(|f| f.code == "dangling-via"));
        // A short.
        pcb.tracks.insert(track(
            vcc,
            Point::mm(10, -2),
            Point::mm(10, 2),
            CopperLayer::Top,
        ));
        let shorted = check(&pcb, None, &Options::default());
        assert!(shorted.findings.iter().any(|f| f.code == "short"));
    }

    #[test]
    fn region_checks_merge_with_the_previous_report() {
        let mut pcb = board_with(Vec::new());
        let (gnd, vcc) = nets(&mut pcb);
        outline(&mut pcb, 40, 10);
        pcb.tracks.insert(track(
            gnd,
            Point::mm(0, 0),
            Point::mm(10, 0),
            CopperLayer::Top,
        ));
        pcb.tracks.insert(track(
            vcc,
            Point::new(0, 300_000),
            Point::new(10_000_000, 300_000),
            CopperLayer::Top,
        ));
        let full = check(&pcb, None, &Options::default());
        assert_eq!(
            full.findings
                .iter()
                .filter(|f| f.code == "clearance")
                .count(),
            1
        );
        // Move the VCC track away and check only around it.
        let id = pcb.tracks.iter().find(|t| t.net == Some(vcc)).unwrap().id;
        let mut moved = pcb.tracks.get(id).unwrap().clone();
        let old = BBox::of(V::of(moved.start)).with(V::of(moved.end));
        moved.start.y = 5_000_000;
        moved.end.y = 5_000_000;
        let new = BBox::of(V::of(moved.start)).with(V::of(moved.end));
        pcb.tracks.insert(moved);
        let region = old.union(new);
        let fresh = check(
            &pcb,
            None,
            &Options {
                region: Some(region),
                ..Options::default()
            },
        );
        let merged = merge(&full, fresh, region, 200_000.0);
        assert!(!merged.findings.iter().any(|f| f.code == "clearance"));
    }
}
