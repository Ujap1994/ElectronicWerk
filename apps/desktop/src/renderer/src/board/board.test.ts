import { describe, expect, it } from 'vitest';

import { dedupe, otherLayer, silkOf, viaBetween, viaTypeOf } from './editor';
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

  it('chooses the via for a layer change', () => {
    const layers = ['cu.top', 'cu.in1', 'cu.in2', 'cu.bottom'];
    const all = ['through', 'blind', 'buried', 'micro'];
    const through = { from: 'cu.top', to: 'cu.bottom', micro: false };
    expect(viaBetween('cu.top', 'cu.in1', layers, 'through', ['through'])).toEqual(through);
    expect(viaBetween('cu.in2', 'cu.top', layers, 'blind', all)).toEqual({
      from: 'cu.top',
      to: 'cu.in2',
      micro: false,
    });
    expect(viaBetween('cu.top', 'cu.bottom', layers, 'blind', ['through'])).toEqual(through);
    expect(viaBetween('cu.in1', 'cu.in2', layers, 'blind', ['through'])).toBe('notAllowed');
    expect(viaBetween('cu.bottom', 'cu.in2', layers, 'micro', all)).toEqual({
      from: 'cu.in2',
      to: 'cu.bottom',
      micro: true,
    });
    expect(viaBetween('cu.top', 'cu.in2', layers, 'micro', all)).toBe('notNeighbours');
    expect(viaBetween('cu.top', 'cu.in1', layers, 'micro', ['through', 'blind'])).toBe(
      'notAllowed',
    );
  });

  it('names via types like the engine', () => {
    expect(viaTypeOf('cu.top', 'cu.bottom', false)).toBe('through');
    expect(viaTypeOf('cu.top', 'cu.in1', false)).toBe('blind');
    expect(viaTypeOf('cu.in2', 'cu.bottom', false)).toBe('blind');
    expect(viaTypeOf('cu.in1', 'cu.in2', false)).toBe('buried');
    expect(viaTypeOf('cu.top', 'cu.in1', true)).toBe('micro');
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
