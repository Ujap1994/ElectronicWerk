/**
 * The 3D view of a board, drawn with three.js (loaded on demand). Board
 * coordinates throughout: millimetres, z up. The board's meshes come as one
 * binary buffer from the engine; the parts' models are GLB files from the
 * mesh cache, placed by the matrices the engine computes.
 */
import {
  AmbientLight,
  Box3,
  Box3Helper,
  BufferAttribute,
  BufferGeometry,
  Color,
  DirectionalLight,
  DoubleSide,
  FrontSide,
  Group,
  HemisphereLight,
  Line,
  LineBasicMaterial,
  Mesh,
  MeshStandardMaterial,
  type Object3D,
  OrthographicCamera,
  PerspectiveCamera,
  Plane,
  PMREMGenerator,
  Points,
  PointsMaterial,
  Raycaster,
  Scene,
  Vector2,
  Vector3,
  WebGLRenderer,
  type Material,
} from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { RoomEnvironment } from 'three/addons/environments/RoomEnvironment.js';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';

import type {
  BoardInfo,
  Marker,
  MechanicalBody,
  MeshInfo,
  Part3d,
  Projection,
  Rgb,
  SceneReply,
  Section,
  ViewPreset,
  Visibility,
} from './types';

/** Directions from the target to the camera (z up), with a tiny tilt at the poles. */
const DIRECTIONS: Readonly<Record<ViewPreset, readonly [number, number, number]>> = {
  top: [0, -1e-4, 1],
  bottom: [0, 1e-4, -1],
  front: [0, -1, 0],
  back: [0, 1, 0],
  left: [-1, 0, 0],
  right: [1, 0, 0],
  iso: [0.55, -0.75, 0.6],
};

function color(rgb: Rgb): Color {
  // The engine's colours are sRGB.
  return new Color().setRGB(rgb[0], rgb[1], rgb[2], 'srgb');
}

/** A mechanical body on screen. */
interface BodyObject {
  readonly group: Group;
  body: MechanicalBody;
  /** The file shown (to see when it changes). */
  file: string;
}

/** A part on screen: its group and what it is. */
interface PartObject {
  readonly group: Group;
  part: Part3d;
  /** Model paths shown (to see when they change). */
  models: string;
  /** Invisible box for picking parts without a model. */
  readonly pick: Mesh;
}

export type PickHandler = (id: string | null, additive: boolean) => void;
export type BodyPickHandler = (id: string) => void;
export type MeasureHandler = (points: readonly (readonly [number, number, number])[]) => void;

export class Board3dScene {
  private readonly renderer: WebGLRenderer;
  private readonly scene = new Scene();
  private readonly perspective = new PerspectiveCamera(35, 1, 0.1, 10_000);
  private readonly orthographic = new OrthographicCamera(-1, 1, 1, -1, -10_000, 10_000);
  private camera: PerspectiveCamera | OrthographicCamera = this.perspective;
  private controls: OrbitControls;
  private readonly board = new Group();
  private readonly parts = new Group();
  private readonly overlay = new Group();
  private readonly mechanical = new Group();
  private readonly bodyObjects = new Map<string, BodyObject>();
  private readonly bodyTemplates = new Map<string, Object3D | null>();
  private readonly bodyPending = new Set<string>();
  private markers: readonly Marker[] = [];
  /** Fit the view once the first bodies are loaded. */
  private fitBodies = false;
  private readonly boardMeshes = new Map<string, Mesh>();
  private readonly partObjects = new Map<string, PartObject>();
  private readonly templates = new Map<string, Object3D | null>();
  private readonly pending = new Set<string>();
  private readonly materials = new Map<string, MeshStandardMaterial>();
  private readonly raycaster = new Raycaster();
  private info: BoardInfo | null = null;
  private visibility: Visibility | null = null;
  private selection: readonly string[] = [];
  private selectionBoxes: Box3Helper[] = [];
  private section: Section = { axis: null, position: 0.5, flip: false };
  private measuring = false;
  private measurePoints: (readonly [number, number, number])[] = [];
  private frame = 0;
  private aspect = 1;
  private disposed = false;
  private down: { x: number; y: number } | null = null;
  private readonly viewListeners = new Set<() => void>();

  private onPick: PickHandler = () => undefined;
  private onMeasure: MeasureHandler = () => undefined;
  private onPickBody: BodyPickHandler = () => undefined;
  private loadBody: (file: string) => Promise<Uint8Array | null> = () => Promise.resolve(null);
  private onBodyLoaded: (file: string, ok: boolean) => void = () => undefined;
  private loadModel: (path: string) => Promise<Uint8Array | null> = () => Promise.resolve(null);
  private onModelLoaded: (path: string, ok: boolean) => void = () => undefined;

