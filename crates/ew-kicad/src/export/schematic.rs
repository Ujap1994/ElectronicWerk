//! Schematics and symbols as KiCad files (`.kicad_sch`, `.kicad_sym`): the
//! inverse of the schematic and symbol import, in the KiCad 8 format.

use std::collections::BTreeMap;

use ew_model::id::Id;
use ew_model::library::{
    Pin, PinDirection, PinKind, PinShape, SymbolBody, SymbolDef, SymbolGraphic, SymbolText,
};
use ew_model::schematic::{
    FieldLabel, LabelKind, Orientation, Page, PageFormat, PlacedSymbol, PortShape, PowerPort,
    PowerStyle, Schematic, SheetSide,
};
use ew_model::shape::{Fill, Geometry, LineStyle, Stroke};
use ew_model::text::{HAlign, TextStyle, VAlign};
use ew_model::units::{Nm, Point};

use super::ExportReport;
use super::board::effects;
use crate::write::{Sx, at, uuid, xy};

/// The KiCad schematic and symbol library format written (KiCad 8).
pub const SCH_VERSION: &str = "20231120";

/// `(paper …)` of a page and its height (y = 0 is at the top in KiCad).
fn paper(page: &Page) -> (Sx, Nm) {
    let height = page.dimensions().h;
    let name = match page.format {
        PageFormat::A5 => "A5",
        PageFormat::A4 => "A4",
        PageFormat::A3 => "A3",
        PageFormat::A2 => "A2",
        PageFormat::A1 => "A1",
        PageFormat::A0 => "A0",
        PageFormat::Letter => "USLetter",
        PageFormat::Legal => "USLegal",
        PageFormat::Tabloid => "USLedger",
        PageFormat::Custom => {
            let size = page.dimensions();
            return (Sx::named("paper", "User").mm(size.w).mm(size.h), height);
        }
    };
    let mut node = Sx::named("paper", name);
    if page.orientation == Orientation::Portrait {
        node = node.word("portrait");
    }
    (node, height)
}

/// Page positions: y up from the bottom in ElectronicWerk, down from the
/// top in KiCad.
#[derive(Debug, Clone, Copy)]
struct Frame {
    height: Nm,
}

impl Frame {
    const fn kicad(self, p: Point) -> (Nm, Nm) {
        (p.x, self.height - p.y)
    }

    fn at(self, p: Point, angle: Option<f64>) -> Sx {
        let (x, y) = self.kicad(p);
        at(x, y, angle)
    }

    fn xy(self, p: Point) -> Sx {
        let (x, y) = self.kicad(p);
        xy(x, y)
    }

    fn point(self, head: &str, p: Point) -> Sx {
        let (x, y) = self.kicad(p);
        Sx::list(head).mm(x).mm(y)
    }
}

fn stroke_sx(stroke: &Stroke) -> Sx {
    let kind = match stroke.style {
        LineStyle::Solid => "default",
        LineStyle::Dash => "dash",
        LineStyle::Dot => "dot",
        LineStyle::DashDot => "dash_dot",
    };
    Sx::list("stroke")
        .child(Sx::list("width").mm(stroke.width))
        .child(Sx::pair("type", kind))
}

fn fill_sx(fill: Fill) -> Sx {
    let kind = match fill {
        Fill::None => "none",
        Fill::Solid => "outline",
        Fill::Background => "background",
    };
    Sx::list("fill").child(Sx::pair("type", kind))
}

