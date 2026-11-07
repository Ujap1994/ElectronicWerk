import { expect, test } from '@playwright/test';

import { launchApp, type LaunchedApp } from './fixtures';

let launched: LaunchedApp;

test.beforeEach(async () => {
  launched = await launchApp();
});

test.afterEach(async () => {
  await launched.close();
});

test('opens from the status bar and runs a command with the keyboard', async () => {
  const { page } = launched;
  await page.getByTestId('open-palette').click();
  const palette = page.getByTestId('command-palette');
  await expect(palette).toBeVisible();

  await page.getByTestId('command-input').fill('german');
  await expect(palette.getByRole('option')).toHaveCount(1);
  await page.keyboard.press('Enter');

  await expect(palette).toBeHidden();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Willkommen bei ElectronicWerk');
});

test('opens with Ctrl/Cmd+Shift+P, navigates with arrow keys and closes with Escape', async () => {
  const { page } = launched;
  // Give the window keyboard focus first (headless Linux displays start unfocused).
  await page.getByRole('heading', { level: 1 }).click();
  await page.keyboard.press('ControlOrMeta+Shift+P');
  const palette = page.getByTestId('command-palette');
  await expect(palette).toBeVisible();

  await page.getByTestId('command-input').fill('appearance');
  const options = palette.getByRole('option');
  await expect(options).toHaveCount(3);
  await expect(options.nth(0)).toHaveAttribute('aria-selected', 'true');
  await page.keyboard.press('ArrowDown');
  await expect(options.nth(1)).toHaveAttribute('aria-selected', 'true');

  await page.keyboard.press('Escape');
  await expect(palette).toBeHidden();
});

test('shows a hint when nothing matches', async () => {
  const { page } = launched;
  await page.getByTestId('open-palette').click();
  await page.getByTestId('command-input').fill('xyz-no-such-command');
  await expect(page.getByTestId('command-palette')).toContainText('No matching commands');
});

test('switches the theme from the palette', async () => {
  const { page } = launched;
  await page.getByTestId('open-palette').click();
  await page.getByTestId('command-input').fill('dark');
  await page.keyboard.press('Enter');
  await expect
    .poll(() => page.evaluate(() => matchMedia('(prefers-color-scheme: dark)').matches))
    .toBe(true);
});
