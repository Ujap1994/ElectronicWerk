/**
 * Generates the screenshots used in the documentation (docs/public/screenshots).
 * Run with `npm run docs:screenshots` after building the app. Only synthetic
 * demo data is shown, so the images are safe to publish.
 */
import { cpSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

import { expect, test, type Locator, type Page } from '@playwright/test';

import {
  APP_VERSION,
  fakeRelease,
  killHard,
  launchApp,
  queueDialogAnswer,
  type LaunchedApp,
  type LaunchOptions,
} from '../fixtures';
import {
  openDocument,
  settle,
  toScreen,
  viewerCanvas,
  zoomAt,
  type Point,
} from '../viewer-helpers';

/** A tidy, anonymous project folder for the screenshots. */
const PROJECTS = '/tmp/ElectronicWerk/Projects';

const OUT = resolve(import.meta.dirname, '../../../../docs/public/screenshots');
mkdirSync(OUT, { recursive: true });

const DEMO_NOTES = [
  '## Added',
  '',
  '- **KiCad import:** open KiCad projects as ElectronicWerk projects',
  '- Import report with everything that could not be mapped',
  '- Netlist import (KiCad and CSV)',
  '',
  '## Fixed',
  '',
  '- The layer panel remembers collapsed groups',
].join('\n');

/** A fictitious next release for the update notice. */
const NEXT_VERSION = (() => {
  const [major = 0, minor = 0] = APP_VERSION.split('.').map(Number);
  return `${String(major)}.${String(minor + 1)}.0`;
})();

async function start(options: LaunchOptions = {}): Promise<LaunchedApp> {
  const launched = await launchApp({ projectsDir: PROJECTS, ...options });
  await launched.page.setViewportSize({ width: 1280, height: 800 });
  await expect(launched.page.getByTestId('engine-status')).toHaveAttribute('data-state', 'ready');
  return launched;
}

async function shoot(target: Page | Locator, name: string): Promise<void> {
  await target.screenshot({ path: join(OUT, `${name}.png`), scale: 'css', animations: 'disabled' });
}

test.describe.configure({ mode: 'serial' });

test('start screen', async () => {
  for (const theme of ['light', 'dark'] as const) {
    const { page, close } = await start({ settings: { theme } });
    await shoot(page, `start-screen-${theme}`);
    await shoot(page.locator('.status-bar'), `status-bar-${theme}`);
    await close();
  }
  const german = await start({ settings: { theme: 'light', language: 'de' } });
  await shoot(german.page, 'start-screen-german');
  await german.close();
});

test('settings dialog', async () => {
  const { page, close } = await start({ settings: { theme: 'light' } });
  await page.getByTestId('open-settings').click();
  await shoot(page, 'settings-window');
  await shoot(page.getByTestId('settings-dialog'), 'settings-dialog');
  await page.getByTestId('setting-language').focus();
  await close();

  const skipped = await start({
    settings: { theme: 'light', updates: { skippedVersion: '0.2.0' } },
  });
  await skipped.page.getByTestId('open-settings').click();
  await shoot(skipped.page.getByTestId('settings-dialog'), 'settings-dialog-skipped');
  await skipped.close();
});

test('command palette', async () => {
  const { page, close } = await start({ settings: { theme: 'light' } });
  await page.getByTestId('open-palette').click();
  await shoot(page, 'command-palette-all');
  await page.getByTestId('command-input').fill('appearance');
  await shoot(page.getByTestId('command-palette'), 'command-palette-search');
  await page.getByTestId('command-input').fill('xyz');
  await shoot(page.getByTestId('command-palette'), 'command-palette-empty');
  await close();
});

test('update notice', async () => {
  for (const theme of ['light', 'dark'] as const) {
    const { page, close } = await start({
      settings: { theme },
      updateFeed: fakeRelease(NEXT_VERSION, DEMO_NOTES),
    });
    await expect(page.getByTestId('update-dialog')).toBeVisible();
    await shoot(page, `update-available-window-${theme}`);
    await shoot(page.getByTestId('update-dialog'), `update-available-${theme}`);
    await close();
  }

  const upToDate = await start({
    settings: { theme: 'light' },
    updateFeed: fakeRelease(APP_VERSION),
  });
  await upToDate.page.getByTestId('open-settings').click();
  await upToDate.page.getByTestId('check-updates-now').click();
  await expect(upToDate.page.getByTestId('update-dialog')).toContainText('latest version');
  await shoot(upToDate.page.getByTestId('update-dialog'), 'update-up-to-date');
  await upToDate.close();

  const failed = await start({ settings: { theme: 'light' }, updateFeed: '{ offline' });
  await failed.page.getByTestId('open-settings').click();
  await failed.page.getByTestId('check-updates-now').click();
  await expect(failed.page.getByTestId('update-dialog')).toContainText('failed');
  await shoot(failed.page.getByTestId('update-dialog'), 'update-error');
  await failed.close();
});

test('engine restart notice', async () => {
  const { app, page, close } = await start({ settings: { theme: 'light' } });
  await app.evaluate(() => {
    const hooks = (globalThis as { __ewTest?: { engine: { crashForTest: () => void } } }).__ewTest;
    hooks?.engine.crashForTest();
  });
  await expect(page.getByTestId('engine-restarted')).toBeVisible();
  await expect(page.getByTestId('engine-status')).toHaveAttribute('data-state', 'ready');
  await shoot(page, 'engine-restarted');
  await close();
});

// --- Projects (M1) ------------------------------------------------------------

async function createDemo(page: Page, name: string, template = 'demo'): Promise<void> {
  await page.getByTestId('new-project').click();
  await page.getByTestId('wizard-name').fill(name);
  await page.getByTestId('wizard-next').click();
  await page.getByTestId(`template-${template}`).click();
  await page.getByTestId('wizard-create').click();
  await page.getByTestId('workspace').waitFor();
}

function freshProjects(): void {
  rmSync(PROJECTS, { recursive: true, force: true });
  mkdirSync(PROJECTS, { recursive: true });
}

test('new project wizard', async () => {
  freshProjects();
  const { page, close } = await start({ settings: { theme: 'light' } });
  await page.getByTestId('new-project').click();
  await page.getByTestId('wizard-name').fill('LED Blinker');
  await shoot(page.getByTestId('wizard'), 'wizard-project');
  await page.getByTestId('wizard-next').click();
  await page.getByTestId('template-board-2l').click();
  await shoot(page.getByTestId('wizard'), 'wizard-template');
  await page.getByTestId('wizard-next').click();
  await page.getByTestId('board-width').fill('80');
  await page.getByTestId('board-height').fill('50');
  await shoot(page.getByTestId('wizard'), 'wizard-board');
  await page.getByTestId('board-height').fill('2');
  await shoot(page.getByTestId('wizard'), 'wizard-board-error');
  await close();
});

test('project workspace', async () => {
  freshProjects();
  for (const theme of ['light', 'dark'] as const) {
    const { page, close } = await start({ settings: { theme } });
    await createDemo(page, theme === 'light' ? 'Demo Board' : 'Demo Board Dark');
    await shoot(page, `workspace-${theme}`);
    await close();
  }
  const german = await start({ settings: { theme: 'light', language: 'de' } });
  await createDemo(german.page, 'Demo Platine');
  await shoot(german.page, 'workspace-german');
  await german.close();
});

test('project overview, variables and history', async () => {
  freshProjects();
  const { page, close } = await start({ settings: { theme: 'light' } });
  await createDemo(page, 'Sensor Node', 'board-4l');
  const revision = page.getByTestId('project-revision');
  await revision.fill('B');
  await revision.press('Enter');
  await page.getByTestId('variable-new-name').fill('AUTHOR');
  await page.getByTestId('variable-new-value').fill('Hardware Lab');
  await page.getByTestId('variable-add').click();
  await expect(page.getByTestId('variable-AUTHOR')).toBeVisible();
  await page.getByTestId('variable-new-name').fill('PCB_HOUSE');
  await page.getByTestId('variable-new-value').fill('Any Fab Ltd.');
  await page.getByTestId('variable-add').click();
  await expect(page.getByTestId('variable-PCB_HOUSE')).toBeVisible();
  await shoot(page.getByTestId('details'), 'project-overview');
  await shoot(page.getByTestId('variables'), 'project-variables');
  await shoot(page.locator('.toolbar'), 'toolbar-dirty');
  await page.getByTestId('toolbar-undo').hover();
  await shoot(page, 'toolbar-undo-tooltip');
  await page.getByTestId('toolbar-save').click();
  await expect(page.getByTestId('dirty-indicator')).toHaveText('All changes saved');
  await shoot(page.locator('.toolbar'), 'toolbar-saved');
  await shoot(page.locator('.status-bar'), 'status-bar-project');
  await close();
});

test('project tree and documents', async () => {
  freshProjects();
  const { page, close } = await start({ settings: { theme: 'light' } });
  await createDemo(page, 'Audio Amp', 'board-2l');
  const root = page.getByTestId(/^tree-doc:/u).filter({ hasText: 'Root' });
  await root.click();
  await page
    .getByTestId(/^tree-actions-doc:/u)
    .first()
    .click();
  await shoot(page, 'tree-menu');
  await page.getByTestId('menu-add-sub-sheet').click();
  await page.getByTestId('tree-rename').fill('Power Supply');
  await shoot(page.locator('.workspace__sidebar'), 'tree-rename');
  await page.getByTestId('tree-rename').press('Enter');
  await page
    .getByTestId(/^tree-doc:/u)
    .filter({ hasText: 'Root' })
    .click();
  await page
    .getByTestId(/^tree-actions-doc:/u)
    .first()
    .click();
  await page.getByTestId('menu-add-sub-sheet').click();
  await page.getByTestId('tree-rename').fill('Amplifier');
  await page.getByTestId('tree-rename').press('Enter');
  await shoot(page.locator('.workspace__sidebar'), 'tree-hierarchy');

  await page
    .getByTestId(/^tree-doc:/u)
    .filter({ hasText: 'Power Supply' })
    .click();
  await shoot(page.getByTestId('details'), 'details-schematic');
  await page
    .getByTestId(/^tree-doc:/u)
    .filter({ hasText: 'Audio Amp' })
    .click();
  await expect(page.getByTestId('stackup')).toBeVisible();
  await shoot(page.getByTestId('details'), 'details-pcb');
  await page
    .getByTestId(/^tree-doc:/u)
    .filter({ hasText: 'Design rules' })
    .click();
  await expect(page.getByTestId('net-classes')).toBeVisible();
  await shoot(page.getByTestId('details'), 'details-rules');
  await page
    .getByTestId(/^tree-doc:/u)
    .filter({ hasText: 'Fabrication' })
    .click();
  await expect(page.getByTestId('outputs')).toBeVisible();
  await shoot(page.getByTestId('details'), 'details-output-job');

  await page
    .getByTestId(/^tree-doc:/u)
    .filter({ hasText: 'Amplifier' })
    .click();
  await page.keyboard.press('Delete');
  await shoot(page.getByTestId('remove-dialog'), 'remove-dialog');
  await page.getByTestId('confirm-yes').click();

  await page.getByTestId('toolbar-validate').click();
  await expect(page.getByTestId('validation-summary')).toBeVisible();
  await shoot(page.getByTestId('validation-dialog'), 'validation-ok');
  await page.keyboard.press('Escape');

  await page.getByTestId('toolbar-close').click();
  await expect(page.getByTestId('close-dialog')).toBeVisible();
  await shoot(page.getByTestId('close-dialog'), 'close-dialog');
  await page.getByTestId('close-save').click();
  await expect(page.getByTestId('recent-list')).toBeVisible();
  await close();
});

test('validation with problems', async () => {
  freshProjects();
  const { page, close } = await start({ settings: { theme: 'light' } });
  await createDemo(page, 'Broken Example', 'empty');
  await page.getByTestId(/^tree-actions-group:schematics$/u).click();
  await page.getByTestId('menu-add-schematics').click();
  await page.getByTestId('tree-rename').press('Enter');
  await page.getByTestId('toolbar-save').click();
  await expect(page.getByTestId('dirty-indicator')).toHaveText('All changes saved');
  await page.getByTestId('toolbar-close').click();
  await expect(page.getByTestId('recent-list')).toBeVisible();

  // Problems a hand edit could introduce: variables the UI would refuse.
  const file = join(PROJECTS, 'Broken Example', 'Broken Example.ewproj');
  const manifest = JSON.parse(readFileSync(file, 'utf8')) as Record<string, unknown>;
  manifest['variables'] = { REVISION: 'x', 'bad name': 'y' };
  writeFileSync(file, JSON.stringify(manifest));

  await page.getByTestId('recent-open').first().click();
  await page.getByTestId('toolbar-validate').click();
  await expect(page.getByTestId('validation-summary')).toContainText('warnings');
  await shoot(page.getByTestId('validation-dialog'), 'validation-issues');
  await close();
});

test('recent projects and crash recovery', async () => {
  freshProjects();
  const userDataDir = '/tmp/ElectronicWerk/profile';
  rmSync(userDataDir, { recursive: true, force: true });
  mkdirSync(userDataDir, { recursive: true });
  writeFileSync(
    join(userDataDir, 'settings.json'),
    JSON.stringify({ language: 'en', theme: 'light' }),
  );

  const first = await start({ userDataDir });
  await createDemo(first.page, 'Weather Station', 'board-2l');
  await first.page.getByTestId('toolbar-close').click();
  await createDemo(first.page, 'Robot Controller', 'board-4l');
  const revision = first.page.getByTestId('project-revision');
  await revision.fill('C');
  await revision.press('Enter');
  await first.page.getByTestId('variable-new-name').fill('MOTOR');
  await first.page.getByTestId('variable-new-value').fill('NEMA 17');
  await first.page.getByTestId('variable-add').click();
  await expect(first.page.getByTestId('variable-MOTOR')).toBeVisible();
  await killHard(first.app);

  const second = await start({ userDataDir });
  await expect(second.page.getByTestId('recent-recovery')).toBeVisible();
  await shoot(second.page, 'start-screen-recent');
  await second.page.getByTestId('recent-open').first().click();
  await expect(second.page.getByTestId('recovery-dialog')).toBeVisible();
  await shoot(second.page.getByTestId('recovery-dialog'), 'recovery-dialog');
  await second.page.getByTestId('recovery-restore').click();
  await expect(second.page.getByTestId('notice').last()).toBeVisible();
  await shoot(second.page, 'recovery-restored');

  await second.app.evaluate(() => {
    const hooks = (globalThis as { __ewTest?: { engine: { crashForTest: () => void } } }).__ewTest;
    hooks?.engine.crashForTest();
  });
  await expect(second.page.getByTestId('notice').last()).toContainText('engine restarted');
  await shoot(second.page.getByTestId('notices'), 'engine-restart-restored');

  const archive = join(PROJECTS, 'Robot Controller.zip');
  await queueDialogAnswer(second.app, archive);
  await second.page.getByTestId('toolbar-archive').click();
  await expect(second.page.getByTestId('notice').last()).toContainText('Archive written');
  await shoot(second.page.getByTestId('notices'), 'archive-notice');
  await second.close();
  rmSync(userDataDir, { recursive: true, force: true });
});

// --- 2D viewer ----------------------------------------------------------------

interface DemoBoard {
  footprints: {
    designator: string;
    at: Point;
    rotation?: number;
    side?: string;
    pads: { at: Point }[];
  }[];
}

function readBoard(project: string): DemoBoard {
  const dir = join(PROJECTS, project, 'pcb');
  const file = readdirSync(dir).find((name) => name.endsWith('.ewpcb')) ?? '';
  return JSON.parse(readFileSync(join(dir, file), 'utf8')) as DemoBoard;
}

test('board viewer', async () => {
  freshProjects();
  const { page, close } = await start({ settings: { theme: 'light' } });
  await page.setViewportSize({ width: 1400, height: 860 });
  await createDemo(page, 'Demo Board');
  const canvas = await openDocument(page, 'Demo Board');
  await shoot(page, 'viewer-board');

  // A footprint near the centre, zoomed in, with a pad selected.
  const board = readBoard('Demo Board');
  const fp =
    board.footprints.find((entry) => entry.designator === 'U8') ??
    board.footprints.find((entry) => (entry.side ?? 'top') === 'top' && !entry.rotation);
  const pad = fp?.pads[0];
  // Pad position: mirrored on the bottom side, rotated with the footprint.
  const local = pad?.at ?? [0, 0];
  const lx = fp?.side === 'bottom' ? -local[0] : local[0];
  const angle = ((fp?.rotation ?? 0) * Math.PI) / 180;
  const at: Point = [
    (fp?.at[0] ?? 0) + lx * Math.cos(angle) - local[1] * Math.sin(angle),
    (fp?.at[1] ?? 0) + lx * Math.sin(angle) + local[1] * Math.cos(angle),
  ];
  await zoomAt(page, [fp?.at[0] ?? 0, fp?.at[1] ?? 0], 7);
  const [px, py] = await toScreen(canvas, at);
  await page.mouse.click(px, py);
  await page.mouse.move(px + 160, py + 40);
  await expect(page.getByTestId('inspector-title')).toContainText(fp?.designator ?? '');
  await settle(page);
  await shoot(page, 'viewer-pad-selected');
  await shoot(page.getByTestId('inspector'), 'viewer-inspector');

  // Box selection while dragging (right to left: crossing).
  await page.keyboard.press('Home');
  await settle(page);
  const box = await canvas.boundingBox();
  if (box === null) {
    throw new Error('no canvas');
  }
  await page.mouse.move(box.x + box.width * 0.62, box.y + box.height * 0.3);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width * 0.4, box.y + box.height * 0.55, { steps: 6 });
  await shoot(page.locator('.viewer__main'), 'viewer-box-selection');
  await page.mouse.up();
  await expect(page.getByTestId('inspector-title')).toContainText('objects selected');
  await shoot(page, 'viewer-multiple-selected');

  // Lasso.
  await page.keyboard.press('Escape');
  await page.getByTestId('viewer-tool-lasso').click();
  const lasso: Point[] = [
    [0.3, 0.35],
    [0.45, 0.25],
    [0.6, 0.35],
    [0.58, 0.6],
    [0.4, 0.68],
    [0.28, 0.55],
    [0.31, 0.4],
  ];
  const [first] = lasso;
  await page.mouse.move(
    box.x + box.width * (first?.[0] ?? 0),
    box.y + box.height * (first?.[1] ?? 0),
  );
  await page.mouse.down();
  for (const [fx, fy] of lasso.slice(1)) {
    await page.mouse.move(box.x + box.width * fx, box.y + box.height * fy, { steps: 5 });
  }
  await shoot(page.locator('.viewer__main'), 'viewer-lasso');
  await page.mouse.up();
  await page.getByTestId('viewer-tool-box').click();
  await canvas.focus();
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('inspector')).toContainText('Click an object');

  // Layers: panel, active bottom copper with high contrast, bottom view.
  await shoot(page.locator('.viewer-panel').first(), 'viewer-layers');
  await page.getByTestId('layer-activate-cu.bottom').click();
  await page.getByTestId('viewer-high-contrast').click();
  await page.mouse.move(box.x + box.width - 4, box.y + 4);
  await settle(page);
  await shoot(page, 'viewer-high-contrast');
  await page.getByTestId('viewer-high-contrast').click();
  await page.getByTestId('layer-activate-cu.top').click();
  await page.getByTestId('viewer-flip').click();
  await settle(page);
  await shoot(page, 'viewer-bottom-view');
  await close();

  const dark = await start({ settings: { theme: 'dark' } });
  await dark.page.setViewportSize({ width: 1400, height: 860 });
  await queueDialogAnswer(dark.app, join(PROJECTS, 'Demo Board', 'Demo Board.ewproj'));
  await dark.page.getByTestId('open-project').click();
  await dark.page.getByTestId('workspace').waitFor();
  await openDocument(dark.page, 'Demo Board');
  await shoot(dark.page, 'viewer-board-dark');
  await dark.close();
});

