//! Schematic sheet scenes.

use ew_model::document::Document;
use ew_model::library::{Pin, PinDirection, PinShape, PinText, SymbolDef};
use ew_model::project::Project;
use ew_model::schematic::{
    Label, LabelKind, PageFormat, PlacedSymbol, PortShape, PowerPort, PowerStyle, Schematic,
    SheetSide, SheetSymbol,
};
use ew_model::shape::Fill;
use ew_model::text::TextStyle;
use ew_model::units::{Point, mm, um};

use crate::Options;
use crate::builder::{Header, LayerDef, NONE, ObjType, Scene, SceneBuilder};
use crate::draw::{self, Paint};
use crate::geom::{self, BBox, P, Xform};
use crate::text::{self, TextItem};

/// Default line widths and sizes (nm).
const WIRE: f64 = 152_400.0;
const BUS: f64 = 304_800.0;
const BODY: f64 = 254_000.0;
const PIN: f64 = 152_400.0;
const FRAME: f64 = 152_400.0;
const JUNCTION_RADIUS: f64 = 457_200.0;
const NO_CONNECT: f64 = 635_000.0;
const PIN_TEXT: f64 = 1_016_000.0;
const GAP: f64 = 508_000.0;

/// Field child indices for the default designator and value of a symbol
/// without explicit fields.
pub const DEFAULT_DESIGNATOR: usize = 0x1_0000;
/// See [`DEFAULT_DESIGNATOR`].
pub const DEFAULT_VALUE: usize = 0x1_0001;

/// The scene layers of a sheet, in panel order.
const LAYERS: [(&str, i32, bool); 16] = [
    ("sch.wires", 55, true),
    ("sch.buses", 50, true),
    ("sch.junctions", 60, true),
    ("sch.labels", 65, true),
    ("sch.power", 66, true),
    ("sch.symbols", 30, true),
    ("sch.symbolFill", 20, true),
    ("sch.pins", 40, true),
    ("sch.pinText", 45, true),
    ("sch.fields", 70, true),
    ("sch.sheets", 25, true),
    ("sch.noConnects", 62, true),
    ("sch.texts", 75, true),
    ("sch.shapes", 28, true),
    ("sch.frame", 10, true),
    ("sch.page", 0, true),
];

struct Layers {
    wires: usize,
    buses: usize,
    junctions: usize,
    labels: usize,
    power: usize,
    symbols: usize,
    fill: usize,
    pins: usize,
    pin_text: usize,
    fields: usize,
    sheets: usize,
    no_connects: usize,
    texts: usize,
    shapes: usize,
    frame: usize,
    page: usize,
}

impl Layers {
    fn new(b: &mut SceneBuilder) -> Self {
        let ids: Vec<usize> = LAYERS
            .iter()
            .map(|(key, z, visible)| {
                b.layer(LayerDef {
                    key: (*key).to_owned(),
                    group: "schematic",
                    side: None,
                    name: None,
                    z: *z,
                    visible: *visible,
                })
            })
            .collect();
        Self {
            wires: ids[0],
            buses: ids[1],
            junctions: ids[2],
            labels: ids[3],
            power: ids[4],
            symbols: ids[5],
            fill: ids[6],
            pins: ids[7],
            pin_text: ids[8],
            fields: ids[9],
            sheets: ids[10],
            no_connects: ids[11],
            texts: ids[12],
            shapes: ids[13],
            frame: ids[14],
            page: ids[15],
        }
    }
}

struct Ctx<'a> {
    project: &'a Project,
    options: &'a Options<'a>,
    origin: P,
    layers: Layers,
}

impl Ctx<'_> {
    fn local(&self, p: Point) -> P {
        P::of(p).sub(self.origin)
    }

    fn resolve(&self, text: &str) -> String {
        if text.contains("${") {
            (self.options.resolve)(text)
        } else {
            text.to_owned()
        }
    }

    fn symbol_def(&self, name: &str) -> Option<&SymbolDef> {
        self.project.documents.values().find_map(|doc| match doc {
            Document::Library(library) => library.symbol(name),
            _ => None,
        })
    }
}

/// A text with explicit alignment.
#[allow(clippy::too_many_arguments, reason = "plain text parameters")]
fn item(text: String, at: P, size: f64, rotation: f64, h: i8, v: i8, obj: u32) -> TextItem {
    TextItem {
        text,
        at,
        size,
        rotation,
        h,
        v,
        flags: 0,
        obj,
    }
}

