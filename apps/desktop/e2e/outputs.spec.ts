/**
 * Output jobs in the running app: the job of a new project generates every
 * output into the project folder, the Gerber viewer shows the files, the
 * BOM preview follows its settings and a board maker preset replaces the
 * outputs (with undo). A panel makes the board outputs for several boards,
 * the documentation output writes one HTML file, and the project details
 * show the design statistics.
 */
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, test } from '@playwright/test';

import { createProjectWithWizard, launchApp, type LaunchedApp } from './fixtures';

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

/** Every file below a folder (relative paths with `/`). */
function listFiles(dir: string, prefix = ''): string[] {
  if (!existsSync(dir)) {
    return [];
  }
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory()
      ? listFiles(join(dir, entry.name), `${prefix}${entry.name}/`)
      : [`${prefix}${entry.name}`],
  );
}

test('generates the outputs of a job and shows the Gerbers', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 860 });
  await createProjectWithWizard(page, 'Output Demo', 'demo');

  // The toolbar opens the project's output job.
  await page.getByTestId('toolbar-outputs').click();
  const editor = page.getByTestId('output-job');
  await expect(editor).toBeVisible();
  const list = page.getByTestId('outputs-list');
  await expect(list.locator('li')).toHaveCount(10);
  await expect(page.getByTestId('output-0')).toHaveAttribute('data-kind', 'gerber');

  // Generate everything.
  await page.getByTestId('outputs-generate').click();
  const results = page.getByTestId('outputs-results');
  await expect(results.locator('[data-status="done"]').first()).toBeVisible({ timeout: 60_000 });
  await expect(list.locator('[data-status="done"]')).toHaveCount(10);
  await expect(list.locator('[data-status="failed"]')).toHaveCount(0);
  const generated = join(projectsDir, 'Output Demo', 'outputs', 'generated');
  const files = listFiles(generated);
  for (const expected of [
    'gerber/Output_Demo-Copper_Top.gtl',
    'gerber/Output_Demo-PTH.drl',
    'gerber/Output_Demo-drill-map.pdf',
    'gerber/Output_Demo-fabrication-drawing.pdf',
    'assembly/Output_Demo-bom.xlsx',
    'assembly/Output_Demo-placement.csv',
    'documentation/Output_Demo-schematic.pdf',
    'documentation/Output_Demo-assembly.pdf',
    'netlists/Output_Demo.d356',
    'Output_Demo-fabrication.zip',
  ]) {
    expect(files).toContain(expected);
  }

  // The Gerber viewer shows the generated files as layers.
  await page.getByTestId('outputs-gerber-viewer').click();
  const viewer = page.getByTestId('gerber-viewer');
  await expect(viewer).toBeVisible();
  await expect(page.getByTestId('gerber-canvas')).toHaveAttribute('data-ready', 'true', {
    timeout: 30_000,
  });
  await expect(viewer.locator('.layer-list li')).toHaveCount(11);
  await expect(page.getByTestId('gerber-problems')).toHaveCount(0);

  // Back to the job: the BOM settings with a preview.
  await page.locator('[data-testid^="tab-"]', { hasText: 'Fabrication' }).first().click();
  // The fabrication drawing: IPC class 3 and a note of our own.
  await page.getByTestId('output-3').getByRole('button').click();
  await expect(page.getByTestId('output-3')).toHaveAttribute('data-kind', 'fabricationDrawing');
  await page.getByTestId('output-ipc-class').selectOption('3');
  await page.getByTestId('output-notes').fill('Electrical test 100 %');
  await page.getByTestId('output-notes').blur();
  await page.getByTestId('output-4').getByRole('button').click();
  await expect(page.getByTestId('output-settings')).toBeVisible();
  const preview = page.getByTestId('output-bom-preview');
  await expect(preview.locator('tbody tr').first()).toBeVisible();
  const lines = await preview.locator('tbody tr').count();
  await page.getByTestId('output-bom-group-value').uncheck();
  await page.getByTestId('output-bom-group-footprint').uncheck();
  await page.getByTestId('output-bom-group-MPN').uncheck();
  await page.getByTestId('output-bom-group-Manufacturer').uncheck();
  await page.getByTestId('output-bom-group-dnp').uncheck();
  await expect.poll(async () => preview.locator('tbody tr').count()).toBeGreaterThan(lines);

  // A board maker preset replaces the outputs; undo brings them back.
  await page.getByTestId('outputs-preset').selectOption('jlcpcb');
  await page
    .getByTestId('outputs-preset-confirm')
    .getByRole('button', { name: 'Apply preset' })
    .click();
  await expect(list.locator('li')).toHaveCount(5);
  await expect(list).toContainText('CPL (JLCPCB)');
  await page.getByTestId('toolbar-undo').click();
  await expect(list.locator('li')).toHaveCount(10);
});

