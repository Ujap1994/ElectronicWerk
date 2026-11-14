import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

import { expect, test } from '@playwright/test';

import { createProjectWithWizard, launchApp, type LaunchedApp } from './fixtures';

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

test('shows how to connect AI agents', async () => {
  launched = await launchApp();
  const { page } = launched;
  await page.getByTestId('open-settings').click();
  const section = page.getByTestId('settings-automation');
  await expect(section).toContainText('AI agents and scripts');
  const config = JSON.parse((await page.getByTestId('automation-config').textContent()) ?? '') as {
    mcpServers: { electronicwerk: { command: string; args: string[] } };
  };
  expect(config.mcpServers.electronicwerk).toEqual({ command: 'ewerk', args: ['mcp'] });
});

test('never overwrites a file another program changed', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await createProjectWithWizard(page, 'Shared');
  // The app changes the project…
  const revision = page.getByTestId('project-revision');
  await revision.fill('B');
  await revision.press('Enter');
  await expect(page.getByTestId('dirty-indicator')).toHaveAttribute('data-dirty', 'true');
  // …while an agent changes the same file on disk.
  const manifestFile = join(projectsDir, 'Shared', 'Shared.ewproj');
  const manifest = JSON.parse(readFileSync(manifestFile, 'utf8')) as Record<string, unknown>;
  manifest['description'] = 'Changed by an agent';
  writeFileSync(manifestFile, `${JSON.stringify(manifest, null, 2)}\n`);
  await page.getByTestId('toolbar-save').click();
  await expect(page.getByTestId('notice').last()).toContainText(
    'Not saved: another program (an AI agent, the command line or another window) changed Shared.ewproj',
  );
  await expect(page.getByTestId('dirty-indicator')).toHaveAttribute('data-dirty', 'true');
  const onDisk = JSON.parse(readFileSync(manifestFile, 'utf8')) as Record<string, unknown>;
  expect(onDisk['description']).toBe('Changed by an agent');
});