  constructor(private readonly canvas: HTMLCanvasElement) {
    this.renderer = new WebGLRenderer({ canvas, antialias: true, alpha: false });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.renderer.localClippingEnabled = true;
    for (const camera of [this.perspective, this.orthographic]) {
      camera.up.set(0, 0, 1);
    }
    const environment = new PMREMGenerator(this.renderer);
    this.scene.environment = environment.fromScene(new RoomEnvironment(), 0.04).texture;
    this.scene.environmentIntensity = 0.75;
    environment.dispose();
    this.scene.add(new HemisphereLight(0xffffff, 0x404850, 0.7));
    this.scene.add(new AmbientLight(0xffffff, 0.25));
    const sun = new DirectionalLight(0xffffff, 1.4);
    sun.position.set(0.4, -0.6, 1);
    this.scene.add(sun);
    this.scene.add(this.board, this.parts, this.mechanical, this.overlay);
    this.controls = this.makeControls();
    canvas.addEventListener('pointerdown', this.pointerDown);
    canvas.addEventListener('pointerup', this.pointerUp);
  }

  private makeControls(): OrbitControls {
    const controls = new OrbitControls(this.camera, this.canvas);
    controls.enableDamping = false;
    controls.screenSpacePanning = true;
    controls.zoomToCursor = true;
    controls.addEventListener('change', () => {
      this.render();
      for (const listener of this.viewListeners) {
        listener();
      }
    });
    return controls;
  }

  /** What the view calls back: model loading, picking and measuring. */
  setCallbacks(callbacks: {
    /** Loads the GLB of a model path (`null` if it cannot be shown). */
    readonly loadModel?: (path: string) => Promise<Uint8Array | null>;
    /** A model finished loading (for progress). */
    readonly onModelLoaded?: (path: string, ok: boolean) => void;
    readonly onPick?: PickHandler;
    readonly onMeasure?: MeasureHandler;
    /** Loads the GLB of a mechanical body's STEP file. */
    readonly loadBody?: (file: string) => Promise<Uint8Array | null>;
    readonly onBodyLoaded?: (file: string, ok: boolean) => void;
    readonly onPickBody?: BodyPickHandler;
  }): void {
    this.loadBody = callbacks.loadBody ?? this.loadBody;
    this.onBodyLoaded = callbacks.onBodyLoaded ?? this.onBodyLoaded;
    this.onPickBody = callbacks.onPickBody ?? this.onPickBody;
    this.loadModel = callbacks.loadModel ?? this.loadModel;
    this.onModelLoaded = callbacks.onModelLoaded ?? this.onModelLoaded;
    this.onPick = callbacks.onPick ?? this.onPick;
    this.onMeasure = callbacks.onMeasure ?? this.onMeasure;
  }

  /** Subscribes to camera changes (for overlays). */
  onView(listener: () => void): () => void {
    this.viewListeners.add(listener);
    return () => {
      this.viewListeners.delete(listener);
    };
  }

  setTheme(dark: boolean): void {
    this.scene.background = new Color(dark ? 0x1b1e23 : 0xe9ecef);
    this.render();
  }

  resize(width: number, height: number): void {
    if (width <= 0 || height <= 0) {
      return;
    }
    this.renderer.setSize(width, height, false);
    this.aspect = width / height;
    this.perspective.aspect = this.aspect;
    this.perspective.updateProjectionMatrix();
    this.updateOrthographic();
    this.render();
  }

  private material(key: string, make: () => MeshStandardMaterial): MeshStandardMaterial {
    let material = this.materials.get(key);
    if (material === undefined) {
      material = make();
      this.materials.set(key, material);
    }
    return material;
  }

  private boardMaterial(key: string): MeshStandardMaterial {
    const [kind = '', side] = key.split(':');
    const top = side !== 'bottom';
    switch (kind) {
      case 'body':
        return this.material('body', () => new MeshStandardMaterial({ roughness: 0.8 }));
      case 'barrels':
      case 'copper':
        return this.material(
          'copper',
          () => new MeshStandardMaterial({ metalness: 0.85, roughness: 0.32 }),
        );
      case 'mask':
        return this.material(
          `mask:${top ? 'top' : 'bottom'}`,
          () =>
            new MeshStandardMaterial({
              roughness: 0.45,
              metalness: 0.05,
              transparent: true,
              opacity: 0.9,
              depthWrite: false,
            }),
        );
      case 'silk':
        return this.material(
          `silk:${top ? 'top' : 'bottom'}`,
          () => new MeshStandardMaterial({ roughness: 0.75 }),
        );
      default:
        return this.material('paste', () => new MeshStandardMaterial({ roughness: 0.9 }));
    }
  }