/// A drawn shape of a symbol or sheet. `point` converts positions.
fn shape_sx(
    geometry: &Geometry,
    stroke: &Stroke,
    fill: Fill,
    point: &dyn Fn(&str, Point) -> Sx,
) -> Sx {
    let polyline = |points: &[Point]| {
        Sx::list("polyline").child(Sx::list("pts").children(points.iter().map(|&p| point("xy", p))))
    };
    let node = match geometry {
        Geometry::Line { start, end } => polyline(&[*start, *end]),
        Geometry::Rect { start, end } => Sx::list("rectangle")
            .child(point("start", *start))
            .child(point("end", *end)),
        Geometry::Circle { center, radius } => Sx::list("circle")
            .child(point("center", *center))
            .child(Sx::list("radius").mm(*radius)),
        Geometry::Arc { start, mid, end } => Sx::list("arc")
            .child(point("start", *start))
            .child(point("mid", *mid))
            .child(point("end", *end)),
        Geometry::Polyline { points } => polyline(points),
        Geometry::Polygon { points } => {
            let mut closed = points.clone();
            if let Some(&first) = points.first() {
                closed.push(first);
            }
            polyline(&closed)
        }
        Geometry::Bezier {
            start,
            control1,
            control2,
            end,
        } => Sx::list("bezier").child(
            Sx::list("pts").children(
                [start, control1, control2, end]
                    .into_iter()
                    .map(|&p| point("xy", p)),
            ),
        ),
    };
    node.child(stroke_sx(stroke)).child(fill_sx(fill))
}

fn pin_kind(kind: PinKind) -> &'static str {
    match kind {
        PinKind::Input => "input",
        PinKind::Output => "output",
        PinKind::Bidirectional => "bidirectional",
        PinKind::TriState => "tri_state",
        PinKind::Passive => "passive",
        PinKind::Free => "free",
        PinKind::Unspecified => "unspecified",
        PinKind::PowerIn => "power_in",
        PinKind::PowerOut => "power_out",
        PinKind::OpenCollector => "open_collector",
        PinKind::OpenEmitter => "open_emitter",
        PinKind::NoConnect => "no_connect",
    }
}

fn pin_shape(shape: PinShape) -> &'static str {
    match shape {
        PinShape::Line => "line",
        PinShape::Inverted => "inverted",
        PinShape::Clock => "clock",
        PinShape::InvertedClock => "inverted_clock",
        PinShape::InputLow => "input_low",
        PinShape::ClockLow => "clock_low",
        PinShape::OutputLow => "output_low",
        PinShape::FallingEdgeClock => "edge_clock_high",
        PinShape::NonLogic => "non_logic",
    }
}

const fn direction_angle(direction: PinDirection) -> f64 {
    match direction {
        PinDirection::Right => 0.0,
        PinDirection::Up => 90.0,
        PinDirection::Left => 180.0,
        PinDirection::Down => 270.0,
    }
}

fn font() -> Sx {
    Sx::list("effects").child(Sx::list("font").child(Sx::list("size").num(1.27).num(1.27)))
}

/// Symbol library coordinates are y up, like ours.
fn up(head: &str, p: Point) -> Sx {
    Sx::list(head).mm(p.x).mm(p.y)
}

fn pin_sx(pin: &Pin) -> Sx {
    let mut node = Sx::list("pin")
        .word(pin_kind(pin.kind))
        .word(pin_shape(pin.shape))
        .child(at(pin.at.x, pin.at.y, Some(direction_angle(pin.direction))))
        .child(Sx::list("length").mm(pin.length));
    if pin.hidden {
        node = node.child(Sx::flag("hide", true));
    }
    let name = if pin.name.is_empty() { "~" } else { &pin.name };
    node.child(Sx::named("name", name).child(font()))
        .child(Sx::named("number", pin.number.clone()).child(font()))
}

fn body_items(body_pins: &[Pin], graphics: &[SymbolGraphic], texts: &[SymbolText]) -> Vec<Sx> {
    let mut out = Vec::new();
    for graphic in graphics {
        out.push(shape_sx(
            &graphic.geometry,
            &graphic.stroke,
            graphic.fill,
            &up,
        ));
    }
    for text in texts {
        out.push(
            Sx::list("text")
                .text(text.text.clone())
                .child(at(text.at.x, text.at.y, Some(text.rotation.degrees())))
                .child(effects(&text.style, false, false)),
        );
    }
    for pin in body_pins {
        out.push(pin_sx(pin));
    }
    out
}

