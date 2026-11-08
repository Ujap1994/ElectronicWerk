/**
 * The viewer controller: owns camera, scene, layer settings, selection and
 * input handling, and renders on demand. React components observe it
 * through `subscribe`/`snapshot` (layers, selection) and `subscribeView`
 * (cursor, zoom, overlay), so pointer moves never re-render the panels.
 */
import { Camera, gridStep, interpolate, type CameraState } from './camera';
import { defaultOpacity, drawOrder, layerColor, palette, rgb, type CanvasTheme } from './layers';
import { boundsOf, pick, selectBox, selectLasso } from './pick';
import { SceneRenderer, WebGlUnavailableError, type LayerAppearance } from './renderer';
import { Scene } from './scene';
import type { Box, LayerGroup, SceneReply } from './types';

export interface LayerUi {
  readonly index: number;
  readonly key: string;
  readonly group: LayerGroup;
  readonly side: 'top' | 'bottom' | null;
  readonly name: string | null;
  readonly color: string;
  readonly visible: boolean;
  readonly opacity: number;
  readonly empty: boolean;
}

export type SelectTool = 'box' | 'lasso';

/** A selected object, as needed by the inspector. */
export interface SelectedObject {
  readonly key: string;
  readonly id: string;
  readonly type: string;
  readonly child: { readonly kind: string; readonly index: number } | null;
}

/** View settings of a document, kept while its tab is closed or hidden. */
export interface ViewMemory {
  readonly camera: CameraState;
  readonly layers: readonly (readonly [string, { visible: boolean; opacity: number }])[];
  readonly active: string | null;
  readonly highContrast: boolean;
  readonly tool: SelectTool;
}

export interface ViewerSnapshot {
  readonly status: 'empty' | 'ready' | 'unavailable';
  /** Increases with every new scene (also reloads of the same document). */
  readonly version: number;
  readonly kind: 'pcb' | 'schematic' | null;
  readonly layers: readonly LayerUi[];
  readonly active: number | null;
  readonly highContrast: boolean;
  readonly flipped: boolean;
  readonly tool: SelectTool;
  readonly selection: readonly SelectedObject[];
}

export type Overlay =
  | {
      readonly kind: 'box';
      readonly x0: number;
      readonly y0: number;
      readonly x1: number;
      readonly y1: number;
      readonly crossing: boolean;
    }
  | { readonly kind: 'lasso'; readonly points: readonly number[] };

export interface ViewState {
  /** Cursor in absolute scene coordinates (nm), or `null`. */
  readonly cursor: readonly [number, number] | null;
  /** CSS pixels per nanometre. */
  readonly scale: number;
  /** Grid spacing on screen (nm). */
  readonly grid: number;
  readonly overlay: Overlay | null;
  /** Object under the cursor. */
  readonly hover: { readonly type: string; readonly net: string | null } | null;
}

interface LayerSetting {
  visible: boolean;
  opacity: number;
}

type Drag =
  | { kind: 'pan'; x: number; y: number }
  | { kind: 'pending'; x: number; y: number; pointer: number }
  | { kind: 'box'; x: number; y: number; pointer: number }
  | { kind: 'lasso'; points: number[]; pointer: number };

const WHEEL_ZOOM = 1.25;
const KEY_ZOOM = 1.5;
const DRAG_THRESHOLD = 4;
const HIT_TOLERANCE_PX = 4;
const HIGH_CONTRAST_DIM = 0.22;

const SELECTED = 1;
const HOVERED = 2;

export class ViewerController {
  readonly camera = new Camera();
  private renderer: SceneRenderer | null = null;
  private canvas: HTMLCanvasElement | null = null;
  private scene: Scene | null = null;
  private dpr = 1;
  private theme: CanvasTheme = 'light';
  private settings: LayerSetting[] = [];
  private settingsByKey = new Map<string, LayerSetting>();
  private active: number | null = null;
  private highContrast = false;
  private tool: SelectTool = 'box';
  private selected = new Set<number>();
  private hovered: number | null = null;
  private flags = new Uint8Array(0);
  private status: ViewerSnapshot['status'] = 'empty';
  private version = 0;
  private restoredCamera = false;
  /** The view shows the whole document and follows size changes. */
  private autoFit = true;
  private snapshotCache: ViewerSnapshot | null = null;
  private viewCache: ViewState | null = null;
  private readonly listeners = new Set<() => void>();
  private readonly viewListeners = new Set<() => void>();
  private frame = 0;
  private cursor: [number, number] | null = null;
  private pointer: [number, number] | null = null;
  private hoverDirty = false;
  private drag: Drag | null = null;
  private overlay: ViewState['overlay'] = null;
  private zoomAnimation: { x: number; y: number; target: number } | null = null;
  private fitAnimation: { from: CameraState; to: CameraState; start: number } | null = null;
  private spaceDown = false;
  private readonly cleanup: (() => void)[] = [];
  /** Duration of the last frame on the CPU (ms). */
  lastFrameMs = 0;

