//! Netlists: KiCad (`.net`, S-expression), XML and CSV, and the comparison
//! of a netlist with the nets of a board.

use std::collections::{BTreeMap, BTreeSet};

use ew_model::pcb::Pcb;
use serde::Serialize;

use crate::sexpr;

/// A connection: component designator and pin number.
pub type Node = (String, String);

/// A netlist.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Netlist {
    /// Components (designator → value), where the format has them.
    pub components: BTreeMap<String, String>,
    /// Nets by name with their nodes.
    pub nets: BTreeMap<String, BTreeSet<Node>>,
}

impl Netlist {
    fn add(&mut self, net: &str, reference: &str, pin: &str) {
        if net.is_empty() || reference.is_empty() {
            return;
        }
        self.nets
            .entry(net.to_owned())
            .or_default()
            .insert((reference.to_owned(), pin.to_owned()));
    }

    /// Number of nodes in every net.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nets.values().map(BTreeSet::len).sum()
    }
}

/// The format of a netlist file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    /// KiCad S-expression netlist (`.net`).
    Kicad,
    /// XML (KiCad XML and similar).
    Xml,
    /// Comma, semicolon or tab separated values.
    Csv,
}

/// Why a netlist could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct NetlistError(pub String);

/// Reads a netlist; the format is detected from the content.
///
/// # Errors
///
/// Fails if the text is not a supported netlist or contains no nets.
pub fn parse(text: &str) -> Result<(Netlist, Format), NetlistError> {
    let trimmed = text.trim_start_matches('\u{feff}').trim_start();
    let (netlist, format) = if trimmed.starts_with('(') {
        (kicad(trimmed)?, Format::Kicad)
    } else if trimmed.starts_with('<') {
        (xml(trimmed), Format::Xml)
    } else {
        (csv(trimmed)?, Format::Csv)
    };
    if netlist.nets.is_empty() {
        return Err(NetlistError("the file contains no nets".to_owned()));
    }
    Ok((netlist, format))
}

fn kicad(text: &str) -> Result<Netlist, NetlistError> {
    let root = sexpr::parse(text).map_err(|error| NetlistError(error.to_string()))?;
    if !root.is("export") {
        return Err(NetlistError(
            "not a KiCad netlist (expected `(export …)`)".to_owned(),
        ));
    }
    let mut out = Netlist::default();
    if let Some(components) = root.get("components") {
        for comp in components.all("comp") {
            if let Some(reference) = comp.value("ref") {
                out.components.insert(
                    reference.to_owned(),
                    comp.value("value").unwrap_or_default().to_owned(),
                );
            }
        }
    }
    if let Some(nets) = root.get("nets") {
        for net in nets.all("net") {
            let name = net.value("name").unwrap_or_default();
            for node in net.all("node") {
                out.add(
                    name,
                    node.value("ref").unwrap_or_default(),
                    node.value("pin").unwrap_or_default(),
                );
            }
        }
    }
    Ok(out)
}

/// Attributes of an XML start tag.
fn attributes(tag: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut rest = tag;
    while let Some(eq) = rest.find('=') {
        let key = rest[..eq]
            .split_whitespace()
            .last()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let after = rest[eq + 1..].trim_start();
        let Some(quote) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
            break;
        };
        let Some(end) = after[1..].find(quote) else {
            break;
        };
        let value = after[1..=end]
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&");
        out.insert(key, value);
        rest = &after[end + 2..];
    }
    out
}

fn pick<'a>(attrs: &'a BTreeMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| attrs.get(*key).map(String::as_str))
}

