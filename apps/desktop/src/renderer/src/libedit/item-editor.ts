/**
 * The interaction of the symbol and footprint editors: drawing tools,
 * snapping, moving with a shader offset, and the engine calls of every
 * edit (`libedit.symbol` / `libedit.footprint`). Items are addressed by
 * kind and index (`ItemRef`), decoded from the editor scene's object ids.
 */
import type { MessageKey } from '@shared/i18n';

import type { Interaction, ViewerController } from '../viewer/controller';
import {
  DIRECTIONS,
  decodeItemId,
  type Body,
  itemId,
  type Direction,
  type Geometry,
  type ItemRef,
  type Pad,
  type PinKind,
  type Point,
} from './types';

export type ItemTool = 'select' | 'pin' | 'pad' | 'line' | 'rect' | 'circle' | 'polyline' | 'text';

export interface ItemEditorState {
  readonly tool: ItemTool;
  readonly cursor: Point | null;
  /** Points of the shape being drawn. */
  readonly points: readonly Point[];
  /** Offset of the selection while it is moved. */
  readonly move: Point | null;
  /** A text being typed at a point. */
  readonly prompt: Point | null;
  readonly grid: number;
  readonly snap: boolean;
  /** Layer of new footprint graphics. */
  readonly layer: string;
  /** Direction of the next pin. */
  readonly direction: Direction;
  readonly pinKind: PinKind;
  readonly pinLength: number;
  readonly hint: MessageKey;
}

export interface ItemEditorHost {
  readonly kind: 'symbol' | 'footprint';
  /** Runs an edit on a body; resolves with the items to select. */
  edit(edit: Record<string, unknown>, body: Body): Promise<readonly ItemRef[] | null>;
}

const HINTS: Readonly<Record<ItemTool, MessageKey>> = {
  select: 'libedit.hints.select',
  pin: 'libedit.hints.pin',
  pad: 'libedit.hints.pad',
  line: 'libedit.hints.line',
  rect: 'libedit.hints.rect',
  circle: 'libedit.hints.circle',
  polyline: 'libedit.hints.polyline',
  text: 'libedit.hints.text',
};

/** Line widths of new footprint graphics per layer (nm). */
export function defaultWidth(layer: string): number {
  if (layer.startsWith('courtyard')) {
    return 50_000;
  }
  if (layer.startsWith('fab')) {
    return 100_000;
  }
  if (layer.startsWith('cu')) {
    return 200_000;
  }
  return 120_000;
}

/** The default pad of a new footprint. */
export function defaultPad(): Pad {
  return {
    id: '00000000-0000-0000-0000-000000000000',
    number: '',
    shape: { roundRect: { ratio: 250 } },
    at: [0, 0],
    size: [1_000_000, 1_200_000],
    layers: ['cu.top', 'paste.top', 'mask.top'],
  };
}

const DRAG_THRESHOLD_PX = 4;

export class ItemEditor implements Interaction {
  private state: ItemEditorState;
  private readonly listeners = new Set<() => void>();
  private press: { start: Point; screen: Point; moving: boolean } | null = null;
  private moveIds: ItemRef[] = [];
  private busy = false;
  /** The shown body and the footprint's pads (set by the view). */
  private body: Body = { unit: 1, alternate: false };
  private pads: readonly Pad[] = [];

  constructor(
    private readonly controller: ViewerController,
    private readonly host: ItemEditorHost,
  ) {
    const symbol = host.kind === 'symbol';
    this.state = {
      tool: 'select',
      cursor: null,
      points: [],
      move: null,
      prompt: null,
      grid: symbol ? 1_270_000 : 50_000,
      snap: true,
      layer: 'silk.top',
      direction: 'right',
      pinKind: 'passive',
      pinLength: 2_540_000,
      hint: HINTS.select,
    };
    controller.setGrid(this.state.grid);
    // An empty item opens with a useful area around its origin.
    const half = symbol ? 12_700_000 : 3_000_000;
    controller.setHomeMinimum([-half, -half, half, half]);
  }

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  snapshot = (): ItemEditorState => this.state;

