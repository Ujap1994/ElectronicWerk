//! Board scenes.

use std::collections::{BTreeSet, HashMap};

use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer, LayerSpec, Side};
use ew_model::pcb::{FootprintInstance, FootprintTextKind, Pad, PadShape, Pcb, StackupLayer};
use ew_model::units::{Point, Rect};

use crate::Options;
use crate::builder::{Header, LayerDef, MESH_HATCH, NONE, ObjType, Scene, SceneBuilder};
use crate::draw::{self, Paint};
use crate::geom::{self, P, Xform};
use crate::text::{self, TextItem};

/// Key of the drill-hole layer.
pub const HOLES: &str = "holes";

/// Panel order of a board layer.
fn panel_rank(layer: Layer, stack: &[CopperLayer]) -> (u8, u32) {
    let side = |side: Side| u32::from(side == Side::Bottom);
    match layer {
        Layer::Copper(copper) => (
            0,
            stack
                .iter()
                .position(|own| *own == copper)
                .and_then(|i| u32::try_from(i).ok())
                .unwrap_or(u32::MAX),
        ),
        Layer::Silkscreen(s) => (1, side(s)),
        Layer::SolderMask(s) => (2, side(s)),
        Layer::Paste(s) => (3, side(s)),
        Layer::Adhesive(s) => (4, side(s)),
        Layer::Fabrication(s) => (5, side(s)),
        Layer::Courtyard(s) => (6, side(s)),
        Layer::Outline => (7, 0),
        Layer::Mechanical(n) => (8, u32::from(n)),
        Layer::User(n) => (9, u32::from(n)),
    }
}

/// Group name, draw order in the top view and default visibility.
fn layer_style(layer: Layer) -> (&'static str, i32, bool) {
    let sided = |side: Side, top: i32, bottom: i32| if side == Side::Top { top } else { bottom };
    match layer {
        Layer::Copper(copper) => {
            let z = match copper {
                CopperLayer::Top => 300,
                CopperLayer::Bottom => 100,
                CopperLayer::Inner(n) => 200 - i32::from(n),
            };
            ("copper", z, true)
        }
        Layer::Silkscreen(s) => ("silkscreen", sided(s, 404, 6), true),
        Layer::SolderMask(s) => ("solderMask", sided(s, 401, 3), false),
        Layer::Paste(s) => ("paste", sided(s, 402, 2), false),
        Layer::Adhesive(s) => ("adhesive", sided(s, 403, 1), false),
        Layer::Fabrication(s) => ("fabrication", sided(s, 405, 5), true),
        Layer::Courtyard(s) => ("courtyard", sided(s, 406, 4), true),
        Layer::Outline => ("outline", 500, true),
        Layer::Mechanical(n) => ("mechanical", 600 + i32::from(n), true),
        Layer::User(n) => ("user", 700 + i32::from(n), true),
    }
}

struct BoardLayers {
    map: HashMap<Layer, usize>,
    stack: Vec<CopperLayer>,
    holes: usize,
}

impl BoardLayers {
    fn new(b: &mut SceneBuilder, pcb: &Pcb) -> Self {
        let stack: Vec<CopperLayer> = pcb.stackup.copper_layers().collect();
        let names: HashMap<CopperLayer, String> = pcb
            .stackup
            .layers
            .iter()
            .filter_map(|layer| match layer {
                StackupLayer::Copper { layer, name, .. } if !name.is_empty() => {
                    Some((*layer, name.clone()))
                }
                _ => None,
            })
            .collect();
        let mut used: BTreeSet<Layer> = BTreeSet::new();
        used.extend(stack.iter().map(|copper| Layer::Copper(*copper)));
        for side in [Side::Top, Side::Bottom] {
            used.extend([
                Layer::Silkscreen(side),
                Layer::SolderMask(side),
                Layer::Paste(side),
                Layer::Fabrication(side),
                Layer::Courtyard(side),
            ]);
        }
        used.insert(Layer::Outline);
        let copper_count = pcb.stackup.copper_count();
        let mut note = |layer: Layer| {
            if !layer.is_copper() {
                used.insert(layer);
            }
        };
        for shape in &pcb.shapes {
            note(shape.layer);
        }
        for text in &pcb.texts {
            note(text.layer);
        }
        for fp in &pcb.footprints {
            let flip = |layer: Layer| {
                if fp.side == Side::Bottom {
                    layer.flipped(copper_count)
                } else {
                    layer
                }
            };
            for graphic in &fp.graphics {
                note(flip(graphic.layer));
            }
            for text in &fp.texts {
                note(flip(text.layer));
            }
        }
        let mut ordered: Vec<Layer> = used.into_iter().collect();
        ordered.sort_by_key(|layer| panel_rank(*layer, &stack));
        let mut map = HashMap::new();
        for layer in ordered {
            let (group, z, visible) = layer_style(layer);
            let name = match layer {
                Layer::Copper(copper) => names.get(&copper).cloned(),
                _ => None,
            };
            let index = b.layer(LayerDef {
                key: layer.to_string(),
                group,
                side: layer.side().map(Side::as_str),
                name,
                z,
                visible,
            });
            map.insert(layer, index);
        }
        let holes = b.layer(LayerDef {
            key: HOLES.to_owned(),
            group: "holes",
            side: None,
            name: None,
            z: 1000,
            visible: true,
        });
        Self { map, stack, holes }
    }