/// A hidden property of a symbol definition.
fn def_property(key: &str, value: &str, shown: bool) -> Sx {
    Sx::list("property")
        .text(key)
        .text(value)
        .child(at(0, 0, Some(0.0)))
        .child(effects(&TextStyle::default(), !shown, false))
}

/// A symbol definition as `(symbol "Library:Name" …)`.
#[must_use]
pub fn symbol_sx(def: &SymbolDef, lib_id: &str) -> Sx {
    let base = lib_id.rsplit(':').next().unwrap_or(lib_id).to_owned();
    let mut node = Sx::list("symbol").text(lib_id);
    if def.power {
        node = node.child(Sx::list("power"));
    }
    if def.pin_text.hide_numbers {
        node = node.child(Sx::list("pin_numbers").child(Sx::flag("hide", true)));
    }
    let mut names = Sx::list("pin_names").child(Sx::list("offset").mm(def.pin_text.name_offset));
    if def.pin_text.hide_names {
        names = names.child(Sx::flag("hide", true));
    }
    node = node
        .child(names)
        .child(Sx::flag("exclude_from_sim", false))
        .child(Sx::flag("in_bom", !def.power))
        .child(Sx::flag("on_board", !def.power));
    let prefix = if def.prefix.is_empty() {
        "U".to_owned()
    } else {
        def.prefix.clone()
    };
    node = node
        .child(def_property("Reference", &prefix, true))
        .child(def_property(
            "Value",
            if def.value.is_empty() {
                &base
            } else {
                &def.value
            },
            true,
        ))
        .child(def_property(
            "Footprint",
            def.footprint.as_deref().unwrap_or(""),
            false,
        ))
        .child(def_property(
            "Datasheet",
            if def.datasheet.is_empty() {
                "~"
            } else {
                &def.datasheet
            },
            false,
        ))
        .child(def_property("Description", &def.description, false));
    if !def.keywords.is_empty() {
        node = node.child(def_property("ki_keywords", &def.keywords, false));
    }
    for (key, value) in &def.properties {
        node = node.child(def_property(key, value, false));
    }
    for (index, unit) in def.units.iter().enumerate() {
        let number = index + 1;
        let mut part = Sx::list("symbol").text(format!("{base}_{number}_1"));
        if !unit.name.is_empty() {
            part = part.child(Sx::named("unit_name", unit.name.clone()));
        }
        part = part.children(body_items(&unit.pins, &unit.graphics, &unit.texts));
        node = node.child(part);
        if let Some(SymbolBody {
            pins,
            graphics,
            texts,
        }) = &unit.alternate
        {
            node = node.child(
                Sx::list("symbol")
                    .text(format!("{base}_{number}_2"))
                    .children(body_items(pins, graphics, texts)),
            );
        }
    }
    node
}

/// A symbol library (`.kicad_sym`).
#[must_use]
pub fn library_file(symbols: &[(String, &SymbolDef)]) -> String {
    Sx::list("kicad_symbol_lib")
        .child(Sx::pair("version", SCH_VERSION))
        .child(Sx::named("generator", "electronicwerk"))
        .children(symbols.iter().map(|(name, def)| symbol_sx(def, name)))
        .render()
}

