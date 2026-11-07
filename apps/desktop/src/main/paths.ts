import { join } from 'node:path';

import { app } from 'electron';

/** Directory of the bundled main-process files (`out/main`). */
const mainDir = import.meta.dirname;

/** Absolute paths to the app's runtime files. */
export const paths = {
  preload: join(mainDir, '../preload/index.cjs'),
  engineHost: join(mainDir, 'engine-host.js'),
  renderer: join(mainDir, '../renderer'),
  /** Window and dialog icon (Windows/Linux); copied from the renderer's public folder. */
  windowIcon: join(mainDir, '../renderer/favicon.png'),
  /** Dock icon for development runs (packaged macOS apps use the .icns). */
  devDockIcon: join(mainDir, '../../build/icon-macos.png'),
  /** Directory that holds the native engine addon (`ew_node.<platform>-<arch>.node`). */
  nativeDir(): string {
    // Development: apps/desktop/native (two levels above out/main).
    return app.isPackaged ? join(process.resourcesPath, 'native') : join(mainDir, '../../native');
  },
} as const;

/** Dev server URL injected by electron-vite in development, otherwise `undefined`. */
export function devServerUrl(): string | undefined {
  const url = process.env['ELECTRON_RENDERER_URL'];
  return !app.isPackaged && url !== undefined && url !== '' ? url : undefined;
}

/** `true` when automated tests enabled the test hooks. */
export function testHooksEnabled(): boolean {
  return process.env['EW_TEST_HOOKS'] === '1';
}

/** Test hook: release JSON file used instead of the GitHub API. */
export function testUpdateFeed(): string | undefined {
  const file = process.env['EW_UPDATE_FEED'];
  return testHooksEnabled() && file !== undefined && file !== '' ? file : undefined;
}

/** Delay before the automatic update check (shorter in tests). */
export function updateCheckDelayMs(): number {
  const value = Number(process.env['EW_UPDATE_DELAY_MS']);
  return testHooksEnabled() && Number.isFinite(value) && value >= 0 ? value : 3_000;
}
