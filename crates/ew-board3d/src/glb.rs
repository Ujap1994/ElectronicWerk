//! GLB files (binary glTF 2.0): reading the meshes of the mesh cache and
//! writing the board assembly.
//!
//! The mesh cache holds one GLB per model, in metres with y up (glTF's
//! frame), one primitive per colour, normals as bytes
//! (`KHR_mesh_quantization`). [`read`] turns such a file back into
//! millimetres with z up, the frame of the board.

use std::fmt::Write as _;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::place::Matrix;

const MAGIC: u32 = 0x4654_6C67;
const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;

/// A material: colour (RGBA, 0–1), metalness and roughness.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    /// Colour and opacity.
    pub color: [f32; 4],
    /// glTF metalness.
    pub metallic: f32,
    /// glTF roughness.
    pub roughness: f32,
}

impl Material {
    /// A plain, non-metallic colour.
    #[must_use]
    pub const fn plain(color: [f32; 4]) -> Self {
        Self {
            color,
            metallic: 0.0,
            roughness: 0.7,
        }
    }
}

/// Triangles of one material, in millimetres with z up.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    /// Material.
    pub material: Material,
    /// Vertex positions.
    pub positions: Vec<[f32; 3]>,
    /// Vertex normals (empty if the file has none).
    pub normals: Vec<[f32; 3]>,
    /// Triangles.
    pub indices: Vec<u32>,
}

/// An error reading a GLB file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlbError(pub String);

impl std::fmt::Display for GlbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GlbError {}

fn bad(message: &str) -> GlbError {
    GlbError(message.to_owned())
}

#[derive(Deserialize)]
struct Gltf {
    #[serde(default)]
    meshes: Vec<MeshJson>,
    #[serde(default)]
    accessors: Vec<Accessor>,
    #[serde(default, rename = "bufferViews")]
    buffer_views: Vec<View>,
    #[serde(default)]
    materials: Vec<MaterialJson>,
}

#[derive(Deserialize)]
struct MeshJson {
    primitives: Vec<Primitive>,
}

#[derive(Deserialize)]
struct Primitive {
    attributes: std::collections::HashMap<String, usize>,
    indices: Option<usize>,
    material: Option<usize>,
    #[serde(default = "triangles")]
    mode: u32,
}

const fn triangles() -> u32 {
    4
}

