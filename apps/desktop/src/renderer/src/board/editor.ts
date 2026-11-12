/**
 * The layout editor: tools, snapping, the routing preview and the engine
 * calls of every board edit. Like the schematic editor it plugs into the
 * viewer controller as its interaction; React components observe it with
 * `subscribe`/`snapshot`.
 *
 * The engine does the editing (`pcb.*` methods); this class collects
 * points, asks the router for previews and selects the results.
 */
import type { MessageKey } from '@shared/i18n';

import type { Interaction, ViewerController } from '../viewer/controller';
import { routingCorners } from './stores';

export type Point = readonly [number, number];

export type BoardTool =
  'select' | 'route' | 'via' | 'zone' | 'keepout' | 'outline' | 'text' | 'footprint' | 'highlight';

/** A route being drawn. */
export interface RouteState {
  readonly net: string | null;
  readonly netName: string;
  readonly width: number;
  readonly viaDiameter: number;
  readonly viaDrill: number;
  /** Direction of the existing track the route started at (bends from it count). */
  readonly startDirection: Point | null;
  /** The layer of the live segment. */
  readonly layer: string;
  /** Start of the live segment. */
  readonly start: Point;
  /** Segments fixed with a click, not yet in the board. */
  readonly fixed: readonly { readonly layer: string; readonly points: readonly Point[] }[];
  /** Vias placed while routing. */
  readonly vias: readonly Point[];
  /** The live path from `start` towards the cursor. */
  readonly preview: readonly Point[];
  readonly reached: boolean;
  readonly viaFits: boolean;
}

export interface BoardSnapshot {
  readonly tool: BoardTool;
  /** The cursor on the grid (absolute nm), or `null` outside the canvas. */
  readonly cursor: Point | null;
  /** The active copper layer (`cu.top`, …). */
  readonly layer: string;
  /** Copper layers of the board, from the top. */
  readonly layers: readonly string[];
  readonly route: RouteState | null;
  /** Corners of a zone, keep-out or outline being drawn. */
  readonly polygon: readonly Point[];
  /** Outlines are drawn as rectangles or polygons. */
  readonly outlineShape: 'rect' | 'polygon';
  /** Objects being moved: the offset. */
  readonly move: { readonly delta: Point } | null;
  /** A text being typed. */
  readonly prompt: { readonly at: Point; readonly value: string } | null;
  /** The footprint being placed. */
  readonly footprint: string | null;
  /** Rotation (degrees) of the next footprint. */
  readonly rotation: number;
  /** Routes start with the diagonal. */
  readonly diagonal: boolean;
  /** Name of the highlighted net. */
  readonly net: string | null;
  readonly hint: MessageKey | null;
  readonly snap: boolean;
}

/** What the editor needs from its surroundings. */
export interface BoardHost {
  readonly doc: string;
  call<T>(method: string, params: unknown): Promise<T>;
  error(error: unknown): void;
  /** Opens the footprint chooser. */
  chooseFootprint(): void;
  /** A zone or keep-out outline was drawn: ask for its settings. */
  newZone(outline: readonly Point[], keepout: boolean, layer: string): void;
  /** Opens the settings of an existing zone. */
  editZone(id: string): void;
  /** The board changed through an edit (for the online DRC). */
  edited(): void;
}

interface EditResult {
  readonly seq: number | null;
  readonly select?: readonly string[];
}

interface StartReply {
  readonly at: Point;
  readonly net: string | null;
  readonly netName: string;
  readonly width: number;
  readonly viaDiameter: number;
  readonly viaDrill: number;
  readonly previous?: Point | null;
}

interface RouteReply {
  readonly points: readonly Point[];
  readonly reached: boolean;
  readonly viaFits?: boolean;
}

const DRAG_THRESHOLD_PX = 4;

const HINTS: Readonly<Record<BoardTool, MessageKey>> = {
  select: 'board.hints.select',
  route: 'board.hints.route',
  via: 'board.hints.via',
  zone: 'board.hints.zone',
  keepout: 'board.hints.zone',
  outline: 'board.hints.outline',
  text: 'board.hints.text',
  footprint: 'board.hints.footprint',
  highlight: 'board.hints.highlight',
};

