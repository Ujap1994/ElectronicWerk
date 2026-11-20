/**
 * The part generator's data as the engine sends it (`datasheet.*` and
 * `part.*`, crate `ew-datasheet`): the draft of a part read from a
 * datasheet and the generated items.
 */

/** Electrical type of a pin. */
export type PinType =
  | 'input'
  | 'output'
  | 'bidirectional'
  | 'triState'
  | 'passive'
  | 'powerIn'
  | 'powerOut'
  | 'openCollector'
  | 'notConnected'
  | 'unspecified';

export const PIN_TYPES: readonly PinType[] = [
  'input',
  'output',
  'bidirectional',
  'triState',
  'passive',
  'powerIn',
  'powerOut',
  'openCollector',
  'notConnected',
  'unspecified',
];

/** Package family: decides the footprint wizard and the 3D model. */
export type Family =
  | 'sop'
  | 'qfp'
  | 'qfn'
  | 'dfn'
  | 'bga'
  | 'sot23'
  | 'sot223'
  | 'sot89'
  | 'dpak'
  | 'dip'
  | 'to220'
  | 'to92'
  | 'chip'
  | 'header'
  | 'other';

export const FAMILIES: readonly Family[] = [
  'sop',
  'qfp',
  'qfn',
  'dfn',
  'bga',
  'sot23',
  'sot223',
  'sot89',
  'dpak',
  'dip',
  'to220',
  'to92',
  'chip',
  'other',
];

/** A rectangle on a page: points from the top left, y down. */
export type Box = readonly [number, number, number, number];

/** Where a value was read. */
export interface Source {
  readonly page: number;
  readonly bbox?: Box;
}

/** A dimension in millimetres. */
export interface Dimension {
  readonly min?: number;
  readonly nom?: number;
  readonly max?: number;
  readonly source?: Source;
}

export interface PackageDraft {
  readonly name: string;
  readonly family: Family;
  readonly pins: number;
  readonly exposedPad?: boolean;
  readonly dimensions?: Readonly<Record<string, Dimension>>;
  readonly source?: Source;
  readonly drawing?: Source;
}

export interface PinDraft {
  readonly name: string;
  /** Pad numbers per package name. */
  readonly numbers: Readonly<Record<string, readonly string[]>>;
  readonly kind?: PinType;
  readonly function?: string;
  readonly source?: Source;
}

/** A note for the user: a code to translate, the English text, values. */
export interface Note {
  readonly code: string;
  readonly message: string;
  readonly args?: Readonly<Record<string, string>>;
}

export interface Draft {
  readonly name: string;
  readonly manufacturer?: string;
  readonly mpn?: string;
  readonly description?: string;
  readonly datasheet?: string;
  readonly packages: readonly PackageDraft[];
  readonly pins: readonly PinDraft[];
  readonly source?: Source;
  readonly notes?: readonly Note[];
}

export interface DatasheetPage {
  readonly number: number;
  readonly width: number;
  readonly height: number;
  readonly scanned?: boolean;
}

export interface DatasheetInfo {
  readonly path: string;
  readonly title: string;
  readonly author: string;
  readonly subject: string;
  readonly encrypted: boolean;
  readonly bookmarks: readonly {
    readonly title: string;
    readonly level: number;
    readonly page?: number;
  }[];
  readonly pages: readonly DatasheetPage[];
}

export type Density = 'least' | 'nominal' | 'most';

export interface PartOptions {
  readonly density?: Density;
  readonly name?: string;
  readonly footprint?: string;
  readonly splitAbove?: number;
  readonly unitSize?: number;
}

export interface PartFinding {
  readonly item: 'symbol' | 'footprint' | 'component';
  readonly finding: {
    readonly code: string;
    readonly severity: 'error' | 'warning' | 'info';
    readonly message: string;
    readonly args?: Readonly<Record<string, string>>;
  };
}

/** Reply of `part.preview`. */
export interface PartPreview {
  readonly symbol: { readonly name: string; readonly units: readonly unknown[] } & Readonly<
    Record<string, unknown>
  >;
  readonly footprint: { readonly name: string } & Readonly<Record<string, unknown>>;
  readonly component: { readonly name: string } & Readonly<Record<string, unknown>>;
  readonly wizard: Readonly<Record<string, unknown>>;
  readonly warnings: readonly Note[];
  readonly findings: readonly PartFinding[];
  readonly step?: string;
}

export interface DuplicateItem {
  readonly source: 'project' | 'base';
  readonly library: string;
  readonly kind: 'component' | 'symbol';
  readonly name: string;
  readonly footprint?: string | null;
  readonly doc?: string;
  readonly id?: number;
}

/** Reply of `part.duplicates`. */
export interface Duplicates {
  readonly matches: readonly DuplicateItem[];
  readonly related: readonly DuplicateItem[];
}

/** Reply of `part.generate`. */
export interface Generated {
  readonly symbol: string;
  readonly footprint: string;
  readonly component: string;
  readonly model: string | null;
  readonly warnings: readonly Note[];
}
