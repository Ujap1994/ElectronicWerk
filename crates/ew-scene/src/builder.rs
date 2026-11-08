//! Collects objects and primitives and writes them into one binary buffer.

use std::collections::HashMap;

use ew_model::id::Id;
use serde::Serialize;

use crate::geom::{BBox, P};
use crate::text::{self, TextItem};

/// "No object" / "no net" marker in the u32 tables.
pub const NONE: u32 = u32::MAX;

/// Instances per culling chunk.
pub const CHUNK: usize = 1024;

/// Mesh vertex flag: draw the triangle as a hatch pattern (unfilled zones).
pub const MESH_HATCH: u32 = 1;

/// Selectable object types, in the order of the `objectTypes` table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjType {
    /// Board: track or arc.
    Track,
    /// Board: via.
    Via,
    /// Board: placed footprint.
    Footprint,
    /// Board: pad of a footprint (child).
    Pad,
    /// Board: text of a footprint (child).
    FootprintText,
    /// Board: zone.
    Zone,
    /// Board: free text.
    BoardText,
    /// Board: graphic shape.
    BoardShape,
    /// Sheet: placed symbol.
    Symbol,
    /// Sheet: pin of a symbol (child).
    Pin,
    /// Sheet: field of a symbol (child).
    Field,
    /// Sheet: wire.
    Wire,
    /// Sheet: bus.
    Bus,
    /// Sheet: bus entry.
    BusEntry,
    /// Sheet: junction.
    Junction,
    /// Sheet: net label.
    Label,
    /// Sheet: power port.
    PowerPort,
    /// Sheet: no-connect flag.
    NoConnect,
    /// Sheet: sheet symbol.
    Sheet,
    /// Sheet: pin of a sheet symbol (child).
    SheetPin,
    /// Sheet: free text.
    Text,
    /// Sheet: graphic shape.
    Shape,
}

impl ObjType {
    /// Every type, indexed by its code.
    pub const ALL: [Self; 22] = [
        Self::Track,
        Self::Via,
        Self::Footprint,
        Self::Pad,
        Self::FootprintText,
        Self::Zone,
        Self::BoardText,
        Self::BoardShape,
        Self::Symbol,
        Self::Pin,
        Self::Field,
        Self::Wire,
        Self::Bus,
        Self::BusEntry,
        Self::Junction,
        Self::Label,
        Self::PowerPort,
        Self::NoConnect,
        Self::Sheet,
        Self::SheetPin,
        Self::Text,
        Self::Shape,
    ];

    /// Name used in the scene and the engine API.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Track => "track",
            Self::Via => "via",
            Self::Footprint => "footprint",
            Self::Pad => "pad",
            Self::FootprintText => "footprintText",
            Self::Zone => "zone",
            Self::BoardText => "boardText",
            Self::BoardShape => "boardShape",
            Self::Symbol => "symbol",
            Self::Pin => "pin",
            Self::Field => "field",
            Self::Wire => "wire",
            Self::Bus => "bus",
            Self::BusEntry => "busEntry",
            Self::Junction => "junction",
            Self::Label => "label",
            Self::PowerPort => "powerPort",
            Self::NoConnect => "noConnect",
            Self::Sheet => "sheet",
            Self::SheetPin => "sheetPin",
            Self::Text => "text",
            Self::Shape => "shape",
        }
    }

    /// Parses a type name.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|ty| ty.name() == name)
    }

    fn code(self) -> u8 {
        Self::ALL
            .iter()
            .position(|ty| *ty == self)
            .and_then(|index| u8::try_from(index).ok())
            .unwrap_or(u8::MAX)
    }
}

/// Static description of a scene layer.
#[derive(Debug, Clone)]
pub struct LayerDef {
    /// Stable key, for example `cu.top` or `sch.wires`.
    pub key: String,
    /// Layer group: `copper`, `silkscreen`, …, `holes`, `schematic`.
    pub group: &'static str,
    /// `top`, `bottom` or none.
    pub side: Option<&'static str>,
    /// User-defined name (copper layers of the stack-up).
    pub name: Option<String>,
    /// Draw order in the top view (back to front).
    pub z: i32,
    /// Visible by default.
    pub visible: bool,
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    a: P,
    b: P,
    half_width: f64,
    obj: u32,
}

