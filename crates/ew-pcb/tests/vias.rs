//! Blind, buried and microvias: their type, the design rules that allow
//! them, the DRC of their layers and sizes, the router that checks only
//! their layers, and the DSN file and session that carry them.

#![allow(clippy::unwrap_used, reason = "test helpers")]

use ew_model::id::Id;
use ew_model::layer::CopperLayer;
use ew_model::pcb::{Net, Pcb, STANDARD_THICKNESS, Stackup, Track, Via, ViaType};
use ew_model::rules::Rules;
use ew_model::units::{Nm, Point, um};
use ew_pcb::drc::{Options, check};
use ew_pcb::route::Router;
use ew_pcb::rules::RuleSet;
use ew_pcb::specctra::{DsnInput, plan_session, read_ses, write_dsn};

const IN1: CopperLayer = CopperLayer::Inner(1);
const IN2: CopperLayer = CopperLayer::Inner(2);

fn via(at: Point, from: CopperLayer, to: CopperLayer, micro: bool, size: (Nm, Nm)) -> Via {
    Via {
        id: Id::random(),
        at,
        diameter: size.0,
        drill: size.1,
        net: None,
        from,
        to,
        micro,
        fill: None,
        locked: false,
    }
}

fn board(vias: Vec<Via>) -> Pcb {
    let mut pcb = Pcb::new(Id::random(), "B", Stackup::standard(4, STANDARD_THICKNESS));
    for via in vias {
        pcb.vias.insert(via);
    }
    pcb
}

/// The codes of the via findings, with the message of each.
fn via_findings(pcb: &Pcb, rules: &Rules) -> Vec<(String, String)> {
    check(pcb, Some(rules), &Options::default())
        .findings
        .into_iter()
        .filter(|f| f.code.starts_with("via-"))
        .map(|f| (f.code, f.message))
        .collect()
}

fn rules(blind: bool, micro: bool) -> Rules {
    let mut rules = Rules::new(Id::random(), "Rules");
    rules.constraints.allow_blind_buried_vias = blind;
    rules.constraints.allow_micro_vias = micro;
    rules
}

const NORMAL: (Nm, Nm) = (um(600), um(300));
const SMALL: (Nm, Nm) = (um(250), um(100));

#[test]
fn via_types_follow_the_layers() {
    let at = Point::mm(1, 1);
    let kind = |from, to, micro| via(at, from, to, micro, NORMAL).via_type();
    assert_eq!(
        kind(CopperLayer::Top, CopperLayer::Bottom, false),
        ViaType::Through
    );
    assert_eq!(
        kind(CopperLayer::Bottom, CopperLayer::Top, false),
        ViaType::Through
    );
    assert_eq!(kind(CopperLayer::Top, IN1, false), ViaType::Blind);
    assert_eq!(kind(IN2, CopperLayer::Bottom, false), ViaType::Blind);
    assert_eq!(kind(IN1, IN2, false), ViaType::Buried);
    assert_eq!(kind(CopperLayer::Top, IN1, true), ViaType::Micro);
}

