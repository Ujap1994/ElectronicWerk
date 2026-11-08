//! Synthetic, reproducible reference designs.
//!
//! The performance budgets of the roadmap (section 6) are measured on
//! generated designs, so they are reproducible and contain no private data:
//!
//! | Size | Parts  | Nets   | Track segments | Copper layers |
//! | ---- | ------ | ------ | -------------- | ------------- |
//! | S    | 50     | 60     | 600            | 2             |
//! | M    | 500    | 1,000  | 20,000         | 4             |
//! | L    | 5,000  | 10,000 | 200,000        | 8             |
//! | XL   | 20,000 | 40,000 | 800,000        | 8             |
//!
//! The same size and seed always produce exactly the same project.

mod parts;
mod rng;

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use ew_model::document::{Document, DocumentKind};
use ew_model::id::Id;
use ew_model::layer::{CopperLayer, Layer};
use ew_model::library::Library;
use ew_model::outputs::OutputJob;
use ew_model::pcb::{
    BoardShape, FootprintAttributes, FootprintInstance, Net, Pcb, STANDARD_THICKNESS, Stackup,
    Track, Via, Zone, ZoneFill,
};
use ew_model::project::{DocumentRef, Project};
use ew_model::rules::{NetClass, Rules};
use ew_model::schematic::{
    ComponentFlags, Label, LabelKind, Orientation, Page, PageFormat, PlacedSymbol, Schematic,
    SheetSymbol, Wire,
};
use ew_model::shape::Geometry;
use ew_model::units::{Angle, Nm, Point, Size, mm, um};

pub use parts::PartKind;
use rng::Rng;

/// Size class of a synthetic design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DesignSize {
    /// Small: 50 parts.
    S,
    /// Medium: 500 parts.
    M,
    /// Large: 5,000 parts.
    L,
    /// Extra large: 20,000 parts (stress test).
    Xl,
}

/// Target numbers of a size class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeSpec {
    /// Number of parts.
    pub parts: usize,
    /// Number of nets.
    pub nets: usize,
    /// Number of track segments.
    pub track_segments: usize,
    /// Copper layers.
    pub copper_layers: u8,
    /// Board width.
    pub width: Nm,
    /// Board height.
    pub height: Nm,
    /// Parts per schematic sheet.
    pub parts_per_sheet: usize,
}

impl DesignSize {
    /// Every size, smallest first.
    pub const ALL: [Self; 4] = [Self::S, Self::M, Self::L, Self::Xl];

    /// The targets of this size.
    #[must_use]
    pub const fn spec(self) -> SizeSpec {
        match self {
            Self::S => SizeSpec {
                parts: 50,
                nets: 60,
                track_segments: 600,
                copper_layers: 2,
                width: mm(80),
                height: mm(60),
                parts_per_sheet: 50,
            },
            Self::M => SizeSpec {
                parts: 500,
                nets: 1_000,
                track_segments: 20_000,
                copper_layers: 4,
                width: mm(160),
                height: mm(120),
                parts_per_sheet: 100,
            },
            Self::L => SizeSpec {
                parts: 5_000,
                nets: 10_000,
                track_segments: 200_000,
                copper_layers: 8,
                width: mm(400),
                height: mm(300),
                parts_per_sheet: 100,
            },
            Self::Xl => SizeSpec {
                parts: 20_000,
                nets: 40_000,
                track_segments: 800_000,
                copper_layers: 8,
                width: mm(800),
                height: mm(600),
                parts_per_sheet: 200,
            },
        }
    }

    /// `S`, `M`, `L` or `XL`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::S => "S",
            Self::M => "M",
            Self::L => "L",
            Self::Xl => "XL",
        }
    }
}

impl fmt::Display for DesignSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for DesignSize {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|size| size.name().eq_ignore_ascii_case(text))
            .ok_or_else(|| format!("unknown design size `{text}` (use S, M, L or XL)"))
    }
}