    fn get(&self, layer: Layer) -> Option<usize> {
        self.map.get(&layer).copied()
    }

    fn copper(&self, copper: CopperLayer) -> Option<usize> {
        self.get(Layer::Copper(copper))
    }

    /// Copper layers from `from` to `to` (inclusive, in stack order).
    fn copper_span(&self, from: CopperLayer, to: CopperLayer) -> Vec<usize> {
        let index = |layer: CopperLayer| self.stack.iter().position(|own| *own == layer);
        let (Some(a), Some(b)) = (index(from), index(to)) else {
            return Vec::new();
        };
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        self.stack[a..=b]
            .iter()
            .filter_map(|copper| self.copper(*copper))
            .collect()
    }

    /// Scene layers selected by a pad layer spec.
    fn pad_layers(&self, spec: LayerSpec, flip: &dyn Fn(Layer) -> Layer) -> Vec<(usize, Layer)> {
        let pick = |layers: Vec<Layer>| -> Vec<(usize, Layer)> {
            layers
                .into_iter()
                .filter_map(|layer| self.get(layer).map(|index| (index, layer)))
                .collect()
        };
        match spec {
            LayerSpec::Layer(layer) => pick(vec![flip(layer)]),
            LayerSpec::AllCopper => pick(self.stack.iter().map(|c| Layer::Copper(*c)).collect()),
            LayerSpec::AllMask => pick(vec![
                Layer::SolderMask(Side::Top),
                Layer::SolderMask(Side::Bottom),
            ]),
            LayerSpec::AllPaste => pick(vec![Layer::Paste(Side::Top), Layer::Paste(Side::Bottom)]),
        }
    }
}

fn rect_bounds(rect: Rect) -> [i64; 4] {
    [rect.min.x, rect.min.y, rect.max.x, rect.max.y]
}

fn resolve(options: &Options<'_>, text: &str) -> String {
    if text.contains("${") {
        (options.resolve)(text)
    } else {
        text.to_owned()
    }
}

/// Builds the scene of a board.
pub fn build(pcb: &Pcb, grid: i64, options: &Options<'_>) -> Scene {
    let outline = pcb.outline_bounds();
    let origin = outline.map_or(Point::ORIGIN, |r| {
        Point::new(r.min.x / 2 + r.max.x / 2, r.min.y / 2 + r.max.y / 2)
    });
    let origin_p = P::of(origin);
    let local = |p: Point| P::of(p).sub(origin_p);
    let mut b = SceneBuilder::new(origin_p);
    let layers = BoardLayers::new(&mut b, pcb);

    let mut net_index: HashMap<Id, u32> = HashMap::new();
    let mut names = Vec::with_capacity(pcb.nets.len());
    for (index, net) in pcb.nets.iter().enumerate() {
        net_index.insert(net.id, u32::try_from(index).unwrap_or(NONE));
        names.push(net.name.clone());
    }
    b.set_nets(names);
    let net_of = |net: Option<Id>| net.and_then(|id| net_index.get(&id).copied());

    // Zones first: they are the largest objects and sit below everything
    // else on their layer.
    for zone in &pcb.zones {
        let obj = b.object(ObjType::Zone, zone.id, None, None);
        b.set_net(obj, net_of(zone.net));
        let outline: Vec<P> = zone.outline.iter().map(|p| local(*p)).collect();
        let holes: Vec<Vec<P>> = zone
            .holes
            .iter()
            .map(|hole| hole.iter().map(|p| local(*p)).collect())
            .collect();
        for copper in &zone.layers {
            let Some(layer) = layers.copper(*copper) else {
                continue;
            };
            // A filled zone shows its copper; an unfilled one is hatched.
            let mut filled = false;
            for area in zone.filled.iter().filter(|area| area.layer == *copper) {
                let points: Vec<P> = area.points.iter().map(|p| local(*p)).collect();
                b.polygon(layer, obj, &points, &[], 0);
                filled = true;
            }
            if !filled {
                b.polygon(layer, obj, &outline, &holes, MESH_HATCH);
            }
            b.polyline(layer, obj, &outline, true, 0.0);
            for hole in &holes {
                b.polyline(layer, obj, hole, true, 0.0);
            }
        }
    }

    graphics(&mut b, &layers, pcb, origin_p, options);

    for fp in &pcb.footprints {
        footprint(
            &mut b,
            &layers,
            fp,
            origin_p,
            &net_of,
            options,
            pcb.stackup.copper_count(),
        );
    }

    for track in &pcb.tracks {
        let Some(layer) = layers.copper(track.layer) else {
            continue;
        };
        let obj = b.object(ObjType::Track, track.id, None, None);
        b.set_net(obj, net_of(track.net));
        let (start, end) = (local(track.start), local(track.end));
        let width = track.width as f64;
        match track
            .mid
            .and_then(|mid| geom::arc_through(start, local(mid), end))
        {
            Some((center, radius, from, sweep)) => {
                b.arc(layer, obj, center, radius, from, sweep, width);
            }
            None => b.segment(layer, obj, start, end, width),
        }
    }

    for via in &pcb.vias {
        let obj = b.object(ObjType::Via, via.id, None, None);
        b.set_net(obj, net_of(via.net));
        let at = local(via.at);
        for layer in layers.copper_span(via.from, via.to) {
            b.circle(layer, obj, at, via.diameter as f64 / 2.0, 0.0);
        }
        b.circle(layers.holes, obj, at, via.drill as f64 / 2.0, 0.0);
    }

    let home = outline.map(rect_bounds);
    b.finish(Header {
        doc: pcb.id,
        kind: "pcb",
        name: pcb.name.clone(),
        page: None,
        home,
        grid,
    })
}