  /** Colours from the stack-up. */
  private paint(info: BoardInfo): void {
    const c = info.colors;
    const set = (key: string, rgb: Rgb): void => {
      this.materials.get(key)?.color.copy(color(rgb));
    };
    set('body', c.body);
    set('copper', c.copper);
    set('mask:top', c.topMask);
    set('mask:bottom', c.bottomMask);
    set('silk:top', c.topSilk);
    set('silk:bottom', c.bottomSilk);
    set('paste', c.paste);
  }

  /** Shows the board meshes of a scene reply (the parts come separately). */
  setBoard(reply: SceneReply): void {
    const first = this.info === null;
    this.info = reply.board;
    const buffer = reply.data.buffer;
    const base = reply.data.byteOffset;
    const keep = new Set<string>();
    for (const info of reply.meshes) {
      keep.add(info.key);
      const geometry = this.geometry(buffer as ArrayBuffer, base, info);
      const existing = this.boardMeshes.get(info.key);
      if (existing !== undefined) {
        existing.geometry.dispose();
        existing.geometry = geometry;
        continue;
      }
      const mesh = new Mesh(geometry, this.boardMaterial(info.key));
      mesh.name = info.key;
      // Transparent mask after the copper and the silkscreen under it.
      mesh.renderOrder = info.key.startsWith('mask') ? 2 : 0;
      this.boardMeshes.set(info.key, mesh);
      this.board.add(mesh);
    }
    for (const [key, mesh] of this.boardMeshes) {
      if (!keep.has(key)) {
        this.board.remove(mesh);
        mesh.geometry.dispose();
        this.boardMeshes.delete(key);
      }
    }
    this.paint(reply.board);
    this.applyVisibility();
    this.applySection();
    if (first) {
      this.view('iso');
    }
    this.render();
  }

  private geometry(buffer: ArrayBuffer, base: number, info: MeshInfo): BufferGeometry {
    const geometry = new BufferGeometry();
    // Copies: the engine's buffer is replaced on the next update.
    const positions = new Float32Array(buffer, base + info.positions, info.vertices * 3).slice();
    const normals = new Float32Array(buffer, base + info.normals, info.vertices * 3).slice();
    const indices = new Uint32Array(buffer, base + info.indices, info.triangles * 3).slice();
    geometry.setAttribute('position', new BufferAttribute(positions, 3));
    geometry.setAttribute('normal', new BufferAttribute(normals, 3));
    geometry.setIndex(new BufferAttribute(indices, 1));
    geometry.computeBoundingSphere();
    return geometry;
  }

  /** Places the parts; loads models it has not seen yet. */
  setParts(parts: readonly Part3d[]): void {
    const seen = new Set<string>();
    for (const part of parts) {
      seen.add(part.id);
      let object = this.partObjects.get(part.id);
      const models = part.models.map((m) => `${m.path}|${m.matrix.join(',')}`).join(';');
      if (object === undefined) {
        const group = new Group();
        group.userData['part'] = part.id;
        const pick = new Mesh(new BufferGeometry(), new MeshStandardMaterial({ visible: false }));
        pick.userData['part'] = part.id;
        group.add(pick);
        object = { group, part, models: '', pick };
        this.partObjects.set(part.id, object);
        this.parts.add(group);
      }
      object.part = part;
      this.placePick(object);
      if (object.models !== models) {
        object.models = models;
        this.fillModels(object);
      }
    }
    for (const [id, object] of this.partObjects) {
      if (!seen.has(id)) {
        this.parts.remove(object.group);
        object.pick.geometry.dispose();
        this.partObjects.delete(id);
      }
    }
    this.applyVisibility();
    this.updateSelection();
    this.render();
  }

  /** A box over the pads, 0.5 mm high, for picking parts without models. */
  private placePick(object: PartObject): void {
    const [x0 = -0.5, y0 = -0.5, x1 = 0.5, y1 = 0.5] = object.part.pads;
    const box = new BufferGeometry();
    const h = 0.5;
    const corners = [
      [x0, y0, 0],
      [x1, y0, 0],
      [x1, y1, 0],
      [x0, y1, 0],
      [x0, y0, h],
      [x1, y0, h],
      [x1, y1, h],
      [x0, y1, h],
    ].flat();
    box.setAttribute('position', new BufferAttribute(new Float32Array(corners), 3));
    box.setIndex([
      0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6, 3,
      0, 4, 3, 4, 7,
    ]);
    object.pick.geometry.dispose();
    object.pick.geometry = box;
    object.pick.matrixAutoUpdate = false;
    object.pick.matrix.fromArray(object.part.matrix);
    object.pick.matrixWorldNeedsUpdate = true;
  }

