import { describe, expect, it } from 'vitest';

import { pick } from './pick';
import { Scene } from './scene';
import type { SceneReply } from './types';

/**
 * A hand-made scene: one layer with a footprint (box 0..100), a pad (disc at
 * 10, 10, r 2), a track (segment 0,50 → 100,50, half width 1) and a zone
 * covering everything (two triangles).
 */
function scene(): Scene {
  const objects = 4;
  // Object table: types (u8, padded), parents, children, nets, boxes, ids.
  const typesOffset = 0;
  const parentsOffset = 4;
  const childrenOffset = parentsOffset + objects * 4;
  const netsOffset = childrenOffset + objects * 4;
  const boxesOffset = netsOffset + objects * 4;
  const idsOffset = boxesOffset + objects * 16;
  const segmentsOffset = idsOffset + objects * 16;
  const circlesOffset = segmentsOffset + 24;
  const verticesOffset = circlesOffset + 20;
  const indicesOffset = verticesOffset + 4 * 16;
  const total = indicesOffset + 6 * 4;
  const buffer = new ArrayBuffer(total);
  const bytes = new Uint8Array(buffer);
  const f32 = new Float32Array(buffer);
  const u32 = new Uint32Array(buffer);
  // types: footprint(2), pad(3), track(0), zone(5)
  bytes.set([2, 3, 0, 5], typesOffset);
  u32.set([0xffff_ffff, 0, 0xffff_ffff, 0xffff_ffff], parentsOffset / 4);
  u32.set([0xffff_ffff, 0, 0xffff_ffff, 0xffff_ffff], childrenOffset / 4);
  u32.set([0xffff_ffff, 0xffff_ffff, 0xffff_ffff, 0xffff_ffff], netsOffset / 4);
  f32.set([0, 0, 100, 100, 8, 8, 12, 12, 0, 49, 100, 51, -50, -50, 150, 150], boxesOffset / 4);
  for (let i = 0; i < objects; i += 1) {
    bytes[idsOffset + i * 16 + 15] = i + 1;
  }
  f32.set([0, 50, 100, 50, 1], segmentsOffset / 4);
  u32[segmentsOffset / 4 + 5] = 2;
  f32.set([10, 10, 2, 0], circlesOffset / 4);
  u32[circlesOffset / 4 + 4] = 1;
  const v = verticesOffset / 4;
  f32.set([-50, -50, 0, 0, 150, -50, 0, 0, 150, 150, 0, 0, -50, 150, 0, 0], v);
  for (let i = 0; i < 4; i += 1) {
    u32[v + i * 4 + 2] = 3;
    u32[v + i * 4 + 3] = 1;
  }
  u32.set([0, 1, 2, 0, 2, 3], indicesOffset / 4);
  const empty = { offset: 0, count: 0, chunks: 0 };
  const reply: SceneReply = {
    doc: 'd',
    kind: 'pcb',
    name: 'Test',
    origin: [0, 0],
    bounds: [-50, -50, 150, 150],
    home: null,
    page: null,
    grid: 1,
    chunk: 1024,
    font: { advance: 0.8, linePitch: 1.6 },
    layers: [
      {
        key: 'cu.top',
        group: 'copper',
        side: 'top',
        name: null,
        z: 300,
        visible: true,
        segments: { offset: segmentsOffset, count: 1, chunks: 0 },
        arcs: empty,
        circles: { offset: circlesOffset, count: 1, chunks: 0 },
        rects: empty,
        mesh: { vertices: verticesOffset, vertexCount: 4, indices: indicesOffset, indexCount: 6 },
        texts: [],
      },
    ],
    objectTypes: ['track', 'via', 'footprint', 'pad', 'footprintText', 'zone'],
    objects: {
      count: objects,
      types: typesOffset,
      parents: parentsOffset,
      children: childrenOffset,
      nets: netsOffset,
      boxes: boxesOffset,
      ids: idsOffset,
    },
    nets: [],
    stats: {
      objects,
      segments: 1,
      arcs: 0,
      circles: 1,
      rects: 0,
      triangles: 2,
      texts: 0,
      bytes: total,
    },
    data: new Uint8Array(buffer),
  };
  return new Scene(reply);
}

const all = (): boolean => true;

describe('pick', () => {
  const s = scene();

  it('prefers the most specific object directly under the cursor', () => {
    expect(s.typeOf(pick(s, 10, 10, 4, all) ?? -1)).toBe('pad');
    expect(s.typeOf(pick(s, 50, 50, 4, all) ?? -1)).toBe('track');
    expect(s.typeOf(pick(s, 70, 20, 4, all) ?? -1)).toBe('footprint');
  });

  it('prefers an object under the cursor to a nearby, more specific one', () => {
    // 3 units from the pad edge: within the tolerance, but the footprint body
    // is directly under the cursor.
    expect(s.typeOf(pick(s, 15, 10, 4, all) ?? -1)).toBe('footprint');
    // With a larger "direct" radius the pad wins again.
    expect(s.typeOf(pick(s, 15, 10, 4, all, null, 4) ?? -1)).toBe('pad');
  });

  it('chooses zones last and only when nothing else is near', () => {
    expect(s.typeOf(pick(s, 120, 52, 4, all) ?? -1)).toBe('zone');
    expect(s.typeOf(pick(s, 120, 50, 4, all) ?? -1)).toBe('zone');
    expect(s.typeOf(pick(s, 99, 53, 4, all) ?? -1)).toBe('track');
    expect(pick(s, 400, 400, 4, all)).toBeNull();
  });

  it('ignores hidden layers', () => {
    expect(pick(s, 10, 10, 4, () => false)).toBeNull();
  });
});
