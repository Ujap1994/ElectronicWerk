import { describe, expect, it } from 'vitest';

import {
  boardLayers,
  isGerberFile,
  joinOutput,
  moved,
  newOutput,
  option,
  splitPath,
  withOption,
} from './model';

describe('output options', () => {
  it('fall back to the defaults of their kind', () => {
    const spec = newOutput('gerber', 'Gerber', 'a');
    expect(option(spec, 'x2')).toBe(true);
    expect(option(spec, 'folder')).toBe('gerber');
    const changed = withOption(spec, 'x2', false);
    expect(option(changed, 'x2')).toBe(false);
    expect(option(spec, 'x2')).toBe(true);
    expect(option(spec, 'unknown', 3)).toBe(3);
    // Outputs read from a file may have no options at all.
    const bare = { id: 'b', name: 'B', kind: 'drill', enabled: true } as unknown as typeof spec;
    expect(option(bare, 'merge')).toBe(false);
    expect(option(withOption(bare, 'merge', true), 'merge')).toBe(true);
  });

  it('move entries within bounds', () => {
    expect(moved(['a', 'b', 'c'], 0, 1)).toEqual(['b', 'a', 'c']);
    expect(moved(['a', 'b', 'c'], 2, -2)).toEqual(['c', 'a', 'b']);
    expect(moved(['a', 'b'], 1, 1)).toEqual(['a', 'b']);
  });

  it('list the board layers of a stack-up', () => {
    expect(boardLayers(4).slice(0, 4)).toEqual(['cu.top', 'cu.in1', 'cu.in2', 'cu.bottom']);
    expect(boardLayers(1)[0]).toBe('cu.top');
    expect(boardLayers(1)).not.toContain('cu.bottom');
    expect(boardLayers(2)).toContain('outline');
  });

  it('recognise Gerber files and join paths', () => {
    expect(isGerberFile('gerber/demo-Copper_Top.gtl')).toBe(true);
    expect(isGerberFile('gerber/demo-Copper_In1.g1')).toBe(true);
    expect(isGerberFile('gerber/demo-PTH.drl')).toBe(true);
    expect(isGerberFile('assembly/demo-bom.csv')).toBe(false);
    expect(splitPath('gerber/a.gtl')).toEqual({ folder: 'gerber', file: 'a.gtl' });
    expect(splitPath('a.zip')).toEqual({ folder: '', file: 'a.zip' });
    expect(joinOutput('C:\\p\\out', 'gerber/a.gtl', '\\')).toBe('C:\\p\\out\\gerber\\a.gtl');
    expect(joinOutput('/p/out/', 'a.zip', '/')).toBe('/p/out/a.zip');
  });
});
