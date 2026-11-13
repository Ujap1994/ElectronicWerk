//! The board for the 3D view: metadata (JSON) plus one binary buffer with
//! the meshes, which the renderer hands to the GPU as it is.
//!
//! Each mesh in the buffer: positions (`f32` × 3 per vertex), normals
//! (`f32` × 3) and indices (`u32` × 3 per triangle), at the byte offsets
//! named in its [`MeshInfo`]. Everything is little-endian and 4-byte
//! aligned.

use serde::Serialize;

use crate::board::Board3d;
use crate::mesh::Mesh;
use crate::place::Part;
use crate::stack::{Colors, Levels};

/// Where a mesh lies in the buffer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeshInfo {
    /// `body`, `barrels`, `copper:top`, `mask:bottom`, `copper:in1`, …
    pub key: String,
    /// Number of vertices.
    pub vertices: usize,
    /// Number of triangles.
    pub triangles: usize,
    /// Byte offset of the positions.
    pub positions: usize,
    /// Byte offset of the normals.
    pub normals: usize,
    /// Byte offset of the indices.
    pub indices: usize,
}

/// The board part of the metadata.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BoardInfo {
    /// Board name.
    pub name: String,
    /// Heights.
    pub levels: Levels,
    /// Material colours.
    pub colors: Colors,
    /// Bounds in mm `[x0, y0, z0, x1, y1, z1]`.
    pub bounds: [f64; 6],
    /// Number of holes.
    pub holes: usize,
    /// Problems found while building.
    pub warnings: Vec<String>,
}

/// The metadata of a 3D scene.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneMeta {
    /// The board.
    pub board: BoardInfo,
    /// The meshes in the buffer.
    pub meshes: Vec<MeshInfo>,
    /// The parts with their models.
    pub parts: Vec<Part>,
}

fn push_mesh(key: String, mesh: &Mesh, data: &mut Vec<u8>, meshes: &mut Vec<MeshInfo>) {
    if mesh.is_empty() {
        return;
    }
    let positions = data.len();
    for p in &mesh.positions {
        for v in p {
            data.extend_from_slice(&v.to_le_bytes());
        }
    }
    let normals = data.len();
    for n in &mesh.normals {
        for v in n {
            data.extend_from_slice(&v.to_le_bytes());
        }
    }
    let indices = data.len();
    for i in &mesh.indices {
        data.extend_from_slice(&i.to_le_bytes());
    }
    meshes.push(MeshInfo {
        key,
        vertices: mesh.positions.len(),
        triangles: mesh.triangles(),
        positions,
        normals,
        indices,
    });
}

/// Encodes a board and its parts.
#[must_use]
pub fn encode(name: &str, board: &Board3d, parts: Vec<Part>) -> (SceneMeta, Vec<u8>) {
    let mut data = Vec::new();
    let mut meshes = Vec::new();
    push_mesh("body".into(), &board.body, &mut data, &mut meshes);
    push_mesh("barrels".into(), &board.barrels, &mut data, &mut meshes);
    for layer in &board.layers {
        push_mesh(layer.key(), &layer.mesh, &mut data, &mut meshes);
    }
    (
        SceneMeta {
            board: BoardInfo {
                name: name.to_owned(),
                levels: board.levels,
                colors: board.colors.clone(),
                bounds: board.bounds,
                holes: board.holes.len(),
                warnings: board.warnings.clone(),
            },
            meshes,
            parts,
        },
        data,
    )
}
