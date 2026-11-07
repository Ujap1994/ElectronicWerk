import { app, type BrowserWindow } from 'electron';

/** Command-line flag that runs the packaged-app smoke test. */
export const SMOKE_TEST_FLAG = '--smoke-test';

const POLL_INTERVAL_MS = 250;

/**
 * Smoke test for packaged builds (used by the release pipeline, where
 * Playwright cannot attach because the Electron fuses disable debugging).
 * Waits until the UI shows a working engine connection, prints `SMOKE_OK`
 * and exits with code 0, or exits with code 1 after the timeout.
 */
export function runSmokeTest(window: BrowserWindow, timeoutMs = 30_000): void {
  const started = Date.now();
  const script =
    "document.querySelector('[data-testid=\"engine-status\"]')?.getAttribute('data-state') ?? ''";

  const finish = (code: number, message: string): void => {
    clearInterval(timer);
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
        if (state === 'ready') {
          finish(0, `SMOKE_OK ${app.getName()} ${app.getVersion()}`);
        } else if (Date.now() - started > timeoutMs) {
          finish(1, `SMOKE_FAIL engine state "${String(state)}"`);
        }
      },
      (error: unknown) => {
        if (Date.now() - started > timeoutMs) {
          finish(1, `SMOKE_FAIL ${String(error)}`);
        }
      },
    );
  }, POLL_INTERVAL_MS);
}
