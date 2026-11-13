/**
 * Loads the GLB mesh of a model the engine found: a STEP file is
 * tessellated (and cached) by the engine host, a base library model comes
 * from the mesh cache or its downloaded mesh pack.
 */
import type { EngineClient } from '../engine-client';
import { loadMesh } from '../library/mesh-loader';
import type { EntryDetails } from '../library/types';
import type { ModelSource } from './types';

const TESSELLATE_MS = 300_000;

/** The GLB of a model, or `null` if it cannot be shown. */
export async function loadModelGlb(
  engine: EngineClient,
  source: ModelSource | undefined,
  release: string | null,
): Promise<Uint8Array | null> {
  if (source === undefined) {
    return null;
  }
  // A library mesh that is already cached is the quickest.
  if (source.mesh !== undefined) {
    try {
      const reply = await engine.call<{ data: Uint8Array }>('mesh.load', { hash: source.mesh });
      return reply.data;
    } catch {
      // Not downloaded yet.
    }
  }
  if (source.file !== undefined && source.format === 'step') {
    try {
      const reply = await engine.call<{ data: Uint8Array }>(
        'mesh.tessellate',
        { file: source.file },
        TESSELLATE_MS,
      );
      return reply.data;
    } catch {
      // Fall back to the library.
    }
  }
  if (source.entry !== undefined) {
    try {
      const details = await engine.call<EntryDetails>('library.get', { id: source.entry });
      return await loadMesh(engine, details, release, () => undefined);
    } catch {
      return null;
    }
  }
  return null;
}
