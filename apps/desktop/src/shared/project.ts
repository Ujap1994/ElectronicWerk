/**
 * Types of the project-related engine API (see crates/ew-core) and helpers
 * shared by the main process and the UI.
 */

/** A length in nanometres. */
export type Nm = number;

export type DocumentKind = 'schematic' | 'pcb' | 'library' | 'rules' | 'outputJob';

/** Document kinds in project tree order. */
export const DOCUMENT_KINDS: readonly DocumentKind[] = [
  'schematic',
  'pcb',
  'library',
  'rules',
  'outputJob',
];

export type Units = 'mm' | 'mil' | 'inch';
export const UNITS: readonly Units[] = ['mm', 'mil', 'inch'];

export interface HistoryState {
  readonly dirty: boolean;
  readonly canUndo: boolean;
  readonly canRedo: boolean;
  readonly undoLabel: string | null;
  readonly redoLabel: string | null;
  readonly undoCount: number;
  readonly redoCount: number;
}

export interface DocumentStats {
  readonly objects: number;
  readonly symbols?: number;
  readonly wires?: number;
  readonly labels?: number;
  /** Schematic: ids of the child sheets placed on this sheet. */
  readonly children?: readonly string[];
  readonly page?: readonly [Nm, Nm];
  readonly footprints?: number;
  readonly tracks?: number;
  readonly vias?: number;
  readonly zones?: number;
  readonly nets?: number;
  readonly copperLayers?: number;
  readonly thickness?: Nm;
  readonly boardSize?: readonly [Nm, Nm];
  readonly netClasses?: number;
  readonly outputs?: number;
}

export interface DocumentSummary {
  readonly id: string;
  readonly kind: DocumentKind;
  readonly name: string;
  readonly path: string;
  readonly dirty: boolean;
  readonly stats: DocumentStats;
}

export interface RecoveryInfo {
  readonly commits: number;
  readonly entries: number;
  readonly modified: string | null;
  readonly matchesFiles: boolean;
}

export interface ProjectSettings {
  readonly units: Units;
  readonly schematicGrid: Nm;
  readonly boardGrid: Nm;
}

/** Result of `project.info`, `project.open` and friends. */
export interface ProjectSnapshot {
  readonly id: string;
  readonly name: string;
  readonly revision: string;
  readonly description: string;
  readonly variables: Readonly<Record<string, string>>;
  readonly path: string;
  readonly manifestPath: string;
  readonly rootSchematic: string | null;
  readonly settings: ProjectSettings;
  readonly documents: readonly DocumentSummary[];
  readonly history: HistoryState;
  readonly needsUpgrade: boolean;
  readonly recovery: RecoveryInfo | null;
  readonly journalError: string | null;
}

export interface BoardOptions {
  readonly width: Nm;
  readonly height: Nm;
  readonly layers: number;
  readonly thickness: Nm;
}

export type TemplateId = 'empty' | 'board-2l' | 'board-4l' | 'arduino-shield' | 'demo';

export interface TemplateInfo {
  readonly id: TemplateId;
  readonly board: BoardOptions | null;
  readonly fixedSize: boolean;
  readonly example: boolean;
}

export interface FileEntry {
  readonly path: string;
  readonly size: number;
  readonly category: 'manifest' | 'document' | 'generated' | 'other';
}

export interface ValidationIssue {
  readonly severity: 'error' | 'warning' | 'info';
  readonly code: string;
  readonly message: string;
  readonly path?: string;
  readonly object?: string;
  /** Values in the message, for translated messages built from `code`. */
  readonly params?: Readonly<Record<string, string>>;
}

export interface ValidationReport {
  readonly issues: readonly ValidationIssue[];
  readonly errors: number;
  readonly warnings: number;
  readonly documents: number;
  readonly objects: number;
}

export interface ProbeResult {
  readonly exists: boolean;
  readonly valid?: boolean;
  readonly name?: string;
  readonly path?: string;
  readonly manifestPath?: string;
  readonly hasRecovery?: boolean;
}

export interface ArchiveReport {
  readonly path: string;
  readonly files: number;
  readonly bytes: number;
}

// --- Imports ----------------------------------------------------------------