/// Free shapes and texts of the board.
fn graphics(
    b: &mut SceneBuilder,
    layers: &BoardLayers,
    pcb: &Pcb,
    origin: P,
    options: &Options<'_>,
) {
    for shape in &pcb.shapes {
        let Some(layer) = layers.get(shape.layer) else {
            continue;
        };
        let obj = b.object(ObjType::BoardShape, shape.id, None, None);
        draw::geometry(
            b,
            obj,
            &shape.geometry,
            Paint::board(layer, shape.width as f64, shape.filled),
            &Xform::translate(origin.scale(-1.0)),
        );
    }
    for text in &pcb.texts {
        let Some(layer) = layers.get(text.layer) else {
            continue;
        };
        let obj = b.object(ObjType::BoardText, text.id, None, None);
        let mut item = TextItem::styled(
            resolve(options, &text.text),
            P::of(text.at).sub(origin),
            text.rotation.degrees(),
            &text.style,
            obj,
        );
        if text.mirrored {
            item.flags |= text::MIRROR;
        }
        b.text(layer, obj, item);
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "internal helper with plain inputs"
)]
fn footprint(
    b: &mut SceneBuilder,
    layers: &BoardLayers,
    fp: &FootprintInstance,
    origin: P,
    net_of: &dyn Fn(Option<Id>) -> Option<u32>,
    options: &Options<'_>,
    copper_count: u8,
) {
    let bottom = fp.side == Side::Bottom;
    let xf = Xform {
        offset: P::of(fp.at).sub(origin),
        angle: geom::radians(fp.rotation),
        mirror: bottom,
    };
    let flip = move |layer: Layer| {
        if bottom {
            layer.flipped(copper_count)
        } else {
            layer
        }
    };
    let obj = b.object(ObjType::Footprint, fp.id, None, None);
    for graphic in &fp.graphics {
        let Some(layer) = layers.get(flip(graphic.layer)) else {
            continue;
        };
        draw::geometry(
            b,
            obj,
            &graphic.geometry,
            Paint::board(layer, graphic.width as f64, graphic.filled),
            &xf,
        );
    }
    for (index, pad) in fp.pads.iter().enumerate() {
        let pad_obj = b.object(ObjType::Pad, fp.id, Some(obj), Some(index));
        b.set_net(pad_obj, net_of(pad.net));
        draw_pad(b, layers, pad_obj, pad, &xf, &flip);
        let bbox = b.bbox(pad_obj);
        b.extend(obj, bbox);
    }
    if b.bbox(obj).is_empty() {
        b.extend(obj, geom::BBox::of(xf.offset).grow(250_000.0));
    }
    for (index, text) in fp.texts.iter().enumerate() {
        if !text.visible {
            continue;
        }
        let Some(layer) = layers.get(flip(text.layer)) else {
            continue;
        };
        let content = match text.kind {
            FootprintTextKind::Designator if text.text.is_empty() => fp.designator.clone(),
            FootprintTextKind::Value if text.text.is_empty() => fp.value.clone(),
            _ => text
                .text
                .replace("${REFERENCE}", &fp.designator)
                .replace("${VALUE}", &fp.value),
        };
        let text_obj = b.object(ObjType::FootprintText, fp.id, Some(obj), Some(index));
        let rotation = xf.shape_angle(geom::radians(text.rotation)).to_degrees();
        let mut item = TextItem::styled(
            resolve(options, &content),
            xf.point(text.at),
            rotation,
            &text.style,
            text_obj,
        );
        if bottom {
            item.flags |= text::MIRROR;
        }
        b.text(layer, text_obj, text::upright(item));
    }
}

