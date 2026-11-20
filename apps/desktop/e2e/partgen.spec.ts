/**
 * The part generator in the running app: a synthetic datasheet (made with
 * `ewerk synth datasheets`) is read, its values are checked against the
 * page, a pin is corrected, the part is generated into the project
 * library, and the duplicate check finds it the second time.
 */
import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';

import { expect, test, type Page } from '@playwright/test';

import {
  APP_DIR,
  createProjectWithWizard,
  launchApp,
  queueDialogAnswer,
  type LaunchedApp,
} from './fixtures';

const REPO = resolve(APP_DIR, '../..');
const SHOTS = process.env['EW_PARTGEN_SHOTS'];

async function shot(page: Page, name: string): Promise<void> {
  if (SHOTS !== undefined) {
    await page.screenshot({ path: `${SHOTS}/${name}.png` });
  }
}

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

/** Writes the synthetic datasheets into `dir`. */
function datasheets(cargo: string, dir: string): void {
  execFileSync(cargo, ['run', '-q', '-p', 'ew-cli', '--', 'synth', 'datasheets', dir], {
    cwd: REPO,
    stdio: 'pipe',
  });
}

async function openGenerator(page: Page, file: string): Promise<void> {
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Library' }).first().dblclick();
  await expect(page.getByTestId('library-editor')).toBeVisible();
  await queueDialogAnswer(launched!.app, file);
  await page.getByTestId('libedit-datasheet').click();
  await expect(page.getByTestId('partgen')).toBeVisible();
  await page.getByTestId('partgen-open').click();
  await expect(page.getByTestId('partgen-step-part')).toBeVisible({ timeout: 60_000 });
}

test('makes a part from a datasheet', async () => {
  test.setTimeout(300_000);
  const cargo = findCargo();
  test.skip(cargo === null, 'needs cargo to make the synthetic datasheets');
  launched = await launchApp();
  const { page } = launched;
  await page.setViewportSize({ width: 1400, height: 900 });
  const folder = join(launched.projectsDir, 'datasheets');
  datasheets(cargo ?? 'cargo', folder);
  await createProjectWithWizard(page, 'Parts');
  await openGenerator(page, join(folder, 'ew7021.pdf'));

  // Step 1: the part, read from the title page.
  await expect(page.getByTestId('partgen-file')).toHaveText('ew7021.pdf');
  await expect(page.getByTestId('partgen-name')).toHaveValue('EW7021');
  await expect(page.getByTestId('partgen-manufacturer')).toHaveValue('Example Semiconductors');
  await expect(
    page.locator('[data-testid="partgen-viewer"] .partgen-viewer__page[data-ready="true"]'),
  ).toBeVisible({
    timeout: 60_000,
  });

  // Help from an AI agent: the saved draft is changed outside and loaded.
  await page.getByTestId('partgen-agent').locator('summary').click();
  await page.getByTestId('partgen-agent-save').click();
  await expect(page.getByTestId('partgen-agent-status')).toContainText('.part.json');
  const status = (await page.getByTestId('partgen-agent-status').textContent()) ?? '';
  const file = status.slice(status.indexOf(':') + 1).trim();
  const saved = JSON.parse(readFileSync(file, 'utf8')) as Record<string, unknown>;
  writeFileSync(file, JSON.stringify({ ...saved, description: 'Checked by an agent' }));
  await page.getByTestId('partgen-agent-load').click();
  await expect(page.getByTestId('partgen-description')).toHaveValue('Checked by an agent');
  await shot(page, '1-part');

  // Step 2: the packages; SOIC-8 dimensions from the table on page 4.
  await page.getByTestId('partgen-next').click();
  await expect(page.getByTestId('partgen-package-SOIC-8')).toBeVisible();
  await page.getByTestId('partgen-package-SOIC-8').click();
  await expect(page.getByTestId('partgen-dim-D-nom')).toHaveText('4.9');
  await page.getByTestId('partgen-dim-e-source').click();
  await expect(page.getByTestId('partgen-page')).toContainText('4');
  await expect(page.getByTestId('partgen-highlight')).toBeVisible();
  await shot(page, '2-package');

  // Step 3: the pins of SOIC-8; one name corrected.
  await page.getByTestId('partgen-next').click();
  const rows = page.locator('[data-testid="partgen-pins"] tbody tr');
  await expect(rows).toHaveCount(8);
  const id = await page
    .locator('[data-testid^="partgen-pin-name-"][value="VCC"]')
    .getAttribute('data-testid');
  const vcc = page.getByTestId(id ?? '');
  await vcc.fill('VDD');
  await vcc.press('Enter');
  await expect(page.locator('[data-testid^="partgen-pin-name-"][value="VDD"]')).toHaveCount(1);

  await shot(page, '3-pins');

  // Step 4: previews, checks and generation into the library.
  await page.getByTestId('partgen-next').click();
  await expect(page.getByTestId('partgen-symbol').locator('svg')).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId('partgen-footprint')).toContainText('SOIC-8_3.9x4.9mm_P1.27mm');
  await expect(page.getByTestId('partgen-findings')).toContainText(/nothing to fix|found nothing/u);
  await expect(page.getByTestId('partgen-duplicate')).toHaveCount(0);
  await page.waitForTimeout(SHOTS === undefined ? 0 : 3000);
  await shot(page, '4-result');
  await page.getByTestId('partgen-generate').click();
  await expect(page.getByTestId('partgen')).toBeHidden({ timeout: 60_000 });
  await expect(page.getByText(/EW7021_SOIC-8 is in the library/u)).toBeVisible();
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Generate part/u);

  // The same part again: the duplicate check names the library part.
  await queueDialogAnswer(launched.app, join(folder, 'ew7021.pdf'));
  await page.getByTestId('libedit-datasheet').click();
  await page.getByTestId('partgen-open').click();
  await expect(page.getByTestId('partgen-step-part')).toBeVisible({ timeout: 60_000 });
  await page.getByTestId('partgen-tab-package').click();
  await page.getByTestId('partgen-package-SOIC-8').click();
  await page.getByTestId('partgen-tab-result').click();
  await expect(page.getByTestId('partgen-duplicate')).toContainText('EW7021_SOIC-8', {
    timeout: 60_000,
  });
  await expect(page.getByTestId('partgen-generate')).toBeDisabled();
  await page.getByTestId('partgen-use-existing').click();
  await expect(page.getByTestId('partgen')).toBeHidden();
});
