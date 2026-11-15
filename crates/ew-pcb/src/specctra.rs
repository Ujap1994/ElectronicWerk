//! Specctra DSN export and SES import, for external autorouters such as
//! Freerouting.
//!
//! The DSN file describes the board for the router: copper layers, the
//! outline, keep-outs and pours, every part with its pads, the nets with
//! their net classes (width, clearance, via) and the tracks and vias that
//! exist already. The router writes a session file (SES) with the routed
//! wires and vias; [`plan_session`] turns it into tracks and vias of the
//! board, replacing the unlocked routing of the nets it routed.
//!
//! Parts are written as placed: every part has an image of its own with
//! the pads where they are on the board (placed at rotation 0 on the
//! front), so rotation and mirroring conventions of routers do not matter.
//! DSN coordinates are micrometres with the y axis pointing up, like the
//! board's.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, LayerSpec, Side};
use ew_model::pcb::{PadKind, Pcb, Track, Via};
use ew_model::units::{Nm, Point};

use crate::geom::{Core, Shape, V, arc_points};
use crate::items::{footprint_xform, outline, pad_hole, pad_shape};
use crate::rules::RuleSet;

/// Nanometres per DSN unit (micrometres).
const UNIT: f64 = 1_000.0;
/// Arcs of tracks are written as segments within this deviation.
const ARC_TOLERANCE: f64 = 5_000.0;

/// Why a session file cannot be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct SpecctraError(pub String);

/// The DSN layer name of a copper layer (KiCad's names, which routers
/// know).
#[must_use]
pub fn layer_name(layer: CopperLayer) -> String {
    match layer {
        CopperLayer::Top => "F.Cu".to_owned(),
        CopperLayer::Bottom => "B.Cu".to_owned(),
        CopperLayer::Inner(n) => format!("In{n}.Cu"),
    }
}

/// A token, quoted when it is not a plain word.
fn quote(text: &str) -> String {
    let plain = !text.is_empty()
        && text.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(
                    c,
                    '_' | '.'
                        | '+'
                        | '-'
                        | '/'
                        | '$'
                        | '['
                        | ']'
                        | ':'
                        | '@'
                        | '#'
                        | '~'
                        | '!'
                        | '*'
                        | '&'
                        | '%'
                        | '^'
                        | '='
                        | '<'
                        | '>'
                        | '?'
                        | '|'
                        | ','
                        | ';'
                )
        });
    if plain {
        text.to_owned()
    } else {
        format!("\"{}\"", text.replace('"', "'"))
    }
}

/// A coordinate in DSN units (micrometres, one decimal).
fn num(value: f64) -> String {
    let text = format!("{:.1}", value / UNIT);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" || text.is_empty() {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

/// A board point in DSN coordinates.
fn xy(p: V) -> String {
    format!("{} {}", num(p.x), num(p.y))
}

/// A relative offset in DSN coordinates (pads around their pin).
fn rel(p: V, origin: V) -> String {
    xy(p.sub(origin))
}

/// The name of a via padstack (KiCad's scheme, read back on import):
/// `Via[0-1]_600:300_um` is a through via of a 2-layer board.
fn via_name(from: usize, to: usize, diameter: Nm, drill: Nm) -> String {
    format!(
        "Via[{from}-{to}]_{}:{}_um",
        num(diameter as f64),
        num(drill as f64)
    )
}

/// The diameter, drill and layer indexes of a via padstack name.
fn parse_via_name(name: &str) -> Option<(usize, usize, Nm, Nm)> {
    let rest = name.strip_prefix("Via[")?;
    let (span, rest) = rest.split_once("]_")?;
    let (from, to) = span.split_once('-')?;
    let sizes = rest.strip_suffix("_um")?;
    let (diameter, drill) = sizes.split_once(':')?;
    let um = |text: &str| -> Option<Nm> {
        let value: f64 = text.parse().ok()?;
        Some((value * UNIT).round() as Nm)
    };
    Some((
        from.parse().ok()?,
        to.parse().ok()?,
        um(diameter)?,
        um(drill)?,
    ))
}

/// What the DSN export needs besides the board.
pub struct DsnInput<'a> {
    /// The board.
    pub pcb: &'a Pcb,
    /// The design rules (net classes).
    pub rules: &'a RuleSet,
    /// The program and its version (`ElectronicWerk 0.15.0`).
    pub host: &'a str,
}