/// Builds the scene of a sheet.
pub fn build(project: &Project, sheet: &Schematic, grid: i64, options: &Options<'_>) -> Scene {
    let page = sheet.page.dimensions();
    let origin = P::new((page.w / 2) as f64, (page.h / 2) as f64);
    let mut b = SceneBuilder::new(origin);
    let layers = Layers::new(&mut b);
    let ctx = Ctx {
        project,
        options,
        origin,
        layers,
    };

    frame(&mut b, &ctx, sheet);
    for shape in &sheet.shapes {
        let obj = b.object(ObjType::Shape, shape.id, None, None);
        let width = if shape.stroke.width > 0 {
            shape.stroke.width as f64
        } else {
            BODY
        };
        let fill = match shape.fill {
            Fill::None => None,
            Fill::Solid => Some(ctx.layers.shapes),
            Fill::Background => Some(ctx.layers.fill),
        };
        let paint = Paint {
            stroke: Some((ctx.layers.shapes, width)),
            fill,
        };
        draw::geometry(
            &mut b,
            obj,
            &shape.geometry,
            paint,
            &Xform::translate(origin.scale(-1.0)),
        );
    }
    for sheet_symbol in &sheet.sheets {
        sheet_box(&mut b, &ctx, sheet_symbol);
    }
    for symbol in &sheet.symbols {
        placed_symbol(&mut b, &ctx, symbol);
    }
    connections(&mut b, &ctx, sheet);
    for label in &sheet.labels {
        net_label(&mut b, &ctx, label);
    }
    for port in &sheet.power_ports {
        power_port(&mut b, &ctx, port);
    }
    for text in &sheet.texts {
        let obj = b.object(ObjType::Text, text.id, None, None);
        let item = TextItem::styled(
            ctx.resolve(&text.text),
            ctx.local(text.at),
            text.rotation.degrees(),
            &text.style,
            obj,
        );
        b.text(ctx.layers.texts, obj, text::upright(item));
    }

    b.finish(Header {
        doc: sheet.id,
        kind: "schematic",
        name: sheet.name.clone(),
        page: Some([page.w, page.h]),
        home: Some([0, 0, page.w, page.h]),
        grid,
    })
}

/// Builds the scene of one library symbol (one unit), without a page:
/// the preview of the library manager.
pub fn preview(def: &SymbolDef, unit: u16, options: &Options<'_>) -> Scene {
    let mut project = Project::new(def.id, "preview");
    let mut library = ew_model::library::Library::new(def.id, "preview");
    library.symbols.insert(def.clone());
    project.documents.insert(def.id, Document::Library(library));
    let origin = P::new(0.0, 0.0);
    let mut b = SceneBuilder::new(origin);
    let layers = Layers::new(&mut b);
    let ctx = Ctx {
        project: &project,
        options,
        origin,
        layers,
    };
    let symbol = PlacedSymbol {
        id: def.id,
        symbol: def.name.clone(),
        unit: unit.max(1),
        at: Point::ORIGIN,
        rotation: ew_model::units::Angle::ZERO,
        mirror: false,
        designator: format!("{}?", def.prefix),
        value: def.value.clone(),
        footprint: def.footprint.clone(),
        component: None,
        properties: std::collections::BTreeMap::new(),
        fields: Vec::new(),
        flags: ew_model::schematic::ComponentFlags::default(),
        instances: std::collections::BTreeMap::new(),
        alternate: false,
    };
    placed_symbol(&mut b, &ctx, &symbol);
    b.finish(Header {
        doc: def.id,
        kind: "schematic",
        name: def.name.clone(),
        page: None,
        home: None,
        grid: 1_270_000,
    })
}

/// The scene of the symbol editor: one body of a symbol with every pin,
/// graphic and text as an object of its own (ids from [`crate::editor`]);
/// hidden pins are drawn too.
pub fn editor(def: &SymbolDef, unit: u16, alternate: bool, options: &Options<'_>) -> Scene {
    use crate::editor::{GRAPHIC, PIN, TEXT, item_id};
    let project = Project::new(def.id, "editor");
    let origin = P::new(0.0, 0.0);
    let mut b = SceneBuilder::new(origin);
    let layers = Layers::new(&mut b);
    let ctx = Ctx {
        project: &project,
        options,
        origin,
        layers,
    };
    let xf = Xform {
        offset: origin,
        angle: 0.0,
        mirror: false,
    };
    if let Some(body) = def.body(unit, alternate) {
        for (index, graphic) in body.graphics.iter().enumerate() {
            let obj = b.object(ObjType::Shape, item_id(GRAPHIC, index), None, None);
            let width = if graphic.stroke.width > 0 {
                graphic.stroke.width as f64
            } else {
                BODY
            };
            let fill = match graphic.fill {
                Fill::None => None,
                Fill::Solid => Some(ctx.layers.symbols),
                Fill::Background => Some(ctx.layers.fill),
            };
            draw::geometry(
                &mut b,
                obj,
                &graphic.geometry,
                Paint {
                    stroke: Some((ctx.layers.symbols, width)),
                    fill,
                },
                &xf,
            );
        }
        for (index, text) in body.texts.iter().enumerate() {
            let obj = b.object(ObjType::Text, item_id(TEXT, index), None, None);
            let item = TextItem::styled(
                ctx.resolve(&text.text),
                xf.point(text.at),
                text.rotation.degrees(),
                &text.style,
                obj,
            );
            b.text(ctx.layers.symbols, obj, text::upright(item));
        }
        for (index, pin) in body.pins.iter().enumerate() {
            let obj = b.object(ObjType::Pin, item_id(PIN, index), None, None);
            draw_pin(&mut b, &ctx, obj, pin, &xf, &def.pin_text);
        }
    }
    b.finish(Header {
        doc: def.id,
        kind: "schematic",
        name: def.name.clone(),
        page: None,
        home: None,
        grid: 1_270_000,
    })
}

