import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

import { DICTIONARIES, LANGUAGES, listKeys } from '../src/shared/i18n';
import { IMPORT_CODES, VALIDATION_CODES } from '../src/shared/project';

/** Every code string the Rust validator can report. */
function rustCodes(): Set<string> {
  const source = readFileSync(
    resolve(import.meta.dirname, '../../../crates/ew-format/src/validate.rs'),
    'utf8',
  );
  const code = source.split('#[cfg(test)]')[0] ?? '';
  return new Set([...code.matchAll(/"([a-z]+(?:-[a-z]+)+)"/gu)].map((match) => match[1] ?? ''));
}

describe('validation codes', () => {
  it('covers every code the Rust validator can report', () => {
    const found = rustCodes();
    expect(found.size).toBeGreaterThan(25);
    expect([...found].filter((code) => !VALIDATION_CODES.has(code))).toEqual([]);
  });

  it.each(LANGUAGES)('has a %s translation for every code', (language) => {
    const keys = new Set(listKeys(DICTIONARIES[language]));
    const missing = [...VALIDATION_CODES].filter((code) => !keys.has(`validation.codes.${code}`));
    expect(missing).toEqual([]);
  });
});

/** Every finding code the KiCad importer can report. */
function importCodes(): Set<string> {
  const dirs = ['../../../crates/ew-kicad/src', '../../../crates/ew-core/src'];
  const codes = new Set<string>();
  for (const dir of dirs) {
    const root = resolve(import.meta.dirname, dir);
    for (const file of readdirSync(root).filter((name) => name.endsWith('.rs'))) {
      const source = readFileSync(join(root, file), 'utf8').split('#[cfg(test)]')[0] ?? '';
      for (const match of source.matchAll(/\.(?:warn|info|error)\(\s*"([a-z]+(?:-[a-z]+)+)"/gu)) {
        codes.add(match[1] ?? '');
      }
    }
  }
  return codes;
}

describe('import codes', () => {
  it('covers every code the importer can report, in every language', () => {
    const codes = importCodes();
    expect(codes.size).toBeGreaterThan(10);
    expect([...codes].filter((code) => !IMPORT_CODES.has(code))).toEqual([]);
    for (const language of LANGUAGES) {
      const keys = new Set(listKeys(DICTIONARIES[language]));
      for (const code of IMPORT_CODES) {
        expect(keys.has(`import.codes.${code}`), `${language}: ${code}`).toBe(true);
      }
    }
  });
});
