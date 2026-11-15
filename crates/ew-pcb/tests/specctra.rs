//! Specctra: the DSN file of a routed board describes every part, pad, net
//! and track; a session made from the board's own routing, read back,
//! gives the same tracks and vias.

#![allow(
    clippy::unwrap_used,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "test helpers"
)]

use std::collections::BTreeSet;
use std::fmt::Write as _;

use ew_model::document::Document;
use ew_model::pcb::Pcb;
use ew_model::rules::Rules;
use ew_pcb::rules::RuleSet;
use ew_pcb::specctra::{DsnInput, layer_name, plan_session, read_ses, write_dsn};
use ew_synth::{DesignSize, generate};

fn board() -> Pcb {
    generate(DesignSize::S, ew_synth::DEFAULT_SEED)
        .documents
        .into_values()
        .find_map(|doc| match doc {
            Document::Pcb(pcb) => Some(pcb),
            _ => None,
        })
        .unwrap()
}

fn rules() -> RuleSet {
    RuleSet::new(Some(&Rules::new(ew_model::id::Id::random(), "Rules")))
}

/// Writes a session (as Freerouting does) from the routing of a board:
/// resolution 0.1 µm, y up like the board.
fn session_of(pcb: &Pcb) -> String {
    let v = |nm: i64| (nm as f64 / 100.0).round() as i64;
    let mut out = String::from("(session \"x.ses\" (routes (resolution um 10) (network_out\n");
    for net in &pcb.nets {
        let _ = writeln!(out, "(net \"{}\"", net.name);
        for track in pcb.tracks.iter().filter(|t| t.net == Some(net.id)) {
            let _ = writeln!(
                out,
                "(wire (path {} {} {} {} {} {}))",
                layer_name(track.layer),
                v(track.width),
                v(track.start.x),
                v(track.start.y),
                v(track.end.x),
                v(track.end.y)
            );
        }
        for via in pcb.vias.iter().filter(|via| via.net == Some(net.id)) {
            let _ = writeln!(
                out,
                "(via \"Via[0-{}]_{}:{}_um\" {} {})",
                pcb.stackup.copper_count() - 1,
                via.diameter / 1000,
                via.drill / 1000,
                v(via.at.x),
                v(via.at.y)
            );
        }
        out.push_str(")\n");
    }
    out.push_str(")))\n");
    out
}

#[test]
fn the_dsn_file_describes_the_board() {
    let pcb = board();
    let rules = rules();
    let text = write_dsn(&DsnInput {
        pcb: &pcb,
        rules: &rules,
        host: "ElectronicWerk test",
    });
    assert!(text.starts_with("(pcb "));
    assert!(text.contains("(resolution um 10)"));
    assert!(text.contains("(layer F.Cu (type signal)"));
    assert!(text.contains("(boundary (path pcb 0 "));
    // Every part is placed and has an image.
    let placed = text.matches("(place ").count();
    assert_eq!(placed, pcb.footprints.len());
    assert_eq!(text.matches("    (image ").count(), pcb.footprints.len());
    // Every net is listed, with the pins of its pads.
    assert_eq!(text.matches("    (net ").count(), pcb.nets.len());
    let pads_with_net: usize = pcb
        .footprints
        .iter()
        .flat_map(|fp| fp.pads.iter())
        .filter(|pad| pad.net.is_some())
        .count();
    let pins: usize = text
        .lines()
        .filter(|line| line.trim_start().starts_with("(net "))
        .map(|line| {
            let pins = line.split("(pins").nth(1).unwrap_or_default();
            pins.split_whitespace().filter(|w| *w != "))").count()
        })
        .sum();
    assert_eq!(pins, pads_with_net);
    // Every padstack a pin uses is defined.
    let defined: BTreeSet<&str> = text
        .lines()
        .filter_map(|l| l.trim_start().strip_prefix("(padstack "))
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    for line in text.lines().filter(|l| l.trim_start().starts_with("(pin ")) {
        let padstack = line.split_whitespace().nth(1).unwrap();
        assert!(defined.contains(padstack), "{padstack} is not defined");
    }
    // The routing that exists already, and a class with its via.
    assert_eq!(text.matches("(wire (path ").count(), pcb.tracks.len());
    assert!(text.contains("(class Default "));
    assert!(text.contains("(use_via Via[0-1]_600:300_um)"));
}

#[test]
fn a_session_gives_back_the_routing() {
    let routed = board();
    assert!(!routed.tracks.is_empty() && !routed.vias.is_empty());
    let session = read_ses(&session_of(&routed)).unwrap();
    let rules = rules();
    let plan = plan_session(&routed, &rules, &session);
    assert!(plan.unknown_nets.is_empty() && plan.unknown_layers.is_empty());
    // The board's own unlocked routing is replaced …
    assert_eq!(plan.remove_tracks.len(), routed.tracks.len());
    assert_eq!(plan.remove_vias.len(), routed.vias.len());
    // … by the same tracks and vias (on the 0.1 µm grid of the session).
    let grid = |p: ew_model::units::Point| {
        (
            (p.x as f64 / 100.0).round() as i64,
            (p.y as f64 / 100.0).round() as i64,
        )
    };
    let key = |t: &ew_model::pcb::Track| (t.layer, t.net, t.width, grid(t.start), grid(t.end));
    let before: BTreeSet<_> = routed.tracks.iter().map(key).collect();
    let after: BTreeSet<_> = plan.tracks.iter().map(key).collect();
    assert_eq!(before, after);
    let via = |v: &ew_model::pcb::Via| (grid(v.at), v.net, v.diameter, v.drill, v.from, v.to);
    let before: BTreeSet<_> = routed.vias.iter().map(via).collect();
    let after: BTreeSet<_> = plan.vias.iter().map(via).collect();
    assert_eq!(before, after);
}

#[test]
fn locked_routing_and_unknown_nets() {
    let mut pcb = board();
    let locked = pcb.tracks.iter().next().unwrap().id;
    pcb.tracks.get_mut(locked).unwrap().locked = true;
    let rules = rules();
    let text = write_dsn(&DsnInput {
        pcb: &pcb,
        rules: &rules,
        host: "test",
    });
    assert_eq!(text.matches("(type protect)").count(), 1);
    let net = pcb.nets.iter().next().unwrap().name.clone();
    let session = read_ses(&format!(
        "(session s (routes (resolution mil 1000) (network_out \
         (net NOT_THERE (wire (path F.Cu 10 0 0 1000 0))) \
         (net \"{net}\" (wire (path X.Cu 10 0 0 1000 0))))))"
    ))
    .unwrap();
    let plan = plan_session(&pcb, &rules, &session);
    assert_eq!(plan.unknown_nets, ["NOT_THERE"]);
    assert_eq!(plan.unknown_layers, ["X.Cu"]);
    // The locked track stays.
    assert!(!plan.remove_tracks.contains(&locked));
}