/// Wires, buses, bus entries, junctions and no-connect flags.
fn connections(b: &mut SceneBuilder, ctx: &Ctx<'_>, sheet: &Schematic) {
    for wire in &sheet.wires {
        let obj = b.object(ObjType::Wire, wire.id, None, None);
        b.segment(
            ctx.layers.wires,
            obj,
            ctx.local(wire.start),
            ctx.local(wire.end),
            WIRE,
        );
    }
    for bus in &sheet.buses {
        let obj = b.object(ObjType::Bus, bus.id, None, None);
        b.segment(
            ctx.layers.buses,
            obj,
            ctx.local(bus.start),
            ctx.local(bus.end),
            BUS,
        );
    }
    for entry in &sheet.bus_entries {
        let obj = b.object(ObjType::BusEntry, entry.id, None, None);
        let at = ctx.local(entry.at);
        b.segment(ctx.layers.buses, obj, at, at.add(P::of(entry.size)), WIRE);
    }
    for junction in &sheet.junctions {
        let obj = b.object(ObjType::Junction, junction.id, None, None);
        b.circle(
            ctx.layers.junctions,
            obj,
            ctx.local(junction.at),
            JUNCTION_RADIUS,
            0.0,
        );
    }
    for flag in &sheet.no_connects {
        let obj = b.object(ObjType::NoConnect, flag.id, None, None);
        let at = ctx.local(flag.at);
        let d = NO_CONNECT;
        let layer = ctx.layers.no_connects;
        b.segment(
            layer,
            obj,
            at.add(P::new(-d, -d)),
            at.add(P::new(d, d)),
            WIRE,
        );
        b.segment(
            layer,
            obj,
            at.add(P::new(-d, d)),
            at.add(P::new(d, -d)),
            WIRE,
        );
    }
}

fn page_name(format: PageFormat) -> &'static str {
    match format {
        PageFormat::A5 => "A5",
        PageFormat::A4 => "A4",
        PageFormat::A3 => "A3",
        PageFormat::A2 => "A2",
        PageFormat::A1 => "A1",
        PageFormat::A0 => "A0",
        PageFormat::Letter => "Letter",
        PageFormat::Legal => "Legal",
        PageFormat::Tabloid => "Tabloid",
        PageFormat::Custom => "Custom",
    }
}

