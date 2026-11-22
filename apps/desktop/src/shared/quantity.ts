/**
 * Numbers with SI prefixes as engineers type them: `4.7k`, `4k7`, `10 k`,
 * `50u`, `50µ`, `3,3` (comma as decimal separator), `1.2e-3`. Units after
 * the number (`4.7kΩ`, `3.3 V`) are ignored.
 */

const PREFIXES: Readonly<Record<string, number>> = {
  p: 1e-12,
  n: 1e-9,
  u: 1e-6,
  µ: 1e-6,
  μ: 1e-6,
  m: 1e-3,
  k: 1e3,
  K: 1e3,
  M: 1e6,
  G: 1e9,
};

/** The value of a typed quantity, or `null`. */
export function parseQuantity(text: string, units: readonly string[] = []): number | null {
  let rest = text.trim().replace(/\s+/gu, '');
  for (const unit of [...units, 'Ω', 'ohm', 'Ohm', 'R', 'V', 'A', 'W', 's']) {
    if (rest.length > unit.length && rest.endsWith(unit)) {
      // `4R7` is 4.7 Ω; a trailing R alone is the unit.
      rest = rest.slice(0, -unit.length);
      break;
    }
  }
  rest = rest.replace(',', '.');
  // 4k7, 2M2, 4R7: the prefix is the decimal point.
  const inner = /^(\d+)([pnuµμmkKMGR])(\d+)$/u.exec(rest);
  if (inner !== null) {
    const [, whole = '', prefix = '', fraction = ''] = inner;
    const factor = prefix === 'R' ? 1 : (PREFIXES[prefix] ?? 1);
    return Number(`${whole}.${fraction}`) * factor;
  }
  const plain = /^([-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[-+]?\d+)?)([pnuµμmkKMG]?)$/iu.exec(rest);
  if (plain === null) {
    return null;
  }
  const [, number = '', prefix = ''] = plain;
  // `e` belongs to the number; `E` alone is no prefix.
  const factor = prefix === '' ? 1 : PREFIXES[prefix];
  if (factor === undefined) {
    return null;
  }
  const value = Number(number) * factor;
  return Number.isFinite(value) ? value : null;
}

const STEPS: readonly (readonly [number, string])[] = [
  [1e9, 'G'],
  [1e6, 'M'],
  [1e3, 'k'],
  [1, ''],
  [1e-3, 'm'],
  [1e-6, 'µ'],
  [1e-9, 'n'],
  [1e-12, 'p'],
];

/** `4.7 kΩ`, `82.3 µA`, `3.243 V`: an SI prefix and `digits` significant digits. */
export function formatQuantity(value: number, unit: string, locale = 'en', digits = 4): string {
  if (!Number.isFinite(value)) {
    return '–';
  }
  if (value === 0) {
    return `0 ${unit}`;
  }
  const size = Math.abs(value);
  const [factor, prefix] = STEPS.find(([f]) => size >= f * 0.9995) ?? [1e-12, 'p'];
  const scaled = value / factor;
  const text = new Intl.NumberFormat(locale, {
    maximumSignificantDigits: digits,
    useGrouping: false,
  }).format(scaled);
  return `${text} ${prefix}${unit}`;
}
