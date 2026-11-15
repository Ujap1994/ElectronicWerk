/**
 * Importing a KiCad project, a netlist and a KiCad library through the UI
 * (with the synthetic KiCad project from crates/ew-kicad/tests/fixtures).
 */
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { expect, test } from '@playwright/test';

import { APP_DIR, launchApp, queueDialogAnswer, type LaunchedApp } from './fixtures';
import { viewerCanvas } from './viewer-helpers';

const FIXTURE = resolve(APP_DIR, '../../crates/ew-kicad/tests/fixtures/blinky');

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

test('imports a KiCad project, a netlist and a library', async () => {
  launched = await launchApp();
  const { app, page, projectsDir } = launched;

  // The KiCad project.
  await queueDialogAnswer(app, join(FIXTURE, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  const dialog = page.getByTestId('kicad-import-dialog');
  await expect(dialog).toBeVisible();
  await expect(page.getByTestId('kicad-import-name')).toHaveValue('blinky');
  await page.getByTestId('kicad-import-name').fill('Blinky Import');
  await expect(page.getByTestId('kicad-import-target')).toContainText('Blinky Import');
  await page.getByTestId('kicad-import-start').click();

  const report = page.getByTestId('import-report');
  await expect(report).toBeVisible();
  await expect(page.getByTestId('import-counts')).toContainText('Footprints');
  await expect(page.getByTestId('import-summary')).toContainText('Errors: 0');
  await expect(report).toContainText('Custom design rules are not imported');
  await report.getByRole('button', { name: 'Close', exact: true }).last().click();
  await expect(page.getByTestId('workspace')).toBeVisible();
  await expect(page.getByTestId('project-title')).toHaveText('Blinky Import');
  await expect(
    page.locator('[data-testid^="tree-file:"]', { hasText: 'kicad-import-report.md' }),
  ).toBeVisible();

  // The netlist matches the board.
  const board = page.locator('[data-testid^="tree-doc:"]', { hasText: 'Blinky Import' });
  await queueDialogAnswer(app, join(FIXTURE, 'blinky.net'));
  await board.click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Import netlist…' }).click();
  await expect(page.getByTestId('netlist-summary')).toContainText('exactly like the netlist');
  await expect(page.getByTestId('netlist-apply')).toHaveCount(0);
  await page
    .getByTestId('netlist-dialog')
    .getByRole('button', { name: 'Close', exact: true })
    .last()
    .click();

  // A changed netlist can be applied (and undone).
  const changed = join(projectsDir, 'changed.csv');
  writeFileSync(
    changed,
    readFileSync(join(FIXTURE, 'blinky-netlist.csv'), 'utf8').replace('/LED_A;D1;2', '/LED_B;D1;2'),
  );
  await queueDialogAnswer(app, changed);
  await board.click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Import netlist…' }).click();
  await expect(page.getByTestId('netlist-differences')).toContainText('D1-2');
  await expect(page.getByTestId('netlist-changes')).toContainText('1 pads');
  await page.getByTestId('netlist-apply').click();
  await expect(page.getByTestId('notice').last()).toContainText('Netlist applied');
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Import netlist/u);

  // A library that is already part of the project adds nothing.
  await queueDialogAnswer(app, join(FIXTURE, 'Blinky.kicad_sym'));
  await page.locator('[data-testid="tree-group:libraries"]').click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Import KiCad library file…' }).click();
  await expect(page.getByTestId('notice').last()).toContainText('Added 0 symbols');
  await expect(page.getByTestId('notice').last()).toContainText('2 were already in the library');

  // The imported board opens in the viewer, with the zone fill on In1.
  await board.dblclick();
  await viewerCanvas(page).waitFor();
  await expect(page.getByTestId('layer-cu.in1')).toBeVisible();
  await expect(page.getByTestId('layer-cu.in1')).toHaveAttribute('data-empty', 'false');
});

test('exports a project as a KiCad project', async () => {
  launched = await launchApp();
  const { app, page, projectsDir } = launched;
  await queueDialogAnswer(app, join(FIXTURE, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-name').fill('Round Trip');
  await page.getByTestId('kicad-import-start').click();
  const report = page.getByTestId('import-report');
  await expect(report).toBeVisible();
  await report.getByRole('button', { name: 'Close', exact: true }).last().click();
  await expect(page.getByTestId('workspace')).toBeVisible();

  // From the command palette into a new folder.
  const target = join(projectsDir, 'Round Trip KiCad');
  await queueDialogAnswer(app, target);
  await page.getByTestId('open-palette').click();
  await page.getByTestId('command-input').fill('export as a kicad');
  await expect(page.getByTestId('command-palette').getByRole('option').first()).toContainText(
    'Export as a KiCad project',
  );
  await page.keyboard.press('Enter');
  await expect(page.getByText(/KiCad project written: .*Round_Trip\.kicad_pro/u)).toBeVisible();
  const files = readdirSync(target).sort();
  expect(files).toEqual(
    expect.arrayContaining([
      'Power.kicad_sch',
      'Round_Trip.kicad_pcb',
      'Round_Trip.kicad_pro',
      'Round_Trip.kicad_sch',
    ]),
  );
  expect(readFileSync(join(target, 'Round_Trip.kicad_pcb'), 'utf8')).toContain(
    '(version 20240108)',
  );

  // The export reads back as a project of its own.
  await page.getByTestId('toolbar-close').click();
  await queueDialogAnswer(app, join(target, 'Round_Trip.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-start').click();
  await expect(page.getByTestId('import-summary')).toContainText(
    'Everything was imported without problems.',
  );
  expect(existsSync(join(projectsDir, 'Round_Trip'))).toBe(true);
});

test('reports a file that is not a KiCad project', async () => {
  launched = await launchApp();
  const { app, page } = launched;
  await queueDialogAnswer(app, join(FIXTURE, 'Blinky.pretty'));
  await page.getByTestId('import-kicad').click();
  await expect(page.getByTestId('notice').last()).toBeVisible();
  await expect(page.getByTestId('kicad-import-dialog')).toHaveCount(0);
});