#[derive(Debug, Clone, Copy)]
struct ArcPrim {
    center: P,
    radius: f64,
    half_width: f64,
    start: f64,
    sweep: f64,
    obj: u32,
}

#[derive(Debug, Clone, Copy)]
struct Circle {
    center: P,
    outer: f64,
    inner: f64,
    obj: u32,
}

#[derive(Debug, Clone, Copy)]
struct RectPrim {
    center: P,
    half_w: f64,
    half_h: f64,
    angle: f64,
    corner: f64,
    obj: u32,
}

#[derive(Debug, Default)]
struct LayerData {
    def: Option<LayerDef>,
    segments: Vec<Segment>,
    arcs: Vec<ArcPrim>,
    circles: Vec<Circle>,
    rects: Vec<RectPrim>,
    vertices: Vec<(P, u32, u32)>,
    indices: Vec<u32>,
    texts: Vec<TextItem>,
}

#[derive(Debug, Clone, Copy)]
struct ObjectRec {
    ty: u8,
    parent: u32,
    child: u32,
    net: u32,
    bbox: BBox,
    id: u128,
}

/// A primitive buffer section: byte offset and count, plus the byte offset of
/// its chunk table (`ceil(count / chunk)` boxes of four f32).
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    /// Byte offset of the first instance.
    pub offset: u32,
    /// Number of instances.
    pub count: u32,
    /// Byte offset of the chunk bounding boxes.
    pub chunks: u32,
}

/// The triangle mesh of a layer.
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshSection {
    /// Byte offset of the vertices (x, y as f32; object and flags as u32).
    pub vertices: u32,
    /// Number of vertices.
    pub vertex_count: u32,
    /// Byte offset of the u32 indices.
    pub indices: u32,
    /// Number of indices (three per triangle).
    pub index_count: u32,
}

/// One layer of a scene.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LayerInfo {
    /// Stable key, for example `cu.top`.
    pub key: String,
    /// Layer group.
    pub group: &'static str,
    /// `top`, `bottom` or `null`.
    pub side: Option<&'static str>,
    /// User-defined name.
    pub name: Option<String>,
    /// Draw order in the top view (back to front).
    pub z: i32,
    /// Visible by default.
    pub visible: bool,
    /// Line segments: ax, ay, bx, by, half width (f32), object (u32).
    pub segments: Section,
    /// Arcs: cx, cy, radius, half width, start, sweep (f32, radians), object.
    pub arcs: Section,
    /// Discs and rings: cx, cy, outer radius, inner radius (f32), object.
    pub circles: Section,
    /// Rotated, rounded rectangles: cx, cy, half w, half h, angle, corner
    /// radius (f32), object.
    pub rects: Section,
    /// Filled polygons.
    pub mesh: MeshSection,
    /// Texts: `[text, x, y, size, rotation°, hAlign, vAlign, flags, object]`.
    pub texts: Vec<TextItem>,
}

/// Byte offsets of the object table (one entry per object).
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectSections {
    /// Number of objects.
    pub count: u32,
    /// Type codes (u8, index into `objectTypes`).
    pub types: u32,
    /// Parent object index (u32, `0xFFFFFFFF` for none).
    pub parents: u32,
    /// Index of a child within its parent (pad, pin, text; u32).
    pub children: u32,
    /// Net index (u32, index into `nets`).
    pub nets: u32,
    /// Bounding boxes (min x, min y, max x, max y as f32).
    pub boxes: u32,
    /// Object ids (16 bytes each; children carry their parent's id).
    pub ids: u32,
}

/// Font metrics shared with the renderer (relative to the text size, which
/// is the cap height).
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FontMetrics {
    /// Advance per character.
    pub advance: f64,
    /// Distance between baselines.
    pub line_pitch: f64,
}

