/**
 * Data of a system of several boards as the engine sends them
 * (`system.info`, `system.check`, `system.connectors`).
 */

export interface ConnectorRef {
  readonly board: string;
  readonly designator: string;
}

export type BoardSource =
  | { readonly kind: 'local'; readonly doc: string }
  | { readonly kind: 'project'; readonly path: string; readonly board?: string };

export interface SystemBoardInfo {
  readonly id: string;
  readonly name: string;
  readonly source: BoardSource;
  readonly quantity: number;
  /** Nanometres; z is the board's bottom face. */
  readonly position: readonly [number, number, number];
  /** Degrees about x, y, z. */
  readonly rotation: readonly [number, number, number];
  readonly hidden: boolean;
  readonly local: boolean;
  readonly project: string;
  readonly loaded: boolean;
  readonly error?: string;
  readonly board?: string;
  readonly size?: readonly [number, number];
  readonly connectors?: number;
}

export type Pairing = 'number' | 'mirrored' | 'position';

export const PAIRINGS: readonly Pairing[] = ['number', 'mirrored', 'position'];

export interface Link {
  readonly id: string;
  readonly a: ConnectorRef;
  readonly b: ConnectorRef;
  readonly pairing?: Pairing;
}

export interface CableEnd {
  readonly id: string;
  readonly label: string;
  readonly connector?: ConnectorRef;
  readonly housing?: string;
}

export interface WireEnd {
  readonly end: string;
  readonly pin: string;
}

export interface Wire {
  readonly id: string;
  readonly from: WireEnd;
  readonly to: WireEnd;
  readonly signal?: string;
  readonly color?: string;
  readonly gauge?: string;
  /** Nanometres. */
  readonly length?: number;
}

export interface Cable {
  readonly id: string;
  readonly name: string;
  readonly part?: string;
  /** Nanometres. */
  readonly length?: number;
  readonly ends: readonly CableEnd[];
  readonly wires: readonly Wire[];
}

export interface SystemInfo {
  readonly id: string;
  readonly name: string;
  readonly boards: readonly SystemBoardInfo[];
  readonly links: readonly Link[];
  readonly cables: readonly Cable[];
  readonly aliases: readonly (readonly string[])[];
  readonly exclusions: readonly string[];
}

export interface PinPair {
  readonly a?: string;
  readonly aNet?: string;
  readonly b?: string;
  readonly bNet?: string;
  readonly wire?: string;
  readonly status: string;
}

export interface ConnectionReport {
  readonly id: string;
  readonly kind: 'link' | 'cable';
  readonly name: string;
  readonly pairs: readonly PinPair[];
}

export interface SystemFinding {
  readonly code: string;
  readonly severity: 'error' | 'warning';
  readonly message: string;
  readonly args: Readonly<Record<string, string>>;
  readonly connection?: string;
  readonly pins: readonly {
    readonly board: string;
    readonly designator: string;
    readonly pin: string;
  }[];
  readonly key: string;
  readonly excluded: boolean;
}

export interface SystemCheck {
  readonly findings: readonly SystemFinding[];
  readonly connections: readonly ConnectionReport[];
  readonly errors: number;
  readonly warnings: number;
  readonly tessellate?: readonly string[];
}

export interface ConnectorInfo {
  readonly designator: string;
  readonly footprint: string;
  readonly value: string;
  readonly side: 'top' | 'bottom';
  readonly pins: number;
  readonly signals: number;
}

/** A net name without its sheet path. */
export function shortName(net: string | undefined): string {
  if (net === undefined) {
    return '';
  }
  const parts = net.split('/');
  return parts[parts.length - 1] ?? net;
}
