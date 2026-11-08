/**
 * The 3D preview of the library manager: a GLB model (from the mesh cache)
 * on a small board, drawn with three.js and turned with the mouse. Loaded on
 * demand (dynamic import), so three.js does not slow down the app start.
 */
import {
  AmbientLight,
  Box3,
  BoxGeometry,
  type BufferGeometry,
  Color,
  DirectionalLight,
  Euler,
  Group,
  HemisphereLight,
  Mesh,
  MeshStandardMaterial,
  PerspectiveCamera,
  Scene,
  Vector3,
  WebGLRenderer,
  type Object3D,
} from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

/** Placement of a model on its footprint (KiCad convention). */
export interface Placement {
  /** Offset in millimetres, z up. */
  readonly offset: readonly [number, number, number];
  /** Rotation about x, y, z in degrees. */
  readonly rotation: readonly [number, number, number];
  readonly scale: readonly [number, number, number];
}

/** Size of the board under the model, in millimetres. */
export interface BoardSize {
  readonly width: number;
  readonly depth: number;
}

/** The board is drawn as a thin plate, so small parts stay visible. */
const BOARD_THICKNESS = 0.2;

export class ModelView {
  private readonly renderer: WebGLRenderer;
  private readonly scene = new Scene();
  private readonly camera = new PerspectiveCamera(35, 1, 0.0001, 10);
  private readonly controls: OrbitControls;
  /** Board coordinates: metres, z up (rotated into three.js' y up). */
  private readonly board = new Group();
  private content: Object3D | null = null;
  private home: { position: Vector3; target: Vector3 } | null = null;
  private frame = 0;

  constructor(private readonly canvas: HTMLCanvasElement) {
    this.renderer = new WebGLRenderer({ canvas, antialias: true, alpha: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    this.board.rotation.x = -Math.PI / 2;
    this.scene.add(this.board);
    this.scene.add(new HemisphereLight(0xffffff, 0x445566, 1.6));
    this.scene.add(new AmbientLight(0xffffff, 0.35));
    const sun = new DirectionalLight(0xffffff, 1.8);
    sun.position.set(0.6, 1, 0.8);
    this.scene.add(sun);
    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.enableDamping = false;
    this.controls.addEventListener('change', () => {
      this.render();
    });
  }

  setTheme(dark: boolean): void {
    this.scene.background = new Color(dark ? 0x1d2126 : 0xf3f4f6);
    this.render();
  }

  resize(width: number, height: number): void {
    if (width <= 0 || height <= 0) {
      return;
    }
    this.renderer.setSize(width, height, false);
    this.camera.aspect = width / height;
    this.camera.updateProjectionMatrix();
    this.render();
  }

  /** Shows a GLB model, placed like on its footprint, over a board. */
  async show(glb: Uint8Array, placement: Placement | null, board: BoardSize | null): Promise<void> {
    const loader = new GLTFLoader();
    const buffer = glb.buffer.slice(glb.byteOffset, glb.byteOffset + glb.byteLength) as ArrayBuffer;
    const gltf = await loader.parseAsync(buffer, '');
    this.clear();
    const content = new Group();
    // The GLB is y up; turn it back into board coordinates (z up).
    const model = new Group();
    model.rotation.x = Math.PI / 2;
    model.add(gltf.scene);
    const placed = new Group();
    if (placement !== null) {
      const [x, y, z] = placement.offset;
      placed.position.set(x / 1000, y / 1000, z / 1000);
      // KiCad turns models by the negated angles, z first.
      const [rx, ry, rz] = placement.rotation.map((deg) => (-deg * Math.PI) / 180);
      placed.setRotationFromEuler(new Euler(rx, ry, rz, 'ZYX'));
      placed.scale.set(...placement.scale);
    }
    placed.add(model);
    content.add(placed);
    if (board !== null && board.width > 0 && board.depth > 0) {
      const slab = new Mesh(
        new BoxGeometry(board.width / 1000, board.depth / 1000, BOARD_THICKNESS / 1000),
        new MeshStandardMaterial({ color: 0x2b5e37, roughness: 0.85 }),
      );
      slab.position.z = -BOARD_THICKNESS / 2000;
      content.add(slab);
    }
    this.board.add(content);
    this.content = content;
    this.fit();
  }

  /** Back to the start view. */
  reset(): void {
    if (this.home !== null) {
      this.camera.position.copy(this.home.position);
      this.controls.target.copy(this.home.target);
      this.controls.update();
      this.render();
    }
  }

  private fit(): void {
    if (this.content === null) {
      return;
    }
    const box = new Box3().setFromObject(this.content);
    const centre = box.getCenter(new Vector3());
    const size = box.getSize(new Vector3()).length() || 0.01;
    const distance = (size / (2 * Math.tan((this.camera.fov * Math.PI) / 360))) * 1.15;
    const direction = new Vector3(0.55, 0.75, 0.9).normalize();
    this.home = {
      position: centre.clone().add(direction.multiplyScalar(distance)),
      target: centre,
    };
    this.camera.near = distance / 100;
    this.camera.far = distance * 100;
    this.camera.updateProjectionMatrix();
    this.reset();
  }

  private clear(): void {
    if (this.content !== null) {
      this.board.remove(this.content);
      this.content.traverse((node) => {
        if (node instanceof Mesh) {
          (node.geometry as BufferGeometry).dispose();
          const materials = Array.isArray(node.material) ? node.material : [node.material];
          for (const material of materials) {
            (material as MeshStandardMaterial).dispose();
          }
        }
      });
      this.content = null;
    }
  }

  render(): void {
    if (this.frame !== 0) {
      return;
    }
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      this.renderer.render(this.scene, this.camera);
      this.canvas.dataset['rendered'] = 'true';
    });
  }

  dispose(): void {
    cancelAnimationFrame(this.frame);
    this.clear();
    this.controls.dispose();
    this.renderer.dispose();
  }
}
