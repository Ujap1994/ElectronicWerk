//! STEP export of the board assembly (ISO 10303-21, AP214).
//!
//! The file has the structure Open CASCADE writes: an assembly product for
//! the board, the board body as a product with an exact B-rep
//! ([`body`]), and one product per 3D model, placed once per part by a
//! `NEXT_ASSEMBLY_USAGE_OCCURRENCE` with an `ITEM_DEFINED_TRANSFORMATION`.
//! STEP models are merged in as they are ([`part21`] renumbers their
//! entities); models that only exist as meshes (the base library's packs)
//! become faceted surface models.

pub mod body;
pub mod part21;

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use ew_model::pcb::Pcb;

use crate::board::Board3d;
use crate::glb;
use crate::mesh::Polygons;
use crate::place::{Matrix, Part};

/// The model of a part, as the exporter receives it.
#[derive(Debug, Clone)]
pub enum ModelData {
    /// The text of a STEP file.
    Step(String),
    /// Triangles (mm, z up), from the mesh cache.
    Mesh(Vec<glb::Part>),
}

/// Where the exported model has its origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Origin {
    /// The board's drill and placement origin (as the placement files).
    #[default]
    Drill,
    /// The centre of the board.
    Center,
    /// The board coordinates as they are.
    Absolute,
}

/// Options of the STEP export.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StepOptions {
    /// Origin of the model.
    pub origin: Origin,
    /// Also export parts marked "do not populate".
    pub include_dnp: bool,
    /// File name (for the header).
    pub name: String,
    /// Creation time (ISO 8601, empty for reproducible files).
    pub created: String,
    /// The application version.
    pub version: String,
}

/// What the export did.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    /// Parts with a model in the file.
    pub placed: usize,
    /// Model files merged as exact STEP geometry.
    pub exact: usize,
    /// Models written as faceted surfaces (only a mesh was found).
    pub faceted: usize,
    /// Model paths that were not found (not exported).
    pub missing: Vec<String>,
    /// Further notes.
    pub warnings: Vec<String>,
    /// `true` if the board body uses the exact outline and holes.
    pub exact_body: bool,
}

/// Collects the entity records of a STEP file.
#[derive(Debug, Default)]
pub struct Writer {
    records: BTreeMap<u64, String>,
    next: u64,
    cache: HashMap<String, u64>,
}

impl Writer {
    fn new() -> Self {
        Self {
            records: BTreeMap::new(),
            next: 1,
            cache: HashMap::new(),
        }
    }

    /// Adds a record; returns its instance number.
    pub fn add(&mut self, body: String) -> u64 {
        let id = self.next;
        self.next += 1;
        self.records.insert(id, body);
        id
    }

    /// Adds a record once (later calls with the same body return the first
    /// one), for points and directions that repeat.
    pub fn shared(&mut self, body: String) -> u64 {
        if let Some(id) = self.cache.get(&body) {
            return *id;
        }
        let id = self.add(body.clone());
        self.cache.insert(body, id);
        id
    }

    /// Reserves a number for a record written later with [`Writer::set`].
    pub fn reserve(&mut self) -> u64 {
        let id = self.next;
        self.next += 1;
        id
    }

    /// Writes a reserved record.
    pub fn set(&mut self, id: u64, body: String) {
        self.records.insert(id, body);
    }

    /// Reserves `count` numbers after the current ones; returns the offset
    /// to add to numbers `1..=count`.
    fn block(&mut self, count: u64) -> u64 {
        let offset = self.next - 1;
        self.next += count;
        offset
    }

    fn finish(self, header: &str) -> String {
        let mut out = String::with_capacity(self.records.len() * 48 + header.len() + 64);
        out.push_str(header);
        out.push_str("DATA;\n");
        for (id, body) in &self.records {
            let _ = writeln!(out, "#{id} = {body};");
        }
        out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
        out
    }
}

/// A STEP real number (always with a decimal point).
#[must_use]
pub fn real(value: f64) -> String {
    let value = if value.abs() < 1e-12 { 0.0 } else { value };
    let mut text = format!("{value:.9}");
    while text.ends_with('0') {
        text.pop();
    }
    if text == "-0." {
        text = "0.".into();
    }
    text
}

/// A `CARTESIAN_POINT` (shared).
pub fn point(w: &mut Writer, p: [f64; 3]) -> u64 {
    w.shared(format!(
        "CARTESIAN_POINT('',({},{},{}))",
        real(p[0]),
        real(p[1]),
        real(p[2])
    ))
}

