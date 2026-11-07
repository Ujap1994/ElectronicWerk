import { describe, expect, it } from 'vitest';

import { de } from './de';
import { en } from './en';
import {
  createTranslator,
  DICTIONARIES,
  interpolate,
  LANGUAGES,
  listKeys,
  resolveLanguage,
  type MessageKey,
} from './index';

const placeholders = (text: string): string[] =>
  [...text.matchAll(/\{(\w+)\}/gu)].map((m) => m[1] ?? '').sort();

function valueAt(tree: object, key: string): string {
  let node: unknown = tree;
  for (const part of key.split('.')) {
    node = (node as Record<string, unknown>)[part];
  }
  return node as string;
}

describe('dictionaries', () => {
  const englishKeys = listKeys(en).sort();

  it.each(LANGUAGES)('%s has exactly the English keys', (language) => {
    expect(listKeys(DICTIONARIES[language]).sort()).toEqual(englishKeys);
  });

  it.each(LANGUAGES)('%s has no empty strings', (language) => {
    for (const key of englishKeys) {
      expect(valueAt(DICTIONARIES[language], key).trim(), key).not.toBe('');
    }
  });

  it('German uses the same placeholders as English', () => {
    for (const key of englishKeys) {
      expect(placeholders(valueAt(de, key)), key).toEqual(placeholders(valueAt(en, key)));
    }
  });
});

describe('resolveLanguage', () => {
  it('prefers an explicit setting', () => {
    expect(resolveLanguage('de', ['en-US'])).toBe('de');
    expect(resolveLanguage('en', ['de-DE'])).toBe('en');
  });

  it('follows the first supported system language', () => {
    expect(resolveLanguage('system', ['de-AT', 'en-US'])).toBe('de');
    expect(resolveLanguage('system', ['fr-FR', 'de_CH'])).toBe('de');
    expect(resolveLanguage('system', ['EN-gb'])).toBe('en');
  });

  it('falls back to English', () => {
    expect(resolveLanguage('system', [])).toBe('en');
    expect(resolveLanguage('system', ['fr-FR', 'ja'])).toBe('en');
  });
});

describe('translator', () => {
  it('translates and interpolates', () => {
    const t = createTranslator('de');
    expect(t('engine.running', { version: '1.2.3', threads: 4 })).toBe('Engine 1.2.3 · 4 Threads');
  });

  it('returns the key for unknown keys', () => {
    const t = createTranslator('en');
    expect(t('does.not.exist' as MessageKey)).toBe('does.not.exist');
  });

  it('leaves unknown placeholders visible', () => {
    expect(interpolate('a {x} b {y}', { x: 1 })).toBe('a 1 b {y}');
    expect(interpolate('plain')).toBe('plain');
  });
});
