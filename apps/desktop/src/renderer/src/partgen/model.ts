/**
 * Pure helpers of the part generator: which dimensions a package family
 * needs, the pins of one package and edits of a draft. Every edit returns
 * a new draft.
 */
import type { Dimension, Draft, Family, PackageDraft, PinDraft, PinType } from '@shared/datasheet';

/** The dimensions of a family in datasheet order (JEDEC symbols). */
export function dimensionKeys(family: Family, exposedPad = false): readonly string[] {
  const pad = exposedPad ? ['D2', 'E2'] : [];
  switch (family) {
    case 'sop':
      return ['e', 'D', 'E', 'E1', 'b', 'L', 'A', 'A1', 'c', ...pad];
    case 'qfp':
      return ['e', 'D', 'E', 'D1', 'E1', 'b', 'L', 'A', 'A1', 'c'];
    case 'qfn':
    case 'dfn':
      return ['e', 'D', 'E', 'b', 'L', 'A', 'A1', ...(exposedPad ? ['D2', 'E2'] : [])];
    case 'bga':
      return ['e', 'D', 'E', 'b', 'A', 'A1'];
    case 'sot23':
      return ['e', 'D', 'E', 'E1', 'b', 'L', 'A', 'A1', 'c'];
    case 'sot223':
    case 'sot89':
    case 'dpak':
      return ['e', 'D', 'E', 'E1', 'b', 'b2', 'L', 'A'];
    case 'dip':
      return ['e', 'D', 'E', 'E1', 'b', 'b1', 'L', 'A', 'A1'];
    case 'to220':
    case 'to92':
      return ['e', 'D', 'E', 'b', 'L', 'A'];
    case 'chip':
      return ['D', 'E', 'L', 'A'];
    case 'header':
    case 'other':
      return [];
  }
}

/** The few dimensions the beginner mode shows first. */
export function mainKeys(family: Family): readonly string[] {
  if (family === 'chip') {
    return ['D', 'E'];
  }
  return family === 'bga' ? ['e', 'D', 'E', 'b'] : ['e', 'D', 'E'];
}

/** The value of a dimension: nominal, else the middle of the range. */
export function dimensionValue(d: Dimension | undefined): number | null {
  if (d === undefined) {
    return null;
  }
  if (d.nom !== undefined) {
    return d.nom;
  }
  if (d.min !== undefined && d.max !== undefined) {
    return (d.min + d.max) / 2;
  }
  return d.min ?? d.max ?? null;
}

/** Millimetres for display (`1.27`, `0.5`). */
export function mm(value: number | null | undefined): string {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return '';
  }
  return String(Math.round(value * 1000) / 1000);
}

/** Natural order of pad numbers (`2` before `10`, `A2` before `B1`). */
export function padOrder(a: string, b: string): number {
  const split = (s: string): [string, number] => {
    const match = /^([A-Za-z]*)(\d*)/u.exec(s);
    return [match?.[1] ?? '', Number.parseInt(match?.[2] ?? '', 10) || 0];
  };
  const [la, na] = split(a);
  const [lb, nb] = split(b);
  if (la.length !== lb.length) {
    return la.length - lb.length;
  }
  if (la !== lb) {
    return la < lb ? -1 : 1;
  }
  return na === nb ? a.localeCompare(b) : na - nb;
}

/** A pin row of one package: the pin and its index in the draft. */
export interface PinRow {
  readonly index: number;
  readonly pin: PinDraft;
  readonly numbers: readonly string[];
}

/** The pins of a package, in pad order; pins without pads at the end. */
export function pinRows(draft: Draft, pkg: string): readonly PinRow[] {
  const rows = draft.pins.map((pin, index) => ({
    index,
    pin,
    numbers: pin.numbers[pkg] ?? [],
  }));
  return rows
    .filter((row) => row.numbers.length > 0 || Object.keys(row.pin.numbers).length === 0)
    .sort((a, b) => {
      const first = (row: PinRow): string => row.numbers[0] ?? '';
      if (first(a) === '' || first(b) === '') {
        return first(a) === '' ? 1 : -1;
      }
      return padOrder(first(a), first(b));
    });
}

