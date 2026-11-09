/**
 * SVG of a sheet, made from the same scene the 2D view draws: every
 * primitive of every visible layer, in drawing order, in millimetres with y
 * pointing down. Used for the SVG export and, one page per sheet, for
 * printing to PDF.
 */
import { drawOrder, layerColor } from '../viewer/layers';
import type { Scene } from '../viewer/scene';
import { STRIDE, TEXT_MIRROR } from '../viewer/types';

const MM = 1e6;
/** Cap height of the monospace export font relative to its em size. */
const CAP_HEIGHT = 0.72;
/** Thinnest line in the export (mm). */
const MIN_STROKE = 0.05;

function n(value: number): string {
  return (Math.round(value * 1e4) / 1e4).toString();
}

export function escapeXml(text: string): string {
  return text
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;');
}

export interface SvgSheet {
  readonly svg: string;
  /** Page size in millimetres. */
  readonly width: number;
  readonly height: number;
}

/** The SVG of a schematic scene (its page) or of a board (its bounds). */
export function sceneToSvg(scene: Scene, title: string): SvgSheet {
  const meta = scene.meta;
  const [ox, oy] = meta.origin;
  const box = meta.bounds ?? [0, 0, 0, 0];
  const bounds = meta.page ?? [box[2] - box[0], box[3] - box[1]];
  const left = meta.page === null ? box[0] : 0;
  const top = meta.page === null ? box[3] : meta.page[1];
  const width = bounds[0] / MM;
  const height = bounds[1] / MM;
  // Scene coordinates (relative to the origin, y up) to page millimetres.
  const X = (x: number): number => (x + ox - left) / MM;
  const Y = (y: number): number => (top - (y + oy)) / MM;
  const parts: string[] = [];
  parts.push(
    `<svg xmlns="http://www.w3.org/2000/svg" width="${n(width)}mm" height="${n(height)}mm" viewBox="0 0 ${n(width)} ${n(height)}">`,
    `<title>${escapeXml(title)}</title>`,
    `<rect width="100%" height="100%" fill="#ffffff"/>`,
  );
  const font = meta.font;
  for (const index of drawOrder(meta.layers, null, false)) {
    const layer = scene.layers[index];
    if (layer?.meta.visible !== true) {
      continue;
    }
    const color =
      meta.kind === 'schematic' && layer.meta.key === 'sch.page'
        ? '#7a3b3b'
        : layerColor(layer.meta, 'light');
    const body: string[] = [];
    const seg = layer.segments;
    for (let i = 0; i < seg.length; i += STRIDE.segments) {
      const w = Math.max(((seg[i + 4] ?? 0) * 2) / MM, MIN_STROKE);
      body.push(
        `<line x1="${n(X(seg[i] ?? 0))}" y1="${n(Y(seg[i + 1] ?? 0))}" x2="${n(X(seg[i + 2] ?? 0))}" y2="${n(Y(seg[i + 3] ?? 0))}" stroke-width="${n(w)}"/>`,
      );
    }
    const arcs = layer.arcs;
    for (let i = 0; i < arcs.length; i += STRIDE.arcs) {
      const cx = arcs[i] ?? 0;
      const cy = arcs[i + 1] ?? 0;
      const r = arcs[i + 2] ?? 0;
      const hw = arcs[i + 3] ?? 0;
      const start = arcs[i + 4] ?? 0;
      const sweep = arcs[i + 5] ?? 0;
      const w = Math.max((hw * 2) / MM, MIN_STROKE);
      if (Math.abs(sweep) >= 2 * Math.PI - 1e-3) {
        body.push(
          `<circle cx="${n(X(cx))}" cy="${n(Y(cy))}" r="${n(r / MM)}" fill="none" stroke-width="${n(w)}"/>`,
        );
        continue;
      }
      const a = [cx + r * Math.cos(start), cy + r * Math.sin(start)] as const;
      const b = [cx + r * Math.cos(start + sweep), cy + r * Math.sin(start + sweep)] as const;
      const large = Math.abs(sweep) > Math.PI ? 1 : 0;
      // y is flipped: a counterclockwise arc is drawn clockwise.
      const flag = sweep > 0 ? 0 : 1;
      body.push(
        `<path d="M${n(X(a[0]))} ${n(Y(a[1]))}A${n(r / MM)} ${n(r / MM)} 0 ${large} ${flag} ${n(X(b[0]))} ${n(Y(b[1]))}" fill="none" stroke-width="${n(w)}"/>`,
      );
    }
    const circles = layer.circles;
    for (let i = 0; i < circles.length; i += STRIDE.circles) {
      const cx = circles[i] ?? 0;
      const cy = circles[i + 1] ?? 0;
      const outer = circles[i + 2] ?? 0;
      const inner = circles[i + 3] ?? 0;
      if (inner <= 0) {
        body.push(
          `<circle cx="${n(X(cx))}" cy="${n(Y(cy))}" r="${n(outer / MM)}" fill="${color}" stroke="none"/>`,
        );
      } else {
        body.push(
          `<circle cx="${n(X(cx))}" cy="${n(Y(cy))}" r="${n((outer + inner) / 2 / MM)}" fill="none" stroke-width="${n((outer - inner) / MM)}"/>`,
        );
      }
    }
    const rects = layer.rects;
    for (let i = 0; i < rects.length; i += STRIDE.rects) {
      const cx = rects[i] ?? 0;
      const cy = rects[i + 1] ?? 0;
      const hx = rects[i + 2] ?? 0;
      const hy = rects[i + 3] ?? 0;
      const angle = rects[i + 4] ?? 0;
      const round = rects[i + 5] ?? 0;
      body.push(
        `<rect x="${n(-hx / MM)}" y="${n(-hy / MM)}" width="${n((2 * hx) / MM)}" height="${n((2 * hy) / MM)}" rx="${n(round / MM)}" transform="translate(${n(X(cx))} ${n(Y(cy))}) rotate(${n((-angle * 180) / Math.PI)})" fill="${color}" stroke="none"/>`,
      );
    }
    const vertices = layer.vertices;
    const indices = layer.indices;
    if (indices.length > 0) {
      const path: string[] = [];
      const at = (v: number): string =>
        `${n(X(vertices[v * STRIDE.vertices] ?? 0))} ${n(Y(vertices[v * STRIDE.vertices + 1] ?? 0))}`;
      for (let i = 0; i + 2 < indices.length; i += 3) {
        path.push(`M${at(indices[i] ?? 0)}L${at(indices[i + 1] ?? 0)}L${at(indices[i + 2] ?? 0)}Z`);
      }
      body.push(
        `<path d="${path.join('')}" fill="${color}" stroke="${color}" stroke-width="0.01"/>`,
      );
    }
    for (const [text, x, y, size, rotation, hAlign, vAlign, flags] of layer.meta.texts) {
      if (text === '') {
        continue;
      }
      const lines = text.split('\n');
      const longest = Math.max(...lines.map((line) => Array.from(line).length));
      const w = longest * font.advance * size;
      const h = size + (lines.length - 1) * font.linePitch * size;
      let x0 = hAlign < 0 ? 0 : hAlign === 0 ? -w / 2 : -w;
      const y0 = vAlign < 0 ? 0 : vAlign === 0 ? -h / 2 : -h;
      const mirror = (flags & TEXT_MIRROR) !== 0;
      if (mirror) {
        x0 = -x0 - w;
      }
      const spans = lines.map((line, row) => {
        const baseline = y0 + h - size - row * font.linePitch * size;
        const length = Array.from(line).length * font.advance * size;
        return length === 0
          ? ''
          : `<text x="${n(x0 / MM)}" y="${n(-baseline / MM)}" textLength="${n(length / MM)}" lengthAdjust="spacingAndGlyphs">${escapeXml(line)}</text>`;
      });
      body.push(
        `<g transform="translate(${n(X(x))} ${n(Y(y))}) rotate(${n(-rotation)})${mirror ? ' scale(-1 1)' : ''}" font-size="${n(size / CAP_HEIGHT / MM)}">${spans.join('')}</g>`,
      );
    }
    if (body.length > 0) {
      parts.push(
        `<g stroke="${color}" stroke-linecap="round" stroke-linejoin="round" fill="${color}" font-family="'DejaVu Sans Mono', Menlo, Consolas, monospace" data-layer="${escapeXml(layer.meta.key)}">`,
        ...body,
        '</g>',
      );
    }
  }
  parts.push('</svg>');
  return { svg: parts.join('\n'), width, height };
}

/** A printable HTML document with one page per sheet (for PDF). */
export function printDocument(sheets: readonly SvgSheet[], title: string): string {
  const pages = sheets.map(
    (sheet, index) =>
      `<section class="page p${index.toString()}">${sheet.svg.replace('<svg ', '<svg class="sheet" ')}</section>`,
  );
  const sizes = sheets
    .map(
      (sheet, index) =>
        `@page p${index.toString()} { size: ${n(sheet.width)}mm ${n(sheet.height)}mm; margin: 0; } .p${index.toString()} { page: p${index.toString()}; width: ${n(sheet.width)}mm; height: ${n(sheet.height)}mm; }`,
    )
    .join('\n');
  return `<!doctype html><html><head><meta charset="utf-8"><title>${escapeXml(title)}</title><style>
html, body { margin: 0; padding: 0; }
.page { break-after: page; overflow: hidden; }
.page:last-child { break-after: auto; }
.sheet { display: block; width: 100%; height: 100%; }
${sizes}
</style></head><body>${pages.join('')}</body></html>`;
}
