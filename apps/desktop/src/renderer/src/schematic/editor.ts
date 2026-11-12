/**
 * The schematic editor: tools, snapping, previews and the engine calls of
 * every editing action. It plugs into the viewer controller as its
 * interaction; React components observe it with `subscribe`/`snapshot`.
 *
 * The engine does the actual editing (`schematic.*` methods); this class
 * only collects points, shows previews and selects the result.
 */
import type { MessageKey } from '@shared/i18n';

import type { Interaction, ViewerController } from '../viewer/controller';
import type { NetPlace } from './stores';

export type Point = readonly [number, number];

export type Tool =
  | 'select'
  | 'wire'
  | 'bus'
  | 'label'
  | 'globalLabel'
  | 'hierarchicalLabel'
  | 'power'
  | 'noConnect'
  | 'junction'
  | 'text'
  | 'symbol'
  | 'sheet'
  | 'highlight';

/** A symbol definition as the engine returns it (`schematic.symbol`). */
export interface SymbolDefJson {
  readonly name: string;
  readonly prefix?: string;
  readonly power?: boolean;
  readonly units: readonly {
    readonly pins?: readonly {
      readonly number?: string;
      readonly at: Point;
      readonly direction?: 'right' | 'left' | 'up' | 'down';
      readonly length?: number;
      readonly hidden?: boolean;
    }[];
    readonly graphics?: readonly {
      readonly geometry: Readonly<Record<string, Readonly<Record<string, unknown>>>>;
      readonly fill?: string;
    }[];
  }[];
}

/** A wire that stretches while objects are dragged (`schematic.dragInfo`). */
export interface DragWire {
  readonly id: string;
  readonly start: Point;
  readonly end: Point;
  readonly startMoves: boolean;
  readonly endMoves: boolean;
}

export type PromptKind =
  'label' | 'globalLabel' | 'hierarchicalLabel' | 'power' | 'text' | 'sheet' | 'rename';

/** A text the user types at a point (a label name, a sheet name, …). */
export interface Prompt {
  readonly kind: PromptKind;
  readonly at: Point;
  readonly value: string;
  /** For `rename`: the object and the field to change. */
  readonly target?: { readonly id: string; readonly field: string };
  /** For `sheet`: the rectangle. */
  readonly rect?: readonly [Point, Point];
}

export interface EditorSnapshot {
  readonly tool: Tool;
  /** The cursor on the grid (absolute nm), or `null` outside the canvas. */
  readonly cursor: Point | null;
  /** The points of the wire being drawn. */
  readonly wire: readonly Point[];
  /** Which way a wire bends: horizontal first or vertical first. */
  readonly bend: 'hv' | 'vh';
  /** The symbol being placed. */
  readonly ghost: SymbolDefJson | null;
  /** Rotation (degrees) and mirror of the next symbol or label. */
  readonly rotation: number;
  readonly mirror: boolean;
  /** Objects being moved: offset, stretched wires and objects moving along. */
  readonly move: {
    readonly delta: Point;
    readonly wires: readonly DragWire[];
    readonly followers: readonly string[];
  } | null;
  /** First corner of a new sheet. */
  readonly sheetCorner: Point | null;
  readonly prompt: Prompt | null;
  /** Name of the highlighted net. */
  readonly net: string | null;
  /** A hint for the status line. */
  readonly hint: MessageKey | null;
  /** Snapping to the grid is on. */
  readonly snap: boolean;
}

/** What the editor needs from its surroundings. */
export interface EditorHost {
  readonly doc: string;
  call<T>(method: string, params: unknown): Promise<T>;
  error(error: unknown): void;
  /** Opens the symbol chooser. */
  chooseSymbol(): void;
  /** Opens the find panel. */
  find(): void;
  /** A net was highlighted (or cleared): its members per sheet. */
  highlight(
    net: string | null,
    sheets: Readonly<Record<string, NetMembers>>,
    places: readonly NetPlace[],
  ): void;
  /** Opens the net navigator on a place. */
  navigate(net: string, places: readonly NetPlace[], current: number): void;
}

