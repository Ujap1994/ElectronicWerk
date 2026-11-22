import { describe, expect, it } from 'vitest';

import { formatQuantity, parseQuantity } from './quantity';

describe('quantities', () => {
  it('reads numbers as engineers type them', () => {
    expect(parseQuantity('4.7k')).toBe(4700);
    expect(parseQuantity('4k7')).toBeCloseTo(4700, 9);
    expect(parseQuantity('2M2')).toBeCloseTo(2.2e6, 6);
    expect(parseQuantity('4R7')).toBeCloseTo(4.7, 12);
    expect(parseQuantity('10 k')).toBe(10_000);
    expect(parseQuantity('3,3')).toBe(3.3);
    expect(parseQuantity('3.3 V')).toBe(3.3);
    expect(parseQuantity('50u')).toBeCloseTo(50e-6, 15);
    expect(parseQuantity('50µA')).toBeCloseTo(50e-6, 15);
    expect(parseQuantity('100Ω')).toBe(100);
    expect(parseQuantity('1.2e-3')).toBeCloseTo(0.0012, 12);
    expect(parseQuantity('25 °C', ['°C'])).toBe(25);
    expect(parseQuantity('')).toBeNull();
    expect(parseQuantity('abc')).toBeNull();
    expect(parseQuantity('1x')).toBeNull();
  });

  it('writes values with a prefix', () => {
    expect(formatQuantity(4700, 'Ω')).toBe('4.7 kΩ');
    expect(formatQuantity(3.2432432, 'V')).toBe('3.243 V');
    expect(formatQuantity(82.3e-6, 'A')).toBe('82.3 µA');
    expect(formatQuantity(1e6, 'Ω')).toBe('1 MΩ');
    expect(formatQuantity(0, 'W')).toBe('0 W');
    expect(formatQuantity(0.0493, 'Ω', 'de')).toBe('49,3 mΩ');
    expect(formatQuantity(999.96, 'Ω')).toBe('1 kΩ');
  });
});
