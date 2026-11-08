//! Fingerprints of 3D meshes in GLB files (binary glTF 2.0).
//!
//! The mesh cache stores one GLB per model, written by the desktop app's
//! tessellator (`apps/desktop/src/shared/mesh`): metres, y up, one mesh with
//! one primitive per colour, no node transforms. The fingerprint hashes the
//! triangles with their corners rounded to 1 µm, independent of the vertex
//! order and of how the triangles are split into primitives, so a model gets
//! the same fingerprint whatever its file name or source.

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::error::LibraryError;
use crate::fingerprint::hex16;

const MAGIC: u32 = 0x4654_6C67;
const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;

/// The fingerprint of a mesh.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshPrint {
    /// The triangles only.
    pub geometry: String,
    /// The triangles with their colours.
    pub colored: String,
    /// Bounding box in µm, z up like the board: `[x0, y0, z0, x1, y1, z1]`.
    pub bbox: [i64; 6],
    /// Number of triangles.
    pub triangles: u64,
}

#[derive(Deserialize)]
struct Gltf {
    #[serde(default)]
    meshes: Vec<Mesh>,
    #[serde(default)]
    accessors: Vec<Accessor>,
    #[serde(default, rename = "bufferViews")]
    buffer_views: Vec<BufferView>,
    #[serde(default)]
    materials: Vec<Material>,
}

#[derive(Deserialize)]
struct Mesh {
    primitives: Vec<Primitive>,
}

#[derive(Deserialize)]
struct Primitive {
    attributes: Attributes,
    indices: Option<usize>,
    material: Option<usize>,
    #[serde(default = "triangles_mode")]
    mode: u32,
}

const fn triangles_mode() -> u32 {
    4
}

#[derive(Deserialize)]
struct Attributes {
    #[serde(rename = "POSITION")]
    position: usize,
}