/** Pad numbers from text: `6, 7`, `9-11`. */
export function parseNumbers(text: string): readonly string[] {
  const out: string[] = [];
  for (const part of text.split(/[,;\s]+/u)) {
    if (part === '') {
      continue;
    }
    const range = /^(\d+)\s*[-–]\s*(\d+)$/u.exec(part);
    if (range !== null) {
      const a = Number(range[1]);
      const b = Number(range[2]);
      if (b >= a && b - a <= 256) {
        for (let n = a; n <= b; n += 1) {
          out.push(String(n));
        }
        continue;
      }
    }
    out.push(part);
  }
  return out;
}

/** Changes fields of one pin. */
export function setPin(
  draft: Draft,
  index: number,
  patch: Partial<Pick<PinDraft, 'name' | 'kind' | 'function'>>,
): Draft {
  return {
    ...draft,
    pins: draft.pins.map((pin, i) => (i === index ? { ...pin, ...patch } : pin)),
  };
}

/** Sets the pad numbers of a pin in a package (empty: not in it). */
export function setPinNumbers(
  draft: Draft,
  index: number,
  pkg: string,
  numbers: readonly string[],
): Draft {
  return {
    ...draft,
    pins: draft.pins.map((pin, i) => {
      if (i !== index) {
        return pin;
      }
      const rest = Object.fromEntries(Object.entries(pin.numbers).filter(([key]) => key !== pkg));
      return {
        ...pin,
        numbers: numbers.length === 0 ? rest : { ...rest, [pkg]: numbers },
      };
    }),
  };
}

/** Adds a pin for a package with the next free pad number. */
export function addPin(draft: Draft, pkg: string, name: string, kind: PinType): Draft {
  const used = new Set(draft.pins.flatMap((pin) => pin.numbers[pkg] ?? []));
  let next = 1;
  while (used.has(String(next))) {
    next += 1;
  }
  return {
    ...draft,
    pins: [...draft.pins, { name, kind, numbers: { [pkg]: [String(next)] } }],
  };
}

/** Removes a pin. */
export function removePin(draft: Draft, index: number): Draft {
  return { ...draft, pins: draft.pins.filter((_, i) => i !== index) };
}

/** Changes a package. */
export function setPackage(draft: Draft, name: string, patch: Partial<PackageDraft>): Draft {
  return {
    ...draft,
    packages: draft.packages.map((p) => (p.name === name ? { ...p, ...patch } : p)),
  };
}

/** Sets one value of a dimension (typed in: the source is dropped). */
export function setDimension(
  draft: Draft,
  pkg: string,
  key: string,
  slot: 'min' | 'nom' | 'max',
  value: number | null,
): Draft {
  return {
    ...draft,
    packages: draft.packages.map((p) => {
      if (p.name !== pkg) {
        return p;
      }
      const current: Dimension = p.dimensions?.[key] ?? {};
      const next: Record<string, number> = {};
      for (const k of ['min', 'nom', 'max'] as const) {
        const v = current[k];
        if (k !== slot && v !== undefined) {
          next[k] = v;
        }
      }
      if (value !== null) {
        next[slot] = value;
      }
      return { ...p, dimensions: { ...(p.dimensions ?? {}), [key]: next } };
    }),
  };
}

/** Pads of the package that no pin has (`1` … pin count). */
export function missingPads(draft: Draft, pkg: PackageDraft): readonly string[] {
  if (pkg.pins <= 0 || pkg.family === 'bga') {
    return [];
  }
  const used = new Set(draft.pins.flatMap((pin) => pin.numbers[pkg.name] ?? []));
  const out: string[] = [];
  for (let n = 1; n <= pkg.pins; n += 1) {
    if (!used.has(String(n))) {
      out.push(String(n));
    }
  }
  return out;
}

/** The package to start with: the first with pins and a known family. */
export function firstPackage(draft: Draft): string | null {
  const usable = draft.packages.find(
    (p) => p.family !== 'other' && draft.pins.some((pin) => (pin.numbers[p.name]?.length ?? 0) > 0),
  );
  return usable?.name ?? draft.packages[0]?.name ?? null;
}