  private fillModels(object: PartObject): void {
    for (const child of [...object.group.children]) {
      if (child !== object.pick) {
        object.group.remove(child);
      }
    }
    for (const model of object.part.models) {
      const placed = new Group();
      placed.matrixAutoUpdate = false;
      placed.matrix.fromArray(model.matrix);
      placed.userData['part'] = object.part.id;
      placed.userData['path'] = model.path;
      object.group.add(placed);
      const template = this.templates.get(model.path);
      if (template !== undefined) {
        if (template !== null) {
          placed.add(this.instance(template, object.part.id));
        }
      } else {
        this.requestModel(model.path);
      }
    }
  }

  private instance(template: Object3D, part: string): Object3D {
    const copy = template.clone();
    copy.traverse((node) => {
      node.userData['part'] = part;
    });
    return copy;
  }

  private requestModel(path: string): void {
    if (this.pending.has(path)) {
      return;
    }
    this.pending.add(path);
    void this.loadModel(path)
      .then(async (glb) => {
        if (glb === null || this.disposed) {
          return null;
        }
        const loader = new GLTFLoader();
        const bytes = glb.buffer.slice(glb.byteOffset, glb.byteOffset + glb.byteLength);
        const gltf = await loader.parseAsync(bytes as ArrayBuffer, '');
        // GLB: metres, y up → board: millimetres, z up.
        const template = new Group();
        template.rotation.x = Math.PI / 2;
        template.scale.setScalar(1000);
        template.add(gltf.scene);
        template.updateMatrix();
        return template;
      })
      .catch(() => null)
      .then((template) => {
        this.pending.delete(path);
        if (this.disposed) {
          return;
        }
        this.templates.set(path, template);
        for (const object of this.partObjects.values()) {
          for (const child of object.group.children) {
            if (child.userData['path'] === path && child.children.length === 0 && template) {
              child.add(this.instance(template, object.part.id));
            }
          }
        }
        this.onModelLoaded(path, template !== null);
        this.applySection();
        this.render();
      });
  }

  /** The number of model paths that are loaded, failed and known. */
  modelState(): { loaded: number; failed: number } {
    let loaded = 0;
    let failed = 0;
    for (const template of this.templates.values()) {
      if (template === null) {
        failed += 1;
      } else {
        loaded += 1;
      }
    }
    return { loaded, failed };
  }

  /** Shows the mechanical bodies; loads files it has not seen yet. */
  setBodies(bodies: readonly MechanicalBody[]): void {
    if (this.bodyObjects.size === 0 && bodies.length > 0) {
      this.fitBodies = true;
    }
    const seen = new Set<string>();
    for (const body of bodies) {
      seen.add(body.id);
      let object = this.bodyObjects.get(body.id);
      if (object === undefined) {
        const group = new Group();
        group.matrixAutoUpdate = false;
        group.userData['body'] = body.id;
        object = { group, body, file: '' };
        this.bodyObjects.set(body.id, object);
        this.mechanical.add(group);
      }
      object.body = body;
      object.group.matrix.fromArray(body.matrix);
      object.group.matrixWorldNeedsUpdate = true;
      object.group.visible = !body.hidden && body.exists;
      if (object.file !== body.file) {
        object.file = body.file;
        this.fillBody(object);
      } else {
        this.paintBody(object);
      }
    }
    for (const [id, object] of this.bodyObjects) {
      if (!seen.has(id)) {
        this.mechanical.remove(object.group);
        this.bodyObjects.delete(id);
      }
    }
    this.fitWhenLoaded();
    this.applySection();
    this.render();
  }

  /** Fits the view to the enclosure when its first bodies are shown. */
  private fitWhenLoaded(): void {
    if (
      this.fitBodies &&
      this.info !== null &&
      [...this.bodyObjects.values()].every((o) => this.bodyTemplates.has(o.body.file))
    ) {
      this.fitBodies = false;
      this.fit();
    }
  }

  private fillBody(object: BodyObject): void {
    object.group.clear();
    const file = object.body.file;
    const template = this.bodyTemplates.get(file);
    if (template === undefined) {
      this.requestBody(file);
      return;
    }
    if (template !== null) {
      object.group.add(this.bodyInstance(template, object.body.id));
      this.paintBody(object);
    }
  }

  /** A copy with its own materials (each body has its own opacity). */
  private bodyInstance(template: Object3D, id: string): Object3D {
    const copy = template.clone();
    copy.traverse((node) => {
      node.userData['body'] = id;
      if (node instanceof Mesh) {
        const list = Array.isArray(node.material) ? node.material : [node.material];
        const own = (list as Material[]).map((m) => m.clone());
        node.material = Array.isArray(node.material) ? own : own[0];
      }
    });
    return copy;
  }

