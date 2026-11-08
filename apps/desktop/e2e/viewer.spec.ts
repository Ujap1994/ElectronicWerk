/**
 * The 2D viewer: opening sheets and boards, navigation, selection, the
 * inspector, layers and live reload after changes.
 */
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, test } from '@playwright/test';

import { createProjectWithWizard, launchApp, type LaunchedApp } from './fixtures';
import { openDocument as open, settle, toScreen, type Point } from './viewer-helpers';

interface Footprint {
  designator: string;
  at: Point;
  rotation?: number;
  side?: string;
  pads: { number: string; at: Point }[];
}

interface Board {
  footprints: Footprint[];
  tracks: { layer: string }[];
  vias: unknown[];
  shapes?: unknown[];
  zones?: unknown[];
  texts?: unknown[];
}

interface Sheet {
  symbols: { designator: string; at: Point }[];
}

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

function readDocument(projectDir: string, folder: string): unknown {
  const dir = join(projectDir, folder);
  const file = readdirSync(dir).find((name) => !name.startsWith('.'));
  return JSON.parse(readFileSync(join(dir, file ?? ''), 'utf8'));
}

test('board view: navigation, selection, inspector and layers', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await createProjectWithWizard(page, 'Viewer Demo', 'demo');
  const board = readDocument(join(projectsDir, 'Viewer Demo'), 'pcb') as Board;
  const canvas = await open(page, 'Viewer Demo');

  await expect(page.getByTestId('viewer')).toHaveAttribute('data-kind', 'pcb');
  await expect(page.getByTestId('layer-cu.top')).toHaveAttribute('data-active', 'true');
  await expect(page.getByTestId('layer-mask.top')).toHaveAttribute('data-visible', 'false');
  await expect(page.getByTestId('inspector')).toContainText('Click an object');

  // Hover and click a pad.
  const fp = board.footprints.find((entry) => (entry.side ?? 'top') === 'top' && !entry.rotation);
  expect(fp).toBeDefined();
  const pad = fp?.pads[0];
  const at: Point = [(fp?.at[0] ?? 0) + (pad?.at[0] ?? 0), (fp?.at[1] ?? 0) + (pad?.at[1] ?? 0)];
  const [px, py] = await toScreen(canvas, at);
  await page.mouse.move(px, py);
  await expect(page.getByTestId('viewer-hover')).toContainText('Pad');
  await expect(page.getByTestId('viewer-cursor')).toContainText('X');
  await page.mouse.click(px, py);
  await expect(page.getByTestId('inspector-title')).toContainText(`${fp?.designator ?? ''}-1`);
  await expect(page.getByTestId('inspector-net')).toContainText('N');
  await expect(page.getByTestId('inspector-position')).toContainText('mm');

  // Escape clears the selection.
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('inspector')).toContainText('Click an object');

  // Select everything; hidden layers do not count.
  const topLevel =
    board.footprints.length +
    board.tracks.length +
    board.vias.length +
    (board.shapes?.length ?? 0) +
    (board.zones?.length ?? 0) +
    (board.texts?.length ?? 0);
  await page.keyboard.press('ControlOrMeta+a');
  await expect(page.getByTestId('inspector-title')).toHaveText(`${topLevel} objects selected`);
  await page.getByTestId('layer-toggle-cu.bottom').click();
  await expect(page.getByTestId('layer-cu.bottom')).toHaveAttribute('data-visible', 'false');
  await canvas.focus();
  await page.keyboard.press('ControlOrMeta+a');
  const bottomTracks = board.tracks.filter((track) => track.layer === 'cu.bottom').length;
  await expect(page.getByTestId('inspector-title')).toHaveText(
    `${topLevel - bottomTracks} objects selected`,
  );
  await page.getByTestId('layers-show-all').click();
  await expect(page.getByTestId('layer-cu.bottom')).toHaveAttribute('data-visible', 'true');

  // Box selection across the whole view (left to right: fully inside).
  const box = await canvas.boundingBox();
  if (box === null) {
    throw new Error('no canvas');
  }
  await page.mouse.move(box.x + 2, box.y + 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps: 4 });
  await page.mouse.move(box.x + box.width - 2, box.y + box.height - 2, { steps: 4 });
  await page.mouse.up();
  await expect(page.getByTestId('inspector-title')).toContainText('objects selected');

  // Active layer and opacity.
  await page.getByTestId('layer-activate-cu.bottom').click();
  await expect(page.getByTestId('layer-cu.bottom')).toHaveAttribute('data-active', 'true');
  await page.getByTestId('layer-opacity').fill('40');
  await expect(page.locator('.viewer-panel__opacity output')).toContainText('40');

  // Zoom, flip and fit.
  const zoom = await page.getByTestId('viewer-zoom').textContent();
  await page.getByTestId('viewer-zoom-in').click();
  await expect(page.getByTestId('viewer-zoom')).not.toHaveText(zoom ?? '');
  await page.getByTestId('viewer-flip').click();
  await expect(page.getByTestId('viewer-flip')).toHaveAttribute('aria-pressed', 'true');
  // The active outer layer follows to the side that faces the viewer.
  await expect(page.getByTestId('layer-cu.top')).toHaveAttribute('data-active', 'true');
  await canvas.focus();
  await page.keyboard.press('b');
  await expect(page.getByTestId('viewer-flip')).toHaveAttribute('aria-pressed', 'false');
  await page.keyboard.press('Home');
  await expect(page.getByTestId('viewer-zoom')).toHaveText(zoom ?? '');

  // The view is remembered when switching tabs.
  await page.getByTestId('viewer-zoom-in').click();
  await page.waitForTimeout(400);
  const zoomed = await page.getByTestId('viewer-zoom').textContent();
  await page.getByTestId('tab-details').click();
  await expect(page.getByTestId('details')).toBeVisible();
  await page.locator('.tabs__tab', { hasText: 'Viewer Demo' }).click();
  await page.locator('[data-testid="viewer-canvas"][data-ready="true"]').waitFor();
  await expect(page.getByTestId('viewer-zoom')).toHaveText(zoomed ?? '');
  await expect(page.getByTestId('layer-cu.bottom')).toHaveAttribute('data-active', 'true');
});

