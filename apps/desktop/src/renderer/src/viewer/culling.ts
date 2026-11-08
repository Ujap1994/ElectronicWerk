/**
 * View culling: primitives are sorted along a Z-order curve in the engine
 * and grouped into chunks with bounding boxes, so only the chunks that touch
 * the view are drawn.
 */
import type { Box } from './types';

/** Contiguous runs of visible chunks: [first instance, count] pairs. */
export function visibleRuns(
  chunks: Float32Array,
  count: number,
  chunkSize: number,
  view: Box,
): [number, number][] {
  const runs: [number, number][] = [];
  const n = Math.ceil(count / chunkSize);
  let runStart = -1;
  for (let i = 0; i <= n; i += 1) {
    const visible =
      i < n &&
      (chunks[i * 4] ?? 0) <= view[2] &&
      (chunks[i * 4 + 2] ?? 0) >= view[0] &&
      (chunks[i * 4 + 1] ?? 0) <= view[3] &&
      (chunks[i * 4 + 3] ?? 0) >= view[1];
    if (visible && runStart < 0) {
      runStart = i;
    } else if (!visible && runStart >= 0) {
      const first = runStart * chunkSize;
      runs.push([first, Math.min(count, i * chunkSize) - first]);
      runStart = -1;
    }
  }
  return runs;
}