/// One part of the DSN file: its image, padstacks and pin references.
struct Part {
    designator: String,
    image: String,
    at: V,
    /// (padstack, pin id, position).
    pins: Vec<(String, String, V)>,
    /// Holes without copper (keep-outs of the image).
    holes: Vec<(f64, V)>,
}

/// Writes the DSN file of a board.
#[must_use]
#[allow(clippy::too_many_lines, reason = "the sections of one file")]
pub fn write_dsn(input: &DsnInput<'_>) -> String {
    let pcb = input.pcb;
    let copper = pcb.stackup.copper_count();
    let layers = CopperLayer::stack(copper);
    let index_of = |layer: CopperLayer| layers.iter().position(|&l| l == layer).unwrap_or(0);
    let net_names: HashMap<Id, String> = pcb
        .nets
        .iter()
        .map(|net| (net.id, net.name.clone()))
        .collect();

    // Padstacks by their content, so equal pads share one.
    let mut padstacks: BTreeMap<String, String> = BTreeMap::new();
    let mut padstack_of = |shapes: String| -> String {
        let count = padstacks.len();
        padstacks
            .entry(shapes)
            .or_insert_with(|| format!("Pad_{}", count + 1))
            .clone()
    };

    // Parts, with unique designators and pin names.
    let mut used_refs: BTreeSet<String> = BTreeSet::new();
    let mut parts: Vec<Part> = Vec::new();
    // Net → pin references.
    let mut net_pins: BTreeMap<Id, Vec<String>> = BTreeMap::new();
    for (index, fp) in pcb.footprints.iter().enumerate() {
        let mut designator = fp.designator.trim().to_owned();
        if designator.is_empty() || used_refs.contains(&designator) {
            designator = format!(
                "{}_{}",
                if designator.is_empty() {
                    "X"
                } else {
                    &designator
                },
                index + 1
            );
        }
        used_refs.insert(designator.clone());
        let xf = footprint_xform(fp);
        let at = V::of(fp.at);
        let flip = |layer: Layer| {
            if fp.side == Side::Bottom {
                layer.flipped(copper)
            } else {
                layer
            }
        };
        let mut part = Part {
            image: format!("{}_{}", fp.footprint, designator),
            designator,
            at,
            pins: Vec::new(),
            holes: Vec::new(),
        };
        let mut pin_names: BTreeMap<String, usize> = BTreeMap::new();
        for (pad_index, pad) in fp.pads.iter().enumerate() {
            let mut on: Vec<CopperLayer> = Vec::new();
            for spec in &pad.layers {
                match *spec {
                    LayerSpec::AllCopper => on.extend(layers.iter().copied()),
                    LayerSpec::Layer(layer) => {
                        if let Layer::Copper(c) = flip(layer) {
                            on.push(c);
                        }
                    }
                    LayerSpec::AllMask | LayerSpec::AllPaste => {}
                }
            }
            on.sort_by_key(|&l| index_of(l));
            on.dedup();
            let shape = pad_shape(pad, &xf, 0.0);
            let center = pad_center(&shape);
            if on.is_empty() || pad.kind == PadKind::NpThroughHole && pad.net.is_none() {
                // A hole without copper: a keep-out for the router.
                if let Some(hole) = pad_hole(pad, &xf) {
                    let b = hole.bbox();
                    let size = (b.max.x - b.min.x).max(b.max.y - b.min.y);
                    part.holes.push((size, center));
                }
                if on.is_empty() {
                    continue;
                }
            }
            let shapes: String = on
                .iter()
                .map(|&layer| shape_text(&shape, &layer_name(layer), center))
                .collect::<Vec<_>>()
                .join(" ");
            let padstack = padstack_of(shapes);
            // Pin names are unique in an image; doubles get `@n` (KiCad).
            let base = if pad.number.trim().is_empty() {
                format!("@{}", pad_index + 1)
            } else {
                pad.number.trim().to_owned()
            };
            let seen = pin_names.entry(base.clone()).or_insert(0);
            let pin = if *seen == 0 {
                base.clone()
            } else {
                format!("{base}@{seen}")
            };
            *seen += 1;
            if let Some(net) = pad.net.filter(|id| net_names.contains_key(id)) {
                net_pins
                    .entry(net)
                    .or_default()
                    .push(format!("{}-{pin}", part.designator));
            }
            part.pins.push((padstack, pin, center));
        }
        parts.push(part);
    }

    // Vias: the net classes' and the existing ones.
    let mut vias: BTreeSet<String> = BTreeSet::new();
    let through = |d: Nm, h: Nm| via_name(0, layers.len().saturating_sub(1), d, h);
    let mut class_nets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for net in &pcb.nets {
        class_nets
            .entry(input.rules.class_name(&net.name, &net.class))
            .or_default()
            .push(net.name.clone());
    }
    let default_class = input.rules.class(ew_model::rules::DEFAULT_NET_CLASS);
    let default_via = through(default_class.via_diameter, default_class.via_drill);
    vias.insert(default_via.clone());
    for name in class_nets.keys() {
        let class = input.rules.class(name);
        vias.insert(through(class.via_diameter, class.via_drill));
    }
    for via in &pcb.vias {
        vias.insert(via_name(
            index_of(via.from),
            index_of(via.to),
            via.diameter,
            via.drill,
        ));
    }

    let mut out = String::new();
    let name = quote(&format!("{}.dsn", pcb.name));
    let _ = writeln!(out, "(pcb {name}");
    out.push_str("  (parser\n    (string_quote \")\n    (space_in_quoted_tokens on)\n");
    let _ = writeln!(out, "    (host_cad {})\n  )", quote(input.host));
    out.push_str("  (resolution um 10)\n  (unit um)\n  (structure\n");
    for (index, &layer) in layers.iter().enumerate() {
        let _ = writeln!(
            out,
            "    (layer {} (type signal) (property (index {index})))",
            layer_name(layer)
        );
    }
    // The board outline; cut-outs are keep-outs.
    let board = outline(pcb);
    if let Some(ring) = board.rings.first() {
        let mut path: Vec<String> = ring.iter().map(|&p| xy(p)).collect();
        if let Some(first) = path.first().cloned() {
            path.push(first);
        }
        let _ = writeln!(out, "    (boundary (path pcb 0 {}))", path.join(" "));
        for cutout in &board.rings[1..] {
            let points: Vec<String> = cutout.iter().map(|&p| xy(p)).collect();
            let _ = writeln!(
                out,
                "    (keepout \"\" (polygon signal 0 {}))",
                points.join(" ")
            );
        }
    }
    for zone in &pcb.zones {
        let points: Vec<String> = zone.outline.iter().map(|&p| xy(V::of(p))).collect();
        if points.len() < 3 {
            continue;
        }
        let points = points.join(" ");
        if let Some(keepout) = zone.keepout {
            for &layer in &zone.layers {
                let layer = layer_name(layer);
                if keepout.tracks {
                    let _ = writeln!(out, "    (keepout \"\" (polygon {layer} 0 {points}))");
                }
                if keepout.vias {
                    let _ = writeln!(out, "    (via_keepout \"\" (polygon {layer} 0 {points}))");
                }
            }
        } else if let Some(net) = zone.net.and_then(|id| net_names.get(&id)) {
            for &layer in &zone.layers {
                let _ = writeln!(
                    out,
                    "    (plane {} (polygon {} 0 {points}))",
                    quote(net),
                    layer_name(layer)
                );
            }
        }
    }
    let via_list: Vec<String> = vias.iter().map(|v| quote(v)).collect();
    let _ = writeln!(out, "    (via {})", via_list.join(" "));
    let _ = writeln!(
        out,
        "    (rule (width {}) (clearance {}) (clearance {} (type smd_smd)))",
        num(default_class.track_width as f64),
        num(default_class.clearance as f64),
        num((default_class.clearance as f64 / 4.0).max(10_000.0)),
    );
    out.push_str("  )\n  (placement\n");
    for part in &parts {
        let _ = writeln!(
            out,
            "    (component {} (place {} {} front 0))",
            quote(&part.image),
            quote(&part.designator),
            xy(part.at)
        );
    }
    out.push_str("  )\n  (library\n");
    for part in &parts {
        let _ = writeln!(out, "    (image {}", quote(&part.image));
        for (padstack, pin, center) in &part.pins {
            let _ = writeln!(
                out,
                "      (pin {padstack} {} {})",
                quote(pin),
                rel(*center, part.at)
            );
        }
        for (size, center) in &part.holes {
            let _ = writeln!(
                out,
                "      (keepout \"\" (circle signal {} {}))",
                num(*size),
                rel(*center, part.at)
            );
        }
        out.push_str("    )\n");
    }
    for (shapes, name) in &padstacks {
        let _ = writeln!(out, "    (padstack {name} {shapes} (attach off))");
    }
    for via in &vias {
        if let Some((from, to, diameter, _)) = parse_via_name(via) {
            let shapes: Vec<String> = layers[from.min(layers.len() - 1)..=to.min(layers.len() - 1)]
                .iter()
                .map(|&l| {
                    format!(
                        "(shape (circle {} {}))",
                        layer_name(l),
                        num(diameter as f64)
                    )
                })
                .collect();
            let _ = writeln!(
                out,
                "    (padstack {} {} (attach off))",
                quote(via),
                shapes.join(" ")
            );
        }
    }
    out.push_str("  )\n  (network\n");
    for net in &pcb.nets {
        let mut pins: Vec<String> = net_pins
            .get(&net.id)
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .map(|p| quote(p))
            .collect();
        // The same file for the same board, whatever the order of the parts.
        pins.sort();
        let _ = writeln!(
            out,
            "    (net {} (pins {}))",
            quote(&net.name),
            pins.join(" ")
        );
    }
    for (class_name, nets) in &class_nets {
        let class = input.rules.class(class_name);
        let nets: Vec<String> = nets.iter().map(|n| quote(n)).collect();
        let _ = writeln!(
            out,
            "    (class {} {} (circuit (use_via {})) (rule (width {}) (clearance {})))",
            quote(class_name),
            nets.join(" "),
            quote(&through(class.via_diameter, class.via_drill)),
            num(class.track_width as f64),
            num(class.clearance as f64),
        );
    }
    out.push_str("  )\n  (wiring\n");
    for track in &pcb.tracks {
        let Some(net) = track.net.and_then(|id| net_names.get(&id)) else {
            continue;
        };
        let points = match track.mid {
            Some(mid) => arc_points(
                V::of(track.start),
                V::of(mid),
                V::of(track.end),
                ARC_TOLERANCE,
            ),
            None => vec![V::of(track.start), V::of(track.end)],
        };
        let path: Vec<String> = points.iter().map(|&p| xy(p)).collect();
        let _ = writeln!(
            out,
            "    (wire (path {} {} {}) (net {}){})",
            layer_name(track.layer),
            num(track.width as f64),
            path.join(" "),
            quote(net),
            if track.locked {
                " (type protect)"
            } else {
                " (type route)"
            }
        );
    }
    for via in &pcb.vias {
        let Some(net) = via.net.and_then(|id| net_names.get(&id)) else {
            continue;
        };
        let _ = writeln!(
            out,
            "    (via {} {} (net {}){})",
            quote(&via_name(
                index_of(via.from),
                index_of(via.to),
                via.diameter,
                via.drill
            )),
            xy(V::of(via.at)),
            quote(net),
            if via.locked {
                " (type protect)"
            } else {
                " (type route)"
            }
        );
    }
    out.push_str("  )\n)\n");
    out
}

