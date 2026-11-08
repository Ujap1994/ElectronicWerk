import { describe, expect, it } from 'vitest';

import {
  GLYPH_STRIDE,
  GlyphBuffer,
  layoutText,
  textCorners,
  textExtent,
  type GlyphSource,
} from './text-layout';
import { TEXT_MIRROR, type TextTuple } from './types';

const FONT = { advance: 0.6 / 0.73, linePitch: 1.6 };

/** Every glyph is a 10 × 20 px box; the advance is 10 px. */
const SOURCE: GlyphSource = {
  advancePx: 10,
  glyph: (char) =>
    char === '?' || char === ' '
      ? null
      : { u0: 0, v0: 0, u1: 0.1, v1: 0.2, left: 0, top: 15, width: 10, height: 20 },
};

const item = (text: string, h: number, v: number, flags = 0, rotation = 0): TextTuple => [
  text,
  100,
  50,
  10,
  rotation,
  h,
  v,
  flags,
  7,
];

describe('text layout', () => {
  it('measures like the engine', () => {
    const [w, h] = textExtent('ab\nabcd', 10, FONT);
    expect(w).toBeCloseTo(4 * FONT.advance * 10);
    expect(h).toBeCloseTo(10 + 1.6 * 10);
    expect(textExtent('Ωµ', 1, FONT)[0]).toBeCloseTo(2 * FONT.advance);
  });

  it('aligns and mirrors the text box', () => {
    const left = textCorners(item('ab', -1, -1), FONT);
    expect([left[0], left[1]]).toEqual([100, 50]);
    const right = textCorners(item('ab', 1, 1), FONT);
    expect(right[4]).toBeCloseTo(100);
    expect(right[5]).toBeCloseTo(50);
    const mirrored = textCorners(item('abc', -1, -1, TEXT_MIRROR), FONT);
    expect(Math.max(mirrored[0] ?? 0, mirrored[2] ?? 0)).toBeCloseTo(100);
  });

  it('rotates the box around the anchor', () => {
    const c = textCorners(item('ab', -1, -1, 0, 90), FONT);
    // The text runs upwards from the anchor.
    expect(c[0]).toBeCloseTo(100);
    expect(c[3]).toBeCloseTo(50 + 2 * FONT.advance * 10);
  });

  it('creates one glyph per visible character', () => {
    const out = new GlyphBuffer();
    layoutText(item('a b?c', -1, -1), FONT, SOURCE, out);
    expect(out.count).toBe(3);
    const data = out.data();
    expect(data.length).toBe(3 * GLYPH_STRIDE);
    // Second glyph starts two advances to the right of the first.
    const step = FONT.advance * 10;
    expect((data[GLYPH_STRIDE] ?? 0) - (data[0] ?? 0)).toBeCloseTo(2 * step);
    // The object index is stored as u32 in the last word.
    expect(new Uint32Array(data.buffer)[GLYPH_STRIDE - 1]).toBe(7);
  });

  it('grows the buffer as needed', () => {
    const out = new GlyphBuffer();
    layoutText(item('x'.repeat(1000), -1, -1), FONT, SOURCE, out);
    expect(out.count).toBe(1000);
  });
});