/// Paper, border and title block.
fn frame(b: &mut SceneBuilder, ctx: &Ctx<'_>, sheet: &Schematic) {
    let page = sheet.page.dimensions();
    let (w, h) = (page.w as f64, page.h as f64);
    let at = |x: f64, y: f64| P::new(x, y).sub(ctx.origin);
    let paper = [at(0.0, 0.0), at(w, 0.0), at(w, h), at(0.0, h)];
    b.polygon(ctx.layers.page, NONE, &paper, &[], 0);
    let layer = ctx.layers.frame;
    let margin = mm(10) as f64;
    let border = [
        at(margin, margin),
        at(w - margin, margin),
        at(w - margin, h - margin),
        at(margin, h - margin),
    ];
    b.polyline(layer, NONE, &border, true, FRAME * 2.0);

    // Title block: four rows in the bottom-right corner.
    let block_w = mm(110) as f64;
    let row = mm(8) as f64;
    let x0 = w - margin - block_w;
    let x1 = w - margin;
    let y0 = margin;
    let rows = 4.0;
    b.polyline(
        layer,
        NONE,
        &[
            at(x0, y0),
            at(x1, y0),
            at(x1, y0 + rows * row),
            at(x0, y0 + rows * row),
        ],
        true,
        FRAME * 2.0,
    );
    for i in 1..4 {
        let y = y0 + f64::from(i) * row;
        b.segment(layer, NONE, at(x0, y), at(x1, y), FRAME);
    }
    // Row 1 is split into size | date | revision.
    let third = block_w / 3.0;
    for i in 1..3 {
        let x = x0 + f64::from(i) * third;
        b.segment(layer, NONE, at(x, y0 + row), at(x, y0 + 2.0 * row), FRAME);
    }
    let labels = &ctx.options.labels;
    let tb = &sheet.title_block;
    let small = um(1_270) as f64;
    let pad = um(1_500) as f64;
    let mut cell = |x: f64, y: f64, text: String, size: f64| {
        b.text(
            layer,
            NONE,
            item(text, at(x + pad, y + row / 2.0), size, 0.0, -1, 0, NONE),
        );
    };
    let path = ctx
        .project
        .reference(sheet.id)
        .map(|entry| entry.path.clone())
        .unwrap_or_default();
    cell(x0, y0, format!("{}: {path}", labels.file), small);
    let size_text = match sheet.page.format {
        PageFormat::Custom => format!(
            "{}: {} × {} mm",
            labels.size,
            ew_model::units::to_mm(page.w),
            ew_model::units::to_mm(page.h)
        ),
        format => format!("{}: {}", labels.size, page_name(format)),
    };
    cell(x0, y0 + row, size_text, small);
    cell(
        x0 + third,
        y0 + row,
        format!("{}: {}", labels.date, ctx.resolve(&tb.date)),
        small,
    );
    cell(
        x0 + 2.0 * third,
        y0 + row,
        format!("{}: {}", labels.revision, ctx.resolve(&tb.revision)),
        small,
    );
    cell(x0, y0 + 2.0 * row, ctx.resolve(&tb.title), um(2_540) as f64);
    let company = ctx.resolve(&tb.company);
    let bottom_line = if company.is_empty() {
        tb.comments
            .first()
            .map(|c| ctx.resolve(c))
            .unwrap_or_default()
    } else {
        company
    };
    cell(x0, y0 + 3.0 * row, bottom_line, small);
}

fn pin_vector(direction: PinDirection) -> P {
    match direction {
        PinDirection::Right => P::new(1.0, 0.0),
        PinDirection::Left => P::new(-1.0, 0.0),
        PinDirection::Up => P::new(0.0, 1.0),
        PinDirection::Down => P::new(0.0, -1.0),
    }
}

fn placed_symbol(b: &mut SceneBuilder, ctx: &Ctx<'_>, symbol: &PlacedSymbol) {
    let xf = Xform {
        offset: ctx.local(symbol.at),
        angle: geom::radians(symbol.rotation),
        mirror: symbol.mirror,
    };
    let obj = b.object(ObjType::Symbol, symbol.id, None, None);
    let def = ctx.symbol_def(&symbol.symbol);
    let pin_text = def.map(|def| def.pin_text).unwrap_or_default();
    let unit = def.and_then(|def| def.body(symbol.unit, symbol.alternate));
    let mut body;
    let mut vertical_pins = 0;
    let mut pin_count = 0;
    if let Some(unit) = unit {
        for graphic in unit.graphics {
            let width = if graphic.stroke.width > 0 {
                graphic.stroke.width as f64
            } else {
                BODY
            };
            let fill = match graphic.fill {
                Fill::None => None,
                Fill::Solid => Some(ctx.layers.symbols),
                Fill::Background => Some(ctx.layers.fill),
            };
            draw::geometry(
                b,
                obj,
                &graphic.geometry,
                Paint {
                    stroke: Some((ctx.layers.symbols, width)),
                    fill,
                },
                &xf,
            );
        }
        body = b.bbox(obj);
        for text in unit.texts {
            let rotation = xf.shape_angle(geom::radians(text.rotation)).to_degrees();
            let item = TextItem::styled(
                ctx.resolve(&text.text),
                xf.point(text.at),
                rotation,
                &text.style,
                obj,
            );
            b.text(ctx.layers.symbols, obj, text::upright(item));
        }
        for (index, pin) in unit.pins.iter().enumerate() {
            if pin.hidden {
                continue;
            }
            let pin_obj = b.object(ObjType::Pin, symbol.id, Some(obj), Some(index));
            let direction = draw_pin(b, ctx, pin_obj, pin, &xf, &pin_text);
            pin_count += 1;
            if direction.y.abs() > direction.x.abs() {
                vertical_pins += 1;
            }
            let bbox = b.bbox(pin_obj);
            b.extend(obj, bbox);
        }
    } else {
        // Unknown symbol: a placeholder box with a question mark.
        let size = um(2_540) as f64;
        let corners = [
            P::new(-size, -size),
            P::new(size, -size),
            P::new(size, size),
            P::new(-size, size),
        ]
        .map(|p| xf.apply(p));
        b.polyline(ctx.layers.symbols, obj, &corners, true, BODY);
        b.text(
            ctx.layers.symbols,
            obj,
            item("?".into(), xf.offset, size, 0.0, 0, 0, obj),
        );
        body = b.bbox(obj);
    }
    if body.is_empty() {
        body = BBox::of(xf.offset).grow(um(1_270) as f64);
    }
    let all = b.bbox(obj).union(body);
    fields(
        b,
        ctx,
        symbol,
        obj,
        body,
        all,
        pin_count == 2 && vertical_pins == 2,
    );
}

