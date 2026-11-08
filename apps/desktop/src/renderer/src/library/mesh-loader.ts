/**
 * Finds the GLB mesh of a library model: from the mesh cache, by
 * tessellating its STEP file (libraries built from local KiCad folders), or
 * by downloading the mesh pack of its 3D library (downloaded libraries).
 */
import type { LibraryRelease } from '@shared/library';

import type { EngineClient } from '../engine-client';
import type { EntryDetails } from './types';

export type MeshStatus =
  | { readonly kind: 'loading' }
  | { readonly kind: 'tessellating' }
  | {
      readonly kind: 'downloading';
      readonly library: string;
      readonly size: number;
      readonly percent: number;
    };

const TESSELLATE_MS = 300_000;
const INSTALL_MS = 120_000;

/** Packs downloaded in this session (one download per pack at a time). */
const downloads = new Map<string, Promise<void>>();

async function loadCached(engine: EngineClient, hash: string): Promise<Uint8Array | null> {
  try {
    const reply = await engine.call<{ data: Uint8Array }>('mesh.load', { hash });
    return reply.data;
  } catch {
    return null;
  }
}

async function downloadPack(
  engine: EngineClient,
  release: LibraryRelease,
  library: string,
  onStatus: (status: MeshStatus) => void,
): Promise<boolean> {
  const pack = release.packs.find((p) => p.kind === 'meshes' && p.library === library);
  if (pack === undefined) {
    return false;
  }
  let running = downloads.get(pack.file);
  if (running === undefined) {
    running = (async () => {
      const off = window.ew.library.onProgress((progress) => {
        if (progress.file === pack.file) {
          onStatus({
            kind: 'downloading',
            library,
            size: pack.size,
            percent: Math.floor((progress.received / Math.max(1, progress.total)) * 100),
          });
        }
      });
      try {
        onStatus({ kind: 'downloading', library, size: pack.size, percent: 0 });
        const file = await window.ew.library.download(release.tag, pack.file);
        await engine.call('library.install', { file, remove: true }, INSTALL_MS);
      } finally {
        off();
      }
    })();
    downloads.set(pack.file, running);
    running.finally(() => downloads.delete(pack.file)).catch(() => undefined);
  }
  await running;
  return true;
}

/**
 * Loads the mesh of a model entry. `release` is the library release the
 * index came from (for mesh packs), if any.
 */
export async function loadMesh(
  engine: EngineClient,
  model: EntryDetails,
  release: string | null,
  onStatus: (status: MeshStatus) => void,
): Promise<Uint8Array> {
  onStatus({ kind: 'loading' });
  if (model.mesh !== null) {
    const cached = await loadCached(engine, model.mesh);
    if (cached !== null) {
      return cached;
    }
  }
  if (model.sourceFile !== null) {
    onStatus({ kind: 'tessellating' });
    const reply = await engine.call<{ data: Uint8Array }>(
      'mesh.tessellate',
      { file: model.sourceFile },
      TESSELLATE_MS,
    );
    return reply.data;
  }
  if (model.mesh !== null && release !== null) {
    const manifest = await window.ew.library.release(release);
    if (await downloadPack(engine, manifest, model.library, onStatus)) {
      const cached = await loadCached(engine, model.mesh);
      if (cached !== null) {
        return cached;
      }
    }
  }
  throw new Error('unavailable');
}