test('makes a panel, the documentation and the design statistics', async () => {
  test.setTimeout(180_000);
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 900 });
  await createProjectWithWizard(page, 'Panel Demo', 'demo');

  // The design statistics in the project details.
  await page.getByTestId('tree-project').click();
  const stats = page.getByTestId('design-statistics');
  await expect(stats.getByTestId('statistics-board')).toHaveCount(1);
  await expect(stats.getByTestId('statistics-schematic')).toContainText('Parts');
  await expect(stats.getByTestId('statistics-drills').locator('tbody tr').first()).toBeVisible();
  await expect(stats.getByTestId('statistics-routing')).toContainText('% routed');

  // A panel: two columns, one row, V-score, no rails.
  await page.getByTestId('toolbar-outputs').click();
  await page.getByTestId('outputs-panel').getByRole('button').click();
  const form = page.getByTestId('panel-settings');
  await expect(form).toBeVisible();
  await expect(page.getByTestId('outputs-panel')).toContainText('Single boards');
  await page.getByTestId('panel-enabled').check();
  await expect(page.getByTestId('panel-image')).toBeVisible();
  await expect(page.getByTestId('panel-size')).toContainText('4 boards');
  await page.getByTestId('panel-rows').fill('1');
  await page.getByTestId('panel-rows').press('Enter');
  await expect(page.getByTestId('panel-size')).toContainText('2 boards');
  await page.getByTestId('panel-separation-vScore').click();
  await expect(page.getByTestId('panel-gap')).toHaveCount(0);
  await page.getByTestId('panel-rails').selectOption('none');
  // Fiducials and tooling holes need rails.
  await expect(form.getByTestId('panel-warning')).toHaveCount(2);
  await page.getByTestId('panel-fiducials').uncheck();
  await page.getByTestId('panel-tooling-holes').uncheck();
  await expect(form.getByTestId('panel-warning')).toHaveCount(0);
  await expect(page.getByTestId('outputs-panel')).toContainText('2 × 1, V-score');

  // The panel is saved in the job file.
  await page.getByTestId('toolbar-save').click();
  const jobFile = join(projectsDir, 'Panel Demo', 'outputs', 'fabrication.ewout');
  await expect
    .poll(() => (JSON.parse(readFileSync(jobFile, 'utf8')) as { panel?: unknown }).panel)
    .toMatchObject({ columns: 2, rows: 1, separation: 'vScore', rails: 'none' });

  // A documentation output next to the board outputs.
  await page.getByTestId('outputs-add').selectOption('documentation');
  await expect(page.getByTestId('output-documentation-schematic')).toBeChecked();
  await page.getByTestId('output-documentation-statistics').uncheck();

  // A DXF drawing of the panel: outline and V-score lines, a file each.
  await page.getByTestId('outputs-add').selectOption('dxf');
  await expect(page.getByTestId('output-dxf-default-layers')).toBeChecked();
  await page.getByTestId('output-option-separate').check();

  await page.getByTestId('outputs-generate').click();
  const list = page.getByTestId('outputs-list');
  await expect(list.locator('[data-status="done"]')).toHaveCount(12, { timeout: 90_000 });
  const generated = join(projectsDir, 'Panel Demo', 'outputs', 'generated');
  const files = listFiles(generated);
  expect(files).toContain('gerber/Panel_Demo_panel-User_1.gbr');
  expect(files).toContain('documentation/Panel_Demo-documentation.html');
  expect(files).toContain('mechanical/Panel_Demo_panel-Board_outline.dxf');
  expect(files).toContain('mechanical/Panel_Demo_panel-User_1.dxf');
  const outline = readFileSync(
    join(generated, 'mechanical', 'Panel_Demo_panel-Board_outline.dxf'),
    'utf8',
  );
  expect(outline).toContain('AC1009');
  const placement = readFileSync(
    join(generated, 'assembly', 'Panel_Demo_panel-placement.csv'),
    'utf8',
  );
  expect(placement).toContain('R1');
  const html = readFileSync(
    join(generated, 'documentation', 'Panel_Demo-documentation.html'),
    'utf8',
  );
  expect(html).toContain('id="schematic"');
  expect(html).not.toContain('id="statistics"');

  // Switched off, the board outputs are for one board again.
  await page.getByTestId('outputs-panel-enabled').uncheck();
  await expect(page.getByTestId('outputs-panel')).toContainText('Single boards');
  await expect(form.getByTestId('panel-columns')).toHaveCount(0);
});
