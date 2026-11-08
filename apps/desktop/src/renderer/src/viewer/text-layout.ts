/**
 * Text layout shared by rendering and hit-testing. It mirrors
 * `crates/ew-scene/src/text.rs`: a monospace font, `size` is the cap
 * height, every character advances `advance × size`.
 */
import { TEXT_BOLD, TEXT_ITALIC, TEXT_MIRROR, type TextTuple } from './types';

export interface FontMetrics {
  readonly advance: number;
  readonly linePitch: number;
}

/** A glyph in the atlas (pixel units of the atlas font size). */
export interface AtlasGlyph {
  /** Texture coordinates (normalized). */
  readonly u0: number;
  readonly v0: number;
  readonly u1: number;
  readonly v1: number;
  /** Bitmap offset from the pen position (x right, y up to the top edge). */
  readonly left: number;
  readonly top: number;
  /** Bitmap size. */
  readonly width: number;
  readonly height: number;
}

export interface GlyphSource {
  /** Advance of every glyph in atlas pixels (monospace). */
  readonly advancePx: number;
  glyph: (char: string) => AtlasGlyph | null;
}

/** Floats per glyph instance: p0 (2), ux (2), uy (2), uv (4), size, edge, object. */
export const GLYPH_STRIDE = 13;

/** SDF threshold of the edge (tiny-sdf with cutoff 0.25). */
export const EDGE_NORMAL = 0.75;
export const EDGE_BOLD = 0.62;
const ITALIC_SHEAR = 0.2;

/** Splits a text into Unicode code points (as Rust's `chars()`). */
export function characters(text: string): string[] {
  return Array.from(text);
}

/** Width and height of a text block (as in Rust). */
export function textExtent(text: string, size: number, font: FontMetrics): [number, number] {
  const lines = text.split('\n');
  let longest = 0;
  for (const line of lines) {
    longest = Math.max(longest, characters(line).length);
  }
  return [longest * font.advance * size, size + (lines.length - 1) * font.linePitch * size];
}

function boxOrigin(w: number, h: number, hAlign: number, vAlign: number): [number, number] {
  const x0 = hAlign < 0 ? 0 : hAlign === 0 ? -w / 2 : -w;
  const y0 = vAlign < 0 ? 0 : vAlign === 0 ? -h / 2 : -h;
  return [x0, y0];
}

/** The four corners of a text box (scene coordinates), as in Rust. */
export function textCorners(item: TextTuple, font: FontMetrics): Float64Array {
  const [text, x, y, size, rotation, hAlign, vAlign, flags] = item;
  const [w, h] = textExtent(text, size, font);
  let [x0] = boxOrigin(w, h, hAlign, vAlign);
  const [, y0] = boxOrigin(w, h, hAlign, vAlign);
  if ((flags & TEXT_MIRROR) !== 0) {
    x0 = -x0 - w;
  }
  const angle = (rotation * Math.PI) / 180;
  const cos = Math.cos(angle);
  const sin = Math.sin(angle);
  const local = [x0, y0, x0 + w, y0, x0 + w, y0 + h, x0, y0 + h];
  const out = new Float64Array(8);
  for (let i = 0; i < 8; i += 2) {
    const lx = local[i] ?? 0;
    const ly = local[i + 1] ?? 0;
    out[i] = x + lx * cos - ly * sin;
    out[i + 1] = y + lx * sin + ly * cos;
  }
  return out;
}

/** A growable buffer of glyph instances. */
export class GlyphBuffer {
  private floats = new Float32Array(GLYPH_STRIDE * 256);
  private words = new Uint32Array(this.floats.buffer);
  count = 0;

  private reserve(extra: number): void {
    const needed = (this.count + extra) * GLYPH_STRIDE;
    if (needed <= this.floats.length) {
      return;
    }
    let size = this.floats.length;
    while (size < needed) {
      size *= 2;
    }
    const next = new Float32Array(size);
    next.set(this.floats);
    this.floats = next;
    this.words = new Uint32Array(next.buffer);
  }

  push(values: readonly number[], obj: number): void {
    this.reserve(1);
    const base = this.count * GLYPH_STRIDE;
    for (let i = 0; i < GLYPH_STRIDE - 1; i += 1) {
      this.floats[base + i] = values[i] ?? 0;
    }
    this.words[base + GLYPH_STRIDE - 1] = obj;
    this.count += 1;
  }

  /** The used part of the buffer. */
  data(): Float32Array {
    return this.floats.subarray(0, this.count * GLYPH_STRIDE);
  }
}

/**
 * Lays out one text into glyph instances. Characters missing from the atlas
 * (and spaces) take up their advance but produce no glyph.
 */
export function layoutText(
  item: TextTuple,
  font: FontMetrics,
  source: GlyphSource,
  out: GlyphBuffer,
): void {
  const [text, x, y, size, rotation, hAlign, vAlign, flags, obj] = item;
  const lines = text.split('\n');
  const [w, h] = textExtent(text, size, font);
  const [x0, y0] = boxOrigin(w, h, hAlign, vAlign);
  const top = y0 + h;
  const step = font.advance * size;
  const k = step / source.advancePx;
  const mirror = (flags & TEXT_MIRROR) !== 0 ? -1 : 1;
  const shear = (flags & TEXT_ITALIC) !== 0 ? ITALIC_SHEAR : 0;
  const edge = (flags & TEXT_BOLD) !== 0 ? EDGE_BOLD : EDGE_NORMAL;
  const angle = (rotation * Math.PI) / 180;
  const cos = Math.cos(angle);
  const sin = Math.sin(angle);
  const toScene = (lx: number, ly: number): [number, number] => {
    const mx = lx * mirror;
    return [x + mx * cos - ly * sin, y + mx * sin + ly * cos];
  };
  const toVector = (lx: number, ly: number): [number, number] => {
    const mx = lx * mirror;
    return [mx * cos - ly * sin, mx * sin + ly * cos];
  };
  lines.forEach((line, index) => {
    const chars = characters(line);
    const lineWidth = chars.length * step;
    const lineX = hAlign < 0 ? x0 : hAlign === 0 ? x0 + (w - lineWidth) / 2 : x0 + w - lineWidth;
    const baseline = top - size - index * font.linePitch * size;
    chars.forEach((char, column) => {
      const glyph = source.glyph(char);
      if (glyph === null || glyph.width === 0) {
        return;
      }
      const gx = lineX + column * step + glyph.left * k;
      const gy = baseline + (glyph.top - glyph.height) * k;
      const gw = glyph.width * k;
      const gh = glyph.height * k;
      // Italic: shear around the baseline.
      const [px, py] = toScene(gx + shear * (gy - baseline), gy);
      const [uxx, uxy] = toVector(gw, 0);
      const [uyx, uyy] = toVector(shear * gh, gh);
      out.push(
        [px, py, uxx, uxy, uyx, uyy, glyph.u0, glyph.v0, glyph.u1, glyph.v1, size, edge],
        obj,
      );
    });
  });
}