/// A small XML reader for netlists: `<net name="…">` elements with
/// `<node ref="…" pin="…"/>` children (KiCad XML and most other tools).
fn xml(text: &str) -> Netlist {
    let mut out = Netlist::default();
    let mut current: Option<String> = None;
    let mut component: Option<String> = None;
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        let Some(close) = rest[open..].find('>') else {
            break;
        };
        let tag = &rest[open + 1..open + close];
        rest = &rest[open + close + 1..];
        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        let closing = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let attrs = attributes(tag);
        match (name.as_str(), closing) {
            ("net" | "signal", false) => {
                current = pick(&attrs, &["name", "netname", "net"]).map(str::to_owned);
            }
            ("net" | "signal", true) => current = None,
            ("node" | "pin" | "connection" | "terminal" | "pinref", false) => {
                if let Some(net) = &current {
                    let reference = pick(
                        &attrs,
                        &["ref", "refdes", "designator", "component", "part"],
                    );
                    let pin = pick(&attrs, &["pin", "pad", "number", "pinnumber"]);
                    if let (Some(reference), Some(pin)) = (reference, pin) {
                        let net = net.clone();
                        out.add(&net, reference, pin);
                    }
                }
            }
            ("comp" | "component", false) => {
                component =
                    pick(&attrs, &["ref", "refdes", "designator", "name"]).map(str::to_owned);
                if let (Some(reference), Some(value)) = (&component, pick(&attrs, &["value"])) {
                    out.components.insert(reference.clone(), value.to_owned());
                }
            }
            ("comp" | "component", true) => component = None,
            _ => {}
        }
    }
    let _ = component;
    out
}

fn csv(text: &str) -> Result<Netlist, NetlistError> {
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let header = lines
        .next()
        .ok_or_else(|| NetlistError("the file is empty".to_owned()))?;
    let separator = [';', '\t', ',']
        .into_iter()
        .max_by_key(|sep| header.matches(*sep).count())
        .unwrap_or(',');
    let split = |line: &str| -> Vec<String> {
        line.split(separator)
            .map(|cell| cell.trim().trim_matches('"').trim().to_owned())
            .collect()
    };
    let columns: Vec<String> = split(header)
        .into_iter()
        .map(|c| c.to_ascii_lowercase())
        .collect();
    let find = |names: &[&str]| -> Option<usize> {
        columns
            .iter()
            .position(|c| names.iter().any(|n| c == n || c.replace(' ', "") == *n))
    };
    let net = find(&["net", "netname", "signal", "net name"]);
    let reference = find(&[
        "ref",
        "reference",
        "designator",
        "refdes",
        "component",
        "part",
    ]);
    let pin = find(&["pin", "pad", "pinnumber", "pin number", "terminal", "node"]);
    let (Some(net), Some(reference), Some(pin)) = (net, reference, pin) else {
        return Err(NetlistError(
            "the CSV header needs columns for the net, the designator and the pin".to_owned(),
        ));
    };
    let mut out = Netlist::default();
    for line in lines {
        let cells = split(line);
        let cell = |i: usize| cells.get(i).map_or("", String::as_str);
        out.add(cell(net), cell(reference), cell(pin));
    }
    Ok(out)
}

/// The nets of a board as a netlist (pads without a net are left out).
#[must_use]
pub fn of_board(pcb: &Pcb) -> Netlist {
    let names: BTreeMap<_, _> = pcb
        .nets
        .iter()
        .map(|net| (net.id, net.name.as_str()))
        .collect();
    let mut out = Netlist::default();
    for fp in &pcb.footprints {
        out.components
            .insert(fp.designator.clone(), fp.value.clone());
        for pad in &fp.pads {
            if let Some(name) = pad.net.and_then(|net| names.get(&net)) {
                out.add(name, &fp.designator, &pad.number);
            }
        }
    }
    out
}

/// A node whose net differs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDifference {
    /// Designator.
    pub reference: String,
    /// Pin.
    pub pin: String,
    /// Net in the netlist (`None`: the node is not in the netlist).
    pub expected: Option<String>,
    /// Net on the board (`None`: no net or no such pad).
    pub actual: Option<String>,
}

