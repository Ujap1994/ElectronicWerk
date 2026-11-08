import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { expect, test, type Page } from '@playwright/test';

import {
  createProjectWithWizard,
  killHard,
  launchApp,
  queueDialogAnswer,
  removeTemp,
  type LaunchedApp,
} from './fixtures';

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

const manifest = (folder: string, name: string): Record<string, unknown> =>
  JSON.parse(readFileSync(join(folder, `${name}.ewproj`), 'utf8')) as Record<string, unknown>;

async function setRevision(page: Page, revision: string): Promise<void> {
  const field = page.getByTestId('project-revision');
  await field.fill(revision);
  await field.press('Enter');
  await expect(page.getByTestId('dirty-indicator')).toHaveAttribute('data-dirty', 'true');
}

test('creates a project with the wizard', async () => {
  launched = await launchApp();
  const { page, app, projectsDir } = launched;
  await page.getByTestId('new-project').click();
  const wizard = page.getByTestId('wizard');
  await expect(wizard).toBeVisible();

  // The name is required.
  await expect(page.getByTestId('wizard-create')).toBeDisabled();
  await page.getByTestId('wizard-next').click();
  await expect(wizard.getByText('Enter a name.')).toBeVisible();

  await page.getByTestId('wizard-name').fill('Blinky');
  await expect(page.getByTestId('wizard-folder')).toContainText(join(projectsDir, 'Blinky'));
  await page.getByTestId('wizard-next').click();
  await page.getByTestId('template-board-4l').click();
  await page.getByTestId('wizard-next').click();
  await page.getByTestId('board-width').fill('50');
  await page.getByTestId('board-height').fill('3');
  await expect(wizard.getByText('Use 5 to 2000 mm.')).toBeVisible();
  await expect(page.getByTestId('wizard-create')).toBeDisabled();
  await page.getByTestId('board-height').fill('40,5');
  await page.getByTestId('wizard-create').click();

  await expect(page.getByTestId('workspace')).toBeVisible();
  await expect(page.getByTestId('project-title')).toHaveText('Blinky');
  await expect(page.getByTestId('dirty-indicator')).toHaveText('All changes saved');
  const folder = join(projectsDir, 'Blinky');
  expect(existsSync(join(folder, 'Blinky.ewproj'))).toBe(true);
  expect(existsSync(join(folder, 'pcb', 'blinky.ewpcb'))).toBe(true);
  expect(existsSync(join(folder, '.gitignore'))).toBe(true);

  // The tree shows the documents; the board details show the chosen size.
  await page
    .getByTestId(/^tree-doc:/u)
    .filter({ hasText: 'Blinky' })
    .click();
  await expect(page.getByTestId('details-pcb')).toContainText('50 × 40.5 mm');
  await expect(page.getByTestId('details-pcb')).toContainText('4');
  await expect(page.getByTestId('stackup')).toBeVisible();

  const title = await app.evaluate(() =>
    (globalThis as unknown as { __ewTest: { windowTitle: () => string } }).__ewTest.windowTitle(),
  );
  expect(title).toBe('Blinky — ElectronicWerk');
});

test('edits properties with undo, redo and save', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await createProjectWithWizard(page, 'Edit Me');
  const folder = join(projectsDir, 'Edit Me');

  await setRevision(page, 'B');
  await expect(page.getByTestId('status-dirty')).toBeVisible();

  await page.getByTestId('variable-new-name').fill('author');
  await page.getByTestId('variable-new-value').fill('Lab');
  await page.getByTestId('variable-add').click();
  await expect(page.getByTestId('variable-AUTHOR')).toBeVisible();

  // Undo and redo through the toolbar.
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute(
    'title',
    'Undo: Change project properties',
  );
  await page.getByTestId('toolbar-undo').click();
  await expect(page.getByTestId('variable-AUTHOR')).toHaveCount(0);
  await page.getByTestId('toolbar-redo').click();
  await expect(page.getByTestId('variable-AUTHOR')).toBeVisible();

  await page.getByTestId('toolbar-save').click();
  await expect(page.getByTestId('dirty-indicator')).toHaveText('All changes saved');
  await expect(page.getByTestId('notice').first()).toContainText('Project saved.');
  expect(manifest(folder, 'Edit Me')).toMatchObject({
    revision: 'B',
    variables: { AUTHOR: 'Lab' },
  });
  expect(existsSync(join(folder, '.ewerk', 'journal.jsonl'))).toBe(false);

  // The project check reports no problems.
  await page.getByTestId('toolbar-validate').click();
  await expect(page.getByTestId('validation-summary')).toContainText('No problems found');
});

