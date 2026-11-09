/**
 * State shared by the sheets of the open project: the highlighted net,
 * requests to show a point on another sheet, and the last ERC report.
 * Each is a tiny store usable with `useSyncExternalStore`.
 */
import type { NetMembers, Point } from './editor';
import type { EditorCommands } from './EditorToolbar';

class Store<T> {
  private listeners = new Set<() => void>();

  constructor(private value: T) {}

  get = (): T => this.value;

  set(value: T): void {
    this.value = value;
    for (const listener of this.listeners) {
      listener();
    }
  }

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };
}

/** Commands of the schematic shown right now (for the command palette). */
export const activeSheet = new Store<EditorCommands | null>(null);

export interface HighlightedNet {
  readonly net: string | null;
  readonly sheets: Readonly<Record<string, NetMembers>>;
}

/** The net highlighted on every sheet. */
export const highlightedNet = new Store<HighlightedNet>({ net: null, sheets: {} });

export interface FocusRequest {
  readonly doc: string;
  readonly at: Point;
  readonly ids: readonly string[];
}

let pendingFocus: FocusRequest | null = null;

/** Asks the viewer of `doc` to show a point and select objects once it opens. */
export function requestFocus(request: FocusRequest): void {
  pendingFocus = request;
}

/** The focus request for a document, taken once. */
export function takeFocus(doc: string): FocusRequest | null {
  if (pendingFocus?.doc !== doc) {
    return null;
  }
  const request = pendingFocus;
  pendingFocus = null;
  return request;
}

export type Severity = 'error' | 'warning';

export interface ErcIssue {
  readonly code: string;
  readonly severity: Severity;
  readonly message: string;
  readonly args: Readonly<Record<string, string>>;
  readonly sheet: string;
  readonly path: string;
  readonly at: Point;
  readonly items: readonly string[];
}

export interface ErcReport {
  readonly issues: readonly ErcIssue[];
  readonly errors: number;
  readonly warnings: number;
  readonly nets: number;
  readonly pins: number;
}

export interface ErcState {
  /** The panel is open. */
  readonly open: boolean;
  readonly report: ErcReport | null;
  readonly running: boolean;
  /** When the report was made (ms since epoch). */
  readonly time: number | null;
}

/** The ERC panel and its last report (kept while switching sheets). */
export const ercState = new Store<ErcState>({
  open: false,
  report: null,
  running: false,
  time: null,
});

/** Forgets everything (when a project closes). */
export function resetSchematicStores(): void {
  highlightedNet.set({ net: null, sheets: {} });
  ercState.set({ open: false, report: null, running: false, time: null });
  pendingFocus = null;
}
