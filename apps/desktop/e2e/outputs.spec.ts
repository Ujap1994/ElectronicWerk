/**
 * Output jobs in the running app: the job of a new project generates every
 * output into the project folder, the Gerber viewer shows the files, the
 * BOM preview follows its settings and a board maker preset replaces the
 * outputs (with undo).
 */
import { existsSync, readdirSync } from 'node:fs';
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
  await expect(list.locator('li')).toHaveCount(9);
  await expect(page.getByTestId('output-0')).toHaveAttribute('data-kind', 'gerber');

  // Generate everything.
  await page.getByTestId('outputs-generate').click();
  const results = page.getByTestId('outputs-results');
  await expect(results.locator('[data-status="done"]').first()).toBeVisible({ timeout: 60_000 });
  await expect(list.locator('[data-status="done"]')).toHaveCount(9);
  await expect(list.locator('[data-status="failed"]')).toHaveCount(0);
  const generated = join(projectsDir, 'Output Demo', 'outputs', 'generated');
  const files = listFiles(generated);
  for (const expected of [
    'gerber/Output_Demo-Copper_Top.gtl',
    'gerber/Output_Demo-PTH.drl',
    'gerber/Output_Demo-drill-map.pdf',
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
  await page.getByTestId('output-3').getByRole('button').click();
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
  await expect(list.locator('li')).toHaveCount(9);
});