  // --- Observation ----------------------------------------------------------

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  subscribeView = (listener: () => void): (() => void) => {
    this.viewListeners.add(listener);
    return () => this.viewListeners.delete(listener);
  };

  snapshot = (): ViewerSnapshot => {
    this.snapshotCache ??= this.buildSnapshot();
    return this.snapshotCache;
  };

  view = (): ViewState => {
    this.viewCache ??= {
      cursor:
        this.cursor === null || this.scene === null
          ? null
          : [
              this.cursor[0] + this.scene.meta.origin[0],
              this.cursor[1] + this.scene.meta.origin[1],
            ],
      scale: this.camera.scale,
      grid: this.gridStep(),
      overlay: this.overlay,
      hover:
        this.hovered === null || this.scene === null
          ? null
          : { type: this.scene.typeOf(this.hovered), net: this.scene.netOf(this.hovered) },
    };
    return this.viewCache;
  };

  private changed(): void {
    this.snapshotCache = null;
    for (const listener of this.listeners) {
      listener();
    }
  }

  private viewChanged(): void {
    this.viewCache = null;
    for (const listener of this.viewListeners) {
      listener();
    }
  }

  private buildSnapshot(): ViewerSnapshot {
    const scene = this.scene;
    const layers: LayerUi[] =
      scene === null
        ? []
        : scene.layers.map((layer, index) => {
            const setting = this.settings[index] ?? { visible: true, opacity: 1 };
            const meta = layer.meta;
            return {
              index,
              key: meta.key,
              group: meta.group,
              side: meta.side,
              name: meta.name,
              color: layerColor(meta, this.theme),
              visible: setting.visible,
              opacity: setting.opacity,
              empty:
                meta.segments.count +
                  meta.arcs.count +
                  meta.circles.count +
                  meta.rects.count +
                  meta.mesh.indexCount +
                  meta.texts.length ===
                0,
            };
          });
    const selection: SelectedObject[] =
      scene === null
        ? []
        : [...this.selected].map((obj) => {
            const parent = scene.parentOf(obj);
            const child = scene.children[obj] ?? 0xffff_ffff;
            return {
              key: scene.keyOf(obj),
              id: scene.idOf(obj),
              type: scene.typeOf(obj),
              child:
                parent === null || child === 0xffff_ffff
                  ? null
                  : { kind: scene.typeOf(obj), index: child },
            };
          });
    return {
      status: this.status,
      version: this.version,
      kind: scene?.meta.kind ?? null,
      layers,
      active: this.active,
      highContrast: this.highContrast,
      flipped: this.camera.flipped,
      tool: this.tool,
      selection,
    };
  }

  // --- Lifecycle ------------------------------------------------------------

  /** Attaches to a canvas; returns `false` if WebGL 2 is unavailable. */
  attach(canvas: HTMLCanvasElement): boolean {
    this.detach();
    this.canvas = canvas;
    try {
      this.renderer = new SceneRenderer(canvas);
    } catch (error) {
      if (error instanceof WebGlUnavailableError) {
        this.status = 'unavailable';
        this.changed();
        return false;
      }
      throw error;
    }
    if (this.scene !== null) {
      this.renderer.setScene(this.scene);
      this.updateFlags();
    }
    this.listen(canvas);
    this.requestRender();
    return true;
  }

  detach(): void {
    for (const off of this.cleanup.splice(0)) {
      off();
    }
    if (this.frame !== 0) {
      cancelAnimationFrame(this.frame);
      this.frame = 0;
    }
    this.renderer?.dispose();
    this.renderer = null;
    this.canvas = null;
  }

  get rendererDescription(): string {
    return this.renderer?.description ?? '';
  }

  get samples(): number {
    return this.renderer?.samples ?? 0;
  }