/// The library symbol of a power port: a hidden power-in pin named after
/// the net, with the port's picture.
#[must_use]
pub fn power_symbol(net: &str, style: PowerStyle) -> SymbolDef {
    use ew_model::library::{PinText, SymbolUnit};
    let line = |a: (i64, i64), b: (i64, i64)| SymbolGraphic {
        geometry: Geometry::Line {
            start: Point::new(a.0, a.1),
            end: Point::new(b.0, b.1),
        },
        stroke: Stroke::default(),
        fill: Fill::None,
    };
    let u = 1_270_000 / 2; // 0.635 mm
    let below = matches!(
        style,
        PowerStyle::Ground | PowerStyle::SignalGround | PowerStyle::Earth
    );
    let s = if below { -1 } else { 1 };
    let mut graphics = vec![line((0, 0), (0, s * 2 * u))];
    match style {
        PowerStyle::Bar => graphics.push(line((-2 * u, 2 * u), (2 * u, 2 * u))),
        PowerStyle::Arrow => {
            graphics.push(line((-u, 2 * u), (0, 4 * u)));
            graphics.push(line((0, 4 * u), (u, 2 * u)));
        }
        PowerStyle::Circle | PowerStyle::Wave => graphics.push(SymbolGraphic {
            geometry: Geometry::Circle {
                center: Point::new(0, 3 * u),
                radius: u,
            },
            stroke: Stroke::default(),
            fill: Fill::None,
        }),
        PowerStyle::Ground => graphics.push(SymbolGraphic {
            geometry: Geometry::Polygon {
                points: vec![
                    Point::new(-2 * u, -2 * u),
                    Point::new(0, -4 * u),
                    Point::new(2 * u, -2 * u),
                ],
            },
            stroke: Stroke::default(),
            fill: Fill::None,
        }),
        PowerStyle::SignalGround => graphics.push(SymbolGraphic {
            geometry: Geometry::Polygon {
                points: vec![
                    Point::new(-2 * u, -2 * u),
                    Point::new(0, -4 * u),
                    Point::new(2 * u, -2 * u),
                ],
            },
            stroke: Stroke::default(),
            fill: Fill::Solid,
        }),
        PowerStyle::Earth => {
            graphics.push(line((-2 * u, -2 * u), (2 * u, -2 * u)));
            graphics.push(line((-u, -3 * u), (u, -3 * u)));
            graphics.push(line((-u / 3, -4 * u), (u / 3, -4 * u)));
        }
    }
    SymbolDef {
        id: Id::derived(&["power-symbol", net]),
        name: net.to_owned(),
        description: format!("Power port {net}"),
        keywords: "power-flag".to_owned(),
        prefix: "#PWR".to_owned(),
        value: net.to_owned(),
        footprint: None,
        datasheet: String::new(),
        power: true,
        units: vec![SymbolUnit {
            name: String::new(),
            pins: vec![Pin {
                number: "1".to_owned(),
                name: net.to_owned(),
                kind: PinKind::PowerIn,
                shape: PinShape::Line,
                at: Point::ORIGIN,
                direction: if below {
                    PinDirection::Down
                } else {
                    PinDirection::Up
                },
                length: 0,
                hidden: true,
            }],
            graphics,
            texts: Vec::new(),
            alternate: None,
        }],
        properties: BTreeMap::new(),
        pin_text: PinText {
            hide_numbers: true,
            hide_names: true,
            name_offset: 0,
        },
    }
}

/// The library name and definition of every power port style per net.
#[must_use]
pub fn power_symbols(sheets: &[&Schematic]) -> BTreeMap<(String, PowerStyle), (String, SymbolDef)> {
    let mut out: BTreeMap<(String, PowerStyle), (String, SymbolDef)> = BTreeMap::new();
    let mut taken: BTreeMap<String, usize> = BTreeMap::new();
    for sheet in sheets {
        for port in &sheet.power_ports {
            let key = (port.net.clone(), port.style);
            if out.contains_key(&key) {
                continue;
            }
            let n = taken.entry(port.net.clone()).or_insert(0);
            let name = if *n == 0 {
                format!("power:{}", port.net)
            } else {
                format!("power:{}_{}", port.net, *n + 1)
            };
            *n += 1;
            out.insert(key, (name, power_symbol(&port.net, port.style)));
        }
    }
    out
}