  private paintBody(object: BodyObject): void {
    const opacity = Math.min(Math.max(object.body.opacity, 5), 100) / 100;
    const transparent = opacity < 1;
    object.group.traverse((node) => {
      if (node instanceof Mesh) {
        const list = Array.isArray(node.material) ? node.material : [node.material];
        for (const material of list as Material[]) {
          if (material.opacity !== opacity || material.transparent !== transparent) {
            material.opacity = opacity;
            material.transparent = transparent;
            // Transparent bodies do not hide what lies behind them, and
            // win over solid faces in the same plane (no stripes).
            material.depthWrite = !transparent;
            material.polygonOffset = transparent;
            material.polygonOffsetFactor = transparent ? -1 : 0;
            material.polygonOffsetUnits = transparent ? -4 : 0;
            material.side = transparent ? DoubleSide : material.side;
            material.needsUpdate = true;
          }
        }
        node.renderOrder = transparent ? 3 : 0;
      }
    });
  }

  private requestBody(file: string): void {
    if (this.bodyPending.has(file)) {
      return;
    }
    this.bodyPending.add(file);
    void this.loadBody(file)
      .then(async (glb) => {
        if (glb === null || this.disposed) {
          return null;
        }
        const loader = new GLTFLoader();
        const bytes = glb.buffer.slice(glb.byteOffset, glb.byteOffset + glb.byteLength);
        const gltf = await loader.parseAsync(bytes as ArrayBuffer, '');
        // GLB: metres, y up → board: millimetres, z up.
        const template = new Group();
        template.rotation.x = Math.PI / 2;
        template.scale.setScalar(1000);
        template.add(gltf.scene);
        template.updateMatrix();
        return template;
      })
      .catch(() => null)
      .then((template) => {
        this.bodyPending.delete(file);
        if (this.disposed) {
          return;
        }
        this.bodyTemplates.set(file, template);
        for (const object of this.bodyObjects.values()) {
          if (object.body.file === file) {
            this.fillBody(object);
          }
        }
        this.onBodyLoaded(file, template !== null);
        this.fitWhenLoaded();
        this.applySection();
        this.render();
      });
  }

  /** The number of body files loaded and failed. */
  bodyState(): { loaded: number; failed: number } {
    let loaded = 0;
    let failed = 0;
    for (const template of this.bodyTemplates.values()) {
      if (template === null) {
        failed += 1;
      } else {
        loaded += 1;
      }
    }
    return { loaded, failed };
  }

  /**
   * How far a body must move (x, y in mm) for the middle of its box to lie
   * over the middle of the board.
   */
  centerShift(id: string): [number, number] | null {
    const object = this.bodyObjects.get(id);
    const b = this.info?.bounds;
    if (object === undefined || b?.length !== 6) {
      return null;
    }
    object.group.updateMatrixWorld(true);
    const box = new Box3().setFromObject(object.group, true);
    if (box.isEmpty()) {
      return null;
    }
    const centre = box.getCenter(new Vector3());
    const bx = ((b[0] ?? 0) + (b[3] ?? 0)) / 2;
    const by = ((b[1] ?? 0) + (b[4] ?? 0)) / 2;
    return [bx - centre.x, by - centre.y];
  }

  /** Shows the findings of the enclosure check. */
  setMarkers(markers: readonly Marker[]): void {
    this.markers = markers;
    for (const child of [...this.overlay.children]) {
      if (child.userData['marker'] === true) {
        this.overlay.remove(child);
        (child as Points).geometry.dispose();
      }
    }
    for (const severity of ['warning', 'error'] as const) {
      const points = markers.filter((m) => m.severity === severity);
      if (points.length === 0) {
        continue;
      }
      const geometry = new BufferGeometry().setFromPoints(
        points.map((m) => new Vector3(m.point[0], m.point[1], m.point[2])),
      );
      const dots = new Points(
        geometry,
        new PointsMaterial({
          color: severity === 'error' ? 0xdc2626 : 0xf59e0b,
          size: 12,
          sizeAttenuation: false,
          depthTest: false,
          transparent: true,
        }),
      );
      dots.userData['marker'] = true;
      dots.renderOrder = 12;
      this.overlay.add(dots);
    }
    this.render();
  }

  /** The markers shown (for tests). */
  markerCount(): number {
    return this.markers.length;
  }

  /** Moves the camera to a point (a finding). */
  focusPoint(point: readonly [number, number, number], radius = 6): void {
    const centre = new Vector3(point[0], point[1], point[2]);
    const direction = this.camera.position.clone().sub(this.controls.target).normalize();
    const distance = (radius / Math.sin((this.perspective.fov * Math.PI) / 360)) * 1.6;
    this.camera.position.copy(centre).addScaledVector(direction, distance);
    this.controls.target.copy(centre);
    this.updateOrthographic();
    this.controls.update();
    this.render();
  }