  resize(width: number, height: number, dpr: number): void {
    const first = this.camera.width <= 1;
    this.camera.setViewport(width, height);
    this.dpr = dpr;
    if (this.canvas !== null) {
      this.canvas.width = Math.round(width * dpr);
      this.canvas.height = Math.round(height * dpr);
    }
    if (this.scene !== null) {
      this.camera.limitTo(this.homeBox());
    }
    if ((first || this.autoFit) && this.scene !== null && !this.restoredCamera) {
      this.camera.set({ ...this.camera.fitState(this.homeBox()), flipped: this.camera.flipped });
    }
    this.requestRender();
    this.viewChanged();
  }

  setTheme(theme: CanvasTheme): void {
    if (theme !== this.theme) {
      this.theme = theme;
      this.changed();
      this.requestRender();
    }
  }

  /**
   * Shows a new scene. Layer settings and the selection are kept when the
   * same document is reloaded; otherwise the view fits the document.
   */
  setScene(reply: SceneReply, memory: ViewMemory | null = null): void {
    const previous = this.scene;
    const sameDoc = previous?.meta.doc === reply.doc;
    const restore = !sameDoc && memory !== null;
    if (restore) {
      this.settingsByKey = new Map(memory.layers.map(([key, setting]) => [key, { ...setting }]));
      this.highContrast = memory.highContrast;
      this.tool = memory.tool;
    }
    const selectedKeys = new Set(
      previous === null ? [] : [...this.selected].map((obj) => previous.keyOf(obj)),
    );
    const scene = new Scene(reply);
    this.scene = scene;
    this.status = 'ready';
    this.version += 1;
    this.settings = scene.meta.layers.map((layer) => {
      const kept = sameDoc || restore ? this.settingsByKey.get(layer.key) : undefined;
      const setting = kept ?? { visible: layer.visible, opacity: defaultOpacity(layer) };
      this.settingsByKey.set(layer.key, setting);
      return setting;
    });
    if (!sameDoc) {
      this.active =
        scene.meta.kind === 'pcb' ? scene.meta.layers.findIndex((l) => l.key === 'cu.top') : null;
      if (restore) {
        this.active = scene.meta.layers.findIndex((l) => l.key === memory.active);
      }
      if (this.active === -1) {
        this.active = null;
      }
      this.selected = new Set();
    } else {
      this.selected = new Set();
      for (let obj = 0; obj < scene.objectCount && selectedKeys.size > 0; obj += 1) {
        if (selectedKeys.has(scene.keyOf(obj))) {
          this.selected.add(obj);
        }
      }
    }
    this.hovered = null;
    if (this.camera.width > 1) {
      this.camera.limitTo(this.homeBox());
    }
    this.renderer?.setScene(scene);
    this.updateFlags();
    if (restore) {
      this.camera.set(memory.camera);
      this.restoredCamera = true;
      this.autoFit = false;
    } else if (!sameDoc) {
      this.autoFit = true;
      if (this.camera.width > 1) {
        this.camera.set({ ...this.camera.fitState(this.homeBox()), flipped: false });
      }
    }
    this.changed();
    this.viewChanged();
    this.requestRender();
  }

  /** The current view settings, to restore them later. */
  exportMemory(): ViewMemory | null {
    const scene = this.scene;
    if (scene === null) {
      return null;
    }
    return {
      camera: this.camera.state,
      layers: scene.meta.layers.map((layer, index) => [
        layer.key,
        { ...(this.settings[index] ?? { visible: true, opacity: 1 }) },
      ]),
      active: this.active === null ? null : (scene.meta.layers[this.active]?.key ?? null),
      highContrast: this.highContrast,
      tool: this.tool,
    };
  }

  get currentScene(): Scene | null {
    return this.scene;
  }

  private homeBox(): Box {
    const meta = this.scene?.meta;
    const box = meta?.home ?? meta?.bounds ?? [-50e6, -50e6, 50e6, 50e6];
    const [ox, oy] = meta?.origin ?? [0, 0];
    return [box[0] - ox, box[1] - oy, box[2] - ox, box[3] - oy];
  }

  // --- Commands -------------------------------------------------------------

  fitAll(animate = true): void {
    this.animateTo(this.camera.fitState(this.homeBox()), animate);
    this.autoFit = true;
    this.restoredCamera = false;
  }

  /** The user moved the view: stop following size changes. */
  private navigated(): void {
    this.autoFit = false;
  }