/// Draws a pin and its texts; returns the direction from the connection
/// point into the body.
fn draw_pin(
    b: &mut SceneBuilder,
    ctx: &Ctx<'_>,
    obj: u32,
    pin: &Pin,
    xf: &Xform,
    text: &PinText,
) -> P {
    let start = xf.point(pin.at);
    let end_local = P::of(pin.at).add(pin_vector(pin.direction).scale(pin.length as f64));
    let end = xf.apply(end_local);
    let length = end.sub(start).len().max(1.0);
    let d = end.sub(start).scale(1.0 / length);
    let perp = P::new(-d.y, d.x);
    let layer = ctx.layers.pins;
    let inverted = matches!(pin.shape, PinShape::Inverted | PinShape::InvertedClock);
    let bubble = um(635) as f64;
    let line_end = if inverted {
        end.sub(d.scale(bubble * 2.0))
    } else {
        end
    };
    b.segment(layer, obj, start, line_end, PIN);
    if inverted {
        b.arc(
            layer,
            obj,
            end.sub(d.scale(bubble)),
            bubble,
            0.0,
            std::f64::consts::TAU,
            PIN,
        );
    }
    if matches!(
        pin.shape,
        PinShape::Clock | PinShape::InvertedClock | PinShape::ClockLow | PinShape::FallingEdgeClock
    ) {
        let points = [
            end.add(perp.scale(bubble)),
            end.add(d.scale(bubble * 1.5)),
            end.sub(perp.scale(bubble)),
        ];
        b.polyline(layer, obj, &points, false, PIN);
    }
    if matches!(
        pin.shape,
        PinShape::InputLow | PinShape::ClockLow | PinShape::OutputLow
    ) {
        let points = [end.sub(d.scale(bubble * 2.0)).add(perp.scale(bubble)), end];
        b.polyline(layer, obj, &points, false, PIN);
    }

    let horizontal = d.x.abs() >= d.y.abs();
    let middle = start.add(end).scale(0.5);
    // Above the pin line (left of it when vertical), or below (right).
    let beside = |above: bool| -> (P, f64, i8) {
        let gap = um(250) as f64;
        if horizontal {
            (
                middle.add(P::new(0.0, if above { gap } else { -gap })),
                0.0,
                if above { -1 } else { 1 },
            )
        } else {
            (
                middle.add(P::new(if above { -gap } else { gap }, 0.0)),
                90.0,
                if above { -1 } else { 1 },
            )
        }
    };
    let names_outside = text.name_offset == 0;
    if !pin.number.is_empty() && !text.hide_numbers {
        let (at, rotation, v) = beside(!names_outside);
        b.text(
            ctx.layers.pin_text,
            obj,
            item(pin.number.clone(), at, PIN_TEXT, rotation, 0, v, obj),
        );
    }
    if !pin.name.is_empty() && pin.name != "~" && !text.hide_names {
        if names_outside {
            let (at, rotation, v) = beside(true);
            b.text(
                ctx.layers.pin_text,
                obj,
                item(pin.name.clone(), at, um(1_270) as f64, rotation, 0, v, obj),
            );
        } else {
            let at = end.add(d.scale(text.name_offset as f64));
            let (rotation, h) = if horizontal {
                (0.0, if d.x > 0.0 { -1 } else { 1 })
            } else {
                (90.0, if d.y > 0.0 { -1 } else { 1 })
            };
            b.text(
                ctx.layers.pin_text,
                obj,
                item(pin.name.clone(), at, um(1_270) as f64, rotation, h, 0, obj),
            );
        }
    }
    d
}