/** The silkscreen layer on the side of a copper layer. */
export function silkOf(layer: string): string {
  return layer === 'cu.bottom' ? 'silk.bottom' : 'silk.top';
}

/** The copper layer reached by a via from `layer` (top ↔ bottom). */
export function otherLayer(layer: string, layers: readonly string[]): string {
  const top = layers[0] ?? 'cu.top';
  const bottom = layers.at(-1) ?? 'cu.bottom';
  return layer === top ? bottom : top;
}

/** Points without repeats in a row. */
export function dedupe(points: readonly Point[]): Point[] {
  const out: Point[] = [];
  for (const p of points) {
    const last = out.at(-1);
    if (last?.[0] !== p[0] || last[1] !== p[1]) {
      out.push(p);
    }
  }
  return out;
}

function rounded(p: Point): Point {
  return [Math.round(p[0]), Math.round(p[1])];
}

export class BoardEditor implements Interaction {
  private state: BoardSnapshot = {
    tool: 'select',
    cursor: null,
    layer: 'cu.top',
    layers: ['cu.top', 'cu.bottom'],
    route: null,
    polygon: [],
    outlineShape: 'rect',
    move: null,
    prompt: null,
    footprint: null,
    rotation: 0,
    diagonal: false,
    net: null,
    hint: HINTS.select,
    snap: true,
  };
  private readonly listeners = new Set<() => void>();
  private pointer: Point | null = null;
  private press: { start: Point; screen: Point; moving: boolean } | null = null;
  private keyMove: { start: Point; drag: boolean } | null = null;
  private moveIds: string[] = [];
  private busy = false;
  /** Routing previews: one request at a time, the latest target wins. */
  private routing = false;
  private pendingTarget: Point | null = null;

  constructor(
    private readonly controller: ViewerController,
    private readonly host: BoardHost,
  ) {}

  // --- Observation ----------------------------------------------------------

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  snapshot = (): BoardSnapshot => this.state;

  private update(patch: Partial<BoardSnapshot>): void {
    this.state = { ...this.state, ...patch };
    for (const listener of this.listeners) {
      listener();
    }
  }

  /** Takes the copper layers of a new scene; keeps the active one if it still exists. */
  setLayers(layers: readonly string[]): void {
    if (layers.length === 0) {
      return;
    }
    const same =
      layers.length === this.state.layers.length &&
      layers.every((layer, i) => layer === this.state.layers[i]);
    if (same) {
      return;
    }
    const layer = layers.includes(this.state.layer) ? this.state.layer : (layers[0] ?? 'cu.top');
    this.update({ layers, layer });
  }

  // --- Grid -----------------------------------------------------------------

  snapPoint(p: Point): Point {
    const grid = this.controller.baseGrid;
    if (!this.state.snap || grid <= 0) {
      return rounded(p);
    }
    const [ox, oy] = this.controller.absoluteGridOrigin;
    return [ox + Math.round((p[0] - ox) / grid) * grid, oy + Math.round((p[1] - oy) / grid) * grid];
  }

  setSnap(on: boolean): void {
    this.update({ snap: on });
  }

  // --- Tools and layers -----------------------------------------------------

  setTool(tool: BoardTool): void {
    this.finishRoute();
    this.cancel();
    this.update({ tool, hint: HINTS[tool], rotation: 0 });
    if (tool !== 'select') {
      this.controller.clearSelection();
    }
  }

  /** Starts placing a footprint of the project libraries. */
  placeFootprint(name: string): void {
    this.cancel();
    this.update({ tool: 'footprint', footprint: name, hint: HINTS.footprint, rotation: 0 });
    this.controller.clearSelection();
  }

  setLayer(layer: string): void {
    if (!this.state.layers.includes(layer) || layer === this.state.layer) {
      return;
    }
    const route = this.state.route;
    if (route !== null) {
      this.switchRouteLayer(layer);
      return;
    }
    this.update({ layer });
  }

