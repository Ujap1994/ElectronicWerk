import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, test, type Page } from '@playwright/test';

import { createProjectWithWizard, launchApp, type LaunchedApp } from './fixtures';

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

/** `done` and `total` of the progress line. */
async function progress(page: Page): Promise<[number, number]> {
  const text = (await page.getByTestId('assembly-progress').textContent()) ?? '';
  const match = /(\d+) of (\d+)/u.exec(text);
  return [Number(match?.[1] ?? -1), Number(match?.[2] ?? -1)];
}

test('guides the assembly step by step and keeps the progress', async () => {
  test.setTimeout(120_000);
  launched = await launchApp();
  const { app, page, projectsDir } = launched;
  await createProjectWithWizard(page, 'Builder', 'demo');
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Builder' }).first().click();
  await page.getByTestId('details-open-assembly').click();
  const view = page.getByTestId('assembly');
  await expect(view).toBeVisible();
  await expect(page.getByTestId('assembly-canvas')).toHaveAttribute('data-ready', 'true', {
    timeout: 30_000,
  });
  const [done, total] = await progress(page);
  expect(done).toBe(0);
  expect(total).toBeGreaterThan(10);
  await expect(page.getByTestId('assembly-build')).toHaveText('Board 1');

  // Place the first group: the progress grows by its quantity.
  const quantity = Number(
    ((await page.getByTestId('assembly-quantity').textContent()) ?? '').split('/')[0],
  );
  const firstValue = await page.getByTestId('assembly-value').textContent();
  await page.getByTestId('assembly-place').click();
  await expect.poll(async () => (await progress(page))[0]).toBe(quantity);
  await expect(page.getByTestId('assembly-step').first()).toHaveAttribute('data-done', 'true');
  await expect(page.getByTestId('assembly-value')).not.toHaveText(firstValue ?? '');

  // One part of the next group, then the rest with the keyboard.
  const part = page.getByTestId('assembly-part').first();
  await part.click();
  await expect(part).toHaveAttribute('aria-pressed', 'true');
  await expect.poll(async () => (await progress(page))[0]).toBe(quantity + 1);
  await page.getByTestId('assembly-canvas').focus();
  await page.keyboard.press('Space');
  await expect(page.getByTestId('assembly-step').nth(1)).toHaveAttribute('data-done', 'true');
  const [afterKeys] = await progress(page);

  // Back and skip only move; nothing changes.
  const value = await page.getByTestId('assembly-value').textContent();
  await page.getByTestId('assembly-skip').click();
  await page.getByTestId('assembly-back').click();
  await expect(page.getByTestId('assembly-value')).toHaveText(value ?? '');
  expect((await progress(page))[0]).toBe(afterKeys);

  // Saved in the project folder at once, without "unsaved changes".
  const file = join(projectsDir, 'Builder', 'assembly.json');
  expect(existsSync(file)).toBe(true);
  const saved = JSON.parse(readFileSync(file, 'utf8')) as {
    boards: Record<string, { placed: string[] }>;
  };
  expect(Object.values(saved.boards)[0]?.placed.length).toBe(afterKeys);
  await expect(page.getByTestId('dirty-indicator')).toHaveAttribute('data-dirty', 'false');

  // Another order lists the same groups differently.
  const before = await page.getByTestId('assembly-step').allTextContents();
  await page.getByTestId('assembly-order').selectOption('count');
  await expect.poll(() => page.getByTestId('assembly-step').allTextContents()).not.toEqual(before);

  // Its own window follows the progress, and the main view follows it.
  const opened = app.waitForEvent('window');
  await page.getByTestId('assembly-detach').click();
  const second = await opened;
  await expect(second.getByTestId('assembly-window')).toBeVisible();
  await expect(second.getByTestId('assembly')).toHaveAttribute('data-detached', 'true');
  await expect(second.getByTestId('assembly-large')).toBeChecked();
  await expect.poll(async () => (await progress(second))[0]).toBe(afterKeys);
  await second.getByTestId('assembly-place').click();
  await expect.poll(async () => (await progress(page))[0]).toBeGreaterThan(afterKeys);

  // Reset needs a second click.
  await page.getByTestId('assembly-reset').click();
  await expect(page.getByTestId('assembly-reset')).toHaveText('Really reset? Click again');
  await page.getByTestId('assembly-reset').click();
  await expect.poll(async () => (await progress(page))[0]).toBe(0);
  await expect.poll(async () => (await progress(second))[0]).toBe(0);

  // Closing the project closes the window.
  const closed = second.waitForEvent('close');
  await page.getByTestId('toolbar-close').click();
  await closed;
});

test('counts finished boards', async () => {
  test.setTimeout(120_000);
  launched = await launchApp();
  const { page } = launched;
  await createProjectWithWizard(page, 'Small Run', 'demo');
  await page.keyboard.press('ControlOrMeta+Shift+P');
  await page.getByTestId('command-input').fill('assembly');
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('assembly-canvas')).toHaveAttribute('data-ready', 'true', {
    timeout: 30_000,
  });
  const [, total] = await progress(page);
  // Place everything, group by group.
  for (let i = 0; i < 200; i += 1) {
    const [before] = await progress(page);
    if (before >= total) {
      break;
    }
    await page.getByTestId('assembly-place').click();
    await expect.poll(async () => (await progress(page))[0]).toBeGreaterThan(before);
  }
  await expect(page.getByTestId('assembly-done')).toBeVisible();
  await page.getByTestId('assembly-finish').click();
  await expect(page.getByTestId('assembly-build')).toHaveText('Board 2');
  await expect.poll(async () => (await progress(page))[0]).toBe(0);
});
