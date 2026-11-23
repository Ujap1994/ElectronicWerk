/**
 * Design blocks in the running app: the blinky sheet saved with its board
 * layout and placed again. The parts are numbered on and the board gets
 * their footprints, tracks and vias as one group, in one undo step.
 */
import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { expect, test } from '@playwright/test';

import { APP_DIR, launchApp, queueDialogAnswer, type LaunchedApp } from './fixtures';
import { openDocument } from './viewer-helpers';

const FIXTURE = resolve(APP_DIR, '../../crates/ew-kicad/tests/fixtures/blinky');

interface Board {
  footprints: { designator: string }[];
  tracks?: unknown[];
  groups?: { name?: string; members: string[] }[];
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

test('saves a sheet as a design block and places it again with its layout', async () => {
  launched = await launchApp();
  const { app, page, projectsDir } = launched;
  await queueDialogAnswer(app, join(FIXTURE, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-name').fill('Blocks');
  await page.getByTestId('kicad-import-start').click();
  const report = page.getByTestId('import-report');
  await expect(report).toBeVisible();
  await report.getByRole('button', { name: 'Close', exact: true }).last().click();
  const project = join(projectsDir, 'Blocks');
  const before = readBoard(project);

  // The whole sheet becomes a block.
  const canvas = await openDocument(page, 'Blinky');
  await expect(page.getByTestId('viewer')).toHaveAttribute('data-kind', 'schematic');
  await canvas.focus();
  await page.keyboard.press('ControlOrMeta+a');
  await page.getByTestId('action-blocks').click();
  const dialog = page.getByTestId('blocks-dialog');
  await expect(dialog).toBeVisible();
  await expect(page.getByTestId('blocks-empty')).toBeVisible();
  await expect(page.getByTestId('block-save')).toBeDisabled();
  await page.getByTestId('block-name').fill('Blinky core');
  await page.getByTestId('block-description').fill('Timer, LED and supply');
  await page.getByTestId('block-save').click();
  const item = page.getByTestId('block-item-0');
  await expect(item).toContainText('Blinky core');
  await expect(item).toContainText('Timer, LED and supply');
  await expect(item).toContainText('with layout');
  // The same name again would replace it.
  await page.getByTestId('block-name').fill('blinky core');
  await expect(page.getByTestId('block-save')).toHaveText('Replace block');

  // Placed again: new designators, the layout as a group.
  await page.getByTestId('block-place-0').click();
  await expect(dialog).toBeHidden();
  const message = page.getByText(/Placed Blinky core/u);
  await expect(message).toBeVisible();
  // "… on the board 7 footprints, …"
  const placed = Number(/board (\d+) footprints/u.exec(await message.innerText())?.[1] ?? 0);
  expect(placed).toBeGreaterThan(1);
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Place design block/u);
  await page.getByTestId('toolbar-save').click();
  await expect(page.getByTestId('dirty-indicator')).not.toHaveText('Unsaved changes');
  const after = readBoard(project);
  expect(after.footprints.length).toBe(before.footprints.length + placed);
  const designators = new Set(after.footprints.map((f) => f.designator));
  expect(designators.size).toBe(after.footprints.length);
  const group = (after.groups ?? []).find((g) => g.name === 'Blinky core');
  expect(group?.members.length ?? 0).toBeGreaterThan(placed);
  expect(after.tracks?.length ?? 0).toBeGreaterThan(before.tracks?.length ?? 0);

  // A block can be deleted after a second click.
  await page.getByTestId('action-blocks').click();
  await page.getByTestId('block-delete-0').click();
  await expect(page.getByTestId('block-delete-0')).toHaveText('Really delete?');
  await page.getByTestId('block-delete-0').click();
  await expect(page.getByTestId('blocks-empty')).toBeVisible();
});
