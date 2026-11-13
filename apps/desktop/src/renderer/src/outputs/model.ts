/**
 * Output jobs in the UI: the job document, option defaults per output kind
 * (the same as the engine's, `crates/ew-output/src/job.rs`) and the run
 * report.
 */

export type OutputKind =
  | 'gerber'
  | 'drill'
  | 'drillMap'
  | 'bom'
  | 'pickAndPlace'
  | 'pdf'
  | 'svg'
  | 'step'
  | 'ipc356'
  | 'ipc2581'
  | 'netlist'
  | 'archive';

/** Kinds in the order of the "add output" menu. */
export const OUTPUT_KINDS: readonly OutputKind[] = [
  'gerber',
  'drill',
  'drillMap',
  'bom',
  'pickAndPlace',
  'pdf',
  'svg',
  'ipc356',
  'ipc2581',
  'netlist',
  'archive',
  'step',
];

export interface OutputSpec {
  readonly id: string;
  readonly name: string;
  readonly kind: OutputKind;
  readonly enabled: boolean;
  readonly source?: string | null;
  readonly options: Readonly<Record<string, unknown>>;
}

export interface OutputJobProps {
  readonly name: string;
  readonly directory: string;
  readonly outputs: readonly OutputSpec[];
}

export type OutputStatus = 'done' | 'skipped' | 'failed';

export interface OutputResult {
  readonly id: string;
  readonly name: string;
  readonly kind: OutputKind;
  readonly status: OutputStatus;
  readonly message?: string;
  readonly files: readonly string[];
}

export interface RunReport {
  readonly directory: string;
  readonly results: readonly OutputResult[];
  readonly files: readonly { readonly path: string; readonly bytes: number }[];
  readonly elapsedMs: number;
}

export interface Preset {
  readonly id: string;
  readonly name: string;
  readonly description: string;
}

export interface BomColumn {
  readonly field: string;
  readonly label?: string;
}

export interface BomPreview {
  readonly headers: readonly string[];
  readonly rows: readonly (readonly (string | number)[])[];
  readonly parts: number;
  readonly fields: readonly string[];
}

/** Default subfolder of each kind. */
const FOLDERS: Readonly<Record<OutputKind, string>> = {
  gerber: 'gerber',
  drill: 'gerber',
  drillMap: 'gerber',
  bom: 'assembly',
  pickAndPlace: 'assembly',
  pdf: 'documentation',
  svg: 'documentation',
  step: '3d',
  ipc356: 'netlists',
  ipc2581: '',
  netlist: 'netlists',
  archive: '',
};

/** Option defaults per kind (missing options take these values). */
export const DEFAULTS: Readonly<Record<OutputKind, Readonly<Record<string, unknown>>>> = {
  gerber: {
    layers: [],
    protelExtensions: true,
    x2: true,
    jobFile: true,
    tentVias: true,
    silkValues: true,
    folder: FOLDERS.gerber,
  },
  drill: { merge: false, folder: FOLDERS.drill },
  drillMap: { format: 'pdf', folder: FOLDERS.drillMap },
  bom: {
    columns: [
      'item',
      'quantity',
      'designators',
      'value',
      'footprint',
      'description',
      'Manufacturer',
      'MPN',
      'dnp',
    ].map((field) => ({ field })),
    groupBy: ['value', 'footprint', 'Manufacturer', 'MPN', 'dnp'],
    includeDnp: true,
    separator: ',',
    format: 'csv',
    folder: FOLDERS.bom,
  },
  pickAndPlace: {
    style: 'generic',
    unit: 'mm',
    boardOrigin: true,
    includeDnp: false,
    splitSides: false,
    mirrorBottomX: false,
    folder: FOLDERS.pickAndPlace,
  },
  pdf: {
    content: 'schematic',
    layers: ['cu.top', 'silk.top'],
    pagePerLayer: false,
    mirror: false,
    color: 'color',
    paper: 'fit',
    values: false,
    folder: FOLDERS.pdf,
  },
  svg: {
    content: 'schematic',
    layers: ['cu.top', 'silk.top'],
    pagePerLayer: false,
    mirror: false,
    color: 'color',
    paper: 'fit',
    values: false,
    folder: FOLDERS.svg,
  },
  step: { format: 'step', origin: 'drill', includeDnp: false, units: 'mm', folder: FOLDERS.step },
  ipc356: { tentVias: true, folder: FOLDERS.ipc356 },
  ipc2581: { folder: FOLDERS.ipc2581 },
  netlist: { format: 'kicad', folder: FOLDERS.netlist },
  archive: { include: ['gerber', 'drill'], file: '{board}-fabrication.zip' },
};

/** An option of an output, or its default. */
export function option<T>(spec: OutputSpec, key: string, fallback?: T): T {
  const own = (spec.options as Readonly<Record<string, unknown>> | undefined)?.[key];
  if (own !== undefined) {
    return own as T;
  }
  const value = DEFAULTS[spec.kind][key];
  return (value === undefined ? fallback : value) as T;
}

/** The output with one option changed. */
export function withOption(spec: OutputSpec, key: string, value: unknown): OutputSpec {
  return {
    ...spec,
    options: { ...(spec.options as Record<string, unknown> | undefined), [key]: value },
  };
}

/** A new output of a kind. */
export function newOutput(kind: OutputKind, name: string, id: string): OutputSpec {
  return { id, name, kind, enabled: true, options: {} };
}

/** Moves an entry of a list by `delta` places. */
export function moved<T>(list: readonly T[], index: number, delta: number): T[] {
  const target = index + delta;
  if (index < 0 || index >= list.length || target < 0 || target >= list.length) {
    return [...list];
  }
  const next = [...list];
  const [entry] = next.splice(index, 1);
  if (entry !== undefined) {
    next.splice(target, 0, entry);
  }
  return next;
}

/** Board layers offered for Gerber and board prints. */
export function boardLayers(copperCount: number): string[] {
  const copper = ['cu.top'];
  for (let n = 1; n <= copperCount - 2; n += 1) {
    copper.push(`cu.in${String(n)}`);
  }
  if (copperCount > 1) {
    copper.push('cu.bottom');
  }
  const sided = ['mask', 'silk', 'paste', 'adhesive', 'fab', 'courtyard'].flatMap((prefix) => [
    `${prefix}.top`,
    `${prefix}.bottom`,
  ]);
  return [...copper, ...sided, 'outline'];
}

/** The file name and folder of a generated file path. */
export function splitPath(path: string): { readonly folder: string; readonly file: string } {
  const slash = path.lastIndexOf('/');
  return slash < 0
    ? { folder: '', file: path }
    : { folder: path.slice(0, slash), file: path.slice(slash + 1) };
}

/** Extensions the Gerber viewer reads. */
const GERBER_FILE =
  /\.(gbr|ger|gtl|gbl|gts|gbs|gto|gbo|gtp|gbp|gta|gba|gm1|gko|gml|g\d+|drl|xln)$/iu;

/** `true` for Gerber and drill files. */
export function isGerberFile(path: string): boolean {
  return GERBER_FILE.test(path);
}

/** Joins the job folder and a relative output path with the platform's separator. */
export function joinOutput(directory: string, relative: string, separator: string): string {
  const base = directory.endsWith(separator) ? directory.slice(0, -1) : directory;
  return `${base}${separator}${relative.split('/').join(separator)}`;
}