/// Default seed.
pub const DEFAULT_SEED: u64 = 0x00E1_EC7E_0000_0001;

/// Generates a design of the given size.
#[must_use]
pub fn generate(size: DesignSize, seed: u64) -> Project {
    generate_spec(&size.spec(), &format!("Synthetic {size}"), seed)
}

/// Generates a design from explicit targets.
#[must_use]
pub fn generate_spec(spec: &SizeSpec, name: &str, seed: u64) -> Project {
    Generator::new(spec, seed).build(name)
}

/// One placed part with everything both editors need.
struct Part {
    kind: PartKind,
    component: Id,
    designator: String,
    at: Point,
    rotation: Angle,
    bottom: bool,
    /// Net index of every pad, in pad order.
    nets: Vec<usize>,
}

struct Generator<'a> {
    spec: &'a SizeSpec,
    rng: Rng,
}

impl<'a> Generator<'a> {
    fn new(spec: &'a SizeSpec, seed: u64) -> Self {
        Self {
            spec,
            rng: Rng::new(seed),
        }
    }

    fn id(&mut self) -> Id {
        Id::from_random_bytes(self.rng.bytes())
    }

    fn build(mut self, name: &str) -> Project {
        let mut project = Project::new(self.id(), name);
        "A".clone_into(&mut project.manifest.revision);
        project.manifest.description = format!(
            "Synthetic reference design with {} parts (generated, not a real circuit).",
            self.spec.parts
        );

        let library_id = self.id();
        let library = parts::library(library_id, &mut || self.id());
        let parts = self.place_parts();
        let net_ids: Vec<Id> = (0..self.spec.nets.max(1)).map(|_| self.id()).collect();

        let (root, sheets) = self.schematics(&parts, &net_ids);
        let root_id = root.id;
        let pcb = self.pcb(&parts, &net_ids, &library);
        let rules = Rules::new(self.id(), "Design rules");
        let job_id = self.id();
        let outputs = OutputJob::fabrication(job_id, "Fabrication", || self.id());

        let mut add = |kind: DocumentKind, path: String, document: Document| {
            let reference = DocumentRef {
                id: document.id(),
                kind,
                path,
            };
            project
                .add_document(reference, usize::MAX, document)
                .unwrap_or_default();
        };
        add(
            DocumentKind::Schematic,
            "schematics/root.ewsch".to_owned(),
            Document::Schematic(root),
        );
        for (index, sheet) in sheets.into_iter().enumerate() {
            add(
                DocumentKind::Schematic,
                format!("schematics/sheet-{:03}.ewsch", index + 1),
                Document::Schematic(sheet),
            );
        }
        add(
            DocumentKind::Pcb,
            "pcb/board.ewpcb".to_owned(),
            Document::Pcb(pcb),
        );
        add(
            DocumentKind::Library,
            "library/project.ewlib".to_owned(),
            Document::Library(library),
        );
        add(
            DocumentKind::Rules,
            "rules/design.ewrules".to_owned(),
            Document::Rules(rules),
        );
        add(
            DocumentKind::OutputJob,
            "outputs/fabrication.ewout".to_owned(),
            Document::OutputJob(outputs),
        );
        project.manifest.root_schematic = Some(root_id);
        project
    }