export interface NetMembers {
  readonly items: readonly string[];
  readonly pins: readonly string[];
}

/** The reply of `schematic.net`. */
interface NetReply {
  readonly net: string | null;
  readonly sheets: Record<string, NetMembers>;
  readonly places: readonly NetPlace[];
}

interface EditResult {
  readonly seq: number | null;
  readonly select: readonly string[];
  readonly changes: number;
}

/** Objects copied with Ctrl+C (shared by all sheets of the session). */
let clipboard: { readonly objects: readonly unknown[]; readonly center: Point } | null = null;

const DRAG_THRESHOLD_PX = 4;

const HINTS: Readonly<Record<Tool, MessageKey>> = {
  select: 'schematic.hints.select',
  wire: 'schematic.hints.wire',
  bus: 'schematic.hints.wire',
  label: 'schematic.hints.label',
  globalLabel: 'schematic.hints.label',
  hierarchicalLabel: 'schematic.hints.label',
  power: 'schematic.hints.power',
  noConnect: 'schematic.hints.flag',
  junction: 'schematic.hints.flag',
  text: 'schematic.hints.text',
  symbol: 'schematic.hints.symbol',
  sheet: 'schematic.hints.sheet',
  highlight: 'schematic.hints.highlight',
};

/** Objects that a wire end connects to (a click there finishes the wire). */
const CONNECTABLE = new Set(['pin', 'wire', 'junction', 'label', 'powerPort', 'sheetPin', 'bus']);

/** The next label name: a trailing number counts up (`D0` → `D1`). */
export function nextName(name: string): string {
  const match = /^(.*?)(\d+)$/u.exec(name);
  if (match === null) {
    return name;
  }
  const digits = match[2] ?? '0';
  const next = String(Number(digits) + 1).padStart(digits.length, '0');
  return `${match[1] ?? ''}${next}`;
}

/** The style of a power port, from its net name. */
export function powerStyle(net: string): string {
  return /^(gnd|vss|0v|agnd|dgnd|pgnd)/iu.test(net) ? 'ground' : 'bar';
}

/** The points that an object of a copy refers to (for the paste position). */
function objectPoints(object: unknown): Point[] {
  if (typeof object !== 'object' || object === null) {
    return [];
  }
  const record = object as Record<string, unknown>;
  const out: Point[] = [];
  for (const key of ['at', 'start', 'end']) {
    const value = record[key];
    if (Array.isArray(value) && value.length === 2) {
      out.push([Number(value[0]), Number(value[1])]);
    }
  }
  return out;
}

/** The centre of the points of copied objects. */
export function centerOf(objects: readonly unknown[]): Point {
  const points = objects.flatMap(objectPoints);
  if (points.length === 0) {
    return [0, 0];
  }
  const xs = points.map((p) => p[0]);
  const ys = points.map((p) => p[1]);
  return [(Math.min(...xs) + Math.max(...xs)) / 2, (Math.min(...ys) + Math.max(...ys)) / 2];
}

/** What a drag drags along (`schematic.dragInfo`). */
export interface DragInfo {
  readonly wires: readonly DragWire[];
  readonly followers: readonly string[];
}

/**
 * The path of a wire whose end `moving` follows a drag by `delta`: straight
 * wires stay straight and get a corner (as the engine does it).
 */
export function stretch(fixed: Point, moving: Point, delta: Point): Point[] {
  const target: Point = [moving[0] + delta[0], moving[1] + delta[1]];
  const horizontal = fixed[1] === moving[1] && fixed[0] !== moving[0];
  const vertical = fixed[0] === moving[0] && fixed[1] !== moving[1];
  let corner: Point;
  if (horizontal && delta[1] !== 0) {
    corner = [target[0], fixed[1]];
  } else if (vertical && delta[0] !== 0) {
    corner = [fixed[0], target[1]];
  } else {
    return [fixed, target];
  }
  const same = (a: Point, b: Point): boolean => a[0] === b[0] && a[1] === b[1];
  return same(corner, fixed) || same(corner, target) ? [fixed, target] : [fixed, corner, target];
}