#[allow(clippy::too_many_arguments, reason = "internal helper")]
fn fields(
    b: &mut SceneBuilder,
    ctx: &Ctx<'_>,
    symbol: &PlacedSymbol,
    obj: u32,
    body: BBox,
    all: BBox,
    two_vertical_pins: bool,
) {
    let value_of = |name: &str| -> String {
        match name.to_ascii_lowercase().as_str() {
            "designator" | "reference" => symbol.designator.clone(),
            "value" => symbol.value.clone(),
            "footprint" => symbol.footprint.clone().unwrap_or_default(),
            _ => symbol.properties.get(name).cloned().unwrap_or_default(),
        }
    };
    if !symbol.fields.is_empty() {
        for (index, field) in symbol.fields.iter().enumerate() {
            if !field.visible {
                continue;
            }
            let field_obj = b.object(ObjType::Field, symbol.id, Some(obj), Some(index));
            let item = TextItem::styled(
                ctx.resolve(&value_of(&field.name)),
                ctx.local(field.at),
                field.rotation.degrees(),
                &field.style,
                field_obj,
            );
            b.text(ctx.layers.fields, field_obj, text::upright(item));
        }
        return;
    }
    let size = TextStyle::schematic().size as f64;
    let gap = um(500) as f64;
    let (designator, value) = if two_vertical_pins {
        let x = body.max_x + gap;
        let y = body.center().y;
        (
            item(
                String::new(),
                P::new(x, y + um(200) as f64),
                size,
                0.0,
                -1,
                -1,
                NONE,
            ),
            item(
                String::new(),
                P::new(x, y - um(200) as f64),
                size,
                0.0,
                -1,
                1,
                NONE,
            ),
        )
    } else {
        (
            item(
                String::new(),
                P::new(all.min_x, all.max_y + gap),
                size,
                0.0,
                -1,
                -1,
                NONE,
            ),
            item(
                String::new(),
                P::new(all.min_x, all.min_y - gap),
                size,
                0.0,
                -1,
                1,
                NONE,
            ),
        )
    };
    for (index, mut text, content) in [
        (DEFAULT_DESIGNATOR, designator, symbol.designator.clone()),
        (DEFAULT_VALUE, value, symbol.value.clone()),
    ] {
        let field_obj = b.object(ObjType::Field, symbol.id, Some(obj), Some(index));
        text.text = ctx.resolve(&content);
        text.obj = field_obj;
        b.text(ctx.layers.fields, field_obj, text);
    }
}

fn sheet_box(b: &mut SceneBuilder, ctx: &Ctx<'_>, sheet: &SheetSymbol) {
    let obj = b.object(ObjType::Sheet, sheet.id, None, None);
    let top_left = ctx.local(sheet.at);
    let (w, h) = (sheet.size.w as f64, sheet.size.h as f64);
    let corners = [
        top_left,
        top_left.add(P::new(w, 0.0)),
        top_left.add(P::new(w, -h)),
        top_left.add(P::new(0.0, -h)),
    ];
    b.polygon(ctx.layers.fill, obj, &corners, &[], 0);
    b.polyline(ctx.layers.sheets, obj, &corners, true, BODY);
    let size = TextStyle::schematic().size as f64;
    b.text(
        ctx.layers.sheets,
        obj,
        item(
            sheet.name.clone(),
            top_left.add(P::new(0.0, GAP)),
            size,
            0.0,
            -1,
            -1,
            obj,
        ),
    );
    let file = ctx
        .project
        .reference(sheet.schematic)
        .map(|entry| entry.path.clone())
        .unwrap_or_default();
    b.text(
        ctx.layers.sheets,
        obj,
        item(
            format!("{}: {file}", ctx.options.labels.file),
            top_left.add(P::new(0.0, -h - GAP)),
            size * 0.8,
            0.0,
            -1,
            1,
            obj,
        ),
    );
    for (index, pin) in sheet.pins.iter().enumerate() {
        let pin_obj = b.object(ObjType::SheetPin, sheet.id, Some(obj), Some(index));
        let offset = pin.offset as f64;
        let (at, inward) = match pin.side {
            SheetSide::Left => (top_left.add(P::new(0.0, -offset)), P::new(1.0, 0.0)),
            SheetSide::Right => (top_left.add(P::new(w, -offset)), P::new(-1.0, 0.0)),
            SheetSide::Top => (top_left.add(P::new(offset, 0.0)), P::new(0.0, -1.0)),
            SheetSide::Bottom => (top_left.add(P::new(offset, -h)), P::new(0.0, 1.0)),
        };
        port_shape(
            b,
            ctx.layers.sheets,
            pin_obj,
            at,
            inward,
            pin.shape,
            size * 0.8,
            um(1_000) as f64,
        );
        let text_at = at.add(inward.scale(um(1_800) as f64));
        let horizontal = inward.x.abs() > 0.5;
        let (rotation, h_align) = if horizontal {
            (0.0, if inward.x > 0.0 { -1 } else { 1 })
        } else {
            (90.0, if inward.y > 0.0 { -1 } else { 1 })
        };
        b.text(
            ctx.layers.sheets,
            pin_obj,
            item(
                pin.name.clone(),
                text_at,
                size * 0.8,
                rotation,
                h_align,
                0,
                pin_obj,
            ),
        );
    }
}