  setVisibility(visibility: Visibility): void {
    this.visibility = visibility;
    this.applyVisibility();
    this.render();
  }

  private applyVisibility(): void {
    const v = this.visibility;
    if (v === null) {
      return;
    }
    for (const [key, mesh] of this.boardMeshes) {
      const [kind, side = ''] = key.split(':');
      mesh.visible =
        kind === 'body'
          ? v.body
          : kind === 'barrels'
            ? v.copper
            : kind === 'copper'
              ? side.startsWith('in')
                ? v.inner
                : v.copper
              : kind === 'mask'
                ? v.mask
                : kind === 'silk'
                  ? v.silk
                  : v.paste;
    }
    for (const object of this.partObjects.values()) {
      const part = object.part;
      object.group.visible =
        (part.side === 'top' ? v.partsTop : v.partsBottom) && (v.dnp || !part.dnp);
    }
  }

  setSelection(ids: readonly string[]): void {
    this.selection = ids;
    this.updateSelection();
    this.render();
  }

  private updateSelection(): void {
    for (const box of this.selectionBoxes) {
      this.overlay.remove(box);
      box.geometry.dispose();
    }
    this.selectionBoxes = [];
    for (const id of this.selection) {
      const object = this.partObjects.get(id);
      if (object === undefined) {
        continue;
      }
      object.group.updateMatrixWorld(true);
      const box = new Box3().setFromObject(object.group, true);
      if (box.isEmpty()) {
        continue;
      }
      box.expandByScalar(0.15);
      const helper = new Box3Helper(box, new Color(0xf59e0b));
      // Drawn with the transparent objects, after the solder mask.
      const material = helper.material as LineBasicMaterial;
      material.depthTest = false;
      material.transparent = true;
      helper.renderOrder = 10;
      this.selectionBoxes.push(helper);
      this.overlay.add(helper);
    }
  }

  setProjection(projection: Projection): void {
    const next = projection === 'orthographic' ? this.orthographic : this.perspective;
    if (next === this.camera) {
      return;
    }
    const target = this.controls.target.clone();
    next.position.copy(this.camera.position);
    next.up.copy(this.camera.up);
    this.camera = next;
    this.controls.dispose();
    this.controls = this.makeControls();
    this.controls.target.copy(target);
    this.updateOrthographic();
    this.controls.update();
    this.render();
  }

  private updateOrthographic(): void {
    // The orthographic camera shows what the perspective camera shows at
    // the target's distance.
    const distance = this.camera.position.distanceTo(this.controls.target);
    const height = 2 * distance * Math.tan((this.perspective.fov * Math.PI) / 360);
    const zoom = this.camera === this.orthographic ? this.orthographic.zoom : 1;
    this.orthographic.left = (-height * this.aspect) / 2;
    this.orthographic.right = (height * this.aspect) / 2;
    this.orthographic.top = height / 2;
    this.orthographic.bottom = -height / 2;
    this.orthographic.zoom = zoom;
    this.orthographic.updateProjectionMatrix();
  }

  /** The box of the board and its parts. */
  private bounds(): Box3 {
    const box = new Box3();
    const b = this.info?.bounds;
    if (b?.length === 6) {
      box.set(new Vector3(b[0], b[1], b[2]), new Vector3(b[3], b[4], b[5]));
    }
    this.parts.updateMatrixWorld(true);
    box.union(new Box3().setFromObject(this.parts, true));
    this.mechanical.updateMatrixWorld(true);
    for (const object of this.bodyObjects.values()) {
      if (object.group.visible) {
        box.union(new Box3().setFromObject(object.group, true));
      }
    }
    if (box.isEmpty()) {
      box.set(new Vector3(-10, -10, -1), new Vector3(10, 10, 1));
    }
    return box;
  }

  /** Looks at the board from a side and fits it. */
  view(preset: ViewPreset): void {
    const box = this.bounds();
    const centre = box.getCenter(new Vector3());
    const radius = box.getSize(new Vector3()).length() / 2 || 10;
    const [dx, dy, dz] = DIRECTIONS[preset];
    const direction = new Vector3(dx, dy, dz).normalize();
    // The bounding sphere is generous for flat boards.
    const distance = (radius / Math.sin((this.perspective.fov * Math.PI) / 360)) * 0.85;
    this.camera.position.copy(centre).addScaledVector(direction, distance);
    this.controls.target.copy(centre);
    for (const camera of [this.perspective, this.orthographic]) {
      camera.near = distance / 1000;
      camera.far = distance * 100;
    }
    this.orthographic.near = -distance * 10;
    this.orthographic.zoom = 1;
    this.perspective.updateProjectionMatrix();
    this.updateOrthographic();
    this.controls.update();
    this.render();
  }