    fn place_parts(&mut self) -> Vec<Part> {
        let spec = self.spec;
        let kinds: Vec<PartKind> = (0..spec.parts)
            .map(|_| PartKind::pick(self.rng.below(100)))
            .collect();
        let total_pads: usize = kinds.iter().map(|kind| kind.pad_count()).sum();
        let nets = spec.nets.max(1);

        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "small positive grid dimensions"
        )]
        let columns = {
            let ratio = spec.width as f64 / spec.height.max(1) as f64;
            ((spec.parts as f64 * ratio).sqrt().ceil() as usize).max(1)
        };
        let rows = spec.parts.div_ceil(columns).max(1);
        let pitch_x = spec.width / i64::try_from(columns).unwrap_or(1);
        let pitch_y = spec.height / i64::try_from(rows).unwrap_or(1);

        let mut counters: BTreeMap<&str, usize> = BTreeMap::new();
        let mut pad_index = 0_usize;
        let mut parts = Vec::with_capacity(spec.parts);
        for (index, kind) in kinds.into_iter().enumerate() {
            let column = i64::try_from(index % columns).unwrap_or(0);
            let row = i64::try_from(index / columns).unwrap_or(0);
            let at = Point::new(pitch_x * column + pitch_x / 2, pitch_y * row + pitch_y / 2);
            let counter = counters.entry(kind.prefix()).or_insert(0);
            *counter += 1;
            let designator = format!("{}{}", kind.prefix(), counter);
            let nets_of_part = (0..kind.pad_count())
                .map(|_| {
                    let net = pad_index * nets / total_pads.max(1);
                    pad_index += 1;
                    net.min(nets - 1)
                })
                .collect();
            let rotation = Angle::from_degrees(i64::from(self.rng.below(4)) * 90);
            let bottom = kind.is_passive() && self.rng.below(10) == 0;
            parts.push(Part {
                kind,
                component: self.id(),
                designator,
                at,
                rotation,
                bottom,
                nets: nets_of_part,
            });
        }
        parts
    }

    fn net_name(index: usize) -> String {
        match index {
            0 => "GND".to_owned(),
            1 => "+3V3".to_owned(),
            2 => "+5V".to_owned(),
            _ => format!("N{index:05}"),
        }
    }

    fn schematics(&mut self, parts: &[Part], net_ids: &[Id]) -> (Schematic, Vec<Schematic>) {
        let _ = net_ids;
        let per_sheet = self.spec.parts_per_sheet.max(1);
        let chunks: Vec<&[Part]> = parts.chunks(per_sheet).collect();
        let mut root = Schematic::new(self.id(), "Root");
        root.page = Page {
            format: PageFormat::A3,
            orientation: Orientation::Landscape,
            size: None,
        };
        if chunks.len() <= 1 {
            self.fill_sheet(&mut root, parts);
            return (root, Vec::new());
        }
        let mut sheets = Vec::with_capacity(chunks.len());
        let columns = 8_i64;
        for (index, chunk) in chunks.iter().enumerate() {
            let mut sheet = Schematic::new(self.id(), format!("Sheet {}", index + 1));
            sheet.page = root.page;
            self.fill_sheet(&mut sheet, chunk);
            let position = i64::try_from(index).unwrap_or(0);
            root.sheets.insert(SheetSymbol {
                id: self.id(),
                name: sheet.name.clone(),
                schematic: sheet.id,
                at: Point::new(
                    mm(20) + (position % columns) * mm(45),
                    mm(260) - (position / columns) * mm(30),
                ),
                size: Size::mm(35, 20),
                pins: Vec::new(),
            });
            sheets.push(sheet);
        }
        (root, sheets)
    }

    fn fill_sheet(&mut self, sheet: &mut Schematic, parts: &[Part]) {
        let columns = 10_i64;
        for (index, part) in parts.iter().enumerate() {
            let position = i64::try_from(index).unwrap_or(0);
            let at = Point::new(
                mm(30) + (position % columns) * um(38_100),
                mm(255) - (position / columns) * um(25_400),
            );
            let symbol = PlacedSymbol {
                id: self.id(),
                symbol: part.kind.symbol_name().to_owned(),
                unit: 1,
                at,
                rotation: Angle::ZERO,
                mirror: false,
                designator: part.designator.clone(),
                value: part.kind.value().to_owned(),
                footprint: Some(part.kind.footprint_name().to_owned()),
                component: Some(part.component),
                properties: BTreeMap::new(),
                fields: Vec::new(),
                flags: ComponentFlags::default(),
                instances: BTreeMap::new(),
            };
            sheet.symbols.insert(symbol);
            for (pin, net) in part.kind.pin_points().into_iter().zip(&part.nets) {
                let start = at + pin.at;
                let end = start + pin.outward(um(2_540));
                sheet.wires.insert(Wire {
                    id: self.id(),
                    start,
                    end,
                });
                // The label points away from the symbol.
                let out = pin.outward(1);
                let rotation = match (out.x, out.y) {
                    (-1, _) => Angle::DEG_180,
                    (_, 1) => Angle::DEG_90,
                    (_, -1) => Angle::DEG_270,
                    _ => Angle::ZERO,
                };
                sheet.labels.insert(Label {
                    id: self.id(),
                    kind: LabelKind::Global,
                    text: Self::net_name(*net),
                    at: end,
                    rotation,
                    shape: ew_model::schematic::PortShape::Passive,
                    style: ew_model::text::TextStyle::default(),
                });
            }
        }
    }

    fn pcb(&mut self, parts: &[Part], net_ids: &[Id], library: &Library) -> Pcb {
        let spec = self.spec;
        let copper = CopperLayer::stack(spec.copper_layers);
        let mut pcb = Pcb::new(
            self.id(),
            "Board",
            Stackup::standard(spec.copper_layers, STANDARD_THICKNESS),
        );
        for (index, id) in net_ids.iter().enumerate() {
            pcb.nets.insert(Net {
                id: *id,
                name: Self::net_name(index),
                class: if index < 3 {
                    "Power".to_owned()
                } else {
                    String::new()
                },
            });
        }
        pcb.shapes.insert(BoardShape {
            id: self.id(),
            layer: Layer::Outline,
            geometry: Geometry::Rect {
                start: Point::ORIGIN,
                end: Point::new(spec.width, spec.height),
            },
            width: um(100),
            filled: false,
        });

        // Footprints and absolute pad positions per net.
        let mut pads_by_net: Vec<Vec<Point>> = vec![Vec::new(); net_ids.len()];
        for part in parts {
            let Some(definition) = library.footprint(part.kind.footprint_name()) else {
                continue;
            };
            let mut pads = definition.pads.clone();
            for (pad, net) in pads.iter_mut().zip(&part.nets) {
                pad.id = self.id();
                pad.net = net_ids.get(*net).copied();
                let local = if part.bottom {
                    Point::new(-pad.at.x, pad.at.y)
                } else {
                    pad.at
                };
                if let Some(list) = pads_by_net.get_mut(*net) {
                    list.push(part.at + rotate(local, part.rotation));
                }
            }
            pcb.footprints.insert(FootprintInstance {
                id: self.id(),
                component: Some(part.component),
                footprint: definition.name.clone(),
                designator: part.designator.clone(),
                value: part.kind.value().to_owned(),
                at: part.at,
                rotation: part.rotation,
                side: if part.bottom {
                    ew_model::layer::Side::Bottom
                } else {
                    ew_model::layer::Side::Top
                },
                locked: false,
                pads,
                graphics: definition.graphics.clone(),
                texts: definition.texts.clone(),
                models: Vec::new(),
                properties: BTreeMap::new(),
                attributes: FootprintAttributes::default(),
            });
        }

        self.route(&mut pcb, &pads_by_net, net_ids, &copper);

        // Ground planes on the inner layers.
        if copper.len() > 2 {
            let outline = vec![
                Point::new(mm(1), mm(1)),
                Point::new(spec.width - mm(1), mm(1)),
                Point::new(spec.width - mm(1), spec.height - mm(1)),
                Point::new(mm(1), spec.height - mm(1)),
            ];
            for layer in &copper[1..copper.len() - 1] {
                pcb.zones.insert(Zone {
                    id: self.id(),
                    name: String::new(),
                    net: net_ids.first().copied(),
                    layers: vec![*layer],
                    outline: outline.clone(),
                    holes: Vec::new(),
                    priority: 0,
                    keepout: None,
                    fill: ZoneFill::default(),
                });
            }
        }
        pcb
    }

    /// Connects consecutive pads of each net with staircase routes until the
    /// track segment target is reached exactly.
    fn route(
        &mut self,
        pcb: &mut Pcb,
        pads_by_net: &[Vec<Point>],
        net_ids: &[Id],
        copper: &[CopperLayer],
    ) {
        let connections: usize = pads_by_net
            .iter()
            .map(|pads| pads.len().saturating_sub(1))
            .sum();
        if connections == 0 {
            return;
        }
        let target = self.spec.track_segments;
        let base = target / connections;
        let mut extra = target % connections;
        let power = NetClass::power_class().track_width;
        let signal = NetClass::default_class().track_width;
        for (net_index, pads) in pads_by_net.iter().enumerate() {
            let net = net_ids.get(net_index).copied();
            let width = if net_index < 3 { power } else { signal };
            for pair in pads.windows(2) {
                let mut segments = base;
                if extra > 0 {
                    segments += 1;
                    extra -= 1;
                }
                if segments == 0 {
                    continue;
                }
                let layer =
                    copper[self.rng.below(u32::try_from(copper.len()).unwrap_or(1)) as usize];
                let points = staircase(pair[0], pair[1], segments);
                for step in points.windows(2) {
                    pcb.tracks.insert(Track {
                        id: self.id(),
                        layer,
                        net,
                        width,
                        start: step[0],
                        end: step[1],
                        mid: None,
                        locked: false,
                    });
                }
                if layer != CopperLayer::Top {
                    for at in [pair[0], pair[1]] {
                        pcb.vias.insert(Via {
                            id: self.id(),
                            at,
                            diameter: um(600),
                            drill: um(300),
                            net,
                            from: CopperLayer::Top,
                            to: CopperLayer::Bottom,
                            micro: false,
                            locked: false,
                        });
                    }
                }
            }
        }
    }
}

