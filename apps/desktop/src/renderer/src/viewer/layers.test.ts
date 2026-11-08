import { describe, expect, it } from 'vitest';

import { defaultOpacity, drawOrder, layerColor, rgb, rgba } from './layers';
import type { SceneLayer } from './types';

function layer(
  key: string,
  group: SceneLayer['group'],
  side: SceneLayer['side'],
  z: number,
): SceneLayer {
  const empty = { offset: 0, count: 0, chunks: 0 };
  return {
    key,
    group,
    side,
    name: null,
    z,
    visible: true,
    segments: empty,
    arcs: empty,
    circles: empty,
    rects: empty,
    mesh: { vertices: 0, vertexCount: 0, indices: 0, indexCount: 0 },
    texts: [],
  };
}

const LAYERS = [
  layer('cu.top', 'copper', 'top', 300),
  layer('cu.in1', 'copper', null, 199),
  layer('cu.bottom', 'copper', 'bottom', 100),
  layer('silk.top', 'silkscreen', 'top', 404),
  layer('silk.bottom', 'silkscreen', 'bottom', 6),
  layer('outline', 'outline', null, 500),
  layer('holes', 'holes', null, 1000),
];
const keys = (order: number[]): string[] => order.map((i) => LAYERS[i]?.key ?? '');

describe('drawOrder', () => {
  it('draws bottom layers first in the top view', () => {
    expect(keys(drawOrder(LAYERS, null, false))).toEqual([
      'silk.bottom',
      'cu.bottom',
      'cu.in1',
      'cu.top',
      'silk.top',
      'outline',
      'holes',
    ]);
  });

  it('reverses the sides when seen from the bottom', () => {
    expect(keys(drawOrder(LAYERS, null, true))).toEqual([
      'silk.top',
      'cu.top',
      'cu.in1',
      'cu.bottom',
      'silk.bottom',
      'outline',
      'holes',
    ]);
  });

  it('puts the active layer above the other sided layers', () => {
    expect(keys(drawOrder(LAYERS, 2, false))).toEqual([
      'silk.bottom',
      'cu.in1',
      'cu.top',
      'silk.top',
      'cu.bottom',
      'outline',
      'holes',
    ]);
  });
});

describe('colours', () => {
  it('has fixed colours, numbered inner layers and sheet themes', () => {
    expect(layerColor(layer('cu.top', 'copper', 'top', 0), 'dark')).toBe('#c83434');
    expect(layerColor(layer('cu.in1', 'copper', null, 0), 'dark')).toBe(
      layerColor(layer('cu.in9', 'copper', null, 0), 'dark'),
    );
    const wires = layer('sch.wires', 'schematic', null, 0);
    expect(layerColor(wires, 'light')).not.toBe(layerColor(wires, 'dark'));
    expect(defaultOpacity(layer('mask.top', 'solderMask', 'top', 0))).toBeLessThan(1);
  });

  it('parses colours', () => {
    expect(rgb('#ff8000')).toEqual([1, 128 / 255, 0]);
    expect(rgba('rgba(255, 0, 51, 0.5)')).toEqual([1, 0, 0.2, 0.5]);
    expect(rgba('#000000')).toEqual([0, 0, 0, 1]);
  });
});