  fitSelection(): void {
    this.navigated();
    const scene = this.scene;
    const box = scene === null ? null : boundsOf(scene, this.selected);
    if (box === null) {
      this.fitAll();
      return;
    }
    const grow = Math.max(box[2] - box[0], box[3] - box[1]) * 0.25 + 1e5;
    this.animateTo(
      this.camera.fitState([box[0] - grow, box[1] - grow, box[2] + grow, box[3] + grow]),
      true,
    );
  }

  zoomBy(factor: number): void {
    this.zoomAround(this.camera.width / 2, this.camera.height / 2, factor);
  }

  /**
   * Views the board from the other side. An active outer copper layer
   * follows to the side that now faces the viewer.
   */
  toggleFlip(): void {
    const scene = this.scene;
    if (scene?.meta.kind !== 'pcb') {
      return;
    }
    this.camera.flip();
    const key = this.active === null ? null : scene.meta.layers[this.active]?.key;
    const other = key === 'cu.top' ? 'cu.bottom' : key === 'cu.bottom' ? 'cu.top' : null;
    const index = other === null ? -1 : scene.meta.layers.findIndex((layer) => layer.key === other);
    if (index >= 0) {
      this.active = index;
      const setting = this.settings[index];
      if (setting !== undefined) {
        setting.visible = true;
      }
    }
    this.changed();
    this.requestRender();
  }

  setTool(tool: SelectTool): void {
    this.tool = tool;
    this.changed();
  }

  setLayerVisible(index: number, visible: boolean): void {
    const setting = this.settings[index];
    if (setting !== undefined) {
      setting.visible = visible;
      this.layersChanged();
    }
  }

  setLayerOpacity(index: number, opacity: number): void {
    const setting = this.settings[index];
    if (setting !== undefined) {
      setting.opacity = Math.min(1, Math.max(0.05, opacity));
      this.layersChanged();
    }
  }

  /** Shows every layer (`true`) or only the active one (`false`). */
  setAllVisible(visible: boolean): void {
    this.settings.forEach((setting, index) => {
      const group = this.scene?.layers[index]?.meta.group;
      setting.visible =
        visible || index === this.active || group === 'outline' || group === 'holes';
    });
    this.layersChanged();
  }

  setActive(index: number | null): void {
    this.active = index;
    const setting = index === null ? undefined : this.settings[index];
    if (setting !== undefined) {
      setting.visible = true;
    }
    this.layersChanged();
  }

  setHighContrast(on: boolean): void {
    this.highContrast = on;
    this.layersChanged();
  }

  private layersChanged(): void {
    this.changed();
    this.requestRender();
  }

  clearSelection(): void {
    if (this.selected.size > 0) {
      this.selected = new Set();
      this.selectionChanged();
    }
  }

  selectAll(): void {
    const scene = this.scene;
    if (scene === null) {
      return;
    }
    const all = new Set<number>();
    for (let obj = 0; obj < scene.objectCount; obj += 1) {
      if (scene.parentOf(obj) === null && scene.isVisible(obj, this.isLayerVisible)) {
        all.add(obj);
      }
    }
    this.selected = all;
    this.selectionChanged();
  }

  private selectionChanged(): void {
    this.updateFlags();
    this.changed();
    this.requestRender();
  }

  // --- Rendering ------------------------------------------------------------

  private isLayerVisible = (layer: number): boolean => this.settings[layer]?.visible ?? false;

  private updateFlags(): void {
    const scene = this.scene;
    if (scene === null) {
      return;
    }
    if (this.flags.length !== scene.objectCount) {
      this.flags = new Uint8Array(scene.objectCount);
    } else {
      this.flags.fill(0);
    }
    const mark = (obj: number, flag: number): void => {
      this.flags[obj] = (this.flags[obj] ?? 0) | flag;
      for (const child of scene.childrenOf(obj)) {
        mark(child, flag);
      }
    };
    for (const obj of this.selected) {
      mark(obj, SELECTED);
    }
    if (this.hovered !== null) {
      mark(this.hovered, HOVERED);
    }
    this.renderer?.setState(this.flags);
  }

  requestRender(): void {
    if (this.frame === 0 && this.renderer !== null) {
      this.frame = requestAnimationFrame(this.onFrame);
    }
  }

  private gridStep(): number {
    const base = this.scene?.meta.grid ?? 0;
    return gridStep(base, this.camera.scale);
  }