#[derive(Deserialize)]
struct Accessor {
    #[serde(rename = "bufferView")]
    buffer_view: Option<usize>,
    #[serde(default, rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "componentType")]
    component_type: u32,
    #[serde(default)]
    normalized: bool,
    count: usize,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
#[allow(clippy::struct_field_names, reason = "the names of the glTF fields")]
struct View {
    #[serde(default, rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "byteLength")]
    byte_length: usize,
    #[serde(rename = "byteStride")]
    byte_stride: Option<usize>,
}

#[derive(Deserialize, Default)]
struct MaterialJson {
    #[serde(default, rename = "pbrMetallicRoughness")]
    pbr: Option<Pbr>,
}

#[derive(Deserialize)]
struct Pbr {
    #[serde(rename = "baseColorFactor")]
    base: Option<[f32; 4]>,
    #[serde(rename = "metallicFactor")]
    metallic: Option<f32>,
    #[serde(rename = "roughnessFactor")]
    roughness: Option<f32>,
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, GlbError> {
    bytes
        .get(offset..offset + 4)
        .and_then(|b| b.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| bad("the GLB file is truncated"))
}

fn chunks(bytes: &[u8]) -> Result<(&[u8], &[u8]), GlbError> {
    if u32_at(bytes, 0)? != MAGIC || u32_at(bytes, 4)? != 2 {
        return Err(bad("not a glTF 2.0 binary file"));
    }
    let mut offset = 12;
    let (mut json, mut bin): (&[u8], &[u8]) = (&[], &[]);
    while offset + 8 <= bytes.len() {
        let length = u32_at(bytes, offset)? as usize;
        let kind = u32_at(bytes, offset + 4)?;
        let data = bytes
            .get(offset + 8..offset + 8 + length)
            .ok_or_else(|| bad("a GLB chunk is truncated"))?;
        match kind {
            CHUNK_JSON => json = data,
            CHUNK_BIN => bin = data,
            _ => {}
        }
        offset += 8 + length;
    }
    Ok((json, bin))
}

fn component_size(component: u32) -> Result<usize, GlbError> {
    Ok(match component {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        _ => return Err(bad("unknown component type")),
    })
}

fn element_count(kind: &str) -> Result<usize, GlbError> {
    Ok(match kind {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        _ => return Err(bad("unsupported accessor type")),
    })
}

/// Reads an accessor as floats (normalized integers scaled to −1…1 / 0…1).
fn read_floats(gltf: &Gltf, bin: &[u8], index: usize) -> Result<Vec<Vec<f32>>, GlbError> {
    let accessor = gltf
        .accessors
        .get(index)
        .ok_or_else(|| bad("missing accessor"))?;
    let view = accessor
        .buffer_view
        .and_then(|v| gltf.buffer_views.get(v))
        .ok_or_else(|| bad("an accessor has no buffer view"))?;
    let size = component_size(accessor.component_type)?;
    let count = element_count(&accessor.kind)?;
    let stride = view.byte_stride.unwrap_or(size * count);
    let mut out = Vec::with_capacity(accessor.count);
    for i in 0..accessor.count {
        let start = view.byte_offset + accessor.byte_offset + i * stride;
        if start + size * count > view.byte_offset + view.byte_length {
            return Err(bad("an accessor reads past its buffer view"));
        }
        let raw = bin
            .get(start..start + size * count)
            .ok_or_else(|| bad("an accessor reads past the binary chunk"))?;
        let values = (0..count)
            .map(|c| {
                let b = &raw[c * size..(c + 1) * size];
                match accessor.component_type {
                    5126 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]),
                    5120 => {
                        let v = f32::from(i8::from_le_bytes([b[0]]));
                        if accessor.normalized {
                            (v / 127.0).max(-1.0)
                        } else {
                            v
                        }
                    }
                    5121 => {
                        let v = f32::from(b[0]);
                        if accessor.normalized { v / 255.0 } else { v }
                    }
                    5122 => {
                        let v = f32::from(i16::from_le_bytes([b[0], b[1]]));
                        if accessor.normalized {
                            (v / 32_767.0).max(-1.0)
                        } else {
                            v
                        }
                    }
                    5123 => {
                        let v = f32::from(u16::from_le_bytes([b[0], b[1]]));
                        if accessor.normalized { v / 65_535.0 } else { v }
                    }
                    _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32,
                }
            })
            .collect();
        out.push(values);
    }
    Ok(out)
}

fn read_indices(gltf: &Gltf, bin: &[u8], index: usize) -> Result<Vec<u32>, GlbError> {
    let accessor = gltf
        .accessors
        .get(index)
        .ok_or_else(|| bad("missing index accessor"))?;
    let view = accessor
        .buffer_view
        .and_then(|v| gltf.buffer_views.get(v))
        .ok_or_else(|| bad("an accessor has no buffer view"))?;
    let size = component_size(accessor.component_type)?;
    let stride = view.byte_stride.unwrap_or(size);
    (0..accessor.count)
        .map(|i| {
            let start = view.byte_offset + accessor.byte_offset + i * stride;
            let b = bin
                .get(start..start + size)
                .ok_or_else(|| bad("an index reads past the binary chunk"))?;
            Ok(match size {
                1 => u32::from(b[0]),
                2 => u32::from(u16::from_le_bytes([b[0], b[1]])),
                _ => u32::from_le_bytes([b[0], b[1], b[2], b[3]]),
            })
        })
        .collect()
}

