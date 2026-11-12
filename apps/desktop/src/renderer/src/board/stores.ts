/**
 * State shared by the board views of the open project: the last design
 * rules check, cross-probing between schematic and board, and the
 * commands of the shown board (for the command palette).
 */
import { Store } from '../schematic/stores';
import type { Point } from './editor';

export type DrcSeverity = 'error' | 'warning';

/** An object a finding is about (a pad: its footprint and index). */
export interface DrcTarget {
  readonly id: string;
  readonly pad?: number;
}

export interface DrcFinding {
  readonly code: string;
  readonly severity: DrcSeverity;
  readonly message: string;
  readonly args: Readonly<Record<string, string>>;
  readonly at: Point;
  readonly layer?: string;
  readonly targets: readonly DrcTarget[];
  readonly key: string;
  readonly excluded: boolean;
}

/** An unrouted connection. */
export interface Airwire {
  readonly net: string;
  readonly from: Point;
  readonly to: Point;
}

export interface DrcReport {
  readonly findings: readonly DrcFinding[];
  readonly counts: {
    readonly errors: number;
    readonly warnings: number;
    readonly excluded: number;
    readonly unrouted: number;
  };
  readonly airwires: readonly Airwire[];
  readonly ruleErrors: readonly { rule: string; message: string; position: number }[];
}

export interface DrcState {
  /** The board the report is about. */
  readonly doc: string | null;
  readonly report: DrcReport | null;
  /** The panel is open. */
  readonly open: boolean;
  readonly running: boolean;
  /** Run the check after every change. */
  readonly online: boolean;
  /** Show the ratsnest. */
  readonly ratsnest: boolean;
}

export const drcState = new Store<DrcState>({
  doc: null,
  report: null,
  open: false,
  running: false,
  online: true,
  ratsnest: true,
});

/** Which bends routes may have: only 45° (default) or also 90°. */
export type Corners = '45' | '90';

const CORNERS_KEY = 'ew.routing.corners';

function storedCorners(): Corners {
  try {
    return window.localStorage.getItem(CORNERS_KEY) === '90' ? '90' : '45';
  } catch {
    return '45';
  }
}

/** The corner mode of the router (kept per user in this browser profile). */
export const routingCorners = new Store<Corners>(
  typeof window === 'undefined' ? '45' : storedCorners(),
);

export function setRoutingCorners(corners: Corners): void {
  routingCorners.set(corners);
  try {
    window.localStorage.setItem(CORNERS_KEY, corners);
  } catch {
    // Storage may be unavailable; the setting then lasts for the session.
  }
}

/** A selection made on one side, to show on the other. */
export interface CrossSelection {
  readonly source: 'board' | 'schematic';
  readonly footprints: readonly string[];
  readonly symbols: readonly { readonly doc: string; readonly id: string }[];
}

export const crossSelection = new Store<CrossSelection | null>(null);

/** Cross-probing is switched on. */
export const crossProbing = new Store<boolean>(true);

/** Commands of the board shown right now (for the command palette). */
export interface BoardCommands {
  readonly drc: () => void;
  readonly fillZones: () => void;
  readonly update: () => void;
  readonly setup: () => void;
  readonly rules: () => void;
  readonly importDxf: () => void;
  readonly placeFootprint: () => void;
}

export const activeBoard = new Store<BoardCommands | null>(null);

/** Forgets the board state (when a project closes). */
export function resetBoardStores(): void {
  drcState.set({ ...drcState.get(), doc: null, report: null, open: false, running: false });
  crossSelection.set(null);
  activeBoard.set(null);
}
