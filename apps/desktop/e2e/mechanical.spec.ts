/**
 * The enclosure in the 3D view (M17): a box enclosure made for the board,
 * a lid moved into the parts and back, visibility and transparency, a STEP
 * file added from outside the project and removed again.
 */
import { copyFileSync, existsSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

import { expect, test, type Page } from '@playwright/test';

import { APP_DIR, launchApp, queueDialogAnswer, type LaunchedApp } from './fixtures';
import { settle } from './viewer-helpers';

const FIXTURE = resolve(APP_DIR, '../../crates/ew-kicad/tests/fixtures/blinky');

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

/** Screenshots for a look while developing (`EW_MECH_SHOTS=<dir>`). */
async function shot(page: Page, name: string): Promise<void> {
  const dir = process.env['EW_MECH_SHOTS'];
  if (dir !== undefined && dir !== '') {
    await page.waitForTimeout(600);
    await page.screenshot({ path: join(dir, `${name}.png`) });
  }
}

function view3d(page: Page): ReturnType<Page['locator']> {
  return page.locator('[data-testid="view3d-canvas"][data-ready="true"]');
}

test('checks the board in its enclosure', async () => {
  test.setTimeout(240_000);
  launched = await launchApp();
  const { app, page, projectsDir } = launched;
  await page.setViewportSize({ width: 1400, height: 860 });
  await queueDialogAnswer(app, join(FIXTURE, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-name').fill('Boxed');
  await page.getByTestId('kicad-import-start').click();
  const report = page.getByTestId('import-report');
  await expect(report).toBeVisible();
  await report.getByRole('button', { name: 'Close', exact: true }).last().click();
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Boxed' }).first().dblclick();
  await settle(page);
  await page.getByTestId('viewer-open-3d').click();
  await expect(view3d(page)).toBeVisible({ timeout: 30_000 });

  // The panel starts empty; a box is made to fit.
  await page.getByTestId('view3d-mechanical').click();
  await expect(page.getByTestId('mech-panel')).toBeVisible();
  await expect(page.getByTestId('mech-empty')).toBeVisible();
  await page.getByTestId('mech-box').click();
  const rows = page.getByTestId('mech-body');
  await expect(rows).toHaveCount(2);
  await expect(rows.nth(0)).toHaveAttribute('data-name', 'Enclosure bottom');
  await expect(view3d(page)).toHaveAttribute('data-bodies', '2/2', { timeout: 60_000 });
  await expect(page.getByTestId('mech-summary')).toContainText('No collisions', {
    timeout: 30_000,
  });
  await shot(page, 'box');
  const folder = join(projectsDir, 'Boxed', 'mechanical');
  expect(existsSync(join(folder, 'enclosure-bottom.step'))).toBe(true);
  expect(existsSync(join(folder, 'enclosure-lid.step'))).toBe(true);

  // The lid 6 mm lower cuts into the board and its parts.
  await rows.nth(1).click();
  await expect(page.getByTestId('mech-editor')).toBeVisible();
  const z = page.getByTestId('mech-z');
  const before = await z.inputValue();
  await z.fill(String(Number(before) - 6));
  await z.press('Enter');
  await expect(page.getByTestId('mech-summary')).toContainText('collisions', { timeout: 30_000 });
  await expect(page.getByTestId('mech-finding').first()).toHaveAttribute(
    'data-code',
    'mechanical-collision',
  );
  expect(Number(await view3d(page).getAttribute('data-markers'))).toBeGreaterThan(0);
  await shot(page, 'collision');
  await page.getByTestId('mech-finding').first().click();
  await shot(page, 'focus');

  // Back up again: no findings.
  await page.getByTestId('mech-z').fill(before);
  await page.getByTestId('mech-z').press('Enter');
  await expect(page.getByTestId('mech-summary')).toContainText('No collisions', {
    timeout: 30_000,
  });
  await expect(view3d(page)).toHaveAttribute('data-markers', '0');

  // Hidden and see-through.
  await page.getByTestId('mech-visible').nth(1).click();
  await expect(page.getByTestId('mech-visible').nth(1)).toHaveAttribute('aria-pressed', 'false');
  await page.getByTestId('mech-visible').nth(1).click();
  await page.getByTestId('mech-opacity').fill('30');
  await expect(page.getByTestId('mech-editor')).toContainText('30 %');
  await shot(page, 'transparent');

  // A STEP file from elsewhere is copied into the project.
  const outside = mkdtempSync(join(tmpdir(), 'ew-mech-'));
  const panel = join(outside, 'Front panel.step');
  copyFileSync(join(folder, 'enclosure-lid.step'), panel);
  await queueDialogAnswer(app, panel);
  await page.getByTestId('mech-add').click();
  await expect(rows).toHaveCount(3);
  await expect(page.getByTestId('view3d-message')).toContainText('mechanical/Front panel.step');
  expect(existsSync(join(folder, 'Front panel.step'))).toBe(true);
  await expect(page.getByTestId('mech-kind')).toHaveValue('panel');
  await page.getByTestId('mech-remove').click();
  await expect(rows).toHaveCount(2);
});
