/**
 * The calculators: open from the command palette, compute while typing.
 */
import { expect, test } from '@playwright/test';

import { launchApp, type LaunchedApp } from './fixtures';

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

test('the calculators compute dividers, E series and track currents', async () => {
  launched = await launchApp();
  const { page } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.getByTestId('open-palette').click();
  await page.getByTestId('command-input').fill('calculators');
  await page.keyboard.press('Enter');
  const dialog = page.getByTestId('calculators');
  await expect(dialog).toBeVisible();

  // Voltage divider: 12 V to 3.3 V from E24, best pair first.
  await dialog.getByTestId('calc-tab-divider').click();
  const pairs = dialog.getByTestId('calc-pair');
  await expect(pairs.first()).toBeVisible();
  const best = await pairs.first().locator('td').allInnerTexts();
  expect(best[2]).toMatch(/^3\.(2|3)\d* V$/u);
  // A wrong input says so.
  await dialog.getByTestId('calc-vout').fill('15');
  await expect(dialog.getByRole('status')).toContainText('below');
  await dialog.getByTestId('calc-vout').fill('3,3');
  await expect(pairs.first()).toBeVisible();

  // Regulator: LM317 to 5 V.
  await dialog.getByTestId('calc-tab-regulator').click();
  await dialog.getByTestId('calc-iadj').fill('50u');
  await expect(pairs.first()).toBeVisible();

  // E series: 4.6k is 4.7k in E24 and 4.64k in E96.
  await dialog.getByTestId('calc-tab-series').click();
  await dialog.getByTestId('calc-value').fill('4k6');
  await expect(pairs).toHaveCount(7);
  await expect(pairs.nth(3)).toContainText('4.7 k');
  await expect(pairs.nth(5)).toContainText('4.64 k');

  // Track: 10 A at 20 °C rise needs 9.46 mm in 1 oz copper (IPC-2152).
  await dialog.getByTestId('calc-tab-track').click();
  await dialog.getByTestId('calc-current-in').fill('10');
  await dialog.getByTestId('calc-rise').fill('20');
  await expect(dialog.getByTestId('calc-width')).toHaveText(/^9\.4[5-7]\d* mm$/u);
  await dialog.getByTestId('calc-standard').selectOption('ipc2221');
  await expect(dialog.getByTestId('calc-placement')).toBeVisible();
  await dialog.getByTestId('calc-rise').fill('150');
  await expect(dialog.getByTestId('calc-width')).toBeVisible();
  await dialog.getByTestId('calc-standard').selectOption('ipc2152');
  await expect(dialog.getByTestId('calc-note-riseOutsideFit')).toBeVisible();

  // Via and fusing current.
  await dialog.getByTestId('calc-tab-via').click();
  await expect(dialog.getByTestId('calc-current')).toHaveText(/ A$/u);
  await dialog.getByTestId('calc-tab-fusing').click();
  await expect(dialog.getByTestId('calc-current')).toHaveText(/^2\.5\d* A$/u);
  if (process.env['EW_LOOK'] !== undefined) {
    await dialog.getByTestId('calc-tab-divider').click();
    await expect(pairs.first()).toBeVisible();
    await page.screenshot({ path: process.env['EW_LOOK'] });
    await dialog.getByTestId('calc-tab-track').click();
    await page.screenshot({ path: process.env['EW_LOOK'].replace('.png', '-track.png') });
  }
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
});