/// Counts for benchmarks and the status bar.
#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneStats {
    /// Objects (including pads, pins and fields).
    pub objects: usize,
    /// Line segments.
    pub segments: usize,
    /// Arcs.
    pub arcs: usize,
    /// Discs and rings.
    pub circles: usize,
    /// Rectangles.
    pub rects: usize,
    /// Triangles.
    pub triangles: usize,
    /// Texts.
    pub texts: usize,
    /// Size of the binary buffer.
    pub bytes: usize,
}

/// Everything about a scene except the binary data.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneMeta {
    /// Document id.
    pub doc: Id,
    /// `pcb` or `schematic`.
    pub kind: &'static str,
    /// Document name.
    pub name: String,
    /// All coordinates in the buffer are relative to this point (nm).
    pub origin: [i64; 2],
    /// Bounds of everything drawn (absolute nm), `null` if empty.
    pub bounds: Option<[i64; 4]>,
    /// Bounds to fit on open (board outline or page).
    pub home: Option<[i64; 4]>,
    /// Page size of a sheet (nm).
    pub page: Option<[i64; 2]>,
    /// Grid spacing from the project settings (nm).
    pub grid: i64,
    /// Instances per culling chunk.
    pub chunk: u32,
    /// Text metrics.
    pub font: FontMetrics,
    /// Layers, in panel order.
    pub layers: Vec<LayerInfo>,
    /// Object type names, indexed by type code.
    pub object_types: Vec<&'static str>,
    /// The object table.
    pub objects: ObjectSections,
    /// Net names, indexed by net index.
    pub nets: Vec<String>,
    /// Counts.
    pub stats: SceneStats,
}

/// A finished scene: metadata plus one little-endian binary buffer.
#[derive(Debug, Clone)]
pub struct Scene {
    /// Metadata with byte offsets into `data`.
    pub meta: SceneMeta,
    /// Primitive and object tables.
    pub data: Vec<u8>,
}

/// Collects the content of one document.
#[derive(Debug)]
pub struct SceneBuilder {
    origin: P,
    layers: Vec<LayerData>,
    layer_index: HashMap<String, usize>,
    objects: Vec<ObjectRec>,
    nets: Vec<String>,
}

impl SceneBuilder {
    /// A builder whose buffer coordinates are relative to `origin` (nm).
    pub fn new(origin: P) -> Self {
        Self {
            origin,
            layers: Vec::new(),
            layer_index: HashMap::new(),
            objects: Vec::new(),
            nets: Vec::new(),
        }
    }

    /// Registers a layer (once per key) and returns its index.
    pub fn layer(&mut self, def: LayerDef) -> usize {
        if let Some(index) = self.layer_index.get(&def.key) {
            return *index;
        }
        let index = self.layers.len();
        self.layer_index.insert(def.key.clone(), index);
        self.layers.push(LayerData {
            def: Some(def),
            ..LayerData::default()
        });
        index
    }

    /// Sets the net names (net index = position).
    pub fn set_nets(&mut self, nets: Vec<String>) {
        self.nets = nets;
    }

    /// Adds an object and returns its index.
    pub fn object(
        &mut self,
        ty: ObjType,
        id: Id,
        parent: Option<u32>,
        child: Option<usize>,
    ) -> u32 {
        let index = u32::try_from(self.objects.len()).unwrap_or(NONE);
        self.objects.push(ObjectRec {
            ty: ty.code(),
            parent: parent.unwrap_or(NONE),
            child: child.and_then(|c| u32::try_from(c).ok()).unwrap_or(NONE),
            net: NONE,
            bbox: BBox::EMPTY,
            id: id.as_u128(),
        });
        index
    }

    /// Sets the net of an object.
    pub fn set_net(&mut self, obj: u32, net: Option<u32>) {
        if let Some(rec) = self.objects.get_mut(obj as usize) {
            rec.net = net.unwrap_or(NONE);
        }
    }

