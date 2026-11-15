//! The DXF export, read back with the board's own DXF import: the same
//! shapes come back at the same places.

#![allow(clippy::unwrap_used, clippy::float_cmp, reason = "test helpers")]

use ew_model::id::Id;
use ew_model::layer::{Layer, Side};
use ew_model::pcb::{BoardShape, BoardText, FootprintInstance, Pcb, Stackup};
use ew_model::shape::Geometry;
use ew_model::text::TextStyle;
use ew_model::units::{Point, mm, um};
use ew_output::dxf::{DxfOptions, default_layers, dxf_files, dxf_layer_name};
use serde_json::json;

/// Bounds equal to within 10 nm (arcs are sampled when read back).
fn assert_bounds(bounds: Option<[i64; 4]>, expected: [i64; 4]) {
    let bounds = bounds.unwrap();
    assert!(
        bounds
            .iter()
            .zip(expected)
            .all(|(a, b)| (a - b).abs() <= 10),
        "{bounds:?} != {expected:?}"
    );
}

fn shape(layer: Layer, geometry: Geometry) -> BoardShape {
    BoardShape {
        id: Id::random(),
        layer,
        geometry,
        width: um(100),
        filled: false,
    }
}

/// A 50 × 30 mm board with a rounded corner, a mounting hole cut-out, a
/// case drawing on mechanical layer 1 and a part with a drawing on the
/// bottom side.
fn board() -> Pcb {
    let mut pcb = Pcb::new(
        Id::random(),
        "Case Test",
        Stackup::standard(2, mm(1) + um(600)),
    );
    let outline = [
        Geometry::Line {
            start: Point::mm(0, 0),
            end: Point::mm(45, 0),
        },
        // A quarter circle around (45, 5) from (45, 0) to (50, 5).
        Geometry::Arc {
            start: Point::mm(45, 0),
            mid: Point::new(mm(45) + um(3536), mm(5) - um(3536)),
            end: Point::mm(50, 5),
        },
        Geometry::Line {
            start: Point::mm(50, 5),
            end: Point::mm(50, 30),
        },
        Geometry::Polyline {
            points: vec![Point::mm(50, 30), Point::mm(0, 30), Point::mm(0, 0)],
        },
        Geometry::Circle {
            center: Point::mm(10, 10),
            radius: mm(2),
        },
    ];
    for geometry in outline {
        pcb.shapes.insert(shape(Layer::Outline, geometry));
    }
    pcb.shapes.insert(shape(
        Layer::Mechanical(1),
        Geometry::Rect {
            start: Point::mm(-2, -2),
            end: Point::mm(52, 32),
        },
    ));
    pcb.texts.insert(BoardText {
        id: Id::random(),
        layer: Layer::Mechanical(1),
        text: "Case – inner wall".into(),
        at: Point::mm(2, 35),
        rotation: ew_model::units::Angle::ZERO,
        style: TextStyle {
            size: mm(2),
            ..TextStyle::default()
        },
        mirrored: false,
    });
    // Copper and silkscreen stay out of the drawing.
    pcb.shapes.insert(shape(
        Layer::Silkscreen(Side::Top),
        Geometry::Line {
            start: Point::mm(1, 1),
            end: Point::mm(5, 1),
        },
    ));
    let part: FootprintInstance = serde_json::from_value(json!({
        "id": Id::random(),
        "footprint": "Header",
        "designator": "J1",
        "at": [mm(30), mm(20)],
        "side": "bottom",
        "graphics": [
            {"layer": "user.2", "geometry": {"line": {"start": [0, 0], "end": [mm(5), 0]}}, "width": um(100)},
            {"layer": "silk.top", "geometry": {"line": {"start": [0, 0], "end": [mm(5), 0]}}, "width": um(100)},
        ],
    }))
    .unwrap();
    pcb.footprints.insert(part);
    pcb
}