/// A small port arrow (sheet pins) pointing along `dir` from `at`.
#[allow(clippy::too_many_arguments, reason = "plain geometry parameters")]
fn port_shape(
    b: &mut SceneBuilder,
    layer: usize,
    obj: u32,
    at: P,
    dir: P,
    shape: PortShape,
    size: f64,
    length: f64,
) {
    let half = size * 0.5;
    let perp = P::new(-dir.y, dir.x);
    let pt = |along: f64, across: f64| at.add(dir.scale(along)).add(perp.scale(across));
    let points: Vec<P> = match shape {
        PortShape::Input => vec![
            pt(0.0, half),
            pt(length - half, half),
            pt(length, 0.0),
            pt(length - half, -half),
            pt(0.0, -half),
        ],
        PortShape::Output => vec![
            pt(0.0, 0.0),
            pt(half, half),
            pt(length, half),
            pt(length, -half),
            pt(half, -half),
        ],
        PortShape::Bidirectional | PortShape::TriState => vec![
            pt(0.0, 0.0),
            pt(half, half),
            pt(length - half, half),
            pt(length, 0.0),
            pt(length - half, -half),
            pt(half, -half),
        ],
        PortShape::Passive => vec![
            pt(0.0, half),
            pt(length, half),
            pt(length, -half),
            pt(0.0, -half),
        ],
    };
    b.polyline(layer, obj, &points, true, PIN);
}

fn net_label(b: &mut SceneBuilder, ctx: &Ctx<'_>, label: &Label) {
    let obj = b.object(ObjType::Label, label.id, None, None);
    let at = ctx.local(label.at);
    let size = label.style.size as f64;
    let angle = geom::normalize(geom::radians(label.rotation));
    let dir = P::new(1.0, 0.0).rotate(angle);
    let horizontal = dir.x.abs() >= dir.y.abs();
    let forward = if horizontal {
        dir.x >= 0.0
    } else {
        dir.y >= 0.0
    };
    let rotation = if horizontal { 0.0 } else { 90.0 };
    let h = if forward { -1 } else { 1 };
    let text = ctx.resolve(&label.text);
    let layer = ctx.layers.labels;
    match label.kind {
        LabelKind::Local => {
            let lift = if horizontal {
                P::new(0.0, um(250) as f64)
            } else {
                P::new(-(um(250) as f64), 0.0)
            };
            b.text(
                layer,
                obj,
                item(text, at.add(lift), size, rotation, h, -1, obj),
            );
        }
        LabelKind::Global | LabelKind::Hierarchical => {
            let half = size * 0.8;
            let (width, _) = text::extent(&text, size);
            let pointed_start = matches!(
                label.shape,
                PortShape::Input | PortShape::Bidirectional | PortShape::TriState
            );
            let pointed_end = matches!(
                label.shape,
                PortShape::Output | PortShape::Bidirectional | PortShape::TriState
            );
            let start_pad = if pointed_start { half } else { half * 0.5 };
            let length = start_pad + width + half * if pointed_end { 1.5 } else { 0.5 };
            let unit = if horizontal {
                P::new(if forward { 1.0 } else { -1.0 }, 0.0)
            } else {
                P::new(0.0, if forward { 1.0 } else { -1.0 })
            };
            let perp = P::new(-unit.y, unit.x);
            let pt = |along: f64, across: f64| at.add(unit.scale(along)).add(perp.scale(across));
            let mut points = Vec::new();
            if pointed_start {
                points.push(pt(0.0, 0.0));
            }
            points.push(pt(if pointed_start { half } else { 0.0 }, half));
            if pointed_end {
                points.extend([
                    pt(length - half, half),
                    pt(length, 0.0),
                    pt(length - half, -half),
                ]);
            } else {
                points.extend([pt(length, half), pt(length, -half)]);
            }
            points.push(pt(if pointed_start { half } else { 0.0 }, -half));
            let width_line = if label.kind == LabelKind::Global {
                PIN
            } else {
                PIN * 0.75
            };
            b.polyline(layer, obj, &points, true, width_line);
            b.text(
                layer,
                obj,
                item(
                    text,
                    pt(start_pad + half * 0.3, 0.0),
                    size,
                    rotation,
                    h,
                    0,
                    obj,
                ),
            );
        }
    }
}