/// The centre of a pad's copper (the pin position).
fn pad_center(shape: &Shape) -> V {
    match &shape.core {
        Core::Point(p) => *p,
        Core::Segment(a, b) => a.add(*b).scale(0.5),
        Core::Poly(poly) => {
            let b = poly.bbox();
            V::new(
                f64::midpoint(b.min.x, b.max.x),
                f64::midpoint(b.min.y, b.max.y),
            )
        }
    }
}

/// The padstack shape of a pad on one layer, relative to its pin.
fn shape_text(shape: &Shape, layer: &str, center: V) -> String {
    match &shape.core {
        Core::Point(p) => {
            let offset = p.sub(center);
            if offset.len() < 1.0 {
                format!("(shape (circle {layer} {}))", num(2.0 * shape.radius))
            } else {
                format!(
                    "(shape (circle {layer} {} {}))",
                    num(2.0 * shape.radius),
                    rel(*p, center)
                )
            }
        }
        Core::Segment(a, b) => format!(
            "(shape (path {layer} {} {} {}))",
            num(2.0 * shape.radius),
            rel(*a, center),
            rel(*b, center)
        ),
        Core::Poly(_) => {
            let rings = shape.outlines(0.0, 1_000.0);
            let ring = rings
                .first()
                .and_then(|rings| rings.first())
                .cloned()
                .unwrap_or_default();
            let points: Vec<String> = ring.iter().map(|&p| rel(p, center)).collect();
            format!("(shape (polygon {layer} 0 {}))", points.join(" "))
        }
    }
}

