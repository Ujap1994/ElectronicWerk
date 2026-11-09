/**
 * The schematic editor in the running app: placing, wiring, editing
 * properties, undo, the electrical rules check, net highlighting, find,
 * annotation, exports and the guide.
 */
import { existsSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { expect, test, type Page } from '@playwright/test';

import {
  APP_DIR,
  createProjectWithWizard,
  launchApp,
  queueDialogAnswer,
  type LaunchedApp,
} from './fixtures';
import { openDocument, settle, toScreen, viewerCanvas, type Point } from './viewer-helpers';

const FIXTURE = resolve(APP_DIR, '../../crates/ew-kicad/tests/fixtures/blinky');
const MM = 1_000_000;

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

async function importBlinky(launchedApp: LaunchedApp): Promise<void> {
  const { app, page } = launchedApp;
  await queueDialogAnswer(app, join(FIXTURE, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-start').click();
  const report = page.getByTestId('import-report');
  await expect(report).toBeVisible();
  await report.getByRole('button', { name: 'Close', exact: true }).last().click();
  await expect(page.getByTestId('workspace')).toBeVisible();
}

/** Clicks a design point (nanometres) on the canvas. */
async function clickAt(page: Page, at: Point, options: { double?: boolean } = {}): Promise<void> {
  const [x, y] = await toScreen(viewerCanvas(page), at);
  await page.mouse.move(x, y);
  if (options.double === true) {
    await page.mouse.dblclick(x, y);
  } else {
    await page.mouse.click(x, y);
  }
}

test('places, wires, edits and checks a schematic', async () => {
  launched = await launchApp();
  const { app, page, projectsDir } = launched;
  await importBlinky(launched);
  const canvas = await openDocument(page, 'Blinky');
  await expect(page.getByTestId('viewer')).toHaveAttribute('data-kind', 'schematic');
  await expect(page.getByTestId('tool-palette')).toBeVisible();
  await expect(page.getByTestId('inspector')).not.toContainText('Read only');

  // Place a resistor from the project library (the page is 210 mm high;
  // the top-left corner of the drawing is empty).
  await canvas.focus();
  await page.keyboard.press('a');
  const chooser = page.getByTestId('symbol-chooser');
  await expect(chooser).toBeVisible();
  await page.getByTestId('chooser-search').fill('Device:R');
  await page.getByTestId('chooser-item-Device:R').click();
  await expect(page.getByTestId('symbol-preview')).toBeVisible();
  await page.getByTestId('chooser-place').click();
  await expect(chooser).toBeHidden();
  await expect(page.getByTestId('tool-symbol')).toHaveAttribute('aria-pressed', 'true');
  const anchor: Point = [40.64 * MM, 180.34 * MM];
  const [ax, ay] = await toScreen(canvas, anchor);
  await page.mouse.move(ax, ay);
  await expect(page.getByTestId('preview-symbol')).toBeVisible();
  await page.mouse.click(ax, ay);
  await expect(page.getByTestId('property-designator')).toHaveValue('R4');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('tool-select')).toHaveAttribute('aria-pressed', 'true');

  // Change its value in the inspector: one undo step.
  await page.getByTestId('property-value').fill('4k7');
  await page.getByTestId('property-value').press('Enter');
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Change properties/u);
  await expect(page.getByTestId('property-value')).toHaveValue('4k7');

  // A wire from pin 1 (3.81 mm above the anchor) to the right; it ends with Enter.
  await canvas.focus();
  await page.keyboard.press('w');
  const pin1: Point = [anchor[0], anchor[1] + 3.81 * MM];
  const loose: Point = [anchor[0] + 15.24 * MM, pin1[1]];
  await clickAt(page, pin1);
  await clickAt(page, loose);
  // A straight preview has no height, so it counts as attached, not visible.
  await expect(page.getByTestId('preview-wire')).toBeAttached();
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('preview-wire')).toHaveCount(0);
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Draw wire/u);
  await page.keyboard.press('Escape');

  // The check finds the loose end and the open pin 2.
  await page.getByTestId('action-erc').click();
  const erc = page.getByTestId('erc-panel');
  await expect(erc).toBeVisible();
  await expect(page.getByTestId('erc-issue-dangling-wire')).toBeVisible();
  await expect(page.getByTestId('erc-issue-unconnected-pin').first()).toBeVisible();
  const before = await canvas.getAttribute('data-view');
  await page.getByTestId('erc-issue-dangling-wire').click();
  await settle(page);
  expect(await canvas.getAttribute('data-view')).not.toBe(before);

  // Undo removes the wire, and the check follows.
  // (Ctrl+Z is a menu accelerator, which Playwright cannot press.)
  await page.getByTestId('toolbar-undo').click();
  await expect(page.getByTestId('erc-issue-dangling-wire')).toBeHidden();

  // Net highlighting on a wire of the timer circuit.
  await page.getByTestId('viewer-fit').click();
  await settle(page);
  await page.getByTestId('tool-highlight').click();
  await clickAt(page, [96.52 * MM, (210 - 83.82) * MM]);
  await expect(page.getByTestId('net-chip')).toBeVisible();
  await page.getByTestId('net-chip').click();
  await expect(page.getByTestId('net-chip')).toBeHidden();
  await canvas.focus();
  await page.keyboard.press('Escape');

  // Find the new resistor by its value.
  await page.keyboard.press(process.platform === 'darwin' ? 'Meta+f' : 'Control+f');
  await expect(page.getByTestId('find-panel')).toBeVisible();
  await page.getByTestId('find-query').fill('4k7');
  await expect(page.getByTestId('find-count')).toHaveText('Matches: 1');
  await page.getByTestId('find-close').click();

  // Annotation has nothing to do.
  await page.getByTestId('action-annotate').click();
  await page.getByTestId('annotate-run').click();
  await expect(page.getByText('Every symbol is already annotated.')).toBeVisible();

  // Exports: a KiCad netlist and a PDF of all sheets.
  const netlist = join(projectsDir, 'blinky-export.net');
  await queueDialogAnswer(app, netlist);
  await page.getByTestId('action-export').click();
  await page.getByTestId('export-netlist-kicad').click();
  await expect.poll(() => existsSync(netlist)).toBe(true);
  expect(readFileSync(netlist, 'utf8')).toContain('(ref "R4") (value "4k7")');
  const pdf = join(projectsDir, 'blinky.pdf');
  await queueDialogAnswer(app, pdf);
  await page.getByTestId('action-export').click();
  await page.getByTestId('export-pdf').click();
  await expect.poll(() => existsSync(pdf), { timeout: 30_000 }).toBe(true);
  expect(readFileSync(pdf).subarray(0, 5).toString()).toBe('%PDF-');
});

