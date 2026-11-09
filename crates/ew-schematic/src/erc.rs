//! The electrical rules check (ERC): finds pins that are not connected,
//! drivers that fight each other, power inputs without a source and other
//! mistakes in the schematics.
//!
//! The pin type matrix and every check's severity can be changed in the
//! rules document (`erc`); the defaults follow KiCad.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use ew_model::document::Document;
use ew_model::id::Id;
use ew_model::library::PinKind;
use ew_model::project::Project;
use ew_model::rules::{ErcSettings, ErcSeverity};
use ew_model::schematic::LabelKind;
use ew_model::units::Point;
use serde::Serialize;

use crate::connectivity::{Connectivity, ItemKind, PinInfo, natural_cmp};
use crate::geometry::{grid_origin, on_grid, on_segment, sheet_pin_point};

pub use ew_model::rules::{ERC_CHECKS as CHECKS, ERC_PIN_MATRIX};

/// A finding of the ERC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// Check code.
    pub code: String,
    /// Severity (never `ignore`).
    pub severity: ErcSeverity,
    /// What is wrong, in English (the app translates by code).
    pub message: String,
    /// Values for the translated message (`pin`, `net`, `name`, …).
    pub args: BTreeMap<String, String>,
    /// The schematic document.
    pub sheet: Id,
    /// The sheet instance path (`/`, `/Power/`).
    pub path: String,
    /// Where to look.
    pub at: Point,
    /// Objects concerned (placed symbols, wires, labels, …).
    pub items: Vec<Id>,
}

/// The ERC result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErcReport {
    /// Findings, errors first, then by sheet and position.
    pub issues: Vec<Issue>,
    /// Number of errors.
    pub errors: usize,
    /// Number of warnings.
    pub warnings: usize,
    /// Number of nets checked.
    pub nets: usize,
    /// Number of pins checked.
    pub pins: usize,
}

struct Ctx<'a> {
    connectivity: &'a Connectivity,
    severities: HashMap<&'a str, ErcSeverity>,
    issues: Vec<Issue>,
}

impl Ctx<'_> {
    fn report(
        &mut self,
        code: &str,
        instance: usize,
        at: Point,
        items: Vec<Id>,
        args: &[(&str, String)],
        message: String,
    ) {
        let severity = self
            .severities
            .get(code)
            .copied()
            .unwrap_or(ErcSeverity::Error);
        if severity == ErcSeverity::Ignore {
            return;
        }
        let inst = &self.connectivity.instances[instance];
        self.issues.push(Issue {
            code: code.to_owned(),
            severity,
            message,
            args: args
                .iter()
                .map(|(k, v)| ((*k).to_owned(), v.clone()))
                .collect(),
            sheet: inst.sheet,
            path: inst.name.clone(),
            at,
            items,
        });
    }
}

fn pin_label(pin: &PinInfo) -> String {
    if pin.name.is_empty() || pin.name == "~" || pin.name == pin.number {
        format!("{}-{}", pin.designator, pin.number)
    } else {
        format!("{}-{} ({})", pin.designator, pin.number, pin.name)
    }
}

