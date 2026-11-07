import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { expect, test } from '@playwright/test';

import { fakeRelease, launchApp } from './fixtures';

const NOTES =
  '## Added\n\n- **Faster** rendering\n- New [guide](https://github.com/Ujap1994/ElectronicWerk#readme)';

function readSettings(userDataDir: string): { updates: { skippedVersion: string | null } } {
  return JSON.parse(readFileSync(join(userDataDir, 'settings.json'), 'utf8')) as {
    updates: { skippedVersion: string | null };
  };
}

test('announces a newer release at startup and can skip it', async () => {
  const profile = mkdtempSync(join(tmpdir(), 'ew-e2e-'));
  writeFileSync(join(profile, 'settings.json'), JSON.stringify({ language: 'en' }));
  try {
    // First start: the dialog appears with version, date and release notes.
    let launched = await launchApp({
      userDataDir: profile,
      updateFeed: fakeRelease('9.9.0', NOTES),
    });
    const dialog = launched.page.getByTestId('update-dialog');
    await expect(dialog).toBeVisible();
    await expect(dialog).toContainText('ElectronicWerk 9.9.0 is available');
    await expect(dialog).toContainText('Installed version: 0.1.0');
    await expect(dialog).toContainText('Released on December 1, 2026');
    await expect(launched.page.getByTestId('update-notes').locator('strong')).toHaveText('Faster');
    await expect(launched.page.getByTestId('update-notes').locator('li')).toHaveCount(2);

    await launched.page.getByTestId('update-skip').check();
    await launched.page.getByTestId('update-later').click();
    await expect(dialog).toBeHidden();
    await expect.poll(() => readSettings(profile).updates.skippedVersion).toBe('9.9.0');
    await launched.close();

    // Second start: the skipped version is not announced again …
    launched = await launchApp({ userDataDir: profile, updateFeed: fakeRelease('9.9.0', NOTES) });
    await expect(launched.page.getByTestId('engine-status')).toHaveAttribute('data-state', 'ready');
    await launched.page.waitForTimeout(500);
    await expect(launched.page.getByTestId('update-dialog')).toBeHidden();

    // … but a manual check still shows it, with "skip" pre-selected.
    await launched.page.getByTestId('open-settings').click();
    await expect(launched.page.getByTestId('skipped-version')).toContainText('9.9.0');
    await launched.page.getByTestId('check-updates-now').click();
    await expect(launched.page.getByTestId('update-dialog')).toContainText('9.9.0 is available');
    await expect(launched.page.getByTestId('update-skip')).toBeChecked();
    await launched.close();
  } finally {
    rmSync(profile, { recursive: true, force: true });
  }
});

test('reports "up to date" on a manual check', async () => {
  const launched = await launchApp({ updateFeed: fakeRelease('0.1.0') });
  try {
    await launched.page.getByTestId('open-settings').click();
    await launched.page.getByTestId('check-updates-now').click();
    const dialog = launched.page.getByTestId('update-dialog');
    await expect(dialog).toContainText('No update available');
    await expect(dialog).toContainText('ElectronicWerk 0.1.0 is the latest version.');
  } finally {
    await launched.close();
  }
});

test('reports that no release exists yet', async () => {
  const launched = await launchApp({ updateFeed: null });
  try {
    await launched.page.getByTestId('open-settings').click();
    await launched.page.getByTestId('check-updates-now').click();
    await expect(launched.page.getByTestId('update-dialog')).toContainText(
      'No release has been published yet.',
    );
  } finally {
    await launched.close();
  }
});

test('shows an error when the update check fails', async () => {
  const launched = await launchApp({ updateFeed: '{ this is not json' });
  try {
    await launched.page.getByTestId('open-settings').click();
    await launched.page.getByTestId('check-updates-now').click();
    await expect(launched.page.getByTestId('update-dialog')).toContainText('Update check failed');
  } finally {
    await launched.close();
  }
});

test('does not check at startup when disabled', async () => {
  const launched = await launchApp({
    settings: { updates: { checkOnStartup: false } },
    updateFeed: fakeRelease('9.9.0'),
  });
  try {
    await expect(launched.page.getByTestId('engine-status')).toHaveAttribute('data-state', 'ready');
    await launched.page.waitForTimeout(500);
    await expect(launched.page.getByTestId('update-dialog')).toBeHidden();
  } finally {
    await launched.close();
  }
});