/// Everything a sheet file needs from the rest of the project.
pub struct SheetContext<'a> {
    /// The project name (instances belong to a project).
    pub project: &'a str,
    /// Library id (`Lib:Name`) of each symbol name used by placed symbols.
    pub lib_ids: &'a BTreeMap<String, String>,
    /// Symbol definitions by library id.
    pub defs: &'a BTreeMap<String, SymbolDef>,
    /// Power symbols by (net, style).
    pub power: &'a BTreeMap<(String, PowerStyle), (String, SymbolDef)>,
    /// The KiCad instance paths of this sheet (`/root/sheet…`), each with
    /// its page number and the sheet ids after the root.
    pub paths: &'a [(String, String, Vec<Id>)],
    /// The file name of each schematic document.
    pub files: &'a BTreeMap<Id, String>,
    /// The paths and pages of child sheets, by sheet symbol id.
    pub child_pages: &'a BTreeMap<Id, Vec<(String, String)>>,
    /// `true` for the root sheet (it lists the sheet instances).
    pub root: bool,
    /// Power symbol designators by port id.
    pub power_refs: &'a BTreeMap<Id, String>,
}

/// The field placement of a placed symbol: centred text at its position,
/// the angle relative to the symbol (0 or 90).
fn property_sx(
    key: &str,
    value: &str,
    field: Option<&FieldLabel>,
    symbol: &PlacedSymbol,
    frame: Frame,
) -> Sx {
    let shown = field.is_some_and(|f| f.visible) && !value.is_empty();
    let (position, turn, style) = match field {
        Some(field) => (field.at, field.rotation.degrees(), field.style),
        None => (symbol.at, 0.0, TextStyle::default()),
    };
    let relative = (turn - symbol.rotation.degrees()).rem_euclid(180.0);
    let style = TextStyle {
        h_align: HAlign::Center,
        v_align: VAlign::Center,
        ..style
    };
    Sx::list("property")
        .text(key)
        .text(if value.is_empty() && key == "Datasheet" {
            "~"
        } else {
            value
        })
        .child(frame.at(position, Some(relative)))
        .child(effects(&style, !shown, false))
}

fn placed_symbol_sx(
    symbol: &PlacedSymbol,
    lib_id: &str,
    def: Option<&SymbolDef>,
    frame: Frame,
    context: &SheetContext<'_>,
) -> Sx {
    let (rotation, mirror) = if symbol.mirror {
        ((-symbol.rotation.degrees()).rem_euclid(360.0), true)
    } else {
        (symbol.rotation.degrees(), false)
    };
    let mut node = Sx::list("symbol")
        .child(Sx::named("lib_id", lib_id))
        .child(frame.at(symbol.at, Some(rotation)));
    if mirror {
        node = node.child(Sx::pair("mirror", "y"));
    }
    node = node.child(Sx::list("unit").word(symbol.unit.max(1).to_string()));
    if symbol.alternate {
        node = node.child(Sx::list("convert").word("2"));
    }
    node = node
        .child(Sx::flag("exclude_from_sim", false))
        .child(Sx::flag("in_bom", !symbol.flags.exclude_from_bom))
        .child(Sx::flag("on_board", !symbol.flags.exclude_from_board))
        .child(Sx::flag("dnp", symbol.flags.dnp))
        .child(uuid(symbol.id));
    let field = |name: &str| symbol.fields.iter().find(|f| f.name == name);
    node = node
        .child(property_sx(
            "Reference",
            &symbol.designator,
            field("designator"),
            symbol,
            frame,
        ))
        .child(property_sx(
            "Value",
            &symbol.value,
            field("value"),
            symbol,
            frame,
        ))
        .child(property_sx(
            "Footprint",
            symbol.footprint.as_deref().unwrap_or(""),
            field("footprint"),
            symbol,
            frame,
        ));
    let datasheet = symbol
        .properties
        .get("Datasheet")
        .cloned()
        .or_else(|| def.map(|d| d.datasheet.clone()))
        .unwrap_or_default();
    node = node.child(property_sx(
        "Datasheet",
        &datasheet,
        field("Datasheet"),
        symbol,
        frame,
    ));
    for (key, value) in symbol.properties.iter().filter(|(k, _)| *k != "Datasheet") {
        node = node.child(property_sx(key, value, field(key), symbol, frame));
    }
    // The pins of the unit, for KiCad's own bookkeeping.
    if let Some(def) = def {
        let unit = def
            .units
            .get(usize::from(symbol.unit.max(1)) - 1)
            .or_else(|| def.units.first());
        for pin in unit.map(|u| u.pins.as_slice()).unwrap_or_default() {
            node = node.child(
                Sx::named("pin", pin.number.clone()).child(uuid(Id::derived(&[
                    &symbol.id.to_string(),
                    "pin",
                    &pin.number,
                ]))),
            );
        }
    }
    let mut project = Sx::named("project", context.project);
    for (path, _, sheets) in context.paths {
        let key = ew_model::schematic::instance_key(sheets);
        let reference = symbol
            .instances
            .get(&key)
            .cloned()
            .unwrap_or_else(|| symbol.designator.clone());
        project = project.child(
            Sx::named("path", path.clone())
                .child(Sx::named("reference", reference))
                .child(Sx::list("unit").word(symbol.unit.max(1).to_string())),
        );
    }
    node.child(Sx::list("instances").child(project))
}