/// Rotates a point counter-clockwise by a multiple of 90°.
fn rotate(point: Point, angle: Angle) -> Point {
    match angle.millidegrees() {
        90_000 => Point::new(-point.y, point.x),
        180_000 => Point::new(-point.x, -point.y),
        270_000 => Point::new(point.y, -point.x),
        _ => point,
    }
}

/// `segments` + 1 points from `a` to `b`, alternating horizontal and
/// vertical moves.
fn staircase(a: Point, b: Point, segments: usize) -> Vec<Point> {
    let mut points = Vec::with_capacity(segments + 1);
    points.push(a);
    let horizontal_steps = i64::try_from(segments.div_ceil(2)).unwrap_or(1).max(1);
    let vertical_steps = i64::try_from(segments / 2).unwrap_or(0).max(1);
    let mut current = a;
    let mut done_h = 0;
    let mut done_v = 0;
    for index in 0..segments {
        let last = index + 1 == segments;
        let next = if index % 2 == 0 {
            done_h += 1;
            let x = if done_h >= horizontal_steps || last {
                b.x
            } else {
                a.x + (b.x - a.x) * done_h / horizontal_steps
            };
            Point::new(x, current.y)
        } else {
            done_v += 1;
            let y = if done_v >= vertical_steps || last {
                b.y
            } else {
                a.y + (b.y - a.y) * done_v / vertical_steps
            };
            Point::new(current.x, y)
        };
        // Keep segments non-degenerate even between pads on one line.
        let next = if next == current {
            Point::new(next.x + um(10), next.y)
        } else {
            next
        };
        points.push(next);
        current = next;
    }
    points
}

