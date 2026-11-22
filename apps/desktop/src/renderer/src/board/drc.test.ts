import { describe, expect, it } from 'vitest';

import { createTranslator } from '@shared/i18n';

import { translateTerm } from './drc-terms';

describe('design rules check messages', () => {
  it('names copper items and rule sources in the UI language', () => {
    const de = createTranslator('de');
    expect(translateTerm(de, 'pad J1-1 (VCC)')).toBe('Pad J1-1 (VCC)');
    expect(translateTerm(de, 'track (GND)')).toBe('Leiterbahn (GND)');
    expect(translateTerm(de, 'via')).toBe('Via');
    expect(translateTerm(de, 'zone Shield (GND)')).toBe('Zone Shield (GND)');
    expect(translateTerm(de, 'net class Power')).toBe('Netzklasse Power');
    expect(translateTerm(de, 'board constraints')).toBe('Mindestwerte der Platine');
    // Anything else stays as it is.
    expect(translateTerm(de, '0.25 mm')).toBe('0.25 mm');
    const en = createTranslator('en');
    expect(translateTerm(en, 'pad R1-2 (/DIS)')).toBe('pad R1-2 (/DIS)');
  });
});
