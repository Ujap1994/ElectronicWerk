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
import { routingCorners, routingMode, routingVia } from './stores';

export type Point = readonly [number, number];

export type BoardTool =
  | 'select'
  | 'route'
  | 'pair'
  | 'via'
  | 'zone'
  | 'keepout'
  | 'outline'
  | 'text'
  | 'footprint'
  | 'highlight';

/** The via type the router places when it changes layers. */
export type ViaChoice = 'through' | 'blind' | 'micro';

/** A track of a preview: what an edit in progress would leave. */
export interface PreviewTrack {
  readonly layer: string;
  readonly width: number;
  readonly start: Point;
  readonly end: Point;
}

/** The reply of `pcb.editPreview`. */
interface EditPreview {
  readonly tracks: readonly PreviewTrack[];
  readonly hidden: readonly string[];
}

/** A via placed while routing. */
export interface RouteVia {
  readonly at: Point;
  readonly from: string;
  readonly to: string;
  readonly micro: boolean;
  readonly diameter: number;
  readonly drill: number;
}

/** A route being drawn. */
/** The other net of a differential pair routed together with the route's net. */
export interface PairState {
  readonly net: string;
  readonly netName: string;
  /** Where the partner begins. */
  readonly start: Point;
  /** Gap between both tracks (edge to edge). */
  readonly gap: number;
  /** The centre line fixed so far. */
  readonly centre: readonly Point[];
  /** The centre line of the live preview. */
  readonly live: readonly Point[];
  /** The partner's track of the live preview (the fixed part included). */
  readonly preview: readonly Point[];
  /** Both tracks of the fixed centre line. */
  readonly fixed: { readonly mine: readonly Point[]; readonly theirs: readonly Point[] };
  /** Earlier fixed states, for Backspace. */
  readonly history: readonly {
    readonly centre: readonly Point[];
    readonly mine: readonly Point[];
    readonly theirs: readonly Point[];
  }[];
  /** The live preview ends on the pads at the target. */
  readonly landed: boolean;
}

export interface RouteState {
  readonly net: string | null;
  readonly netName: string;
  readonly width: number;
  readonly viaDiameter: number;
  readonly viaDrill: number;
  readonly microViaDiameter: number;
  readonly microViaDrill: number;
  /** The via types the design rules allow (`through`, `blind`, `buried`, `micro`). */
  readonly viaTypes: readonly string[];
  /**
   * The track width on each copper layer when the net's class follows an
   * impedance profile: the route changes its width with the layer.
   */
  readonly layerWidths: Readonly<Record<string, number>>;
  /** Direction of the existing track the route started at (bends from it count). */
  readonly startDirection: Point | null;
  /** The layer of the live segment. */
  readonly layer: string;
  /** Start of the live segment. */
  readonly start: Point;
  /** Segments fixed with a click, not yet in the board. */
  readonly fixed: readonly {
    readonly layer: string;
    readonly points: readonly Point[];
    readonly width: number;
  }[];
  /** Vias placed while routing. */
  readonly vias: readonly RouteVia[];
  /** The live path from `start` towards the cursor. */
  readonly preview: readonly Point[];
  readonly reached: boolean;
  readonly viaFits: boolean;
  /** A differential pair: the other net, routed together. */
  readonly pair: PairState | null;
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
  /** Tracks an edit in progress would leave (a drag), drawn over the board. */
  readonly preview: readonly PreviewTrack[] | null;
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
  readonly microViaDiameter?: number;
  readonly microViaDrill?: number;
  readonly viaTypes?: readonly string[];
  readonly previous?: Point | null;
  /** An impedance profile's track width on every copper layer. */
  readonly layerWidths?: Readonly<Record<string, number>>;
  readonly pairWidth?: number;
  readonly pairGap?: number;
  /** With `pair`: the other net of the differential pair and where it begins. */
  readonly partner?: { readonly net: string; readonly netName: string; readonly at: Point } | null;
}