/// Reads the triangles of a GLB file in millimetres with z up.
///
/// # Errors
///
/// Fails if the file is not a GLB file with triangle meshes.
pub fn read(bytes: &[u8]) -> Result<Vec<Part>, GlbError> {
    let (json, bin) = chunks(bytes)?;
    let gltf: Gltf =
        serde_json::from_slice(json).map_err(|e| GlbError(format!("invalid glTF JSON: {e}")))?;
    let mut parts = Vec::new();
    for mesh in &gltf.meshes {
        for primitive in &mesh.primitives {
            if primitive.mode != 4 {
                continue;
            }
            let Some(&position) = primitive.attributes.get("POSITION") else {
                continue;
            };
            // m, y up → mm, z up: (x, y, z) → (x, -z, y) · 1000.
            let positions: Vec<[f32; 3]> = read_floats(&gltf, bin, position)?
                .into_iter()
                .map(|p| {
                    let g = |i: usize| p.get(i).copied().unwrap_or(0.0);
                    [g(0) * 1000.0, -g(2) * 1000.0, g(1) * 1000.0]
                })
                .collect();
            let normals: Vec<[f32; 3]> = match primitive.attributes.get("NORMAL") {
                Some(&normal) => read_floats(&gltf, bin, normal)?
                    .into_iter()
                    .map(|n| {
                        let g = |i: usize| n.get(i).copied().unwrap_or(0.0);
                        let (x, y, z) = (g(0), -g(2), g(1));
                        let length = (x * x + y * y + z * z).sqrt().max(1e-9);
                        [x / length, y / length, z / length]
                    })
                    .collect(),
                None => Vec::new(),
            };
            let indices = match primitive.indices {
                Some(index) => read_indices(&gltf, bin, index)?,
                None => (0..u32::try_from(positions.len()).unwrap_or(u32::MAX)).collect(),
            };
            if indices.iter().any(|&i| i as usize >= positions.len()) {
                return Err(bad("an index points past the vertices"));
            }
            let material = primitive
                .material
                .and_then(|m| gltf.materials.get(m))
                .and_then(|m| m.pbr.as_ref())
                .map_or(Material::plain([0.75, 0.75, 0.75, 1.0]), |pbr| Material {
                    color: pbr.base.unwrap_or([1.0; 4]),
                    metallic: pbr.metallic.unwrap_or(1.0),
                    roughness: pbr.roughness.unwrap_or(1.0),
                });
            parts.push(Part {
                material,
                positions,
                normals,
                indices,
            });
        }
    }
    Ok(parts)
}

/// One mesh to write: material and triangles in millimetres, z up.
#[derive(Debug, Clone, Copy)]
pub struct MeshRef<'a> {
    /// Name.
    pub name: &'a str,
    /// Material.
    pub material: Material,
    /// Positions.
    pub positions: &'a [[f32; 3]],
    /// Normals (same count as positions, or empty).
    pub normals: &'a [[f32; 3]],
    /// Triangles.
    pub indices: &'a [u32],
}

/// A node of the written scene: a group of meshes placed by a matrix (in
/// millimetres, z up).
#[derive(Debug, Clone)]
pub struct Node<'a> {
    /// Name (the designator of a part).
    pub name: String,
    /// Placement.
    pub matrix: Matrix,
    /// Index of the mesh group in the `groups` given to [`write`].
    pub group: usize,
    /// Marker for the borrowed groups.
    pub phantom: std::marker::PhantomData<&'a ()>,
}