#[derive(Deserialize)]
struct Accessor {
    #[serde(rename = "bufferView")]
    buffer_view: Option<usize>,
    #[serde(default, rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "componentType")]
    component_type: u32,
    count: usize,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
#[allow(clippy::struct_field_names, reason = "the names of the glTF fields")]
struct BufferView {
    #[serde(default, rename = "byteOffset")]
    byte_offset: usize,
    #[serde(rename = "byteLength")]
    byte_length: usize,
    #[serde(rename = "byteStride")]
    byte_stride: Option<usize>,
}

#[derive(Deserialize, Default)]
struct Material {
    #[serde(default, rename = "pbrMetallicRoughness")]
    pbr: Pbr,
}

#[derive(Deserialize)]
struct Pbr {
    #[serde(default = "white", rename = "baseColorFactor")]
    base_color: [f64; 4],
}

impl Default for Pbr {
    fn default() -> Self {
        Self {
            base_color: white(),
        }
    }
}

const fn white() -> [f64; 4] {
    [1.0, 1.0, 1.0, 1.0]
}

fn bad(message: impl Into<String>) -> LibraryError {
    LibraryError::Format(message.into())
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, LibraryError> {
    bytes
        .get(offset..offset + 4)
        .and_then(|b| b.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| bad("the GLB file is truncated"))
}

/// Splits a GLB file into its JSON and binary chunks.
fn chunks(bytes: &[u8]) -> Result<(&[u8], &[u8]), LibraryError> {
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

/// The bytes of element `index` of an accessor.
fn element<'a>(
    gltf: &Gltf,
    bin: &'a [u8],
    accessor: &Accessor,
    index: usize,
    size: usize,
) -> Result<&'a [u8], LibraryError> {
    let view = accessor
        .buffer_view
        .and_then(|v| gltf.buffer_views.get(v))
        .ok_or_else(|| bad("an accessor has no buffer view"))?;
    let stride = view.byte_stride.unwrap_or(size);
    let start = view.byte_offset + accessor.byte_offset + index * stride;
    if start + size > view.byte_offset + view.byte_length {
        return Err(bad("an accessor reads past its buffer view"));
    }
    bin.get(start..start + size)
        .ok_or_else(|| bad("a buffer view reads past the binary chunk"))
}

fn positions(gltf: &Gltf, bin: &[u8], accessor: &Accessor) -> Result<Vec<[i64; 3]>, LibraryError> {
    if accessor.component_type != 5126 || accessor.kind != "VEC3" {
        return Err(bad("positions must be float VEC3"));
    }
    (0..accessor.count)
        .map(|index| {
            let raw = element(gltf, bin, accessor, index, 12)?;
            let mut out = [0_i64; 3];
            for (axis, value) in out.iter_mut().enumerate() {
                let bytes: [u8; 4] = raw[axis * 4..axis * 4 + 4]
                    .try_into()
                    .map_err(|_| bad("bad position"))?;
                // Metres → µm.
                *value = (f64::from(f32::from_le_bytes(bytes)) * 1e6).round() as i64;
            }
            Ok(out)
        })
        .collect()
}

fn indices(gltf: &Gltf, bin: &[u8], accessor: &Accessor) -> Result<Vec<usize>, LibraryError> {
    let size = match accessor.component_type {
        5121 => 1,
        5123 => 2,
        5125 => 4,
        _ => return Err(bad("indices must be unsigned integers")),
    };
    (0..accessor.count)
        .map(|index| {
            let raw = element(gltf, bin, accessor, index, size)?;
            Ok(match size {
                1 => usize::from(raw[0]),
                2 => usize::from(u16::from_le_bytes([raw[0], raw[1]])),
                _ => u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize,
            })
        })
        .collect()
}

/// Reads a GLB file and computes its fingerprint.
///
/// # Errors
///
/// Fails if the file is not a GLB file with triangle meshes.
pub fn read_glb(bytes: &[u8]) -> Result<MeshPrint, LibraryError> {
    let (json, bin) = chunks(bytes)?;
    let gltf: Gltf =
        serde_json::from_slice(json).map_err(|error| bad(format!("invalid glTF JSON: {error}")))?;
    let mut triangles: Vec<([[i64; 3]; 3], [u8; 4])> = Vec::new();
    for mesh in &gltf.meshes {
        for primitive in &mesh.primitives {
            if primitive.mode != 4 {
                return Err(bad("only triangle meshes are supported"));
            }
            let accessor = gltf
                .accessors
                .get(primitive.attributes.position)
                .ok_or_else(|| bad("missing position accessor"))?;
            let points = positions(&gltf, bin, accessor)?;
            let order = match primitive.indices {
                Some(index) => indices(
                    &gltf,
                    bin,
                    gltf.accessors
                        .get(index)
                        .ok_or_else(|| bad("missing index accessor"))?,
                )?,
                None => (0..points.len()).collect(),
            };
            let color = primitive
                .material
                .and_then(|m| gltf.materials.get(m))
                .map_or_else(white, |m| m.pbr.base_color)
                .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
            for corner in order.chunks_exact(3) {
                let mut tri = [[0_i64; 3]; 3];
                for (slot, &index) in tri.iter_mut().zip(corner) {
                    *slot = *points
                        .get(index)
                        .ok_or_else(|| bad("an index points past the vertices"))?;
                }
                // Same triangle, same winding: start at the smallest corner.
                let first = (0..3).min_by_key(|&i| tri[i]).unwrap_or(0);
                tri.rotate_left(first);
                triangles.push((tri, color));
            }
        }
    }
    triangles.sort_unstable();
    let mut geometry = Sha256::new();
    let mut colored = Sha256::new();
    let mut bbox = [i64::MAX, i64::MAX, i64::MAX, i64::MIN, i64::MIN, i64::MIN];
    for (tri, color) in &triangles {
        for corner in tri {
            for value in corner {
                geometry.update(value.to_le_bytes());
                colored.update(value.to_le_bytes());
            }
            // y up (glTF) → z up (board): (x, y, z) → (x, -z, y).
            let board = [corner[0], -corner[2], corner[1]];
            for axis in 0..3 {
                bbox[axis] = bbox[axis].min(board[axis]);
                bbox[axis + 3] = bbox[axis + 3].max(board[axis]);
            }
        }
        colored.update(color);
    }
    if triangles.is_empty() {
        bbox = [0; 6];
    }
    Ok(MeshPrint {
        geometry: hex16(&geometry.finalize()),
        colored: hex16(&colored.finalize()),
        bbox,
        triangles: triangles.len() as u64,
    })
}

/// One primitive for [`write_glb`]: positions (metres, y up), triangle
/// indices and the colour.
pub type Part<'a> = (&'a [[f32; 3]], &'a [u32], [f32; 4]);

/// Writes a minimal GLB file (positions and indices, one primitive per
/// colour). Used by tests and the synthetic fixtures; the desktop app has
/// its own writer with normals.
#[must_use]
pub fn write_glb(parts: &[Part<'_>]) -> Vec<u8> {
    let mut bin: Vec<u8> = Vec::new();
    let mut views = Vec::new();
    let mut accessors = Vec::new();
    let mut materials = Vec::new();
    let mut primitives = Vec::new();
    for (index, (points, triangles, color)) in parts.iter().enumerate() {
        let start = bin.len();
        let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
        for point in *points {
            for axis in 0..3 {
                min[axis] = min[axis].min(point[axis]);
                max[axis] = max[axis].max(point[axis]);
                bin.extend_from_slice(&point[axis].to_le_bytes());
            }
        }
        views.push(
            serde_json::json!({"buffer": 0, "byteOffset": start, "byteLength": bin.len() - start}),
        );
        accessors.push(serde_json::json!({
            "bufferView": views.len() - 1, "componentType": 5126, "count": points.len(),
            "type": "VEC3", "min": min, "max": max
        }));
        let start = bin.len();
        for value in *triangles {
            bin.extend_from_slice(&value.to_le_bytes());
        }
        views.push(
            serde_json::json!({"buffer": 0, "byteOffset": start, "byteLength": bin.len() - start}),
        );
        accessors.push(serde_json::json!({
            "bufferView": views.len() - 1, "componentType": 5125, "count": triangles.len(), "type": "SCALAR"
        }));
        materials.push(serde_json::json!({"pbrMetallicRoughness": {"baseColorFactor": color}}));
        primitives.push(serde_json::json!({
            "attributes": {"POSITION": accessors.len() - 2}, "indices": accessors.len() - 1, "material": index
        }));
    }
    let json = serde_json::json!({
        "asset": {"version": "2.0", "generator": "ElectronicWerk"},
        "scene": 0, "scenes": [{"nodes": [0]}], "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": primitives}],
        "accessors": accessors, "bufferViews": views, "materials": materials,
        "buffers": [{"byteLength": bin.len()}]
    });
    let mut json = serde_json::to_vec(&json).unwrap_or_default();
    while json.len() % 4 != 0 {
        json.push(b' ');
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&2_u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
    out.extend_from_slice(&bin);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A box of `w × d × h` mm (metres, y up), split into two colours.
    #[allow(clippy::many_single_char_names, reason = "coordinates")]
    fn cube(w: f32, d: f32, h: f32, color: [f32; 4], reorder: bool) -> Vec<u8> {
        let (x, z, y) = (w / 2000.0, d / 2000.0, h / 1000.0);
        let points = [
            [-x, 0.0, -z],
            [x, 0.0, -z],
            [x, 0.0, z],
            [-x, 0.0, z],
            [-x, y, -z],
            [x, y, -z],
            [x, y, z],
            [-x, y, z],
        ];
        let mut sides: Vec<u32> = vec![
            0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6, 3, 0, 4, 3, 4, 7,
        ];
        let mut caps: Vec<u32> = vec![0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7];
        if reorder {
            sides.reverse();
            caps.rotate_left(3);
            // Reversing the list also reverses each triangle's winding;
            // rotate the corners back.
            for tri in sides.chunks_exact_mut(3) {
                tri.reverse();
            }
        }
        write_glb(&[
            (&points, &sides, color),
            (&points, &caps, [0.1, 0.1, 0.1, 1.0]),
        ])
    }

    #[test]
    fn prints_ignore_order_and_detect_colours() {
        let a = read_glb(&cube(2.0, 1.25, 0.5, [0.8, 0.8, 0.8, 1.0], false)).unwrap();
        let b = read_glb(&cube(2.0, 1.25, 0.5, [0.8, 0.8, 0.8, 1.0], true)).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.triangles, 12);
        assert_eq!(a.bbox, [-1000, -625, 0, 1000, 625, 500]);
        let red = read_glb(&cube(2.0, 1.25, 0.5, [0.8, 0.1, 0.1, 1.0], false)).unwrap();
        assert_eq!(red.geometry, a.geometry);
        assert_ne!(red.colored, a.colored);
        let taller = read_glb(&cube(2.0, 1.25, 0.6, [0.8, 0.8, 0.8, 1.0], false)).unwrap();
        assert_ne!(taller.geometry, a.geometry);
    }

    #[test]
    fn rejects_other_files() {
        assert!(read_glb(b"not a glb file").is_err());
        assert!(read_glb(&[]).is_err());
    }
}