export interface ImportFinding {
  readonly severity: 'info' | 'warning' | 'error';
  readonly code: string;
  readonly message: string;
  readonly file?: string;
  readonly count: number;
}

/** The report of `import.kicad` (see crates/ew-kicad/src/report.rs). */
export interface ImportReport {
  readonly source: string;
  readonly versions: Readonly<Record<string, string>>;
  readonly counts: Readonly<Record<string, number>>;
  readonly findings: readonly ImportFinding[];
}

export interface KicadProbe {
  readonly name: string;
  readonly folder: string;
  readonly project: boolean;
  readonly schematic: boolean;
  readonly board: boolean;
}

export interface NetlistComparison {
  readonly nets: number;
  readonly nodes: number;
  readonly matchingNets: number;
  readonly sameNames: number;
  readonly missingComponents: readonly string[];
  readonly differences: readonly {
    readonly reference: string;
    readonly pin: string;
    readonly expected: string | null;
    readonly actual: string | null;
    /** The netlist leaves the pin unconnected, but the board connects it. */
    readonly unconnected: boolean;
  }[];
}

/** The result of `import.netlist`. */
export interface NetlistResult {
  readonly format: 'kicad' | 'xml' | 'csv';
  readonly nets: number;
  readonly components: number;
  readonly comparison: NetlistComparison;
  readonly changedPads: number;
  readonly newNets: number;
  readonly missingFootprints: readonly string[];
  readonly extraFootprints: readonly string[];
  readonly applied: boolean;
}

/** Codes of import findings (crates/ew-kicad). */
export const IMPORT_CODES: ReadonlySet<string> = new Set([
  'alternate-body-style',
  'already-in-library',
  'chamfered-pad',
  'custom-pad',
  'custom-rules',
  'erc-exclusions',
  'external-library',
  'footprint-zone',
  'missing-parent',
  'missing-sheet',
  'missing-symbol',
  'no-board',
  'no-designator',
  'no-project-file',
  'no-schematic',
  'reused-sheet',
  'text-box',
  'unknown-layer',
  'unreadable-file',
  'unsupported-item',
  'vertical-slot',
  'zone-layer',
]);

/** Events the engine streams after each call. */
export type EngineEvent =
  | {
      readonly type: 'changed';
      readonly changeSet: {
        readonly seq: number;
        readonly cause: 'commit' | 'undo' | 'redo' | 'recover';
        readonly label: string;
        readonly truncated: boolean;
        readonly history: HistoryState;
      };
    }
  | { readonly type: 'project'; readonly action: string; readonly path: string | null };

export function isEngineEvent(value: unknown): value is EngineEvent {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const type = (value as { type?: unknown }).type;
  return type === 'changed' || type === 'project';
}

// --- Units ------------------------------------------------------------------

const NM_PER_UNIT: Readonly<Record<Units, number>> = { mm: 1e6, mil: 25_400, inch: 25_400_000 };
const DIGITS: Readonly<Record<Units, number>> = { mm: 3, mil: 1, inch: 4 };
const SUFFIX: Readonly<Record<Units, string>> = { mm: 'mm', mil: 'mil', inch: 'in' };

/** Unit symbol, for example `mm`. */
export function unitSymbol(units: Units): string {
  return SUFFIX[units];
}

/** Formats a length, for example `1.6 mm` or `63 mil` (trailing zeros removed). */
export function formatLength(nm: Nm, units: Units, locale = 'en'): string {
  const value = nm / NM_PER_UNIT[units];
  const text = new Intl.NumberFormat(locale, {
    maximumFractionDigits: DIGITS[units],
    useGrouping: false,
  }).format(value);
  return `${text} ${SUFFIX[units]}`;
}

/** Formats a size as `100 × 80 mm`. */
export function formatSize(width: Nm, height: Nm, units: Units, locale = 'en'): string {
  const w = formatLength(width, units, locale).replace(/\s\S+$/u, '');
  return `${w} × ${formatLength(height, units, locale)}`;
}

/**
 * Parses a length typed by the user in the given units (comma or point as
 * decimal separator). Returns nanometres, or `null` for invalid input.
 */
export function parseLength(text: string, units: Units): Nm | null {
  const normalized = text.trim().replace(',', '.');
  if (!/^-?\d+(?:\.\d+)?$/u.test(normalized)) {
    return null;
  }
  const value = Number(normalized);
  return Number.isFinite(value) ? Math.round(value * NM_PER_UNIT[units]) : null;
}

