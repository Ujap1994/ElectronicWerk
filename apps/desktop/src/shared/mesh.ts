/**
 * Tessellated 3D models as GLB (binary glTF 2.0), shared by the engine host
 * (on-demand tessellation) and the library build (`scripts/tessellate.ts`,
 * which runs this file directly with Node's type stripping, so it must not
 * import anything and must only use erasable TypeScript syntax).
 *
 * Input are the meshes of `occt-import-js` (millimetres, z up). Output is a
 * GLB file in metres with y up, as glTF requires: one mesh with one
 * primitive per colour, float positions and normals quantized to bytes
 * (KHR_mesh_quantization). The Rust library index fingerprints these files
 * (crates/ew-library/src/mesh.rs).
 */

/** One mesh as returned by `occt-import-js`. */
export interface OcctMesh {
  readonly name?: string;
  readonly color?: readonly number[] | null;
  readonly brep_faces?: readonly {
    readonly first: number;
    readonly last: number;
    readonly color?: readonly number[] | null;
  }[];
  readonly attributes: {
    readonly position: { readonly array: ArrayLike<number> };
    readonly normal?: { readonly array: ArrayLike<number> };
  };
  readonly index: { readonly array: ArrayLike<number> };
}

/** The result of `occt.ReadStepFile`. */
export interface OcctResult {
  readonly success: boolean;
  readonly meshes: readonly OcctMesh[];
}

/** Tessellation settings for `occt.ReadStepFile`. */
export const OCCT_PARAMS = {
  linearUnit: 'millimeter',
  linearDeflectionType: 'bounding_box_ratio',
  linearDeflection: 0.001,
  angularDeflection: 0.5,
} as const;

/** Colour of faces without one (light grey). */
const DEFAULT_COLOR: readonly number[] = [0.75, 0.75, 0.75];

/** Numbers of a GLB file, for reports. */
export interface GlbStats {
  readonly triangles: number;
  readonly vertices: number;
  readonly colors: number;
}

function colorKey(color: readonly number[] | null | undefined): string {
  const c = color ?? DEFAULT_COLOR;
  return [c[0] ?? 0, c[1] ?? 0, c[2] ?? 0].map((v) => Math.round(v * 255)).join(',');
}

function pad4(length: number): number {
  return (4 - (length % 4)) % 4;
}

/**
 * Converts the meshes of a STEP file into a GLB file. Returns `null` if
 * there are no triangles.
 */
