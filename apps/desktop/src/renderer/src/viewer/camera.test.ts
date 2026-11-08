import { describe, expect, it } from 'vitest';

import { Camera, gridStep, interpolate, MAX_SCALE, zoomPercent } from './camera';

function camera(): Camera {
  const c = new Camera();
  c.setViewport(800, 600);
  c.set({ cx: 1000, cy: -500, scale: 0.01, flipped: false });
  return c;
}

describe('Camera', () => {
  it('maps scene and screen coordinates both ways (y up on screen means smaller y)', () => {
    const c = camera();
    expect(c.toScreen(1000, -500)).toEqual([400, 300]);
    const [sx, sy] = c.toScreen(2000, 500);
    expect([sx, sy]).toEqual([410, 290]);
    expect(c.toScene(sx, sy)).toEqual([2000, 500]);
  });

  it('mirrors x when flipped', () => {
    const c = camera();
    c.flip();
    expect(c.toScreen(2000, -500)).toEqual([390, 300]);
    expect(c.toScene(390, 300)).toEqual([2000, -500]);
  });

  it('keeps the point under the cursor when zooming', () => {
    const c = camera();
    const before = c.toScene(123, 456);
    c.zoomAt(123, 456, 3);
    const after = c.toScene(123, 456);
    expect(after[0]).toBeCloseTo(before[0], 6);
    expect(after[1]).toBeCloseTo(before[1], 6);
    expect(c.scale).toBeCloseTo(0.03);
  });

  it('pans by screen distances', () => {
    const c = camera();
    c.pan(100, 50);
    expect(c.cx).toBeCloseTo(1000 - 100 / 0.01);
    expect(c.cy).toBeCloseTo(-500 + 50 / 0.01);
  });

  it('fits a box with a margin and limits zooming out', () => {
    const c = camera();
    const fit = c.fitState([0, 0, 80_000_000, 60_000_000], 0);
    expect(fit.cx).toBe(40_000_000);
    expect(fit.scale).toBeCloseTo(800 / 80_000_000);
    c.limitTo([0, 0, 80_000_000, 60_000_000]);
    c.set({ ...fit, scale: fit.scale / 100 });
    expect(c.scale).toBeCloseTo(fit.scale / 10);
    c.set({ ...fit, scale: 1 });
    expect(c.scale).toBe(MAX_SCALE);
  });

  it('reports the visible rectangle', () => {
    expect(camera().visible()).toEqual([
      1000 - 40_000,
      -500 - 30_000,
      1000 + 40_000,
      -500 + 30_000,
    ]);
  });
});

describe('interpolate', () => {
  it('starts and ends at the given states and zooms in log space', () => {
    const from = { cx: 0, cy: 0, scale: 1, flipped: false };
    const to = { cx: 10, cy: 20, scale: 100, flipped: false };
    expect(interpolate(from, to, 0)).toEqual(from);
    const end = interpolate(from, to, 1);
    expect(end.cx).toBe(10);
    expect(end.scale).toBeCloseTo(100);
    expect(interpolate(from, to, 0.5).scale).toBeCloseTo(10);
  });
});

describe('gridStep', () => {
  it('coarsens the grid until it is wide enough on screen', () => {
    expect(gridStep(1_270_000, 1e-4)).toBe(1_270_000);
    expect(gridStep(1_270_000, 1e-6)).toBe(12_700_000);
    expect(gridStep(0, 1)).toBe(0);
  });
});

describe('zoomPercent', () => {
  it('is 100 % when a millimetre has its physical size', () => {
    expect(zoomPercent(96 / 25.4 / 1e6, 96 / 25.4)).toBeCloseTo(100);
  });
});