test('sheet view: symbols, inspector, live reload and tabs', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await createProjectWithWizard(page, 'Sheet Demo', 'demo');
  const sheet = readDocument(join(projectsDir, 'Sheet Demo'), 'schematics') as Sheet;
  // A change that the viewer must pick up later (undone in the viewer).
  await page.getByTestId('project-revision').fill('B');
  await page.getByTestId('project-revision').press('Enter');
  await expect(page.getByTestId('dirty-indicator')).toHaveText('Unsaved changes');

  const canvas = await open(page, 'Root');
  await expect(page.getByTestId('viewer')).toHaveAttribute('data-kind', 'schematic');
  await expect(page.getByLabel('Display')).toBeVisible();
  await expect(page.getByTestId('viewer-flip')).toHaveCount(0);

  // Zoom in at the symbol with the mouse wheel, then click its body.
  const symbol = sheet.symbols[0];
  const [zx, zy] = await toScreen(canvas, symbol?.at ?? [0, 0]);
  await page.mouse.move(zx, zy);
  for (let i = 0; i < 4; i += 1) {
    await page.mouse.wheel(0, -120);
  }
  await settle(page);
  const [sx, sy] = await toScreen(canvas, symbol?.at ?? [0, 0]);
  await page.mouse.click(sx, sy);
  await expect(page.getByTestId('inspector-title')).toContainText('Symbol');
  await expect(page.getByTestId('inspector-title')).toContainText(symbol?.designator ?? '');
  await expect(page.getByTestId('inspector-designator')).toContainText(symbol?.designator ?? '');

  // Undo changes the project: the open sheet is rebuilt, the selection stays.
  const version = Number(await canvas.getAttribute('data-version'));
  await page.getByTestId('toolbar-undo').click();
  await expect(canvas).not.toHaveAttribute('data-version', String(version));
  await expect(page.getByTestId('inspector-title')).toContainText(symbol?.designator ?? '');

  // Tabs: the board opens next to the sheet and tabs can be closed.
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Sheet Demo' }).first().dblclick();
  await expect(page.locator('.tabs__item')).toHaveCount(2);
  await expect(page.getByTestId('viewer')).toHaveAttribute('data-kind', 'pcb');
  await page.locator('.tabs__item[data-selected="true"] .tabs__close').click();
  await expect(page.locator('.tabs__item')).toHaveCount(1);
  await expect(page.getByTestId('viewer')).toHaveAttribute('data-kind', 'schematic');
});

test('documents open from the details panel and the context menu', async () => {
  launched = await launchApp();
  const { page } = launched;
  await createProjectWithWizard(page, 'Open Demo', 'board-2l');
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Open Demo' }).first().click();
  await page.getByTestId('details-open').click();
  await page.locator('[data-testid="viewer-canvas"][data-ready="true"]').waitFor();
  await expect(page.getByTestId('layer-outline')).toBeVisible();
  await page.getByTestId('tab-details').click();
  await page
    .locator('[data-testid^="tree-doc:"]', { hasText: 'Root' })
    .first()
    .click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Open' }).click();
  await expect(page.locator('.tabs__item')).toHaveCount(2);
  await expect(page.getByTestId('viewer')).toHaveAttribute('data-kind', 'schematic');
});
