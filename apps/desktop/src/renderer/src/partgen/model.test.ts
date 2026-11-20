import { describe, expect, it } from 'vitest';

import type { Draft } from '@shared/datasheet';

import {
  addPin,
  dimensionKeys,
  dimensionValue,
  firstPackage,
  missingPads,
  padOrder,
  parseNumbers,
  pinRows,
  removePin,
  setDimension,
  setPinNumbers,
} from './model';

const draft: Draft = {
  name: 'EW7021',
  packages: [
    {
      name: 'SOIC-8',
      family: 'sop',
      pins: 8,
      dimensions: { e: { nom: 1.27, source: { page: 4 } }, D: { min: 4.8, max: 5.0 } },
    },
    { name: 'LCCC-20', family: 'other', pins: 20 },
  ],
  pins: [
    { name: 'VCC', kind: 'powerIn', numbers: { 'SOIC-8': ['8'] } },
    { name: 'OUT1', kind: 'output', numbers: { 'SOIC-8': ['1'], 'LCCC-20': ['2'] } },
    { name: 'GND', kind: 'powerIn', numbers: { 'SOIC-8': ['4'] } },
  ],
};

describe('part generator model', () => {
  it('lists the dimensions of a family', () => {
    expect(dimensionKeys('sop')).toContain('E1');
    expect(dimensionKeys('qfn', true)).toEqual(['e', 'D', 'E', 'b', 'L', 'A', 'A1', 'D2', 'E2']);
    expect(dimensionKeys('other')).toEqual([]);
  });

  it('reads dimension values', () => {
    expect(dimensionValue({ nom: 1.27 })).toBe(1.27);
    expect(dimensionValue({ min: 4.8, max: 5.0 })).toBeCloseTo(4.9);
    expect(dimensionValue(undefined)).toBeNull();
  });

  it('orders pad numbers naturally', () => {
    expect(['10', '2', 'B1', 'A10', 'A2'].sort(padOrder)).toEqual(['2', '10', 'A2', 'A10', 'B1']);
  });

  it('lists the pins of a package in pad order', () => {
    expect(pinRows(draft, 'SOIC-8').map((row) => row.pin.name)).toEqual(['OUT1', 'GND', 'VCC']);
    expect(pinRows(draft, 'LCCC-20').map((row) => row.pin.name)).toEqual(['OUT1']);
  });

  it('parses pad numbers with ranges', () => {
    expect(parseNumbers('6, 7')).toEqual(['6', '7']);
    expect(parseNumbers('9-11')).toEqual(['9', '10', '11']);
    expect(parseNumbers('A1 B2')).toEqual(['A1', 'B2']);
  });

  it('edits pins and pads', () => {
    const moved = setPinNumbers(draft, 0, 'SOIC-8', ['7', '8']);
    expect(moved.pins[0]?.numbers['SOIC-8']).toEqual(['7', '8']);
    const gone = setPinNumbers(draft, 1, 'LCCC-20', []);
    expect(gone.pins[1]?.numbers).toEqual({ 'SOIC-8': ['1'] });
    const added = addPin(draft, 'SOIC-8', 'NC', 'notConnected');
    expect(added.pins.at(-1)?.numbers).toEqual({ 'SOIC-8': ['2'] });
    expect(removePin(draft, 0).pins).toHaveLength(2);
    expect(missingPads(draft, draft.packages[0]!)).toEqual(['2', '3', '5', '6', '7']);
  });

  it('edits a dimension and drops its source', () => {
    const next = setDimension(draft, 'SOIC-8', 'e', 'nom', 0.65);
    expect(next.packages[0]?.dimensions?.['e']).toEqual({ nom: 0.65 });
    const cleared = setDimension(draft, 'SOIC-8', 'D', 'min', null);
    expect(cleared.packages[0]?.dimensions?.['D']).toEqual({ max: 5.0 });
  });

  it('starts with a package that has pins and a known family', () => {
    expect(firstPackage(draft)).toBe('SOIC-8');
    expect(firstPackage({ ...draft, packages: [] })).toBeNull();
  });
});
