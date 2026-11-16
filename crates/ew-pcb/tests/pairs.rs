//! Differential pairs: found by their names, and the DRC checks the gap
//! between their coupled tracks against the impedance profile.

#![allow(clippy::unwrap_used, reason = "test helpers")]

use ew_model::id::Id;
use ew_model::layer::CopperLayer;
use ew_model::pcb::{Net, Pcb, STANDARD_THICKNESS, Stackup, Track};
use ew_model::rules::{ImpedanceProfile, NetClass, NetClassAssignment, Rules};
use ew_model::units::{Point, um};
use ew_pcb::drc::{Options, check};
use ew_pcb::pairs;

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

fn net(pcb: &mut Pcb, name: &str) -> Id {
    let id = Id::random();
    pcb.nets.insert(Net {
        id,
        name: name.into(),
        class: String::new(),
    });
    id
}

fn track(net: Id, y: i64, width: i64) -> Track {
    Track {
        id: Id::random(),
        layer: CopperLayer::Top,
        net: Some(net),
        width,
        start: Point::new(0, y),
        end: Point::new(um(10_000), y),
        mid: None,
        locked: false,
    }
}

#[test]
fn pairs_are_found_by_their_names() {
    let mut pcb = Pcb::new(Id::random(), "B", Stackup::standard(4, STANDARD_THICKNESS));
    let dp = net(&mut pcb, "USB_DP");
    let dn = net(&mut pcb, "USB_DN");
    let tx = net(&mut pcb, "TX+");
    let rx = net(&mut pcb, "TX-");
    // Two negatives for one positive: not a pair.
    net(&mut pcb, "VP");
    net(&mut pcb, "VN");
    net(&mut pcb, "VM");
    net(&mut pcb, "GND");
    let found = pairs::pairs(&pcb);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].name, "TX");
    assert_eq!((found[0].positive, found[0].negative), (tx, rx));
    assert_eq!(found[1].name, "USB_D");
    assert_eq!(pairs::partner(&pcb, dn), Some(dp));
    assert_eq!(pairs::partner(&pcb, dp), Some(dn));
}

#[test]
#[allow(clippy::cast_possible_truncation, reason = "track sizes")]
fn the_drc_checks_the_gap_of_coupled_tracks() {
    let rules = rules();
    let mut pcb = Pcb::new(Id::random(), "B", Stackup::standard(4, STANDARD_THICKNESS));
    let dp = net(&mut pcb, "USB_DP");
    let dn = net(&mut pcb, "USB_DN");
    let layout = ew_pcb::impedance::layout(
        &rules.impedance_profiles[0],
        &pcb.stackup,
        CopperLayer::Top,
        f64::from(rules.constraints.min_track_width as i32),
        200_000.0,
    )
    .unwrap();
    let (width, gap) = (layout.width as i64, layout.gap as i64);

    // At the profile's gap: nothing to report.
    pcb.tracks.insert(track(dp, 0, width));
    pcb.tracks.insert(track(dn, width + gap, width));
    let gaps = |pcb: &Pcb| {
        check(pcb, Some(&rules), &Options::default())
            .findings
            .into_iter()
            .filter(|f| f.code == "pair-gap")
            .collect::<Vec<_>>()
    };
    assert_eq!(gaps(&pcb).len(), 0);

    // Twice the gap further down: reported once, with both tracks.
    let y = um(5_000);
    pcb.tracks.insert(track(dp, y, width));
    pcb.tracks.insert(track(dn, y + width + 2 * gap, width));
    let found = gaps(&pcb);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].args["pair"], "USB_D");
    assert_eq!(found[0].targets.len(), 2);
}
