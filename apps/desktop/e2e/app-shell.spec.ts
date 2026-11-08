import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, test } from '@playwright/test';

import { APP_DIR, APP_VERSION, launchApp, type LaunchedApp } from './fixtures';

let launched: LaunchedApp;

test.beforeEach(async () => {
  launched = await launchApp();
});

test.afterEach(async () => {
  await launched.close();
});

test('shows the start screen with a secure, sandboxed renderer', async () => {
  const { page } = launched;
  await expect(page).toHaveTitle('ElectronicWerk');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Welcome to ElectronicWerk');

  // No Node.js in the page; only the typed preload API is visible.
  const exposure = await page.evaluate(() => ({
    hasRequire: 'require' in globalThis,
    hasProcess: 'process' in globalThis,
    api: Object.keys((globalThis as unknown as { ew: object }).ew).sort(),
  }));
  expect(exposure).toEqual({
    hasRequire: false,
    hasProcess: false,
    api: ['app', 'dialogs', 'engine', 'library', 'projects', 'settings', 'updates', 'window'],
  });
  expect(page.url()).toBe('app://bundle/index.html');
});

test('connects to the Rust engine over the MessagePort', async () => {
  const version = APP_VERSION;
  const status = launched.page.getByTestId('engine-status');
  await expect(status).toHaveAttribute('data-state', 'ready');
  await expect(status).toContainText(`Engine ${version}`);
  await expect(launched.page.getByText(`Version ${version}`)).toBeVisible();
});

test('follows the theme setting', async () => {
  const { page } = launched;
  const isDark = () => page.evaluate(() => matchMedia('(prefers-color-scheme: dark)').matches);
  await page.getByTestId('open-settings').click();
  await page.getByTestId('setting-theme').selectOption('dark');
  await expect.poll(isDark).toBe(true);
  await page.getByTestId('setting-theme').selectOption('light');
  await expect.poll(isDark).toBe(false);
});

test('switches the language at runtime and remembers it', async () => {
  const { page, userDataDir } = launched;
  await page.getByTestId('open-settings').click();
  await expect(page.getByTestId('settings-dialog')).toBeVisible();
  await page.getByTestId('setting-language').selectOption('de');
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Willkommen bei ElectronicWerk');
  const saved = JSON.parse(readFileSync(join(userDataDir, 'settings.json'), 'utf8')) as {
    language: string;
  };
  expect(saved.language).toBe('de');
});

test('restarts the engine after a crash without closing the window', async () => {
  const { app, page } = launched;
  const status = page.getByTestId('engine-status');
  await expect(status).toHaveAttribute('data-state', 'ready');

  await app.evaluate(() => {
    const hooks = (globalThis as { __ewTest?: { engine: { crashForTest: () => void } } }).__ewTest;
    hooks?.engine.crashForTest();
  });

  await expect(page.getByTestId('engine-restarted')).toBeVisible();
  await expect(status).toHaveAttribute('data-state', 'ready');
});

test('allows only one running instance', async () => {
  const electronBinary = launched.app.process().spawnfile;
  // Mirror the first instance: Playwright runs Electron without the OS
  // sandbox on Linux CI runners (see fixtures.ts).
  const args = process.platform === 'linux' ? [APP_DIR, '--no-sandbox'] : [APP_DIR];
  const second = spawn(electronBinary, args, {
    cwd: APP_DIR,
    env: { ...process.env, EW_USER_DATA_DIR: launched.userDataDir },
    stdio: 'ignore',
  });
  const exitCode = await new Promise<number | null>((resolve) => {
    second.on('exit', resolve);
  });
  expect(exitCode).toBe(0);
  // The first instance is still alive and responsive.
  await expect(launched.page.getByRole('heading', { level: 1 })).toBeVisible();
});

test('closing the window quits the application', async () => {
  const { app, page } = launched;
  const closed = new Promise<void>((resolve) => {
    app.process().on('exit', () => {
      resolve();
    });
  });
  await page.close();
  await closed;
});