    /// Grows the bounding box of an object.
    pub fn extend(&mut self, obj: u32, bbox: BBox) {
        if let Some(rec) = self.objects.get_mut(obj as usize) {
            rec.bbox = rec.bbox.union(bbox);
        }
    }

    /// Bounding box of an object so far.
    pub fn bbox(&self, obj: u32) -> BBox {
        self.objects
            .get(obj as usize)
            .map_or(BBox::EMPTY, |rec| rec.bbox)
    }

    fn data(&mut self, layer: usize) -> Option<&mut LayerData> {
        self.layers.get_mut(layer)
    }

    /// A line segment with round caps.
    pub fn segment(&mut self, layer: usize, obj: u32, a: P, b: P, width: f64) {
        let half_width = width.max(0.0) / 2.0;
        self.extend(obj, BBox::of(a).with(b).grow(half_width));
        if let Some(data) = self.data(layer) {
            data.segments.push(Segment {
                a,
                b,
                half_width,
                obj,
            });
        }
    }

    /// A closed or open polyline.
    pub fn polyline(&mut self, layer: usize, obj: u32, points: &[P], closed: bool, width: f64) {
        for pair in points.windows(2) {
            self.segment(layer, obj, pair[0], pair[1], width);
        }
        if closed && points.len() > 2 {
            if let (Some(first), Some(last)) = (points.first(), points.last()) {
                self.segment(layer, obj, *last, *first, width);
            }
        }
    }

    /// An arc stroke (`sweep` > 0 counter-clockwise).
    #[allow(clippy::too_many_arguments, reason = "plain geometry parameters")]
    pub fn arc(
        &mut self,
        layer: usize,
        obj: u32,
        center: P,
        radius: f64,
        start: f64,
        sweep: f64,
        width: f64,
    ) {
        let half_width = width.max(0.0) / 2.0;
        self.extend(
            obj,
            crate::geom::arc_bounds(center, radius, start, sweep).grow(half_width),
        );
        if let Some(data) = self.data(layer) {
            data.arcs.push(ArcPrim {
                center,
                radius,
                half_width,
                start,
                sweep,
                obj,
            });
        }
    }

    /// A disc (`inner` = 0) or ring.
    pub fn circle(&mut self, layer: usize, obj: u32, center: P, outer: f64, inner: f64) {
        self.extend(obj, BBox::of(center).grow(outer));
        if let Some(data) = self.data(layer) {
            data.circles.push(Circle {
                center,
                outer,
                inner: inner.max(0.0),
                obj,
            });
        }
    }

    /// A filled, rotated rectangle with rounded corners.
    #[allow(clippy::too_many_arguments, reason = "plain geometry parameters")]
    pub fn rect(
        &mut self,
        layer: usize,
        obj: u32,
        center: P,
        half_w: f64,
        half_h: f64,
        angle: f64,
        corner: f64,
    ) {
        let (sin, cos) = angle.sin_cos();
        let ex = half_w * cos.abs() + half_h * sin.abs();
        let ey = half_w * sin.abs() + half_h * cos.abs();
        self.extend(
            obj,
            BBox {
                min_x: center.x - ex,
                min_y: center.y - ey,
                max_x: center.x + ex,
                max_y: center.y + ey,
            },
        );
        if let Some(data) = self.data(layer) {
            data.rects.push(RectPrim {
                center,
                half_w,
                half_h,
                angle,
                corner: corner.clamp(0.0, half_w.min(half_h)),
                obj,
            });
        }
    }