fn power_port_sx(
    port: &PowerPort,
    lib_id: &str,
    reference: &str,
    frame: Frame,
    context: &SheetContext<'_>,
) -> Sx {
    let mut project = Sx::named("project", context.project);
    for (path, _, _) in context.paths {
        project = project.child(
            Sx::named("path", path.clone())
                .child(Sx::named("reference", reference))
                .child(Sx::list("unit").word("1")),
        );
    }
    let hidden = |key: &str, value: &str| {
        Sx::list("property")
            .text(key)
            .text(value)
            .child(frame.at(port.at, Some(0.0)))
            .child(effects(&TextStyle::default(), true, false))
    };
    let below = matches!(
        port.style,
        PowerStyle::Ground | PowerStyle::SignalGround | PowerStyle::Earth
    );
    let label_at = Point::new(
        port.at.x,
        port.at.y + if below { -3_810_000 } else { 3_810_000 },
    );
    Sx::list("symbol")
        .child(Sx::named("lib_id", lib_id))
        .child(frame.at(port.at, Some(port.rotation.degrees())))
        .child(Sx::list("unit").word("1"))
        .child(Sx::flag("exclude_from_sim", false))
        .child(Sx::flag("in_bom", true))
        .child(Sx::flag("on_board", true))
        .child(Sx::flag("dnp", false))
        .child(uuid(port.id))
        .child(hidden("Reference", reference))
        .child(
            Sx::list("property")
                .text("Value")
                .text(port.net.clone())
                .child(frame.at(label_at, Some(0.0)))
                .child(effects(&TextStyle::default(), false, false)),
        )
        .child(hidden("Footprint", ""))
        .child(hidden("Datasheet", "~"))
        .child(Sx::named("pin", "1").child(uuid(Id::derived(&[&port.id.to_string(), "pin", "1"]))))
        .child(Sx::list("instances").child(project))
}

fn port_shape(shape: PortShape) -> &'static str {
    match shape {
        PortShape::Input => "input",
        PortShape::Output => "output",
        PortShape::Bidirectional => "bidirectional",
        PortShape::TriState => "tri_state",
        PortShape::Passive => "passive",
    }
}