// --- Sessions ----------------------------------------------------------------

/// A routed wire of a session.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionWire {
    /// DSN layer name.
    pub layer: String,
    /// Width in nanometres.
    pub width: f64,
    /// Points on the board (nanometres, y down).
    pub points: Vec<V>,
    /// A wire the router was not allowed to change.
    pub protect: bool,
}

/// A via of a session.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionVia {
    /// Padstack name.
    pub padstack: String,
    /// Position on the board.
    pub at: V,
    /// A via the router was not allowed to change.
    pub protect: bool,
}

/// The routing of one net in a session.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SessionNet {
    /// Net name.
    pub name: String,
    /// Wires.
    pub wires: Vec<SessionWire>,
    /// Vias.
    pub vias: Vec<SessionVia>,
}

/// A Specctra session file (the router's result).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Session {
    /// The routed nets.
    pub nets: Vec<SessionNet>,
    /// Via padstacks of the session: name → copper diameter (nm).
    pub padstacks: BTreeMap<String, f64>,
}

/// A parsed S-expression.
#[derive(Debug, Clone, PartialEq)]
enum Node {
    Atom(String),
    List(Vec<Node>),
}

impl Node {
    fn head(&self) -> Option<&str> {
        match self {
            Self::List(items) => match items.first() {
                Some(Self::Atom(a)) => Some(a.as_str()),
                _ => None,
            },
            Self::Atom(_) => None,
        }
    }

