import { describe, expect, it } from 'vitest';

import { centerOf, nextName, powerStyle, route } from './editor';
import { arcPath } from './SymbolShapes';
import { escapeXml, printDocument } from './svg';

describe('schematic editor helpers', () => {
  it('counts label names up', () => {
    expect(nextName('D0')).toBe('D1');
    expect(nextName('ADDR09')).toBe('ADDR10');
    expect(nextName('SDA')).toBe('SDA');
  });

  it('picks the power port symbol from the net name', () => {
    expect(powerStyle('GND')).toBe('ground');
    expect(powerStyle('agnd')).toBe('ground');
    expect(powerStyle('+5V')).toBe('bar');
  });

  it('routes wires with one corner', () => {
    expect(route([0, 0], [10, 5], 'hv')).toEqual([
      [0, 0],
      [10, 0],
      [10, 5],
    ]);
    expect(route([0, 0], [10, 5], 'vh')).toEqual([
      [0, 0],
      [0, 5],
      [10, 5],
    ]);
    expect(route([0, 0], [10, 0], 'vh')).toEqual([
      [0, 0],
      [10, 0],
    ]);
  });

  it('finds the centre of copied objects', () => {
    expect(centerOf([{ at: [0, 0] }, { start: [10, 4], end: [20, 8] }, { other: 1 }])).toEqual([
      10, 4,
    ]);
    expect(centerOf([])).toEqual([0, 0]);
  });

  it('draws arcs through three points', () => {
    // A quarter circle from (1, 0) through 45° to (0, 1): counterclockwise.
    const s = Math.SQRT1_2;
    const [move, arc] = arcPath([1, 0], [s, s], [0, 1]).split('A');
    expect(move).toBe('M1 0');
    const [rx, ry, rotation, large, sweep, x, y] = (arc ?? '').split(' ').map(Number);
    expect(rx).toBeCloseTo(1);
    expect(ry).toBeCloseTo(1);
    expect([rotation, large, sweep, x, y]).toEqual([0, 0, 1, 0, 1]);
    expect(arcPath([0, 0], [1, 1], [2, 2])).toBe('M0 0L2 2');
  });

  it('escapes text for SVG and makes one page per sheet', () => {
    expect(escapeXml('<R&D "x">')).toBe('&lt;R&amp;D &quot;x&quot;&gt;');
    const html = printDocument(
      [
        { svg: '<svg a="1"></svg>', width: 297, height: 210 },
        { svg: '<svg b="2"></svg>', width: 420, height: 297 },
      ],
      'Demo',
    );
    expect(html).toContain('@page p0 { size: 297mm 210mm; margin: 0; }');
    expect(html).toContain('@page p1 { size: 420mm 297mm; margin: 0; }');
    expect(html).toContain('<svg class="sheet" b="2">');
  });
});