fn draw_pad(
    b: &mut SceneBuilder,
    layers: &BoardLayers,
    obj: u32,
    pad: &Pad,
    xf: &Xform,
    flip: &dyn Fn(Layer) -> Layer,
) {
    let rotation = geom::radians(pad.rotation);
    let at = P::of(pad.at);
    // Pad-local point → scene.
    let place = |q: P| xf.apply(at.add(q.rotate(rotation)));
    let center = place(P::default());
    let angle = xf.shape_angle(rotation);
    let half_w = pad.size.w as f64 / 2.0;
    let half_h = pad.size.h as f64 / 2.0;
    let mut targets: Vec<(usize, Layer)> = Vec::new();
    for spec in &pad.layers {
        for target in layers.pad_layers(*spec, flip) {
            if !targets.contains(&target) {
                targets.push(target);
            }
        }
    }
    for (layer, kind) in targets {
        let margin = match kind {
            Layer::SolderMask(_) => pad.mask_margin.unwrap_or(0) as f64,
            Layer::Paste(_) => pad.paste_margin.unwrap_or(0) as f64,
            _ => 0.0,
        };
        let (w, h) = ((half_w + margin).max(0.0), (half_h + margin).max(0.0));
        match &pad.shape {
            PadShape::Circle => b.circle(layer, obj, center, w, 0.0),
            PadShape::Rect => b.rect(layer, obj, center, w, h, angle, 0.0),
            PadShape::Oval => b.rect(layer, obj, center, w, h, angle, w.min(h)),
            PadShape::RoundRect { ratio } => {
                let corner = (half_w.min(half_h) * 2.0) * f64::from(*ratio) / 1000.0 + margin;
                b.rect(layer, obj, center, w, h, angle, corner);
            }
            PadShape::Trapezoid { delta } => {
                let (dw, dh) = (delta.w as f64 / 2.0, delta.h as f64 / 2.0);
                let corners = [
                    P::new(-(w + dw), -(h + dh)),
                    P::new(w + dw, -(h - dh)),
                    P::new(w - dw, h - dh),
                    P::new(-(w - dw), h + dh),
                ]
                .map(place);
                b.polygon(layer, obj, &corners, &[], 0);
            }
            PadShape::Custom { points } => {
                let points: Vec<P> = points.iter().map(|p| place(P::of(*p))).collect();
                b.polygon(layer, obj, &points, &[], 0);
            }
        }
    }
    if let Some(drill) = pad.drill {
        let radius = drill.diameter as f64 / 2.0;
        let offset = P::of(drill.offset);
        match drill.slot_length {
            Some(length) if length as f64 > drill.diameter as f64 => {
                let reach = (length as f64 - drill.diameter as f64) / 2.0;
                let a = place(offset.add(P::new(-reach, 0.0)));
                let c = place(offset.add(P::new(reach, 0.0)));
                b.segment(layers.holes, obj, a, c, drill.diameter as f64);
            }
            _ => b.circle(layers.holes, obj, place(offset), radius, 0.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{options, synthetic};

    #[test]
    fn synthetic_board_has_copper_holes_and_texts() {
        let project = synthetic(ew_synth::DesignSize::S);
        let pcb = project
            .documents
            .values()
            .find_map(|doc| match doc {
                ew_model::document::Document::Pcb(pcb) => Some(pcb),
                _ => None,
            })
            .unwrap();
        let scene = build(pcb, 1_270_000, &options());
        let meta = &scene.meta;
        assert_eq!(meta.kind, "pcb");
        let keys: Vec<&str> = meta.layers.iter().map(|l| l.key.as_str()).collect();
        assert_eq!(keys[0], "cu.top");
        assert_eq!(keys[1], "cu.bottom");
        assert!(keys.contains(&"outline") && keys.contains(&HOLES));
        let top = &meta.layers[0];
        assert!(top.segments.count > 0, "tracks on top copper");
        assert!(
            top.rects.count + top.circles.count > 0,
            "pads on top copper"
        );
        let silk = meta.layers.iter().find(|l| l.key == "silk.top").unwrap();
        assert!(!silk.texts.is_empty(), "designators");
        assert_eq!(meta.nets.len(), pcb.nets.len());
        let objects = pcb.tracks.len() + pcb.vias.len() + pcb.footprints.len();
        assert!(
            meta.objects.count as usize > objects,
            "pads and texts are objects too"
        );
        // The board outline decides the home view.
        let home = meta.home.unwrap();
        assert_eq!(home[2] - home[0], 80_000_000);
    }
}