test('the guide helps with a first schematic; check settings', async () => {
  launched = await launchApp();
  const { page } = launched;
  await createProjectWithWizard(page, 'Guided', 'empty');
  await openDocument(page, 'Root');
  const guide = page.getByTestId('guide');
  await expect(guide).toBeVisible();
  await expect(page.getByTestId('guide-place')).toHaveAttribute('data-done', 'false');
  await page.getByTestId('guide-close').click();
  await expect(guide).toBeHidden();
  await page.getByTestId('action-guide').click();
  await expect(guide).toBeVisible();

  // The grid assistant.
  await page.getByTestId('action-grid').click();
  await expect(page.getByTestId('grid-status')).toHaveText('All wire ends are on the grid.');
  await page.getByTestId('grid-snap').uncheck();
  await expect(page.getByTestId('action-grid')).toHaveAttribute('aria-pressed', 'false');
  await page.getByTestId('grid-snap').check();

  // Check settings: one severity and one matrix cell.
  await page.getByTestId('action-erc').click();
  await expect(page.getByTestId('erc-clean')).toBeVisible();
  await page.getByTestId('erc-settings').click();
  const dialog = page.getByTestId('erc-settings-dialog');
  await expect(dialog).toBeVisible();
  await page.getByTestId('erc-severity-unconnected-pin').selectOption('warning');
  const cell = page.getByTestId('erc-cell-output-output');
  await expect(cell).toHaveAttribute('data-severity', 'error');
  await cell.click();
  await expect(cell).toHaveAttribute('data-severity', 'ignore');
  await page.getByTestId('erc-settings-save').click();
  await expect(dialog).toBeHidden();
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Change check settings/u);
});