/// The result of comparing a netlist with a board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    /// Nets in the netlist.
    pub nets: usize,
    /// Nodes in the netlist.
    pub nodes: usize,
    /// Nets whose nodes are connected the same way on the board (names may
    /// differ).
    pub matching_nets: usize,
    /// Nets with the same nodes and the same name.
    pub same_names: usize,
    /// Components of the netlist without a footprint on the board.
    pub missing_components: Vec<String>,
    /// Nodes whose connection differs.
    pub differences: Vec<NodeDifference>,
}

impl Comparison {
    /// `true` if the board connects exactly like the netlist.
    #[must_use]
    pub fn is_equivalent(&self) -> bool {
        self.matching_nets == self.nets
            && self.differences.is_empty()
            && self.missing_components.is_empty()
    }
}

/// Compares the connectivity of `expected` with `actual`: two nets match if
/// they connect exactly the same nodes. Single-node nets only count as
/// "unconnected" (tools name them differently): such a node is a difference
/// if the board connects it to other nodes.
#[must_use]
pub fn compare(expected: &Netlist, actual: &Netlist) -> Comparison {
    let multi = |list: &Netlist| -> BTreeMap<String, BTreeSet<Node>> {
        list.nets
            .iter()
            .filter(|(_, nodes)| nodes.len() > 1)
            .map(|(name, nodes)| (name.clone(), nodes.clone()))
            .collect()
    };
    let expected_nets = multi(expected);
    let actual_nets = multi(actual);
    let actual_of: BTreeMap<&Node, &String> = actual
        .nets
        .iter()
        .flat_map(|(name, nodes)| nodes.iter().map(move |node| (node, name)))
        .collect();
    let sets_on_board: BTreeMap<&BTreeSet<Node>, &String> = actual_nets
        .iter()
        .map(|(name, nodes)| (nodes, name))
        .collect();
    let mut comparison = Comparison {
        nets: expected_nets.len(),
        nodes: expected_nets.values().map(BTreeSet::len).sum(),
        matching_nets: 0,
        same_names: 0,
        missing_components: Vec::new(),
        differences: Vec::new(),
    };
    let components: BTreeSet<&str> = expected
        .components
        .keys()
        .map(String::as_str)
        .chain(
            expected
                .nets
                .values()
                .flatten()
                .map(|(reference, _)| reference.as_str()),
        )
        .collect();
    let on_board: BTreeSet<&str> = actual
        .components
        .keys()
        .map(String::as_str)
        .chain(
            actual
                .nets
                .values()
                .flatten()
                .map(|(reference, _)| reference.as_str()),
        )
        .collect();
    comparison.missing_components = components
        .difference(&on_board)
        .map(|s| (*s).to_owned())
        .collect();
    for (name, nodes) in &expected_nets {
        match sets_on_board.get(nodes) {
            Some(actual_name) => {
                comparison.matching_nets += 1;
                if *actual_name == name {
                    comparison.same_names += 1;
                }
            }
            None => {
                for node in nodes {
                    let actual_net = actual_of.get(node).map(|n| (*n).clone());
                    let actual_nodes = actual_net.as_ref().and_then(|n| actual.nets.get(n));
                    if actual_nodes != Some(nodes) {
                        comparison.differences.push(NodeDifference {
                            reference: node.0.clone(),
                            pin: node.1.clone(),
                            expected: Some(name.clone()),
                            actual: actual_net,
                        });
                    }
                }
            }
        }
    }
    // A node the netlist leaves unconnected must not be connected on the board.
    for (name, nodes) in expected.nets.iter().filter(|(_, nodes)| nodes.len() == 1) {
        for node in nodes {
            let actual_net = actual_of.get(node).map(|n| (*n).clone());
            let connected = actual_net
                .as_ref()
                .and_then(|n| actual.nets.get(n))
                .is_some_and(|nodes| nodes.len() > 1);
            if connected {
                comparison.differences.push(NodeDifference {
                    reference: node.0.clone(),
                    pin: node.1.clone(),
                    expected: Some(name.clone()),
                    actual: actual_net,
                });
            }
        }
    }
    comparison
        .differences
        .sort_by(|a, b| (&a.reference, &a.pin).cmp(&(&b.reference, &b.pin)));
    comparison.differences.dedup();
    comparison
}

