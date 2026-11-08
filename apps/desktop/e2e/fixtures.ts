import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

import { _electron as electron, type ElectronApplication, type Page } from '@playwright/test';

export const APP_DIR = resolve(import.meta.dirname, '..');

/** The version of the app under test (from its package.json). */
export const APP_VERSION = (
  JSON.parse(readFileSync(join(APP_DIR, 'package.json'), 'utf8')) as { version: string }
).version;

export interface LaunchedApp {
  readonly app: ElectronApplication;
  readonly page: Page;
  readonly userDataDir: string;
  /** Default folder for new projects. */
  readonly projectsDir: string;
  close: () => Promise<void>;
}

export interface LaunchOptions {
  /** Reuse an existing profile directory instead of a fresh one. */
  readonly userDataDir?: string;
  /** Settings written to a fresh profile before start (default: English UI). */
  readonly settings?: Record<string, unknown>;
  /**
   * Release JSON the update check sees instead of the GitHub API (`null`
   * means "no release published"). A string is written to the feed file
   * as-is, which allows testing broken responses.
   */
  readonly updateFeed?: unknown;
  /** Default folder for new projects (a fresh temporary folder if omitted). */
  readonly projectsDir?: string;
}

/** A GitHub API release object for tests. */
export function fakeRelease(version: string, notes = ''): Record<string, unknown> {
  return {
    tag_name: `v${version}`,
    name: `ElectronicWerk ${version}`,
    body: notes,
    html_url: `https://github.com/Ujap1994/ElectronicWerk/releases/tag/v${version}`,
    published_at: '2026-12-01T10:00:00Z',
    draft: false,
    prerelease: false,
  };
}

/** Starts the built app with an isolated profile and the test hooks enabled. */
export async function launchApp(options: LaunchOptions = {}): Promise<LaunchedApp> {
  const userDataDir = options.userDataDir ?? mkdtempSync(join(tmpdir(), 'ew-e2e-'));
  if (options.userDataDir === undefined) {
    // Deterministic language, independent of the machine running the tests.
    writeFileSync(
      join(userDataDir, 'settings.json'),
      JSON.stringify({ language: 'en', ...options.settings }),
    );
  }
  // Tests never contact GitHub: the update check reads this file instead.
  const feedFile = join(userDataDir, 'update-feed.json');
  const feed = options.updateFeed ?? null;
  writeFileSync(feedFile, typeof feed === 'string' ? feed : JSON.stringify(feed));

  const projectsDir = options.projectsDir ?? mkdtempSync(join(tmpdir(), 'ew-projects-'));
  const app = await electron.launch({
    // Start the app folder (not the script) so the version comes from package.json.
    args: [APP_DIR],
    cwd: APP_DIR,
    // Let the app's own theme setting drive `prefers-color-scheme`.
    colorScheme: null,
    // The OS sandbox needs extra privileges on Linux CI runners.
    chromiumSandbox: process.platform !== 'linux',
    env: {
      ...process.env,
      EW_USER_DATA_DIR: userDataDir,
      EW_TEST_HOOKS: '1',
      EW_UPDATE_FEED: feedFile,
      EW_UPDATE_DELAY_MS: '0',
      EW_PROJECTS_DIR: projectsDir,
      ELECTRON_RENDERER_URL: '',
    },
  });
  const page = await app.firstWindow();
  await page.waitForLoadState('domcontentloaded');
  return {
    app,
    page,
    userDataDir,
    projectsDir,
    close: async () => {
      // Projects may be open with unsaved changes: skip the prompt.
      await app
        .evaluate(({ BrowserWindow }) => {
          for (const window of BrowserWindow.getAllWindows()) {
            window.destroy();
          }
        })
        .catch(() => undefined);
      await app.close().catch(() => undefined);
      if (options.userDataDir === undefined) {
        rmSync(userDataDir, { recursive: true, force: true });
      }
      if (options.projectsDir === undefined) {
        rmSync(projectsDir, { recursive: true, force: true });
      }
    },
  };
}

/** Makes the next native file dialog return `answer` (`null` = cancelled). */
export async function queueDialogAnswer(
  app: ElectronApplication,
  answer: string | null,
): Promise<void> {
  await app.evaluate((_electron, value) => {
    const hooks = (globalThis as { __ewTest?: { queueDialogAnswer: (v: string | null) => void } })
      .__ewTest;
    hooks?.queueDialogAnswer(value);
  }, answer);
}

/** Creates a project through the wizard and waits for the workspace. */
export async function createProjectWithWizard(
  page: Page,
  name: string,
  template = 'board-2l',
): Promise<void> {
  await page.getByTestId('new-project').click();
  await page.getByTestId('wizard-name').fill(name);
  await page.getByTestId('wizard-next').click();
  await page.getByTestId(`template-${template}`).click();
  await page.getByTestId('wizard-create').click();
  await page.getByTestId('workspace').waitFor();
}