test('sheet viewer', async () => {
  freshProjects();
  for (const theme of ['light', 'dark'] as const) {
    const { page, close } = await start({ settings: { theme } });
    await page.setViewportSize({ width: 1400, height: 860 });
    await createDemo(page, theme === 'light' ? 'Demo Board' : 'Demo Board Dark');
    await openDocument(page, 'Root');
    await shoot(page, `viewer-sheet-${theme}`);
    if (theme === 'light') {
      const sheetDir = join(PROJECTS, 'Demo Board', 'schematics');
      const file = readdirSync(sheetDir).find((name) => name.endsWith('.ewsch')) ?? '';
      const sheet = JSON.parse(readFileSync(join(sheetDir, file), 'utf8')) as {
        symbols: { designator: string; at: Point }[];
      };
      const symbol = sheet.symbols.find((entry) => entry.designator === 'U2') ?? sheet.symbols[0];
      await zoomAt(page, symbol?.at ?? [0, 0], 6);
      const [sx, sy] = await toScreen(viewerCanvas(page), symbol?.at ?? [0, 0]);
      await page.mouse.click(sx, sy);
      await page.mouse.move(sx + 220, sy - 120);
      await expect(page.getByTestId('inspector-title')).toContainText('Symbol');
      await settle(page);
      await shoot(page, 'viewer-sheet-symbol');
      await shoot(page.locator('.viewer-panel').first(), 'viewer-sheet-display');
    }
    await close();
  }
});

