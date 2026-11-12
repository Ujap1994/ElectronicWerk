import { describe, expect, it } from 'vitest';

import { createTranslator, listKeys } from '@shared/i18n';
import { en } from '@shared/i18n/en';

import { findingText } from './labels';
import { padPath } from './FootprintSvg';
import { parsePinKind, parsePinList, placePastedPins, selectedIndices } from './pinlist';
import { shapeKey, shapeOf } from './shapes';
import { bodyOf, decodeItemId, fromMm, itemId, toMm, type Pad, type SymbolDef } from './types';

describe('editor scene ids', () => {
  it('round-trips kind and index', () => {
    for (const ref of [
      { kind: 'pin', index: 0 },
      { kind: 'pad', index: 41 },
      { kind: 'graphic', index: 65_536 },
    ] as const) {
      expect(decodeItemId(itemId(ref))).toEqual(ref);
    }
    // The engine writes tag << 64 | index as a UUID.
    expect(itemId({ kind: 'text', index: 2 })).toBe('00000000-0000-0003-0000-000000000002');
    expect(decodeItemId('not-an-id')).toBeNull();
    expect(decodeItemId('00000000-0000-0009-0000-000000000002')).toBeNull();
  });

  it('converts millimetres', () => {
    expect(fromMm(1.27)).toBe(1_270_000);
    expect(toMm(254_000)).toBe(0.254);
  });
});

describe('bodyOf', () => {
  const def: SymbolDef = {
    id: 'x',
    name: 'U',
    prefix: 'U',
    units: [
      {
        pins: [{ number: '1', at: [0, 0], length: 1 }],
        alternate: { pins: [{ number: '1', at: [5, 0], length: 1 }] },
      },
    ],
  };

  it('picks the normal or the alternate body', () => {
    expect(bodyOf(def, { unit: 1, alternate: false }).pins?.[0]?.at).toEqual([0, 0]);
    expect(bodyOf(def, { unit: 1, alternate: true }).pins?.[0]?.at).toEqual([5, 0]);
    // A unit that is gone falls back to the first one.
    expect(bodyOf(def, { unit: 3, alternate: false }).pins).toHaveLength(1);
  });
});

describe('pin lists', () => {
  it('knows names and KiCad letters of the electrical types', () => {
    expect(parsePinKind('W')).toBe('powerIn');
    expect(parsePinKind('w')).toBe('powerOut');
    expect(parsePinKind('Open collector')).toBe('openCollector');
    expect(parsePinKind('I/O')).toBe('bidirectional');
    expect(parsePinKind('tristate')).toBe('triState');
    expect(parsePinKind('banana')).toBeNull();
  });

  it('parses tab, semicolon and comma separated lines', () => {
    const parsed = parsePinList(
      'Pin\tName\tType\n1\tVCC\tW\n2;GND;power in\n\n3,SDA,banana\n\tSCL\tB\nRESET',
      'passive',
    );
    expect(parsed.pins).toEqual([
      { number: '1', name: 'VCC', kind: 'powerIn' },
      { number: '2', name: 'GND', kind: 'powerIn' },
      { number: '3', name: 'SDA', kind: 'passive' },
      { number: '', name: 'SCL', kind: 'bidirectional' },
      { number: '', name: 'RESET', kind: 'passive' },
    ]);
    expect(parsed.unknown).toEqual([{ line: 5, kind: 'banana' }]);
  });

  it('places pasted pins in a column below the existing ones', () => {
    const pins = placePastedPins(
      [
        { number: '', name: 'A', kind: 'input' },
        { number: '', name: 'B', kind: 'input' },
      ],
      [{ number: '1', at: [-5_080_000, 2_540_000], length: 2_540_000 }],
      2_540_000,
    );
    expect(pins.map((pin) => pin.at)).toEqual([
      [-5_080_000, 0],
      [-5_080_000, -2_540_000],
    ]);
    expect(pins[0]?.direction).toBe('right');
    expect(placePastedPins([{ number: '7', name: 'X', kind: 'free' }], [], 1)[0]?.at).toEqual([
      -7_620_000, 0,
    ]);
  });

  it('sorts the selected rows of a kind', () => {
    expect(
      selectedIndices(
        [
          { kind: 'pin', index: 4 },
          { kind: 'graphic', index: 0 },
          { kind: 'pin', index: 1 },
        ],
        'pin',
      ),
    ).toEqual([1, 4]);
  });
});

describe('pad shapes', () => {
  const pad = (shape: Pad['shape']): Pad => ({
    id: 'p',
    number: '1',
    shape,
    at: [0, 0],
    size: [2_000, 1_000],
    layers: ['cu.top'],
  });

  it('names and makes shapes', () => {
    expect(shapeKey('oval')).toBe('oval');
    expect(shapeKey({ roundRect: { ratio: 250 } })).toBe('roundRect');
    expect(shapeKey({ custom: { points: [] } })).toBe('custom');
    expect(shapeOf('roundRect', [1, 1])).toEqual({ roundRect: { ratio: 250 } });
    expect(shapeOf('custom', [2, 4])).toEqual({
      custom: {
        points: [
          [-1, -2],
          [1, -2],
          [1, 2],
          [-1, 2],
        ],
      },
    });
  });

  it('draws outlines around the pad centre', () => {
    expect(padPath(pad('rect'))).toBe('M-1000 -500H1000V500H-1000Z');
    expect(padPath(pad('circle'))).toContain('A1000 1000');
    // An oval is a rounded rectangle with half the short side as radius.
    expect(padPath(pad('oval'))).toContain('A500 500');
    expect(padPath(pad({ roundRect: { ratio: 250 } }))).toContain('A250 250');
  });
});

describe('library editor strings', () => {
  it('translates every check code with its parameters', () => {
    const t = createTranslator('de');
    expect(
      findingText(t, {
        code: 'pin-without-pad',
        severity: 'error',
        message: 'x',
        args: { pin: '3' },
      }),
    ).toBe('Pin 3 hat kein Pad mit derselben Nummer.');
    // Unknown codes keep the engine's English text.
    expect(
      findingText(t, { code: 'future-check', severity: 'info', message: 'raw', args: {} }),
    ).toBe('raw');
  });

  it('has a section for the editors', () => {
    expect(listKeys(en.libedit).length).toBeGreaterThan(200);
  });
});
