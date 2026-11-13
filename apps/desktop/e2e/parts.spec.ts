/**
 * Online part libraries in the running app, against a local website that
 * stands in for SnapEDA and Ultra Librarian (tests never contact the real
 * providers): searching opens the provider window, a file downloaded there
 * is offered for import, the import brings symbol, footprint and 3D model
 * into the project; a download from the user's own browser is imported
 * from the file; the settings show the provider accounts.
 */
import { createServer, type Server } from 'node:http';
import { existsSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { expect, test } from '@playwright/test';

import {
  APP_DIR,
  createProjectWithWizard,
  launchApp,
  queueDialogAnswer,
  type LaunchedApp,
} from './fixtures';

const PARTS = resolve(APP_DIR, '../../crates/ew-core/tests/fixtures/parts');

let launched: LaunchedApp | undefined;
let server: Server | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
  await new Promise<void>((done) => {
    if (server === undefined) {
      done();
      return;
    }
    server.close(() => {
      done();
    });
  });
  server = undefined;
});

/** A tiny provider website: a search page with a download link. */
async function startProvider(): Promise<string> {
  server = createServer((request, response) => {
    const url = new URL(request.url ?? '/', 'http://localhost');
    if (url.pathname.endsWith('/search')) {
      const query = url.searchParams.get('q') ?? '';
      response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
      response.end(
        `<!doctype html><title>Search</title><h1 id="query">${query.replace(/[<>&]/gu, '')}</h1>` +
          '<a id="download" href="/files/NE555-DEMO.zip">Download KiCad</a>',
      );
      return;
    }
    if (url.pathname === '/files/NE555-DEMO.zip') {
      response.writeHead(200, {
        'content-type': 'application/zip',
        'content-disposition': 'attachment; filename="NE555-DEMO.zip"',
      });
      response.end(readFileSync(join(PARTS, 'NE555-DEMO.zip')));
      return;
    }
    response.writeHead(200, { 'content-type': 'text/html' });
    response.end('<!doctype html><title>Provider</title><p>Provider page</p>');
  });
  await new Promise<void>((done) => {
    server?.listen(0, '127.0.0.1', () => {
      done();
    });
  });
  const address = server.address();
  const port = typeof address === 'object' && address !== null ? address.port : 0;
  return `http://127.0.0.1:${String(port)}`;
}

test('imports a part downloaded in the provider window', async () => {
  const base = await startProvider();
  launched = await launchApp({ env: { EW_PROVIDER_BASE: base } });
  const { app, page, projectsDir } = launched;
  await createProjectWithWizard(page, 'Online Parts', 'empty');

  await page.keyboard.press('ControlOrMeta+Shift+P');
  await page.getByTestId('command-input').fill('parts online');
  await page.keyboard.press('Enter');
  const dialog = page.getByTestId('online-parts');
  await expect(dialog).toBeVisible();
  await expect(dialog.getByTestId('online-provider-snapeda')).toBeVisible();
  await expect(dialog.getByTestId('online-provider-ultralibrarian')).toBeVisible();
  await page.getByTestId('online-query').fill('NE555');

  // The provider window opens at the search page.
  const opened = app.waitForEvent('window');
  await page.getByTestId('online-search-snapeda').click();
  const provider = await opened;
  await expect(provider.locator('#query')).toHaveText('NE555');
  // It cannot reach the app's API.
  expect(await provider.evaluate(() => 'ew' in globalThis)).toBe(false);

  // The download is offered for import.
  await provider.locator('#download').click();
  const importer = page.getByTestId('download-import');
  await expect(importer).toBeVisible({ timeout: 30_000 });
  await expect(importer).toContainText('NE555-DEMO.zip');
  await expect(importer).toContainText('SnapEDA');
  await expect(importer.getByTestId('import-symbol')).toContainText('SnapEDA:NE555-DEMO');
  await expect(importer.getByTestId('import-footprint')).toContainText(
    'SnapEDA:SOIC127P600X175-8N',
  );
  await expect(importer.getByTestId('import-model')).toContainText('NE555-DEMO.step');
  await page.getByTestId('download-import-start').click();
  await expect(importer).toBeHidden();
  await expect(page.getByTestId('notice').last()).toContainText('Imported 1 symbols');
  expect(existsSync(join(projectsDir, 'Online Parts', '3d', 'SnapEDA', 'NE555-DEMO.step'))).toBe(
    true,
  );
  const windows = await app.evaluate(() =>
    (
      globalThis as unknown as { __ewTest: { providerWindows: () => string[] } }
    ).__ewTest.providerWindows(),
  );
  expect(windows).toEqual(['snapeda']);
});

test('imports a download from the own browser and shows the accounts', async () => {
  launched = await launchApp();
  const { app, page, projectsDir } = launched;
  await createProjectWithWizard(page, 'Browser Parts', 'empty');
  await page.keyboard.press('ControlOrMeta+Shift+P');
  await page.getByTestId('command-input').fill('parts online');
  await page.keyboard.press('Enter');
  await queueDialogAnswer(app, join(PARTS, 'ul_OPAMP-DEMO.zip'));
  await page.getByTestId('online-import-file').click();
  const importer = page.getByTestId('download-import');
  await expect(importer).toContainText('Ultra Librarian');
  await page.getByTestId('download-nickname').fill('Amplifiers');
  await expect(importer.getByTestId('import-symbol')).toContainText('Amplifiers:OPAMP-DEMO');
  await page.getByTestId('download-import-start').click();
  await expect(importer).toBeHidden();
  expect(existsSync(join(projectsDir, 'Browser Parts', '3d', 'Amplifiers', 'DBV0005A.step'))).toBe(
    true,
  );

  // The accounts in the settings.
  await page.getByTestId('open-settings').click();
  const accounts = page.getByTestId('settings-providers');
  await expect(accounts).toContainText('SnapEDA');
  await expect(accounts).toContainText('Ultra Librarian');
  await expect(page.getByTestId('provider-sign-out-snapeda')).toBeDisabled();
});
