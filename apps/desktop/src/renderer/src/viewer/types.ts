/**
 * Scene metadata as returned by the engine method `view.scene` (see
 * crates/ew-scene). All coordinates are nanometres relative to `origin`.
 */

export interface Section {
  /** Byte offset of the first instance. */
  readonly offset: number;
  readonly count: number;
  /** Byte offset of the chunk boxes (four f32 per chunk). */
  readonly chunks: number;
}

export interface MeshSection {
  readonly vertices: number;
  readonly vertexCount: number;
  readonly indices: number;
  readonly indexCount: number;
}

/** `[text, x, y, size, rotation°, hAlign, vAlign, flags, object]`. */
export type TextTuple = readonly [
  text: string,
  x: number,
  y: number,
  size: number,
  rotation: number,
  h: number,
  v: number,
  flags: number,
  obj: number,
];

export type LayerGroup =
  | 'copper'
  | 'silkscreen'
  | 'solderMask'
  | 'paste'
  | 'adhesive'
  | 'fabrication'
  | 'courtyard'
  | 'outline'
  | 'mechanical'
  | 'user'
  | 'holes'
  | 'schematic';

export interface SceneLayer {
  readonly key: string;
  readonly group: LayerGroup;
  readonly side: 'top' | 'bottom' | null;
  readonly name: string | null;
  /** Draw order in the top view (back to front). */
  readonly z: number;
  readonly visible: boolean;
  readonly segments: Section;
  readonly arcs: Section;
  readonly circles: Section;
  readonly rects: Section;
  readonly mesh: MeshSection;
  readonly texts: readonly TextTuple[];
}

export interface ObjectSections {
  readonly count: number;
  readonly types: number;
  readonly parents: number;
  readonly children: number;
  readonly nets: number;
  readonly boxes: number;
  readonly ids: number;
}

export interface SceneStats {
  readonly objects: number;
  readonly segments: number;
  readonly arcs: number;
  readonly circles: number;
  readonly rects: number;
  readonly triangles: number;
  readonly texts: number;
  readonly bytes: number;
}

export type Box = readonly [minX: number, minY: number, maxX: number, maxY: number];

export interface SceneMeta {
  readonly doc: string;
  readonly kind: 'pcb' | 'schematic';
  readonly name: string;
  readonly origin: readonly [number, number];
  readonly bounds: Box | null;
  readonly home: Box | null;
  readonly page: readonly [number, number] | null;
  readonly grid: number;
  readonly chunk: number;
  readonly font: { readonly advance: number; readonly linePitch: number };
  readonly layers: readonly SceneLayer[];
  readonly objectTypes: readonly string[];
  readonly objects: ObjectSections;
  readonly nets: readonly string[];
  readonly stats: SceneStats;
}

/** `view.scene` reply in the desktop app: metadata plus the raw buffer. */
export interface SceneReply extends SceneMeta {
  readonly data: Uint8Array | string | null;
}

/** One row of `view.object`. */
export interface InspectorRow {
  readonly key: string;
  readonly value: unknown;
  readonly kind:
    | 'text'
    | 'length'
    | 'point'
    | 'size'
    | 'angle'
    | 'bool'
    | 'layer'
    | 'layers'
    | 'net'
    | 'enum'
    | 'count';
}

export interface ObjectInfo {
  readonly type: string;
  readonly id: string;
  readonly title: string;
  readonly rows: readonly InspectorRow[];
}

/** Text flags. */
export const TEXT_MIRROR = 1;
export const TEXT_BOLD = 2;
export const TEXT_ITALIC = 4;

/** Mesh vertex flag: hatched fill. */
export const MESH_HATCH = 1;

/** "No object" in the u32 tables. */
export const NO_OBJECT = 0xffff_ffff;

/** Floats (or u32 words) per instance. */
export const STRIDE = { segments: 6, arcs: 7, circles: 5, rects: 7, vertices: 4 } as const;