/// A schematic sheet as a `.kicad_sch` file.
#[allow(clippy::too_many_lines, reason = "one item kind after the other")]
#[must_use]
pub fn sheet_file(
    sheet: &Schematic,
    context: &SheetContext<'_>,
    report: &mut ExportReport,
) -> String {
    let (paper_sx, height) = paper(&sheet.page);
    let frame = Frame { height };
    let point = |head: &str, p: Point| frame.point(head, p);
    let mut root = Sx::list("kicad_sch")
        .child(Sx::pair("version", SCH_VERSION))
        .child(Sx::named("generator", "electronicwerk"))
        .child(Sx::named("generator_version", "8.0"))
        .child(uuid(sheet.id))
        .child(paper_sx);
    let tb = &sheet.title_block;
    let mut title = Sx::list("title_block").child(Sx::named("title", tb.title.clone()));
    if !tb.date.is_empty() {
        title = title.child(Sx::named("date", tb.date.clone()));
    }
    if !tb.revision.is_empty() {
        title = title.child(Sx::named("rev", tb.revision.clone()));
    }
    if !tb.company.is_empty() {
        title = title.child(Sx::named("company", tb.company.clone()));
    }
    for (index, comment) in tb.comments.iter().enumerate() {
        title = title.child(
            Sx::list("comment")
                .word((index + 1).to_string())
                .text(comment.clone()),
        );
    }
    root = root.child(title);

    // The symbols this sheet uses, embedded (KiCad's symbol cache).
    let mut used: BTreeMap<String, &SymbolDef> = BTreeMap::new();
    for symbol in &sheet.symbols {
        let lib_id = context
            .lib_ids
            .get(&symbol.symbol)
            .cloned()
            .unwrap_or_else(|| symbol.symbol.clone());
        match context.defs.get(&lib_id) {
            Some(def) => {
                used.insert(lib_id, def);
            }
            None => report.note(
                "symbol",
                format!(
                    "the symbol `{}` of {} is not in the project libraries",
                    symbol.symbol, symbol.designator
                ),
            ),
        }
    }
    for port in &sheet.power_ports {
        if let Some((name, def)) = context.power.get(&(port.net.clone(), port.style)) {
            used.insert(name.clone(), def);
        }
    }
    root = root.child(
        Sx::list("lib_symbols").children(used.iter().map(|(name, def)| symbol_sx(def, name))),
    );

    for junction in &sheet.junctions {
        root = root.child(
            Sx::list("junction")
                .child(frame.at(junction.at, None))
                .child(Sx::list("diameter").num(0.0))
                .child(Sx::list("color").num(0.0).num(0.0).num(0.0).num(0.0))
                .child(uuid(junction.id)),
        );
    }
    for nc in &sheet.no_connects {
        root = root.child(
            Sx::list("no_connect")
                .child(frame.at(nc.at, None))
                .child(uuid(nc.id)),
        );
    }
    for entry in &sheet.bus_entries {
        root = root.child(
            Sx::list("bus_entry")
                .child(frame.at(entry.at, None))
                .child(Sx::list("size").mm(entry.size.x).mm(-entry.size.y))
                .child(stroke_sx(&Stroke::default()))
                .child(uuid(entry.id)),
        );
    }
    for (head, segments) in [
        (
            "wire",
            sheet
                .wires
                .iter()
                .map(|w| (w.id, w.start, w.end))
                .collect::<Vec<_>>(),
        ),
        (
            "bus",
            sheet.buses.iter().map(|b| (b.id, b.start, b.end)).collect(),
        ),
    ] {
        for (id, start, end) in segments {
            root = root.child(
                Sx::list(head)
                    .child(Sx::list("pts").child(frame.xy(start)).child(frame.xy(end)))
                    .child(stroke_sx(&Stroke::default()))
                    .child(uuid(id)),
            );
        }
    }
    for shape in &sheet.shapes {
        root = root.child(
            shape_sx(&shape.geometry, &shape.stroke, shape.fill, &point).child(uuid(shape.id)),
        );
    }
    for text in &sheet.texts {
        root = root.child(
            Sx::list("text")
                .text(text.text.clone())
                .child(Sx::flag("exclude_from_sim", false))
                .child(frame.at(text.at, Some(text.rotation.degrees())))
                .child(effects(&text.style, false, false))
                .child(uuid(text.id)),
        );
    }
    for label in &sheet.labels {
        let head = match label.kind {
            LabelKind::Local => "label",
            LabelKind::Global => "global_label",
            LabelKind::Hierarchical => "hierarchical_label",
        };
        let mut node = Sx::list(head).text(label.text.clone());
        if label.kind != LabelKind::Local {
            node = node.child(Sx::pair("shape", port_shape(label.shape)));
        }
        node = node
            .child(frame.at(label.at, Some(label.rotation.degrees())))
            .child(Sx::flag("fields_autoplaced", true))
            .child(effects(&label.style, false, false))
            .child(uuid(label.id));
        if label.kind == LabelKind::Global {
            node = node.child(
                Sx::list("property")
                    .text("Intersheetrefs")
                    .text("${INTERSHEET_REFS}")
                    .child(frame.at(label.at, Some(0.0)))
                    .child(effects(&TextStyle::default(), true, false)),
            );
        }
        root = root.child(node);
    }
    for symbol in &sheet.symbols {
        let lib_id = context
            .lib_ids
            .get(&symbol.symbol)
            .cloned()
            .unwrap_or_else(|| symbol.symbol.clone());
        let def = context.defs.get(&lib_id);
        root = root.child(placed_symbol_sx(symbol, &lib_id, def, frame, context));
    }
    for port in &sheet.power_ports {
        let Some((lib_id, _)) = context.power.get(&(port.net.clone(), port.style)) else {
            continue;
        };
        let reference = context
            .power_refs
            .get(&port.id)
            .cloned()
            .unwrap_or_else(|| "#PWR?".to_owned());
        root = root.child(power_port_sx(port, lib_id, &reference, frame, context));
    }
    for child in &sheet.sheets {
        let (kx, ky) = frame.kicad(child.at);
        let file = context
            .files
            .get(&child.schematic)
            .cloned()
            .unwrap_or_else(|| format!("{}.kicad_sch", child.name));
        let mut node = Sx::list("sheet")
            .child(at(kx, ky, None))
            .child(Sx::list("size").mm(child.size.w).mm(child.size.h))
            .child(Sx::flag("fields_autoplaced", true))
            .child(
                Sx::list("stroke")
                    .child(Sx::list("width").num(0.1524))
                    .child(Sx::pair("type", "solid")),
            )
            .child(Sx::list("fill").child(Sx::list("color").num(0.0).num(0.0).num(0.0).num(0.0)))
            .child(uuid(child.id))
            .child(
                Sx::list("property")
                    .text("Sheetname")
                    .text(child.name.clone())
                    .child(at(kx, ky - 700_000, Some(0.0)))
                    .child(effects(
                        &TextStyle {
                            h_align: HAlign::Left,
                            v_align: VAlign::Bottom,
                            ..TextStyle::default()
                        },
                        false,
                        false,
                    )),
            )
            .child(
                Sx::list("property")
                    .text("Sheetfile")
                    .text(file)
                    .child(at(kx, ky + child.size.h + 600_000, Some(0.0)))
                    .child(effects(
                        &TextStyle {
                            h_align: HAlign::Left,
                            v_align: VAlign::Top,
                            ..TextStyle::default()
                        },
                        false,
                        false,
                    )),
            );
        for pin in &child.pins {
            let (px, py, angle) = match pin.side {
                SheetSide::Left => (kx, ky + pin.offset, 180.0),
                SheetSide::Right => (kx + child.size.w, ky + pin.offset, 0.0),
                SheetSide::Top => (kx + pin.offset, ky, 90.0),
                SheetSide::Bottom => (kx + pin.offset, ky + child.size.h, 270.0),
            };
            node = node.child(
                Sx::list("pin")
                    .text(pin.name.clone())
                    .word(port_shape(pin.shape))
                    .child(at(px, py, Some(angle)))
                    .child(effects(&TextStyle::default(), false, false))
                    .child(uuid(pin.id)),
            );
        }
        let mut project = Sx::named("project", context.project);
        for (path, page) in context.child_pages.get(&child.id).into_iter().flatten() {
            project = project
                .child(Sx::named("path", path.clone()).child(Sx::named("page", page.clone())));
        }
        root = root.child(node.child(Sx::list("instances").child(project)));
    }
    if context.root {
        root = root.child(
            Sx::list("sheet_instances").child(Sx::named("path", "/").child(Sx::named("page", "1"))),
        );
    }
    root.render()
}
