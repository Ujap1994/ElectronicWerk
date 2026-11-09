import { Worker } from 'node:worker_threads';

import { app, type BrowserWindow } from 'electron';

import { paths } from './paths';

/** Command-line flag that runs the packaged-app smoke test. */
export const SMOKE_TEST_FLAG = '--smoke-test';

const POLL_INTERVAL_MS = 250;

/**
 * Tessellates a small STEP file with the bundled mesh worker and
 * OpenCASCADE, as the engine host does; resolves to the GLB size.
 */
function checkMeshes(timeoutMs: number): Promise<number> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(paths.meshWorker, {
      env: { ...process.env, EW_OCCT_DIR: paths.occtDir() },
    });
    const timer = setTimeout(() => {
      void worker.terminate();
      reject(new Error('mesh worker timed out'));
    }, timeoutMs);
    worker.on('message', (result: { glb?: Uint8Array; error?: string }) => {
      clearTimeout(timer);
      void worker.terminate();
      if (result.glb === undefined) {
        reject(new Error(result.error ?? 'no mesh'));
      } else {
        resolve(result.glb.byteLength);
      }
    });
    worker.on('error', (error: Error) => {
      clearTimeout(timer);
      reject(error);
    });
    worker.postMessage({ id: 1, file: paths.smokeModel() });
  });
}

/**
 * Smoke test for packaged builds (used by the release pipeline, where
 * Playwright cannot attach because the Electron fuses disable debugging).
 * Waits until the UI shows a working engine connection and the mesh worker
 * can tessellate a STEP model, prints `SMOKE_OK` and exits with code 0, or
 * exits with code 1 on an error or after the timeout. The mesh check has a
 * longer timeout of its own: compiling OpenCASCADE's WebAssembly takes a
 * while on slow machines and under Rosetta.
 */
export function runSmokeTest(
  window: BrowserWindow,
  timeoutMs = 30_000,
  meshTimeoutMs = 180_000,
): void {
  const started = Date.now();
  // Several polls may still be pending when the UI turns ready: the mesh
  // check starts once, and the test ends once.
  let checking = false;
  let done = false;
  const script =
    "document.querySelector('[data-testid=\"engine-status\"]')?.getAttribute('data-state') ?? ''";

  const finish = (code: number, message: string): void => {
    clearInterval(timer);
    if (done) {
      return;
    }
    done = true;
    process.stdout.write(`${message}\n`);
    app.exit(code);
  };

  const timer = setInterval(() => {
    if (window.isDestroyed()) {
      finish(1, 'SMOKE_FAIL window closed');
      return;
    }
    window.webContents.executeJavaScript(script).then(
      (state: unknown) => {
        if (checking || done) {
          return;
        }
        if (state === 'ready') {
          clearInterval(timer);
          checking = true;
          checkMeshes(meshTimeoutMs).then(
            (bytes) => {
              finish(
                0,
                `SMOKE_OK ${app.getName()} ${app.getVersion()} (mesh ${String(bytes)} bytes)`,
              );
            },
            (error: unknown) => {
              finish(1, `SMOKE_FAIL mesh worker: ${String(error)}`);
            },
          );
        } else if (Date.now() - started > timeoutMs) {
          finish(1, `SMOKE_FAIL engine state "${String(state)}"`);
        }
      },
      (error: unknown) => {
        if (!checking && Date.now() - started > timeoutMs) {
          finish(1, `SMOKE_FAIL ${String(error)}`);
        }
      },
    );
  }, POLL_INTERVAL_MS);
}
