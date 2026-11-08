/**
 * Tessellates the STEP models of a KiCad 3D library into GLB files for the
 * library build (ADR-0005):
 *
 *   node apps/desktop/scripts/tessellate.ts <models> <out> [--jobs N] [--filter TEXT]
 *
 * `<models>` holds `<Library>.3dshapes/<name>.step`; the GLB files go to
 * `<out>/<Library>.3dshapes/<name>.glb`. Files that are newer than their
 * STEP file are kept, so a second run only does what changed. A summary is
 * written to `<out>/tessellate-report.json`.
 *
 * Runs directly with Node 24 (type stripping); every worker thread loads
 * OpenCASCADE (occt-import-js, WebAssembly) once.
 */
import { mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { cpus } from 'node:os';
import { dirname, join, relative } from 'node:path';
import { createRequire } from 'node:module';
import { isMainThread, parentPort, Worker } from 'node:worker_threads';

import { OCCT_PARAMS, occtToGlb, type OcctResult } from '../src/shared/mesh.ts';

interface Job {
  readonly source: string;
  readonly target: string;
}

interface Done {
  readonly source: string;
  readonly ok: boolean;
  readonly error?: string;
  readonly triangles?: number;
  readonly bytes?: number;
  readonly ms: number;
}

/** Restart a worker after this many files: WebAssembly memory never shrinks. */
const FILES_PER_WORKER = 100;
/** Give up on a file after this many milliseconds. */
const TIMEOUT_MS = 180_000;

interface Occt {
  ReadStepFile: (data: Uint8Array, params: unknown) => OcctResult;
}

async function workerMain(): Promise<void> {
  const require = createRequire(import.meta.url);
  const factory = require('occt-import-js') as () => Promise<Occt>;
  const occt = await factory();
  parentPort?.on('message', (job: Job) => {
    const started = performance.now();
    try {
      const result = occt.ReadStepFile(new Uint8Array(readFileSync(job.source)), OCCT_PARAMS);
      if (!result.success) {
        throw new Error('OpenCASCADE could not read the file');
      }
      const converted = occtToGlb(result.meshes);
      if (converted === null) {
        throw new Error('the model has no triangles');
      }
      mkdirSync(dirname(job.target), { recursive: true });
      writeFileSync(job.target, converted.glb);
      parentPort?.postMessage({
        source: job.source,
        ok: true,
        triangles: converted.stats.triangles,
        bytes: converted.glb.byteLength,
        ms: performance.now() - started,
      } satisfies Done);
    } catch (error) {
      parentPort?.postMessage({
        source: job.source,
        ok: false,
        error: error instanceof Error ? error.message : String(error),
        ms: performance.now() - started,
      } satisfies Done);
    }
  });
  parentPort?.postMessage('ready');
}

function listJobs(
  models: string,
  out: string,
  filter: string | undefined,
): { jobs: Job[]; kept: number } {
  const jobs: Job[] = [];
  let kept = 0;
  for (const folder of readdirSync(models).sort()) {
    if (!folder.endsWith('.3dshapes')) {
      continue;
    }
    for (const file of readdirSync(join(models, folder)).sort()) {
      if (!/\.(step|stp)$/iu.test(file) || (filter !== undefined && !file.includes(filter))) {
        continue;
      }
      const source = join(models, folder, file);
      const target = join(out, folder, file.replace(/\.(step|stp)$/iu, '.glb'));
      try {
        if (statSync(target).mtimeMs >= statSync(source).mtimeMs) {
          kept++;
          continue;
        }
      } catch {
        // Not tessellated yet.
      }
      jobs.push({ source, target });
    }
  }
  return { jobs, kept };
}

function log(line: string): void {
  process.stdout.write(`${line}\n`);
}

async function main(): Promise<void> {
  const args = process.argv.slice(2);
  const option = (name: string): string | undefined => {
    const index = args.indexOf(name);
    return index === -1 ? undefined : args.splice(index, 2)[1];
  };
  const jobsOption = Number(option('--jobs') ?? cpus().length);
  const filter = option('--filter');
  const [models, out] = args;
  if (models === undefined || out === undefined) {
    process.stderr.write('usage: node tessellate.ts <models> <out> [--jobs N] [--filter TEXT]\n');
    process.exit(2);
  }
  const { jobs, kept } = listJobs(models, out, filter);
  const started = performance.now();
  const results: Done[] = [];
  let next = 0;
  const workers = Math.max(1, Math.min(jobsOption, jobs.length));
  log(`${jobs.length} models to tessellate (${kept} up to date) with ${workers} workers`);

  const runWorker = (): Promise<void> =>
    new Promise((resolve) => {
      let count = 0;
      let current: Job | undefined;
      let timer: NodeJS.Timeout | undefined;
      const worker = new Worker(new URL(import.meta.url));
      const finish = (): void => {
        if (timer !== undefined) {
          clearTimeout(timer);
        }
        void worker.terminate().then(() => {
          resolve(next < jobs.length ? runWorker() : undefined);
        });
      };
      const send = (): void => {
        current = jobs[next++];
        if (current === undefined || count >= FILES_PER_WORKER) {
          if (current !== undefined) {
            next--;
          }
          finish();
          return;
        }
        count++;
        const job = current;
        timer = setTimeout(() => {
          results.push({ source: job.source, ok: false, error: 'timed out', ms: TIMEOUT_MS });
          worker.removeAllListeners('message');
          finish();
        }, TIMEOUT_MS);
        worker.postMessage(job);
      };
      worker.on('message', (message: Done | 'ready') => {
        if (message !== 'ready') {
          if (timer !== undefined) {
            clearTimeout(timer);
          }
          results.push(message);
          if (results.length % 100 === 0 || results.length === jobs.length) {
            const seconds = (performance.now() - started) / 1000;
            log(`${results.length}/${jobs.length} (${seconds.toFixed(0)} s)`);
          }
        }
        send();
      });
      worker.on('error', (error: Error) => {
        if (current !== undefined) {
          results.push({ source: current.source, ok: false, error: error.message, ms: 0 });
        }
        finish();
      });
    });

  await Promise.all(Array.from({ length: workers }, () => runWorker()));
  const failed = results.filter((r) => !r.ok);
  const report = {
    models: jobs.length + kept,
    tessellated: results.length - failed.length,
    upToDate: kept,
    failed: failed.map((r) => ({ file: relative(models, r.source), error: r.error })),
    triangles: results.reduce((sum, r) => sum + (r.triangles ?? 0), 0),
    bytes: results.reduce((sum, r) => sum + (r.bytes ?? 0), 0),
    seconds: Math.round((performance.now() - started) / 100) / 10,
    slowest: [...results]
      .sort((a, b) => b.ms - a.ms)
      .slice(0, 10)
      .map((r) => ({ file: relative(models, r.source), ms: Math.round(r.ms) })),
  };
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, 'tessellate-report.json'), `${JSON.stringify(report, null, 2)}\n`);
  log(
    `done in ${report.seconds} s: ${report.tessellated} tessellated, ${failed.length} failed, ` +
      `${(report.bytes / 1e6).toFixed(1)} MB`,
  );
}

if (isMainThread) {
  await main();
} else {
  await workerMain();
}