/// Counts of a generated (or any) project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DesignStats {
    /// Placed symbols.
    pub symbols: usize,
    /// Schematic sheets.
    pub sheets: usize,
    /// Board footprints.
    pub footprints: usize,
    /// Pads.
    pub pads: usize,
    /// Nets.
    pub nets: usize,
    /// Track segments.
    pub tracks: usize,
    /// Vias.
    pub vias: usize,
    /// Zones.
    pub zones: usize,
    /// Copper layers.
    pub copper_layers: u8,
}

/// Counts the objects of a project.
#[must_use]
pub fn stats(project: &Project) -> DesignStats {
    let mut stats = DesignStats::default();
    for document in project.documents.values() {
        match document {
            Document::Schematic(sheet) => {
                stats.sheets += 1;
                stats.symbols += sheet.symbols.len();
            }
            Document::Pcb(pcb) => {
                stats.footprints += pcb.footprints.len();
                stats.pads += pcb.footprints.iter().map(|fp| fp.pads.len()).sum::<usize>();
                stats.nets += pcb.nets.len();
                stats.tracks += pcb.tracks.len();
                stats.vias += pcb.vias.len();
                stats.zones += pcb.zones.len();
                stats.copper_layers = stats.copper_layers.max(pcb.stackup.copper_count());
            }
            _ => {}
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_design_matches_its_spec() {
        let project = generate(DesignSize::S, DEFAULT_SEED);
        let stats = stats(&project);
        let spec = DesignSize::S.spec();
        assert_eq!(stats.symbols, spec.parts);
        assert_eq!(stats.footprints, spec.parts);
        assert_eq!(stats.nets, spec.nets);
        assert_eq!(stats.tracks, spec.track_segments);
        assert_eq!(stats.copper_layers, 2);
        assert_eq!(stats.sheets, 1);
        assert!(project.manifest.root_schematic.is_some());
    }

    #[test]
    fn medium_design_is_hierarchical() {
        let project = generate(DesignSize::M, 7);
        let stats = stats(&project);
        assert_eq!(stats.symbols, 500);
        assert_eq!(stats.sheets, 6);
        assert_eq!(stats.tracks, 20_000);
        assert_eq!(stats.zones, 2);
        assert_eq!(stats.copper_layers, 4);
        assert!(stats.vias > 0);
    }

    #[test]
    fn generation_is_deterministic() {
        let a = generate(DesignSize::S, 42);
        let b = generate(DesignSize::S, 42);
        let c = generate(DesignSize::S, 43);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn components_link_schematic_and_board() {
        let project = generate(DesignSize::S, 1);
        let mut symbol_components = Vec::new();
        let mut footprint_components = Vec::new();
        for document in project.documents.values() {
            match document {
                Document::Schematic(sheet) => {
                    symbol_components.extend(sheet.symbols.iter().filter_map(|s| s.component));
                }
                Document::Pcb(pcb) => {
                    footprint_components.extend(pcb.footprints.iter().filter_map(|f| f.component));
                }
                _ => {}
            }
        }
        symbol_components.sort();
        footprint_components.sort();
        assert_eq!(symbol_components, footprint_components);
    }

    #[test]
    fn staircase_routes() {
        let a = Point::mm(0, 0);
        let b = Point::mm(10, 5);
        for segments in 1..12 {
            let points = staircase(a, b, segments);
            assert_eq!(points.len(), segments + 1);
            assert_eq!(points[0], a);
            for pair in points.windows(2) {
                assert_ne!(pair[0], pair[1]);
                assert!(pair[0].x == pair[1].x || pair[0].y == pair[1].y);
            }
        }
        assert_eq!(*staircase(a, b, 2).last().unwrap(), b);
    }

    #[test]
    fn sizes_parse() {
        assert_eq!("xl".parse::<DesignSize>().unwrap(), DesignSize::Xl);
        assert!("huge".parse::<DesignSize>().is_err());
        assert_eq!(DesignSize::L.to_string(), "L");
        assert_eq!(rotate(Point::new(1, 2), Angle::DEG_90), Point::new(-2, 1));
    }
}
