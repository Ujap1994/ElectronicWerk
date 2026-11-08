/**
 * Signed-distance-field glyph atlas. Glyphs are rasterized on demand from a
 * bundled monospace font (JetBrains Mono, SIL Open Font License) with
 * tiny-sdf and packed into one single-channel texture.
 */
import latinExtUrl from '@fontsource/jetbrains-mono/files/jetbrains-mono-latin-ext-400-normal.woff2?url';
import latinUrl from '@fontsource/jetbrains-mono/files/jetbrains-mono-latin-400-normal.woff2?url';
import greekUrl from '@fontsource/jetbrains-mono/files/jetbrains-mono-greek-400-normal.woff2?url';
import TinySDF from '@mapbox/tiny-sdf';

import type { AtlasGlyph, GlyphSource } from './text-layout';

export const CANVAS_FONT = 'EW Canvas Mono';

const FACES: readonly (readonly [string, string])[] = [
  [
    latinUrl,
    'U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD',
  ],
  [
    latinExtUrl,
    'U+0100-02BA, U+02BD-02C5, U+02C7-02CC, U+02CE-02D7, U+02DD-02FF, U+1E00-1E9F, U+1EF2-1EFF, U+2020, U+20A0-20AB, U+20AD-20C0, U+2113, U+2C60-2C7F, U+A720-A7FF',
  ],
  [greekUrl, 'U+0370-0377, U+037A-037F, U+0384-038A, U+038C, U+038E-03A1, U+03A3-03FF'],
];

let fontLoading: Promise<void> | null = null;

/** Loads the canvas font once; resolves even if loading fails (fallback font). */
export function loadCanvasFont(): Promise<void> {
  fontLoading ??= Promise.all(
    FACES.map(async ([url, unicodeRange]) => {
      const face = new FontFace(CANVAS_FONT, `url(${url})`, { unicodeRange });
      document.fonts.add(face);
      await face.load();
    }),
  ).then(
    () => undefined,
    () => undefined,
  );
  return fontLoading;
}

const SIZE = 2048;
const FONT_PX = 48;
const BUFFER = 6;
const RADIUS = 12;

export class GlyphAtlas implements GlyphSource {
  readonly size = SIZE;
  readonly pixels = new Uint8Array(SIZE * SIZE);
  readonly advancePx: number;
  /** Rows changed since the last upload, or `null`. */
  dirty: { y0: number; y1: number } | null = null;
  private readonly sdf: TinySDF;
  private readonly glyphs = new Map<string, AtlasGlyph | null>();
  private x = 0;
  private y = 0;
  private rowHeight = 0;

  constructor() {
    this.sdf = new TinySDF({
      fontSize: FONT_PX,
      buffer: BUFFER,
      radius: RADIUS,
      cutoff: 0.25,
      fontFamily: `"${CANVAS_FONT}", ui-monospace, monospace`,
    });
    this.advancePx = this.sdf.draw('M').glyphAdvance || FONT_PX * 0.6;
  }

  glyph(char: string): AtlasGlyph | null {
    const cached = this.glyphs.get(char);
    if (cached !== undefined) {
      return cached;
    }
    const result = char.trim() === '' ? null : this.add(char);
    this.glyphs.set(char, result);
    return result;
  }

  private add(char: string): AtlasGlyph | null {
    const g = this.sdf.draw(char);
    if (g.glyphWidth === 0 || g.glyphHeight === 0) {
      return null;
    }
    if (this.x + g.width > SIZE) {
      this.x = 0;
      this.y += this.rowHeight + 1;
      this.rowHeight = 0;
    }
    if (this.y + g.height > SIZE) {
      return null;
    }
    for (let row = 0; row < g.height; row += 1) {
      this.pixels.set(
        g.data.subarray(row * g.width, (row + 1) * g.width),
        (this.y + row) * SIZE + this.x,
      );
    }
    const glyph: AtlasGlyph = {
      u0: this.x / SIZE,
      v0: this.y / SIZE,
      u1: (this.x + g.width) / SIZE,
      v1: (this.y + g.height) / SIZE,
      left: g.glyphLeft - BUFFER,
      top: g.glyphTop + BUFFER,
      width: g.width,
      height: g.height,
    };
    this.dirty = {
      y0: Math.min(this.dirty?.y0 ?? this.y, this.y),
      y1: Math.max(this.dirty?.y1 ?? 0, this.y + g.height),
    };
    this.x += g.width + 1;
    this.rowHeight = Math.max(this.rowHeight, g.height);
    return glyph;
  }
}
