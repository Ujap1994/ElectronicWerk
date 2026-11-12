/**
 * The library editors in the running app: a symbol drawn with the pin tool
 * and edited in the pin table, a footprint from the wizard edited in the
 * pad table, a component, the library checks, renaming and undo.
 */
import { expect, test, type Locator, type Page } from '@playwright/test';

import { createProjectWithWizard, launchApp, type LaunchedApp } from './fixtures';
import { openDocument, toScreen, type Point } from './viewer-helpers';

const MM = 1_000_000;
const SHOTS = process.env['EW_LIBEDIT_SHOTS'];

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

function itemCanvas(page: Page): Locator {
  return page.locator('[data-testid="item-canvas-area"][data-ready="true"]');
}

/** Waits until the item view stops moving. */
async function settleItem(page: Page): Promise<void> {
  const canvas = itemCanvas(page);
  await canvas.waitFor();
  await expect(async () => {
    const first = await canvas.getAttribute('data-view');
    await page.waitForTimeout(150);
    expect(await canvas.getAttribute('data-view')).toBe(first);
  }).toPass();
}

async function clickAt(page: Page, at: Point): Promise<void> {
  const [x, y] = await toScreen(itemCanvas(page), at);
  await page.mouse.move(x, y);
  await page.mouse.click(x, y);
}

async function shot(page: Page, name: string): Promise<void> {
  if (SHOTS !== undefined) {
    await page.screenshot({ path: `${SHOTS}/${name}.png` });
  }
}

/** Opens the project library in its editor tab. */
async function openLibrary(page: Page): Promise<void> {
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Library' }).first().dblclick();
  await expect(page.getByTestId('library-editor')).toBeVisible();
}