    fn items(&self) -> &[Self] {
        match self {
            Self::List(items) => items,
            Self::Atom(_) => &[],
        }
    }

    fn atom(&self) -> Option<&str> {
        match self {
            Self::Atom(a) => Some(a),
            Self::List(_) => None,
        }
    }

    fn children<'a>(&'a self, head: &'a str) -> impl Iterator<Item = &'a Self> + 'a {
        self.items().iter().filter(move |n| n.head() == Some(head))
    }

    fn child(&self, head: &str) -> Option<&Self> {
        self.items().iter().find(|n| n.head() == Some(head))
    }
}

fn parse(text: &str) -> Result<Node, SpecctraError> {
    let mut stack: Vec<Vec<Node>> = vec![Vec::new()];
    let mut chars = text.chars().peekable();
    let mut quote_char = '"';
    while let Some(c) = chars.next() {
        match c {
            '(' => stack.push(Vec::new()),
            ')' => {
                let list = stack
                    .pop()
                    .ok_or_else(|| SpecctraError("unbalanced parentheses".into()))?;
                let parent = stack
                    .last_mut()
                    .ok_or_else(|| SpecctraError("unbalanced parentheses".into()))?;
                // `(string_quote ")` changes the quote character.
                if list.first() == Some(&Node::Atom("string_quote".into())) {
                    if let Some(Node::Atom(q)) = list.get(1) {
                        quote_char = q.chars().next().unwrap_or('"');
                    }
                }
                parent.push(Node::List(list));
            }
            c if c.is_whitespace() => {}
            c if c == quote_char
                && stack.last().and_then(|l| l.first())
                    != Some(&Node::Atom("string_quote".into())) =>
            {
                let mut word = String::new();
                for next in chars.by_ref() {
                    if next == quote_char {
                        break;
                    }
                    word.push(next);
                }
                stack
                    .last_mut()
                    .ok_or_else(|| SpecctraError("unbalanced parentheses".into()))?
                    .push(Node::Atom(word));
            }
            c => {
                let mut word = String::from(c);
                while let Some(&next) = chars.peek() {
                    if next.is_whitespace() || next == '(' || next == ')' {
                        break;
                    }
                    word.push(next);
                    chars.next();
                }
                stack
                    .last_mut()
                    .ok_or_else(|| SpecctraError("unbalanced parentheses".into()))?
                    .push(Node::Atom(word));
            }
        }
    }
    let mut top = stack
        .pop()
        .ok_or_else(|| SpecctraError("empty file".into()))?;
    if !stack.is_empty() {
        return Err(SpecctraError("the file ends inside a list".into()));
    }
    if top.len() != 1 {
        return Err(SpecctraError("not a Specctra file".into()));
    }
    Ok(top.remove(0))
}

