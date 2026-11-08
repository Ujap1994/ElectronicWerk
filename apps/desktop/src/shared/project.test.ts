import { describe, expect, it } from 'vitest';

import {
  baseName,
  checkName,
  dirName,
  formatLength,
  formatSize,
  isEngineEvent,
  isVariableName,
  joinPath,
  parseLength,
  sanitizeFolderName,
  toUnits,
} from './project';

describe('lengths', () => {
  it('formats in every unit without trailing zeros', () => {
    expect(formatLength(1_600_000, 'mm')).toBe('1.6 mm');
    expect(formatLength(100_000_000, 'mm')).toBe('100 mm');
    expect(formatLength(254_000, 'mil')).toBe('10 mil');
    expect(formatLength(25_400_000, 'inch')).toBe('1 in');
    expect(formatLength(1_500_000, 'mm', 'de')).toBe('1,5 mm');
    expect(formatSize(100_000_000, 80_000_000, 'mm')).toBe('100 × 80 mm');
  });

  it('parses user input', () => {
    expect(parseLength('1.6', 'mm')).toBe(1_600_000);
    expect(parseLength(' 1,6 ', 'mm')).toBe(1_600_000);
    expect(parseLength('10', 'mil')).toBe(254_000);
    expect(parseLength('abc', 'mm')).toBeNull();
    expect(parseLength('1.2.3', 'mm')).toBeNull();
    expect(parseLength('', 'mm')).toBeNull();
    expect(toUnits(1_600_000, 'mm')).toBe('1.6');
    expect(toUnits(68_580_000, 'inch')).toBe('2.7');
  });
});

describe('names', () => {
  it('checks names like the engine', () => {
    expect(checkName('Blinky')).toBeNull();
    expect(checkName('  ')).toBe('empty');
    expect(checkName('x'.repeat(101))).toBe('tooLong');
    expect(checkName('a\tb')).toBe('invalid');
  });

  it('sanitizes folder names like the engine', () => {
    expect(sanitizeFolderName('My Board')).toBe('My Board');
    expect(sanitizeFolderName('a/b:c')).toBe('a_b_c');
    expect(sanitizeFolderName('  ..  ')).toBe('project');
    expect(sanitizeFolderName('con')).toBe('_con');
    expect(sanitizeFolderName('LPT1.txt')).toBe('_LPT1.txt');
  });

  it('validates variable names', () => {
    expect(isVariableName('AUTHOR')).toBe(true);
    expect(isVariableName('_x1')).toBe(true);
    expect(isVariableName('1A')).toBe(false);
    expect(isVariableName('A-B')).toBe(false);
  });
});

describe('paths', () => {
  it('joins and splits paths per platform', () => {
    expect(joinPath('/home/a/', 'P', 'linux')).toBe('/home/a/P');
    expect(joinPath('C:\\Users\\a', 'P', 'win32')).toBe('C:\\Users\\a\\P');
    expect(baseName('C:\\x\\Board.ewproj')).toBe('Board.ewproj');
    expect(baseName('/a/b/')).toBe('b');
    expect(dirName('/a/b/c.ewproj')).toBe('/a/b');
    expect(dirName('C:\\a\\b')).toBe('C:\\a');
    expect(dirName('/x')).toBe('/');
  });
});

describe('isEngineEvent', () => {
  it('accepts only known event types', () => {
    expect(isEngineEvent({ type: 'project', action: 'opened', path: null })).toBe(true);
    expect(isEngineEvent({ type: 'other' })).toBe(false);
    expect(isEngineEvent(null)).toBe(false);
  });
});
