//! Exports of the 3D board: STEP ([`crate::step`]), GLB ([`crate::glb`])
//! and VRML ([`crate::vrml`]), with the same options and report.

use std::collections::HashMap;

use ew_model::pcb::Pcb;
use ew_output::art::Resolver;
use serde::{Deserialize, Serialize};

use crate::board::{self, Board3d, BoardOptions, LayerKind};
use crate::glb::{self, Material};
use crate::mesh::Mesh;
use crate::place::{self, Matrix, Part};
use crate::step::{self, ExportReport, ModelData, Origin, StepOptions};
use crate::vrml::{self, Units};

/// File format of a 3D export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    /// STEP AP214 (exact geometry for mechanical CAD).
    #[default]
    Step,
    /// Binary glTF (meshes for viewers, renderers and the web).
    Glb,
    /// VRML 2.0 (meshes, for older tools).
    Vrml,
}

impl Format {
    /// File extension.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Step => "step",
            Self::Glb => "glb",
            Self::Vrml => "wrl",
        }
    }
}

/// Options of a 3D export.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExportOptions {
    /// File format.
    pub format: Format,
    /// Origin of the model.
    pub origin: Origin,
    /// Also export parts marked "do not populate".
    pub include_dnp: bool,
    /// Units of VRML files.
    pub units: Units,
    /// File name (for the STEP header).
    pub name: String,
    /// Creation time (ISO 8601, empty for reproducible files).
    pub created: String,
    /// Application version.
    pub version: String,
}

/// Exports a board. `models` returns the data of a model path; for GLB and
/// VRML only meshes can be used.
#[must_use]
pub fn export(
    pcb: &Pcb,
    resolve: Resolver<'_>,
    models: &mut dyn FnMut(&str) -> Option<ModelData>,
    options: &ExportOptions,
) -> (Vec<u8>, ExportReport) {
    let built = board::build(pcb, resolve, &BoardOptions::default());
    let parts = place::parts(pcb, &built.levels);
    match options.format {
        Format::Step => {
            let (text, report) = step::export(
                pcb,
                &built,
                &parts,
                models,
                &StepOptions {
                    origin: options.origin,
                    include_dnp: options.include_dnp,
                    name: options.name.clone(),
                    created: options.created.clone(),
                    version: options.version.clone(),
                },
            );
            (text.into_bytes(), report)
        }
        Format::Glb | Format::Vrml => meshes(pcb, &built, &parts, models, options),
    }
}

/// The board's meshes with their materials (paste and inner copper are
/// left out).
fn board_shapes(board: &Board3d) -> Vec<(String, Material, &Mesh)> {
    let c = &board.colors;
    let copper = Material {
        color: [c.copper[0], c.copper[1], c.copper[2], 1.0],
        metallic: 0.8,
        roughness: 0.35,
    };
    let mut out = vec![(
        "board".to_owned(),
        Material::plain([c.body[0], c.body[1], c.body[2], 1.0]),
        &board.body,
    )];
    if !board.barrels.is_empty() {
        out.push(("barrels".to_owned(), copper, &board.barrels));
    }
    for layer in &board.layers {
        let material = match (layer.kind, layer.side) {
            (LayerKind::Copper, Some(_)) => copper,
            (LayerKind::Mask, Some(side)) => {
                let m = if side == ew_model::layer::Side::Top {
                    c.top_mask
                } else {
                    c.bottom_mask
                };
                Material {
                    color: [m[0], m[1], m[2], 0.88],
                    metallic: 0.0,
                    roughness: 0.4,
                }
            }
            (LayerKind::Silk, Some(side)) => {
                let s = if side == ew_model::layer::Side::Top {
                    c.top_silk
                } else {
                    c.bottom_silk
                };
                Material::plain([s[0], s[1], s[2], 1.0])
            }
            _ => continue,
        };
        out.push((layer.key(), material, &layer.mesh));
    }
    out
}