export function occtToGlb(
  meshes: readonly OcctMesh[],
): { glb: Uint8Array; stats: GlbStats } | null {
  // Combined vertices (metres, y up) and one index list per colour.
  const positions: number[] = [];
  const normals: number[] = [];
  const byColor = new Map<string, number[]>();
  let haveNormals = true;
  for (const mesh of meshes) {
    const base = positions.length / 3;
    const p = mesh.attributes.position.array;
    const n = mesh.attributes.normal?.array;
    if (n?.length !== p.length) {
      haveNormals = false;
    }
    for (let i = 0; i + 2 < p.length; i += 3) {
      // mm, z up → m, y up: (x, y, z) → (x, z, -y).
      positions.push((p[i] ?? 0) / 1000, (p[i + 2] ?? 0) / 1000, -(p[i + 1] ?? 0) / 1000);
      if (n?.length === p.length) {
        normals.push(n[i] ?? 0, n[i + 2] ?? 0, -(n[i + 1] ?? 0));
      }
    }
    const index = mesh.index.array;
    const triangles = Math.floor(index.length / 3);
    // Colour of every triangle: its face's colour, else the mesh colour.
    const colors: string[] = new Array<string>(triangles).fill(colorKey(mesh.color));
    for (const face of mesh.brep_faces ?? []) {
      if (face.color === undefined || face.color === null) {
        continue;
      }
      const key = colorKey(face.color);
      for (let t = Math.max(0, face.first); t <= Math.min(face.last, triangles - 1); t++) {
        colors[t] = key;
      }
    }
    for (let t = 0; t < triangles; t++) {
      const key = colors[t] ?? colorKey(null);
      let list = byColor.get(key);
      if (list === undefined) {
        list = [];
        byColor.set(key, list);
      }
      list.push(
        base + (index[t * 3] ?? 0),
        base + (index[t * 3 + 1] ?? 0),
        base + (index[t * 3 + 2] ?? 0),
      );
    }
  }
  const vertexCount = positions.length / 3;
  if (vertexCount === 0 || byColor.size === 0) {
    return null;
  }
  const keys = [...byColor.keys()].sort();
  const wide = vertexCount > 65_535;

  // Binary chunk: positions, normals, then the index lists.
  const views: Record<string, unknown>[] = [];
  const accessors: Record<string, unknown>[] = [];
  const parts: Uint8Array[] = [];
  let offset = 0;
  const addView = (bytes: Uint8Array, target: number, stride?: number): number => {
    views.push({
      buffer: 0,
      byteOffset: offset,
      byteLength: bytes.byteLength,
      target,
      ...(stride === undefined ? {} : { byteStride: stride }),
    });
    parts.push(bytes);
    offset += bytes.byteLength;
    const padding = pad4(bytes.byteLength);
    if (padding > 0) {
      parts.push(new Uint8Array(padding));
      offset += padding;
    }
    return views.length - 1;
  };

  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  const positionData = new Float32Array(positions);
  for (let i = 0; i < positionData.length; i++) {
    const axis = i % 3;
    const value = positionData[i] ?? 0;
    min[axis] = Math.min(min[axis] ?? Infinity, value);
    max[axis] = Math.max(max[axis] ?? -Infinity, value);
  }
  accessors.push({
    bufferView: addView(new Uint8Array(positionData.buffer), 34962),
    componentType: 5126,
    count: vertexCount,
    type: 'VEC3',
    min,
    max,
  });
  const attributes: Record<string, number> = { POSITION: 0 };
  if (haveNormals && normals.length === positions.length) {
    // Bytes, padded to 4 per vertex (vertex attributes must be aligned).
    const packed = new Int8Array(vertexCount * 4);
    for (let v = 0; v < vertexCount; v++) {
      const x = normals[v * 3] ?? 0;
      const y = normals[v * 3 + 1] ?? 0;
      const z = normals[v * 3 + 2] ?? 0;
      const length = Math.hypot(x, y, z) || 1;
      packed[v * 4] = Math.round((x / length) * 127);
      packed[v * 4 + 1] = Math.round((y / length) * 127);
      packed[v * 4 + 2] = Math.round((z / length) * 127);
    }
    accessors.push({
      bufferView: addView(new Uint8Array(packed.buffer), 34962, 4),
      componentType: 5120,
      normalized: true,
      count: vertexCount,
      type: 'VEC3',
    });
    attributes['NORMAL'] = 1;
  }
  const materials: Record<string, unknown>[] = [];
  const primitives: Record<string, unknown>[] = [];
  let triangles = 0;
  for (const key of keys) {
    const list = byColor.get(key) ?? [];
    triangles += list.length / 3;
    const data = wide ? new Uint32Array(list) : new Uint16Array(list);
    accessors.push({
      bufferView: addView(new Uint8Array(data.buffer), 34963),
      componentType: wide ? 5125 : 5123,
      count: list.length,
      type: 'SCALAR',
    });
    const [r, g, b] = key.split(',').map((v) => Number(v) / 255);
    // Light, unsaturated colours are usually metal (pins, shields).
    const metal =
      (r ?? 0) > 0.6 &&
      Math.abs((r ?? 0) - (b ?? 0)) < 0.12 &&
      Math.abs((r ?? 0) - (g ?? 0)) < 0.12;
    materials.push({
      pbrMetallicRoughness: {
        baseColorFactor: [r, g, b, 1],
        metallicFactor: metal ? 0.6 : 0,
        roughnessFactor: metal ? 0.35 : 0.6,
      },
    });
    primitives.push({ attributes, indices: accessors.length - 1, material: materials.length - 1 });
  }
  const json = {
    asset: { version: '2.0', generator: 'ElectronicWerk' },
    ...(attributes['NORMAL'] === undefined
      ? {}
      : {
          extensionsUsed: ['KHR_mesh_quantization'],
          extensionsRequired: ['KHR_mesh_quantization'],
        }),
    scene: 0,
    scenes: [{ nodes: [0] }],
    nodes: [{ mesh: 0 }],
    meshes: [{ primitives }],
    materials,
    accessors,
    bufferViews: views,
    buffers: [{ byteLength: offset }],
  };
  const jsonBytes = new TextEncoder().encode(JSON.stringify(json));
  const jsonLength = jsonBytes.byteLength + pad4(jsonBytes.byteLength);
  const total = 12 + 8 + jsonLength + 8 + offset;
  const glb = new Uint8Array(total);
  const view = new DataView(glb.buffer);
  view.setUint32(0, 0x46546c67, true);
  view.setUint32(4, 2, true);
  view.setUint32(8, total, true);
  view.setUint32(12, jsonLength, true);
  view.setUint32(16, 0x4e4f534a, true);
  glb.set(jsonBytes, 20);
  glb.fill(0x20, 20 + jsonBytes.byteLength, 20 + jsonLength);
  let at = 20 + jsonLength;
  view.setUint32(at, offset, true);
  view.setUint32(at + 4, 0x004e4942, true);
  at += 8;
  for (const part of parts) {
    glb.set(part, at);
    at += part.byteLength;
  }
  return { glb, stats: { triangles, vertices: vertexCount, colors: keys.length } };
}
