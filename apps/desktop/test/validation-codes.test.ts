import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

import { DICTIONARIES, LANGUAGES, listKeys } from '../src/shared/i18n';
import { VALIDATION_CODES } from '../src/shared/project';

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