fn meshes(
    pcb: &Pcb,
    built: &Board3d,
    parts: &[Part],
    models: &mut dyn FnMut(&str) -> Option<ModelData>,
    options: &ExportOptions,
) -> (Vec<u8>, ExportReport) {
    let mut report = ExportReport::default();
    let shift = step::origin_shift(pcb, built, options.origin);
    let to_origin = Matrix::translation(shift[0], shift[1], 0.0);
    // Each model once.
    let mut groups: Vec<Vec<glb::Part>> = Vec::new();
    let mut index: HashMap<String, Option<usize>> = HashMap::new();
    let mut instances: Vec<(String, usize, Matrix)> = Vec::new();
    for part in parts {
        if part.dnp && !options.include_dnp {
            continue;
        }
        let mut placed = false;
        for model in &part.models {
            let group =
                *index
                    .entry(model.path.clone())
                    .or_insert_with(|| match models(&model.path) {
                        Some(ModelData::Mesh(mesh)) => {
                            groups.push(mesh);
                            report.faceted += 1;
                            Some(groups.len() - 1)
                        }
                        Some(ModelData::Step(_)) => {
                            report.warnings.push(format!(
                                "{}: no mesh of this STEP model yet (open the 3D view once)",
                                model.path
                            ));
                            None
                        }
                        None => {
                            report.missing.push(model.path.clone());
                            None
                        }
                    });
            if let Some(group) = group {
                instances.push((part.designator.clone(), group, model.matrix));
                placed = true;
            }
        }
        if placed {
            report.placed += 1;
        }
    }
    let shapes = board_shapes(built);
    let bytes = if options.format == Format::Glb {
        write_glb(
            pcb,
            &shapes,
            &groups,
            &instances,
            to_origin,
            &options.version,
        )
    } else {
        write_vrml(pcb, &shapes, &groups, &instances, shift, options.units)
    };
    report.exact_body = false;
    (bytes, report)
}

type Instances = [(String, usize, Matrix)];

fn write_glb(
    pcb: &Pcb,
    shapes: &[(String, Material, &Mesh)],
    groups: &[Vec<glb::Part>],
    instances: &Instances,
    to_origin: Matrix,
    version: &str,
) -> Vec<u8> {
    let mut all: Vec<Vec<glb::MeshRef<'_>>> = vec![
        shapes
            .iter()
            .map(|(name, material, mesh)| glb::MeshRef {
                name,
                material: *material,
                positions: &mesh.positions,
                normals: &mesh.normals,
                indices: &mesh.indices,
            })
            .collect(),
    ];
    for group in groups {
        all.push(
            group
                .iter()
                .map(|part| glb::MeshRef {
                    name: "",
                    material: part.material,
                    positions: &part.positions,
                    normals: &part.normals,
                    indices: &part.indices,
                })
                .collect(),
        );
    }
    let mut nodes = vec![glb::Node {
        name: format!("{}_PCB", pcb.name),
        matrix: to_origin,
        group: 0,
        phantom: std::marker::PhantomData,
    }];
    nodes.extend(instances.iter().map(|(name, group, matrix)| glb::Node {
        name: name.clone(),
        matrix: to_origin.mul(matrix),
        group: group + 1,
        phantom: std::marker::PhantomData,
    }));
    glb::write(&all, &nodes, &format!("ElectronicWerk {version}"))
}

fn write_vrml(
    pcb: &Pcb,
    shapes: &[(String, Material, &Mesh)],
    groups: &[Vec<glb::Part>],
    instances: &Instances,
    shift: [f64; 2],
    units: Units,
) -> Vec<u8> {
    let board: Vec<vrml::Shape<'_>> = shapes
        .iter()
        .map(|(_, material, mesh)| vrml::Shape {
            material: *material,
            positions: &mesh.positions,
            normals: &mesh.normals,
            indices: &mesh.indices,
        })
        .collect();
    let groups: Vec<Vec<vrml::Shape<'_>>> = groups
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|part| vrml::Shape {
                    material: part.material,
                    positions: &part.positions,
                    normals: &part.normals,
                    indices: &part.indices,
                })
                .collect()
        })
        .collect();
    let instances: Vec<vrml::Instance> = instances
        .iter()
        .map(|(name, group, matrix)| vrml::Instance {
            name: name.clone(),
            group: *group,
            matrix: *matrix,
        })
        .collect();
    vrml::write(&board, &groups, &instances, shift, units, &pcb.name).into_bytes()
}
