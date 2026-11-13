/**
 * Data of the 3D view as the engine sends them (`board3d.scene`,
 * `board3d.parts`). Lengths are millimetres, z points up, matrices are
 * column-major like three.js stores them.
 */

/** Where one mesh lies in the binary buffer (byte offsets). */
export interface MeshInfo {
  /** `body`, `barrels`, `copper:top`, `mask:bottom`, `copper:in1`, … */
  readonly key: string;
  readonly vertices: number;
  readonly triangles: number;
  readonly positions: number;
  readonly normals: number;
  readonly indices: number;
}

export interface Levels {
  readonly bodyTop: number;
  readonly topCopper: number;
  readonly bottomCopper: number;
  readonly topMask: number;
  readonly bottomMask: number;
  readonly copperLayers: number;
}

export type Rgb = readonly [number, number, number];

export interface BoardColors {
  readonly body: Rgb;
  readonly topMask: Rgb;
  readonly bottomMask: Rgb;
  readonly topSilk: Rgb;
  readonly bottomSilk: Rgb;
  readonly copper: Rgb;
  readonly paste: Rgb;
  readonly finish: string;
}

export interface BoardInfo {
  readonly name: string;
  readonly levels: Levels;
  readonly colors: BoardColors;
  /** `[x0, y0, z0, x1, y1, z1]`. */
  readonly bounds: readonly number[];
  readonly holes: number;
  readonly warnings: readonly string[];
}

export interface PlacedModel {
  readonly path: string;
  readonly matrix: readonly number[];
}

export interface Part3d {
  readonly id: string;
  readonly designator: string;
  readonly value: string;
  readonly footprint: string;
  readonly side: 'top' | 'bottom';
  readonly dnp: boolean;
  /** The footprint placement. */
  readonly matrix: readonly number[];
  /** The pads' box in footprint coordinates `[x0, y0, x1, y1]`. */
  readonly pads: readonly number[];
  readonly models: readonly PlacedModel[];
}

/** Where the engine found a model. */
export interface ModelSource {
  readonly path: string;
  readonly file?: string;
  readonly format?: 'step' | 'vrml';
  readonly entry?: number;
  readonly mesh?: string;
}

export interface SceneReply {
  readonly board: BoardInfo;
  readonly meshes: readonly MeshInfo[];
  readonly parts: readonly Part3d[];
  readonly models: readonly ModelSource[];
  readonly data: Uint8Array;
}

export interface PartsReply {
  readonly parts: readonly Part3d[];
  readonly models: readonly ModelSource[];
}

/** Report of `board3d.export`. */
export interface ExportReply {
  readonly placed: number;
  readonly exact: number;
  readonly faceted: number;
  readonly missing: readonly string[];
  readonly warnings: readonly string[];
  readonly bytes: number;
  readonly path: string;
}

/** What the view shows. */
export interface Visibility {
  readonly partsTop: boolean;
  readonly partsBottom: boolean;
  readonly dnp: boolean;
  readonly silk: boolean;
  readonly mask: boolean;
  readonly copper: boolean;
  readonly paste: boolean;
  readonly body: boolean;
  readonly inner: boolean;
}

export type ViewPreset = 'top' | 'bottom' | 'front' | 'back' | 'left' | 'right' | 'iso';

export type Projection = 'perspective' | 'orthographic';

export interface Section {
  readonly axis: 'x' | 'y' | 'z' | null;
  /** 0–1 across the board. */
  readonly position: number;
  readonly flip: boolean;
}

/** A finished measurement. */
export interface Measurement {
  readonly a: readonly [number, number, number];
  readonly b: readonly [number, number, number];
}
