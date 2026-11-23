/**
 * The selection filter: which kinds of objects a click, a box or a lasso
 * selects (and the select tool moves). Pads and texts of a part go with the
 * part. The filter holds for the session, one per kind of document.
 */
import { Store } from '../schematic/stores';

export type BoardFilterGroup = 'parts' | 'tracks' | 'vias' | 'zones' | 'texts' | 'graphics';
export type SheetFilterGroup =
  'symbols' | 'wires' | 'labels' | 'sheets' | 'flags' | 'texts' | 'graphics';
export type FilterGroup = BoardFilterGroup | SheetFilterGroup;

export const BOARD_GROUPS: readonly BoardFilterGroup[] = [
  'parts',
  'tracks',
  'vias',
  'zones',
  'texts',
  'graphics',
];
export const SHEET_GROUPS: readonly SheetFilterGroup[] = [
  'symbols',
  'wires',
  'labels',
  'sheets',
  'flags',
  'texts',
  'graphics',
];

/** The group of a top-level object type of the scene. */
const TYPE_GROUP: Readonly<Record<string, FilterGroup>> = {
  footprint: 'parts',
  pad: 'parts',
  footprintText: 'parts',
  track: 'tracks',
  via: 'vias',
  zone: 'zones',
  boardText: 'texts',
  boardShape: 'graphics',
  dimension: 'graphics',
  symbol: 'symbols',
  pin: 'symbols',
  field: 'symbols',
  wire: 'wires',
  bus: 'wires',
  busEntry: 'wires',
  junction: 'wires',
  label: 'labels',
  powerPort: 'labels',
  sheet: 'sheets',
  sheetPin: 'sheets',
  noConnect: 'flags',
  text: 'texts',
  shape: 'graphics',
};

export function groupOf(type: string): FilterGroup | null {
  return TYPE_GROUP[type] ?? null;
}

/** The groups switched off, per kind of document (empty: all selectable). */
export const selectionFilter = new Store<{
  readonly pcb: ReadonlySet<BoardFilterGroup>;
  readonly schematic: ReadonlySet<SheetFilterGroup>;
}>({ pcb: new Set(), schematic: new Set() });

/** Whether objects of a type may be selected in a kind of document. */
export function selectable(kind: 'pcb' | 'schematic', type: string): boolean {
  const group = groupOf(type);
  if (group === null) {
    return true;
  }
  const off: ReadonlySet<string> = selectionFilter.get()[kind];
  return !off.has(group);
}
