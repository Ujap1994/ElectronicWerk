/**
 * Channels and assembly variants in the running app, on the synthetic
 * design with a sheet used three times (made with `ewerk synth channels`).
 */
import { execFileSync } from 'node:child_process';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';

import { expect, test } from '@playwright/test';

import { APP_DIR, launchApp, queueDialogAnswer, type LaunchedApp } from './fixtures';
import { settle } from './viewer-helpers';

const REPO = resolve(APP_DIR, '../..');

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

/** `cargo` from the PATH or the default installation, if any. */
function findCargo(): string | null {
  const installed = join(
    homedir(),
    '.cargo',
    'bin',
    process.platform === 'win32' ? 'cargo.exe' : 'cargo',
  );
  for (const candidate of ['cargo', installed]) {
    try {
      execFileSync(candidate, ['--version'], { stdio: 'ignore' });
      return candidate;
    } catch {
      // Try the next one.
    }
  }
  return null;
}

/** Makes the synthetic channels project and opens it. */
async function openChannels(app: LaunchedApp, cargo: string): Promise<void> {
  const folder = join(app.projectsDir, 'Channels');
  execFileSync(cargo, ['run', '-q', '-p', 'ew-cli', '--', 'synth', 'channels', folder], {
    cwd: REPO,
    stdio: 'pipe',
  });
  await queueDialogAnswer(app.app, join(folder, 'Channels.ewproj'));
  await app.page.getByTestId('open-project').click();
  await expect(app.page.getByTestId('project-title')).toHaveText('Channels');
}

test('copies the layout of one channel to the others', async () => {
  test.setTimeout(240_000);
  const cargo = findCargo();
  test.skip(cargo === null, 'needs cargo to make the synthetic design');
  launched = await launchApp();
  const { page } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await openChannels(launched, cargo ?? 'cargo');
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Board' }).first().dblclick();
  await settle(page);

  await page.getByTestId('board-channels').click();
  const dialog = page.getByTestId('channels-dialog');
  await expect(dialog).toBeVisible();
  await expect(page.getByTestId('channels-instances').locator('tbody tr')).toHaveCount(3);
  await expect(page.getByTestId('channels-from-0')).toBeChecked();
  await expect(page.getByTestId('channels-to-0')).toBeDisabled();
  await expect(page.getByTestId('channels-to-1')).toBeChecked();
  await expect(page.getByTestId('channels-to-2')).toBeChecked();
  await page.getByTestId('channels-copy').click();
  await expect(dialog).toBeHidden();
  await expect(page.getByText(/4 parts placed, 4 tracks copied/u)).toBeVisible();
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Copy channel layout/u);
});

test('keeps assembly variants and makes outputs for one', async () => {
  test.setTimeout(240_000);
  const cargo = findCargo();
  test.skip(cargo === null, 'needs cargo to make the synthetic design');
  launched = await launchApp();
  const { page } = launched;
  await page.setViewportSize({ width: 1280, height: 900 });
  await openChannels(launched, cargo ?? 'cargo');

  // A variant without R2, with another value for C1.
  const variants = page.getByTestId('variants');
  await variants.scrollIntoViewIfNeeded();
  await page.getByTestId('variant-add').click();
  await expect(page.getByTestId('variant-name-0')).toHaveValue('Variant 1');
  const name = page.getByTestId('variant-name-0');
  await name.fill('Lite');
  await name.press('Enter');
  await expect(page.getByTestId('variant-parts-0')).toBeVisible();
  await page.getByTestId('variant-fitting-R2').selectOption('left');
  await expect(page.getByTestId('variant-0')).toContainText('1 changed parts');
  const value = page.getByTestId('variant-value-C1');
  await value.fill('220n');
  await value.press('Enter');
  await expect(page.getByTestId('variant-0')).toContainText('2 changed parts');
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Change project/u);

  // The output job is made for the variant.
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Fabrication' }).first().dblclick();
  const select = page.getByTestId('outputs-variant');
  await expect(select).toBeVisible();
  await select.selectOption('Lite');
  await expect(select).toHaveValue('Lite');
});