/** The two-segment path from `a` to `b` with a corner (`hv`: across first). */
export function route(a: Point, b: Point, bend: 'hv' | 'vh'): Point[] {
  if (a[0] === b[0] || a[1] === b[1]) {
    return [a, b];
  }
  const corner: Point = bend === 'hv' ? [b[0], a[1]] : [a[0], b[1]];
  return [a, corner, b];
}

export class SchematicEditor implements Interaction {
  private state: EditorSnapshot = {
    tool: 'select',
    cursor: null,
    wire: [],
    bend: 'hv',
    ghost: null,
    rotation: 0,
    mirror: false,
    move: null,
    sheetCorner: null,
    prompt: null,
    net: null,
    hint: HINTS.select,
    snap: true,
  };
  private readonly listeners = new Set<() => void>();
  /** Raw pointer position (absolute nm). */
  private pointer: Point | null = null;
  /** The press of a possible drag in the select tool. */
  private press: { start: Point; screen: Point; moving: boolean; drag: boolean } | null = null;
  /** A move started from the keyboard (M or G): the next click ends it. */
  private keyMove: { start: Point; drag: boolean } | null = null;
  private moveIds: string[] = [];
  private symbolName: string | null = null;
  /** The component being placed (its symbol is `symbolName`). */
  private componentName: string | null = null;
  private lastLabel = '';
  private lastPower = 'GND';
  private busy = false;

  constructor(
    private readonly controller: ViewerController,
    private readonly host: EditorHost,
  ) {}

  // --- Observation ----------------------------------------------------------

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  snapshot = (): EditorSnapshot => this.state;