/// Nanometres per value of a `(resolution unit count)` list.
fn resolution(node: Option<&Node>) -> f64 {
    let Some(node) = node else {
        return UNIT;
    };
    let items = node.items();
    let unit = match items.get(1).and_then(Node::atom) {
        Some("inch") => 25_400_000.0,
        Some("mil") => 25_400.0,
        Some("cm") => 10_000_000.0,
        Some("mm") => 1_000_000.0,
        _ => UNIT,
    };
    let count: f64 = items
        .get(2)
        .and_then(Node::atom)
        .and_then(|a| a.parse().ok())
        .filter(|&c: &f64| c > 0.0)
        .unwrap_or(1.0);
    unit / count
}

fn number(node: &Node) -> Option<f64> {
    node.atom()?.parse().ok()
}

fn is_protected(node: &Node) -> bool {
    node.child("type")
        .and_then(|t| t.items().get(1))
        .and_then(Node::atom)
        == Some("protect")
}

/// Reads a session file.
///
/// # Errors
///
/// If the text is not a Specctra session.
pub fn read_ses(text: &str) -> Result<Session, SpecctraError> {
    let root = parse(text)?;
    if root.head() != Some("session") {
        return Err(SpecctraError("not a Specctra session file (SES)".into()));
    }
    let routes = root
        .child("routes")
        .ok_or_else(|| SpecctraError("the session has no routes".into()))?;
    let scale = resolution(routes.child("resolution"));
    let point =
        |x: &Node, y: &Node| -> Option<V> { Some(V::new(number(x)? * scale, number(y)? * scale)) };
    let mut session = Session::default();
    if let Some(library) = routes.child("library_out") {
        for padstack in library.children("padstack") {
            let Some(name) = padstack.items().get(1).and_then(Node::atom) else {
                continue;
            };
            let diameter = padstack
                .children("shape")
                .filter_map(|s| s.child("circle"))
                .filter_map(|c| c.items().get(2).and_then(number))
                .fold(0.0_f64, f64::max);
            session.padstacks.insert(name.to_owned(), diameter * scale);
        }
    }
    let Some(network) = routes.child("network_out") else {
        return Ok(session);
    };
    for net in network.children("net") {
        let Some(name) = net.items().get(1).and_then(Node::atom) else {
            continue;
        };
        let mut found = SessionNet {
            name: name.to_owned(),
            ..SessionNet::default()
        };
        for wire in net.children("wire") {
            let Some(path) = wire.child("path").or_else(|| wire.child("polyline_path")) else {
                continue;
            };
            let items = path.items();
            let (Some(layer), Some(width)) = (
                items.get(1).and_then(Node::atom),
                items.get(2).and_then(number),
            ) else {
                continue;
            };
            let points: Vec<V> = items[3..]
                .chunks_exact(2)
                .filter_map(|pair| point(&pair[0], &pair[1]))
                .collect();
            if points.len() >= 2 {
                found.wires.push(SessionWire {
                    layer: layer.to_owned(),
                    width: width * scale,
                    points,
                    protect: is_protected(wire),
                });
            }
        }
        for via in net.children("via") {
            let items = via.items();
            let (Some(padstack), Some(at)) = (
                items.get(1).and_then(Node::atom),
                items
                    .get(2)
                    .zip(items.get(3))
                    .and_then(|(x, y)| point(x, y)),
            ) else {
                continue;
            };
            found.vias.push(SessionVia {
                padstack: padstack.to_owned(),
                at,
                protect: is_protected(via),
            });
        }
        session.nets.push(found);
    }
    Ok(session)
}

