/**
 * Library items as the engine stores them (crates/ew-model/src/library.rs
 * and pcb.rs) and the replies of the `libedit.*` methods.
 */

export type Point = readonly [number, number];

export type LibraryKind = 'symbol' | 'footprint' | 'component';

export type PinKind =
  | 'input'
  | 'output'
  | 'bidirectional'
  | 'triState'
  | 'passive'
  | 'free'
  | 'unspecified'
  | 'powerIn'
  | 'powerOut'
  | 'openCollector'
  | 'openEmitter'
  | 'noConnect';

export const PIN_KINDS: readonly PinKind[] = [
  'input',
  'output',
  'bidirectional',
  'triState',
  'passive',
  'free',
  'unspecified',
  'powerIn',
  'powerOut',
  'openCollector',
  'openEmitter',
  'noConnect',
];

export type PinShape =
  | 'line'
  | 'inverted'
  | 'clock'
  | 'invertedClock'
  | 'inputLow'
  | 'clockLow'
  | 'outputLow'
  | 'fallingEdgeClock'
  | 'nonLogic';

export const PIN_SHAPES: readonly PinShape[] = [
  'line',
  'inverted',
  'clock',
  'invertedClock',
  'inputLow',
  'clockLow',
  'outputLow',
  'fallingEdgeClock',
  'nonLogic',
];

export type Direction = 'right' | 'left' | 'up' | 'down';

export const DIRECTIONS: readonly Direction[] = ['right', 'up', 'left', 'down'];

export interface Pin {
  readonly number: string;
  readonly name?: string;
  readonly kind?: PinKind;
  readonly shape?: PinShape;
  readonly at: Point;
  readonly direction?: Direction;
  readonly length: number;
  readonly hidden?: boolean;
}

export type Geometry =
  | { readonly line: { readonly start: Point; readonly end: Point } }
  | { readonly rect: { readonly start: Point; readonly end: Point } }
  | { readonly circle: { readonly center: Point; readonly radius: number } }
  | { readonly arc: { readonly start: Point; readonly mid: Point; readonly end: Point } }
  | { readonly polyline: { readonly points: readonly Point[] } }
  | { readonly polygon: { readonly points: readonly Point[] } };

export type Fill = 'none' | 'solid' | 'background';

export interface SymbolGraphic {
  readonly geometry: Geometry;
  readonly stroke?: { readonly width?: number };
  readonly fill?: Fill;
}

export interface TextStyle {
  readonly size?: number;
  readonly thickness?: number;
  readonly bold?: boolean;
  readonly italic?: boolean;
  readonly hAlign?: string;
  readonly vAlign?: string;
}

export interface SymbolText {
  readonly text: string;
  readonly at: Point;
  readonly rotation?: number;
  readonly style?: TextStyle;
}

export interface SymbolBody {
  readonly pins?: readonly Pin[];
  readonly graphics?: readonly SymbolGraphic[];
  readonly texts?: readonly SymbolText[];
}

export interface SymbolUnit extends SymbolBody {
  readonly name?: string;
  readonly alternate?: SymbolBody;
}

export interface SymbolDef {
  readonly id: string;
  readonly name: string;
  readonly description?: string;
  readonly keywords?: string;
  readonly prefix: string;
  readonly value?: string;
  readonly footprint?: string;
  readonly datasheet?: string;
  readonly power?: boolean;
  readonly units: readonly SymbolUnit[];
  readonly properties?: Readonly<Record<string, string>>;
  readonly pinText?: {
    readonly hideNumbers?: boolean;
    readonly hideNames?: boolean;
    readonly nameOffset?: number;
  };
}

export type PadKind = 'smd' | 'throughHole' | 'npThroughHole' | 'connector';

export const PAD_KINDS: readonly PadKind[] = ['smd', 'throughHole', 'npThroughHole', 'connector'];

export type PadShape =
  | 'circle'
  | 'rect'
  | 'oval'
  | { readonly roundRect: { readonly ratio: number } }
  | { readonly trapezoid: { readonly delta: Point } }
  | { readonly custom: { readonly points: readonly Point[] } };

export interface Drill {
  readonly diameter: number;
  readonly slotLength?: number;
  readonly offset?: Point;
}

export interface Pad {
  readonly id: string;
  readonly number: string;
  readonly kind?: PadKind;
  readonly shape: PadShape;
  readonly at: Point;
  readonly rotation?: number;
  readonly size: Point;
  readonly drill?: Drill;
  readonly layers: readonly string[];
  readonly maskMargin?: number;
  readonly pasteMargin?: number;
}

export interface BoardGraphic {
  readonly layer: string;
  readonly geometry: Geometry;
  readonly width: number;
  readonly filled?: boolean;
}

