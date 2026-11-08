/**
 * Selection queries: the object under the cursor, box and lasso selection.
 */
import { pointInPolygon, type Scene } from './scene';
import type { Box } from './types';

/** Objects selected by clicking anywhere inside their bounding box. */
const AREA_TYPES = new Set(['footprint', 'symbol', 'sheet']);

/** Lower wins: the most specific object under the cursor. */
const PRIORITY: Readonly<Record<string, number>> = {
  pad: 0,
  pin: 0,
  sheetPin: 0,
  via: 1,
  junction: 1,
  noConnect: 1,
  label: 1,
  track: 2,
  wire: 2,
  bus: 2,
  busEntry: 2,
  powerPort: 2,
  footprintText: 3,
  field: 3,
  boardText: 3,
  text: 3,
  boardShape: 4,
  shape: 4,
  footprint: 5,
  symbol: 5,
  sheet: 5,
  zone: 6,
};

export type LayerFilter = (layer: number) => boolean;

function inside(box: Box, x: number, y: number): boolean {
  return x >= box[0] && x <= box[2] && y >= box[1] && y <= box[3];
}

function area(box: Box): number {
  return (box[2] - box[0]) * (box[3] - box[1]);
}

/** `true` if the object has a primitive on `layer`. */
function onLayer(scene: Scene, obj: number, layer: number): boolean {
  const from = scene.refStart[obj] ?? 0;
  const to = scene.refStart[obj + 1] ?? 0;
  for (let r = from; r < to; r += 1) {
    if ((scene.refLayerKind[r] ?? 0) >> 3 === layer) {
      return true;
    }
  }
  return false;
}

/**
 * The best object at (x, y) within `tolerance` (scene units), or `null`.
 * Pads beat tracks, tracks beat footprints, footprints beat zones; within a
 * class, objects on the active layer and closer objects win.
 */
export function pick(
  scene: Scene,
  x: number,
  y: number,
  tolerance: number,
  visible: LayerFilter,
  active: number | null = null,
): number | null {
  const index = scene.index;
  if (index === null) {
    return null;
  }
  const candidates = index.search(x - tolerance, y - tolerance, x + tolerance, y + tolerance);
  let best: number | null = null;
  let bestKey: [number, number, number, number] = [Infinity, 0, 0, 0];
  for (const obj of candidates) {
    const type = scene.typeOf(obj);
    const box = scene.box(obj);
    let distance: number;
    if (AREA_TYPES.has(type)) {
      if (!inside(box, x, y) || !scene.isVisible(obj, visible)) {
        continue;
      }
      distance = 0;
    } else {
      distance = scene.distance(obj, x, y, visible);
      if (distance > tolerance) {
        continue;
      }
    }
    const key: [number, number, number, number] = [
      PRIORITY[type] ?? 4,
      active !== null && onLayer(scene, obj, active) ? 0 : 1,
      distance,
      area(box),
    ];
    if (
      key[0] < bestKey[0] ||
      (key[0] === bestKey[0] &&
        (key[1] < bestKey[1] ||
          (key[1] === bestKey[1] &&
            (key[2] < bestKey[2] || (key[2] === bestKey[2] && key[3] < bestKey[3])))))
    ) {
      best = obj;
      bestKey = key;
    }
  }
  return best;
}

/**
 * Top-level objects in a rectangle. `window`: completely inside (drag to the
 * right); `crossing`: touching (drag to the left).
 */
export function selectBox(
  scene: Scene,
  box: Box,
  mode: 'window' | 'crossing',
  visible: LayerFilter,
): number[] {
  const index = scene.index;
  if (index === null) {
    return [];
  }
  return index
    .search(box[0], box[1], box[2], box[3])
    .filter((obj) => {
      if (scene.parentOf(obj) !== null || !scene.isVisible(obj, visible)) {
        return false;
      }
      if (mode === 'crossing') {
        return true;
      }
      const b = scene.box(obj);
      return b[0] >= box[0] && b[1] >= box[1] && b[2] <= box[2] && b[3] <= box[3];
    })
    .sort((a, b) => a - b);
}

/** Top-level objects completely inside a lasso polygon (x0, y0, x1, y1, …). */
export function selectLasso(
  scene: Scene,
  polygon: readonly number[],
  visible: LayerFilter,
): number[] {
  const index = scene.index;
  if (index === null || polygon.length < 6) {
    return [];
  }
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (let i = 0; i < polygon.length; i += 2) {
    const px = polygon[i] ?? 0;
    const py = polygon[i + 1] ?? 0;
    minX = Math.min(minX, px);
    maxX = Math.max(maxX, px);
    minY = Math.min(minY, py);
    maxY = Math.max(maxY, py);
  }
  return index
    .search(minX, minY, maxX, maxY)
    .filter((obj) => {
      if (scene.parentOf(obj) !== null || !scene.isVisible(obj, visible)) {
        return false;
      }
      const b = scene.box(obj);
      return (
        pointInPolygon(b[0], b[1], polygon) &&
        pointInPolygon(b[2], b[1], polygon) &&
        pointInPolygon(b[2], b[3], polygon) &&
        pointInPolygon(b[0], b[3], polygon)
      );
    })
    .sort((a, b) => a - b);
}

/** Bounding box of several objects, or `null`. */
export function boundsOf(scene: Scene, objects: Iterable<number>): Box | null {
  let result: [number, number, number, number] | null = null;
  for (const obj of objects) {
    const b = scene.box(obj);
    result =
      result === null
        ? [b[0], b[1], b[2], b[3]]
        : [
            Math.min(result[0], b[0]),
            Math.min(result[1], b[1]),
            Math.max(result[2], b[2]),
            Math.max(result[3], b[3]),
          ];
  }
  return result;
}
