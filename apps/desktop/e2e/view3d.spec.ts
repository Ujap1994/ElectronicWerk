/**
 * The 3D view in the running app: opening it from the layout editor, the
 * view controls, the selection shared with the layout editor, the 3D view
 * in its own window with live updates, STEP export and screenshots.
 */
import { existsSync, mkdtempSync, readdirSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

import { expect, test, type Page } from '@playwright/test';

import { APP_DIR, launchApp, queueDialogAnswer, type LaunchedApp } from './fixtures';
import { settle, toScreen, viewerCanvas, type Point } from './viewer-helpers';

const FIXTURE = resolve(APP_DIR, '../../crates/ew-kicad/tests/fixtures/blinky');

interface Board {
  footprints: { designator: string; at: Point; side?: string }[];
}

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

function readBoard(projectDir: string): Board {
  const dir = join(projectDir, 'pcb');
  const file = readdirSync(dir).find((name) => !name.startsWith('.'));
  return JSON.parse(readFileSync(join(dir, file ?? ''), 'utf8')) as Board;
}

async function importBlinky(app: LaunchedApp, name: string): Promise<void> {
  const { page } = app;
  await queueDialogAnswer(app.app, join(FIXTURE, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-name').fill(name);
  await page.getByTestId('kicad-import-start').click();
  const report = page.getByTestId('import-report');
  await expect(report).toBeVisible();
  await report.getByRole('button', { name: 'Close', exact: true }).last().click();
  await expect(page.getByTestId('workspace')).toBeVisible();
}

function view3d(page: Page): ReturnType<Page['locator']> {
  return page.locator('[data-testid="view3d-canvas"][data-ready="true"]');
}

test('shows the board in 3D, live and in its own window', async () => {
  launched = await launchApp();
  const { app, page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importBlinky(launched, 'Board 3D');
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Board 3D' }).first().dblclick();
  await settle(page);

  // Select U1 in the layout editor.
  const board = readBoard(join(projectsDir, 'Board 3D'));
  const u1 = board.footprints.find((f) => f.designator === 'U1');
  expect(u1).toBeDefined();
  const [ux, uy] = await toScreen(viewerCanvas(page), u1?.at ?? [0, 0]);
  await page.mouse.click(ux, uy);

  // The 3D view opens in a tab next to the board and shows the selection.
  await page.getByTestId('viewer-open-3d').click();
  await expect(page.locator('[role="tab"]', { hasText: 'Board 3D (3D)' })).toBeVisible();
  await expect(view3d(page)).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId('view3d-selected')).toContainText('U1');
  await expect(page.getByTestId('view3d-models')).toBeVisible();

  // Views, projection, layers and sections.
  await page.getByTestId('view3d-views').click();
  await page.getByTestId('view3d-view-top').click();
  await page.getByTestId('view3d-projection').click();
  await expect(page.getByTestId('view3d-projection')).toHaveAttribute('aria-pressed', 'true');
  await page.getByTestId('view3d-show').click();
  await page.getByTestId('view3d-show-silk').uncheck();
  await page.getByTestId('view3d-show-silk').check();
  await page.getByTestId('view3d-show').click();
  await page.getByTestId('view3d-section').click();
  await page.getByTestId('view3d-section-x').check();
  await expect(page.getByTestId('view3d-section')).toHaveAttribute('aria-pressed', 'true');
  await page.getByTestId('view3d-section-off').check();
  await page.getByTestId('view3d-section').click();

  // Measuring: two clicks on the board in the top view.
  await page.getByTestId('view3d-projection').click();
  await page.getByTestId('view3d-views').click();
  await page.getByTestId('view3d-view-top').click();
  await page.getByTestId('view3d-measure').click();
  await expect(page.getByTestId('view3d-measure-hint')).toBeVisible();
  const box = await view3d(page).boundingBox();
  const cx = (box?.x ?? 0) + (box?.width ?? 0) / 2;
  const cy = (box?.y ?? 0) + (box?.height ?? 0) / 2;
  await page.mouse.click(cx - 60, cy);
  await page.mouse.click(cx + 60, cy);
  await expect(page.getByTestId('view3d-measure-label')).toContainText('Distance');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('view3d-measure-label')).toHaveCount(0);

  // Export as STEP and save a screenshot.
  const out = mkdtempSync(join(tmpdir(), 'ew-3d-'));
  const step = join(out, 'board.step');
  await queueDialogAnswer(app, step);
  await page.getByTestId('view3d-export').click();
  await page.getByTestId('view3d-export-step').click();
  await expect(page.getByTestId('view3d-message')).toContainText('Exported', { timeout: 60_000 });
  expect(readFileSync(step, 'utf8').startsWith('ISO-10303-21')).toBe(true);
  const png = join(out, 'board.png');
  await queueDialogAnswer(app, png);
  await page.getByTestId('view3d-screenshot').click();
  await expect(page.getByTestId('view3d-message')).toContainText('Saved');
  expect(existsSync(png)).toBe(true);
  expect([...readFileSync(png).subarray(1, 4)].map((c) => String.fromCharCode(c)).join('')).toBe(
    'PNG',
  );

  // Its own window: it follows edits in the layout editor at once.
  const opened = app.waitForEvent('window');
  await page.getByTestId('view3d-detach').click();
  const second = await opened;
  await expect(second.getByTestId('view3d-window')).toBeVisible();
  await expect(view3d(second)).toBeVisible({ timeout: 30_000 });
  const before = Number(await view3d(second).getAttribute('data-updates'));
  await page.locator('[role="tab"]', { hasText: /^Board 3D$/u }).click();
  await settle(page);
  const [sx, sy] = await toScreen(viewerCanvas(page), u1?.at ?? [0, 0]);
  await page.mouse.click(sx, sy);
  await viewerCanvas(page).focus();
  const started = Date.now();
  await page.keyboard.press('r');
  await expect(view3d(second)).not.toHaveAttribute('data-updates', String(before));
  test.info().annotations.push({
    type: 'live update',
    description: `${String(Date.now() - started)} ms until the 3D window moved the part`,
  });
  // The selection is shared between the windows too.
  await expect(second.getByTestId('view3d-selected')).toContainText('U1');

  // Closing the project closes the 3D window.
  const closed = second.waitForEvent('close');
  await page.getByTestId('toolbar-close').click();
  await page.getByTestId('close-dont-save').click();
  await closed;
});

test('opens the 3D view from the details and the command palette', async () => {
  launched = await launchApp();
  const { page } = launched;
  await importBlinky(launched, 'Palette 3D');
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Palette 3D' }).first().click();
  await page.getByTestId('details-open-3d').click();
  await expect(view3d(page)).toBeVisible({ timeout: 30_000 });
  await page.getByTestId('tab-details').click();
  await page.keyboard.press('ControlOrMeta+Shift+P');
  await page.getByTestId('command-input').fill('3D view');
  await page.keyboard.press('Enter');
  await expect(view3d(page)).toBeVisible({ timeout: 30_000 });
});