/// The ERC settings of a project: those of its first rules document.
#[must_use]
pub fn settings(project: &Project) -> ErcSettings {
    project
        .documents_of(ew_model::document::DocumentKind::Rules)
        .find_map(|r| match project.document(r.id) {
            Some(Document::Rules(rules)) => Some(rules.erc.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

/// Runs the ERC.
#[must_use]
pub fn check(project: &Project, connectivity: &Connectivity, settings: &ErcSettings) -> ErcReport {
    let severities: HashMap<&str, ErcSeverity> = CHECKS
        .iter()
        .map(|(code, _)| (*code, settings.severity(code)))
        .collect();
    let mut ctx = Ctx {
        connectivity,
        severities,
        issues: Vec::new(),
    };
    for problem in &connectivity.problems {
        let code = problem.code.as_str();
        let args = [("name", problem.name.clone())];
        let message = match code {
            "missing-symbol" => format!("symbol `{}` is not in a project library", problem.name),
            "missing-sheet" => format!(
                "sheet `{}` refers to a schematic that does not exist",
                problem.name
            ),
            _ => format!("sheet `{}` includes itself", problem.name),
        };
        ctx.report(
            code,
            problem.instance,
            Point::ORIGIN,
            vec![problem.id],
            &args,
            message,
        );
    }
    pin_checks(&mut ctx, settings);
    item_checks(&mut ctx, project);
    symbol_checks(&mut ctx, project);
    let mut issues = ctx.issues;
    let order: HashMap<Id, usize> = connectivity
        .instances
        .iter()
        .enumerate()
        .map(|(i, inst)| (inst.sheet, i))
        .rev()
        .collect();
    issues.sort_by(|a, b| {
        (
            a.severity,
            order.get(&a.sheet),
            &a.path,
            -a.at.y,
            a.at.x,
            &a.code,
        )
            .cmp(&(
                b.severity,
                order.get(&b.sheet),
                &b.path,
                -b.at.y,
                b.at.x,
                &b.code,
            ))
    });
    issues.dedup();
    ErcReport {
        errors: issues
            .iter()
            .filter(|i| i.severity == ErcSeverity::Error)
            .count(),
        warnings: issues
            .iter()
            .filter(|i| i.severity == ErcSeverity::Warning)
            .count(),
        nets: connectivity.nets.len(),
        pins: connectivity.pins.len(),
        issues,
    }
}

/// Checks per net: pin conflicts, undriven inputs and power, unconnected
/// pins.
#[allow(clippy::too_many_lines, reason = "one block per check")]
fn pin_checks(ctx: &mut Ctx<'_>, settings: &ErcSettings) {
    let table = settings.matrix();
    let connectivity = ctx.connectivity;
    // No-connect flags by sheet instance and point.
    let mut no_connect: BTreeSet<(usize, Point)> = BTreeSet::new();
    for piece in &connectivity.pieces {
        for item in &piece.items {
            if item.kind == ItemKind::NoConnect {
                no_connect.insert((item.instance, item.at));
            }
        }
    }
    for net in &connectivity.nets {
        let pins: Vec<&PinInfo> = net.pins.iter().map(|&p| &connectivity.pins[p]).collect();
        let real: Vec<&PinInfo> = pins.iter().copied().filter(|p| !p.power_symbol).collect();
        // Pin conflicts: each pair of pin types once per net.
        let mut seen: BTreeSet<(usize, usize)> = BTreeSet::new();
        for (i, a) in pins.iter().enumerate() {
            for b in &pins[i + 1..] {
                if a.symbol == b.symbol && a.number == b.number {
                    continue;
                }
                let (ka, kb) = (a.kind.index(), b.kind.index());
                let key = (ka.min(kb), ka.max(kb));
                if seen.contains(&key) {
                    continue;
                }
                let severity = table[ka][kb];
                if severity != ErcSeverity::Ignore {
                    seen.insert(key);
                    let code = "pin-conflict";
                    if ctx.severities.get(code) == Some(&ErcSeverity::Ignore) {
                        continue;
                    }
                    let message = format!(
                        "pins {} ({}) and {} ({}) on net {} should not be connected",
                        pin_label(a),
                        a.kind.name().to_owned(),
                        pin_label(b),
                        b.kind.name().to_owned(),
                        net.name
                    );
                    let args = [
                        ("pin", pin_label(a)),
                        ("kind", a.kind.name().to_owned()),
                        ("other", pin_label(b)),
                        ("otherKind", b.kind.name().to_owned()),
                        ("net", net.name.clone()),
                    ];
                    let before = ctx.issues.len();
                    ctx.report(
                        code,
                        a.instance,
                        a.at,
                        vec![a.symbol, b.symbol],
                        &args,
                        message,
                    );
                    if let Some(issue) = ctx.issues.get_mut(before) {
                        issue.severity = severity;
                    }
                }
            }
        }
        // Power inputs need a power output (or a power flag) on the net.
        let drives_power = |p: &&PinInfo| p.kind == PinKind::PowerOut;
        if let Some(first) = pins
            .iter()
            .find(|p| p.kind == PinKind::PowerIn && !p.hidden)
        {
            if !pins.iter().any(drives_power)
                && pins.iter().filter(|p| p.kind == PinKind::PowerIn).count() > 0
            {
                ctx.report(
                    "power-not-driven",
                    first.instance,
                    first.at,
                    vec![first.symbol],
                    &[("pin", pin_label(first)), ("net", net.name.clone())],
                    format!(
                        "the power input {} on net {} is not driven by any power output",
                        pin_label(first),
                        net.name
                    ),
                );
            }
        }
        // Inputs need a driver.
        let drives = |p: &&&PinInfo| {
            matches!(
                p.kind,
                PinKind::Output
                    | PinKind::Bidirectional
                    | PinKind::TriState
                    | PinKind::Passive
                    | PinKind::PowerOut
                    | PinKind::OpenCollector
                    | PinKind::OpenEmitter
                    | PinKind::Unspecified
            )
        };
        if real.len() > 1 && !real.iter().any(|p| drives(&p)) {
            if let Some(first) = real.iter().find(|p| p.kind == PinKind::Input) {
                if !net.named || !pins.iter().any(|p| p.power_symbol) {
                    ctx.report(
                        "input-not-driven",
                        first.instance,
                        first.at,
                        vec![first.symbol],
                        &[("pin", pin_label(first)), ("net", net.name.clone())],
                        format!(
                            "the input {} on net {} is not driven by any pin",
                            pin_label(first),
                            net.name
                        ),
                    );
                }
            }
        }
        // Unconnected pins: alone on their net, without a no-connect flag.
        let items: usize = net
            .pieces
            .iter()
            .map(|&p| {
                connectivity.pieces[p]
                    .items
                    .iter()
                    .filter(|i| i.kind != ItemKind::NoConnect)
                    .count()
            })
            .sum();
        if pins.len() == 1 && items == 0 {
            let pin = pins[0];
            let excused = pin.power_symbol
                || pin.kind == PinKind::NoConnect
                || (pin.hidden && pin.kind == PinKind::PowerIn)
                || no_connect.contains(&(pin.instance, pin.at));
            if !excused {
                ctx.report(
                    "unconnected-pin",
                    pin.instance,
                    pin.at,
                    vec![pin.symbol],
                    &[("pin", pin_label(pin))],
                    format!("pin {} is not connected", pin_label(pin)),
                );
            }
        }
        // No-connect flags on connected pins.
        if real.len() > 1 {
            for piece in &net.pieces {
                for item in &connectivity.pieces[*piece].items {
                    if item.kind == ItemKind::NoConnect {
                        ctx.report(
                            "no-connect-connected",
                            item.instance,
                            item.at,
                            vec![item.id],
                            &[("net", net.name.clone())],
                            format!(
                                "a no-connect flag sits on net {}, which connects pins",
                                net.name
                            ),
                        );
                    }
                }
            }
        }
        if net.names.len() > 1 {
            let names: Vec<String> = net.names.iter().cloned().collect();
            if let Some(piece) = net.pieces.first() {
                let piece = &connectivity.pieces[*piece];
                let at = piece.items.first().map_or(Point::ORIGIN, |i| i.at);
                ctx.report(
                    "net-name-conflict",
                    piece.instance,
                    at,
                    piece.items.iter().map(|i| i.id).collect(),
                    &[("names", names.join(", "))],
                    format!("one net has several names: {}", names.join(", ")),
                );
            }
        }
    }
}

/// Checks of wires, labels, flags and sheet pins.
#[allow(clippy::too_many_lines, reason = "one block per item kind")]
fn item_checks(ctx: &mut Ctx<'_>, project: &Project) {
    let connectivity = ctx.connectivity;
    let grid = project.manifest.settings.schematic_grid;
    let mut global_count: BTreeMap<String, usize> = BTreeMap::new();
    let mut global_first: BTreeMap<String, (usize, Point, Id)> = BTreeMap::new();
    for (instance, inst) in connectivity.instances.iter().enumerate() {
        let Some(Document::Schematic(sheet)) = project.document(inst.sheet) else {
            continue;
        };
        let pieces: Vec<&crate::connectivity::Piece> = connectivity
            .pieces
            .iter()
            .filter(|p| p.instance == instance)
            .collect();
        let piece_of = |id: Id, at: Point| {
            pieces
                .iter()
                .find(|p| p.items.iter().any(|i| i.id == id && i.at == at))
                .copied()
        };
        // Everything that can end a wire: wire ends, pins, labels, flags,
        // junctions and sheet pins.
        let mut touching: HashMap<Point, usize> = HashMap::new();
        for wire in &sheet.wires {
            *touching.entry(wire.start).or_default() += 1;
            *touching.entry(wire.end).or_default() += 1;
        }
        for piece in &pieces {
            for item in piece.items.iter().filter(|i| i.kind != ItemKind::Wire) {
                *touching.entry(item.at).or_default() += 1;
            }
            for &pin in &piece.pins {
                *touching.entry(connectivity.pins[pin].at).or_default() += 1;
            }
        }
        let origin = grid_origin(sheet);
        for wire in &sheet.wires {
            for end in [wire.start, wire.end] {
                let alone = touching.get(&end).copied().unwrap_or(0) <= 1;
                if alone
                    && !sheet
                        .wires
                        .iter()
                        .any(|o| o.id != wire.id && on_segment(end, o.start, o.end))
                {
                    ctx.report(
                        "dangling-wire",
                        instance,
                        end,
                        vec![wire.id],
                        &[],
                        "a wire end is not connected to anything".to_owned(),
                    );
                }
            }
            if let Some(end) = [wire.start, wire.end]
                .into_iter()
                .find(|p| !on_grid(*p, grid, origin))
            {
                ctx.report(
                    "off-grid",
                    instance,
                    end,
                    vec![wire.id],
                    &[],
                    "a wire end is not on the grid".to_owned(),
                );
            }
        }
        for label in &sheet.labels {
            let piece = piece_of(label.id, label.at);
            let connected = piece.is_some_and(|p| {
                !p.pins.is_empty()
                    || p.items
                        .iter()
                        .any(|i| i.kind == ItemKind::Wire || i.kind == ItemKind::SheetPin)
            });
            if !connected {
                ctx.report(
                    "dangling-label",
                    instance,
                    label.at,
                    vec![label.id],
                    &[("name", label.text.clone())],
                    format!("label `{}` is not connected to a wire or pin", label.text),
                );
            }
            match label.kind {
                LabelKind::Global => {
                    *global_count.entry(label.text.clone()).or_default() += 1;
                    global_first
                        .entry(label.text.clone())
                        .or_insert((instance, label.at, label.id));
                }
                LabelKind::Hierarchical => {
                    // The sheet symbol above must have a pin with this name.
                    let matched =
                        inst.parent
                            .zip(inst.path.last())
                            .is_none_or(|(parent, symbol)| {
                                sheet_symbol(project, connectivity, parent, *symbol)
                                    .is_some_and(|s| s.pins.iter().any(|p| p.name == label.text))
                            });
                    if !matched {
                        ctx.report(
                            "hierarchical-label-unmatched",
                            instance,
                            label.at,
                            vec![label.id],
                            &[("name", label.text.clone())],
                            format!(
                                "hierarchical label `{}` has no pin on the sheet symbol",
                                label.text
                            ),
                        );
                    }
                }
                LabelKind::Local => {}
            }
        }
        for flag in &sheet.no_connects {
            let piece = piece_of(flag.id, flag.at);
            if piece.is_none_or(|p| p.pins.is_empty()) {
                ctx.report(
                    "no-connect-dangling",
                    instance,
                    flag.at,
                    vec![flag.id],
                    &[],
                    "a no-connect flag is not on a pin".to_owned(),
                );
            }
        }
        // Sheet pins need a hierarchical label in the child.
        for symbol in &sheet.sheets {
            let child: Option<&ew_model::schematic::Schematic> =
                match project.document(symbol.schematic) {
                    Some(Document::Schematic(child)) => Some(child),
                    _ => None,
                };
            let Some(child) = child else { continue };
            for pin in &symbol.pins {
                let found = child
                    .labels
                    .iter()
                    .any(|l| l.kind == LabelKind::Hierarchical && l.text == pin.name);
                if !found {
                    ctx.report(
                        "sheet-pin-unmatched",
                        instance,
                        sheet_pin_point(symbol, pin),
                        vec![symbol.id],
                        &[("name", pin.name.clone()), ("sheet", symbol.name.clone())],
                        format!(
                            "sheet pin `{}` of `{}` has no hierarchical label in the sheet",
                            pin.name, symbol.name
                        ),
                    );
                }
            }
        }
    }
    for (name, count) in global_count {
        if count == 1 {
            if let Some((instance, at, id)) = global_first.get(&name) {
                ctx.report(
                    "single-global-label",
                    *instance,
                    *at,
                    vec![*id],
                    &[("name", name.clone())],
                    format!("global label `{name}` appears only once"),
                );
            }
        }
    }
}

fn sheet_symbol<'a>(
    project: &'a Project,
    connectivity: &Connectivity,
    instance: usize,
    id: Id,
) -> Option<&'a ew_model::schematic::SheetSymbol> {
    match project.document(connectivity.instances.get(instance)?.sheet) {
        Some(Document::Schematic(sheet)) => sheet.sheets.get(id),
        _ => None,
    }
}

/// A placed symbol with a designator.
struct Placement {
    symbol: String,
    unit: u16,
    instance: usize,
    at: Point,
    id: Id,
}

/// Designator checks: unannotated and duplicate designators.
fn symbol_checks(ctx: &mut Ctx<'_>, project: &Project) {
    let connectivity = ctx.connectivity;
    let mut seen: BTreeMap<String, Vec<Placement>> = BTreeMap::new();
    for (instance, inst) in connectivity.instances.iter().enumerate() {
        let Some(Document::Schematic(sheet)) = project.document(inst.sheet) else {
            continue;
        };
        for symbol in &sheet.symbols {
            let designator = symbol.designator_in(&inst.key);
            if designator.starts_with('#') {
                continue;
            }
            if designator.is_empty() || designator.ends_with('?') {
                ctx.report(
                    "unannotated",
                    instance,
                    symbol.at,
                    vec![symbol.id],
                    &[("designator", designator.to_owned())],
                    format!(
                        "{} is not annotated",
                        if designator.is_empty() {
                            "a symbol"
                        } else {
                            designator
                        }
                    ),
                );
                continue;
            }
            seen.entry(designator.to_owned())
                .or_default()
                .push(Placement {
                    symbol: symbol.symbol.clone(),
                    unit: symbol.unit,
                    instance,
                    at: symbol.at,
                    id: symbol.id,
                });
        }
    }
    let mut designators: Vec<(&String, &Vec<Placement>)> = seen.iter().collect();
    designators.sort_by(|a, b| natural_cmp(a.0, b.0));
    for (designator, uses) in designators {
        // Units of one part may share a designator; anything else may not.
        let mut units: BTreeSet<u16> = BTreeSet::new();
        let symbols: BTreeSet<&String> = uses.iter().map(|u| &u.symbol).collect();
        let clash = symbols.len() > 1 || uses.iter().any(|u| !units.insert(u.unit));
        if clash {
            for placement in uses.iter().skip(1) {
                ctx.report(
                    "duplicate-designator",
                    placement.instance,
                    placement.at,
                    vec![placement.id],
                    &[("designator", designator.clone())],
                    format!("designator {designator} is used more than once"),
                );
            }
        }
    }
}
