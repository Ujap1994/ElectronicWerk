import { describe, expect, it } from 'vitest';

import { visibleRuns } from './culling';

describe('visibleRuns', () => {
  // Three chunks of four instances side by side, the last one half full.
  const chunks = new Float32Array([0, 0, 10, 10, 10, 0, 20, 10, 20, 0, 30, 10]);

  it('merges neighbouring visible chunks into one draw call', () => {
    expect(visibleRuns(chunks, 10, 4, [-100, -100, 100, 100])).toEqual([[0, 10]]);
  });

  it('skips chunks outside the view', () => {
    expect(visibleRuns(chunks, 10, 4, [12, 2, 18, 8])).toEqual([[4, 4]]);
    expect(visibleRuns(chunks, 10, 4, [25, 0, 40, 5])).toEqual([[8, 2]]);
    expect(visibleRuns(chunks, 10, 4, [50, 50, 60, 60])).toEqual([]);
  });

  it('splits runs around a gap', () => {
    const gap = new Float32Array([0, 0, 1, 1, 50, 50, 51, 51, 2, 0, 3, 1]);
    expect(visibleRuns(gap, 12, 4, [0, 0, 5, 5])).toEqual([
      [0, 4],
      [8, 4],
    ]);
  });
});
