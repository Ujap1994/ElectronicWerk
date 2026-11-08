/**
 * Replies of the engine's `library.*` methods (crates/ew-library/src/index.rs
 * and crates/ew-core/src/library.rs).
 */
import type { LibraryKind, LibrarySource } from '@shared/library';

export type DuplicateClass = 'exact' | 'near' | 'alias';
export type DuplicateDecision = 'merge' | 'separate' | 'pending';

export interface IndexStatus {
  readonly path: string;
  readonly format: number;
  readonly sources: readonly LibrarySource[];
  readonly counts: Readonly<Record<LibraryKind, number>>;
  readonly visible: Readonly<Record<LibraryKind, number>>;
  readonly groups: Readonly<Record<DuplicateClass, number>>;
  readonly pending: number;
  readonly meshes: number;
  readonly bytes: number;
  /** The library release the index came from, if it was downloaded. */
  readonly release: string | null;
}

export interface BuildProgress {
  readonly stage: string;
  readonly done: number;
  readonly total: number;
}

export interface LibraryStatus {
  readonly dir: string | null;
  readonly installed: boolean;
  readonly building: BuildProgress | null;
  readonly status: IndexStatus | null;
}

export interface SearchQuery {
  readonly text: string;
  readonly kind?: LibraryKind;
  readonly libraries?: readonly string[];
  readonly mounting?: string;
  readonly withModel?: boolean;
  readonly duplicates?: boolean;
  readonly includeMerged?: boolean;
  readonly offset: number;
  readonly limit: number;
}

export interface EntrySummary {
  readonly id: number;
  readonly kind: LibraryKind;
  readonly library: string;
  readonly name: string;
  readonly description: string;
  readonly value: string;
  readonly package: string;
  readonly pins: number;
  readonly mounting: string | null;
  readonly mergedInto: number | null;
  readonly groups: number;
}

export interface SearchResult {
  readonly total: number;
  readonly items: readonly EntrySummary[];
  readonly elapsedMs: number;
}

export interface Facet {
  readonly library: string;
  readonly count: number;
}

export interface GroupMember {
  readonly id: number;
  readonly name: string;
}

export interface GroupInfo {
  readonly key: string;
  readonly kind: LibraryKind;
  readonly class: DuplicateClass;
  readonly reason: string;
  readonly decision: DuplicateDecision;
  readonly canonical: number;
  readonly members: readonly GroupMember[];
}

export interface GroupPage {
  readonly total: number;
  readonly groups: readonly GroupInfo[];
}

/** Model placement of a footprint (from its definition). */
export interface ModelRef {
  readonly path: string;
  /** Offset in nanometres (z up, away from the board). */
  readonly offset?: { readonly x: number; readonly y: number; readonly z: number };
  /** Rotation about x, y and z in degrees. */
  readonly rotation?: readonly number[];
  /** Scale factors. */
  readonly scale?: readonly number[];
  readonly hidden?: boolean;
}

export interface EntryDetails extends EntrySummary {
  readonly keywords: string;
  readonly mpn: string;
  readonly manufacturer: string;
  readonly datasheet: string;
  readonly params: string;
  readonly units: number;
  readonly size: readonly [number, number, number] | null;
  readonly model: string | null;
  readonly modelId: number | null;
  readonly file: string | null;
  readonly sourceFile: string | null;
  readonly mesh: string | null;
  readonly aliases: readonly string[];
  readonly duplicateGroups: readonly GroupInfo[];
  readonly source: LibrarySource;
  readonly definition: {
    readonly footprint?: string | null;
    readonly models?: readonly ModelRef[];
    readonly units?: readonly unknown[];
  } | null;
}

/** `library` events of the engine. */
export type LibraryEvent =
  | {
      readonly type: 'library';
      readonly action: 'progress';
      readonly stage: string;
      readonly done: number;
      readonly total: number;
    }
  | { readonly type: 'library'; readonly action: 'built'; readonly status: LibraryStatus }
  | { readonly type: 'library'; readonly action: 'failed'; readonly message: string }
  | { readonly type: 'library'; readonly action: 'changed' };

export function isLibraryEvent(value: unknown): value is LibraryEvent {
  return (
    typeof value === 'object' &&
    value !== null &&
    (value as { type?: unknown }).type === 'library' &&
    typeof (value as { action?: unknown }).action === 'string'
  );
}