/** Converts nanometres to a plain number in the given units (for inputs). */
export function toUnits(nm: Nm, units: Units): string {
  const factor = 10 ** DIGITS[units];
  return String(Math.round((nm / NM_PER_UNIT[units]) * factor) / factor);
}

// --- Names and paths --------------------------------------------------------

export type NameProblem = 'empty' | 'tooLong' | 'invalid';

/** Number of Unicode code points (what the engine counts as characters). */
function codePoints(text: string): number {
  let count = 0;
  for (const char of text) {
    count += char === '' ? 0 : 1;
  }
  return count;
}

/** The first `limit` code points of a text. */
function takeCodePoints(text: string, limit: number): string {
  let result = '';
  let count = 0;
  for (const char of text) {
    if (count >= limit) {
      break;
    }
    result += char;
    count += 1;
  }
  return result;
}

/** Checks a project or document name the same way the engine does. */
export function checkName(name: string): NameProblem | null {
  const trimmed = name.trim();
  if (trimmed === '') {
    return 'empty';
  }
  if (codePoints(trimmed) > 100) {
    return 'tooLong';
  }
  // eslint-disable-next-line no-control-regex -- control characters are exactly what we reject
  if (/[\u0000-\u001f\u007f]/u.test(name)) {
    return 'invalid';
  }
  return null;
}

const RESERVED = /^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])$/iu;

/** Turns a project name into a safe folder name (mirrors the engine). */
export function sanitizeFolderName(name: string): string {
  // eslint-disable-next-line no-control-regex -- control characters are replaced on purpose
  const cleaned = takeCodePoints(name.replace(/[\u0000-\u001f\u007f<>:"/\\|?*]/gu, '_'), 100);
  const trimmed = cleaned.replace(/^[.\s]+|[.\s]+$/gu, '');
  if (trimmed === '') {
    return 'project';
  }
  const stem = trimmed.split('.')[0] ?? trimmed;
  return RESERVED.test(stem) ? `_${trimmed}` : trimmed;
}

/** Joins a folder and a name with the platform's separator. */
export function joinPath(folder: string, name: string, platform: string): string {
  const separator = platform === 'win32' ? '\\' : '/';
  const base = folder.replace(/[\\/]+$/u, '');
  return base === '' ? `${separator}${name}` : `${base}${separator}${name}`;
}

/** The last segment of a path (either separator). */
export function baseName(path: string): string {
  const parts = path.split(/[\\/]/u).filter((part) => part !== '');
  return parts.at(-1) ?? path;
}

/** The folder part of a path (either separator). */
export function dirName(path: string): string {
  const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return index <= 0 ? path.slice(0, index + 1) : path.slice(0, index);
}

/** `true` for a valid user variable name (letters, digits, `_`). */
export function isVariableName(name: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]{0,63}$/u.test(name);
}

/** Variables provided by ElectronicWerk itself. */
export const BUILTIN_VARIABLES = [
  'PROJECT_NAME',
  'REVISION',
  'CURRENT_DATE',
  'SHEET_NAME',
  'FILENAME',
] as const;

/** Codes of the engine's validation findings that have a translation. */
export const VALIDATION_CODES: ReadonlySet<string> = new Set([
  'unreadable-manifest',
  'invalid-document-path',
  'duplicate-document',
  'document-id-mismatch',
  'unreadable-document',
  'unlisted-file',
  'coordinate-out-of-range',
  'invalid-variable-name',
  'builtin-variable-shadowed',
  'duplicate-id',
  'root-schematic-missing',
  'no-root-schematic',
  'sheet-target-missing',
  'sheet-cycle',
  'unplaced-sheet',
  'symbol-not-in-library',
  'footprint-not-in-library',
  'duplicate-designator',
  'zero-length-wire',
  'empty-label',
  'stackup-no-copper',
  'no-board-outline',
  'unknown-net-class',
  'unknown-net',
  'layer-not-in-stackup',
  'invalid-width',
  'invalid-via',
  'zone-outline-too-small',
  'duplicate-net-class',
  'invalid-net-class',
  'missing-default-net-class',
  'output-source-missing',
]);