  private onFrame = (time: number): void => {
    this.frame = 0;
    const animating = this.stepAnimations(time);
    if (this.hoverDirty) {
      this.hoverDirty = false;
      this.updateHover();
    }
    const renderer = this.renderer;
    const scene = this.scene;
    if (renderer === null) {
      return;
    }
    const started = performance.now();
    const kind = scene?.meta.kind ?? 'pcb';
    const order =
      scene === null ? [] : drawOrder(scene.meta.layers, this.active, this.camera.flipped);
    const appearance: LayerAppearance[] =
      scene === null
        ? []
        : scene.layers.map((layer, index) => {
            const setting = this.settings[index] ?? { visible: true, opacity: 1 };
            const dim =
              this.highContrast &&
              index !== this.active &&
              layer.meta.group !== 'holes' &&
              layer.meta.key !== 'sch.page';
            return {
              visible: setting.visible,
              opacity: setting.opacity * (dim ? HIGH_CONTRAST_DIM : 1),
              color: rgb(layerColor(layer.meta, this.theme)),
            };
          });
    renderer.render({
      camera: this.camera,
      dpr: this.dpr,
      order,
      appearance,
      palette: palette(kind, this.theme),
      gridStep: this.gridStep(),
    });
    this.lastFrameMs = performance.now() - started;
    if (this.canvas !== null) {
      // Exposed for the performance tests (no React render involved).
      this.canvas.dataset['frameMs'] = this.lastFrameMs.toFixed(2);
      this.canvas.dataset['frames'] = String(Number(this.canvas.dataset['frames'] ?? '0') + 1);
      const origin = scene?.meta.origin ?? [0, 0];
      const c = this.camera;
      this.canvas.dataset['view'] = [
        c.cx + origin[0],
        c.cy + origin[1],
        c.scale,
        c.flipped ? 1 : 0,
      ].join(',');
    }
    if (animating) {
      this.viewChanged();
      this.requestRender();
    }
  };

  private stepAnimations(time: number): boolean {
    let animating = false;
    if (this.fitAnimation !== null) {
      const { from, to, start } = this.fitAnimation;
      const t = Math.min(1, (time - start) / 200);
      this.camera.set(interpolate(from, to, t));
      if (t >= 1) {
        this.fitAnimation = null;
      } else {
        animating = true;
      }
    }
    if (this.zoomAnimation !== null) {
      const { x, y, target } = this.zoomAnimation;
      const current = this.camera.scale;
      const next = current * (target / current) ** 0.38;
      this.camera.zoomAt(x, y, next / current);
      if (Math.abs(Math.log(target / this.camera.scale)) < 0.003) {
        this.camera.zoomAt(x, y, target / this.camera.scale);
        this.zoomAnimation = null;
      }
      animating = true;
    }
    return animating;
  }

  private animateTo(state: CameraState, animate: boolean): void {
    this.zoomAnimation = null;
    if (!animate) {
      this.fitAnimation = null;
      this.camera.set(state);
      this.viewChanged();
      this.requestRender();
      return;
    }
    this.fitAnimation = { from: this.camera.state, to: state, start: performance.now() };
    this.requestRender();
  }

  private zoomAround(x: number, y: number, factor: number): void {
    this.navigated();
    this.fitAnimation = null;
    const base = this.zoomAnimation?.target ?? this.camera.scale;
    this.zoomAnimation = { x, y, target: this.camera.clamp(base * factor) };
    this.requestRender();
  }

  // --- Input ----------------------------------------------------------------

  private listen(canvas: HTMLCanvasElement): void {
    const on = <K extends keyof HTMLElementEventMap>(
      type: K,
      handler: (event: HTMLElementEventMap[K]) => void,
      options?: AddEventListenerOptions,
    ): void => {
      canvas.addEventListener(type, handler, options);
      this.cleanup.push(() => {
        canvas.removeEventListener(type, handler, options);
      });
    };
    on('pointerdown', this.onPointerDown);
    on('pointermove', this.onPointerMove);
    on('pointerup', this.onPointerUp);
    on('pointercancel', this.onPointerCancel);
    on('pointerleave', this.onPointerLeave);
    on('wheel', this.onWheel, { passive: false });
    on('keydown', this.onKeyDown);
    on('keyup', this.onKeyUp);
    on('contextmenu', (event) => {
      event.preventDefault();
    });
  }