#[test]
fn the_outline_comes_back() {
    let pcb = board();
    assert_eq!(
        default_layers(&pcb),
        [Layer::Outline, Layer::Mechanical(1), Layer::User(2)]
    );
    let files = dxf_files(
        &pcb,
        &DxfOptions {
            layers: vec![Layer::Outline],
            inch: false,
            separate: false,
        },
        "Case_Test",
    );
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "Case_Test.dxf");
    let text = String::from_utf8(files[0].data.clone()).unwrap();
    assert!(text.contains("AC1009"));
    assert!(text.ends_with("0\nEOF\n"));

    let drawing = ew_pcb::dxf::read(&text, None).unwrap();
    assert_eq!(drawing.units, "mm");
    assert_eq!(drawing.layers, ["Board_outline"]);
    // Line, arc, line, polyline, circle.
    assert_eq!(drawing.count, 5);
    // The import mirrors y back: the same box as the board.
    assert_bounds(drawing.bounds, [0, 0, mm(50), mm(30)]);

    let arc = drawing
        .shapes
        .iter()
        .find_map(|(_, g)| match g {
            Geometry::Arc { start, mid, end } => Some((*start, *mid, *end)),
            _ => None,
        })
        .unwrap();
    let ends = [arc.0, arc.2];
    for expected in [Point::mm(45, 0), Point::mm(50, 5)] {
        assert!(
            ends.iter()
                .any(|p| (p.x - expected.x).abs() < 10 && (p.y - expected.y).abs() < 10),
            "{arc:?}"
        );
    }
    // The middle stays on the outside of the corner (towards (50, 0)).
    assert!(arc.1.x > mm(48) && arc.1.y < mm(2), "{arc:?}");
}

#[test]
fn layers_files_and_inches() {
    let pcb = board();
    let files = dxf_files(
        &pcb,
        &DxfOptions {
            layers: Vec::new(),
            inch: false,
            separate: true,
        },
        "Case_Test",
    );
    let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Case_Test-Board_outline.dxf",
            "Case_Test-Mechanical_1.dxf",
            "Case_Test-User_2.dxf"
        ]
    );
    let mechanical = String::from_utf8(files[1].data.clone()).unwrap();
    assert!(mechanical.contains("TEXT\n8\nMechanical_1"));
    // Characters DXF R12 cannot hold become `?`.
    assert!(mechanical.contains("Case ? inner wall"));
    let drawing = ew_pcb::dxf::read(&mechanical, None).unwrap();
    assert_bounds(drawing.bounds, [-mm(2), -mm(2), mm(52), mm(32)]);

    // The part on the bottom is mirrored: its line runs to the left.
    let user = ew_pcb::dxf::read(&String::from_utf8(files[2].data.clone()).unwrap(), None).unwrap();
    assert_bounds(user.bounds, [mm(25), mm(20), mm(30), mm(20)]);
    assert_eq!(user.layers, [dxf_layer_name(Layer::User(2))]);

    // Inches: the same drawing, a 25.4th of the numbers.
    let inch = dxf_files(
        &pcb,
        &DxfOptions {
            layers: vec![Layer::Outline],
            inch: true,
            separate: false,
        },
        "Case_Test",
    );
    let drawing =
        ew_pcb::dxf::read(&String::from_utf8(inch[0].data.clone()).unwrap(), None).unwrap();
    assert_eq!(drawing.units, "inch");
    let bounds = drawing.bounds.unwrap();
    assert!((bounds[2] - mm(50)).abs() < 100, "{bounds:?}");
}

#[test]
fn coordinates_follow_the_board_origin() {
    let mut pcb = board();
    pcb.origin = Point::mm(10, 30);
    let files = dxf_files(
        &pcb,
        &DxfOptions {
            layers: vec![Layer::Outline],
            inch: false,
            separate: false,
        },
        "Case_Test",
    );
    let text = String::from_utf8(files[0].data.clone()).unwrap();
    // The lower-left corner (0, 30) is the origin: x from -10, y from 0 up.
    let drawing = ew_pcb::dxf::read(&text, None).unwrap();
    assert_bounds(drawing.bounds, [-mm(10), -mm(30), mm(40), 0]);
}
