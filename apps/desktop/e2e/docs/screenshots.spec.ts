/**
 * Generates the screenshots used in the documentation (docs/public/screenshots).
 * Run with `npm run docs:screenshots` after building the app. Only synthetic
 * demo data is shown, so the images are safe to publish.
 */
import { mkdirSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { expect, test, type Locator, type Page } from '@playwright/test';

import { fakeRelease, launchApp, type LaunchedApp, type LaunchOptions } from '../fixtures';

const OUT = resolve(import.meta.dirname, '../../../../docs/public/screenshots');
mkdirSync(OUT, { recursive: true });

const DEMO_NOTES = [
  '## Added',
  '',
  '- **Projects:** create, open and save projects with schematic and PCB',
  '- Project tree panel with schematics, PCB, libraries and output jobs',
  '- Command palette entries for every project action',
  '',
  '## Fixed',
  '',
  '- The settings dialog remembers the last selected section',
].join('\n');

async function start(options: LaunchOptions = {}): Promise<LaunchedApp> {
  const launched = await launchApp(options);
  await launched.page.setViewportSize({ width: 1280, height: 800 });
  await expect(launched.page.getByTestId('engine-status')).toHaveAttribute('data-state', 'ready');
  return launched;
}

async function shoot(target: Page | Locator, name: string): Promise<void> {
  await target.screenshot({ path: join(OUT, `${name}.png`), scale: 'css', animations: 'disabled' });
}

test.describe.configure({ mode: 'serial' });

test('start screen', async () => {
  for (const theme of ['light', 'dark'] as const) {
    const { page, close } = await start({ settings: { theme } });
    await shoot(page, `start-screen-${theme}`);
    await shoot(page.locator('.status-bar'), `status-bar-${theme}`);
    await close();
  }
  const german = await start({ settings: { theme: 'light', language: 'de' } });
  await shoot(german.page, 'start-screen-german');
  await german.close();
});

test('settings dialog', async () => {
  const { page, close } = await start({ settings: { theme: 'light' } });
  await page.getByTestId('open-settings').click();
  await shoot(page, 'settings-window');
  await shoot(page.getByTestId('settings-dialog'), 'settings-dialog');
  await page.getByTestId('setting-language').focus();
  await close();

  const skipped = await start({
    settings: { theme: 'light', updates: { skippedVersion: '0.2.0' } },
  });
  await skipped.page.getByTestId('open-settings').click();
  await shoot(skipped.page.getByTestId('settings-dialog'), 'settings-dialog-skipped');
  await skipped.close();
});

test('command palette', async () => {
  const { page, close } = await start({ settings: { theme: 'light' } });
  await page.getByTestId('open-palette').click();
  await shoot(page, 'command-palette-all');
  await page.getByTestId('command-input').fill('appearance');
  await shoot(page.getByTestId('command-palette'), 'command-palette-search');
  await page.getByTestId('command-input').fill('xyz');
  await shoot(page.getByTestId('command-palette'), 'command-palette-empty');
  await close();
});

test('update notice', async () => {
  for (const theme of ['light', 'dark'] as const) {
    const { page, close } = await start({
      settings: { theme },
      updateFeed: fakeRelease('0.2.0', DEMO_NOTES),
    });
    await expect(page.getByTestId('update-dialog')).toBeVisible();
    await shoot(page, `update-available-window-${theme}`);
    await shoot(page.getByTestId('update-dialog'), `update-available-${theme}`);
    await close();
  }

  const upToDate = await start({ settings: { theme: 'light' }, updateFeed: fakeRelease('0.1.0') });
  await upToDate.page.getByTestId('open-settings').click();
  await upToDate.page.getByTestId('check-updates-now').click();
  await expect(upToDate.page.getByTestId('update-dialog')).toContainText('latest version');
  await shoot(upToDate.page.getByTestId('update-dialog'), 'update-up-to-date');
  await upToDate.close();

  const failed = await start({ settings: { theme: 'light' }, updateFeed: '{ offline' });
  await failed.page.getByTestId('open-settings').click();
  await failed.page.getByTestId('check-updates-now').click();
  await expect(failed.page.getByTestId('update-dialog')).toContainText('failed');
  await shoot(failed.page.getByTestId('update-dialog'), 'update-error');
  await failed.close();
});

test('engine restart notice', async () => {
  const { app, page, close } = await start({ settings: { theme: 'light' } });
  await app.evaluate(() => {
    const hooks = (globalThis as { __ewTest?: { engine: { crashForTest: () => void } } }).__ewTest;
    hooks?.engine.crashForTest();
  });
  await expect(page.getByTestId('engine-restarted')).toBeVisible();
  await expect(page.getByTestId('engine-status')).toHaveAttribute('data-state', 'ready');
  await shoot(page, 'engine-restarted');
  await close();
});