  private update(patch: Partial<EditorSnapshot>): void {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) {
      listener();
    }
  }

  // --- Grid -----------------------------------------------------------------

  /** Rounds a point to the grid through the page's top-left corner. */
  snapPoint(p: Point): Point {
    const grid = this.controller.baseGrid;
    if (!this.state.snap || grid <= 0) {
      return [Math.round(p[0]), Math.round(p[1])];
    }
    const [ox, oy] = this.controller.absoluteGridOrigin;
    return [ox + Math.round((p[0] - ox) / grid) * grid, oy + Math.round((p[1] - oy) / grid) * grid];
  }

  setSnap(on: boolean): void {
    this.update({ snap: on });
  }

  // --- Tools ----------------------------------------------------------------

  setTool(tool: Tool): void {
    this.cancel();
    this.update({ tool, hint: HINTS[tool], rotation: 0, mirror: false });
    if (tool !== 'select') {
      this.controller.clearSelection();
    }
  }

  /**
   * Starts placing a symbol of the project library, or a component (its
   * symbol with footprint, value and order data).
   */
  placeSymbol(name: string, component: string | null = null): void {
    this.cancel();
    this.symbolName = name;
    this.componentName = component;
    this.update({ tool: 'symbol', hint: HINTS.symbol, rotation: 0, mirror: false, ghost: null });
    this.controller.clearSelection();
    this.host
      .call<SymbolDefJson>('schematic.symbol', { name })
      .then((def) => {
        if (this.symbolName === name) {
          this.update({ ghost: def });
        }
      })
      .catch((error: unknown) => {
        this.host.error(error);
      });
  }

  /** Ends what is in progress (wire, move, prompt) without changing anything. */
  cancel(): void {
    if (this.state.move !== null) {
      this.controller.setMoveOffset(null);
    }
    this.press = null;
    this.keyMove = null;
    this.update({ wire: [], move: null, sheetCorner: null, prompt: null });
  }

  // --- Engine calls ---------------------------------------------------------

  /** Runs an edit; selects its result once the new scene arrives. */
  private async edit(method: string, params: Record<string, unknown>): Promise<boolean> {
    if (this.busy) {
      return false;
    }
    this.busy = true;
    try {
      const result = await this.host.call<EditResult>(method, { doc: this.host.doc, ...params });
      if (result.select.length > 0) {
        this.controller.selectAfterReload(result.select);
      }
      return true;
    } catch (error) {
      this.host.error(error);
      return false;
    } finally {
      this.busy = false;
    }
  }

  private run(method: string, params: Record<string, unknown>): void {
    void this.edit(method, params);
  }

  rotateSelection(clockwise = false): void {
    const ids = this.controller.selectedIds();
    if (ids.length > 0) {
      this.run('schematic.orient', { ids, orientation: clockwise ? 'right' : 'left' });
    }
  }

  mirrorSelection(axis: 'horizontal' | 'vertical'): void {
    const ids = this.controller.selectedIds();
    if (ids.length > 0) {
      this.run('schematic.orient', { ids, orientation: axis });
    }
  }

  deleteSelection(): void {
    const ids = this.controller.selectedIds();
    if (ids.length > 0) {
      this.controller.clearSelection();
      this.run('schematic.delete', { ids });
    }
  }

  async copySelection(): Promise<boolean> {
    const ids = this.controller.selectedIds();
    if (ids.length === 0) {
      return false;
    }
    try {
      const reply = await this.host.call<{ objects: unknown[] }>('schematic.copy', {
        doc: this.host.doc,
        ids,
      });
      clipboard = { objects: reply.objects, center: centerOf(reply.objects) };
      return true;
    } catch (error) {
      this.host.error(error);
      return false;
    }
  }

  async cutSelection(): Promise<void> {
    if (await this.copySelection()) {
      this.deleteSelection();
    }
  }

  /** Pastes the copied objects with their centre at `at` (default: the cursor). */
  paste(at?: Point): void {
    if (clipboard === null) {
      return;
    }
    const target = this.snapPoint(at ?? this.state.cursor ?? clipboard.center);
    const center = this.snapPoint(clipboard.center);
    this.run('schematic.paste', {
      objects: clipboard.objects,
      offset: [target[0] - center[0], target[1] - center[1]],
    });
  }

  async duplicateSelection(): Promise<void> {
    if (await this.copySelection()) {
      const grid = Math.max(this.controller.baseGrid, 1);
      const center = clipboard?.center ?? [0, 0];
      this.paste([center[0] + grid * 4, center[1] - grid * 4]);
    }
  }

  get canPaste(): boolean {
    return clipboard !== null;
  }

  /** Starts a move of the selection from the keyboard (follows the cursor). */
  startKeyMove(drag: boolean): void {
    const ids = this.controller.selectedIds();
    const cursor = this.state.cursor;
    if (ids.length === 0 || cursor === null) {
      return;
    }
    this.keyMove = { start: cursor, drag };
    this.beginMove(ids, drag);
  }

  private beginMove(ids: string[], drag: boolean): void {
    this.moveIds = ids;
    this.controller.setMoveOffset([0, 0]);
    this.update({ move: { delta: [0, 0], wires: [], followers: [] } });
    if (!drag) {
      return;
    }
    this.host
      .call<DragInfo>('schematic.dragInfo', { doc: this.host.doc, ids })
      .then(({ wires, followers }) => {
        const move = this.state.move;
        if (move !== null) {
          this.controller.setMoveOffset(
            move.delta,
            wires.map((w) => w.id),
            followers,
          );
          this.update({ move: { ...move, wires, followers } });
        }
      })
      .catch(() => undefined);
  }

  private updateMove(start: Point, cursor: Point): void {
    const move = this.state.move;
    if (move === null) {
      return;
    }
    const delta: Point = [cursor[0] - start[0], cursor[1] - start[1]];
    if (delta[0] !== move.delta[0] || delta[1] !== move.delta[1]) {
      this.controller.setMoveOffset(
        delta,
        move.wires.map((w) => w.id),
        move.followers,
      );
      this.update({ move: { ...move, delta } });
    }
  }

  private finishMove(drag: boolean): void {
    const move = this.state.move;
    this.press = null;
    this.keyMove = null;
    if (move === null) {
      return;
    }
    if (move.delta[0] === 0 && move.delta[1] === 0) {
      this.controller.setMoveOffset(null);
      this.update({ move: null });
      return;
    }
    // The offset stays until the new scene arrives (no flicker).
    this.update({ move: null });
    void this.edit('schematic.move', { ids: this.moveIds, delta: move.delta, drag }).then((ok) => {
      if (!ok) {
        this.controller.setMoveOffset(null);
      }
    });
  }

  /** The `schematic.net` parameters of the object at a point, if any. */
  private netParamsAt(point: Point): Record<string, unknown> | null {
    const obj = this.controller.pickAtPoint(point);
    const scene = this.controller.currentScene;
    if (obj === null || scene === null) {
      return null;
    }
    const type = scene.typeOf(obj);
    const parent = scene.parentOf(obj);
    const params: Record<string, unknown> = { doc: this.host.doc };
    if (type === 'pin' && parent !== null) {
      params['pin'] = { symbol: scene.idOf(parent), index: scene.children[obj] ?? 0 };
    } else {
      params['id'] = scene.idOf(this.controller.rootOf(obj));
      params['at'] = this.snapPoint(point);
    }
    return params;
  }

  /** Highlights the net at a point (or clears the highlight). */
  highlightAt(point: Point): void {
    const params = this.netParamsAt(point);
    if (params === null) {
      this.clearHighlight();
      return;
    }
    this.host
      .call<NetReply>('schematic.net', params)
      .then((reply) => {
        this.showNet(reply.net, reply.sheets);
        this.host.highlight(reply.net, reply.sheets, reply.places);
      })
      .catch((error: unknown) => {
        this.host.error(error);
      });
  }

  /**
   * Highlights the net at a point and lists every place it shows up on
   * any sheet (Ctrl/⌘ + double-click), starting at the clicked one.
   */
  navigateAt(point: Point): void {
    const params = this.netParamsAt(point);
    if (params === null) {
      return;
    }
    this.host
      .call<NetReply>('schematic.net', params)
      .then((reply) => {
        if (reply.net === null) {
          return;
        }
        this.showNet(reply.net, reply.sheets);
        this.host.highlight(reply.net, reply.sheets, reply.places);
        // The place nearest to the click on this sheet.
        let current = 0;
        let best = Number.POSITIVE_INFINITY;
        reply.places.forEach((place, index) => {
          const d = Math.hypot(place.at[0] - point[0], place.at[1] - point[1]);
          if (place.doc === this.host.doc && d < best) {
            best = d;
            current = index;
          }
        });
        this.host.navigate(reply.net, reply.places, current);
      })
      .catch((error: unknown) => {
        this.host.error(error);
      });
  }

  /** Shows a highlighted net (also one chosen on another sheet). */
  showNet(net: string | null, sheets: Readonly<Record<string, NetMembers>>): void {
    const members = sheets[this.host.doc];
    this.controller.setHighlight(members === undefined ? [] : [...members.items, ...members.pins]);
    this.update({ net });
  }

  clearHighlight(): void {
    this.controller.setHighlight([]);
    this.update({ net: null });
    this.host.highlight(null, {}, []);
  }

  // --- Prompts --------------------------------------------------------------

  private ask(prompt: Prompt): void {
    this.update({ prompt });
  }

  /** The user confirmed a prompt with this text. */
  submitPrompt(text: string): void {
    const prompt = this.state.prompt;
    this.update({ prompt: null });
    const value = text.trim();
    if (prompt === null || value === '') {
      return;
    }
    const rotation = this.state.rotation;
    switch (prompt.kind) {
      case 'label':
      case 'globalLabel':
      case 'hierarchicalLabel': {
        const label =
          prompt.kind === 'label'
            ? 'local'
            : prompt.kind === 'globalLabel'
              ? 'global'
              : 'hierarchical';
        this.lastLabel = nextName(value);
        this.run('schematic.add', {
          item: { kind: 'label', label, text: value },
          at: prompt.at,
          rotation,
        });
        break;
      }
      case 'power':
        this.lastPower = value;
        this.run('schematic.add', {
          item: { kind: 'powerPort', net: value, style: powerStyle(value) },
          at: prompt.at,
          rotation,
        });
        break;
      case 'text':
        this.run('schematic.add', { item: { kind: 'text', text: value }, at: prompt.at, rotation });
        break;
      case 'sheet': {
        const [a, b] = prompt.rect ?? [prompt.at, prompt.at];
        const at: Point = [Math.min(a[0], b[0]), Math.max(a[1], b[1])];
        const size: Point = [Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1])];
        this.host
          .call('document.add', { kind: 'schematic', name: value, parent: this.host.doc, at, size })
          .catch((error: unknown) => {
            this.host.error(error);
          });
        break;
      }
      case 'rename':
        if (prompt.target !== undefined) {
          void this.host
            .call('edit.apply', {
              label: 'edit-properties',
              changes: [
                {
                  op: 'patch',
                  doc: this.host.doc,
                  id: prompt.target.id,
                  fields: { [prompt.target.field]: value },
                },
              ],
            })
            .catch((error: unknown) => {
              this.host.error(error);
            });
        }
        break;
    }
  }

  cancelPrompt(): void {
    this.update({ prompt: null });
  }

  /** Asks for a new text of a label, power port, text or sheet. */
  private renameAt(point: Point): void {
    const obj = this.controller.pickAtPoint(point);
    const scene = this.controller.currentScene;
    if (obj === null || scene === null) {
      return;
    }
    const root = this.controller.rootOf(obj);
    const type = scene.typeOf(root);
    const field =
      type === 'label' || type === 'text'
        ? 'text'
        : type === 'powerPort'
          ? 'net'
          : type === 'sheet'
            ? 'name'
            : null;
    if (field === null) {
      return;
    }
    const id = scene.idOf(root);
    this.host
      .call<{ objects: Record<string, unknown>[] }>('schematic.copy', {
        doc: this.host.doc,
        ids: [id],
      })
      .then((reply) => {
        const current = reply.objects[0]?.[field];
        this.ask({
          kind: 'rename',
          at: this.snapPoint(point),
          value: typeof current === 'string' ? current : '',
          target: { id, field },
        });
      })
      .catch((error: unknown) => {
        this.host.error(error);
      });
  }

  // --- Wires ----------------------------------------------------------------

  private finishWire(): void {
    const points = this.state.wire;
    this.update({ wire: [] });
    if (points.length >= 2) {
      this.run('schematic.wire', { points, bus: this.state.tool === 'bus' });
    }
  }

  private addWirePoint(p: Point, connects: boolean): void {
    const wire = this.state.wire;
    const last = wire.at(-1);
    if (last === undefined) {
      this.update({ wire: [p] });
      return;
    }
    if (last[0] === p[0] && last[1] === p[1]) {
      // A second click on the same point ends the wire.
      this.finishWire();
      return;
    }
    const path = route(last, p, this.state.bend).slice(1);
    this.update({ wire: [...wire, ...path] });
    if (connects) {
      this.finishWire();
    }
  }

  // --- Interaction ----------------------------------------------------------

  pointerDown(point: Point, event: PointerEvent): boolean {
    if (this.state.prompt !== null) {
      this.cancelPrompt();
    }
    const p = this.snapPoint(point);
    const tool = this.state.tool;
    if (this.keyMove !== null) {
      this.finishMove(this.keyMove.drag);
      return true;
    }
    switch (tool) {
      case 'select': {
        if (event.shiftKey || event.ctrlKey || event.metaKey || event.altKey) {
          return false;
        }
        const obj = this.controller.pickAtPoint(point);
        if (obj === null) {
          return false;
        }
        const root = this.controller.rootOf(obj);
        if (!this.controller.isSelected(root)) {
          this.controller.select([root]);
        }
        const screen = this.controller.screenOf(point);
        this.press = { start: p, screen, moving: false, drag: true };
        return true;
      }
      case 'wire':
      case 'bus': {
        const obj = this.controller.pickAtPoint(point);
        const scene = this.controller.currentScene;
        const connects =
          obj !== null &&
          scene !== null &&
          CONNECTABLE.has(scene.typeOf(obj)) &&
          this.state.wire.length > 0;
        this.addWirePoint(p, connects);
        return true;
      }
      case 'label':
      case 'globalLabel':
      case 'hierarchicalLabel':
        this.ask({ kind: tool, at: p, value: this.lastLabel });
        return true;
      case 'power':
        this.ask({ kind: 'power', at: p, value: this.lastPower });
        return true;
      case 'text':
        this.ask({ kind: 'text', at: p, value: '' });
        return true;
      case 'noConnect':
      case 'junction':
        this.run('schematic.add', { item: { kind: tool }, at: p });
        return true;
      case 'symbol':
        if (this.symbolName !== null) {
          this.run('schematic.place', {
            ...(this.componentName === null
              ? { symbol: this.symbolName }
              : { component: this.componentName }),
            at: p,
            rotation: this.state.rotation,
            mirror: this.state.mirror,
            annotate: true,
          });
        }
        return true;
      case 'sheet': {
        const corner = this.state.sheetCorner;
        if (corner === null) {
          this.update({ sheetCorner: p });
        } else if (corner[0] !== p[0] && corner[1] !== p[1]) {
          this.update({ sheetCorner: null });
          this.ask({ kind: 'sheet', at: p, value: '', rect: [corner, p] });
        }
        return true;
      }
      case 'highlight':
        this.highlightAt(point);
        return true;
    }
  }

  pointerMove(point: Point): void {
    this.pointer = point;
    const p = this.snapPoint(point);
    const cursor = this.state.cursor;
    if (cursor?.[0] !== p[0] || cursor[1] !== p[1]) {
      this.update({ cursor: p });
    }
    const press = this.press;
    if (press !== null) {
      if (!press.moving) {
        const screen = this.controller.screenOf(point);
        if (
          Math.hypot(screen[0] - press.screen[0], screen[1] - press.screen[1]) > DRAG_THRESHOLD_PX
        ) {
          press.moving = true;
          this.beginMove(this.controller.selectedIds(), press.drag);
        }
      }
      if (press.moving) {
        this.updateMove(press.start, p);
      }
    } else if (this.keyMove !== null) {
      this.updateMove(this.keyMove.start, p);
    }
  }

  pointerUp(_point: Point, event: PointerEvent): void {
    const press = this.press;
    if (press?.moving === true) {
      this.finishMove(!event.altKey);
    }
    this.press = null;
  }

  doubleClick(point: Point, event?: MouseEvent): void {
    const tool = this.state.tool;
    if (tool === 'wire' || tool === 'bus') {
      this.finishWire();
    } else if (tool === 'select' || tool === 'highlight') {
      if (event?.metaKey === true || event?.ctrlKey === true) {
        this.navigateAt(point);
      } else if (tool === 'select') {
        this.renameAt(point);
      }
    }
  }

  pointerLeave(): void {
    this.pointer = null;
    this.update({ cursor: null });
  }

  keyDown(event: KeyboardEvent): boolean {
    if (this.state.prompt !== null) {
      return false;
    }
    const mod = event.ctrlKey || event.metaKey;
    const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
    if (mod) {
      return this.modKey(key, event);
    }
    if (event.altKey) {
      return false;
    }
    switch (key) {
      case 'Escape':
        return this.escape();
      case 'Enter':
        if (this.state.wire.length > 0) {
          this.finishWire();
          return true;
        }
        return false;
      case 'Backspace':
        if (this.state.wire.length > 0) {
          this.update({ wire: this.state.wire.slice(0, -1) });
          return true;
        }
        this.deleteSelection();
        return true;
      case 'Delete':
        this.deleteSelection();
        return true;
      case '/':
        this.update({ bend: this.state.bend === 'hv' ? 'vh' : 'hv' });
        return true;
      case 'r':
        if (this.placing()) {
          this.update({ rotation: (this.state.rotation + (event.shiftKey ? 270 : 90)) % 360 });
        } else {
          this.rotateSelection(event.shiftKey);
        }
        return true;
      case 'x':
      case 'y':
        // KiCad keys: X mirrors top to bottom, Y left to right.
        if (this.placing()) {
          // Mirrored before turning: left-right gives −θ, top-bottom 180° − θ.
          const base = key === 'x' ? 180 : 360;
          this.update({
            mirror: !this.state.mirror,
            rotation: (base - this.state.rotation + 360) % 360,
          });
        } else {
          this.mirrorSelection(key === 'x' ? 'vertical' : 'horizontal');
        }
        return true;
      case 'm':
      case 'g':
        this.startKeyMove(key === 'g');
        return true;
      case '`':
        if (this.pointer !== null) {
          this.highlightAt(this.pointer);
        }
        return true;
      default: {
        const tool = this.toolKey(key, event.shiftKey);
        if (tool === null) {
          return false;
        }
        if (tool === 'symbol') {
          this.host.chooseSymbol();
        } else {
          this.setTool(tool);
        }
        return true;
      }
    }
  }

  private toolKey(key: string, shift: boolean): Tool | null {
    switch (key) {
      case 'w':
        return 'wire';
      case 'b':
        return 'bus';
      case 'l':
        return shift ? 'globalLabel' : 'label';
      case 'h':
        return 'hierarchicalLabel';
      case 'p':
        return 'power';
      case 'q':
        return 'noConnect';
      case 'j':
        return 'junction';
      case 't':
        return 'text';
      case 'a':
        return 'symbol';
      case 's':
        return 'sheet';
      case 'n':
        return 'highlight';
      default:
        return null;
    }
  }

  private modKey(key: string, event: KeyboardEvent): boolean {
    switch (key) {
      case 'c':
        void this.copySelection();
        return true;
      case 'x':
        void this.cutSelection();
        return true;
      case 'v':
        this.paste();
        return true;
      case 'd':
        void this.duplicateSelection();
        return true;
      case 'f':
        this.host.find();
        return true;
      case 'l':
        this.setTool('globalLabel');
        return true;
      default:
        return event.key === 'Delete';
    }
  }

  private placing(): boolean {
    const tool = this.state.tool;
    return (
      tool === 'symbol' ||
      tool === 'label' ||
      tool === 'globalLabel' ||
      tool === 'hierarchicalLabel' ||
      tool === 'power' ||
      tool === 'text'
    );
  }

  private escape(): boolean {
    if (this.state.move !== null || this.keyMove !== null) {
      this.cancel();
      return true;
    }
    if (this.state.wire.length > 0 || this.state.sheetCorner !== null) {
      this.update({ wire: [], sheetCorner: null });
      return true;
    }
    if (this.state.tool !== 'select') {
      this.setTool('select');
      this.symbolName = null;
      this.componentName = null;
      this.update({ ghost: null });
      return true;
    }
    if (this.state.net !== null) {
      this.clearHighlight();
      return true;
    }
    return false;
  }
}