    /// A filled polygon with holes, triangulated.
    pub fn polygon(&mut self, layer: usize, obj: u32, outline: &[P], holes: &[Vec<P>], flags: u32) {
        if outline.len() < 3 {
            return;
        }
        let mut coords: Vec<[f64; 2]> = outline.iter().map(|p| [p.x, p.y]).collect();
        let mut hole_starts: Vec<u32> = Vec::with_capacity(holes.len());
        for hole in holes.iter().filter(|hole| hole.len() >= 3) {
            hole_starts.push(u32::try_from(coords.len()).unwrap_or(u32::MAX));
            coords.extend(hole.iter().map(|p| [p.x, p.y]));
        }
        let mut triangles: Vec<u32> = Vec::new();
        earcut::Earcut::new().earcut_slice(&coords, &hole_starts, &mut triangles);
        let mut bbox = BBox::EMPTY;
        for p in outline {
            bbox = bbox.with(*p);
        }
        self.extend(obj, bbox);
        let Some(data) = self.data(layer) else {
            return;
        };
        let base = u32::try_from(data.vertices.len()).unwrap_or(0);
        data.vertices
            .extend(coords.iter().map(|c| (P::new(c[0], c[1]), obj, flags)));
        data.indices.extend(triangles.iter().map(|i| base + i));
    }

    /// A text (also grows the object by the text box).
    pub fn text(&mut self, layer: usize, obj: u32, item: TextItem) {
        if item.text.trim().is_empty() {
            return;
        }
        self.extend(obj, text::text_bounds(&item));
        if let Some(data) = self.data(layer) {
            data.texts.push(item);
        }
    }

    /// Writes the buffer.
    pub fn finish(mut self, header: Header) -> Scene {
        let mut writer = Writer::default();
        let mut stats = SceneStats {
            objects: self.objects.len(),
            ..SceneStats::default()
        };

        let mut bounds = BBox::EMPTY;
        for layer in &self.layers {
            for text in &layer.texts {
                bounds = bounds.union(text::text_bounds(text));
            }
        }
        for rec in &self.objects {
            bounds = bounds.union(rec.bbox);
        }
        for layer in &self.layers {
            for v in &layer.vertices {
                bounds = bounds.with(v.0);
            }
        }

        let objects = self.write_objects(&mut writer);
        let mut layers = Vec::with_capacity(self.layers.len());
        for data in &mut self.layers {
            stats.segments += data.segments.len();
            stats.arcs += data.arcs.len();
            stats.circles += data.circles.len();
            stats.rects += data.rects.len();
            stats.triangles += data.indices.len() / 3;
            stats.texts += data.texts.len();
            layers.push(write_layer(&mut writer, data, bounds));
        }
        stats.bytes = writer.data.len();

        let origin = self.origin;
        let absolute = |b: BBox| -> Option<[i64; 4]> {
            (!b.is_empty()).then(|| {
                [
                    (b.min_x + origin.x).floor() as i64,
                    (b.min_y + origin.y).floor() as i64,
                    (b.max_x + origin.x).ceil() as i64,
                    (b.max_y + origin.y).ceil() as i64,
                ]
            })
        };
        let meta = SceneMeta {
            doc: header.doc,
            kind: header.kind,
            name: header.name,
            origin: [origin.x as i64, origin.y as i64],
            bounds: absolute(bounds),
            home: header.home.or_else(|| absolute(bounds)),
            page: header.page,
            grid: header.grid,
            chunk: u32::try_from(CHUNK).unwrap_or(1024),
            font: FontMetrics {
                advance: text::ADVANCE,
                line_pitch: text::LINE_PITCH,
            },
            layers,
            object_types: ObjType::ALL.iter().map(|ty| ty.name()).collect(),
            objects,
            nets: self.nets,
            stats,
        };
        Scene {
            meta,
            data: writer.data,
        }
    }

    fn write_objects(&self, writer: &mut Writer) -> ObjectSections {
        let count = u32::try_from(self.objects.len()).unwrap_or(0);
        let types = writer.offset();
        for rec in &self.objects {
            writer.data.push(rec.ty);
        }
        writer.align();
        let parents = writer.offset();
        for rec in &self.objects {
            writer.u32(rec.parent);
        }
        let children = writer.offset();
        for rec in &self.objects {
            writer.u32(rec.child);
        }
        let nets = writer.offset();
        for rec in &self.objects {
            writer.u32(rec.net);
        }
        let boxes = writer.offset();
        for rec in &self.objects {
            let b = if rec.bbox.is_empty() {
                BBox::of(P::new(0.0, 0.0))
            } else {
                rec.bbox
            };
            writer.f32s(&[b.min_x, b.min_y, b.max_x, b.max_y]);
        }
        let ids = writer.offset();
        for rec in &self.objects {
            writer.data.extend_from_slice(&rec.id.to_be_bytes());
        }
        ObjectSections {
            count,
            types,
            parents,
            children,
            nets,
            boxes,
            ids,
        }
    }
}