  private local(event: MouseEvent): [number, number] {
    const rect = this.canvas?.getBoundingClientRect();
    return [event.clientX - (rect?.left ?? 0), event.clientY - (rect?.top ?? 0)];
  }

  private onPointerDown = (event: PointerEvent): void => {
    const canvas = this.canvas;
    if (canvas === null) {
      return;
    }
    canvas.focus();
    const [x, y] = this.local(event);
    canvas.setPointerCapture(event.pointerId);
    if (event.button === 1 || event.button === 2 || (event.button === 0 && this.spaceDown)) {
      event.preventDefault();
      this.drag = { kind: 'pan', x, y };
      canvas.style.cursor = 'grabbing';
      return;
    }
    if (event.button === 0) {
      this.drag = { kind: 'pending', x, y, pointer: event.pointerId };
    }
  };

  private onPointerMove = (event: PointerEvent): void => {
    const [x, y] = this.local(event);
    this.pointer = [x, y];
    this.cursor = this.camera.toScene(x, y);
    const drag = this.drag;
    if (drag?.kind === 'pan') {
      this.navigated();
      this.camera.pan(x - drag.x, y - drag.y);
      this.drag = { kind: 'pan', x, y };
      this.requestRender();
    } else if (drag?.kind === 'pending') {
      if (Math.hypot(x - drag.x, y - drag.y) > DRAG_THRESHOLD) {
        this.drag =
          this.tool === 'lasso' || event.altKey
            ? { kind: 'lasso', points: [drag.x, drag.y, x, y], pointer: drag.pointer }
            : { kind: 'box', x: drag.x, y: drag.y, pointer: drag.pointer };
        this.updateOverlay(x, y);
      }
    } else if (drag?.kind === 'box' || drag?.kind === 'lasso') {
      this.updateOverlay(x, y);
    } else {
      this.hoverDirty = true;
      this.requestRender();
    }
    this.viewChanged();
  };

  private updateOverlay(x: number, y: number): void {
    const drag = this.drag;
    if (drag?.kind === 'box') {
      this.overlay = { kind: 'box', x0: drag.x, y0: drag.y, x1: x, y1: y, crossing: x < drag.x };
    } else if (drag?.kind === 'lasso') {
      const n = drag.points.length;
      if (Math.hypot(x - (drag.points[n - 2] ?? x), y - (drag.points[n - 1] ?? y)) > 3) {
        drag.points.push(x, y);
      }
      this.overlay = { kind: 'lasso', points: [...drag.points] };
    }
  }

  private onPointerUp = (event: PointerEvent): void => {
    const drag = this.drag;
    this.drag = null;
    if (this.canvas !== null) {
      this.canvas.style.cursor = '';
      if (this.canvas.hasPointerCapture(event.pointerId)) {
        this.canvas.releasePointerCapture(event.pointerId);
      }
    }
    const scene = this.scene;
    if (drag === null || drag.kind === 'pan' || scene === null) {
      return;
    }
    const additive = event.shiftKey;
    const toggle = event.ctrlKey || event.metaKey;
    let hits: number[];
    if (drag.kind === 'pending') {
      const [sx, sy] = this.camera.toScene(drag.x, drag.y);
      const hit = pick(
        scene,
        sx,
        sy,
        HIT_TOLERANCE_PX / this.camera.scale,
        this.isLayerVisible,
        this.active,
      );
      hits = hit === null ? [] : [hit];
    } else if (drag.kind === 'box') {
      const [x, y] = this.local(event);
      const [ax, ay] = this.camera.toScene(drag.x, drag.y);
      const [bx, by] = this.camera.toScene(x, y);
      const box: Box = [Math.min(ax, bx), Math.min(ay, by), Math.max(ax, bx), Math.max(ay, by)];
      hits = selectBox(scene, box, x < drag.x ? 'crossing' : 'window', this.isLayerVisible);
    } else {
      const polygon: number[] = [];
      for (let i = 0; i < drag.points.length; i += 2) {
        polygon.push(...this.camera.toScene(drag.points[i] ?? 0, drag.points[i + 1] ?? 0));
      }
      hits = selectLasso(scene, polygon, this.isLayerVisible);
    }
    this.overlay = null;
    this.applySelection(hits, additive, toggle);
    this.viewChanged();
  };