// --- KiCad import -------------------------------------------------------------

const KICAD_FIXTURE = resolve(
  import.meta.dirname,
  '../../../../crates/ew-kicad/tests/fixtures/blinky',
);

test('kicad import', async () => {
  freshProjects();
  // A copy next to the projects, so the dialog shows a neutral path.
  const kicad = '/tmp/ElectronicWerk/KiCad/blinky';
  rmSync(kicad, { recursive: true, force: true });
  cpSync(KICAD_FIXTURE, kicad, { recursive: true });
  const { app, page, close } = await start({ settings: { theme: 'light' } });
  await page.setViewportSize({ width: 1400, height: 860 });
  await shoot(page.locator('.start__actions'), 'import-start-action');
  await queueDialogAnswer(app, join(kicad, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-name').fill('Blinky');
  await expect(page.getByTestId('kicad-import-target')).toContainText('Blinky');
  await shoot(page.getByTestId('kicad-import-dialog'), 'import-dialog');
  await page.getByTestId('kicad-import-start').click();
  await expect(page.getByTestId('import-report')).toBeVisible();
  await shoot(page.getByTestId('import-report'), 'import-report');
  await page
    .getByTestId('import-report')
    .getByRole('button', { name: 'Close', exact: true })
    .last()
    .click();
  await shoot(page, 'import-workspace');

  // The imported board and sheet in the viewer.
  const boardRow = page.locator('[data-testid^="tree-doc:"]').filter({ hasText: /^Blinky$/u });
  const sheetRow = page.locator('[data-testid^="tree-doc:"]').filter({ hasText: 'Top sheet' });
  await boardRow.dblclick();
  await settle(page);
  await shoot(page, 'import-board');
  await page.getByTestId('tab-details').click();
  await sheetRow.dblclick();
  await expect(page.getByTestId('viewer')).toHaveAttribute('data-kind', 'schematic');
  await settle(page);
  // Zoom in on the circuit (Ctrl + wheel zooms around the pointer).
  await page.mouse.move(682, 407);
  await page.keyboard.down('Control');
  await page.mouse.wheel(0, -46);
  await page.keyboard.up('Control');
  await page.mouse.move(150, 700);
  await settle(page);
  await shoot(page, 'import-sheet');

  // Netlists: the context menu, a matching and a changed netlist.
  await page.getByTestId('tab-details').click();
  await boardRow.click({ button: 'right' });
  await page.screenshot({
    path: join(OUT, 'netlist-menu.png'),
    clip: { x: 0, y: 40, width: 480, height: 480 },
  });
  await queueDialogAnswer(app, join(kicad, 'blinky.net'));
  await page.getByRole('menuitem', { name: 'Import netlist…' }).click();
  await expect(page.getByTestId('netlist-summary')).toBeVisible();
  await shoot(page.getByTestId('netlist-dialog'), 'netlist-match');
  await page
    .getByTestId('netlist-dialog')
    .getByRole('button', { name: 'Close', exact: true })
    .last()
    .click();
  const changed = join(PROJECTS, 'blinky-changed.csv');
  writeFileSync(
    changed,
    readFileSync(join(kicad, 'blinky-netlist.csv'), 'utf8')
      .replace('/LED_A;D1;2', '/LED_B;D1;2')
      .replace('/DIS;R2;1', '/TRIG;R2;1'),
  );
  await queueDialogAnswer(app, changed);
  await boardRow.click({ button: 'right' });
  await page.getByRole('menuitem', { name: 'Import netlist…' }).click();
  await expect(page.getByTestId('netlist-differences')).toBeVisible();
  await shoot(page.getByTestId('netlist-dialog'), 'netlist-differences');
  await page
    .getByTestId('netlist-dialog')
    .getByRole('button', { name: 'Close', exact: true })
    .last()
    .click();
  await page.locator('[data-testid="tree-group:libraries"]').click({ button: 'right' });
  await page.screenshot({
    path: join(OUT, 'library-import-menu.png'),
    clip: { x: 0, y: 40, width: 480, height: 480 },
  });
  await page.keyboard.press('Escape');
  await close();
});
