/**
 * Worker thread of the engine host: tessellates STEP files with
 * OpenCASCADE (occt-import-js, WebAssembly) into GLB files, so a slow model
 * never blocks engine requests.
 */
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join } from 'node:path';
import { parentPort } from 'node:worker_threads';

import { OCCT_PARAMS, occtToGlb, type OcctResult } from '@shared/mesh';

interface Occt {
  ReadStepFile: (data: Uint8Array, params: unknown) => OcctResult;
}

/** A request from the host. */
export interface MeshJob {
  readonly id: number;
  readonly file: string;
}

/** The reply to a {@link MeshJob}. */
export type MeshJobResult =
  | { readonly id: number; readonly glb: Uint8Array }
  | { readonly id: number; readonly error: string };

let occt: Promise<Occt> | undefined;

function load(): Promise<Occt> {
  if (occt === undefined) {
    const dir = process.env['EW_OCCT_DIR'] ?? '';
    const require = createRequire(import.meta.url);
    const factory = require(join(dir, 'occt-import-js.js')) as () => Promise<Occt>;
    occt = factory();
  }
  return occt;
}

parentPort?.on('message', (job: MeshJob) => {
  void (async () => {
    try {
      const module = await load();
      const result = module.ReadStepFile(new Uint8Array(readFileSync(job.file)), OCCT_PARAMS);
      if (!result.success) {
        throw new Error('OpenCASCADE could not read the file');
      }
      const converted = occtToGlb(result.meshes);
      if (converted === null) {
        throw new Error('the model has no triangles');
      }
      parentPort?.postMessage({ id: job.id, glb: converted.glb } satisfies MeshJobResult, [
        converted.glb.buffer as ArrayBuffer,
      ]);
    } catch (error) {
      parentPort?.postMessage({
        id: job.id,
        error: error instanceof Error ? error.message : String(error),
      } satisfies MeshJobResult);
    }
  })();
});