/// A `DIRECTION` (shared).
pub fn direction(w: &mut Writer, d: [f64; 3]) -> u64 {
    w.shared(format!(
        "DIRECTION('',({},{},{}))",
        real(d[0]),
        real(d[1]),
        real(d[2])
    ))
}

fn text(value: &str) -> String {
    value.replace('\'', "''").replace('\\', "\\\\")
}

fn ascii(value: &str) -> String {
    // Part 21 strings are ISO 8859-1 without escapes; keep names simple.
    value
        .chars()
        .map(|c| {
            if c.is_ascii() && !c.is_ascii_control() {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// The shared parts of every representation.
struct Common {
    context: u64,
    application: u64,
    origin: u64,
}

fn common(w: &mut Writer) -> Common {
    let application =
        w.add("APPLICATION_CONTEXT('core data for automotive mechanical design processes')".into());
    w.add(format!(
        "APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{application})"
    ));
    let length = w.add("( LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.) )".into());
    let angle = w.add("( NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.) )".into());
    let solid = w.add("( NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT() )".into());
    let uncertainty = w.add(format!(
        "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-07),#{length},'distance_accuracy_value','confusion accuracy')"
    ));
    let context = w.add(format!(
        "( GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{uncertainty})) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{length},#{angle},#{solid})) REPRESENTATION_CONTEXT('Context #1','3D Context with UNIT and UNCERTAINTY') )"
    ));
    let o = point(w, [0.0, 0.0, 0.0]);
    let z = direction(w, [0.0, 0.0, 1.0]);
    let x = direction(w, [1.0, 0.0, 0.0]);
    let origin = w.add(format!("AXIS2_PLACEMENT_3D('',#{o},#{z},#{x})"));
    Common {
        context,
        application,
        origin,
    }
}

/// A product with its definition; returns the `PRODUCT_DEFINITION`.
fn product(w: &mut Writer, c: &Common, name: &str, representation: u64) -> u64 {
    let name = text(&ascii(name));
    let product_context = w.add(format!(
        "PRODUCT_CONTEXT('',#{},'mechanical')",
        c.application
    ));
    let product = w.add(format!(
        "PRODUCT('{name}','{name}','',(#{product_context}))"
    ));
    w.add(format!(
        "PRODUCT_RELATED_PRODUCT_CATEGORY('part',$,(#{product}))"
    ));
    let formation = w.add(format!("PRODUCT_DEFINITION_FORMATION('','',#{product})"));
    let definition_context = w.add(format!(
        "PRODUCT_DEFINITION_CONTEXT('part definition',#{},'design')",
        c.application
    ));
    let definition = w.add(format!(
        "PRODUCT_DEFINITION('design','',#{formation},#{definition_context})"
    ));
    let shape = w.add(format!("PRODUCT_DEFINITION_SHAPE('','',#{definition})"));
    w.add(format!(
        "SHAPE_DEFINITION_REPRESENTATION(#{shape},#{representation})"
    ));
    definition
}

/// A colour style for `item`; returns the `STYLED_ITEM`.
fn styled(w: &mut Writer, item: u64, color: [f32; 3]) -> u64 {
    let rgb = w.shared(format!(
        "COLOUR_RGB('',{},{},{})",
        real(f64::from(color[0])),
        real(f64::from(color[1])),
        real(f64::from(color[2]))
    ));
    let fill_colour = w.add(format!("FILL_AREA_STYLE_COLOUR('',#{rgb})"));
    let fill = w.add(format!("FILL_AREA_STYLE('',(#{fill_colour}))"));
    let area = w.add(format!("SURFACE_STYLE_FILL_AREA(#{fill})"));
    let side = w.add(format!("SURFACE_SIDE_STYLE('',(#{area}))"));
    let usage = w.add(format!("SURFACE_STYLE_USAGE(.BOTH.,#{side})"));
    let assignment = w.add(format!("PRESENTATION_STYLE_ASSIGNMENT((#{usage}))"));
    w.add(format!("STYLED_ITEM('color',(#{assignment}),#{item})"))
}

/// An axis placement for a rigid matrix.
fn placement(w: &mut Writer, m: &Matrix) -> u64 {
    let location = point(w, [m.at(0, 3), m.at(1, 3), m.at(2, 3)]);
    let z = m.apply_direction([0.0, 0.0, 1.0]);
    let x = m.apply_direction([1.0, 0.0, 0.0]);
    let z = direction(w, z);
    let x = direction(w, x);
    w.add(format!("AXIS2_PLACEMENT_3D('',#{location},#{z},#{x})"))
}

/// Places `child` (a product definition with its representation) into the
/// assembly.
#[allow(clippy::too_many_arguments, reason = "the parts of one STEP link")]
fn link(
    w: &mut Writer,
    c: &Common,
    parent: (u64, u64),
    child: (u64, u64),
    matrix: &Matrix,
    number: usize,
    name: &str,
    axes: &mut Vec<u64>,
) {
    let axis = placement(w, matrix);
    axes.push(axis);
    let transformation = w.add(format!(
        "ITEM_DEFINED_TRANSFORMATION('','',#{},#{axis})",
        c.origin
    ));
    let relationship = w.add(format!(
        "( REPRESENTATION_RELATIONSHIP('','',#{},#{}) REPRESENTATION_RELATIONSHIP_WITH_TRANSFORMATION(#{transformation}) SHAPE_REPRESENTATION_RELATIONSHIP() )",
        child.1, parent.1
    ));
    let name = text(&ascii(name));
    let usage = w.add(format!(
        "NEXT_ASSEMBLY_USAGE_OCCURRENCE('{number}','{name}','',#{},#{},$)",
        parent.0, child.0
    ));
    let shape = w.add(format!(
        "PRODUCT_DEFINITION_SHAPE('Placement','Placement of an item',#{usage})"
    ));
    w.add(format!(
        "CONTEXT_DEPENDENT_SHAPE_REPRESENTATION(#{relationship},#{shape})"
    ));
}

/// Merges a component STEP file; returns its product and representation
/// (an extra assembly level if the file has several root products).
fn merge(w: &mut Writer, c: &Common, file: &part21::StepFile, name: &str) -> Option<(u64, u64)> {
    let roots = file.roots();
    if roots.is_empty() {
        return None;
    }
    let max = file.max_id();
    let offset = w.block(max);
    let contexts: Vec<u64> = file
        .records
        .iter()
        .filter(|r| r.kind() == "APPLICATION_CONTEXT")
        .map(|r| r.id)
        .collect();
    for record in &file.records {
        match record.kind() {
            // One application context and protocol for the whole file.
            "APPLICATION_CONTEXT" | "APPLICATION_PROTOCOL_DEFINITION" => continue,
            _ => {}
        }
        let body = part21::rewrite(&record.body, &mut |id| {
            if contexts.contains(&id) {
                c.application
            } else {
                id + offset
            }
        });
        w.set(record.id + offset, body);
    }
    let shifted: Vec<(u64, u64)> = roots
        .iter()
        .map(|root| (root.definition + offset, root.representation + offset))
        .collect();
    if let [single] = shifted.as_slice() {
        return Some(*single);
    }
    // Several roots: one more product that holds them all.
    let representation = w.reserve();
    let definition = product(w, c, name, representation);
    let mut axes = vec![c.origin];
    for (index, root) in shifted.iter().enumerate() {
        link(
            w,
            c,
            (definition, representation),
            *root,
            &Matrix::IDENTITY,
            index + 1,
            name,
            &mut axes,
        );
    }
    let items = axes
        .iter()
        .map(|a| format!("#{a}"))
        .collect::<Vec<_>>()
        .join(",");
    w.set(
        representation,
        format!("SHAPE_REPRESENTATION('',({items}),#{})", c.context),
    );
    Some((definition, representation))
}

/// Writes triangles as faceted surface models (one per colour); returns the
/// product and representation.
fn faceted(w: &mut Writer, c: &Common, parts: &[glb::Part], name: &str) -> Option<(u64, u64)> {
    let mut models = Vec::new();
    let mut styles = Vec::new();
    for part in parts {
        let mut faces = Vec::with_capacity(part.indices.len() / 3);
        let mut points: HashMap<[i64; 3], u64> = HashMap::new();
        let mut point_of = |w: &mut Writer, p: [f32; 3]| -> u64 {
            let key = p.map(|v| (f64::from(v) * 1e4).round() as i64);
            *points.entry(key).or_insert_with(|| {
                w.add(format!(
                    "CARTESIAN_POINT('',({},{},{}))",
                    real(f64::from(p[0])),
                    real(f64::from(p[1])),
                    real(f64::from(p[2]))
                ))
            })
        };
        for tri in part.indices.chunks_exact(3) {
            let [a, b, cc] = [tri[0], tri[1], tri[2]].map(|i| part.positions[i as usize]);
            let e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]].map(f64::from);
            let e2 = [cc[0] - a[0], cc[1] - a[1], cc[2] - a[2]].map(f64::from);
            let n = [
                e1[1] * e2[2] - e1[2] * e2[1],
                e1[2] * e2[0] - e1[0] * e2[2],
                e1[0] * e2[1] - e1[1] * e2[0],
            ];
            let nl = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            let el = (e1[0] * e1[0] + e1[1] * e1[1] + e1[2] * e1[2]).sqrt();
            if nl < 1e-12 || el < 1e-9 {
                continue;
            }
            let pa = point_of(w, a);
            let pb = point_of(w, b);
            let pc = point_of(w, cc);
            if pa == pb || pb == pc || pa == pc {
                continue;
            }
            let normal = w.add(format!(
                "DIRECTION('',({},{},{}))",
                real(n[0] / nl),
                real(n[1] / nl),
                real(n[2] / nl)
            ));
            let reference = w.add(format!(
                "DIRECTION('',({},{},{}))",
                real(e1[0] / el),
                real(e1[1] / el),
                real(e1[2] / el)
            ));
            let axis = w.add(format!(
                "AXIS2_PLACEMENT_3D('',#{pa},#{normal},#{reference})"
            ));
            let plane = w.add(format!("PLANE('',#{axis})"));
            let poly = w.add(format!("POLY_LOOP('',(#{pa},#{pb},#{pc}))"));
            let bound = w.add(format!("FACE_OUTER_BOUND('',#{poly},.T.)"));
            faces.push(w.add(format!("FACE_SURFACE('',(#{bound}),#{plane},.T.)")));
        }
        if faces.is_empty() {
            continue;
        }
        let list = faces
            .iter()
            .map(|f| format!("#{f}"))
            .collect::<Vec<_>>()
            .join(",");
        let shell = w.add(format!("OPEN_SHELL('',({list}))"));
        let model = w.add(format!("SHELL_BASED_SURFACE_MODEL('',(#{shell}))"));
        let [r, g, b, _] = part.material.color;
        styles.push(styled(w, model, [r, g, b]));
        models.push(model);
    }
    if models.is_empty() {
        return None;
    }
    let items = std::iter::once(c.origin)
        .chain(models.iter().copied())
        .map(|m| format!("#{m}"))
        .collect::<Vec<_>>()
        .join(",");
    let representation = w.add(format!(
        "MANIFOLD_SURFACE_SHAPE_REPRESENTATION('',({items}),#{})",
        c.context
    ));
    let list = styles
        .iter()
        .map(|s| format!("#{s}"))
        .collect::<Vec<_>>()
        .join(",");
    w.add(format!(
        "MECHANICAL_DESIGN_GEOMETRIC_PRESENTATION_REPRESENTATION('',({list}),#{})",
        c.context
    ));
    let definition = product(w, c, name, representation);
    Some((definition, representation))
}

/// The shift that moves the board to the chosen origin (mm).
#[must_use]
pub fn origin_shift(pcb: &Pcb, board: &Board3d, origin: Origin) -> [f64; 2] {
    match origin {
        Origin::Absolute => [0.0, 0.0],
        Origin::Drill => [-(pcb.origin.x as f64) / 1e6, -(pcb.origin.y as f64) / 1e6],
        Origin::Center => [
            -f64::midpoint(board.bounds[0], board.bounds[3]),
            -f64::midpoint(board.bounds[1], board.bounds[4]),
        ],
    }
}

/// The model file name of a path (for product names).
fn model_name(path: &str) -> String {
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    file.rsplit_once('.')
        .map_or(file, |(stem, _)| stem)
        .to_owned()
}

/// Writes the STEP file of a board assembly. `models` returns the data of a
/// model path (`None` if it was not found).
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "the assembly in file order: body, models, placements, header"
)]
pub fn export(
    pcb: &Pcb,
    board: &Board3d,
    parts: &[Part],
    models: &mut dyn FnMut(&str) -> Option<ModelData>,
    options: &StepOptions,
) -> (String, ExportReport) {
    let mut report = ExportReport::default();
    let mut w = Writer::new();
    let c = common(&mut w);
    let shift = origin_shift(pcb, board, options.origin);
    let to_origin = Matrix::translation(shift[0], shift[1], 0.0);

    // The assembly (its representation lists the placements; written last).
    let top_representation = w.reserve();
    let top = product(&mut w, &c, &pcb.name, top_representation);
    let mut axes = vec![c.origin];
    let mut number = 0_usize;

    // The board body.
    let drilled = drilled_polygons(board);
    let (loops, exact) = body::body_loops(pcb, &board.holes, &board.area, &drilled);
    report.exact_body = exact;
    if !loops.is_empty() {
        let solid = body::write_solid(&mut w, &loops, board.levels.body_top);
        let representation = w.add(format!(
            "ADVANCED_BREP_SHAPE_REPRESENTATION('',(#{},#{solid}),#{})",
            c.origin, c.context
        ));
        let style = styled(&mut w, solid, board.colors.top_mask);
        w.add(format!(
            "MECHANICAL_DESIGN_GEOMETRIC_PRESENTATION_REPRESENTATION('',(#{style}),#{})",
            c.context
        ));
        let definition = product(&mut w, &c, &format!("{}_PCB", pcb.name), representation);
        number += 1;
        link(
            &mut w,
            &c,
            (top, top_representation),
            (definition, representation),
            &to_origin,
            number,
            &format!("{}_PCB", pcb.name),
            &mut axes,
        );
    }

    // Components: each model once, placed per part.
    let mut written: HashMap<String, Option<(u64, u64)>> = HashMap::new();
    for part in parts {
        if part.dnp && !options.include_dnp {
            continue;
        }
        let mut placed = false;
        for model in &part.models {
            let rigid = model.matrix.is_rigid(1e-6);
            let key = if rigid {
                model.path.clone()
            } else {
                // Scaled models are baked into their own faceted copy.
                format!("{}#{}", model.path, part.designator)
            };
            let entry = if let Some(entry) = written.get(&key) {
                *entry
            } else {
                let made = match models(&model.path) {
                    Some(ModelData::Step(content)) if rigid => match part21::parse(&content) {
                        Ok(file) => {
                            let made = merge(&mut w, &c, &file, &model_name(&model.path));
                            if made.is_some() {
                                report.exact += 1;
                            }
                            made
                        }
                        Err(error) => {
                            report.warnings.push(format!("{}: {error}", model.path));
                            None
                        }
                    },
                    Some(ModelData::Mesh(mesh)) => {
                        let mesh = if rigid {
                            mesh
                        } else {
                            bake(&mesh, &model.matrix)
                        };
                        let made = faceted(&mut w, &c, &mesh, &model_name(&model.path));
                        if made.is_some() {
                            report.faceted += 1;
                        }
                        made
                    }
                    Some(ModelData::Step(_)) => {
                        report.warnings.push(format!(
                            "{}: scaled STEP models cannot be placed exactly; left out of {}",
                            model.path, part.designator
                        ));
                        None
                    }
                    None => {
                        if !report.missing.contains(&model.path) {
                            report.missing.push(model.path.clone());
                        }
                        None
                    }
                };
                written.insert(key, made);
                made
            };
            let Some(child) = entry else {
                continue;
            };
            let matrix = if rigid {
                to_origin.mul(&model.matrix)
            } else {
                to_origin
            };
            number += 1;
            link(
                &mut w,
                &c,
                (top, top_representation),
                child,
                &matrix,
                number,
                &part.designator,
                &mut axes,
            );
            placed = true;
        }
        if placed {
            report.placed += 1;
        }
    }
    let items = axes
        .iter()
        .map(|a| format!("#{a}"))
        .collect::<Vec<_>>()
        .join(",");
    w.set(
        top_representation,
        format!("SHAPE_REPRESENTATION('',({items}),#{})", c.context),
    );
    let name = if options.name.is_empty() {
        format!("{}.step", pcb.name)
    } else {
        options.name.clone()
    };
    let header = format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('ElectronicWerk board assembly'),'2;1');\nFILE_NAME('{}','{}',('ElectronicWerk'),('ElectronicWerk'),'ElectronicWerk {}','ElectronicWerk','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN {{ 1 0 10303 214 1 1 1 1 }}'));\nENDSEC;\n",
        text(&ascii(&name)),
        text(&options.created),
        text(&options.version)
    );
    (w.finish(&header), report)
}

/// The board area minus the holes (the fallback body).
fn drilled_polygons(board: &Board3d) -> Polygons {
    let holes: Polygons = board
        .holes
        .iter()
        .map(|hole| vec![crate::board::hole_polygon(hole, 2_000.0)])
        .collect();
    ew_pcb::poly::difference(&board.area, &holes)
}

/// A mesh with a (scaling) matrix applied.
fn bake(mesh: &[glb::Part], matrix: &Matrix) -> Vec<glb::Part> {
    mesh.iter()
        .map(|part| glb::Part {
            material: part.material,
            positions: part
                .positions
                .iter()
                .map(|p| {
                    let q = matrix.apply(p.map(f64::from));
                    [q[0] as f32, q[1] as f32, q[2] as f32]
                })
                .collect(),
            normals: Vec::new(),
            indices: part.indices.clone(),
        })
        .collect()
}