  private applySelection(hits: readonly number[], additive: boolean, toggle: boolean): void {
    let next: Set<number>;
    if (toggle) {
      next = new Set(this.selected);
      for (const obj of hits) {
        if (next.has(obj)) {
          next.delete(obj);
        } else {
          next.add(obj);
        }
      }
    } else if (additive) {
      next = new Set([...this.selected, ...hits]);
    } else {
      next = new Set(hits);
    }
    this.selected = next;
    this.selectionChanged();
  }

  private onPointerCancel = (): void => {
    this.drag = null;
    this.overlay = null;
    this.viewChanged();
  };

  private onPointerLeave = (): void => {
    if (this.drag === null) {
      this.cursor = null;
      this.pointer = null;
      if (this.hovered !== null) {
        this.hovered = null;
        this.updateFlags();
        this.requestRender();
      }
      this.viewChanged();
    }
  };

  private updateHover(): void {
    const scene = this.scene;
    const pointer = this.pointer;
    if (scene === null || pointer === null) {
      return;
    }
    const [x, y] = this.camera.toScene(pointer[0], pointer[1]);
    const hit = pick(
      scene,
      x,
      y,
      HIT_TOLERANCE_PX / this.camera.scale,
      this.isLayerVisible,
      this.active,
    );
    if (hit !== this.hovered) {
      this.hovered = hit;
      this.updateFlags();
      this.viewChanged();
    }
  }

  private onWheel = (event: WheelEvent): void => {
    event.preventDefault();
    this.navigated();
    const [x, y] = this.local(event);
    const lines = event.deltaMode === WheelEvent.DOM_DELTA_LINE;
    const dy = lines ? event.deltaY * 33 : event.deltaY;
    const dx = lines ? event.deltaX * 33 : event.deltaX;
    if (event.ctrlKey || event.metaKey) {
      // Pinch gesture (trackpad) or Ctrl+wheel.
      this.fitAnimation = null;
      this.zoomAnimation = null;
      this.camera.zoomAt(x, y, Math.exp(-dy * 0.01));
      this.requestRender();
    } else if (event.shiftKey) {
      this.camera.pan(-(dy || dx), 0);
      this.requestRender();
    } else {
      const mouseWheel = lines || (dx === 0 && Math.abs(dy) >= 50 && Number.isInteger(dy));
      if (mouseWheel) {
        this.zoomAround(x, y, dy < 0 ? WHEEL_ZOOM : 1 / WHEEL_ZOOM);
      } else {
        this.fitAnimation = null;
        this.camera.pan(-dx, -dy);
        this.requestRender();
      }
    }
    this.cursor = this.camera.toScene(x, y);
    this.viewChanged();
  };

  private onKeyDown = (event: KeyboardEvent): void => {
    const pan = (fx: number, fy: number): void => {
      this.navigated();
      const step = event.shiftKey ? 0.5 : 0.15;
      this.camera.pan(fx * this.camera.width * step, fy * this.camera.height * step);
      this.requestRender();
      this.viewChanged();
    };
    const mod = event.ctrlKey || event.metaKey;
    let handled = true;
    switch (event.key) {
      case 'ArrowLeft':
        pan(1, 0);
        break;
      case 'ArrowRight':
        pan(-1, 0);
        break;
      case 'ArrowUp':
        pan(0, 1);
        break;
      case 'ArrowDown':
        pan(0, -1);
        break;
      case '+':
      case '=':
        this.zoomBy(KEY_ZOOM);
        break;
      case '-':
        this.zoomBy(1 / KEY_ZOOM);
        break;
      case 'Home':
        this.fitAll();
        break;
      case 'f':
      case 'F':
        if (mod) {
          handled = false;
        } else {
          this.fitSelection();
        }
        break;
      case 'b':
      case 'B':
        if (mod) {
          handled = false;
        } else {
          this.toggleFlip();
        }
        break;
      case 'Escape':
        if (this.drag !== null) {
          this.onPointerCancel();
        } else {
          this.clearSelection();
        }
        break;
      case 'a':
      case 'A':
        if (mod) {
          this.selectAll();
        } else {
          handled = false;
        }
        break;
      case ' ':
        this.spaceDown = true;
        break;
      default:
        handled = false;
    }
    if (handled) {
      event.preventDefault();
      event.stopPropagation();
    }
  };

  private onKeyUp = (event: KeyboardEvent): void => {
    if (event.key === ' ') {
      this.spaceDown = false;
    }
  };
}