  /** Fits the view without turning it. */
  fit(): void {
    const box = this.bounds();
    const centre = box.getCenter(new Vector3());
    const radius = box.getSize(new Vector3()).length() / 2 || 10;
    const direction = this.camera.position.clone().sub(this.controls.target).normalize();
    const distance = (radius / Math.sin((this.perspective.fov * Math.PI) / 360)) * 0.85;
    this.camera.position.copy(centre).addScaledVector(direction, distance);
    this.controls.target.copy(centre);
    this.orthographic.zoom = 1;
    this.updateOrthographic();
    this.controls.update();
    this.render();
  }

  /** Moves the camera to a part. */
  focus(id: string): void {
    const object = this.partObjects.get(id);
    if (object === undefined) {
      return;
    }
    const box = new Box3().setFromObject(object.group, true);
    if (box.isEmpty()) {
      return;
    }
    const centre = box.getCenter(new Vector3());
    const radius = Math.max(box.getSize(new Vector3()).length() / 2, 2);
    const direction = this.camera.position.clone().sub(this.controls.target).normalize();
    const distance = (radius / Math.sin((this.perspective.fov * Math.PI) / 360)) * 1.6;
    this.camera.position.copy(centre).addScaledVector(direction, distance);
    this.controls.target.copy(centre);
    this.updateOrthographic();
    this.controls.update();
    this.render();
  }

  setSection(section: Section): void {
    this.section = section;
    this.applySection();
    this.render();
  }

  private applySection(): void {
    const { axis, position, flip } = this.section;
    if (axis === null || this.info === null) {
      this.renderer.clippingPlanes = [];
      this.setDoubleSided(false);
      return;
    }
    const b = this.info.bounds;
    const index = axis === 'x' ? 0 : axis === 'y' ? 1 : 2;
    let lo = (b[index] ?? 0) - (axis === 'z' ? 3 : 0);
    let hi = (b[index + 3] ?? 0) + (axis === 'z' ? 3 : 0);
    // The enclosure around the board is cut too.
    if ([...this.bodyObjects.values()].some((o) => o.group.visible)) {
      const all = this.bounds();
      lo = Math.min(lo, all.min.getComponent(index));
      hi = Math.max(hi, all.max.getComponent(index));
    }
    const at = lo + (hi - lo) * position;
    const normal = new Vector3(axis === 'x' ? -1 : 0, axis === 'y' ? -1 : 0, axis === 'z' ? -1 : 0);
    if (flip) {
      normal.negate();
    }
    // Keeps what lies on the normal's side: x < at by default.
    this.renderer.clippingPlanes = [new Plane(normal, flip ? -at : at)];
    this.setDoubleSided(true);
  }

  /** Cut-open solids show their inside. */
  private setDoubleSided(on: boolean): void {
    const side = on ? DoubleSide : FrontSide;
    const update = (material: Material): void => {
      if (material.side !== side && !material.transparent) {
        material.side = side;
        material.needsUpdate = true;
      }
    };
    for (const material of this.materials.values()) {
      update(material);
    }
    for (const group of [this.parts, this.mechanical]) {
      group.traverse((node) => {
        if (node instanceof Mesh) {
          const list = Array.isArray(node.material) ? node.material : [node.material];
          for (const material of list as Material[]) {
            update(material);
          }
        }
      });
    }
  }

  setMeasuring(on: boolean): void {
    this.measuring = on;
    this.measurePoints = [];
    this.drawMeasure();
    this.onMeasure([]);
  }

  clearMeasure(): void {
    this.measurePoints = [];
    this.drawMeasure();
    this.onMeasure([]);
  }

  private drawMeasure(): void {
    for (const child of [...this.overlay.children]) {
      if (child.userData['measure'] === true) {
        this.overlay.remove(child);
        if (child instanceof Line || child instanceof Points) {
          (child.geometry as BufferGeometry).dispose();
        }
      }
    }
    if (this.measurePoints.length === 0) {
      this.render();
      return;
    }
    const geometry = new BufferGeometry().setFromPoints(
      this.measurePoints.map((p) => new Vector3(p[0], p[1], p[2])),
    );
    const points = new Points(
      geometry,
      new PointsMaterial({
        color: 0xef4444,
        size: 9,
        sizeAttenuation: false,
        depthTest: false,
        transparent: true,
      }),
    );
    points.userData['measure'] = true;
    points.renderOrder = 11;
    this.overlay.add(points);
    if (this.measurePoints.length === 2) {
      const line = new Line(
        geometry.clone(),
        new LineBasicMaterial({ color: 0xef4444, depthTest: false, transparent: true }),
      );
      line.userData['measure'] = true;
      line.renderOrder = 11;
      this.overlay.add(line);
    }
    this.render();
  }

