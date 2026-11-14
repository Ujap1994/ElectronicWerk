import { execFile } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { expect, test, type Page } from '@playwright/test';

import { FAKE_USER_CODE, startFakeGithub, type FakeGithub } from '../test/github-server';
import { launchApp, removeTemp, type LaunchedApp } from './fixtures';

let launched: LaunchedApp | undefined;
let github: FakeGithub;

test.beforeEach(async () => {
  github = await startFakeGithub();
});

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
  await github.close();
});

/** Starts the app with the local GitHub (never github.com). */
async function launch(projectsDir?: string): Promise<LaunchedApp> {
  launched = await launchApp({
    ...(projectsDir === undefined ? {} : { projectsDir }),
    env: {
      EW_GITHUB_API: github.api,
      EW_GITHUB_WEB: github.url,
      EW_GITHUB_CLIENT_ID: 'Iv1.e2e-client',
    },
  });
  return launched;
}

async function openGithubSettings(page: Page): Promise<void> {
  await page.getByTestId('open-settings').click();
  await expect(page.getByTestId('settings-github')).toBeVisible();
}

async function signInWithToken(page: Page): Promise<void> {
  await page.getByTestId('github-token-input').fill(github.token);
  await page.getByTestId('github-token-sign-in').click();
  await expect(page.getByTestId('github-sign-in')).toBeHidden();
}

/**
 * Runs the system git in a folder (to play "someone else"). Asynchronous:
 * the local GitHub runs in this process and must keep answering.
 */
function git(cwd: string, ...args: string[]): Promise<string> {
  return new Promise((resolve, reject) => {
    // No credential helper and no prompts (the token is in the URL).
    const options = {
      cwd,
      encoding: 'utf8' as const,
      env: { ...process.env, GIT_TERMINAL_PROMPT: '0' },
    };
    execFile('git', ['-c', 'credential.helper=', ...args], options, (error, stdout, stderr) => {
      if (error === null) {
        resolve(stdout);
      } else {
        reject(new Error(`git ${args.join(' ')}: ${stderr}`));
      }
    });
  });
}

test('signs in with a token or the device flow and signs out', async () => {
  const { page, app } = await launch();
  await openGithubSettings(page);
  await expect(page.getByTestId('github-account')).toHaveText('Not signed in');
  await expect(page.getByTestId('github-storage')).toContainText('system keychain');

  // A wrong token is rejected.
  await page.getByTestId('github-sign-in-open').click();
  const dialog = page.getByTestId('github-sign-in');
  await expect(dialog).toBeVisible();
  await page.getByTestId('github-token-input').fill('ghp_wrong');
  await page.getByTestId('github-token-sign-in').click();
  await expect(page.getByTestId('github-token-error')).toContainText('GitHub rejected');
  await page.getByTestId('github-token-create').click();
  await signInWithToken(page);
  await expect(page.getByTestId('github-account')).toContainText('Signed in as octo-maker');
  await expect(page.getByTestId('github-account')).toContainText('Octo Maker');

  await page.getByTestId('github-sign-out').click();
  await expect(page.getByTestId('github-account')).toHaveText('Not signed in');

  // The device flow: the code is shown, GitHub confirms.
  await page.getByTestId('github-sign-in-open').click();
  await page.getByTestId('github-device-start').click();
  await expect(page.getByTestId('github-device-code')).toHaveText(FAKE_USER_CODE);
  await page.getByTestId('github-device-open').click();
  github.approveDevice();
  await expect(dialog).toBeHidden();
  await expect(page.getByTestId('github-account')).toContainText('Signed in as octo-maker');

  // Pages open in the browser (recorded by the test hooks instead).
  const pages = await app.evaluate(() =>
    (globalThis as unknown as { __ewTest: { githubPages: () => string[] } }).__ewTest.githubPages(),
  );
  expect(pages).toEqual([
    `${github.url}/settings/tokens/new?scopes=repo%2Cread%3Auser&description=ElectronicWerk`,
    `${github.url}/login/device`,
  ]);
});

