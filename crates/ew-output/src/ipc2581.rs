//! IPC-2581 (revision C): the whole board in one XML file — stack-up,
//! outline, packages, components, nets, the copper, mask, paste and
//! silkscreen features of every layer, the drill holes and the bill of
//! materials. Board makers and assembly houses read it instead of Gerber,
//! drill and placement files.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use ew_model::layer::{CopperLayer, Layer, Side};
use ew_model::pcb::{FootprintInstance, Mounting, Pad, PadKind, PadShape, Pcb, StackupLayer};
use ew_pcb::geom::{self, V, Xform};
use ew_pcb::items::{footprint_xform, pad_shape};
use ew_schematic::connectivity::natural_cmp;

use crate::art::{BoardArt, Feature};
use crate::bom::escape_xml;
use crate::drill::{HoleKind, holes};
use crate::gerber::fabrication_layers;

fn mm(nm: f64) -> String {
    let text = format!("{:.6}", nm / 1e6);
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if text == "-0" || text.is_empty() {
        "0".to_owned()
    } else {
        text.to_owned()
    }
}

fn degrees(value: f64) -> String {
    let v = value.rem_euclid(360.0);
    let text = format!("{v:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// The IPC-2581 name and function of a board layer.
fn layer_info(layer: Layer) -> (&'static str, &'static str) {
    let side = |s: Side| if s == Side::Top { "TOP" } else { "BOTTOM" };
    match layer {
        Layer::Copper(CopperLayer::Top) => ("CONDUCTOR", "TOP"),
        Layer::Copper(CopperLayer::Bottom) => ("CONDUCTOR", "BOTTOM"),
        Layer::Copper(CopperLayer::Inner(_)) => ("CONDUCTOR", "INTERNAL"),
        Layer::SolderMask(s) => ("SOLDERMASK", side(s)),
        Layer::Silkscreen(s) => ("SILKSCREEN", side(s)),
        Layer::Paste(s) => ("SOLDERPASTE", side(s)),
        Layer::Adhesive(s) => ("GLUE", side(s)),
        Layer::Fabrication(s) => ("ASSEMBLY", side(s)),
        Layer::Courtyard(s) => ("COURTYARD", side(s)),
        Layer::Outline => ("BOARD_OUTLINE", "ALL"),
        Layer::Mechanical(_) | Layer::User(_) => ("DOCUMENT", "ALL"),
    }
}

/// Dictionaries of line widths and pad primitives.
#[derive(Default)]
struct Dictionaries {
    lines: BTreeMap<i64, String>,
    standard: BTreeMap<String, String>,
    user: BTreeMap<String, String>,
}

impl Dictionaries {
    fn line(&mut self, width: f64) -> String {
        let key = width.round() as i64;
        let next = self.lines.len() + 1;
        self.lines
            .entry(key)
            .or_insert_with(|| format!("LINE_{next}"))
            .clone()
    }

    /// The primitive id of a pad shape (in pad-local coordinates).
    fn pad(&mut self, pad: &Pad) -> (String, bool) {
        let (w, h) = (pad.size.w as f64, pad.size.h as f64);
        let body = match &pad.shape {
            PadShape::Circle => Some(format!("<Circle diameter=\"{}\"/>", mm(w))),
            PadShape::Rect => Some(format!(
                "<RectCenter width=\"{}\" height=\"{}\"/>",
                mm(w),
                mm(h)
            )),
            PadShape::Oval => Some(format!("<Oval width=\"{}\" height=\"{}\"/>", mm(w), mm(h))),
            PadShape::RoundRect { ratio } => {
                let radius = w.min(h) * f64::from(*ratio) / 1000.0;
                Some(format!(
                    "<RectRound width=\"{}\" height=\"{}\" radius=\"{}\" upperRight=\"true\" upperLeft=\"true\" lowerRight=\"true\" lowerLeft=\"true\"/>",
                    mm(w),
                    mm(h),
                    mm(radius)
                ))
            }
            PadShape::Trapezoid { .. } | PadShape::Custom { .. } => None,
        };
        if let Some(body) = body {
            let next = self.standard.len() + 1;
            let id = self
                .standard
                .entry(body)
                .or_insert_with(|| format!("SHAPE_{next}"))
                .clone();
            return (id, true);
        }
        // Trapezoids and custom pads: the outline of the pad at the origin.
        let local = Pad {
            at: ew_model::units::Point::ORIGIN,
            rotation: ew_model::units::Angle::ZERO,
            ..pad.clone()
        };
        let shape = pad_shape(&local, &Xform::IDENTITY, 0.0);
        let mut body = String::from("<UserSpecial>");
        for polygon in shape.outlines(0.0, 1_000.0) {
            body.push_str("<Contour>");
            for (i, ring) in polygon.iter().enumerate() {
                body.push_str(if i == 0 { "<Polygon>" } else { "<Cutout>" });
                polygon_steps(&mut body, ring, V::new(0.0, 0.0));
                body.push_str(if i == 0 { "</Polygon>" } else { "</Cutout>" });
            }
            body.push_str("</Contour>");
        }
        body.push_str("</UserSpecial>");
        let next = self.user.len() + 1;
        let id = self
            .user
            .entry(body)
            .or_insert_with(|| format!("USER_{next}"))
            .clone();
        (id, false)
    }
}

/// `PolyBegin` and `PolyStepSegment`s of a closed ring.
fn polygon_steps(out: &mut String, ring: &[V], offset: V) {
    let Some(first) = ring.first() else {
        return;
    };
    let p = first.sub(offset);
    let _ = write!(out, "<PolyBegin x=\"{}\" y=\"{}\"/>", mm(p.x), mm(p.y));
    for q in ring[1..].iter().chain(std::iter::once(first)) {
        let q = q.sub(offset);
        let _ = write!(
            out,
            "<PolyStepSegment x=\"{}\" y=\"{}\"/>",
            mm(q.x),
            mm(q.y)
        );
    }
}

fn primitive_ref(id: &str, standard: bool) -> String {
    if standard {
        format!("<StandardPrimitiveRef id=\"{id}\"/>")
    } else {
        format!("<UserPrimitiveRef id=\"{id}\"/>")
    }
}

/// The package name of a footprint (its library name).
fn package_name(fp: &FootprintInstance) -> String {
    if fp.footprint.is_empty() {
        format!("PKG_{}", fp.designator)
    } else {
        fp.footprint.clone()
    }
}

/// Context of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    /// Application version.
    pub version: String,
    /// Creation time (ISO 8601).
    pub created: String,
}

/// Writes the IPC-2581 file of a board.
#[must_use]
#[allow(clippy::too_many_lines, reason = "one section after the other")]
pub fn write(art: &BoardArt<'_>, context: &Context) -> String {
    let pcb: &Pcb = art.pcb();
    let step = escape_xml(&pcb.name);
    let mut dict = Dictionaries::default();
    let copper_count = pcb.stackup.copper_count();
    let net_names: BTreeMap<_, _> = pcb.nets.iter().map(|n| (n.id, n.name.clone())).collect();
    let layers: Vec<Layer> = fabrication_layers(pcb);
    let side_layer = |side: Side| {
        if side == Side::Top {
            Layer::TOP_COPPER.to_string()
        } else {
            Layer::BOTTOM_COPPER.to_string()
        }
    };

    // Packages: one per footprint name, from the first footprint using it.
    let mut packages: BTreeMap<String, String> = BTreeMap::new();
    for fp in &pcb.footprints {
        let name = package_name(fp);
        if packages.contains_key(&name) {
            continue;
        }
        let mut body = String::new();
        // Outline: the box around the pads (in footprint coordinates).
        let mut lo = V::new(f64::MAX, f64::MAX);
        let mut hi = V::new(f64::MIN, f64::MIN);
        for pad in &fp.pads {
            let shape = pad_shape(pad, &Xform::IDENTITY, 0.0);
            let b = shape.bbox();
            lo = V::new(lo.x.min(b.min.x), lo.y.min(b.min.y));
            hi = V::new(hi.x.max(b.max.x), hi.y.max(b.max.y));
        }
        if lo.x > hi.x {
            lo = V::new(-500_000.0, -500_000.0);
            hi = V::new(500_000.0, 500_000.0);
        }
        let outline_line = dict.line(100_000.0);
        let _ = write!(
            body,
            "<Package name=\"{}\" type=\"OTHER\" pinOne=\"{}\"><Outline><Polygon>",
            escape_xml(&name),
            escape_xml(fp.pads.first().map_or("1", |p| p.number.as_str()))
        );
        polygon_steps(
            &mut body,
            &[lo, V::new(hi.x, lo.y), hi, V::new(lo.x, hi.y)],
            V::new(0.0, 0.0),
        );
        let _ = write!(
            body,
            "</Polygon><LineDescRef id=\"{outline_line}\"/></Outline>"
        );
        for pad in &fp.pads {
            let (id, standard) = dict.pad(pad);
            let kind = if pad.drill.is_some() {
                "THRU"
            } else {
                "SURFACE"
            };
            let _ = write!(
                body,
                "<Pin number=\"{}\" type=\"{kind}\" electricalType=\"{}\">",
                escape_xml(&pad.number),
                if pad.kind == PadKind::NpThroughHole {
                    "MECHANICAL"
                } else {
                    "ELECTRICAL"
                }
            );
            if pad.rotation.degrees() != 0.0 {
                let _ = write!(
                    body,
                    "<Xform rotation=\"{}\"/>",
                    degrees(pad.rotation.degrees())
                );
            }
            let _ = write!(
                body,
                "<Location x=\"{}\" y=\"{}\"/>{}</Pin>",
                mm(pad.at.x as f64),
                mm(pad.at.y as f64),
                primitive_ref(&id, standard)
            );
        }
        body.push_str("</Package>");
        packages.insert(name, body);
    }

    // Components and logical nets.
    let mut components = String::new();
    let mut nets: BTreeMap<String, Vec<(String, String)>> = BTreeMap::new();
    let mut footprints: Vec<&FootprintInstance> = pcb.footprints.iter().collect();
    footprints.sort_by(|a, b| natural_cmp(&a.designator, &b.designator));
    for fp in &footprints {
        let mount = match fp.attributes.mounting {
            Mounting::Smd => "SMT",
            Mounting::ThroughHole => "THMT",
            Mounting::Other => "OTHER",
        };
        let _ = write!(
            components,
            "<Component refDes=\"{}\" packageRef=\"{}\" part=\"{}\" layerRef=\"{}\" mountType=\"{mount}\">",
            escape_xml(&fp.designator),
            escape_xml(&package_name(fp)),
            escape_xml(format!("{} {}", fp.footprint, fp.value).trim()),
            side_layer(fp.side)
        );
        let rotation = fp.rotation.degrees();
        if rotation != 0.0 || fp.side == Side::Bottom {
            let _ = write!(components, "<Xform rotation=\"{}\"", degrees(rotation));
            if fp.side == Side::Bottom {
                components.push_str(" mirror=\"true\"");
            }
            components.push_str("/>");
        }
        let _ = write!(
            components,
            "<Location x=\"{}\" y=\"{}\"/></Component>",
            mm(fp.at.x as f64),
            mm(fp.at.y as f64)
        );
        for pad in &fp.pads {
            if let Some(name) = pad.net.and_then(|n| net_names.get(&n)) {
                nets.entry(name.clone())
                    .or_default()
                    .push((fp.designator.clone(), pad.number.clone()));
            }
        }
    }

    // Layer features.
    let mut features = String::new();
    for layer in &layers {
        let list = art.features(*layer);
        if list.is_empty() {
            continue;
        }
        let _ = write!(features, "<LayerFeature layerRef=\"{layer}\">");
        let mut sets: BTreeMap<String, String> = BTreeMap::new();
        for feature in &list {
            let net = feature
                .attributes()
                .net
                .clone()
                .filter(|n| !n.is_empty() && n != "N/C")
                .unwrap_or_default();
            let entry = sets.entry(net).or_default();
            match feature {
                Feature::Flash {
                    at,
                    shape,
                    attributes,
                    ..
                } => {
                    // Pads by their primitive; other flashes as contours.
                    let pin = attributes.pin.clone();
                    let mut written = false;
                    if let Some((designator, number)) = &pin
                        && let Some(fp) =
                            pcb.footprints.iter().find(|f| &f.designator == designator)
                        && let Some(pad) = fp.pads.iter().find(|p| &p.number == number)
                        && layer.is_copper()
                    {
                        let (id, standard) = dict.pad(pad);
                        let xf = footprint_xform(fp);
                        let rotation = xf.angle_of(geom::radians(pad.rotation)).to_degrees();
                        let _ = write!(
                            entry,
                            "<Pad><Xform rotation=\"{}\"{}/><Location x=\"{}\" y=\"{}\"/>{}<PinRef componentRef=\"{}\" pin=\"{}\"/></Pad>",
                            degrees(rotation),
                            if xf.mirror { " mirror=\"true\"" } else { "" },
                            mm(at.x),
                            mm(at.y),
                            primitive_ref(&id, standard),
                            escape_xml(designator),
                            escape_xml(number)
                        );
                        written = true;
                    }
                    if !written {
                        entry.push_str("<Features><Contour>");
                        for (i, ring) in shape
                            .outlines(0.0, 1_000.0)
                            .into_iter()
                            .flatten()
                            .enumerate()
                        {
                            entry.push_str(if i == 0 { "<Polygon>" } else { "<Cutout>" });
                            polygon_steps(entry, &ring, V::new(0.0, 0.0));
                            entry.push_str(if i == 0 { "</Polygon>" } else { "</Cutout>" });
                        }
                        entry.push_str("</Contour></Features>");
                    }
                }
                Feature::Line { a, b, width, .. } => {
                    let id = dict.line(*width);
                    let _ = write!(
                        entry,
                        "<Features><Line startX=\"{}\" startY=\"{}\" endX=\"{}\" endY=\"{}\"><LineDescRef id=\"{id}\"/></Line></Features>",
                        mm(a.x),
                        mm(a.y),
                        mm(b.x),
                        mm(b.y)
                    );
                }
                Feature::Arc {
                    start,
                    end,
                    center,
                    ccw,
                    width,
                    ..
                } => {
                    let id = dict.line(*width);
                    let _ = write!(
                        entry,
                        "<Features><Arc startX=\"{}\" startY=\"{}\" endX=\"{}\" endY=\"{}\" centerX=\"{}\" centerY=\"{}\" clockwise=\"{}\"><LineDescRef id=\"{id}\"/></Arc></Features>",
                        mm(start.x),
                        mm(start.y),
                        mm(end.x),
                        mm(end.y),
                        mm(center.x),
                        mm(center.y),
                        !ccw
                    );
                }
                Feature::Region { outline, holes, .. } => {
                    entry.push_str("<Features><Contour><Polygon>");
                    polygon_steps(entry, outline, V::new(0.0, 0.0));
                    entry.push_str("</Polygon>");
                    for hole in holes {
                        entry.push_str("<Cutout>");
                        polygon_steps(entry, hole, V::new(0.0, 0.0));
                        entry.push_str("</Cutout>");
                    }
                    entry.push_str("</Contour></Features>");
                }
            }
        }
        for (net, body) in sets {
            if net.is_empty() {
                let _ = write!(features, "<Set>{body}</Set>");
            } else {
                let _ = write!(features, "<Set net=\"{}\">{body}</Set>", escape_xml(&net));
            }
        }
        features.push_str("</LayerFeature>");
    }
    // Drill layers: through holes and one layer per blind or buried span.
    let all = holes(pcb);
    let last = usize::from(copper_count.max(1)) - 1;
    let mut spans: BTreeMap<(usize, usize), Vec<&crate::drill::Hole>> = BTreeMap::new();
    for hole in &all {
        spans.entry(hole.span).or_default().push(hole);
    }
    let stack: Vec<CopperLayer> = pcb.stackup.copper_layers().collect();
    let mut drill_layers = String::new();
    for ((a, b), list) in &spans {
        let name = format!("DRILL_{}_{}", a + 1, b + 1);
        let (from, to) = (
            stack.get(*a).copied().unwrap_or(CopperLayer::Top),
            stack.get(*b).copied().unwrap_or(CopperLayer::Bottom),
        );
        let _ = write!(
            drill_layers,
            "<Layer name=\"{name}\" layerFunction=\"DRILL\" side=\"{}\" polarity=\"POSITIVE\"><Span fromLayer=\"{from}\" toLayer=\"{to}\"/></Layer>",
            if *a == 0 && *b == last {
                "ALL"
            } else {
                "INTERNAL"
            }
        );
        let _ = write!(features, "<LayerFeature layerRef=\"{name}\">");
        for (i, hole) in list.iter().enumerate() {
            let status = match hole.kind {
                HoleKind::Via => "VIA",
                HoleKind::Component => "PLATED",
                HoleKind::Mechanical => "NONPLATED",
            };
            let at = hole.end.map_or(hole.start, |end| {
                V::new(
                    f64::midpoint(hole.start.x, end.x),
                    f64::midpoint(hole.start.y, end.y),
                )
            });
            let _ = write!(
                features,
                "<Set><Hole name=\"H{}\" diameter=\"{}\" platingStatus=\"{status}\" plusTol=\"0\" minusTol=\"0\" x=\"{}\" y=\"{}\"/></Set>",
                i + 1,
                mm(hole.diameter),
                mm(at.x),
                mm(at.y)
            );
        }
        features.push_str("</LayerFeature>");
    }

    // The file.
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <IPC-2581 revision=\"C\" xmlns=\"http://webstds.ipc.org/2581\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n",
    );
    let _ = write!(
        out,
        "<Content roleRef=\"Owner\"><FunctionMode mode=\"ASSEMBLY\"/><StepRef name=\"{step}\"/>"
    );
    for layer in &layers {
        let _ = write!(out, "<LayerRef name=\"{layer}\"/>");
    }
    for (a, b) in spans.keys() {
        let _ = write!(out, "<LayerRef name=\"DRILL_{}_{}\"/>", a + 1, b + 1);
    }
    let _ = write!(out, "<BomRef name=\"{step}_BOM\"/>");
    out.push_str("<DictionaryLineDesc units=\"MILLIMETER\">");
    for (width, id) in &dict.lines {
        let _ = write!(
            out,
            "<EntryLineDesc id=\"{id}\"><LineDesc lineWidth=\"{}\" lineEnd=\"ROUND\"/></EntryLineDesc>",
            mm(*width as f64)
        );
    }
    out.push_str("</DictionaryLineDesc><DictionaryStandard units=\"MILLIMETER\">");
    for (body, id) in &dict.standard {
        let _ = write!(out, "<EntryStandard id=\"{id}\">{body}</EntryStandard>");
    }
    out.push_str("</DictionaryStandard><DictionaryUser units=\"MILLIMETER\">");
    for (body, id) in &dict.user {
        let _ = write!(out, "<EntryUser id=\"{id}\">{body}</EntryUser>");
    }
    out.push_str("</DictionaryUser></Content>\n");
    out.push_str(
        "<LogisticHeader><Role id=\"Owner\" roleFunction=\"SENDER\"/>\
         <Enterprise id=\"UNKNOWN\" code=\"NONE\"/>\
         <Person name=\"UNKNOWN\" enterpriseRef=\"UNKNOWN\" roleRef=\"Owner\"/></LogisticHeader>\n",
    );
    let _ = writeln!(
        out,
        "<HistoryRecord number=\"1\" origination=\"{0}\" software=\"ElectronicWerk\" lastChange=\"{0}\">\
         <FileRevision fileRevisionId=\"1\" comment=\"\" label=\"\">\
         <SoftwarePackage name=\"ElectronicWerk\" revision=\"{1}\" vendor=\"ElectronicWerk\">\
         <Certification certificationStatus=\"SELFTEST\"/></SoftwarePackage></FileRevision></HistoryRecord>",
        escape_xml(&context.created),
        escape_xml(&context.version)
    );
    // Bill of materials: equal footprint and value share an item.
    let _ = write!(
        out,
        "<Bom name=\"{step}_BOM\"><BomHeader assembly=\"{step}\" revision=\"1\"><StepRef name=\"{step}\"/></BomHeader>"
    );
    let mut items: BTreeMap<(String, String), Vec<&FootprintInstance>> = BTreeMap::new();
    for fp in footprints.iter().filter(|f| !f.attributes.exclude_from_bom) {
        items
            .entry((fp.footprint.clone(), fp.value.clone()))
            .or_default()
            .push(fp);
    }
    for ((footprint, value), list) in &items {
        let _ = write!(
            out,
            "<BomItem OEMDesignNumberRef=\"{}\" quantity=\"{}\" pinCount=\"{}\" category=\"ELECTRICAL\">",
            escape_xml(format!("{footprint} {value}").trim()),
            list.len(),
            list[0].pads.len()
        );
        for fp in list {
            let _ = write!(
                out,
                "<RefDes name=\"{}\" packageRef=\"{}\" populate=\"{}\" layerRef=\"{}\"/>",
                escape_xml(&fp.designator),
                escape_xml(&package_name(fp)),
                !fp.attributes.dnp,
                side_layer(fp.side)
            );
        }
        let _ = write!(
            out,
            "<Characteristics category=\"ELECTRICAL\"><Textual definitionSource=\"ElectronicWerk\" textualCharacteristicName=\"Value\" textualCharacteristicValue=\"{}\"/></Characteristics></BomItem>",
            escape_xml(value)
        );
    }
    out.push_str("</Bom>\n");
    // CAD data.
    out.push_str("<Ecad name=\"Design\"><CadHeader units=\"MILLIMETER\"/><CadData>");
    for layer in &layers {
        let (function, side) = layer_info(*layer);
        let _ = write!(
            out,
            "<Layer name=\"{layer}\" layerFunction=\"{function}\" side=\"{side}\" polarity=\"POSITIVE\"/>"
        );
    }
    out.push_str(&drill_layers);
    let thickness = mm(pcb.stackup.thickness() as f64);
    let _ = write!(
        out,
        "<Stackup name=\"PRIMARY\" overallThickness=\"{thickness}\" tolPlus=\"0\" tolMinus=\"0\" whereMeasured=\"METAL\">\
         <StackupGroup name=\"PRIMARY_GROUP\" thickness=\"{thickness}\" tolPlus=\"0\" tolMinus=\"0\">"
    );
    for (sequence, layer) in pcb.stackup.layers.iter().enumerate() {
        let name = match layer {
            StackupLayer::Copper { layer, .. } => layer.to_string(),
            StackupLayer::Silkscreen { side, .. } => Layer::Silkscreen(*side).to_string(),
            StackupLayer::SolderMask { side, .. } => Layer::SolderMask(*side).to_string(),
            StackupLayer::Dielectric { .. } => format!("DIELECTRIC_{sequence}"),
        };
        let _ = write!(
            out,
            "<StackupLayer layerOrGroupRef=\"{name}\" thickness=\"{}\" tolPlus=\"0\" tolMinus=\"0\" sequence=\"{sequence}\"/>",
            mm(layer.thickness() as f64)
        );
    }
    out.push_str("</StackupGroup></Stackup>");
    for (sequence, layer) in pcb.stackup.layers.iter().enumerate() {
        if matches!(layer, StackupLayer::Dielectric { .. }) {
            let _ = write!(
                out,
                "<Layer name=\"DIELECTRIC_{sequence}\" layerFunction=\"DIELCORE\" side=\"INTERNAL\" polarity=\"POSITIVE\"/>"
            );
        }
    }
    let _ = write!(out, "<Step name=\"{step}\"><Datum x=\"0\" y=\"0\"/>");
    let outline = ew_pcb::items::outline(pcb);
    if let Some(board) = outline.rings.first() {
        out.push_str("<Profile><Polygon>");
        polygon_steps(&mut out, board, V::new(0.0, 0.0));
        out.push_str("</Polygon>");
        for cutout in &outline.rings[1..] {
            out.push_str("<Cutout>");
            polygon_steps(&mut out, cutout, V::new(0.0, 0.0));
            out.push_str("</Cutout>");
        }
        out.push_str("</Profile>");
    }
    for body in packages.values() {
        out.push_str(body);
    }
    out.push_str(&components);
    for (name, pins) in &nets {
        let _ = write!(out, "<LogicalNet name=\"{}\">", escape_xml(name));
        for (designator, pin) in pins {
            let _ = write!(
                out,
                "<PinRef componentRef=\"{}\" pin=\"{}\"/>",
                escape_xml(designator),
                escape_xml(pin)
            );
        }
        out.push_str("</LogicalNet>");
    }
    out.push_str(&features);
    out.push_str("</Step></CadData></Ecad>\n</IPC-2581>\n");
    out
}
