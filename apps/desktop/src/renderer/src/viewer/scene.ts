/**
 * A decoded scene: typed views on the engine's binary buffer, the object
 * table, a per-object list of primitives and exact hit tests.
 */
import Flatbush from 'flatbush';

import { textCorners, type FontMetrics } from './text-layout';
import {
  NO_OBJECT,
  STRIDE,
  type Box,
  type SceneLayer,
  type SceneMeta,
  type SceneReply,
  type Section,
} from './types';

/** Primitive kinds in the object → primitive table. */
export const Prim = {
  Segment: 0,
  Arc: 1,
  Circle: 2,
  Rect: 3,
  Triangle: 4,
  Text: 5,
} as const;
export type Prim = (typeof Prim)[keyof typeof Prim];

/** Typed views of one layer. */
export interface LayerViews {
  readonly meta: SceneLayer;
  readonly segments: Float32Array;
  readonly segmentObjects: Uint32Array;
  readonly arcs: Float32Array;
  readonly arcObjects: Uint32Array;
  readonly circles: Float32Array;
  readonly circleObjects: Uint32Array;
  readonly rects: Float32Array;
  readonly rectObjects: Uint32Array;
  readonly vertices: Float32Array;
  readonly vertexWords: Uint32Array;
  readonly indices: Uint32Array;
  /** Text box corners (8 numbers per text). */
  readonly textCorners: readonly Float64Array[];
}

