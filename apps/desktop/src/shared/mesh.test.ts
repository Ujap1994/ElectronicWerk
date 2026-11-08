import { describe, expect, it } from 'vitest';

import { occtToGlb, type OcctMesh } from './mesh';

/** A 2 × 1 × 0.5 mm box as OpenCASCADE returns it (mm, z up), two colours. */
function box(): OcctMesh {
  const positions = [
    0, 0, 0, 2, 0, 0, 2, 1, 0, 0, 1, 0, 0, 0, 0.5, 2, 0, 0.5, 2, 1, 0.5, 0, 1, 0.5,
  ];
  return {
    color: [0.1, 0.1, 0.1],
    brep_faces: [{ first: 2, last: 3, color: [0.8, 0.8, 0.8] }],
    attributes: {
      position: { array: positions },
      normal: { array: positions.map((_, i) => (i % 3 === 2 ? 1 : 0)) },
    },
    index: { array: [0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7] },
  };
}

function chunks(glb: Uint8Array): { json: Record<string, unknown>; bin: Uint8Array } {
  const view = new DataView(glb.buffer, glb.byteOffset, glb.byteLength);
  expect(view.getUint32(0, true)).toBe(0x46546c67);
  expect(view.getUint32(4, true)).toBe(2);
  expect(view.getUint32(8, true)).toBe(glb.byteLength);
  const jsonLength = view.getUint32(12, true);
  const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + jsonLength))) as Record<
    string,
    unknown
  >;
  const binLength = view.getUint32(20 + jsonLength, true);
  return { json, bin: glb.subarray(28 + jsonLength, 28 + jsonLength + binLength) };
}

describe('occtToGlb', () => {
  it('writes metres, y up, one primitive per colour', () => {
    const result = occtToGlb([box()]);
    expect(result).not.toBeNull();
    const { glb, stats } = result ?? { glb: new Uint8Array(), stats: null };
    expect(stats).toEqual({ triangles: 4, vertices: 8, colors: 2 });
    const { json, bin } = chunks(glb);
    const meshes = json['meshes'] as { primitives: { indices: number; material: number }[] }[];
    expect(meshes[0]?.primitives).toHaveLength(2);
    const accessors = json['accessors'] as {
      min?: number[];
      max?: number[];
      componentType: number;
    }[];
    // x stays, z (height) becomes y, y becomes -z.
    expect(accessors[0]?.min).toEqual([0, 0, -0.0010000000474974513]);
    expect(accessors[0]?.max?.[1]).toBeCloseTo(0.0005);
    expect(accessors[1]?.componentType).toBe(5120);
    expect(json['extensionsRequired']).toEqual(['KHR_mesh_quantization']);
    expect(bin.byteLength % 4).toBe(0);
  });

  it('is deterministic and skips empty input', () => {
    const a = occtToGlb([box()])?.glb;
    const b = occtToGlb([box()])?.glb;
    expect(a).toEqual(b);
    expect(occtToGlb([])).toBeNull();
  });
});
