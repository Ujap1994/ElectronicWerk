/**
 * The library manager (M4) with the synthetic KiCad library from
 * crates/ew-library/tests/fixtures: building from local folders, search,
 * 2D and 3D previews, duplicate review, adding parts to a project, and
 * downloading library packs from a (local) release.
 */
import { execFileSync } from 'node:child_process';
import { mkdtempSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

import { expect, test, type Page } from '@playwright/test';

import {
  APP_DIR,
  createProjectWithWizard,
  launchApp,
  queueDialogAnswer,
  removeTemp,
  type LaunchedApp,
} from './fixtures';

const REPO = resolve(APP_DIR, '../..');
const FIXTURES = join(REPO, 'crates/ew-library/tests/fixtures');

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

function row(page: Page, name: string) {
  return page.locator(`.library-row[data-name="${name}"]`);
}

test('builds the library from KiCad folders, searches, previews and reviews duplicates', async () => {
  test.setTimeout(120_000);
  launched = await launchApp();
  const { app, page } = launched;

  // No library yet: the setup offers the local KiCad libraries.
  await page.getByTestId('library').click();
  await expect(page.getByTestId('library-setup')).toBeVisible();
  await expect(page.getByTestId('library-setup')).toContainText(
    'No library release is available yet',
  );
  await queueDialogAnswer(app, join(FIXTURES, 'kicad'));
  await page.getByTestId('library-local').click();
  await expect(page.getByTestId('library-manager')).toHaveAttribute('data-installed', 'true', {
    timeout: 60_000,
  });
  await expect(page.getByTestId('notice').last()).toContainText('The library is ready');

  // Search footprints and look at one in 2D and 3D.
  await page.getByTestId('library-kind-footprint').click();
  await page.getByTestId('library-search').fill('R_0805');
  await expect(page.getByTestId('library-count')).toContainText('2 results');
  await row(page, 'Demo:R_0805').click();
  await expect(page.getByTestId('library-details')).toHaveAttribute('data-name', 'Demo:R_0805');
  await expect(page.getByTestId('library-preview')).toHaveAttribute('data-state', 'ready');
  await expect(page.getByTestId('library-aliases')).toContainText('Demo_Copy:R_0805');
  await page.getByTestId('library-tab-3d').click();
  // The STEP model is tessellated on demand (OpenCASCADE in a worker).
  await expect(page.getByTestId('library-model')).toHaveAttribute('data-state', 'ready', {
    timeout: 60_000,
  });

  // Symbols are found by part number, too.
  await page.getByTestId('library-kind-all').click();
  await page.getByTestId('library-search').fill('LMA-123');
  await row(page, 'Demo:LM_A').click();
  await expect(page.getByTestId('library-details')).toContainText('Demo Semi');
  await expect(page.getByTestId('library-preview')).toHaveAttribute('data-state', 'ready');
  await expect(page.getByTestId('library-groups')).toContainText('Alias');

  // Merged duplicates are hidden unless asked for.
  await page.getByTestId('library-kind-footprint').click();
  await page.getByTestId('library-search').fill('');
  await expect(page.getByTestId('library-count')).toContainText('5 results');
  await page.getByTestId('library-show-merged').check();
  await expect(page.getByTestId('library-count')).toContainText('6 results');

  // Review the near duplicates (symbols and footprints; the 3D models have
  // no meshes yet, so they are not compared).
  await page.getByTestId('library-tab-duplicates').click();
  await expect(page.getByTestId('duplicates-count')).toContainText('2 groups');
  await page.locator('[data-testid="duplicates-list"] .vlist__row').first().click();
  await expect(page.getByTestId('duplicate-member').first()).toBeVisible();
  await page.getByTestId('duplicate-merge').click();
  await expect(page.getByTestId('duplicate-group')).toHaveAttribute('data-decision', 'merge');
  await expect(page.getByTestId('duplicates-count')).toContainText('1 group');

  // Escape closes the manager; the start screen is back.
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('library-manager')).toHaveCount(0);
  await expect(page.getByTestId('new-project')).toBeVisible();

  // With a project open, parts go into its library (one undoable step).
  await createProjectWithWizard(page, 'Library Test');
  await page.getByTestId('toolbar-library').click();
  await page.getByTestId('library-tab-browse').click();
  await page.getByTestId('library-kind-symbol').click();
  await page.getByTestId('library-search').fill('LM_B');
  await row(page, 'Demo:LM_B').click();
  await page.getByTestId('library-add').click();
  await expect(page.getByTestId('notice').last()).toContainText('Added 1 symbols');
  await page.getByTestId('library-add').click();
  await expect(page.getByTestId('notice').last()).toContainText('already has Demo:LM_B');
  await page.getByTestId('library-close').click();
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Add from library/u);
});

test('downloads the library and its 3D models from a release', async () => {
  test.setTimeout(240_000);
  const cargo = findCargo();
  test.skip(cargo === null, 'needs cargo to make a library release');
  // A library release made with the command-line tool, like the Library workflow does.
  const work = mkdtempSync(join(tmpdir(), 'ew-library-release-'));
  const ewerk = (...args: string[]): void => {
    execFileSync(cargo ?? 'cargo', ['run', '-q', '-p', 'ew-cli', '--', 'library', ...args], {
      cwd: REPO,
      stdio: 'pipe',
    });
  };
  try {
    ewerk(
      'build',
      '--kicad',
      join(FIXTURES, 'kicad'),
      '--meshes',
      join(FIXTURES, 'meshes'),
      '--version',
      'test',
      '--dir',
      join(work, 'built'),
    );
    ewerk(
      'pack',
      join(work, 'feed'),
      '--prefix',
      'ewerk-library-test',
      '--name',
      'Test library',
      '--dir',
      join(work, 'built'),
    );

    launched = await launchApp({ libraryFeed: join(work, 'feed') });
    const { page } = launched;
    await page.getByTestId('library').click();
    await expect(page.getByTestId('library-setup')).toContainText('Test library');
    await page.getByTestId('library-download').click();
    await expect(page.getByTestId('library-manager')).toHaveAttribute('data-installed', 'true', {
      timeout: 60_000,
    });
    // The 3D model comes from the mesh pack of its library, downloaded on demand.
    await page.getByTestId('library-kind-model').click();
    await page.getByTestId('library-search').fill('SOIC');
    await row(page, 'Demo:SOIC-8').click();
    await expect(page.getByTestId('library-model')).toHaveAttribute('data-state', 'ready', {
      timeout: 60_000,
    });
    await page.getByTestId('library-info-button').click();
    await expect(page.getByTestId('library-info')).toContainText('6 symbols');
  } finally {
    removeTemp(work);
  }
});