function decodeBase64(text: string): Uint8Array {
  const binary = atob(text);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

/** Returns an ArrayBuffer whose byte 0 is the scene's byte 0. */
function alignedBuffer(data: Uint8Array | string | null): ArrayBuffer {
  if (data === null) {
    return new ArrayBuffer(0);
  }
  const bytes = typeof data === 'string' ? decodeBase64(data) : data;
  if (bytes.byteOffset === 0 && bytes.byteLength === bytes.buffer.byteLength) {
    return bytes.buffer as ArrayBuffer;
  }
  return bytes.slice().buffer;
}

function floats(buffer: ArrayBuffer, section: Section, stride: number): Float32Array {
  return new Float32Array(buffer, section.offset, section.count * stride);
}

function words(buffer: ArrayBuffer, section: Section, stride: number): Uint32Array {
  return new Uint32Array(buffer, section.offset, section.count * stride);
}

/** Squared distance from (px, py) to the segment a–b. */
function segmentDistance(
  px: number,
  py: number,
  ax: number,
  ay: number,
  bx: number,
  by: number,
): number {
  const dx = bx - ax;
  const dy = by - ay;
  const len2 = dx * dx + dy * dy;
  let t = len2 > 0 ? ((px - ax) * dx + (py - ay) * dy) / len2 : 0;
  t = Math.max(0, Math.min(1, t));
  const cx = ax + t * dx - px;
  const cy = ay + t * dy - py;
  return Math.sqrt(cx * cx + cy * cy);
}

function pointInPolygon(px: number, py: number, polygon: ArrayLike<number>): boolean {
  let inside = false;
  const n = polygon.length / 2;
  for (let i = 0, j = n - 1; i < n; j = i, i += 1) {
    const xi = polygon[2 * i] ?? 0;
    const yi = polygon[2 * i + 1] ?? 0;
    const xj = polygon[2 * j] ?? 0;
    const yj = polygon[2 * j + 1] ?? 0;
    if (yi > py !== yj > py && px < ((xj - xi) * (py - yi)) / (yj - yi) + xi) {
      inside = !inside;
    }
  }
  return inside;
}

export { pointInPolygon };

export class Scene {
  readonly meta: SceneMeta;
  readonly buffer: ArrayBuffer;
  readonly layers: readonly LayerViews[];
  readonly objectCount: number;
  readonly types: Uint8Array;
  readonly parents: Uint32Array;
  readonly children: Uint32Array;
  readonly nets: Uint32Array;
  readonly boxes: Float32Array;
  readonly ids: Uint8Array;
  /** Primitive refs per object: `refStart[o]` … `refStart[o + 1]`. */
  readonly refStart: Uint32Array;
  /** `layer << 3 | kind` per ref. */
  readonly refLayerKind: Uint16Array;
  readonly refIndex: Uint32Array;
  /** Child objects per object (CSR like the refs). */
  readonly childStart: Uint32Array;
  readonly childList: Uint32Array;
  readonly index: Flatbush | null;

  constructor(reply: SceneReply) {
    const { data, ...meta } = reply;
    this.meta = meta;
    const buffer = alignedBuffer(data);
    this.buffer = buffer;
    const objects = meta.objects;
    const count = objects.count;
    this.objectCount = count;
    this.types = new Uint8Array(buffer, objects.types, count);
    this.parents = new Uint32Array(buffer, objects.parents, count);
    this.children = new Uint32Array(buffer, objects.children, count);
    this.nets = new Uint32Array(buffer, objects.nets, count);
    this.boxes = new Float32Array(buffer, objects.boxes, count * 4);
    this.ids = new Uint8Array(buffer, objects.ids, count * 16);
    this.layers = meta.layers.map((layer) => ({
      meta: layer,
      segments: floats(buffer, layer.segments, STRIDE.segments),
      segmentObjects: words(buffer, layer.segments, STRIDE.segments),
      arcs: floats(buffer, layer.arcs, STRIDE.arcs),
      arcObjects: words(buffer, layer.arcs, STRIDE.arcs),
      circles: floats(buffer, layer.circles, STRIDE.circles),
      circleObjects: words(buffer, layer.circles, STRIDE.circles),
      rects: floats(buffer, layer.rects, STRIDE.rects),
      rectObjects: words(buffer, layer.rects, STRIDE.rects),
      vertices: new Float32Array(
        buffer,
        layer.mesh.vertices,
        layer.mesh.vertexCount * STRIDE.vertices,
      ),
      vertexWords: new Uint32Array(
        buffer,
        layer.mesh.vertices,
        layer.mesh.vertexCount * STRIDE.vertices,
      ),
      indices: new Uint32Array(buffer, layer.mesh.indices, layer.mesh.indexCount),
      textCorners: layer.texts.map((text) => textCorners(text, meta.font)),
    }));
    [this.refStart, this.refLayerKind, this.refIndex] = this.buildRefs();
    [this.childStart, this.childList] = this.buildChildren();
    this.index = this.buildIndex();
  }

  get font(): FontMetrics {
    return this.meta.font;
  }

  /** Visits every primitive as (object, layer, kind, index). */
  private forEachPrimitive(
    visit: (obj: number, layer: number, kind: Prim, index: number) => void,
  ): void {
    const stride = { s: STRIDE.segments, a: STRIDE.arcs, c: STRIDE.circles, r: STRIDE.rects };
    this.layers.forEach((layer, l) => {
      const visitWords = (objectsWords: Uint32Array, size: number, kind: Prim): void => {
        const n = objectsWords.length / size;
        for (let i = 0; i < n; i += 1) {
          visit(objectsWords[i * size + size - 1] ?? NO_OBJECT, l, kind, i);
        }
      };
      visitWords(layer.segmentObjects, stride.s, Prim.Segment);
      visitWords(layer.arcObjects, stride.a, Prim.Arc);
      visitWords(layer.circleObjects, stride.c, Prim.Circle);
      visitWords(layer.rectObjects, stride.r, Prim.Rect);
      const triangles = layer.indices.length / 3;
      for (let t = 0; t < triangles; t += 1) {
        const v = layer.indices[t * 3] ?? 0;
        visit(layer.vertexWords[v * STRIDE.vertices + 2] ?? NO_OBJECT, l, Prim.Triangle, t);
      }
      layer.meta.texts.forEach((text, i) => {
        visit(text[8], l, Prim.Text, i);
      });
    });
  }

  private buildRefs(): [Uint32Array, Uint16Array, Uint32Array] {
    const count = this.objectCount;
    const start = new Uint32Array(count + 1);
    this.forEachPrimitive((obj) => {
      if (obj < count) {
        start[obj + 1] = (start[obj + 1] ?? 0) + 1;
      }
    });
    for (let i = 0; i < count; i += 1) {
      start[i + 1] = (start[i + 1] ?? 0) + (start[i] ?? 0);
    }
    const total = start[count] ?? 0;
    const layerKind = new Uint16Array(total);
    const index = new Uint32Array(total);
    const fill = start.slice(0, count);
    this.forEachPrimitive((obj, layer, kind, i) => {
      if (obj < count) {
        const at = fill[obj] ?? 0;
        fill[obj] = at + 1;
        layerKind[at] = (layer << 3) | kind;
        index[at] = i;
      }
    });
    return [start, layerKind, index];
  }

  private buildChildren(): [Uint32Array, Uint32Array] {
    const count = this.objectCount;
    const start = new Uint32Array(count + 1);
    for (let o = 0; o < count; o += 1) {
      const parent = this.parents[o] ?? NO_OBJECT;
      if (parent < count) {
        start[parent + 1] = (start[parent + 1] ?? 0) + 1;
      }
    }
    for (let i = 0; i < count; i += 1) {
      start[i + 1] = (start[i + 1] ?? 0) + (start[i] ?? 0);
    }
    const list = new Uint32Array(start[count] ?? 0);
    const fill = start.slice(0, count);
    for (let o = 0; o < count; o += 1) {
      const parent = this.parents[o] ?? NO_OBJECT;
      if (parent < count) {
        const at = fill[parent] ?? 0;
        fill[parent] = at + 1;
        list[at] = o;
      }
    }
    return [start, list];
  }

  private buildIndex(): Flatbush | null {
    if (this.objectCount === 0) {
      return null;
    }
    const index = new Flatbush(this.objectCount);
    const b = this.boxes;
    for (let o = 0; o < this.objectCount; o += 1) {
      index.add(b[o * 4] ?? 0, b[o * 4 + 1] ?? 0, b[o * 4 + 2] ?? 0, b[o * 4 + 3] ?? 0);
    }
    index.finish();
    return index;
  }

  /** Object type name. */
  typeOf(obj: number): string {
    return this.meta.objectTypes[this.types[obj] ?? 0] ?? 'unknown';
  }

  /** Object id as a UUID string. */
  idOf(obj: number): string {
    const hex: string[] = [];
    for (let i = 0; i < 16; i += 1) {
      hex.push((this.ids[obj * 16 + i] ?? 0).toString(16).padStart(2, '0'));
    }
    const s = hex.join('');
    return `${s.slice(0, 8)}-${s.slice(8, 12)}-${s.slice(12, 16)}-${s.slice(16, 20)}-${s.slice(20)}`;
  }

  /** Parent object, or `null`. */
  parentOf(obj: number): number | null {
    const parent = this.parents[obj] ?? NO_OBJECT;
    return parent === NO_OBJECT ? null : parent;
  }

  /** A stable key (survives a scene reload): id, type and child index. */
  keyOf(obj: number): string {
    const child = this.children[obj] ?? NO_OBJECT;
    return child === NO_OBJECT ? this.idOf(obj) : `${this.idOf(obj)}/${this.typeOf(obj)}/${child}`;
  }

  /** Net name of an object, or `null`. */
  netOf(obj: number): string | null {
    const net = this.nets[obj] ?? NO_OBJECT;
    return net === NO_OBJECT ? null : (this.meta.nets[net] ?? null);
  }

  box(obj: number): Box {
    const b = this.boxes;
    return [b[obj * 4] ?? 0, b[obj * 4 + 1] ?? 0, b[obj * 4 + 2] ?? 0, b[obj * 4 + 3] ?? 0];
  }

  /** Children of an object. */
  childrenOf(obj: number): Uint32Array {
    return this.childList.subarray(this.childStart[obj] ?? 0, this.childStart[obj + 1] ?? 0);
  }

  /** `true` if the object has a primitive on a visible layer. */
  isVisible(obj: number, visible: (layer: number) => boolean): boolean {
    const from = this.refStart[obj] ?? 0;
    const to = this.refStart[obj + 1] ?? 0;
    for (let r = from; r < to; r += 1) {
      if (visible((this.refLayerKind[r] ?? 0) >> 3)) {
        return true;
      }
    }
    for (const child of this.childrenOf(obj)) {
      if (this.isVisible(child, visible)) {
        return true;
      }
    }
    return false;
  }

  /**
   * Distance from (x, y) to the drawn shape of an object on visible layers
   * (0 inside), or `Infinity`.
   */
  distance(obj: number, x: number, y: number, visible: (layer: number) => boolean): number {
    let best = Infinity;
    const from = this.refStart[obj] ?? 0;
    const to = this.refStart[obj + 1] ?? 0;
    for (let r = from; r < to && best > 0; r += 1) {
      const lk = this.refLayerKind[r] ?? 0;
      const layerIndex = lk >> 3;
      if (!visible(layerIndex)) {
        continue;
      }
      const layer = this.layers[layerIndex];
      if (layer === undefined) {
        continue;
      }
      best = Math.min(
        best,
        primitiveDistance(layer, (lk & 7) as Prim, this.refIndex[r] ?? 0, x, y),
      );
    }
    return best;
  }
}

/** Distance from a point to one primitive (0 inside). */
export function primitiveDistance(
  layer: LayerViews,
  kind: Prim,
  i: number,
  x: number,
  y: number,
): number {
  switch (kind) {
    case Prim.Segment: {
      const s = layer.segments;
      const o = i * STRIDE.segments;
      const d = segmentDistance(x, y, s[o] ?? 0, s[o + 1] ?? 0, s[o + 2] ?? 0, s[o + 3] ?? 0);
      return Math.max(0, d - (s[o + 4] ?? 0));
    }
    case Prim.Arc: {
      const a = layer.arcs;
      const o = i * STRIDE.arcs;
      const cx = a[o] ?? 0;
      const cy = a[o + 1] ?? 0;
      const r = a[o + 2] ?? 0;
      const hw = a[o + 3] ?? 0;
      let start = a[o + 4] ?? 0;
      let sweep = a[o + 5] ?? 0;
      if (sweep < 0) {
        start += sweep;
        sweep = -sweep;
      }
      const angle = Math.atan2(y - cy, x - cx);
      const tau = Math.PI * 2;
      const rel = (((angle - start) % tau) + tau) % tau;
      if (sweep >= tau - 1e-6 || rel <= sweep) {
        return Math.max(0, Math.abs(Math.hypot(x - cx, y - cy) - r) - hw);
      }
      const e0 = Math.hypot(x - cx - r * Math.cos(start), y - cy - r * Math.sin(start));
      const end = start + sweep;
      const e1 = Math.hypot(x - cx - r * Math.cos(end), y - cy - r * Math.sin(end));
      return Math.max(0, Math.min(e0, e1) - hw);
    }
    case Prim.Circle: {
      const c = layer.circles;
      const o = i * STRIDE.circles;
      const d = Math.hypot(x - (c[o] ?? 0), y - (c[o + 1] ?? 0));
      return Math.max(0, d - (c[o + 2] ?? 0), (c[o + 3] ?? 0) - d);
    }
    case Prim.Rect: {
      const r = layer.rects;
      const o = i * STRIDE.rects;
      const angle = r[o + 4] ?? 0;
      const cos = Math.cos(-angle);
      const sin = Math.sin(-angle);
      const dx = x - (r[o] ?? 0);
      const dy = y - (r[o + 1] ?? 0);
      const lx = Math.abs(dx * cos - dy * sin);
      const ly = Math.abs(dx * sin + dy * cos);
      const corner = r[o + 5] ?? 0;
      const qx = lx - ((r[o + 2] ?? 0) - corner);
      const qy = ly - ((r[o + 3] ?? 0) - corner);
      const outside = Math.hypot(Math.max(qx, 0), Math.max(qy, 0));
      return Math.max(0, outside + Math.min(Math.max(qx, qy), 0) - corner);
    }
    case Prim.Triangle: {
      const v = layer.vertices;
      const tri: number[] = [];
      for (let k = 0; k < 3; k += 1) {
        const index = layer.indices[i * 3 + k] ?? 0;
        tri.push(v[index * STRIDE.vertices] ?? 0, v[index * STRIDE.vertices + 1] ?? 0);
      }
      return pointInPolygon(x, y, tri) ? 0 : Infinity;
    }
    case Prim.Text: {
      const corners = layer.textCorners[i];
      if (corners === undefined) {
        return Infinity;
      }
      if (pointInPolygon(x, y, corners)) {
        return 0;
      }
      let best = Infinity;
      for (let k = 0; k < 4; k += 1) {
        const n = (k + 1) % 4;
        best = Math.min(
          best,
          segmentDistance(
            x,
            y,
            corners[2 * k] ?? 0,
            corners[2 * k + 1] ?? 0,
            corners[2 * n] ?? 0,
            corners[2 * n + 1] ?? 0,
          ),
        );
      }
      return best;
    }
  }
}