  /** Screen position (CSS pixels in the canvas) of a board point. */
  project(point: readonly [number, number, number]): { x: number; y: number } | null {
    const v = new Vector3(point[0], point[1], point[2]).project(this.camera);
    if (v.z > 1) {
      return null;
    }
    const rect = this.canvas.getBoundingClientRect();
    return { x: ((v.x + 1) / 2) * rect.width, y: ((1 - v.y) / 2) * rect.height };
  }

  private pointerDown = (event: PointerEvent): void => {
    this.down = { x: event.clientX, y: event.clientY };
  };

  private pointerUp = (event: PointerEvent): void => {
    const down = this.down;
    this.down = null;
    if (down === null || event.button !== 0) {
      return;
    }
    // A drag turns the view; only a click picks.
    if (Math.hypot(event.clientX - down.x, event.clientY - down.y) > 4) {
      return;
    }
    const hit = this.hit(event.clientX, event.clientY);
    if (this.measuring) {
      if (hit.solid === null) {
        return;
      }
      if (this.measurePoints.length >= 2) {
        this.measurePoints = [];
      }
      const { point } = hit.solid;
      this.measurePoints.push([point.x, point.y, point.z]);
      this.drawMeasure();
      this.onMeasure([...this.measurePoints]);
      return;
    }
    const body = hit.any?.userData['body'] as string | undefined;
    if (body !== undefined) {
      this.onPickBody(body);
      return;
    }
    const part = hit.any?.userData['part'] as string | undefined;
    this.onPick(part ?? null, event.shiftKey || event.metaKey || event.ctrlKey);
  };

  /**
   * The nearest visible surface under a screen point (`solid`), and the
   * nearest object including the invisible pick boxes of parts (`any`).
   */
  private hit(
    clientX: number,
    clientY: number,
  ): { solid: { point: Vector3; object: Object3D } | null; any: Object3D | null } {
    const rect = this.canvas.getBoundingClientRect();
    const pointer = new Vector2(
      ((clientX - rect.left) / rect.width) * 2 - 1,
      -((clientY - rect.top) / rect.height) * 2 + 1,
    );
    this.raycaster.setFromCamera(pointer, this.camera);
    const planes = this.renderer.clippingPlanes;
    const hits = this.raycaster
      .intersectObjects([this.parts, this.board, this.mechanical], true)
      .filter((h) => {
        // See-through bodies let the clicks through to the board.
        if (h.object.userData['body'] !== undefined) {
          const material = (h.object as Mesh).material;
          const list = Array.isArray(material) ? material : [material];
          if (list.some((m) => m.transparent)) {
            return false;
          }
        }
        // Shown all the way up, and not cut away by a section.
        let node: Object3D | null = h.object;
        while (node !== null) {
          if (!node.visible) {
            return false;
          }
          node = node.parent;
        }
        return planes.every((plane) => plane.distanceToPoint(h.point) >= 0);
      });
    const shown = (object: Object3D): boolean => {
      const material = (object as Mesh).material as Material | Material[] | undefined;
      return Array.isArray(material) || material?.visible !== false;
    };
    const solid = hits.find((h) => shown(h.object));
    return {
      solid: solid === undefined ? null : { point: solid.point, object: solid.object },
      any: (solid ?? hits[0])?.object ?? null,
    };
  }

  /** The view as a PNG file. */
  async screenshot(): Promise<Uint8Array> {
    this.renderer.render(this.scene, this.camera);
    const blob = await new Promise<Blob | null>((resolve) => {
      this.canvas.toBlob(resolve, 'image/png');
    });
    if (blob === null) {
      throw new Error('no image');
    }
    return new Uint8Array(await blob.arrayBuffer());
  }

  render(): void {
    if (this.frame !== 0 || this.disposed) {
      return;
    }
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      this.renderer.render(this.scene, this.camera);
      this.canvas.dataset['rendered'] = 'true';
    });
  }

  dispose(): void {
    this.disposed = true;
    cancelAnimationFrame(this.frame);
    this.canvas.removeEventListener('pointerdown', this.pointerDown);
    this.canvas.removeEventListener('pointerup', this.pointerUp);
    this.controls.dispose();
    this.scene.traverse((node) => {
      if (node instanceof Mesh || node instanceof Line || node instanceof Points) {
        (node.geometry as BufferGeometry).dispose();
      }
    });
    for (const material of this.materials.values()) {
      material.dispose();
    }
    this.renderer.dispose();
  }
}