interface PairReply {
  readonly centre: readonly Point[];
  readonly points: readonly Point[];
  readonly partner: readonly Point[];
  readonly reached: boolean;
  readonly landed: boolean;
}

interface RouteReply {
  readonly points: readonly Point[];
  readonly reached: boolean;
  readonly viaFits?: boolean;
  /** In push-and-shove mode: the tracks pushed aside, or `null` if they cannot make room. */
  readonly shove?: EditPreview | null;
}

/** How a route deals with the tracks of other nets. */
export type RouteMode = 'walkaround' | 'shove';

const DRAG_THRESHOLD_PX = 4;

const HINTS: Readonly<Record<BoardTool, MessageKey>> = {
  select: 'board.hints.select',
  route: 'board.hints.route',
  pair: 'board.hints.pair',
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

/**
 * The via that takes a route from one copper layer to another with the
 * chosen via type: its first and last layer and whether it is a microvia,
 * or why the rules do not allow it.
 */
export function viaBetween(
  from: string,
  to: string,
  layers: readonly string[],
  choice: ViaChoice,
  allowed: readonly string[],
): { from: string; to: string; micro: boolean } | 'notAllowed' | 'notNeighbours' {
  const first = layers[0] ?? 'cu.top';
  const last = layers.at(-1) ?? 'cu.bottom';
  const a = layers.indexOf(from);
  const b = layers.indexOf(to);
  const [low, high] = a <= b ? [from, to] : [to, from];
  const through = { from: first, to: last, micro: false };
  if (choice === 'through' || a < 0 || b < 0) {
    return through;
  }
  if (choice === 'micro') {
    if (Math.abs(a - b) !== 1) {
      return 'notNeighbours';
    }
    return allowed.includes('micro') ? { from: low, to: high, micro: true } : 'notAllowed';
  }
  if (low === first && high === last) {
    return through;
  }
  return allowed.includes('blind') ? { from: low, to: high, micro: false } : 'notAllowed';
}

/** The type of a via by its layers, as the engine names it. */
export function viaTypeOf(
  from: string,
  to: string,
  micro: boolean,
): 'through' | 'blind' | 'buried' | 'micro' {
  if (micro) {
    return 'micro';
  }
  const outer = (layer: string): boolean => !layer.startsWith('cu.in');
  if (outer(from) && outer(to) && from !== to) {
    return 'through';
  }
  return !outer(from) && !outer(to) ? 'buried' : 'blind';
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
    preview: null,
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
  /** The track a drag moves (its neighbours follow), or `null` for a move. */
  private dragId: string | null = null;
  /** Drag previews: one request at a time, the latest offset wins. */
  private previewing = false;
  private pendingDelta: Point | null = null;
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
    this.dragId = null;
    this.update({ move: null, preview: null, polygon: [], prompt: null });
  }

  // --- Engine calls ---------------------------------------------------------

  /** Runs an edit; `null` when it failed (or another one is running). */
  private async edit(method: string, params: Record<string, unknown>): Promise<EditResult | null> {
    if (this.busy) {
      return null;
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
      return result;
    } catch (error) {
      this.host.error(error);
      return null;
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
    this.beginMove(ids, drag);
  }

  private beginMove(ids: string[], drag: boolean): void {
    this.moveIds = ids;
    const types = this.controller.snapshot().selection.map((s) => s.type);
    // A single track is dragged: its neighbours follow, drawn as a preview.
    this.dragId =
      drag && ids.length === 1 && types.length === 1 && types[0] === 'track'
        ? (ids[0] ?? null)
        : null;
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
      this.update({ move: { delta } });
      if (this.dragId === null) {
        this.controller.setMoveOffset(delta);
      } else {
        this.requestDragPreview(delta);
      }
    }
  }

  /** Asks the engine what a drag by `delta` would leave. */
  private requestDragPreview(delta: Point): void {
    const id = this.dragId;
    if (id === null) {
      return;
    }
    if (this.previewing) {
      this.pendingDelta = delta;
      return;
    }
    this.previewing = true;
    this.host
      .call<EditPreview>('pcb.editPreview', {
        doc: this.host.doc,
        op: 'dragTrack',
        id,
        offset: rounded(delta),
      })
      .then((reply) => {
        if (this.dragId !== id || this.state.move === null) {
          return;
        }
        // The dragged track stays hidden even when the drag changes nothing.
        const hidden = reply.hidden.length > 0 ? reply.hidden : [];
        this.controller.setMoveOffset([0, 0], hidden);
        this.update({ preview: reply.tracks.length > 0 ? reply.tracks : null });
      })
      .catch(() => undefined)
      .finally(() => {
        this.previewing = false;
        const next = this.pendingDelta;
        this.pendingDelta = null;
        if (next !== null) {
          this.requestDragPreview(next);
        }
      });
  }

  private finishMove(drag: boolean): void {
    const move = this.state.move;
    const dragged = this.dragId;
    this.press = null;
    this.keyMove = null;
    this.dragId = null;
    this.pendingDelta = null;
    if (move === null) {
      return;
    }
    if (move.delta[0] === 0 && move.delta[1] === 0) {
      this.controller.setMoveOffset(null);
      this.update({ move: null, preview: null });
      return;
    }
    this.update({ move: null });
    const ids = this.moveIds;
    const request =
      drag && dragged !== null
        ? this.edit('pcb.edit', { op: 'dragTrack', id: dragged, offset: rounded(move.delta) })
        : this.edit('pcb.edit', { op: 'move', ids, offset: move.delta });
    void request.then((result) => {
      // The board reloads with the result and ends the move; without a
      // change nothing reloads, so the move ends here.
      this.update({ preview: null });
      if ((result?.seq ?? null) === null) {
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
    const pair = this.state.tool === 'pair';
    this.host
      .call<StartReply>('pcb.routeStart', {
        doc: this.host.doc,
        at: rounded(point),
        layer,
        ...(pair ? { pair: true } : {}),
      })
      .then((start) => {
        const at = rounded(start.at);
        const partner = start.partner ?? null;
        if (pair && (partner === null || start.net === null)) {
          this.update({ hint: 'board.hints.noPartner' });
          return;
        }
        const pairWidth = start.pairWidth ?? start.width;
        this.update({
          hint: HINTS[this.state.tool],
          route: {
            net: start.net,
            netName: start.netName,
            width: pair ? pairWidth : start.width,
            viaDiameter: start.viaDiameter,
            viaDrill: start.viaDrill,
            microViaDiameter: start.microViaDiameter ?? start.viaDiameter,
            microViaDrill: start.microViaDrill ?? start.viaDrill,
            viaTypes: start.viaTypes ?? ['through'],
            layerWidths: start.layerWidths ?? {},
            startDirection: start.previous ?? null,
            layer,
            start: at,
            fixed: [],
            vias: [],
            preview: [at],
            reached: true,
            viaFits: true,
            pair:
              pair && partner !== null
                ? {
                    net: partner.net,
                    netName: partner.netName,
                    start: rounded(partner.at),
                    gap: start.pairGap ?? pairWidth,
                    centre: [],
                    live: [],
                    preview: [rounded(partner.at)],
                    fixed: { mine: [], theirs: [] },
                    history: [],
                    landed: false,
                  }
                : null,
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
      const net = scene.netOf(obj);
      const ours = net === route.netName || (route.pair !== null && net === route.pair.netName);
      if ((type === 'pad' || type === 'via') && ours) {
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
    if (route.vias.some((via) => via.at[0] === at[0] && via.at[1] === at[1])) {
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
    if (route.pair !== null) {
      this.requestPairRoute(route, route.pair, target);
      return;
    }
    const asked = route;
    const shove = routingMode.get() === 'shove';
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
        ...(shove
          ? {
              mode: 'shove',
              fixed: route.fixed.map((s) => ({
                layer: s.layer,
                points: s.points,
                width: Math.round(s.width),
              })),
              vias: this.routeVias(route),
            }
          : {}),
      })
      .then((reply) => {
        const current = this.state.route;
        if (current !== null && current.start === asked.start && current.layer === asked.layer) {
          // Pushed tracks are drawn over the board in place of the originals.
          const pushed = reply.shove ?? null;
          this.controller.setPreviewHidden(pushed?.hidden ?? []);
          this.update({
            preview: pushed !== null && pushed.tracks.length > 0 ? pushed.tracks : null,
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

  /** The preview of a differential pair: both tracks routed together. */
  private requestPairRoute(route: RouteState, pair: PairState, target: Point): void {
    this.host
      .call<PairReply>('pcb.routePair', {
        doc: this.host.doc,
        layer: route.layer,
        net: route.net,
        partner: pair.net,
        width: route.width,
        gap: pair.gap,
        start: route.start,
        partnerStart: pair.start,
        centre: pair.centre,
        target,
        diagonalFirst: this.state.diagonal,
        corners: routingCorners.get(),
      })
      .then((reply) => {
        const current = this.state.route;
        if (current?.pair === undefined || current.pair === null) {
          return;
        }
        if (current.pair.centre !== pair.centre) {
          return;
        }
        this.update({
          route: {
            ...current,
            preview: dedupe(reply.points.map(rounded)),
            reached: reply.reached,
            pair: {
              ...current.pair,
              live: reply.centre.map(rounded),
              preview: dedupe(reply.partner.map(rounded)),
              landed: reply.landed,
            },
          },
        });
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
    const pair = route.pair;
    if (pair !== null) {
      if (pair.live.length <= pair.centre.length || route.preview.length < 2) {
        return route;
      }
      const fixed: RouteState = {
        ...route,
        pair: {
          ...pair,
          history: [...pair.history, { centre: pair.centre, ...pair.fixed }],
          centre: pair.live,
          fixed: { mine: route.preview, theirs: pair.preview },
        },
      };
      this.update({ route: fixed });
      return fixed;
    }
    const points = dedupe(route.preview);
    if (points.length < 2) {
      return route;
    }
    const end = points.at(-1) ?? route.start;
    const fixed = {
      ...route,
      fixed: [...route.fixed, { layer: route.layer, points, width: route.width }],
      start: end,
      preview: [end],
    };
    this.update({ route: fixed });
    return fixed;
  }

  /**
   * Places a via of the chosen type at the end of the live path and
   * continues on `layer`.
   */
  private switchRouteLayer(layer: string): void {
    const current = this.state.route;
    if (current !== null && current.pair !== null) {
      this.update({ hint: 'board.hints.pairLayer' });
      return;
    }
    const route = this.fixPreview();
    if (route === null || layer === route.layer) {
      return;
    }
    const via = viaBetween(route.layer, layer, this.state.layers, routingVia.get(), route.viaTypes);
    if (via === 'notAllowed') {
      this.update({ hint: 'board.hints.viaNotAllowed' });
      return;
    }
    if (via === 'notNeighbours') {
      this.update({ hint: 'board.hints.microNeighbours' });
      return;
    }
    const diameter = via.micro ? route.microViaDiameter : route.viaDiameter;
    const drill = via.micro ? route.microViaDrill : route.viaDrill;
    const place = (): void => {
      this.update({
        layer,
        hint: HINTS.route,
        route: {
          ...route,
          layer,
          width: route.layerWidths[layer] ?? route.width,
          vias: [...route.vias, { at: route.start, ...via, diameter, drill }],
        },
      });
    };
    const through = via.from === this.state.layers[0] && via.to === this.state.layers.at(-1);
    if (through && !via.micro) {
      if (route.viaFits) {
        place();
      } else {
        this.update({ hint: 'board.hints.viaBlocked' });
      }
      return;
    }
    // A blind, buried or microvia only needs room on its own layers.
    this.host
      .call<{ fits: boolean }>('pcb.viaFits', {
        doc: this.host.doc,
        net: route.net,
        at: route.start,
        diameter,
        layers: [via.from, via.to],
      })
      .then(({ fits }) => {
        if (this.state.route !== route) {
          return;
        }
        if (fits) {
          place();
        } else {
          this.update({ hint: 'board.hints.viaBlocked' });
        }
      })
      .catch((error: unknown) => {
        this.host.error(error);
      });
  }

  /** The vias of a route, as the engine takes them. */
  private routeVias(route: RouteState): Record<string, unknown>[] {
    return route.vias.map((via) => ({
      at: via.at,
      diameter: Math.round(via.diameter),
      drill: Math.round(via.drill),
      from: via.from,
      to: via.to,
      micro: via.micro,
    }));
  }

  /**
   * Puts the fixed segments and vias into the board; in push-and-shove
   * mode the tracks they push move aside in the same step.
   */
  finishRoute(): void {
    const route = this.state.route;
    if (route === null) {
      return;
    }
    // Still in the routing tool: say how to get back to the select tool.
    const routing = this.state.tool === 'route' || this.state.tool === 'pair';
    this.update({
      route: null,
      preview: null,
      ...(routing ? { hint: 'board.hints.routeEnded' as const } : {}),
    });
    const pair = route.pair;
    if (pair !== null) {
      if (pair.fixed.mine.length >= 2 && pair.fixed.theirs.length >= 2) {
        void this.edit('pcb.addTracks', {
          net: route.net,
          width: Math.round(route.width),
          segments: [{ layer: route.layer, points: pair.fixed.mine }],
          pair: { net: pair.net, segments: [{ layer: route.layer, points: pair.fixed.theirs }] },
        });
      }
      return;
    }
    if (route.fixed.length === 0 && route.vias.length === 0) {
      this.controller.setPreviewHidden([]);
      return;
    }
    const shove = routingMode.get() === 'shove';
    void this.edit('pcb.addTracks', {
      net: route.net,
      width: Math.round(route.width),
      segments: route.fixed.map((s) => ({
        layer: s.layer,
        points: s.points,
        width: Math.round(s.width),
      })),
      vias: this.routeVias(route),
      ...(shove ? { mode: 'shove', corners: routingCorners.get() } : {}),
    }).then((result) => {
      // The board reloads and shows the pushed tracks; without a change
      // the originals come back.
      if (result?.seq === undefined || result.seq === null) {
        this.controller.setPreviewHidden([]);
      }
    });
  }

  private undoSegment(): void {
    const route = this.state.route;
    if (route === null) {
      return;
    }
    const pair = route.pair;
    if (pair !== null) {
      const last = pair.history.at(-1);
      if (last === undefined) {
        this.update({ route: null, preview: null });
        return;
      }
      this.update({
        route: {
          ...route,
          preview: last.mine.length > 0 ? last.mine : [route.start],
          pair: {
            ...pair,
            history: pair.history.slice(0, -1),
            centre: last.centre,
            live: last.centre,
            fixed: { mine: last.mine, theirs: last.theirs },
            preview: last.theirs.length > 0 ? last.theirs : [pair.start],
            landed: false,
          },
        },
      });
      if (this.pointer !== null) {
        this.requestRoute(this.routeTarget(this.pointer));
      }
      return;
    }
    const last = route.fixed.at(-1);
    if (last === undefined) {
      this.controller.setPreviewHidden([]);
      this.update({ route: null, preview: null });
      return;
    }
    const start = last.points[0] ?? route.start;
    const vias = route.vias.filter((v) => v.at[0] !== route.start[0] || v.at[1] !== route.start[1]);
    this.update({
      layer: last.layer,
      route: {
        ...route,
        fixed: route.fixed.slice(0, -1),
        start,
        layer: last.layer,
        width: last.width,
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
      case 'route':
      case 'pair': {
        const route = this.state.route;
        if (route === null) {
          this.startRoute(point);
          return true;
        }
        const fixed = this.fixPreview();
        // A pair that ends on the pads at the target is complete.
        if (fixed?.pair?.landed === true) {
          this.finishRoute();
          return true;
        }
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
          this.beginMove(this.selectedObjects(), true);
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
      case 'pair':
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
      case '6':
        return 'pair';
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
