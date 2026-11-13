/**
 * 3D meshes for the renderer, served by the engine host itself (not by the
 * Rust engine): `mesh.load` reads a GLB file from the library's mesh cache,
 * `mesh.tessellate` turns a STEP file into a GLB file in a worker thread and
 * caches it.
 */
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { extname, isAbsolute, join } from 'node:path';
import { Worker } from 'node:worker_threads';

import type { EngineReply } from '@shared/engine';

import type { MeshJob, MeshJobResult } from './mesh-worker';

const HASH = /^[0-9a-f]{32}$/u;

/** Methods handled here instead of in the engine. */
export function isMeshMethod(method: unknown): boolean {
  return method === 'mesh.load' || method === 'mesh.tessellate';
}

function meshDir(): string | null {
  const dir = process.env['EW_LIBRARY_DIR'];
  return dir === undefined || dir === '' ? null : join(dir, 'meshes');
}

let worker: Worker | null = null;
let nextJob = 1;
const waiting = new Map<number, (result: MeshJobResult) => void>();

function tessellate(file: string): Promise<Uint8Array> {
  if (worker === null) {
    worker = new Worker(join(import.meta.dirname, 'mesh-worker.js'), { env: process.env });
    worker.on('message', (result: MeshJobResult) => {
      waiting.get(result.id)?.(result);
      waiting.delete(result.id);
    });
    worker.on('error', (error: Error) => {
      for (const [id, resolve] of waiting) {
        resolve({ id, error: error.message });
      }
      waiting.clear();
      worker = null;
    });
    worker.unref();
  }
  const active = worker;
  const id = nextJob++;
  return new Promise((resolve, reject) => {
    waiting.set(id, (result) => {
      if ('glb' in result) {
        resolve(result.glb);
      } else {
        reject(new Error(result.error));
      }
    });
    active.postMessage({ id, file } satisfies MeshJob);
  });
}

function fail(
  id: number,
  code: 'notFound' | 'invalidParams' | 'ioError',
  message: string,
): EngineReply {
  return { id, error: { code, message } };
}

/** Handles a mesh request; the reply carries the GLB bytes as `data`. */
export async function handleMeshRequest(
  id: number,
  method: string,
  params: unknown,
): Promise<EngineReply> {
  const dir = meshDir();
  const record = (typeof params === 'object' && params !== null ? params : {}) as Record<
    string,
    unknown
  >;
  if (dir === null) {
    return fail(id, 'notFound', 'no library folder');
  }
  if (method === 'mesh.load') {
    const hash = record['hash'];
    if (typeof hash !== 'string' || !HASH.test(hash)) {
      return fail(id, 'invalidParams', 'not a mesh hash');
    }
    try {
      return { id, result: { data: new Uint8Array(readFileSync(join(dir, `${hash}.glb`))) } };
    } catch {
      return fail(id, 'notFound', `mesh ${hash} is not in the cache`);
    }
  }
  const file = record['file'];
  if (typeof file !== 'string' || !isAbsolute(file) || !/^\.(step|stp)$/iu.test(extname(file))) {
    return fail(id, 'invalidParams', 'file must be an absolute path to a STEP file');
  }
  try {
    // Cached by content, so the engine finds the mesh of a STEP file too
    // (3D exports) and a changed file is tessellated again.
    const key = createHash('sha256').update(readFileSync(file)).digest('hex').slice(0, 32);
    const cached = join(dir, `step-${key}.glb`);
    try {
      return { id, result: { data: new Uint8Array(readFileSync(cached)), cached: true } };
    } catch {
      // Not tessellated yet.
    }
    const glb = await tessellate(file);
    mkdirSync(dir, { recursive: true });
    writeFileSync(`${cached}.part`, glb);
    renameSync(`${cached}.part`, cached);
    return { id, result: { data: glb, cached: false } };
  } catch (error) {
    return fail(id, 'ioError', error instanceof Error ? error.message : String(error));
  }
}