test('creates a project on GitHub, commits, pushes, pulls and branches', async () => {
  const { page, projectsDir } = await launch();

  // The wizard asks to sign in first.
  await page.getByTestId('new-project').click();
  await page.getByTestId('wizard-name').fill('Bench Supply');
  await page.getByTestId('wizard-storage-github').click();
  await expect(page.getByTestId('repo-target')).toContainText('Sign in to GitHub');
  await page.getByTestId('repo-target-sign-in').click();
  await signInWithToken(page);
  await expect(page.getByTestId('repo-target-account')).toHaveText('GitHub account: octo-maker');
  await expect(page.getByTestId('repo-target-name')).toHaveValue('Bench-Supply');
  await expect(page.getByTestId('repo-target-private')).toBeChecked();
  await page.getByTestId('repo-target-description').fill('A small bench supply');
  await page.getByTestId('wizard-next').click();
  await page.getByTestId('template-board-2l').click();
  await page.getByTestId('wizard-create').click();
  await expect(page.getByTestId('workspace')).toBeVisible();
  await expect(page.getByText('The project is on GitHub: octo-maker/Bench-Supply')).toBeVisible();

  const repo = 'octo-maker/Bench-Supply';
  expect(github.repos().find((entry) => `${entry.owner}/${entry.name}` === repo)).toMatchObject({
    private: true,
    description: 'A small bench supply',
  });
  const files = github.git(repo, ['ls-tree', '-r', '--name-only', 'main']).trim().split('\n');
  expect(files).toContain('Bench Supply.ewproj');
  expect(files).toContain('README.md');
  expect(files).toContain('.gitignore');
  expect(files.some((file) => file.startsWith('.ewerk/'))).toBe(false);

  // Source control: up to date.
  await page.getByTestId('toolbar-git').click();
  const panel = page.getByTestId('source-control');
  await expect(panel).toBeVisible();
  await expect(page.getByTestId('git-remote')).toHaveText(repo);
  await expect(page.getByTestId('git-sync')).toHaveText('Up to date with GitHub');
  await expect(page.getByTestId('git-no-changes')).toBeVisible();
  await expect(page.getByTestId('git-log')).toContainText('Create Bench Supply');

  // A change: unsaved first, then saved and committed.
  await page.getByTestId('tree-project').click();
  const revision = page.getByTestId('project-revision');
  // (New projects start with revision A.)
  await revision.fill('B');
  await revision.press('Enter');
  await page.getByTestId('toolbar-git').click();
  await expect(page.getByTestId('git-save-first')).toBeVisible();
  await page.getByTestId('git-save-first').getByRole('button').click();
  await expect(page.getByTestId('git-save-first')).toBeHidden();
  await expect(page.getByTestId('toolbar-git-count')).toHaveText('1');
  await expect(page.getByTestId('git-changes')).toContainText('Bench Supply.ewproj');
  await expect(page.getByTestId('git-commit-push')).toBeDisabled();
  await page.getByTestId('git-message').fill('Revision B');
  await page.getByTestId('git-commit-push').click();
  await expect(page.getByTestId('git-no-changes')).toBeVisible();
  await expect(page.getByTestId('git-sync')).toHaveText('Up to date with GitHub');
  await expect(page.getByTestId('toolbar-git-count')).toBeHidden();
  expect(github.git(repo, ['log', '--format=%s', 'main']).trim().split('\n')).toEqual([
    'Revision B',
    'Create Bench Supply',
  ]);

  // Someone else changes the project on GitHub.
  const elsewhere = mkdtempSync(join(tmpdir(), 'ew-other-'));
  try {
    const authed = `${github.url.replace('http://', `http://x-access-token:${github.token}@`)}/${repo}.git`;
    await git(elsewhere, 'clone', authed, 'copy');
    const copy = join(elsewhere, 'copy');
    const manifestFile = join(copy, 'Bench Supply.ewproj');
    const manifest = JSON.parse(readFileSync(manifestFile, 'utf8')) as Record<string, unknown>;
    manifest['revision'] = 'C';
    writeFileSync(manifestFile, `${JSON.stringify(manifest, null, 2)}\n`);
    await git(
      copy,
      '-c',
      'user.name=Other',
      '-c',
      'user.email=other@example.com',
      'commit',
      '-am',
      'Revision C',
    );
    await git(copy, 'push', 'origin', 'main');
  } finally {
    removeTemp(elsewhere);
  }

  await page.getByTestId('git-fetch').click();
  await expect(page.getByTestId('git-sync')).toHaveText('Commits to pull: 1');
  await page.getByTestId('git-pull').click();
  await expect(page.getByText('Took the changes from GitHub')).toBeVisible();
  await expect(page.getByTestId('git-sync')).toHaveText('Up to date with GitHub');
  await expect(page.getByTestId('git-log')).toContainText('Revision C');
  // The project was read again.
  await page.getByTestId('tree-project').click();
  await expect(page.getByTestId('project-revision')).toHaveValue('C');

  // A new branch, pushed to GitHub.
  await page.getByTestId('toolbar-git').click();
  await page.getByTestId('git-branch').selectOption('::new');
  await page.getByTestId('git-new-branch-name').fill('bad name');
  await expect(page.getByTestId('git-new-branch')).toContainText('Not a valid branch name.');
  await page.getByTestId('git-new-branch-name').fill('rev-c');
  await page.getByTestId('git-new-branch-create').click();
  await expect(page.getByText('Created branch rev-c')).toBeVisible();
  await expect(page.getByTestId('git-branch')).toHaveValue('rev-c');
  await expect(page.getByTestId('git-sync')).toHaveText(
    'This branch is not on GitHub yet: push it.',
  );
  await page.getByTestId('git-push').click();
  await expect(page.getByTestId('git-sync')).toHaveText('Up to date with GitHub');
  expect(github.git(repo, ['branch', '--list', 'rev-c']).trim()).toBe('rev-c');

  // Back to main.
  await page.getByTestId('git-branch').selectOption('main');
  await expect(page.getByText('Switched to branch main')).toBeVisible();
  await expect(page.getByTestId('git-branch')).toHaveValue('main');
  expect(projectsDir).not.toBe('');
});

