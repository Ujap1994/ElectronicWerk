/**
 * Datasheet files for the part generator, served by the engine host
 * itself: `datasheet.file` reads a PDF file for the viewer in the renderer
 * (which may not read files), `datasheet.previewModel` keeps the STEP text
 * of a preview model in a temporary file, so `mesh.tessellate` can show it.
 */
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, isAbsolute, join } from 'node:path';

import type { EngineReply } from '@shared/engine';

/** Largest datasheet the viewer loads (bytes). */
const MAX_PDF = 256 * 1024 * 1024;
/** Largest preview model (bytes). */
const MAX_STEP = 32 * 1024 * 1024;

/** Methods handled here instead of in the engine. */
export function isDatasheetMethod(method: unknown): boolean {
  return method === 'datasheet.file' || method === 'datasheet.previewModel';
}

function fail(
  id: number,
  code: 'notFound' | 'invalidParams' | 'ioError',
  message: string,
): EngineReply {
  return { id, error: { code, message } };
}

/** Handles a datasheet request. */
export function handleDatasheetRequest(id: number, method: string, params: unknown): EngineReply {
  const record = (typeof params === 'object' && params !== null ? params : {}) as Record<
    string,
    unknown
  >;
  if (method === 'datasheet.file') {
    const path = record['path'];
    if (typeof path !== 'string' || !isAbsolute(path) || extname(path).toLowerCase() !== '.pdf') {
      return fail(id, 'invalidParams', 'path must be an absolute path to a PDF file');
    }
    try {
      if (statSync(path).size > MAX_PDF) {
        return fail(id, 'invalidParams', 'the datasheet is too large');
      }
      return { id, result: { data: new Uint8Array(readFileSync(path)) } };
    } catch (error) {
      return fail(id, 'notFound', error instanceof Error ? error.message : String(error));
    }
  }
  const step = record['step'];
  if (typeof step !== 'string' || !step.startsWith('ISO-10303-21;') || step.length > MAX_STEP) {
    return fail(id, 'invalidParams', 'step must be the text of a STEP file');
  }
  try {
    const hash = createHash('sha256').update(step).digest('hex').slice(0, 32);
    const dir = join(tmpdir(), 'electronicwerk-part-preview');
    mkdirSync(dir, { recursive: true });
    const file = join(dir, `${hash}.step`);
    writeFileSync(file, step);
    return { id, result: { file } };
  } catch (error) {
    return fail(id, 'ioError', error instanceof Error ? error.message : String(error));
  }
}
