//! Impedance profiles: the router takes the width and gap of a net class's
//! profile on the layer it routes, and the DRC warns about tracks whose
//! width misses the target impedance.

#![allow(clippy::unwrap_used, reason = "test helpers")]

use ew_model::id::Id;
use ew_model::layer::CopperLayer;
use ew_model::pcb::{Net, Pcb, STANDARD_THICKNESS, Stackup, Track};
use ew_model::rules::{ImpedanceProfile, NetClass, NetClassAssignment, Rules};
use ew_model::units::{Point, um};
use ew_pcb::drc::{Options, check};
use ew_pcb::impedance;
use ew_pcb::route::Router;
use ew_pcb::rules::RuleSet;

fn rules() -> Rules {
    let mut rules = Rules::new(Id::random(), "Rules");
    rules.impedance_profiles.push(ImpedanceProfile {
        name: "USB 2.0".into(),
        differential: true,
        target: 90,
        tolerance: 10,
        preset: "usb2".into(),
        layers: std::collections::BTreeMap::new(),
    });
    rules.net_classes.push(NetClass {
        name: "USB".into(),
        impedance: "USB 2.0".into(),
        ..NetClass::default_class()
    });
    rules.assignments.push(NetClassAssignment {
        pattern: "USB_D?".into(),
        class: "USB".into(),
    });
    rules
}

fn board() -> (Pcb, Id) {
    let mut pcb = Pcb::new(Id::random(), "B", Stackup::standard(4, STANDARD_THICKNESS));
    let net = Id::random();
    pcb.nets.insert(Net {
        id: net,
        name: "USB_DP".into(),
        class: String::new(),
    });
    (pcb, net)
}

fn track(net: Id, y: i64, width: i64) -> Track {
    Track {
        id: Id::random(),
        layer: CopperLayer::Top,
        net: Some(net),
        width,
        start: Point::mm(0, y),
        end: Point::mm(10, y),
        mid: None,
        locked: false,
    }
}

#[test]
fn the_router_and_the_drc_follow_the_profile() {
    let rules = rules();
    let (mut pcb, net) = board();
    let profile = &rules.impedance_profiles[0];
    let layout = impedance::layout(
        profile,
        &pcb.stackup,
        CopperLayer::Top,
        f64::from(200_000),
        f64::from(200_000),
    )
    .unwrap();
    assert!(impedance::within(profile, layout.impedance));
    assert!(!layout.manual && layout.fits);

    // A track of the right width, where the routes start.
    #[allow(clippy::cast_possible_truncation, reason = "a track width")]
    let right = layout.width as i64;
    pcb.tracks.insert(track(net, 0, right));

    // The router routes USB_DP with the profile's sizes on the top.
    let router = Router::new(&pcb, Some(&rules));
    let start = router.start(ew_pcb::geom::V::new(0.0, 0.0), CopperLayer::Top);
    assert_eq!(start.profile.as_deref(), Some("USB 2.0"));
    assert!((start.pair_width - layout.width).abs() < 1.0);
    assert!((start.pair_gap - layout.gap).abs() < 1.0);
    // Every copper layer has its own width; the route changes to it with
    // the layer.
    assert_eq!(start.layer_widths.len(), 4);
    assert!(start.layer_widths["cu.in1"] < start.layer_widths["cu.top"]);
    drop(router);
    // An inner layer (stripline) needs narrower tracks for the same target.
    let inner = impedance::layout(
        profile,
        &pcb.stackup,
        CopperLayer::Inner(1),
        200_000.0,
        200_000.0,
    )
    .unwrap();
    assert!(inner.width < layout.width);

    // The track of the right width passes; a much wider one is reported.
    pcb.tracks.insert(track(net, 5, um(800)));
    let report = check(&pcb, Some(&rules), &Options::default());
    let found: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.code == "impedance")
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].args["target"], "90");
    assert_eq!(found[0].args["profile"], "USB 2.0");

    // Set by hand on the top layer: the hand-made sizes win.
    let mut by_hand = rules.clone();
    by_hand.impedance_profiles[0].layers.insert(
        "cu.top".into(),
        ew_model::rules::TrackGeometry {
            width: um(300),
            gap: um(200),
        },
    );
    let set = RuleSet::new(Some(&by_hand));
    let top = impedance::layout(
        set.profile("USB").unwrap(),
        &pcb.stackup,
        CopperLayer::Top,
        200_000.0,
        200_000.0,
    )
    .unwrap();
    assert!(top.manual);
    assert!((top.width - 300_000.0).abs() < 1.0);
}