/// What importing a session changes on the board.
#[derive(Debug, Clone, Default)]
pub struct SessionPlan {
    /// Unlocked tracks of the routed nets, replaced by the session.
    pub remove_tracks: Vec<Id>,
    /// Unlocked vias of the routed nets.
    pub remove_vias: Vec<Id>,
    /// New tracks.
    pub tracks: Vec<Track>,
    /// New vias.
    pub vias: Vec<Via>,
    /// Net names of the session that the board does not have.
    pub unknown_nets: Vec<String>,
    /// Layer names of the session that the board does not have.
    pub unknown_layers: Vec<String>,
}

/// Turns a session into board changes: the routing of every net in the
/// session replaces the unlocked tracks and vias of that net. Protected
/// wires (the board's locked tracks) stay as they are.
#[must_use]
pub fn plan_session(pcb: &Pcb, rules: &RuleSet, session: &Session) -> SessionPlan {
    let copper = pcb.stackup.copper_count();
    let layers = CopperLayer::stack(copper);
    let by_name: HashMap<String, CopperLayer> =
        layers.iter().map(|&l| (layer_name(l), l)).collect();
    let nets: HashMap<&str, (Id, &str)> = pcb
        .nets
        .iter()
        .map(|net| (net.name.as_str(), (net.id, net.class.as_str())))
        .collect();
    let mut plan = SessionPlan::default();
    let mut unknown_layers = BTreeSet::new();
    let mut routed: BTreeSet<Id> = BTreeSet::new();
    let round = |p: V| -> Point { p.point() };
    for net in &session.nets {
        let Some(&(id, board_class)) = nets.get(net.name.as_str()) else {
            plan.unknown_nets.push(net.name.clone());
            continue;
        };
        routed.insert(id);
        for wire in net.wires.iter().filter(|w| !w.protect) {
            let Some(&layer) = by_name.get(&wire.layer) else {
                unknown_layers.insert(wire.layer.clone());
                continue;
            };
            for pair in wire.points.windows(2) {
                let (start, end) = (round(pair[0]), round(pair[1]));
                if start == end {
                    continue;
                }
                plan.tracks.push(Track {
                    id: Id::random(),
                    layer,
                    net: Some(id),
                    width: wire.width.round() as Nm,
                    start,
                    end,
                    mid: None,
                    locked: false,
                });
            }
        }
        let class = rules.class(&rules.class_name(&net.name, board_class));
        for via in net.vias.iter().filter(|v| !v.protect) {
            let (from, to, diameter, drill) = parse_via_name(&via.padstack).unwrap_or_else(|| {
                let diameter = session
                    .padstacks
                    .get(&via.padstack)
                    .map_or(class.via_diameter, |&d| d.round() as Nm);
                (0, layers.len().saturating_sub(1), diameter, class.via_drill)
            });
            let layer = |index: usize| layers.get(index).copied().unwrap_or(CopperLayer::Bottom);
            plan.vias.push(Via {
                id: Id::random(),
                at: round(via.at),
                diameter,
                drill,
                net: Some(id),
                from: layer(from),
                to: layer(to),
                micro: false,
                locked: false,
            });
        }
    }
    plan.remove_tracks = pcb
        .tracks
        .iter()
        .filter(|t| !t.locked && t.net.is_some_and(|n| routed.contains(&n)))
        .map(|t| t.id)
        .collect();
    plan.remove_vias = pcb
        .vias
        .iter()
        .filter(|v| !v.locked && v.net.is_some_and(|n| routed.contains(&n)))
        .map(|v| v.id)
        .collect();
    plan.unknown_layers = unknown_layers.into_iter().collect();
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn via_names_round_trip() {
        let name = via_name(0, 3, 600_000, 300_000);
        assert_eq!(name, "Via[0-3]_600:300_um");
        assert_eq!(parse_via_name(&name), Some((0, 3, 600_000, 300_000)));
        assert_eq!(
            parse_via_name("Via[0-1]_450.5:200_um"),
            Some((0, 1, 450_500, 200_000))
        );
        assert_eq!(parse_via_name("Round[A]Pad_1524_um"), None);
    }

    #[test]
    fn tokens_are_quoted_when_needed() {
        assert_eq!(quote("GND"), "GND");
        assert_eq!(quote("Net-(R1-Pad2)"), "\"Net-(R1-Pad2)\"");
        assert_eq!(quote("my net"), "\"my net\"");
        assert_eq!(quote(""), "\"\"");
        assert_eq!(num(1_234_567.0), "1234.6");
        assert_eq!(num(-40.0), "0");
    }

    #[test]
    fn sessions_with_resolution_and_quotes() {
        let text = r#"(session "b.ses"
          (base_design "b.dsn")
          (routes
            (resolution um 10)
            (parser (string_quote ") (host_cad "x"))
            (library_out
              (padstack "Via[0-1]_600:300_um" (shape (circle F.Cu 6000 0 0)) (shape (circle B.Cu 6000 0 0)) (attach off)))
            (network_out
              (net "Net-(R1-Pad2)"
                (wire (path F.Cu 2500 10000 -20000 30000 -20000))
                (wire (path B.Cu 2500 0 0 10 10) (type protect))
                (via "Via[0-1]_600:300_um" 30000 -20000)))))"#;
        let session = read_ses(text).unwrap();
        assert_eq!(
            session.padstacks.get("Via[0-1]_600:300_um"),
            Some(&600_000.0)
        );
        let net = &session.nets[0];
        assert_eq!(net.name, "Net-(R1-Pad2)");
        assert_eq!(net.wires[0].width, 250_000.0);
        assert_eq!(
            net.wires[0].points,
            [
                V::new(1_000_000.0, -2_000_000.0),
                V::new(3_000_000.0, -2_000_000.0)
            ]
        );
        assert!(net.wires[1].protect);
        assert_eq!(net.vias[0].at, V::new(3_000_000.0, -2_000_000.0));
        assert!(read_ses("(pcb x)").is_err());
        assert!(read_ses("(session x").is_err());
    }
}