test('opens a project from GitHub and puts a local project on GitHub', async () => {
  // A project on GitHub, made by the app itself in a first run.
  const projects = mkdtempSync(join(tmpdir(), 'ew-projects-'));
  try {
    const first = await launch(projects);
    await openGithubSettings(first.page);
    await first.page.getByTestId('github-sign-in-open').click();
    await signInWithToken(first.page);
    await first.page.keyboard.press('Escape');

    // A local project, put on GitHub from the source control view.
    await first.page.getByTestId('new-project').click();
    await first.page.getByTestId('wizard-name').fill('Clock');
    await first.page.getByTestId('wizard-next').click();
    await first.page.getByTestId('template-empty').click();
    await first.page.getByTestId('wizard-create').click();
    await first.page.getByTestId('toolbar-git').click();
    await expect(first.page.getByTestId('source-control')).toContainText(
      'This project is not under version control yet.',
    );
    // An existing empty repository.
    github.addRepo({ owner: 'octo-maker', name: 'clock-board', private: false, description: '' });
    await first.page.getByTestId('repo-target-existing').click();
    await first.page.getByTestId('repo-target-select').selectOption('octo-maker/clock-board');
    await first.page.getByTestId('git-publish').click();
    await expect(
      first.page.getByText('The project is on GitHub: octo-maker/clock-board'),
    ).toBeVisible();
    await expect(first.page.getByTestId('git-remote')).toHaveText('octo-maker/clock-board');
    await first.close();
    launched = undefined;

    // A second computer: open it from GitHub.
    const second = await launch();
    await second.page.getByTestId('open-github').click();
    const dialog = second.page.getByTestId('github-clone');
    await expect(dialog).toContainText('Sign in to GitHub first.');
    await second.page.getByTestId('github-clone-sign-in').click();
    await signInWithToken(second.page);
    await expect(second.page.locator('[data-repo="octo-maker/clock-board"]')).toBeVisible();
    await second.page.getByTestId('github-clone-filter').fill('nothing like this');
    await expect(dialog).toContainText('No repositories found.');
    await second.page.getByTestId('github-clone-filter').fill('clock');
    await second.page.locator('[data-repo="octo-maker/clock-board"]').click();
    await expect(second.page.getByTestId('github-clone-folder')).toContainText(
      join(second.projectsDir, 'clock-board'),
    );
    await second.page.getByTestId('github-clone-open').click();
    await expect(second.page.getByTestId('workspace')).toBeVisible();
    await expect(second.page.getByTestId('project-title')).toHaveText('Clock');
    await second.page.getByTestId('toolbar-git').click();
    await expect(second.page.getByTestId('git-sync')).toHaveText('Up to date with GitHub');

    // Cloning again into the same folder fails cleanly.
    await second.page.getByTestId('toolbar-close').click();
    await second.page.getByTestId('open-github').click();
    await second.page.locator('[data-repo="octo-maker/clock-board"]').click();
    await second.page.getByTestId('github-clone-open').click();
    await expect(second.page.getByTestId('github-clone-error')).toHaveText(
      'The folder or repository is not empty.',
    );
  } finally {
    removeTemp(projects);
  }
});