/// Writes a GLB file: `groups` are lists of meshes (one per model and one
/// for the board), `nodes` place them. The file is in metres with y up.
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one glTF document: buffers, accessors, materials, meshes, nodes"
)]
pub fn write(groups: &[Vec<MeshRef<'_>>], nodes: &[Node<'_>], generator: &str) -> Vec<u8> {
    let mut bin: Vec<u8> = Vec::new();
    let mut views = Vec::new();
    let mut accessors = Vec::new();
    let mut materials: Vec<Value> = Vec::new();
    let mut material_keys: Vec<String> = Vec::new();
    let mut meshes = Vec::new();
    let mut add_view = |bin: &mut Vec<u8>, bytes: &[u8], target: u32| -> usize {
        let offset = bin.len();
        bin.extend_from_slice(bytes);
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        views.push(
            json!({"buffer": 0, "byteOffset": offset, "byteLength": bytes.len(), "target": target}),
        );
        views.len() - 1
    };
    for group in groups {
        let mut primitives = Vec::new();
        for mesh in group {
            if mesh.indices.is_empty() {
                continue;
            }
            let mut bytes = Vec::with_capacity(mesh.positions.len() * 12);
            let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
            for p in mesh.positions {
                // mm, z up → m, y up.
                let g = [p[0] / 1000.0, p[2] / 1000.0, -p[1] / 1000.0];
                for axis in 0..3 {
                    min[axis] = min[axis].min(g[axis]);
                    max[axis] = max[axis].max(g[axis]);
                    bytes.extend_from_slice(&g[axis].to_le_bytes());
                }
            }
            let view = add_view(&mut bin, &bytes, 34962);
            accessors.push(json!({"bufferView": view, "componentType": 5126, "count": mesh.positions.len(), "type": "VEC3", "min": min, "max": max}));
            let mut attributes = json!({"POSITION": accessors.len() - 1});
            if mesh.normals.len() == mesh.positions.len() {
                let mut bytes = Vec::with_capacity(mesh.normals.len() * 12);
                for n in mesh.normals {
                    for v in [n[0], n[2], -n[1]] {
                        bytes.extend_from_slice(&v.to_le_bytes());
                    }
                }
                let view = add_view(&mut bin, &bytes, 34962);
                accessors.push(json!({"bufferView": view, "componentType": 5126, "count": mesh.normals.len(), "type": "VEC3"}));
                attributes["NORMAL"] = json!(accessors.len() - 1);
            }
            let mut bytes = Vec::with_capacity(mesh.indices.len() * 4);
            for i in mesh.indices {
                bytes.extend_from_slice(&i.to_le_bytes());
            }
            let view = add_view(&mut bin, &bytes, 34963);
            accessors.push(json!({"bufferView": view, "componentType": 5125, "count": mesh.indices.len(), "type": "SCALAR"}));
            let m = mesh.material;
            let mut key = String::new();
            let _ = write!(
                key,
                "{:.3},{:.3},{:.3},{:.3},{:.2},{:.2}",
                m.color[0], m.color[1], m.color[2], m.color[3], m.metallic, m.roughness
            );
            let material = if let Some(index) = material_keys.iter().position(|k| *k == key) {
                index
            } else {
                let mut value = json!({
                    "pbrMetallicRoughness": {
                        "baseColorFactor": m.color,
                        "metallicFactor": m.metallic,
                        "roughnessFactor": m.roughness,
                    },
                    "doubleSided": false,
                });
                if m.color[3] < 1.0 {
                    value["alphaMode"] = json!("BLEND");
                }
                materials.push(value);
                material_keys.push(key);
                materials.len() - 1
            };
            primitives.push(
                json!({"attributes": attributes, "indices": accessors.len() - 1, "material": material}),
            );
        }
        meshes
            .push(json!({"name": group.first().map_or("", |m| m.name), "primitives": primitives}));
    }
    // Node matrices are given in mm with z up; the file is in m with y up:
    // N' = C · N · C⁻¹ with C = (x, z, -y) · 0.001 and its inverse.
    let c = Matrix([
        0.001, 0.0, 0.0, 0.0, 0.0, 0.0, -0.001, 0.0, 0.0, 0.001, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);
    let c_inv = Matrix([
        1000.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1000.0, 0.0, 0.0, -1000.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);
    let json_nodes: Vec<Value> = nodes
        .iter()
        .map(|node| {
            let m = c.mul(&node.matrix).mul(&c_inv);
            let mut value = json!({"name": node.name, "mesh": node.group});
            if m != Matrix::IDENTITY {
                value["matrix"] = json!(m.0);
            }
            value
        })
        .collect();
    let root: Vec<usize> = (0..json_nodes.len()).collect();
    let document = json!({
        "asset": {"version": "2.0", "generator": generator},
        "scene": 0,
        "scenes": [{"nodes": root}],
        "nodes": json_nodes,
        "meshes": meshes,
        "materials": materials,
        "accessors": accessors,
        "bufferViews": views,
        "buffers": [{"byteLength": bin.len()}],
    });
    let mut json_bytes = serde_json::to_vec(&document).unwrap_or_default();
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let total = 12 + 8 + json_bytes.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&2_u32.to_le_bytes());
    out.extend_from_slice(&u32::try_from(total).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(
        &u32::try_from(json_bytes.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&u32::try_from(bin.len()).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
    out.extend_from_slice(&bin);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_and_reads_back() {
        let positions = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 5.0]];
        let normals = [[0.0, 0.0, 1.0]; 3];
        let indices = [0, 1, 2];
        let mesh = MeshRef {
            name: "board",
            material: Material::plain([0.1, 0.5, 0.2, 0.8]),
            positions: &positions,
            normals: &normals,
            indices: &indices,
        };
        let nodes = [Node {
            name: "B".into(),
            matrix: Matrix::IDENTITY,
            group: 0,
            phantom: std::marker::PhantomData,
        }];
        let glb = write(&[vec![mesh]], &nodes, "test");
        let parts = read(&glb).unwrap();
        assert_eq!(parts.len(), 1);
        let part = &parts[0];
        for (got, want) in part.positions.iter().zip(&positions) {
            for axis in 0..3 {
                assert!((got[axis] - want[axis]).abs() < 1e-4, "{got:?} {want:?}");
            }
        }
        assert!((part.normals[0][2] - 1.0).abs() < 1e-6);
        assert_eq!(part.indices, indices);
        assert!((part.material.color[3] - 0.8).abs() < 1e-6);
        assert!(read(b"nope").is_err());
    }
}