/// Document-level values for [`SceneBuilder::finish`].
#[derive(Debug, Clone)]
pub struct Header {
    /// Document id.
    pub doc: Id,
    /// `pcb` or `schematic`.
    pub kind: &'static str,
    /// Document name.
    pub name: String,
    /// Page size of a sheet.
    pub page: Option<[i64; 2]>,
    /// Area to fit on open.
    pub home: Option<[i64; 4]>,
    /// Grid spacing.
    pub grid: i64,
}

/// Sorts instances along a Z-order curve so that chunks are compact, then
/// writes them and their chunk boxes.
fn write_sorted<T: Copy>(
    writer: &mut Writer,
    items: &mut [T],
    scene: BBox,
    bbox_of: impl Fn(&T) -> BBox,
    write: impl Fn(&mut Writer, &T),
) -> Section {
    if items.len() > CHUNK {
        let width = (scene.max_x - scene.min_x).max(1.0);
        let height = (scene.max_y - scene.min_y).max(1.0);
        items.sort_by_cached_key(|item| {
            let b = bbox_of(item);
            let x = ((f64::midpoint(b.min_x, b.max_x) - scene.min_x) / width * 65535.0)
                .clamp(0.0, 65535.0);
            let y = ((f64::midpoint(b.min_y, b.max_y) - scene.min_y) / height * 65535.0)
                .clamp(0.0, 65535.0);
            morton(x as u32, y as u32)
        });
    }
    let offset = writer.offset();
    for item in items.iter() {
        write(writer, item);
    }
    let chunks = writer.offset();
    for chunk in items.chunks(CHUNK) {
        let b = chunk
            .iter()
            .fold(BBox::EMPTY, |acc, item| acc.union(bbox_of(item)));
        writer.f32s(&[b.min_x, b.min_y, b.max_x, b.max_y]);
    }
    Section {
        offset,
        count: u32::try_from(items.len()).unwrap_or(0),
        chunks,
    }
}

fn write_layer(writer: &mut Writer, data: &mut LayerData, scene: BBox) -> LayerInfo {
    let segments = write_sorted(
        writer,
        &mut data.segments,
        scene,
        |s| BBox::of(s.a).with(s.b).grow(s.half_width),
        |w, s| {
            w.f32s(&[s.a.x, s.a.y, s.b.x, s.b.y, s.half_width]);
            w.u32(s.obj);
        },
    );
    let arcs = write_sorted(
        writer,
        &mut data.arcs,
        scene,
        |a| crate::geom::arc_bounds(a.center, a.radius, a.start, a.sweep).grow(a.half_width),
        |w, a| {
            w.f32s(&[
                a.center.x,
                a.center.y,
                a.radius,
                a.half_width,
                a.start,
                a.sweep,
            ]);
            w.u32(a.obj);
        },
    );
    let circles = write_sorted(
        writer,
        &mut data.circles,
        scene,
        |c| BBox::of(c.center).grow(c.outer),
        |w, c| {
            w.f32s(&[c.center.x, c.center.y, c.outer, c.inner]);
            w.u32(c.obj);
        },
    );
    let rects = write_sorted(
        writer,
        &mut data.rects,
        scene,
        |r| BBox::of(r.center).grow(r.half_w.hypot(r.half_h)),
        |w, r| {
            w.f32s(&[
                r.center.x, r.center.y, r.half_w, r.half_h, r.angle, r.corner,
            ]);
            w.u32(r.obj);
        },
    );
    let vertices = writer.offset();
    for (p, obj, flags) in &data.vertices {
        writer.f32s(&[p.x, p.y]);
        writer.u32(*obj);
        writer.u32(*flags);
    }
    let indices = writer.offset();
    for index in &data.indices {
        writer.u32(*index);
    }
    let def = data.def.take().unwrap_or(LayerDef {
        key: String::new(),
        group: "user",
        side: None,
        name: None,
        z: 0,
        visible: true,
    });
    LayerInfo {
        key: def.key,
        group: def.group,
        side: def.side,
        name: def.name,
        z: def.z,
        visible: def.visible,
        segments,
        arcs,
        circles,
        rects,
        mesh: MeshSection {
            vertices,
            vertex_count: u32::try_from(data.vertices.len()).unwrap_or(0),
            indices,
            index_count: u32::try_from(data.indices.len()).unwrap_or(0),
        },
        texts: std::mem::take(&mut data.texts),
    }
}