#[test]
fn the_rules_allow_via_types() {
    let pcb = board(vec![
        via(Point::mm(10, 10), CopperLayer::Top, IN1, false, NORMAL),
        via(Point::mm(20, 10), IN1, IN2, false, NORMAL),
        via(Point::mm(30, 10), CopperLayer::Top, IN1, true, SMALL),
    ]);
    let found = via_findings(&pcb, &rules(false, false));
    let types: Vec<&String> = found
        .iter()
        .filter(|(code, _)| code == "via-type")
        .map(|(_, message)| message)
        .collect();
    assert_eq!(types.len(), 3, "{found:?}");
    for kind in [
        "Blind via on layers 1–2",
        "Buried via on layers 2–3",
        "Microvia on layers 1–2",
    ] {
        assert!(
            types.iter().any(|m| m.starts_with(kind)),
            "{kind}: {found:?}"
        );
    }
    // A microvia is measured against the microvia minimums.
    assert!(
        found.iter().all(|(code, _)| code != "via-drill"),
        "{found:?}"
    );

    let found = via_findings(&pcb, &rules(true, true));
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn small_vias_need_to_be_microvias() {
    // 0.25 mm with a 0.1 mm drill keeps the microvia minimums only.
    let pcb = board(vec![via(
        Point::mm(10, 10),
        CopperLayer::Top,
        IN1,
        false,
        SMALL,
    )]);
    let found = via_findings(&pcb, &rules(true, true));
    assert!(
        found.iter().any(|(code, _)| code == "via-drill"),
        "{found:?}"
    );
    assert!(
        found.iter().any(|(code, _)| code == "via-diameter"),
        "{found:?}"
    );
}

#[test]
fn via_layers_must_exist_and_microvias_join_neighbours() {
    let pcb = board(vec![
        // A microvia across three layers.
        via(Point::mm(10, 10), CopperLayer::Top, IN2, true, SMALL),
        // A layer the 4-layer board does not have.
        via(
            Point::mm(20, 10),
            CopperLayer::Top,
            CopperLayer::Inner(5),
            false,
            NORMAL,
        ),
        // From a layer to itself.
        via(Point::mm(30, 10), IN1, IN1, false, NORMAL),
        // Fine.
        via(Point::mm(40, 10), IN2, CopperLayer::Bottom, true, SMALL),
    ]);
    let found = via_findings(&pcb, &rules(true, true));
    let layers: Vec<&String> = found
        .iter()
        .filter(|(code, _)| code == "via-layers")
        .map(|(_, message)| message)
        .collect();
    assert_eq!(layers.len(), 3, "{found:?}");
    assert!(
        layers
            .iter()
            .any(|m| m.contains("1–3") && m.contains("neighbours"))
    );
    assert!(layers.iter().any(|m| m.contains("does not have")));
    assert!(layers.iter().any(|m| m.contains("itself")));
}

#[test]
fn a_blind_via_fits_over_copper_of_other_layers() {
    let net = Id::random();
    let mut pcb = board(Vec::new());
    pcb.nets.insert(Net {
        id: net,
        name: "SIG".into(),
        class: String::new(),
    });
    // A track of no net on the bottom layer, under the via.
    pcb.tracks.insert(Track {
        id: Id::random(),
        layer: CopperLayer::Bottom,
        net: None,
        width: um(500),
        start: Point::mm(0, 0),
        end: Point::mm(10, 0),
        mid: None,
        locked: false,
    });
    pcb.shapes.insert(ew_model::pcb::BoardShape {
        id: Id::random(),
        layer: ew_model::layer::Layer::Outline,
        geometry: ew_model::shape::Geometry::Rect {
            start: Point::mm(-20, -20),
            end: Point::mm(20, 20),
        },
        width: um(100),
        filled: false,
    });
    let rules = rules(true, true);
    let router = Router::new(&pcb, Some(&rules));
    let at = ew_pcb::geom::V::new(5e6, 0.0);
    assert!(!router.via_fits(Some(net), at, 600_000.0, None));
    assert!(router.via_fits(Some(net), at, 600_000.0, Some((CopperLayer::Top, IN1))));
    assert!(!router.via_fits(Some(net), at, 600_000.0, Some((IN2, CopperLayer::Bottom))));
}

#[test]
fn the_dsn_file_offers_the_allowed_vias_and_a_session_brings_them_back() {
    let pcb = board(Vec::new());
    let strict = RuleSet::new(Some(&rules(false, false)));
    let text = write_dsn(&DsnInput {
        pcb: &pcb,
        rules: &strict,
        host: "test",
    });
    assert!(!text.contains("Via[0-1]"));
    assert!(!text.contains("uVia"));

    let all = RuleSet::new(Some(&rules(true, true)));
    let text = write_dsn(&DsnInput {
        pcb: &pcb,
        rules: &all,
        host: "test",
    });
    for name in [
        "Via[0-3]_600:300_um",
        "Via[0-1]_600:300_um",
        "Via[1-2]_600:300_um",
    ] {
        assert!(text.contains(name), "{name} missing");
    }
    for name in ["uVia[0-1]_300:100_um", "uVia[2-3]_300:100_um"] {
        assert!(text.contains(name), "{name} missing");
    }
    assert!(!text.contains("uVia[0-2]"));

    let mut pcb = pcb;
    let net = Id::random();
    pcb.nets.insert(Net {
        id: net,
        name: "SIG".into(),
        class: String::new(),
    });
    let session = read_ses(
        "(session s (routes (resolution um 10) (network_out (net SIG \
         (via \"uVia[0-1]_300:100_um\" 0 0) (via \"Via[1-2]_600:300_um\" 1000 0)))))",
    )
    .unwrap();
    let plan = plan_session(&pcb, &all, &session);
    assert_eq!(plan.vias.len(), 2);
    let micro = &plan.vias[0];
    assert!(micro.micro);
    assert_eq!((micro.from, micro.to), (CopperLayer::Top, IN1));
    assert_eq!((micro.diameter, micro.drill), (um(300), um(100)));
    assert_eq!(plan.vias[1].via_type(), ViaType::Buried);
}