test('manages documents in the project tree', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await createProjectWithWizard(page, 'Tree');

  // Add a sub-sheet to the top sheet and rename it in place.
  const root = page.getByTestId(/^tree-doc:/u).filter({ hasText: 'Root' });
  await root.click();
  await page
    .getByTestId(/^tree-actions-doc:/u)
    .first()
    .click();
  await page.getByTestId('menu-add-sub-sheet').click();
  const rename = page.getByTestId('tree-rename');
  await expect(rename).toBeFocused();
  await rename.fill('Power Supply');
  await rename.press('Enter');
  const sheet = page.getByTestId(/^tree-doc:/u).filter({ hasText: 'Power Supply' });
  await expect(sheet).toBeVisible();
  await expect(page.getByTestId('details-schematic')).toBeVisible();

  // Paper settings are document properties.
  await page.getByTestId('page-format').selectOption('a3');
  await expect(page.getByTestId('details-schematic')).toContainText('420 × 297 mm');

  await page.getByTestId('toolbar-save').click();
  await expect(page.getByTestId('dirty-indicator')).toHaveText('All changes saved');
  const file = join(projectsDir, 'Tree', 'schematics', 'new-sheet.ewsch');
  expect(readFileSync(file, 'utf8')).toContain('"name": "Power Supply"');

  // Remove it (with confirmation), then undo.
  await sheet.click();
  await page.keyboard.press('Delete');
  await expect(page.getByTestId('remove-dialog')).toContainText('Power Supply');
  await page.getByTestId('confirm-yes').click();
  await expect(sheet).toHaveCount(0);
  await page.getByTestId('toolbar-undo').click();
  await expect(sheet).toBeVisible();
});

test('asks about unsaved changes when closing the project', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await createProjectWithWizard(page, 'Closing');
  await setRevision(page, 'Z');

  await page.getByTestId('toolbar-close').click();
  await expect(page.getByTestId('close-dialog')).toContainText('“Closing”');
  await page.getByTestId('close-cancel').click();
  await expect(page.getByTestId('workspace')).toBeVisible();

  await page.getByTestId('toolbar-close').click();
  await page.getByTestId('close-dont-save').click();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Welcome to ElectronicWerk');
  const folder = join(projectsDir, 'Closing');
  expect(manifest(folder, 'Closing')['revision']).toBe('A');
  expect(existsSync(join(folder, '.ewerk', 'journal.jsonl'))).toBe(false);

  // The project is in the recent list and opens again.
  await expect(page.getByTestId('recent-list')).toContainText('Closing');
  await page.getByTestId('recent-open').first().click();
  await expect(page.getByTestId('project-title')).toHaveText('Closing');
});

test('saves before the window closes', async () => {
  launched = await launchApp();
  const { app, page, projectsDir } = launched;
  await createProjectWithWizard(page, 'Quit');
  await setRevision(page, 'Q');

  const closed = new Promise<void>((resolve) => {
    app.process().on('exit', () => {
      resolve();
    });
  });
  // Ask the window to close, as the close button would.
  await app.evaluate(({ BrowserWindow }) => {
    BrowserWindow.getAllWindows()[0]?.close();
  });
  await expect(page.getByTestId('close-dialog')).toBeVisible();
  await page.getByTestId('close-save').click();
  await closed;
  expect(manifest(join(projectsDir, 'Quit'), 'Quit')['revision']).toBe('Q');
});

test('restores the project after an engine crash', async () => {
  launched = await launchApp();
  const { app, page } = launched;
  await createProjectWithWizard(page, 'Crashy');
  await setRevision(page, 'C');

  await app.evaluate(() => {
    const hooks = (globalThis as { __ewTest?: { engine: { crashForTest: () => void } } }).__ewTest;
    hooks?.engine.crashForTest();
  });
  await expect(page.getByTestId('notice').last()).toContainText('The engine restarted');
  await expect(page.getByTestId('project-revision')).toHaveValue('C');
  await expect(page.getByTestId('dirty-indicator')).toHaveAttribute('data-dirty', 'true');
  // Undo still works after the restart: the history was replayed.
  await page.getByTestId('toolbar-undo').click();
  await expect(page.getByTestId('project-revision')).toHaveValue('A');
});