  /** The next (or previous) copper layer. */
  stepLayer(direction: 1 | -1): void {
    const layers = this.state.layers;
    const index = layers.indexOf(this.state.route?.layer ?? this.state.layer);
    const next = layers[(index + direction + layers.length) % layers.length];
    if (next !== undefined) {
      this.setLayer(next);
    }
  }

  setOutlineShape(shape: 'rect' | 'polygon'): void {
    this.update({ outlineShape: shape, polygon: [] });
  }

  /** Ends what is in progress (move, polygon, prompt) without changes. */
  cancel(): void {
    if (this.state.move !== null) {
      this.controller.setMoveOffset(null);
    }
    this.press = null;
    this.keyMove = null;
    this.update({ move: null, polygon: [], prompt: null });
  }

  // --- Engine calls ---------------------------------------------------------

  private async edit(method: string, params: Record<string, unknown>): Promise<boolean> {
    if (this.busy) {
      return false;
    }
    this.busy = true;
    try {
      const result = await this.host.call<EditResult>(method, { doc: this.host.doc, ...params });
      if (result.select !== undefined && result.select.length > 0) {
        this.controller.selectAfterReload(result.select);
      }
      if (result.seq !== null) {
        this.host.edited();
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

  /** Runs a `pcb.edit` operation on the selection. */
  editSelection(op: string, extra: Record<string, unknown> = {}): void {
    const ids = this.selectedObjects();
    if (ids.length > 0) {
      this.run('pcb.edit', { op, ids, ...extra });
    }
  }

  /** The selected board objects (pads and footprint texts count as their footprint). */
  selectedObjects(): string[] {
    return [...new Set(this.controller.selectedIds())];
  }

  rotateSelection(clockwise = false): void {
    this.editSelection('rotate', { angle: clockwise ? 270 : 90 });
  }

  flipSelection(): void {
    this.editSelection('flip');
  }

  deleteSelection(): void {
    const ids = this.selectedObjects();
    if (ids.length > 0) {
      this.controller.clearSelection();
      this.run('pcb.edit', { op: 'delete', ids });
    }
  }

  lockSelection(locked: boolean): void {
    this.editSelection('lock', { locked });
  }

  align(mode: string): void {
    this.editSelection('align', { mode });
  }

  distribute(axis: 'x' | 'y'): void {
    this.editSelection('distribute', { axis });
  }

  fillZones(fill = true): void {
    this.run(fill ? 'pcb.fillZones' : 'pcb.unfillZones', {});
  }

  // --- Moving ---------------------------------------------------------------

  startKeyMove(drag: boolean): void {
    const ids = this.selectedObjects();
    const cursor = this.state.cursor;
    if (ids.length === 0 || cursor === null) {
      return;
    }
    this.keyMove = { start: cursor, drag };
    this.beginMove(ids);
  }

  private beginMove(ids: string[]): void {
    this.moveIds = ids;
    this.controller.setMoveOffset([0, 0]);
    this.update({ move: { delta: [0, 0] } });
  }

  private updateMove(start: Point, cursor: Point): void {
    const move = this.state.move;
    if (move === null) {
      return;
    }
    const delta: Point = [cursor[0] - start[0], cursor[1] - start[1]];
    if (delta[0] !== move.delta[0] || delta[1] !== move.delta[1]) {
      this.controller.setMoveOffset(delta);
      this.update({ move: { delta } });
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
    this.update({ move: null });
    const ids = this.moveIds;
    const types = this.controller.snapshot().selection.map((s) => s.type);
    const track =
      ids.length === 1 && types.length === 1 && types[0] === 'track' ? ids[0] : undefined;
    const request =
      drag && track !== undefined
        ? this.edit('pcb.edit', { op: 'dragTrack', id: track, offset: move.delta })
        : this.edit('pcb.edit', { op: 'move', ids, offset: move.delta });
    void request.then((ok) => {
      if (!ok) {
        this.controller.setMoveOffset(null);
      }
    });
  }

  // --- Net highlight --------------------------------------------------------

  /** Highlights the net of the copper at a point (or clears it). */
  highlightAt(point: Point): void {
    const obj = this.controller.pickAtPoint(point);
    const scene = this.controller.currentScene;
    const net = obj === null || scene === null ? null : scene.netOf(obj);
    this.showNet(net);
  }

  /** Highlights every object of a net. */
  showNet(net: string | null): void {
    const scene = this.controller.currentScene;
    if (net === null || scene === null) {
      this.controller.setHighlight([]);
      this.update({ net: null });
      return;
    }
    const keys: string[] = [];
    for (let obj = 0; obj < scene.objectCount; obj += 1) {
      if (scene.netOf(obj) === net) {
        keys.push(scene.keyOf(obj));
      }
    }
    this.controller.setHighlight(keys);
    this.update({ net });
  }

  // --- Routing --------------------------------------------------------------

  private startRoute(point: Point): void {
    const layer = this.state.layer;
    this.host
      .call<StartReply>('pcb.routeStart', { doc: this.host.doc, at: rounded(point), layer })
      .then((start) => {
        const at = rounded(start.at);
        this.update({
          route: {
            net: start.net,
            netName: start.netName,
            width: start.width,
            viaDiameter: start.viaDiameter,
            viaDrill: start.viaDrill,
            startDirection: start.previous ?? null,
            layer,
            start: at,
            fixed: [],
            vias: [],
            preview: [at],
            reached: true,
            viaFits: true,
          },
        });
        if (start.netName !== '') {
          this.showNet(start.netName);
        }
      })
      .catch((error: unknown) => {
        this.host.error(error);
      });
  }

  /** The routing target for a pointer position: a pad of the net snaps to its centre. */
  private routeTarget(point: Point): Point {
    const route = this.state.route;
    const scene = this.controller.currentScene;
    const obj = this.controller.pickAtPoint(point);
    if (route !== null && scene !== null && obj !== null) {
      const type = scene.typeOf(obj);
      if ((type === 'pad' || type === 'via') && scene.netOf(obj) === route.netName) {
        const [x0, y0, x1, y1] = scene.box(obj);
        const [ox, oy] = scene.meta.origin;
        return rounded([(x0 + x1) / 2 + ox, (y0 + y1) / 2 + oy]);
      }
    }
    return this.snapPoint(point);
  }

  /**
   * The direction of the segment that ends where the live path starts: the
   * last fixed segment on the same layer (not across a via), or the track
   * the route began at.
   */
  private previousDirection(route: RouteState): Point | null {
    const at = route.start;
    if (route.vias.some((via) => via[0] === at[0] && via[1] === at[1])) {
      return null;
    }
    const last = route.fixed.at(-1);
    if (last === undefined) {
      return route.startDirection;
    }
    const end = last.points.at(-1);
    const before = last.points.at(-2);
    if (last.layer !== route.layer || end === undefined || before === undefined) {
      return null;
    }
    return [end[0] - before[0], end[1] - before[1]];
  }

  private requestRoute(target: Point): void {
    const route = this.state.route;
    if (route === null) {
      return;
    }
    if (this.routing) {
      this.pendingTarget = target;
      return;
    }
    this.routing = true;
    const asked = route;
    this.host
      .call<RouteReply>('pcb.route', {
        doc: this.host.doc,
        layer: route.layer,
        net: route.net,
        width: route.width,
        start: route.start,
        target,
        diagonalFirst: this.state.diagonal,
        corners: routingCorners.get(),
        previous: this.previousDirection(route),
        via: route.viaDiameter,
      })
      .then((reply) => {
        const current = this.state.route;
        if (current !== null && current.start === asked.start && current.layer === asked.layer) {
          this.update({
            route: {
              ...current,
              preview: dedupe(reply.points.map(rounded)),
              reached: reply.reached,
              viaFits: reply.viaFits ?? true,
            },
          });
        }
      })
      .catch(() => undefined)
      .finally(() => {
        this.routing = false;
        const next = this.pendingTarget;
        this.pendingTarget = null;
        if (next !== null) {
          this.requestRoute(next);
        }
      });
  }

  /** Asks for a new preview (after the corner mode changed). */
  refreshRoute(): void {
    if (this.state.route !== null && this.pointer !== null) {
      this.requestRoute(this.routeTarget(this.pointer));
    }
  }

  /** Fixes the live path; the route continues from its end. */
  private fixPreview(): RouteState | null {
    const route = this.state.route;
    if (route === null) {
      return null;
    }
    const points = dedupe(route.preview);
    if (points.length < 2) {
      return route;
    }
    const end = points.at(-1) ?? route.start;
    const fixed = {
      ...route,
      fixed: [...route.fixed, { layer: route.layer, points }],
      start: end,
      preview: [end],
    };
    this.update({ route: fixed });
    return fixed;
  }

  /** Places a via at the end of the live path and continues on `layer`. */
  private switchRouteLayer(layer: string): void {
    const route = this.fixPreview();
    if (route === null || layer === route.layer) {
      return;
    }
    if (!route.viaFits) {
      this.update({ hint: 'board.hints.viaBlocked' });
      return;
    }
    this.update({
      layer,
      hint: HINTS.route,
      route: { ...route, layer, vias: [...route.vias, route.start] },
    });
  }

  /** Puts the fixed segments and vias into the board. */
  finishRoute(): void {
    const route = this.state.route;
    if (route === null) {
      return;
    }
    this.update({ route: null });
    if (route.fixed.length === 0 && route.vias.length === 0) {
      return;
    }
    this.run('pcb.addTracks', {
      net: route.net,
      width: Math.round(route.width),
      segments: route.fixed.map((s) => ({ layer: s.layer, points: s.points })),
      vias: route.vias.map((at) => ({
        at,
        diameter: Math.round(route.viaDiameter),
        drill: Math.round(route.viaDrill),
      })),
    });
  }

  private undoSegment(): void {
    const route = this.state.route;
    if (route === null) {
      return;
    }
    const last = route.fixed.at(-1);
    if (last === undefined) {
      this.update({ route: null });
      return;
    }
    const start = last.points[0] ?? route.start;
    const vias = route.vias.filter((v) => v[0] !== route.start[0] || v[1] !== route.start[1]);
    this.update({
      layer: last.layer,
      route: {
        ...route,
        fixed: route.fixed.slice(0, -1),
        start,
        layer: last.layer,
        preview: [start],
        vias,
      },
    });
  }

  // --- Polygons and texts ---------------------------------------------------

  private closePolygon(): void {
    const points = this.state.polygon;
    const tool = this.state.tool;
    this.update({ polygon: [] });
    if (tool === 'outline') {
      if (points.length >= 3) {
        this.insertOutline({ polygon: { points } });
      }
      return;
    }
    if (points.length >= 3 && (tool === 'zone' || tool === 'keepout')) {
      this.host.newZone(points, tool === 'keepout', this.state.layer);
    }
  }

  private insertOutline(geometry: Record<string, unknown>): void {
    this.run('edit.apply', {
      label: 'draw-outline',
      changes: [
        {
          op: 'insert',
          doc: this.host.doc,
          object: {
            type: 'boardShape',
            id: crypto.randomUUID(),
            layer: 'outline',
            geometry,
            width: 100_000,
          },
        },
      ],
    });
  }

  submitPrompt(text: string): void {
    const prompt = this.state.prompt;
    this.update({ prompt: null });
    const value = text.trim();
    if (prompt === null || value === '') {
      return;
    }
    const layer = silkOf(this.state.layer);
    this.run('edit.apply', {
      label: 'add-text',
      changes: [
        {
          op: 'insert',
          doc: this.host.doc,
          object: {
            type: 'boardText',
            id: crypto.randomUUID(),
            layer,
            text: value,
            at: prompt.at,
            style: { size: 1_000_000, thickness: 150_000 },
            mirrored: layer === 'silk.bottom',
          },
        },
      ],
    });
  }

  cancelPrompt(): void {
    this.update({ prompt: null });
  }

  // --- Interaction ----------------------------------------------------------

  pointerDown(point: Point, event: PointerEvent): boolean {
    if (this.state.prompt !== null) {
      this.cancelPrompt();
    }
    const p = this.snapPoint(point);
    if (this.keyMove !== null) {
      this.finishMove(this.keyMove.drag);
      return true;
    }
    switch (this.state.tool) {
      case 'select': {
        if (event.shiftKey || event.ctrlKey || event.metaKey || event.altKey) {
          return false;
        }
        const obj = this.controller.pickAtPoint(point);
        if (obj === null) {
          return false;
        }
        // The hit object itself is selected (a pad shows in the inspector);
        // a drag moves its footprint.
        const root = this.controller.rootOf(obj);
        if (!this.controller.isSelected(obj) && !this.controller.isSelected(root)) {
          this.controller.select([obj]);
        }
        this.press = { start: p, screen: this.controller.screenOf(point), moving: false };
        return true;
      }
      case 'route': {
        const route = this.state.route;
        if (route === null) {
          this.startRoute(point);
          return true;
        }
        const fixed = this.fixPreview();
        // A click on copper of the route's net ends the route.
        const obj = this.controller.pickAtPoint(point);
        const scene = this.controller.currentScene;
        const end = fixed?.start;
        const target = this.routeTarget(point);
        if (
          fixed !== null &&
          obj !== null &&
          scene !== null &&
          route.netName !== '' &&
          scene.netOf(obj) === route.netName &&
          fixed.reached &&
          end?.[0] === target[0] &&
          end[1] === target[1]
        ) {
          this.finishRoute();
        }
        return true;
      }
      case 'via':
        this.host
          .call<StartReply>('pcb.routeStart', {
            doc: this.host.doc,
            at: rounded(point),
            layer: this.state.layer,
          })
          .then((start) => {
            this.run('pcb.addTracks', {
              net: start.net,
              width: Math.round(start.width),
              vias: [
                {
                  at: p,
                  diameter: Math.round(start.viaDiameter),
                  drill: Math.round(start.viaDrill),
                },
              ],
            });
          })
          .catch((error: unknown) => {
            this.host.error(error);
          });
        return true;
      case 'zone':
      case 'keepout':
        this.update({ polygon: [...this.state.polygon, p] });
        return true;
      case 'outline': {
        const polygon = this.state.polygon;
        if (this.state.outlineShape === 'rect') {
          const first = polygon[0];
          if (first === undefined) {
            this.update({ polygon: [p] });
          } else if (first[0] !== p[0] && first[1] !== p[1]) {
            this.update({ polygon: [] });
            this.insertOutline({ rect: { start: first, end: p } });
          }
        } else {
          this.update({ polygon: [...polygon, p] });
        }
        return true;
      }
      case 'text':
        this.update({ prompt: { at: p, value: '' } });
        return true;
      case 'footprint':
        if (this.state.footprint !== null) {
          this.run('pcb.place', {
            footprint: this.state.footprint,
            at: p,
            rotation: this.state.rotation,
            side: this.state.layer === 'cu.bottom' ? 'bottom' : 'top',
          });
        }
        return true;
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
      if (this.state.route !== null) {
        this.requestRoute(this.routeTarget(point));
      }
    }
    const press = this.press;
    if (press !== null) {
      if (!press.moving) {
        const screen = this.controller.screenOf(point);
        if (
          Math.hypot(screen[0] - press.screen[0], screen[1] - press.screen[1]) > DRAG_THRESHOLD_PX
        ) {
          press.moving = true;
          this.beginMove(this.selectedObjects());
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
    if (this.press?.moving === true) {
      this.finishMove(!event.altKey);
    }
    this.press = null;
  }

  doubleClick(point: Point): void {
    switch (this.state.tool) {
      case 'route':
        this.finishRoute();
        return;
      case 'zone':
      case 'keepout':
      case 'outline':
        // The double click added the last point twice.
        this.update({ polygon: dedupe(this.state.polygon) });
        this.closePolygon();
        return;
      case 'select': {
        const obj = this.controller.pickAtPoint(point);
        const scene = this.controller.currentScene;
        if (obj !== null && scene !== null) {
          const root = this.controller.rootOf(obj);
          if (scene.typeOf(root) === 'zone') {
            this.host.editZone(scene.idOf(root));
          }
        }
        return;
      }
      case 'via':
      case 'text':
      case 'footprint':
      case 'highlight':
        return;
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
    if (mod || event.altKey) {
      return mod && (event.key === 'Delete' || event.key === 'Backspace');
    }
    const key = event.key.length === 1 ? event.key.toLowerCase() : event.key;
    switch (key) {
      case 'Escape':
        return this.escape();
      case 'Enter':
        if (this.state.route !== null) {
          this.fixPreview();
          this.finishRoute();
          return true;
        }
        if (this.state.polygon.length > 0) {
          this.closePolygon();
          return true;
        }
        return false;
      case 'Backspace':
        if (this.state.route !== null) {
          this.undoSegment();
          return true;
        }
        if (this.state.polygon.length > 0) {
          this.update({ polygon: this.state.polygon.slice(0, -1) });
          return true;
        }
        this.deleteSelection();
        return true;
      case 'Delete':
        this.deleteSelection();
        return true;
      case 'PageUp':
        this.stepLayer(-1);
        return true;
      case 'PageDown':
        this.stepLayer(1);
        return true;
      case '/':
        if (this.state.tool === 'outline') {
          this.setOutlineShape(this.state.outlineShape === 'rect' ? 'polygon' : 'rect');
        } else {
          this.update({ diagonal: !this.state.diagonal });
          const target = this.pointer === null ? null : this.routeTarget(this.pointer);
          if (target !== null) {
            this.requestRoute(target);
          }
        }
        return true;
      case 'v':
        if (this.state.route !== null) {
          this.switchRouteLayer(otherLayer(this.state.route.layer, this.state.layers));
        } else {
          this.setTool('via');
        }
        return true;
      case 'r':
        if (this.state.tool === 'footprint') {
          this.update({ rotation: (this.state.rotation + (event.shiftKey ? 270 : 90)) % 360 });
        } else {
          this.rotateSelection(event.shiftKey);
        }
        return true;
      case 'f':
        if (event.shiftKey) {
          this.flipSelection();
          return true;
        }
        return false;
      case 'b':
        if (event.shiftKey) {
          this.fillZones(true);
          return true;
        }
        return false;
      case 'l':
        this.lockSelection(!event.shiftKey);
        return true;
      case 'm':
      case 'd':
        this.startKeyMove(key === 'd');
        return true;
      case '`':
        if (this.pointer !== null) {
          this.highlightAt(this.pointer);
        }
        return true;
      default: {
        const tool = this.toolKey(key);
        if (tool === null) {
          return false;
        }
        if (tool === 'footprint') {
          this.host.chooseFootprint();
        } else {
          this.setTool(tool);
        }
        return true;
      }
    }
  }

  private toolKey(key: string): BoardTool | null {
    switch (key) {
      case 'x':
        return 'route';
      case 'z':
        return 'zone';
      case 'k':
        return 'keepout';
      case 'o':
        return 'outline';
      case 't':
        return 'text';
      case 'a':
        return 'footprint';
      case 'n':
        return 'highlight';
      case 's':
        return 'select';
      default:
        return null;
    }
  }

  private escape(): boolean {
    if (this.state.route !== null) {
      // The live path is dropped; fixed segments go into the board.
      this.finishRoute();
      return true;
    }
    if (this.state.move !== null || this.keyMove !== null || this.state.polygon.length > 0) {
      this.cancel();
      return true;
    }
    if (this.state.tool !== 'select') {
      this.setTool('select');
      this.update({ footprint: null });
      return true;
    }
    if (this.state.net !== null) {
      this.showNet(null);
      return true;
    }
    return false;
  }
}