/// Interleaves the bits of two 16-bit values.
fn morton(x: u32, y: u32) -> u32 {
    fn spread(mut v: u32) -> u32 {
        v &= 0xFFFF;
        v = (v | (v << 8)) & 0x00FF_00FF;
        v = (v | (v << 4)) & 0x0F0F_0F0F;
        v = (v | (v << 2)) & 0x3333_3333;
        (v | (v << 1)) & 0x5555_5555
    }
    spread(x) | (spread(y) << 1)
}

#[derive(Debug, Default)]
struct Writer {
    data: Vec<u8>,
}

impl Writer {
    fn offset(&self) -> u32 {
        u32::try_from(self.data.len()).unwrap_or(u32::MAX)
    }

    fn align(&mut self) {
        while self.data.len() % 4 != 0 {
            self.data.push(0);
        }
    }

    fn u32(&mut self, value: u32) {
        self.data.extend_from_slice(&value.to_le_bytes());
    }

    fn f32s(&mut self, values: &[f64]) {
        for value in values {
            self.data.extend_from_slice(&(*value as f32).to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morton_interleaves() {
        assert_eq!(morton(0, 0), 0);
        assert_eq!(morton(1, 0), 1);
        assert_eq!(morton(0, 1), 2);
        assert_eq!(morton(3, 3), 15);
    }

    #[test]
    fn type_names_round_trip() {
        for ty in ObjType::ALL {
            assert_eq!(ObjType::from_name(ty.name()), Some(ty));
            assert_eq!(ObjType::ALL[usize::from(ty.code())], ty);
        }
    }

    #[test]
    fn writes_aligned_sections_and_chunks() {
        let mut builder = SceneBuilder::new(P::new(0.0, 0.0));
        let layer = builder.layer(LayerDef {
            key: "cu.top".into(),
            group: "copper",
            side: Some("top"),
            name: None,
            z: 0,
            visible: true,
        });
        let obj = builder.object(ObjType::Track, Id::from_u128(7), None, None);
        for i in 0..(CHUNK + 5) {
            let x = i as f64 * 10.0;
            builder.segment(layer, obj, P::new(x, 0.0), P::new(x + 5.0, 0.0), 2.0);
        }
        builder.circle(layer, obj, P::new(1.0, 1.0), 3.0, 1.0);
        let scene = builder.finish(Header {
            doc: Id::from_u128(1),
            kind: "pcb",
            name: "Board".into(),
            page: None,
            home: None,
            grid: 1,
        });
        let info = &scene.meta.layers[0];
        assert_eq!(info.segments.count as usize, CHUNK + 5);
        assert_eq!(info.segments.offset % 4, 0);
        assert_eq!(
            info.segments.chunks - info.segments.offset,
            u32::try_from((CHUNK + 5) * 24).unwrap()
        );
        assert_eq!(info.circles.count, 1);
        assert_eq!(scene.meta.objects.count, 1);
        assert_eq!(scene.meta.stats.segments, CHUNK + 5);
        let bounds = scene.meta.bounds.unwrap();
        assert_eq!(bounds[0], -2);
        assert_eq!(scene.data.len() % 4, 0);
        // The id is stored big-endian.
        let ids = scene.meta.objects.ids as usize;
        assert_eq!(scene.data[ids + 15], 7);
    }
}