export interface FootprintText {
  readonly kind?: 'designator' | 'value' | 'user';
  readonly text?: string;
  readonly layer: string;
  readonly at: Point;
  readonly rotation?: number;
  readonly style: TextStyle;
  readonly visible?: boolean;
}

export interface Model3d {
  readonly path: string;
  readonly offset?: readonly [number, number, number];
  readonly rotation?: readonly [number, number, number];
  readonly scale?: readonly [number, number, number];
  readonly hidden?: boolean;
}

export interface FootprintDef {
  readonly id: string;
  readonly name: string;
  readonly description?: string;
  readonly keywords?: string;
  readonly pads?: readonly Pad[];
  readonly graphics?: readonly BoardGraphic[];
  readonly texts?: readonly FootprintText[];
  readonly models?: readonly Model3d[];
  readonly attributes?: {
    readonly mounting?: 'smd' | 'throughHole' | 'other';
    readonly excludeFromBom?: boolean;
    readonly excludeFromPlacement?: boolean;
    readonly dnp?: boolean;
  };
}

export interface SupplierPart {
  readonly supplier: string;
  readonly partNumber: string;
  readonly url?: string;
}

export interface ComponentDef {
  readonly id: string;
  readonly name: string;
  readonly description?: string;
  readonly keywords?: string;
  readonly symbol: string;
  readonly footprint?: string;
  readonly value?: string;
  readonly manufacturer?: string;
  readonly mpn?: string;
  readonly datasheet?: string;
  readonly properties?: Readonly<Record<string, string>>;
  readonly suppliers?: readonly SupplierPart[];
}

export type Severity = 'error' | 'warning' | 'info';

export interface Finding {
  readonly code: string;
  readonly severity: Severity;
  readonly message: string;
  readonly args: Readonly<Record<string, string>>;
  readonly target?: {
    readonly kind: string;
    readonly unit?: number;
    readonly alternate?: boolean;
    readonly index?: number;
  };
}

export interface CheckCounts {
  readonly errors: number;
  readonly warnings: number;
  readonly infos: number;
}

export interface ItemSummary {
  readonly name: string;
  readonly description: string;
  readonly checks: CheckCounts;
  readonly units?: number;
  readonly pins?: number;
  readonly alternate?: boolean;
  readonly power?: boolean;
  readonly pads?: number;
  readonly models?: number;
  readonly symbol?: string;
  readonly footprint?: string | null;
  readonly mpn?: string;
}

export interface LibraryList {
  readonly name: string;
  readonly symbols: readonly ItemSummary[];
  readonly footprints: readonly ItemSummary[];
  readonly components: readonly ItemSummary[];
}

export interface Usage {
  readonly placed: number;
  readonly board: number;
  readonly symbols: number;
  readonly components: number;
}

export interface ItemReply<T> {
  readonly def: T;
  readonly usage: Usage;
  readonly findings: readonly Finding[];
}

export type ItemKind = 'pin' | 'graphic' | 'text' | 'pad' | 'model';

export interface ItemRef {
  readonly kind: ItemKind;
  readonly index: number;
}

/** A body of a symbol: a unit (1-based), normal or alternate. */
export interface Body {
  readonly unit: number;
  readonly alternate: boolean;
}

/** The pins, graphics and texts of a body. */
export function bodyOf(def: SymbolDef, body: Body): SymbolBody {
  const unit = def.units[body.unit - 1] ?? def.units[0];
  if (unit === undefined) {
    return {};
  }
  return body.alternate && unit.alternate !== undefined ? unit.alternate : unit;
}

/** The kind and index of an editor scene id (tag in the high 64 bits). */
export function decodeItemId(id: string): ItemRef | null {
  const hex = id.replaceAll('-', '');
  if (hex.length !== 32) {
    return null;
  }
  const tag = Number.parseInt(hex.slice(0, 16), 16);
  const index = Number.parseInt(hex.slice(16), 16);
  const kind: ItemKind | undefined = (['pin', 'graphic', 'text', 'pad', 'model'] as const)[tag - 1];
  return kind === undefined || !Number.isFinite(index) ? null : { kind, index };
}

/** The editor scene id of an item. */
export function itemId(ref: ItemRef): string {
  const tag = ['pin', 'graphic', 'text', 'pad', 'model'].indexOf(ref.kind) + 1;
  const hex = tag.toString(16).padStart(16, '0') + ref.index.toString(16).padStart(16, '0');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

/** Millimetres for inputs. */
export function toMm(nm: number): number {
  return Math.round(nm / 10) / 100_000;
}

/** Nanometres from millimetres. */
export function fromMm(mm: number): number {
  return Math.round(mm * 1_000_000);
}
