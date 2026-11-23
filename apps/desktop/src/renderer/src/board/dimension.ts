/**
 * Dimensions while they are placed: the offset a third click gives, and the
 * lines of the preview. The engine draws the finished dimension
 * (`ew_model::dimension`); this follows the same construction.
 */
import type { Point } from './editor';

export type DimensionKind =
  'aligned' | 'horizontal' | 'vertical' | 'radial' | 'diameter' | 'leader';

export const DIMENSION_KINDS: readonly DimensionKind[] = [
  'aligned',
  'horizontal',
  'vertical',
  'radial',
  'diameter',
  'leader',
];

/** How many clicks a dimension takes (a leader asks for its text after two). */
export function clicksOf(kind: DimensionKind): number {
  return kind === 'leader' ? 2 : 3;
}

function unit(dx: number, dy: number): [number, number] {
  const length = Math.hypot(dx, dy);
  return length > 0 ? [dx / length, dy / length] : [1, 0];
}

/** The offset of a dimension from its two points and the third click. */
export function offsetOf(kind: DimensionKind, a: Point, b: Point, at: Point): number {
  switch (kind) {
    case 'horizontal':
      return Math.round(at[1] - a[1]);
    case 'vertical':
      return Math.round(at[0] - a[0]);
    case 'aligned': {
      const [ux, uy] = unit(b[0] - a[0], b[1] - a[1]);
      // Along the left normal of a → b.
      return Math.round((at[0] - a[0]) * -uy + (at[1] - a[1]) * ux);
    }
    case 'radial':
    case 'diameter': {
      const [ux, uy] = unit(b[0] - a[0], b[1] - a[1]);
      return Math.max(0, Math.round((at[0] - b[0]) * ux + (at[1] - b[1]) * uy));
    }
    case 'leader':
      return 0;
  }
}

export interface DimensionPreview {
  readonly lines: readonly (readonly [Point, Point])[];
  /** The measured value (nm). */
  readonly value: number;
  /** Where the value goes. */
  readonly at: Point;
}

/** Lines of a dimension being placed (without arrows and text). */
export function previewOf(
  kind: DimensionKind,
  a: Point,
  b: Point,
  offset: number,
): DimensionPreview {
  switch (kind) {
    case 'horizontal': {
      const y = a[1] + offset;
      const from: Point = [a[0], y];
      const to: Point = [b[0], y];
      return {
        lines: [
          [a, from],
          [b, to],
          [from, to],
        ],
        value: Math.abs(b[0] - a[0]),
        at: [(a[0] + b[0]) / 2, y],
      };
    }
    case 'vertical': {
      const x = a[0] + offset;
      const from: Point = [x, a[1]];
      const to: Point = [x, b[1]];
      return {
        lines: [
          [a, from],
          [b, to],
          [from, to],
        ],
        value: Math.abs(b[1] - a[1]),
        at: [x, (a[1] + b[1]) / 2],
      };
    }
    case 'aligned': {
      const [ux, uy] = unit(b[0] - a[0], b[1] - a[1]);
      const from: Point = [a[0] - uy * offset, a[1] + ux * offset];
      const to: Point = [b[0] - uy * offset, b[1] + ux * offset];
      return {
        lines: [
          [a, from],
          [b, to],
          [from, to],
        ],
        value: Math.hypot(b[0] - a[0], b[1] - a[1]),
        at: [(from[0] + to[0]) / 2, (from[1] + to[1]) / 2],
      };
    }
    case 'radial':
    case 'diameter': {
      const radius = Math.hypot(b[0] - a[0], b[1] - a[1]);
      const [ux, uy] = unit(b[0] - a[0], b[1] - a[1]);
      const beyond: Point = [b[0] + ux * offset, b[1] + uy * offset];
      const start: Point = kind === 'diameter' ? [a[0] - (b[0] - a[0]), a[1] - (b[1] - a[1])] : a;
      return {
        lines: [[start, beyond]],
        value: kind === 'diameter' ? radius * 2 : radius,
        at: beyond,
      };
    }
    case 'leader':
      return { lines: [[a, b]], value: 0, at: b };
  }
}