test('offers to restore unsaved work after the app was killed', async () => {
  const userDataDir = mkdtempSync(join(tmpdir(), 'ew-e2e-kill-'));
  const projectsDir = mkdtempSync(join(tmpdir(), 'ew-projects-kill-'));
  try {
    writeFileSync(join(userDataDir, 'settings.json'), JSON.stringify({ language: 'en' }));
    const first = await launchApp({ userDataDir, projectsDir });
    launched = first;
    await createProjectWithWizard(first.page, 'Killed');
    await setRevision(first.page, 'K');
    // Hard kill: no chance to save or clean up.
    await killHard(first.app);
    launched = undefined;

    const second = await launchApp({ userDataDir, projectsDir });
    launched = second;
    const { page } = second;
    await expect(page.getByTestId('recent-recovery')).toBeVisible();
    await page.getByTestId('recent-open').first().click();
    await expect(page.getByTestId('recovery-dialog')).toContainText('1 unsaved changes');
    await page.getByTestId('recovery-restore').click();
    await expect(page.getByTestId('project-revision')).toHaveValue('K');
    await expect(page.getByTestId('dirty-indicator')).toHaveAttribute('data-dirty', 'true');
    await page.getByTestId('toolbar-save').click();
    await expect(page.getByTestId('dirty-indicator')).toHaveText('All changes saved');
    expect(manifest(join(projectsDir, 'Killed'), 'Killed')['revision']).toBe('K');
  } finally {
    await launched?.close();
    launched = undefined;
    removeTemp(userDataDir);
    removeTemp(projectsDir);
  }
});

test('opens, saves as, copies and archives projects', async () => {
  launched = await launchApp();
  const { app, page, projectsDir } = launched;
  await createProjectWithWizard(page, 'Source', 'arduino-shield');
  await page.getByTestId('toolbar-close').click();
  await expect(page.getByTestId('new-project')).toBeVisible();

  // Open through the (stubbed) file dialog.
  await queueDialogAnswer(app, join(projectsDir, 'Source', 'Source.ewproj'));
  await page.getByTestId('open-project').click();
  await expect(page.getByTestId('project-title')).toHaveText('Source');

  // Archive.
  const archive = join(projectsDir, 'Source.zip');
  await queueDialogAnswer(app, archive);
  await page.getByTestId('toolbar-archive').click();
  await expect(page.getByTestId('notice').last()).toContainText('Archive written');
  expect(existsSync(archive)).toBe(true);

  // Save a copy, then save as (switches to the new folder).
  const copies = mkdtempSync(join(tmpdir(), 'ew-copies-'));
  const moved = mkdtempSync(join(tmpdir(), 'ew-moved-'));
  try {
    await setRevision(page, 'S');
    await queueDialogAnswer(app, copies);
    await app.evaluate(({ BrowserWindow }) => {
      BrowserWindow.getAllWindows()[0]?.webContents.send('menu:action', 'save-copy');
    });
    await expect(page.getByTestId('notice').last()).toContainText('Copy saved');
    expect(manifest(join(copies, 'Source'), 'Source')['revision']).toBe('S');
    await expect(page.getByTestId('dirty-indicator')).toHaveAttribute('data-dirty', 'true');

    await queueDialogAnswer(app, moved);
    await app.evaluate(({ BrowserWindow }) => {
      BrowserWindow.getAllWindows()[0]?.webContents.send('menu:action', 'save-as');
    });
    await expect(page.getByTestId('notice').last()).toContainText('now saved in');
    await expect(page.getByTestId('dirty-indicator')).toHaveText('All changes saved');
    await expect(page.getByTestId('project-path')).toContainText(join(moved, 'Source'));
    expect(manifest(join(projectsDir, 'Source'), 'Source')['revision']).toBe('A');
  } finally {
    rmSync(copies, { recursive: true, force: true });
    rmSync(moved, { recursive: true, force: true });
  }
});