  private update(patch: Partial<ItemEditorState>): void {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) {
      listener();
    }
  }

  setTool(tool: ItemTool): void {
    this.cancel();
    this.update({ tool, hint: HINTS[tool] });
    if (tool !== 'select') {
      this.controller.clearSelection();
    }
  }

  /** The shown body (edits go there) and the pads (the pad tool copies one). */
  setContext(body: Body, pads: readonly Pad[]): void {
    this.body = body;
    this.pads = pads;
  }

  /** The pad to copy for the pad tool: the selected one, else the last one. */
  private padTemplate(): Pad | null {
    const selected = this.selection().find((ref) => ref.kind === 'pad');
    return selected === undefined
      ? (this.pads.at(-1) ?? null)
      : (this.pads[selected.index] ?? null);
  }

  setGrid(grid: number): void {
    this.controller.setGrid(grid);
    this.update({ grid });
  }

  setSnap(snap: boolean): void {
    this.update({ snap });
  }

  setLayer(layer: string): void {
    this.update({ layer });
  }

  setPinDefaults(patch: { pinKind?: PinKind; pinLength?: number; direction?: Direction }): void {
    this.update(patch);
  }

  cancel(): void {
    if (this.state.move !== null) {
      this.controller.setMoveOffset(null);
    }
    this.press = null;
    this.update({ points: [], move: null, prompt: null });
  }

  snapPoint(p: Point): Point {
    const grid = this.state.grid;
    if (!this.state.snap || grid <= 0) {
      return [Math.round(p[0]), Math.round(p[1])];
    }
    return [Math.round(p[0] / grid) * grid, Math.round(p[1] / grid) * grid];
  }

  /** The selected items. */
  selection(): ItemRef[] {
    return this.controller
      .selectedIds()
      .map(decodeItemId)
      .filter((ref): ref is ItemRef => ref !== null);
  }

  /** Selects items (after the next scene, if one is coming). */
  select(refs: readonly ItemRef[], now = false): void {
    const ids = refs.map(itemId);
    if (now) {
      this.controller.selectIds(ids);
    } else {
      this.controller.selectAfterReload(ids);
    }
  }

  // --- Edits ----------------------------------------------------------------

  async run(edit: Record<string, unknown>): Promise<boolean> {
    if (this.busy) {
      return false;
    }
    this.busy = true;
    try {
      const select = await this.host.edit(edit, this.body);
      if (select === null) {
        return false;
      }
      // The drawing grows while it is edited; the view stays put.
      this.controller.keepView();
      this.select(select);
      return true;
    } finally {
      this.busy = false;
    }
  }

  deleteSelection(): void {
    const items = this.selection();
    if (items.length > 0) {
      this.controller.clearSelection();
      void this.run({ op: 'delete', items });
    }
  }

  orientSelection(orientation: 'left' | 'right' | 'horizontal' | 'vertical'): void {
    const items = this.selection();
    if (items.length > 0) {
      void this.run({ op: 'orient', items, orientation });
    }
  }

  duplicateSelection(): void {
    const items = this.selection();
    if (items.length > 0) {
      const step = this.state.grid * 2;
      void this.run({ op: 'duplicate', items, offset: [step, -step] });
    }
  }

  private finishMove(): void {
    const move = this.state.move;
    this.press = null;
    if (move === null) {
      return;
    }
    this.update({ move: null });
    if (move[0] === 0 && move[1] === 0) {
      this.controller.setMoveOffset(null);
      return;
    }
    void this.run({ op: 'move', items: this.moveIds, delta: move }).then((ok) => {
      if (!ok) {
        this.controller.setMoveOffset(null);
      }
    });
  }

  private graphic(geometry: Geometry): void {
    if (this.host.kind === 'symbol') {
      void this.run({ op: 'addGraphic', graphic: { geometry } });
    } else {
      const layer = this.state.layer;
      void this.run({ op: 'addGraphic', graphic: { layer, geometry, width: defaultWidth(layer) } });
    }
  }

  /** The text typed for the text tool. */
  submitText(text: string): void {
    const at = this.state.prompt;
    this.update({ prompt: null });
    if (at === null || text.trim() === '') {
      return;
    }
    if (this.host.kind === 'symbol') {
      void this.run({ op: 'addText', text: { text, at, style: { size: 1_270_000 } } });
    } else {
      void this.run({
        op: 'addText',
        text: {
          kind: 'user',
          text,
          layer: this.state.layer,
          at,
          style: { size: 1_000_000, thickness: 150_000 },
        },
      });
    }
  }

  cancelText(): void {
    this.update({ prompt: null });
  }

  private finishPolyline(): void {
    const points = this.state.points;
    this.update({ points: [] });
    if (points.length >= 2) {
      this.graphic({ polyline: { points } });
    }
  }

  // --- Interaction ----------------------------------------------------------

  pointerDown(point: Point, event: PointerEvent): boolean {
    const p = this.snapPoint(point);
    const points = this.state.points;
    switch (this.state.tool) {
      case 'select': {
        if (event.shiftKey || event.ctrlKey || event.metaKey) {
          return false;
        }
        const obj = this.controller.pickAtPoint(point);
        if (obj === null) {
          return false;
        }
        if (!this.controller.isSelected(obj)) {
          this.controller.select([obj]);
        }
        this.press = { start: p, screen: this.controller.screenOf(point), moving: false };
        return true;
      }
      case 'pin':
        void this.run({
          op: 'addPin',
          pin: {
            number: '',
            name: '',
            kind: this.state.pinKind,
            at: p,
            direction: this.state.direction,
            length: this.state.pinLength,
          },
        });
        return true;
      case 'pad': {
        const template = this.padTemplate() ?? defaultPad();
        void this.run({ op: 'addPad', pad: { ...template, number: '', at: p } });
        return true;
      }
      case 'line': {
        const last = points.at(-1);
        if (last === undefined) {
          this.update({ points: [p] });
        } else if (last[0] !== p[0] || last[1] !== p[1]) {
          this.graphic({ line: { start: last, end: p } });
          // Lines chain: the end starts the next one.
          this.update({ points: [p] });
        }
        return true;
      }
      case 'rect':
      case 'circle': {
        const first = points[0];
        if (first === undefined) {
          this.update({ points: [p] });
        } else {
          this.update({ points: [] });
          if (this.state.tool === 'rect' && first[0] !== p[0] && first[1] !== p[1]) {
            this.graphic({ rect: { start: first, end: p } });
          } else if (this.state.tool === 'circle') {
            const radius = Math.round(Math.hypot(p[0] - first[0], p[1] - first[1]));
            if (radius > 0) {
              this.graphic({ circle: { center: first, radius } });
            }
          }
        }
        return true;
      }
      case 'polyline': {
        const last = points.at(-1);
        if (last?.[0] === p[0] && last[1] === p[1]) {
          this.finishPolyline();
        } else {
          this.update({ points: [...points, p] });
        }
        return true;
      }
      case 'text':
        this.update({ prompt: p });
        return true;
    }
  }

  pointerMove(point: Point): void {
    const p = this.snapPoint(point);
    const cursor = this.state.cursor;
    if (cursor?.[0] !== p[0] || cursor[1] !== p[1]) {
      this.update({ cursor: p });
    }
    const press = this.press;
    if (press === null) {
      return;
    }
    if (!press.moving) {
      const screen = this.controller.screenOf(point);
      if (
        Math.hypot(screen[0] - press.screen[0], screen[1] - press.screen[1]) > DRAG_THRESHOLD_PX
      ) {
        press.moving = true;
        this.moveIds = this.selection();
        this.controller.setMoveOffset([0, 0]);
      }
    }
    if (press.moving) {
      const delta: Point = [p[0] - press.start[0], p[1] - press.start[1]];
      const move = this.state.move;
      if (move?.[0] !== delta[0] || move[1] !== delta[1]) {
        this.controller.setMoveOffset(delta);
        this.update({ move: delta });
      }
    }
  }

  pointerUp(): void {
    if (this.press?.moving === true) {
      this.finishMove();
    }
    this.press = null;
  }

  doubleClick(): void {
    if (this.state.tool === 'polyline') {
      this.finishPolyline();
    } else if (this.state.tool === 'line') {
      this.update({ points: [] });
    }
  }

  pointerLeave(): void {
    this.update({ cursor: null });
  }

  keyDown(event: KeyboardEvent): boolean {
    if (this.state.prompt !== null) {
      return false;
    }
    const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
    if (event.ctrlKey || event.metaKey) {
      if (key === 'd') {
        this.duplicateSelection();
        return true;
      }
      return false;
    }
    switch (key) {
      case 'Escape':
        if (this.state.points.length > 0 || this.state.move !== null) {
          this.cancel();
          return true;
        }
        if (this.state.tool !== 'select') {
          this.setTool('select');
          return true;
        }
        return false;
      case 'Enter':
        if (this.state.tool === 'polyline') {
          this.finishPolyline();
          return true;
        }
        return false;
      case 'Delete':
      case 'Backspace':
        this.deleteSelection();
        return true;
      case 'r':
        if (this.state.tool === 'pin') {
          const index = DIRECTIONS.indexOf(this.state.direction);
          this.update({ direction: DIRECTIONS[(index + (event.shiftKey ? 3 : 1)) % 4] ?? 'right' });
        } else {
          this.orientSelection(event.shiftKey ? 'right' : 'left');
        }
        return true;
      case 'x':
        this.orientSelection('vertical');
        return true;
      case 'y':
        this.orientSelection('horizontal');
        return true;
      case 'p':
        this.setTool(this.host.kind === 'symbol' ? 'pin' : 'pad');
        return true;
      case 'l':
        this.setTool('line');
        return true;
      case 'b':
        this.setTool('rect');
        return true;
      case 'c':
        this.setTool('circle');
        return true;
      case 'k':
        this.setTool('polyline');
        return true;
      case 't':
        this.setTool('text');
        return true;
      default:
        return false;
    }
  }
}
