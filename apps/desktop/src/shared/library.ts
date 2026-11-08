/**
 * Types of the base library (ADR-0005), shared by the main process and the
 * renderer, plus the parsing of library releases and their manifests.
 */

/** Kind of a library entry. */
export type LibraryKind = 'symbol' | 'footprint' | 'model';

/** One pack of a library release. */
export interface LibraryPack {
  readonly file: string;
  readonly kind: 'index' | 'meshes';
  /** The 3D library of a mesh pack. */
  readonly library?: string;
  readonly size: number;
  readonly sha256: string;
  readonly entries: number;
}

/** Source of library data (KiCad and its license). */
export interface LibrarySource {
  readonly name: string;
  readonly version: string;
  readonly license: string;
  readonly url: string;
}

/** A library release on GitHub (`library-…` tag) with its manifest. */
export interface LibraryRelease {
  readonly tag: string;
  readonly name: string;
  readonly version: string;
  readonly sources: readonly LibrarySource[];
  /** Entries by kind. */
  readonly counts: Readonly<Record<LibraryKind, number>>;
  readonly packs: readonly LibraryPack[];
}

/** Download progress, sent by the main process. */
export interface LibraryDownloadProgress {
  readonly file: string;
  readonly received: number;
  readonly total: number;
}

/** Tags of library releases. */
export const LIBRARY_TAG = /^library-[\w.-]{1,64}$/u;
/** File names of packs. */
export const PACK_FILE = /^[\w.-]{1,120}\.zip$/u;
const SHA256 = /^[0-9a-f]{64}$/u;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function parsePack(raw: unknown): LibraryPack | null {
  if (!isRecord(raw)) {
    return null;
  }
  const { file, kind, library, size, sha256, entries } = raw;
  if (
    typeof file !== 'string' ||
    !PACK_FILE.test(file) ||
    (kind !== 'index' && kind !== 'meshes') ||
    typeof size !== 'number' ||
    !Number.isSafeInteger(size) ||
    size < 0 ||
    typeof sha256 !== 'string' ||
    !SHA256.test(sha256) ||
    typeof entries !== 'number'
  ) {
    return null;
  }
  return {
    file,
    kind,
    ...(typeof library === 'string' ? { library } : {}),
    size,
    sha256,
    entries,
  };
}

function parseSource(raw: unknown): LibrarySource | null {
  if (!isRecord(raw)) {
    return null;
  }
  const { name, version, license, url } = raw;
  return typeof name === 'string' &&
    typeof version === 'string' &&
    typeof license === 'string' &&
    typeof url === 'string'
    ? { name, version, license, url }
    : null;
}

function count(counts: unknown, kind: LibraryKind): number {
  const value = isRecord(counts) ? counts[kind] : undefined;
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : 0;
}

/** Validates a `manifest.json` of a library release. */
export function parseManifest(tag: string, raw: unknown): LibraryRelease | null {
  if (!LIBRARY_TAG.test(tag) || !isRecord(raw) || raw['format'] !== 1) {
    return null;
  }
  const { name, version, sources, packs, counts } = raw;
  if (typeof name !== 'string' || typeof version !== 'string' || !Array.isArray(packs)) {
    return null;
  }
  const parsedPacks = packs.map(parsePack);
  if (parsedPacks.some((pack) => pack === null) || !parsedPacks.some((p) => p?.kind === 'index')) {
    return null;
  }
  return {
    tag,
    name,
    version,
    sources: Array.isArray(sources)
      ? sources.map(parseSource).filter((s): s is LibrarySource => s !== null)
      : [],
    counts: {
      symbol: count(counts, 'symbol'),
      footprint: count(counts, 'footprint'),
      model: count(counts, 'model'),
    },
    packs: parsedPacks.filter((p): p is LibraryPack => p !== null),
  };
}

/** The newest library tag in a GitHub releases list (`/releases`). */
export function newestLibraryTag(raw: unknown): string | null {
  if (!Array.isArray(raw)) {
    return null;
  }
  for (const release of raw) {
    if (!isRecord(release) || release['draft'] === true) {
      continue;
    }
    const tag = release['tag_name'];
    if (typeof tag === 'string' && LIBRARY_TAG.test(tag)) {
      // The API lists releases newest first.
      return tag;
    }
  }
  return null;
}

/** Download URL of a file of a library release. */
export function releaseFileUrl(tag: string, file: string): string {
  return `https://github.com/Ujap1994/ElectronicWerk/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(file)}`;
}

/** Human-readable size (1 decimal, MB above 1 MB). */
export function formatBytes(bytes: number): string {
  if (bytes >= 1e9) {
    return `${(bytes / 1e9).toFixed(1)} GB`;
  }
  if (bytes >= 1e6) {
    return `${(bytes / 1e6).toFixed(1)} MB`;
  }
  return `${Math.max(1, Math.round(bytes / 1e3)).toString()} kB`;
}