fn power_port(b: &mut SceneBuilder, ctx: &Ctx<'_>, port: &PowerPort) {
    let obj = b.object(ObjType::PowerPort, port.id, None, None);
    let xf = Xform {
        offset: ctx.local(port.at),
        angle: geom::radians(port.rotation),
        mirror: false,
    };
    let layer = ctx.layers.power;
    let u = |v: i64| um(v) as f64;
    let line = |b: &mut SceneBuilder, points: &[(i64, i64)], closed: bool| {
        let points: Vec<P> = points
            .iter()
            .map(|(x, y)| xf.apply(P::new(u(*x), u(*y))))
            .collect();
        b.polyline(layer, obj, &points, closed, PIN);
    };
    // The local symbol points up (+y) from the connection point, except the
    // ground styles, which point down.
    let tip = match port.style {
        PowerStyle::Bar | PowerStyle::Wave => {
            line(b, &[(0, 0), (0, 1_270)], false);
            line(b, &[(-762, 1_270), (762, 1_270)], false);
            P::new(0.0, u(1_270))
        }
        PowerStyle::Arrow => {
            line(b, &[(0, 0), (0, 1_016)], false);
            line(b, &[(-508, 1_016), (0, 1_778), (508, 1_016)], true);
            P::new(0.0, u(1_778))
        }
        PowerStyle::Circle => {
            line(b, &[(0, 0), (0, 1_016)], false);
            b.arc(
                layer,
                obj,
                xf.apply(P::new(0.0, u(1_524))),
                u(508),
                0.0,
                std::f64::consts::TAU,
                PIN,
            );
            P::new(0.0, u(2_032))
        }
        PowerStyle::Ground => {
            line(b, &[(0, 0), (0, -1_270)], false);
            line(b, &[(-1_270, -1_270), (1_270, -1_270)], false);
            line(b, &[(-762, -1_778), (762, -1_778)], false);
            line(b, &[(-254, -2_286), (254, -2_286)], false);
            P::new(0.0, -u(2_286))
        }
        PowerStyle::SignalGround => {
            line(b, &[(0, 0), (0, -1_270)], false);
            line(b, &[(-1_270, -1_270), (1_270, -1_270), (0, -2_540)], true);
            P::new(0.0, -u(2_540))
        }
        PowerStyle::Earth => {
            line(b, &[(0, 0), (0, -1_270)], false);
            line(b, &[(-1_270, -1_270), (1_270, -1_270)], false);
            for x in [-1_270, 0, 1_270] {
                line(b, &[(x, -1_270), (x - 508, -2_032)], false);
            }
            P::new(0.0, -u(2_032))
        }
    };
    let tip_world = xf.apply(tip);
    let out = tip_world.sub(xf.offset);
    let length = out.len().max(1.0);
    let dir = out.scale(1.0 / length);
    let at = tip_world.add(dir.scale(u(500)));
    let size = TextStyle::schematic().size as f64;
    let text = if dir.y.abs() >= dir.x.abs() {
        item(
            port.net.clone(),
            at,
            size,
            0.0,
            0,
            if dir.y > 0.0 { -1 } else { 1 },
            obj,
        )
    } else {
        item(
            port.net.clone(),
            at,
            size,
            0.0,
            if dir.x > 0.0 { -1 } else { 1 },
            0,
            obj,
        )
    };
    b.text(layer, obj, text);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{options, synthetic};

    #[test]
    fn synthetic_sheet_has_symbols_wires_labels_and_frame() {
        let project = synthetic(ew_synth::DesignSize::S);
        let sheet = project
            .documents
            .values()
            .find_map(|doc| match doc {
                Document::Schematic(sheet) if !sheet.symbols.is_empty() => Some(sheet),
                _ => None,
            })
            .unwrap();
        let scene = build(&project, sheet, 1_270_000, &options());
        let meta = &scene.meta;
        assert_eq!(meta.kind, "schematic");
        let layer = |key: &str| meta.layers.iter().find(|l| l.key == key).unwrap();
        assert!(layer("sch.wires").segments.count as usize >= sheet.wires.len());
        assert!(layer("sch.symbols").segments.count > 0, "symbol bodies");
        assert!(layer("sch.pins").segments.count > 0, "pins");
        assert!(
            !layer("sch.fields").texts.is_empty(),
            "designators and values"
        );
        assert!(!layer("sch.labels").texts.is_empty(), "labels");
        assert!(!layer("sch.frame").texts.is_empty(), "title block");
        assert_eq!(layer("sch.page").mesh.index_count, 6);
        let page = meta.page.unwrap();
        assert_eq!(meta.home, Some([0, 0, page[0], page[1]]));
        assert_eq!(meta.origin, [page[0] / 2, page[1] / 2]);
    }
}
