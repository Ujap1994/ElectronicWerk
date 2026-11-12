import { describe, expect, it } from 'vitest';

import { dedupe, otherLayer, silkOf } from './editor';
import { standardStackup } from './stackup';

describe('layout editor helpers', () => {
  it('finds the silkscreen and the via target layer', () => {
    expect(silkOf('cu.top')).toBe('silk.top');
    expect(silkOf('cu.bottom')).toBe('silk.bottom');
    expect(silkOf('cu.in2')).toBe('silk.top');
    const layers = ['cu.top', 'cu.in1', 'cu.in2', 'cu.bottom'];
    expect(otherLayer('cu.top', layers)).toBe('cu.bottom');
    expect(otherLayer('cu.bottom', layers)).toBe('cu.top');
    expect(otherLayer('cu.in1', layers)).toBe('cu.top');
  });

  it('drops repeated points', () => {
    expect(
      dedupe([
        [0, 0],
        [0, 0],
        [1, 0],
        [1, 0],
        [0, 0],
      ]),
    ).toEqual([
      [0, 0],
      [1, 0],
      [0, 0],
    ]);
  });

  it('builds standard stack-ups with the requested thickness', () => {
    for (const count of [1, 2, 4, 6, 8]) {
      const stackup = standardStackup(count, 1_600_000);
      const copper = stackup.layers.filter((layer) => layer.type === 'copper');
      expect(copper).toHaveLength(count);
      const total = stackup.layers.reduce(
        (sum, layer) => sum + ('thickness' in layer ? layer.thickness : 0),
        0,
      );
      expect(total).toBe(1_600_000);
    }
    const four = standardStackup(4, 1_600_000);
    const kinds = four.layers.flatMap((layer) => (layer.type === 'dielectric' ? [layer.kind] : []));
    expect(kinds).toEqual(['prepreg', 'core', 'prepreg']);
  });
});
