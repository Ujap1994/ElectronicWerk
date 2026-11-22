/**
 * A system of several boards in the running app (M18), on the synthetic
 * "Station" (made with `ewerk synth system`): the tree, the boards, the
 * connection check with its findings, renaming a signal across boards,
 * stacking a board, a cable and the system in 3D.
 */
import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';

import { expect, test, type Page } from '@playwright/test';

import { APP_DIR, launchApp, queueDialogAnswer, type LaunchedApp } from './fixtures';

const REPO = resolve(APP_DIR, '../..');

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

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

/** Screenshots for a look while developing (`EW_SYSTEM_SHOTS=<dir>`). */
async function shot(page: Page, name: string): Promise<void> {
  const dir = process.env['EW_SYSTEM_SHOTS'];
  if (dir !== undefined && dir !== '') {
    await page.waitForTimeout(500);
    await page.screenshot({ path: join(dir, `${name}.png`) });
  }
}

test('checks the connections of a system of boards', async () => {
  test.setTimeout(300_000);
  const cargo = findCargo();
  test.skip(cargo === null, 'needs cargo to make the synthetic design');
  launched = await launchApp();
  const { app, page, projectsDir } = launched;
  await page.setViewportSize({ width: 1400, height: 860 });
  const folder = join(projectsDir, 'Station');
  execFileSync(cargo ?? 'cargo', ['run', '-q', '-p', 'ew-cli', '--', 'synth', 'system', folder], {
    cwd: REPO,
    stdio: 'pipe',
  });
  await queueDialogAnswer(app, join(folder, 'Station.ewproj'));
  await page.getByTestId('open-project').click();
  await expect(page.getByTestId('project-title')).toHaveText('Station');

  // The system in the tree, opened in a tab.
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Station' }).first().dblclick();
  const editor = page.getByTestId('system-editor');
  await expect(editor).toBeVisible();
  await expect(page.getByTestId('system-board')).toHaveCount(3);
  await expect(page.getByTestId('system-summary')).toContainText('2 errors', { timeout: 60_000 });
  await shot(page, 'boards');

  // The connection: the pins with their signals.
  await page.getByTestId('system-tab-connections').click();
  await expect(page.getByTestId('system-link')).toHaveCount(1);
  const pins = page.getByTestId('system-link-pins');
  await expect(pins.locator('tbody tr')).toHaveCount(16);
  await expect(pins.locator('tbody tr[data-status="swapped"]')).toHaveCount(2);
  await shot(page, 'connections');

  // The findings; BUTTON meets BTN through the cable: renamed on the panel.
  await page.getByTestId('system-tab-check').click();
  await expect(page.getByTestId('system-finding')).toHaveCount(3);
  await expect(page.getByTestId('system-findings')).toContainText('SDA and SCL are swapped');
  await shot(page, 'check');
  await page.getByTestId('system-rename').click();
  await expect(page.getByTestId('system-message')).toContainText('Renamed on 1 boards');
  await expect(page.getByTestId('system-summary')).toContainText('1 errors', { timeout: 60_000 });
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Rename signal/u);

  // Accepting the open A0.
  const missing = page
    .getByTestId('system-finding')
    .filter({ hasText: 'is not connected' })
    .getByTestId('system-accept');
  await missing.click();
  await expect(page.getByTestId('system-summary')).toContainText('0 warnings', { timeout: 60_000 });

  // Aliases.
  await page.getByTestId('system-aliases').fill('SCL = SCK');
  await page.getByTestId('system-aliases-save').click();
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Change system/u);

  // Stacking the shield on the carrier's socket again after moving it.
  await page.getByTestId('system-tab-boards').click();
  const shield = page.getByTestId('system-board').filter({ has: page.locator('[value="Shield"]') });
  const z = shield.getByTestId('system-board-z');
  await z.fill('30');
  await z.press('Enter');
  await expect(shield.getByTestId('system-board-z')).toHaveValue('30');
  await page.getByTestId('system-tab-connections').click();
  await page.getByTestId('system-link-stack').click();
  await expect(page.getByTestId('system-message')).toContainText('placed on its connector');
  await page.getByTestId('system-tab-boards').click();
  await expect(shield.getByTestId('system-board-z')).toHaveValue('10.08');

  // The cable and its wires.
  await page.getByTestId('system-tab-cables').click();
  await expect(page.getByTestId('system-cable')).toHaveCount(1);
  await expect(page.getByTestId('system-wire')).toHaveCount(4);
  await shot(page, 'cables');

  // Outputs.
  await page.getByTestId('system-outputs').click();
  await expect(page.getByTestId('system-message')).toContainText('7 files written');
  expect(existsSync(join(folder, 'outputs/station/station-w1-harness.pdf'))).toBe(true);

  // The system in 3D.
  await page.getByTestId('system-tab-view3d').click();
  const canvas = page.locator('[data-testid="view3d-canvas"][data-ready="true"]');
  await expect(canvas).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId('view3d-mechanical')).toHaveCount(0);
  await shot(page, 'view3d');
});