test('edits symbols, footprints and components', async () => {
  launched = await launchApp();
  const { page } = launched;
  await createProjectWithWizard(page, 'Parts');
  await openLibrary(page);
  // The starter symbols of a new project.
  await page.getByTestId('libedit-item-R').click();
  await settleItem(page);
  await shot(page, '01-library');

  // A new symbol: two pins with the pin tool, a body rectangle.
  await page.getByTestId('libedit-new').click();
  await page.getByTestId('libedit-name-input').fill('Driver');
  await page.getByTestId('libedit-name-confirm').click();
  await expect(page.getByTestId('item-name')).toHaveText('Driver');
  await settleItem(page);
  await page.getByTestId('item-tool-pin').click();
  await clickAt(page, [-7.62 * MM, 2.54 * MM]);
  await expect(page.getByTestId('pin-row-0')).toBeVisible();
  await clickAt(page, [-7.62 * MM, 0]);
  await expect(page.getByTestId('pin-row-1')).toBeVisible();
  await expect(page.getByTestId('pin-number-1')).toHaveValue('2');
  await page.getByTestId('item-tool-rect').click();
  await clickAt(page, [-5.08 * MM, 5.08 * MM]);
  await clickAt(page, [5.08 * MM, -2.54 * MM]);
  await expect(page.getByTestId('selection-props')).toContainText('Graphic');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('item-tool-select')).toHaveAttribute('aria-pressed', 'true');
  await shot(page, '02-symbol');
  await page.getByRole('button', { name: 'Fit to window' }).first().click();
  await settleItem(page);

  // The pin table: a name, then a bulk change of both pins' type.
  // Quick edits one after another all arrive (none is dropped).
  await page.getByTestId('pin-name-0').fill('IN');
  await page.getByTestId('pin-name-0').press('Enter');
  await page.getByTestId('pin-name-1').fill('EN');
  await page.getByTestId('pin-name-1').press('Enter');
  await expect(page.getByTestId('pin-name-0')).toHaveValue('IN');
  await page.getByTestId('tab-checks').click();
  await page.getByTestId('tab-table').click();
  await expect(page.getByTestId('pin-name-1')).toHaveValue('EN');
  await page.getByTestId('pin-row-0').click({ position: { x: 4, y: 4 } });
  await page.getByTestId('pin-row-1').click({ position: { x: 4, y: 4 }, modifiers: ['Shift'] });
  await expect(page.getByTestId('pin-table')).toContainText('2 selected');
  await page.getByTestId('pin-kind-1').selectOption('input');
  await expect(page.getByTestId('pin-kind-0')).toHaveValue('input');
  await expect(page.getByTestId('selection-note')).toContainText('2 items selected');

  // A pasted pin list adds pins with the next free numbers.
  await page.getByTestId('paste-pins').click();
  await page.getByTestId('paste-pins-text').fill('\tOUT\tO\n\tVCC\tW\n\tGND\tW');
  await page.getByTestId('paste-pins-add').click();
  await expect(page.getByTestId('paste-pins-dialog')).toBeHidden();
  await expect(page.getByTestId('pin-row-4')).toBeVisible();
  await expect(page.getByTestId('pin-number-4')).toHaveValue('5');
  await expect(page.getByTestId('pin-kind-3')).toHaveValue('powerIn');

  // A De Morgan body and a second unit.
  await page.getByTestId('body-alternate').click();
  await expect(page.getByTestId('body-alternate')).toHaveText('De Morgan');
  await page.getByTestId('body-alternate').click();
  await expect(page.getByTestId('body-alternate')).toHaveAttribute('aria-pressed', 'true');
  await page.getByTestId('body-normal').click();
  await page.getByTestId('unit-add').click();
  await expect(page.getByTestId('unit-select')).toHaveValue('2');
  await expect(page.getByTestId('pin-number-0')).toHaveValue('6');
  await page.getByTestId('unit-select').selectOption('1');

  // Properties and checks.
  await page.getByTestId('prop-description').fill('Line driver');
  await page.getByTestId('prop-description').blur();
  await page.getByTestId('tab-checks').click();
  await expect(page.getByTestId('checks-panel')).toBeVisible();
  await expect(page.getByTestId('finding-no-description')).toBeHidden();
  await page.getByTestId('tab-table').click();

  // A footprint from the wizard, then a bulk pad change.
  await page.getByTestId('libedit-kind-footprint').click();
  await page.getByTestId('libedit-wizard').click();
  await expect(page.getByTestId('fpwizard-dialog')).toBeVisible();
  await page.getByTestId('fpwizard-preset').selectOption('SOIC-8');
  await expect(page.getByTestId('fpwizard-name')).toHaveValue('SOIC-8_3.9x4.9mm_P1.27mm');
  await expect(page.getByTestId('fpwizard-preview')).toContainText('8 pads');
  await shot(page, '03-wizard');
  await page.getByTestId('fpwizard-create').click();
  await expect(page.getByTestId('fpwizard-dialog')).toBeHidden();
  await expect(page.getByTestId('item-name')).toHaveText('SOIC-8_3.9x4.9mm_P1.27mm');
  await settleItem(page);
  await expect(page.getByTestId('pad-row-7')).toBeVisible();
  await page.getByTestId('pad-table').getByRole('checkbox', { name: 'Select all' }).click();
  await expect(page.getByTestId('pad-table')).toContainText('8 selected');
  await page.getByTestId('pad-width-0').fill('0.6');
  await page.getByTestId('pad-width-0').press('Enter');
  await expect(page.getByTestId('pad-width-7')).toHaveValue('0.6');
  await shot(page, '04-footprint');
  // Undo brings the old width back.
  await page.getByTestId('toolbar-undo').click();
  await expect(page.getByTestId('pad-width-7')).not.toHaveValue('0.6');

  // A pad with the pad tool gets the next number.
  await page.getByTestId('item-tool-pad').click();
  await clickAt(page, [0, 0]);
  await expect(page.getByTestId('pad-number-8')).toHaveValue('9');
  await page.keyboard.press('Escape');

  // A component joins the symbol and the footprint.
  await page.getByTestId('libedit-kind-component').click();
  await page.getByTestId('libedit-new').click();
  await page.getByTestId('libedit-name-input').fill('DRV-1');
  await page.getByTestId('libedit-name-confirm').click();
  await expect(page.getByTestId('component-editor')).toBeVisible();
  await page.getByTestId('component-symbol').selectOption('Driver');
  await page.getByTestId('component-footprint').fill('SOIC-8_3.9x4.9mm_P1.27mm');
  await page.getByTestId('component-footprint').press('Enter');
  await page.getByTestId('component-mpn').fill('DRV-1-SO8');
  await page.getByTestId('component-mpn').press('Enter');
  await page.getByTestId('supplier-new').fill('Example Parts');
  await page.getByTestId('supplier-add').click();
  await expect(page.getByTestId('supplier-pn-0')).toBeVisible();
  await expect(page.getByTestId('component-pin-map')).toContainText('9');
  // Unit B has pins 6–10, the footprint only 9 pads.
  await expect(page.getByTestId('finding-pin-without-pad')).toContainText('Pin 10');
  await shot(page, '05-component');

  // Renaming the symbol follows the component's reference.
  await page.getByTestId('libedit-kind-symbol').click();
  await page.getByTestId('libedit-item-Driver').click();
  await page.getByTestId('item-rename').click();
  await page.getByTestId('libedit-name-input').fill('LineDriver');
  await page.getByTestId('libedit-name-confirm').click();
  await expect(page.getByTestId('item-name')).toHaveText('LineDriver');
  await page.getByTestId('libedit-kind-component').click();
  await expect(page.getByTestId('component-symbol')).toHaveValue('LineDriver');

  // The component is placed on the sheet with its footprint and order data.
  const canvas = await openDocument(page, 'Root');
  await canvas.focus();
  await page.keyboard.press('a');
  await page.getByTestId('chooser-search').fill('DRV');
  await page.getByTestId('chooser-component-DRV-1').click();
  await expect(page.getByTestId('symbol-chooser')).toContainText('SOIC-8_3.9x4.9mm_P1.27mm');
  await page.getByTestId('chooser-place').click();
  const [x, y] = await toScreen(canvas, [101.6 * MM, 101.6 * MM]);
  await page.mouse.move(x, y);
  await expect(page.getByTestId('preview-symbol')).toBeVisible();
  await page.mouse.click(x, y);
  await expect(page.getByTestId('property-designator')).toHaveValue('U1');
  await expect(page.getByTestId('property-param-MPN')).toHaveValue('DRV-1-SO8');
});
