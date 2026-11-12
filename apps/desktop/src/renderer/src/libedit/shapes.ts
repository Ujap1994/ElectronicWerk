/** Pad shapes as the tables and properties offer them. */
import type { PadShape } from './types';

/** The name of a pad shape for the select. */
export function shapeKey(
  shape: PadShape,
): 'circle' | 'rect' | 'oval' | 'roundRect' | 'trapezoid' | 'custom' {
  if (typeof shape === 'string') {
    return shape;
  }
  if ('roundRect' in shape) {
    return 'roundRect';
  }
  return 'trapezoid' in shape ? 'trapezoid' : 'custom';
}

/** A pad shape of a kind (keeping the pad's size). */
export function shapeOf(
  key: ReturnType<typeof shapeKey>,
  size: readonly [number, number],
): PadShape {
  switch (key) {
    case 'roundRect':
      return { roundRect: { ratio: 250 } };
    case 'trapezoid':
      return { trapezoid: { delta: [0, Math.round(size[1] / 4)] } };
    case 'custom': {
      const [hw, hh] = [size[0] / 2, size[1] / 2];
      return {
        custom: {
          points: [
            [-hw, -hh],
            [hw, -hh],
            [hw, hh],
            [-hw, hh],
          ],
        },
      };
    }
    case 'circle':
    case 'rect':
    case 'oval':
      return key;
  }
}

export const SHAPE_KEYS = ['circle', 'rect', 'oval', 'roundRect', 'trapezoid', 'custom'] as const;