#[cfg(test)]
mod tests {
    use super::*;

    const KICAD: &str = r#"(export (version "E")
        (components (comp (ref "R1") (value "10k")) (comp (ref "C1") (value "100n")))
        (nets
            (net (code "1") (name "GND") (class "Default") (node (ref "R1") (pin "2")) (node (ref "C1") (pin "2")))
            (net (code "2") (name "/VIN") (node (ref "R1") (pin "1")) (node (ref "C1") (pin "1")))
            (net (code "3") (name "unconnected-(U1-NC)") (node (ref "U1") (pin "8")))))"#;

    #[test]
    fn reads_every_format_to_the_same_netlist() {
        let (kicad, format) = parse(KICAD).unwrap();
        assert_eq!(format, Format::Kicad);
        assert_eq!(kicad.nets.len(), 3);
        assert_eq!(kicad.components.get("R1").map(String::as_str), Some("10k"));
        let xml_text = r#"<?xml version="1.0"?><export><nets>
            <net code="1" name="GND"><node ref="R1" pin="2"/><node ref="C1" pin="2"/></net>
            <net code="2" name="/VIN"><node ref="R1" pin="1"/><node ref="C1" pin="1"/></net>
            <net code="3" name="unconnected-(U1-NC)"><node ref="U1" pin="8"/></net></nets></export>"#;
        let (xml, format) = parse(xml_text).unwrap();
        assert_eq!(format, Format::Xml);
        assert_eq!(xml.nets, kicad.nets);
        let csv_text = "Net;Designator;Pin\nGND;R1;2\nGND;C1;2\n/VIN;R1;1\n/VIN;C1;1\n\"unconnected-(U1-NC)\";U1;8\n";
        let (csv, format) = parse(csv_text).unwrap();
        assert_eq!(format, Format::Csv);
        assert_eq!(csv.nets, kicad.nets);
        assert!(parse("Name,Value\nx,y\n").is_err());
        assert!(parse("(export (nets))").is_err());
    }

    #[test]
    fn compares_connectivity_not_names() {
        let (expected, _) = parse(KICAD).unwrap();
        let mut actual = Netlist::default();
        actual.add("GND", "R1", "2");
        actual.add("GND", "C1", "2");
        actual.add("Net-1", "R1", "1");
        actual.add("Net-1", "C1", "1");
        actual.components.insert("U1".into(), String::new());
        let comparison = compare(&expected, &actual);
        assert!(comparison.is_equivalent(), "{comparison:?}");
        assert_eq!((comparison.nets, comparison.same_names), (2, 1));
        // A pad on the wrong net is reported.
        let mut wrong = actual.clone();
        wrong
            .nets
            .get_mut("Net-1")
            .unwrap()
            .remove(&("C1".into(), "1".into()));
        wrong.add("GND", "C1", "1");
        let comparison = compare(&expected, &wrong);
        assert!(!comparison.is_equivalent());
        assert!(
            comparison
                .differences
                .iter()
                .any(|d| d.reference == "C1" && d.pin == "1" && d.actual.as_deref() == Some("GND"))
        );
        // A pin that the netlist leaves alone but the board connects.
        let mut unconnected = expected.clone();
        unconnected
            .nets
            .get_mut("/VIN")
            .unwrap()
            .remove(&("C1".into(), "1".into()));
        unconnected.add("NC-C1", "C1", "1");
        let comparison = compare(&unconnected, &actual);
        assert!(
            comparison
                .differences
                .iter()
                .any(|d| d.reference == "C1" && d.pin == "1")
        );
        // Missing components.
        let empty = Netlist::default();
        assert_eq!(
            compare(&expected, &empty).missing_components,
            vec!["C1", "R1", "U1"]
        );
    }
}
