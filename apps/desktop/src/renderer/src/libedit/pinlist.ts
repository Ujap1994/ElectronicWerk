/**
 * Pasted pin lists (from datasheet tables or spreadsheets) and other pure
 * helpers of the pin and pad tables.
 */
import { PIN_KINDS, type Pin, type PinKind, type Point } from './types';

/** Names and KiCad letters of the electrical types, lower case. */
const KIND_NAMES: Readonly<Record<string, PinKind>> = {
  i: 'input',
  in: 'input',
  input: 'input',
  o: 'output',
  out: 'output',
  output: 'output',
  b: 'bidirectional',
  bi: 'bidirectional',
  bidi: 'bidirectional',
  io: 'bidirectional',
  'i/o': 'bidirectional',
  bidirectional: 'bidirectional',
  t: 'triState',
  tri: 'triState',
  tristate: 'triState',
  'tri-state': 'triState',
  tri_state: 'triState',
  p: 'passive',
  passive: 'passive',
  f: 'free',
  free: 'free',
  nic: 'free',
  u: 'unspecified',
  unspecified: 'unspecified',
  w: 'powerIn',
  pwr: 'powerIn',
  power: 'powerIn',
  'power in': 'powerIn',
  power_in: 'powerIn',
  powerin: 'powerIn',
  supply: 'powerIn',
  'power out': 'powerOut',
  power_out: 'powerOut',
  powerout: 'powerOut',
  c: 'openCollector',
  oc: 'openCollector',
  'open collector': 'openCollector',
  open_collector: 'openCollector',
  opencollector: 'openCollector',
  e: 'openEmitter',
  oe: 'openEmitter',
  'open emitter': 'openEmitter',
  open_emitter: 'openEmitter',
  openemitter: 'openEmitter',
  n: 'noConnect',
  nc: 'noConnect',
  'no connect': 'noConnect',
  no_connect: 'noConnect',
  noconnect: 'noConnect',
  'not connected': 'noConnect',
};

/** The electrical type of a typed name or KiCad letter (`W` is power in). */
export function parsePinKind(text: string): PinKind | null {
  const trimmed = text.trim();
  // KiCad uses `W` (power in) and `w` (power out).
  if (trimmed === 'w') {
    return 'powerOut';
  }
  const key = trimmed.toLowerCase();
  const found = KIND_NAMES[key];
  if (found !== undefined) {
    return found;
  }
  return PIN_KINDS.find((kind) => kind.toLowerCase() === key) ?? null;
}

export interface ParsedPinList {
  readonly pins: readonly { number: string; name: string; kind: PinKind }[];
  /** Lines (1-based) whose type was not understood, with the text. */
  readonly unknown: readonly { line: number; kind: string }[];
}

/**
 * Parses one pin per line: number, name and type separated by tabs,
 * semicolons or commas. A line with one field is a name; a header line
 * (`Pin`, `Number`, `Name` …) is skipped.
 */
export function parsePinList(text: string, fallback: PinKind): ParsedPinList {
  const pins: { number: string; name: string; kind: PinKind }[] = [];
  const unknown: { line: number; kind: string }[] = [];
  text.split(/\r?\n/u).forEach((raw, index) => {
    // Only the line end is trimmed: a leading tab is an empty number.
    const line = raw.trimEnd();
    if (line.trim() === '') {
      return;
    }
    const separator = line.includes('\t') ? '\t' : line.includes(';') ? ';' : ',';
    const fields = line.split(separator).map((field) => field.trim());
    const first = (fields[0] ?? '').toLowerCase();
    if (index === 0 && ['pin', 'no', 'no.', 'number', 'nr', 'nr.', '#'].includes(first)) {
      return;
    }
    if (fields.length === 1) {
      pins.push({ number: '', name: fields[0] ?? '', kind: fallback });
      return;
    }
    const kindText = fields[2] ?? '';
    let kind = fallback;
    if (kindText !== '') {
      const parsed = parsePinKind(kindText);
      if (parsed === null) {
        unknown.push({ line: index + 1, kind: kindText });
      } else {
        kind = parsed;
      }
    }
    pins.push({ number: fields[0] ?? '', name: fields[1] ?? '', kind });
  });
  return { pins, unknown };
}

/** Pin spacing of pasted pins (2.54 mm). */
const PITCH = 2_540_000;

/**
 * Places pasted pins in a column left of the body's pins, pointing right,
 * below the lowest existing pin.
 */
export function placePastedPins(
  parsed: ParsedPinList['pins'],
  existing: readonly Pin[],
  length: number,
): Pin[] {
  const xs = existing.map((pin) => pin.at[0]);
  const ys = existing.map((pin) => pin.at[1]);
  const x = xs.length === 0 ? -7_620_000 : Math.min(...xs);
  const top = ys.length === 0 ? 0 : Math.min(...ys) - PITCH;
  return parsed.map((pin, index) => ({
    number: pin.number,
    name: pin.name,
    kind: pin.kind,
    at: [x, top - index * PITCH] as Point,
    direction: 'right',
    length,
  }));
}

/** Indices of the selected rows in the order shown. */
export function selectedIndices(
  selection: readonly { kind: string; index: number }[],
  kind: string,
): number[] {
  return selection
    .filter((ref) => ref.kind === kind)
    .map((ref) => ref.index)
    .sort((a, b) => a - b);
}
