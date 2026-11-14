/**
 * Generates the screenshots used in the documentation (docs/public/screenshots).
 * Run with `npm run docs:screenshots` after building the app. Only synthetic
 * demo data is shown, so the images are safe to publish.
 */
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs';
import { execFile, execFileSync } from 'node:child_process';
import { homedir } from 'node:os';
import { join, resolve } from 'node:path';

import { expect, test, type Locator, type Page } from '@playwright/test';

import {
  APP_VERSION,
  createProjectWithWizard,
  fakeRelease,
  killHard,
  launchApp,
  queueDialogAnswer,
  type LaunchedApp,
  type LaunchOptions,
} from '../fixtures';
import { startFakeGithub, type FakeGithub } from '../../test/github-server';
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
  // The 3D views show the models of a local KiCad installation, if any.
  const launched = await launchApp({ projectsDir: PROJECTS, kicadModels: true, ...options });
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

// --- Library manager ----------------------------------------------------------

const LIBRARY_FIXTURES = resolve(
  import.meta.dirname,
  '../../../../crates/ew-library/tests/fixtures',
);

/** A library release of the synthetic demo library, made with `ewerk`. */
function demoLibraryRelease(): string {
  const work = '/tmp/ElectronicWerk/LibraryRelease';
  rmSync(work, { recursive: true, force: true });
  const repo = resolve(import.meta.dirname, '../../../..');
  const cargo = join(homedir(), '.cargo', 'bin', 'cargo');
  const ewerk = (...args: string[]): void => {
    execFileSync(cargo, ['run', '-q', '-p', 'ew-cli', '--', 'library', ...args], { cwd: repo });
  };
  ewerk(
    'build',
    '--kicad',
    join(LIBRARY_FIXTURES, 'kicad'),
    '--meshes',
    join(LIBRARY_FIXTURES, 'meshes'),
    '--version',
    'demo',
    '--dir',
    join(work, 'built'),
  );
  ewerk(
    'pack',
    join(work, 'feed'),
    '--prefix',
    'ewerk-library-demo',
    '--name',
    'Demo library',
    '--version',
    'demo',
    '--dir',
    join(work, 'built'),
  );
  return join(work, 'feed');
}

test('library manager', async () => {
  test.setTimeout(180_000);
  freshProjects();
  const { page, close } = await start({
    settings: { theme: 'light' },
    libraryFeed: demoLibraryRelease(),
  });
  await page.setViewportSize({ width: 1400, height: 860 });
  await shoot(page.locator('.start__actions'), 'library-start-action');

  // Setting up: download (here from the demo release) or build locally.
  await page.getByTestId('library').click();
  await expect(page.getByTestId('library-setup')).toContainText('Demo library');
  await shoot(page, 'library-setup');
  await page.getByTestId('library-download').click();
  await expect(page.getByTestId('library-manager')).toHaveAttribute('data-installed', 'true');

  // Browsing and searching.
  await page.getByTestId('library-kind-footprint').click();
  await page.getByTestId('library-search').fill('0805');
  await page.locator('.library-row[data-name="Demo:R_0805"]').click();
  await expect(page.getByTestId('library-preview')).toHaveAttribute('data-state', 'ready');
  await page.mouse.move(10, 840);
  await page.waitForTimeout(300);
  await shoot(page, 'library-footprint');
  await page.getByTestId('library-tab-3d').click();
  await expect(page.getByTestId('library-model')).toHaveAttribute('data-state', 'ready', {
    timeout: 60_000,
  });
  await page.waitForTimeout(500);
  await shoot(page, 'library-footprint-3d');
  await shoot(page.locator('.library-filters'), 'library-filters');

  await page.getByTestId('library-kind-symbol').click();
  await page.getByTestId('library-search').fill('demo semi');
  await page.locator('.library-row[data-name="Demo:LM_A"]').click();
  await expect(page.getByTestId('library-preview')).toHaveAttribute('data-state', 'ready');
  await page.waitForTimeout(300);
  await shoot(page, 'library-symbol');

  await page.getByTestId('library-kind-model').click();
  await page.getByTestId('library-search').fill('');
  await page.locator('.library-row[data-name="Demo:SOIC-8"]').click();
  await expect(page.getByTestId('library-model')).toHaveAttribute('data-state', 'ready', {
    timeout: 60_000,
  });
  await page.waitForTimeout(500);
  await shoot(page, 'library-model');

  // Reviewing duplicates.
  await page.getByTestId('library-tab-duplicates').click();
  await page.getByTestId('duplicates-kind').selectOption('footprint');
  await page.locator('[data-testid="duplicates-list"] .vlist__row').first().click();
  await expect(page.getByTestId('duplicate-preview').first()).toHaveAttribute(
    'data-state',
    'ready',
  );
  await page.waitForTimeout(500);
  await shoot(page, 'library-duplicates');
  await page.getByTestId('duplicates-kind').selectOption('model');
  await page.locator('[data-testid="duplicates-list"] .vlist__row').first().click();
  await expect(page.getByTestId('library-model').first()).toHaveAttribute('data-state', 'ready', {
    timeout: 60_000,
  });
  await page.waitForTimeout(800);
  await shoot(page, 'library-duplicates-3d');

  await page.getByTestId('library-info-button').click();
  await shoot(page.getByTestId('library-info'), 'library-info');
  await page.keyboard.press('Escape');
  await page.keyboard.press('Escape');

  // Adding a part to the project library.
  await page.getByTestId('new-project').click();
  await page.getByTestId('wizard-name').fill('Library Demo');
  await page.getByTestId('wizard-next').click();
  await page.getByTestId('template-board-2l').click();
  await page.getByTestId('wizard-create').click();
  await page.getByTestId('workspace').waitFor();
  await shoot(page.locator('.toolbar'), 'library-toolbar');
  await page.getByTestId('toolbar-library').click();
  await page.getByTestId('library-tab-browse').click();
  await page.getByTestId('library-kind-symbol').click();
  await page.getByTestId('library-search').fill('LM_B');
  await page.locator('.library-row[data-name="Demo:LM_B"]').click();
  await expect(page.getByTestId('library-preview')).toHaveAttribute('data-state', 'ready');
  await page.getByTestId('library-add').click();
  await expect(page.getByTestId('notice').last()).toContainText('Added 1 symbols');
  await page.waitForTimeout(300);
  await shoot(page, 'library-add');
  await close();
});

// --- Schematic editor ---------------------------------------------------------

const MM = 1_000_000;

/** Moves the mouse to a design point (nanometres). */
async function pointAt(page: Page, at: Point): Promise<Point> {
  const [x, y] = await toScreen(viewerCanvas(page), at);
  await page.mouse.move(x, y);
  return [x, y];
}

async function clickPoint(page: Page, at: Point): Promise<void> {
  const [x, y] = await pointAt(page, at);
  await page.mouse.click(x, y);
}

/** Waits until the last edit is drawn (the undo button names it). */
async function edited(page: Page, label: RegExp): Promise<void> {
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', label);
  await settle(page);
}

async function placeSymbol(page: Page, name: string, at: Point): Promise<void> {
  await viewerCanvas(page).focus();
  await page.keyboard.press('a');
  await page.getByTestId('chooser-search').fill(name);
  await page.getByTestId(`chooser-item-${name}`).click();
  await page.getByTestId('chooser-place').click();
  await clickPoint(page, at);
  await expect(page.getByTestId('property-designator')).toBeVisible();
  await page.keyboard.press('Escape');
}

/** Pans so that a design point is in the middle of the drawing area. */
async function centerOn(page: Page, at: Point): Promise<void> {
  const box = await viewerCanvas(page).boundingBox();
  await pointAt(page, at);
  await page.mouse.down({ button: 'middle' });
  await page.mouse.move(
    (box?.x ?? 0) + (box?.width ?? 0) / 2,
    (box?.y ?? 0) + (box?.height ?? 0) / 2,
    {
      steps: 6,
    },
  );
  await page.mouse.up({ button: 'middle' });
  await settle(page);
}

async function answer(page: Page, text: string): Promise<void> {
  await page.getByTestId('editor-prompt-input').fill(text);
  await page.getByTestId('editor-prompt-input').press('Enter');
}

test('schematic editor', async () => {
  test.setTimeout(180_000);
  freshProjects();
  const kicad = '/tmp/ElectronicWerk/KiCad/blinky';
  rmSync(kicad, { recursive: true, force: true });
  cpSync(KICAD_FIXTURE, kicad, { recursive: true });
  const { app, page, close } = await start({ settings: { theme: 'light' } });
  await page.setViewportSize({ width: 1400, height: 860 });
  await queueDialogAnswer(app, join(kicad, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-name').fill('Blinky');
  await page.getByTestId('kicad-import-start').click();
  await page
    .getByTestId('import-report')
    .getByRole('button', { name: 'Close', exact: true })
    .last()
    .click();
  await page.locator('[data-testid^="tree-doc:"]').filter({ hasText: 'Top sheet' }).dblclick();
  await expect(page.getByTestId('viewer')).toHaveAttribute('data-kind', 'schematic');
  await settle(page);
  await page.mouse.move(700, 840);
  await shoot(page, 'schematic-editor');
  await shoot(page.getByTestId('tool-palette'), 'schematic-tools');

  // The symbol chooser.
  await viewerCanvas(page).focus();
  await page.keyboard.press('a');
  await page.getByTestId('chooser-search').fill('Device');
  await page.getByTestId('chooser-item-Device:R').click();
  await expect(page.getByTestId('symbol-preview')).toBeVisible();
  await shoot(page.getByTestId('symbol-chooser'), 'schematic-chooser');
  await page.keyboard.press('Escape');

  // A small circuit in the empty top-left corner, zoomed in.
  await zoomAt(page, [55 * MM, 182 * MM], 5);
  await centerOn(page, [52 * MM, 183 * MM]);
  await viewerCanvas(page).focus();
  await page.keyboard.press('a');
  await page.getByTestId('chooser-search').fill('Device:R');
  await page.getByTestId('chooser-item-Device:R').click();
  await page.getByTestId('chooser-place').click();
  await pointAt(page, [40.64 * MM, 180.34 * MM]);
  await expect(page.getByTestId('preview-symbol')).toBeVisible();
  await shoot(page, 'schematic-place');
  await clickPoint(page, [40.64 * MM, 180.34 * MM]);
  await expect(page.getByTestId('property-designator')).toBeVisible();
  await page.keyboard.press('Escape');
  await placeSymbol(page, 'Device:C', [60.96 * MM, 180.34 * MM]);
  await settle(page);

  // Wires: from R4 pin 1 up, across and down to C3 pin 1.
  await viewerCanvas(page).focus();
  await page.keyboard.press('w');
  await clickPoint(page, [40.64 * MM, 184.15 * MM]);
  await clickPoint(page, [40.64 * MM, 189.23 * MM]);
  await pointAt(page, [60.96 * MM, 184.15 * MM]);
  await expect(page.getByTestId('preview-wire')).toBeAttached();
  await shoot(page, 'schematic-wire');
  await clickPoint(page, [60.96 * MM, 184.15 * MM]);
  await edited(page, /Draw wire/u);
  await page.keyboard.press('Escape');

  // A label on the wire, ground on both lower pins.
  await page.keyboard.press('l');
  await clickPoint(page, [50.8 * MM, 189.23 * MM]);
  await page.getByTestId('editor-prompt-input').fill('LED_CTRL');
  await shoot(page, 'schematic-label');
  await page.getByTestId('editor-prompt-input').press('Enter');
  await edited(page, /Add label/u);
  await viewerCanvas(page).focus();
  await page.keyboard.press('p');
  await clickPoint(page, [40.64 * MM, 176.53 * MM]);
  await answer(page, 'GND');
  await edited(page, /Add power port/u);
  await viewerCanvas(page).focus();
  await clickPoint(page, [60.96 * MM, 176.53 * MM]);
  await answer(page, 'GND');
  await expect(page.getByTestId('toolbar-undo')).toBeEnabled();
  await settle(page);
  await viewerCanvas(page).focus();
  await page.keyboard.press('Escape');

  // The properties of the capacitor (a click on its upper pin selects it).
  await clickPoint(page, [60.96 * MM, 182.88 * MM]);
  await expect(page.getByTestId('property-designator')).toBeVisible();
  await page.getByTestId('property-value').fill('100n');
  await page.getByTestId('property-value').press('Enter');
  await expect(page.getByTestId('property-value')).toHaveValue('100n');
  await page.mouse.move(700, 840);
  await shoot(page.getByTestId('inspector'), 'schematic-properties');

  // Dragging the capacitor: the wire stretches.
  const [cx, cy] = await pointAt(page, [60.96 * MM, 182.88 * MM]);
  await page.mouse.down();
  await page.mouse.move(cx + 40, cy, { steps: 4 });
  await page.mouse.move(cx + 80, cy + 20, { steps: 4 });
  await page.waitForTimeout(300);
  await shoot(page, 'schematic-drag');
  await page.mouse.up();
  await settle(page);

  // Net highlighting.
  await page.getByTestId('tool-highlight').click();
  await clickPoint(page, [45.72 * MM, 189.23 * MM]);
  await expect(page.getByTestId('net-chip')).toBeVisible();
  await page.mouse.move(700, 840);
  await shoot(page, 'schematic-net');
  await page.getByTestId('net-chip').click();
  await viewerCanvas(page).focus();
  await page.keyboard.press('Escape');

  // The electrical rules check and its settings.
  await page.getByTestId('viewer-fit').click();
  await page.getByTestId('action-erc').click();
  await expect(page.getByTestId('erc-list')).toBeVisible();
  await page.mouse.move(700, 840);
  await settle(page);
  await shoot(page, 'schematic-erc');
  await shoot(page.getByTestId('erc-panel'), 'schematic-erc-panel');
  await page.getByTestId('erc-settings').click();
  await expect(page.getByTestId('erc-matrix')).toBeVisible();
  await shoot(page.getByTestId('erc-settings-dialog'), 'schematic-erc-settings');
  await shoot(page.getByTestId('erc-matrix'), 'schematic-erc-matrix');
  await page.keyboard.press('Escape');
  await page.getByTestId('erc-close').click();

  // Find, annotate, export, grid.
  await page.getByTestId('action-find').click();
  await page.getByTestId('find-query').fill('GND');
  await expect(page.getByTestId('find-count')).toContainText('Matches');
  await shoot(page.getByTestId('find-panel'), 'schematic-find');
  await page.getByTestId('find-close').click();
  await page.getByTestId('action-annotate').click();
  await shoot(page.getByTestId('annotate-dialog'), 'schematic-annotate');
  await page.keyboard.press('Escape');
  await page.getByTestId('action-export').click();
  await shoot(page.getByTestId('export-menu'), 'schematic-export');
  await page.getByTestId('action-export').click();
  await page.getByTestId('action-grid').click();
  await expect(page.getByTestId('grid-status')).toBeVisible();
  await shoot(page.getByTestId('grid-assistant'), 'schematic-grid');
  await page.getByTestId('action-grid').click();
  await close();

  // The guide on the empty sheet of a new project, and the dark theme.
  const dark = await start({ settings: { theme: 'dark' } });
  await dark.page.setViewportSize({ width: 1400, height: 860 });
  await createDemo(dark.page, 'First Schematic', 'empty');
  await openDocument(dark.page, 'Root');
  await expect(dark.page.getByTestId('guide')).toBeVisible();
  await dark.page.mouse.move(900, 840);
  await shoot(dark.page, 'schematic-guide-dark');
  await shoot(dark.page.getByTestId('guide'), 'schematic-guide');
  await dark.close();
});

test('net navigator', async () => {
  freshProjects();
  const kicad = '/tmp/ElectronicWerk/KiCad/blinky';
  rmSync(kicad, { recursive: true, force: true });
  cpSync(KICAD_FIXTURE, kicad, { recursive: true });
  const { app, page, close } = await start({ settings: { theme: 'light' } });
  await page.setViewportSize({ width: 1400, height: 860 });
  await queueDialogAnswer(app, join(kicad, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-name').fill('Blinky');
  await page.getByTestId('kicad-import-start').click();
  await page
    .getByTestId('import-report')
    .getByRole('button', { name: 'Close', exact: true })
    .last()
    .click();
  await page.locator('[data-testid^="tree-doc:"]').filter({ hasText: 'Top sheet' }).dblclick();
  await settle(page);
  await page.getByTestId('action-find').click();
  await page.getByTestId('find-query').fill('GND');
  await page.getByTestId('find-hits').getByRole('button').first().click();
  await page.getByTestId('find-query').press('Escape');
  await settle(page);
  const canvas = viewerCanvas(page);
  const box = await canvas.boundingBox();
  await canvas.dblclick({
    position: { x: (box?.width ?? 0) / 2, y: (box?.height ?? 0) / 2 },
    modifiers: ['ControlOrMeta'],
  });
  await expect(page.getByTestId('net-navigator')).toContainText('Net GND');
  await settle(page);
  await page.mouse.move(700, 840);
  await shoot(page, 'schematic-net-navigator');
  await shoot(page.getByTestId('net-navigator'), 'schematic-net-navigator-panel');
  await close();
});

/** The drawn canvas of the symbol or footprint editor. */
function itemCanvas(page: Page): Locator {
  return page.locator('[data-testid="item-canvas-area"][data-ready="true"]');
}

/** Waits until the item view stops moving. */
async function settleItem(page: Page): Promise<void> {
  const canvas = itemCanvas(page);
  await canvas.waitFor();
  await expect(async () => {
    const first = await canvas.getAttribute('data-view');
    await page.waitForTimeout(150);
    expect(await canvas.getAttribute('data-view')).toBe(first);
  }).toPass();
}

async function clickItem(page: Page, at: Point): Promise<void> {
  const [x, y] = await toScreen(itemCanvas(page), at);
  await page.mouse.move(x, y);
  await page.mouse.click(x, y);
}

/** Types into a table cell and saves it. */
async function cell(page: Page, testId: string, value: string): Promise<void> {
  await page.getByTestId(testId).fill(value);
  await page.getByTestId(testId).press('Enter');
  await expect(page.getByTestId(testId)).toHaveValue(value);
}

test('library editor', async () => {
  test.setTimeout(180_000);
  freshProjects();
  const { page, close } = await start({ settings: { theme: 'light' } });
  await page.setViewportSize({ width: 1600, height: 940 });
  await createDemo(page, 'Level Shifter', 'board-2l');
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Project library' }).dblclick();
  await expect(page.getByTestId('library-editor')).toBeVisible();
  await page.getByTestId('libedit-item-Q_NPN_BCE').click();
  await settleItem(page);
  await shoot(page, 'libedit-library');

  // A new symbol: four pins from a pasted list on the left …
  await page.getByTestId('libedit-new').click();
  await page.getByTestId('libedit-name-input').fill('LVL_SHIFT_2CH');
  await shoot(page.getByTestId('libedit-name-dialog'), 'libedit-new');
  await page.getByTestId('libedit-name-confirm').click();
  await expect(page.getByTestId('item-name')).toHaveText('LVL_SHIFT_2CH');
  await settleItem(page);
  await page.getByTestId('paste-pins').click();
  await page.getByTestId('paste-pins-text').fill('1\tVCCA\tW\n2\tA1\tB\n3\tA2\tB\n4\tGND\tW');
  await shoot(page.getByTestId('paste-pins-dialog'), 'libedit-paste-pins');
  await page.getByTestId('paste-pins-add').click();
  await expect(page.getByTestId('pin-row-3')).toBeVisible();

  // … four pins with the pin tool on the right (R turns them to the left) …
  await page.getByTestId('item-tool-pin').click();
  await itemCanvas(page).focus();
  await page.keyboard.press('r');
  await page.keyboard.press('r');
  await expect(page.getByTestId('pin-default-direction')).toHaveValue('left');
  for (const [index, y] of [0, -2.54, -5.08, -7.62].entries()) {
    await clickItem(page, [7.62 * MM, y * MM]);
    await expect(page.getByTestId(`pin-row-${String(index + 4)}`)).toBeVisible();
  }
  await shoot(page.getByTestId('item-options'), 'libedit-pin-options');
  // … and the body.
  await page.getByTestId('item-tool-rect').click();
  await clickItem(page, [-5.08 * MM, 2.54 * MM]);
  await clickItem(page, [5.08 * MM, -10.16 * MM]);
  await expect(page.getByTestId('selection-props')).toContainText('Graphic');
  await page.keyboard.press('Escape');
  for (const [row, name] of [
    [4, 'B1'],
    [5, 'B2'],
    [6, 'OE'],
    [7, 'VCCB'],
  ] as const) {
    await cell(page, `pin-name-${String(row)}`, name);
  }
  await page.getByTestId('pin-kind-4').selectOption('bidirectional');
  await page.getByTestId('pin-kind-5').selectOption('bidirectional');
  await page.getByTestId('pin-kind-6').selectOption('input');
  await page.getByTestId('pin-kind-7').selectOption('powerIn');
  await page.getByTestId('prop-description').fill('Two-channel bidirectional level shifter');
  await page.getByTestId('prop-description').blur();
  await page.getByRole('button', { name: 'Fit to window' }).first().click();
  await settleItem(page);
  // Rows 2 and 3 selected for a bulk edit.
  await page.getByTestId('pin-row-1').click({ position: { x: 4, y: 4 } });
  await page.getByTestId('pin-row-2').click({ position: { x: 4, y: 4 }, modifiers: ['Shift'] });
  await expect(page.getByTestId('pin-table')).toContainText('2 selected');
  await page.mouse.move(10, 10);
  await page
    .getByTestId('pin-table')
    .locator('.item-table__scroll')
    .evaluate((element) => {
      element.scrollTop = 0;
    });
  await shoot(page, 'libedit-symbol');
  await shoot(page.getByTestId('pin-table'), 'libedit-pin-table');

  // One pin selected: its properties.
  await page.getByTestId('pin-row-6').click({ position: { x: 4, y: 4 } });
  await expect(page.getByTestId('selection-props')).toContainText('Pin 7');
  await shoot(page.getByTestId('selection-props'), 'libedit-pin-properties');

  // A De Morgan body and a second unit.
  await page.getByTestId('body-alternate').click();
  await expect(page.getByTestId('body-alternate')).toHaveText('De Morgan');
  await page.getByTestId('body-alternate').click();
  await settleItem(page);
  await shoot(page.getByTestId('unit-bar'), 'libedit-units');
  await page.getByTestId('body-normal').click();
  await settleItem(page);

  // The footprint wizard.
  await page.getByTestId('libedit-kind-footprint').click();
  await page.getByTestId('libedit-wizard').click();
  await page.getByTestId('fpwizard-preset').selectOption('QFN-32 5x5');
  await expect(page.getByTestId('fpwizard-preview')).toContainText('33 pads');
  await shoot(page.getByTestId('fpwizard-dialog'), 'libedit-wizard-qfn');
  await page.getByTestId('fpwizard-preset').selectOption('SOIC-8');
  await page.getByTestId('fpwizard-density').selectOption('most');
  await expect(page.getByTestId('fpwizard-name')).toHaveValue('SOIC-8_3.9x4.9mm_P1.27mm');
  await page.getByTestId('fpwizard-name').fill('SOIC-8_HandSolder');
  await shoot(page.getByTestId('fpwizard-dialog'), 'libedit-wizard');
  await page.getByTestId('fpwizard-create').click();
  await expect(page.getByTestId('item-name')).toHaveText('SOIC-8_HandSolder');
  await settleItem(page);
  // A 3D model reference and pad 1 selected.
  await page
    .getByTestId('model-add-path')
    .fill('${KICAD9_3DMODEL_DIR}/Package_SO.3dshapes/SOIC-8_3.9x4.9mm_P1.27mm.step');
  await page.getByTestId('model-add').click();
  await expect(page.getByTestId('model-0')).toBeVisible();
  await page.getByTestId('pad-row-0').click({ position: { x: 4, y: 4 } });
  await expect(page.getByTestId('selection-props')).toContainText('Pad 1');
  await page.mouse.move(10, 10);
  await shoot(page, 'libedit-footprint');
  await shoot(page.getByTestId('pad-table'), 'libedit-pad-table');
  await shoot(page.getByTestId('selection-props'), 'libedit-pad-properties');
  await shoot(page.getByTestId('item-models'), 'libedit-models');

  // A pad array.
  await page.getByTestId('pad-array').click();
  await page.getByTestId('pad-array-columns').fill('2');
  await page.getByTestId('pad-array-rows').fill('3');
  await page.getByTestId('pad-array-rows').press('Tab');
  await expect(page.getByTestId('pad-array-add')).toHaveText('Add 6 pads');
  await shoot(page.getByTestId('pad-array-dialog'), 'libedit-pad-array');
  await page.keyboard.press('Escape');

  // The checks of the whole library.
  await page.getByTestId('tab-checks').click();
  await page.getByTestId('check-library').click();
  await expect(page.getByTestId('library-findings')).toBeVisible();
  await page
    .getByTestId('library-findings')
    .locator('summary')
    .first()
    .click({ position: { x: 4, y: 6 } });
  await shoot(page.getByTestId('checks-panel'), 'libedit-checks');
  await page.getByTestId('tab-table').click();

  // A component: symbol, footprint and order data.
  await page.getByTestId('libedit-kind-component').click();
  await page.getByTestId('libedit-new').click();
  await page.getByTestId('libedit-name-input').fill('LVL_SHIFT_2CH_SO8');
  await page.getByTestId('libedit-name-confirm').click();
  await page.getByTestId('component-symbol').selectOption('LVL_SHIFT_2CH');
  await page.getByTestId('component-footprint').fill('SOIC-8_HandSolder');
  await page.getByTestId('component-footprint').press('Enter');
  await cell(page, 'component-value', 'LVL_SHIFT_2CH');
  await cell(page, 'component-manufacturer', 'Example Semiconductors');
  await cell(page, 'component-mpn', 'EX-LS2-SO8');
  await page.getByTestId('prop-description').fill('Two-channel level shifter, SOIC-8');
  await page.getByTestId('prop-description').blur();
  await page.getByTestId('supplier-new').fill('Example Distribution');
  await page.getByTestId('supplier-add').click();
  await cell(page, 'supplier-pn-0', '123-EXLS2SO8');
  await expect(page.getByTestId('component-pin-map')).toContainText('VCCB');
  await page.mouse.move(10, 10);
  await shoot(page, 'libedit-component');

  // Placing the component on the sheet.
  await openDocument(page, 'Root');
  await viewerCanvas(page).focus();
  await page.keyboard.press('a');
  await page.getByTestId('chooser-search').fill('shift');
  await page.getByTestId('chooser-component-LVL_SHIFT_2CH_SO8').click();
  await expect(page.getByTestId('symbol-preview')).toBeVisible();
  await shoot(page.getByTestId('symbol-chooser'), 'libedit-chooser-component');
  await page.keyboard.press('Escape');
  await page.getByTestId('toolbar-save').click();
  await expect(page.getByTestId('dirty-indicator')).toHaveAttribute('data-dirty', 'false');
  await close();

  // The dark theme.
  const dark = await start({ settings: { theme: 'dark' } });
  await dark.page.setViewportSize({ width: 1600, height: 940 });
  await queueDialogAnswer(dark.app, join(PROJECTS, 'Level Shifter', 'Level Shifter.ewproj'));
  await dark.page.getByTestId('open-project').click();
  await dark.page.locator('[data-testid^="tree-doc:"]', { hasText: 'Project library' }).dblclick();
  await dark.page.getByTestId('libedit-kind-footprint').click();
  await settleItem(dark.page);
  await dark.page.mouse.move(10, 10);
  await shoot(dark.page, 'libedit-footprint-dark');
  await dark.close();
});

interface BoardJson {
  footprints: {
    id: string;
    designator: string;
    at: Point;
    rotation?: number;
    side?: string;
    pads: { number: string; at: Point; net?: string }[];
  }[];
}

function readBoardJson(project: string): BoardJson {
  const dir = join(PROJECTS, project, 'pcb');
  const file = readdirSync(dir).find((name) => !name.startsWith('.'));
  return JSON.parse(readFileSync(join(dir, file ?? ''), 'utf8')) as BoardJson;
}

test('layout editor', async () => {
  test.setTimeout(240_000);
  freshProjects();
  const kicad = '/tmp/ElectronicWerk/KiCad/blinky';
  rmSync(kicad, { recursive: true, force: true });
  cpSync(KICAD_FIXTURE, kicad, { recursive: true });
  const { app, page, close } = await start({ settings: { theme: 'light' } });
  await page.setViewportSize({ width: 1500, height: 900 });
  await queueDialogAnswer(app, join(kicad, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-name').fill('Blinky');
  await page.getByTestId('kicad-import-start').click();
  await page
    .getByTestId('import-report')
    .getByRole('button', { name: 'Close', exact: true })
    .last()
    .click();
  const board = readBoardJson('Blinky');
  await page
    .locator('[data-testid^="tree-doc:"]')
    .filter({ hasText: /^Blinky$/u })
    .dblclick();
  await settle(page);
  await expect(page.getByTestId('viewer')).toHaveAttribute('data-kind', 'pcb');

  // The editor with the online design rules check and the ratsnest.
  await page.getByTestId('board-fill').click();
  await edited(page, /fill-zones|Fill/u);
  await page.getByTestId('board-drc').click();
  await expect(page.getByTestId('drc-summary')).toBeVisible();
  await settle(page);
  await page.mouse.move(10, 10);
  await shoot(page, 'layout-editor');
  await shoot(page.getByTestId('drc-panel'), 'layout-drc-panel');
  await shoot(page.getByTestId('board-tool-palette'), 'layout-tools');

  // A footprint in the inspector.
  const u1 = board.footprints.find((f) => f.designator === 'U1');
  if (u1 !== undefined) {
    await clickPoint(page, u1.at);
    await expect(page.getByTestId('board-properties')).toBeVisible();
    await page.mouse.move(10, 10);
    await shoot(page.getByTestId('inspector'), 'layout-properties');
    await page.keyboard.press('Escape');
  }

  // Routing: from a pad of a net towards a free spot.
  const fp = board.footprints.find(
    (f) => f.designator === 'R3' || (f.pads.some((p) => p.net !== undefined) && !f.rotation),
  );
  const pad = fp?.pads.find((p) => p.net !== undefined);
  if (fp !== undefined && pad !== undefined) {
    const start: Point = [fp.at[0] + pad.at[0], fp.at[1] + pad.at[1]];
    await viewerCanvas(page).focus();
    await page.keyboard.press('x');
    await clickPoint(page, start);
    await expect(page.getByTestId('route-status')).toBeVisible();
    const [sx, sy] = await toScreen(viewerCanvas(page), start);
    await page.mouse.move(sx + 40, sy + 90, { steps: 8 });
    await expect(page.getByTestId('route-preview')).toBeVisible();
    await settle(page);
    await shoot(page, 'layout-routing');
    await page.keyboard.press('Escape');
    await page.keyboard.press('Escape');
  }

  // Dialogs.
  await page.getByTestId('board-rules').click();
  const rules = page.getByTestId('rules-dialog');
  await expect(page.getByTestId('class-name-0')).toHaveValue('Default');
  await shoot(rules, 'layout-rules-classes');
  await page.getByTestId('rules-tab-constraints').click();
  await shoot(rules, 'layout-rules-minimums');
  await page.getByTestId('rules-tab-custom').click();
  await page.getByTestId('rule-add').click();
  await page.getByTestId('rule-name-0').fill('HV spacing');
  await page.getByTestId('rule-name-0').press('Enter');
  await page.getByTestId('rule-condition-0').fill("A.netclass == 'Power' && B.netclass != 'Power'");
  await page.getByTestId('rule-condition-0').press('Enter');
  await page.getByTestId('rule-clearance-0').check();
  await page.getByTestId('rule-value-clearance-0').fill('0.5');
  await page.getByTestId('rule-value-clearance-0').press('Enter');
  await page.getByTestId('rule-add').click();
  await page.getByTestId('rule-condition-1').fill("A.type == 'via' && A.layer = 'cu.top'");
  await page.getByTestId('rule-condition-1').press('Enter');
  await expect(page.getByTestId('rule-error-1')).toBeVisible();
  await shoot(rules, 'layout-rules-custom');
  await page.getByTestId('rules-tab-checks').click();
  await shoot(rules, 'layout-rules-checks');
  await rules.getByRole('button', { name: 'Cancel' }).click();

  await page.getByTestId('board-setup').click();
  const setup = page.getByTestId('setup-dialog');
  await expect(page.getByTestId('preset-standard-2l')).toBeVisible();
  await shoot(setup, 'layout-setup-presets');
  await page.getByTestId('setup-tab-stackup').click();
  await expect(page.getByTestId('stackup-table')).toBeVisible();
  await shoot(setup, 'layout-setup-stackup');
  await page.getByTestId('setup-tab-outline').click();
  await shoot(setup, 'layout-setup-outline');
  await setup.getByRole('button', { name: 'Cancel' }).click();

  await page.getByTestId('board-update').click();
  const sync = page.getByTestId('sync-dialog');
  await expect(page.getByTestId('sync-none').or(page.getByTestId('sync-list'))).toBeVisible();
  await shoot(sync, 'layout-sync');
  await sync.getByRole('button', { name: 'Cancel' }).click();

  // A new zone: its settings dialog.
  await viewerCanvas(page).focus();
  await page.keyboard.press('z');
  const box = await viewerCanvas(page).boundingBox();
  if (box !== null) {
    for (const [fx, fy] of [
      [0.62, 0.25],
      [0.8, 0.25],
      [0.8, 0.45],
      [0.62, 0.45],
    ] as const) {
      await page.mouse.click(box.x + box.width * fx, box.y + box.height * fy);
    }
    await settle(page);
    await shoot(page.locator('.viewer__area'), 'layout-zone-drawing');
    await page.keyboard.press('Enter');
    const zone = page.getByTestId('zone-dialog');
    await expect(zone).toBeVisible();
    await page.getByTestId('zone-name').fill('Shield');
    await page.getByTestId('zone-name').press('Enter');
    await page.getByTestId('zone-net').selectOption({ label: 'GND' });
    await shoot(zone, 'layout-zone-dialog');
    await page.getByTestId('zone-save').click();
    await expect(zone).toBeHidden();
    await edited(page, /fill-zones|Fill/u);
    await page.mouse.move(10, 10);
    await shoot(page, 'layout-zone-filled');
  }

  // The DXF import.
  const dxf = '/tmp/ElectronicWerk/Drawings/enclosure.dxf';
  mkdirSync('/tmp/ElectronicWerk/Drawings', { recursive: true });
  writeFileSync(
    dxf,
    [
      ...['0', 'SECTION', '2', 'HEADER', '9', '$INSUNITS', '70', '4', '0', 'ENDSEC'],
      ...['0', 'SECTION', '2', 'ENTITIES'],
      ...['0', 'LWPOLYLINE', '8', 'BOARD', '90', '6', '70', '1'],
      ...['10', '0', '20', '0', '10', '60', '20', '0', '42', '0'],
      ...['10', '60', '20', '35', '42', '0.4142', '10', '50', '20', '45'],
      ...['10', '0', '20', '45', '10', '0', '20', '0'],
      ...['0', 'CIRCLE', '8', 'HOLES', '10', '5', '20', '5', '40', '1.6'],
      ...['0', 'CIRCLE', '8', 'HOLES', '10', '55', '20', '5', '40', '1.6'],
      ...['0', 'TEXT', '8', 'NOTES', '1', 'Enclosure rev 2'],
      ...['0', 'ENDSEC', '0', 'EOF', ''],
    ].join('\n'),
  );
  await queueDialogAnswer(app, dxf);
  await page.getByTestId('board-dxf').click();
  await page.getByTestId('dxf-choose').click();
  await expect(page.getByTestId('dxf-summary')).toBeVisible();
  await shoot(page.getByTestId('dxf-dialog'), 'layout-dxf');
  await page.getByTestId('dxf-dialog').getByRole('button', { name: 'Cancel' }).click();
  await close();
});

// --- Manufacturing outputs (M8) ----------------------------------------------

/** Every file below a folder, as paths relative to it. */
function filesBelow(dir: string, prefix = ''): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory()
      ? filesBelow(join(dir, entry.name), `${prefix}${entry.name}/`)
      : [`${prefix}${entry.name}`],
  );
}

/** Shows a generated SVG drawing over the window and photographs it. */
async function shootSvg(page: Page, file: string, name: string): Promise<void> {
  const markup = readFileSync(file, 'utf8').replace(/^<\?xml[^>]*>\s*/u, '');
  await page.evaluate((svg) => {
    const frame = document.createElement('div');
    frame.id = 'doc-drawing';
    frame.style.cssText =
      'position:fixed;inset:0;z-index:100000;display:flex;align-items:center;' +
      'justify-content:center;padding:24px;background:#e9ecef';
    const sheet = document.createElement('div');
    sheet.style.cssText =
      'width:100%;height:100%;display:flex;align-items:center;justify-content:center';
    sheet.innerHTML = svg;
    const element = sheet.querySelector('svg');
    if (element !== null) {
      element.style.cssText =
        'max-width:100%;max-height:100%;width:auto;height:100%;background:#fff;' +
        'box-shadow:0 2px 12px rgb(0 0 0 / 25%)';
    }
    frame.appendChild(sheet);
    document.body.appendChild(frame);
  }, markup);
  await shoot(page.locator('#doc-drawing'), name);
  await page.evaluate(() => {
    document.getElementById('doc-drawing')?.remove();
  });
}

test('manufacturing outputs', async () => {
  test.setTimeout(300_000);
  freshProjects();
  const kicad = '/tmp/ElectronicWerk/KiCad/blinky';
  rmSync(kicad, { recursive: true, force: true });
  cpSync(KICAD_FIXTURE, kicad, { recursive: true });
  const { app, page, close } = await start({ settings: { theme: 'light' } });
  await page.setViewportSize({ width: 1500, height: 900 });
  await queueDialogAnswer(app, join(kicad, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-name').fill('Blinky');
  await page.getByTestId('kicad-import-start').click();
  await page
    .getByTestId('import-report')
    .getByRole('button', { name: 'Close', exact: true })
    .last()
    .click();
  const generated = join(PROJECTS, 'Blinky', 'outputs', 'generated');

  // The output job with the Gerber settings.
  await page.getByTestId('toolbar-outputs').click();
  await expect(page.getByTestId('output-job')).toBeVisible();
  await expect(page.getByTestId('output-settings')).toBeVisible();
  await page.mouse.move(10, 10);
  await shoot(page, 'outputs-job');

  // Generate everything.
  await page.getByTestId('outputs-generate').click();
  const list = page.getByTestId('outputs-list');
  await expect(list.locator('[data-status="done"]')).toHaveCount(9, { timeout: 60_000 });
  await page.locator('.toast__close').first().click();
  await page.mouse.move(10, 10);
  await shoot(page, 'outputs-generated');
  await shoot(page.getByTestId('outputs-results'), 'outputs-results');

  // Settings of the bill of materials, the placement file and a print.
  const settings = page.locator('.outputs__settings');
  await page.getByTestId('output-3').getByRole('button').click();
  await expect(page.getByTestId('output-bom-preview').locator('tbody tr').first()).toBeVisible();
  await shoot(settings, 'outputs-bom');
  await page.getByTestId('output-4').getByRole('button').click();
  await expect(page.getByTestId('output-placement-style')).toBeVisible();
  await shoot(settings, 'outputs-placement');
  await page.getByTestId('output-7').getByRole('button').click();
  await expect(page.getByTestId('output-content')).toHaveValue('assembly');
  await shoot(settings, 'outputs-print');

  // A board maker preset.
  await page.getByTestId('outputs-preset').selectOption('jlcpcb');
  await expect(page.getByTestId('outputs-preset-confirm')).toBeVisible();
  await shoot(page.getByTestId('outputs-preset-confirm'), 'outputs-preset');
  await page.getByTestId('outputs-preset-confirm').getByRole('button', { name: 'Cancel' }).click();

  // The Gerber viewer.
  await page.getByTestId('outputs-gerber-viewer').click();
  await expect(page.getByTestId('gerber-canvas')).toHaveAttribute('data-ready', 'true', {
    timeout: 30_000,
  });
  // The inner GND plane covers everything: show the outer layers.
  const eyes = page.getByTestId('gerber-viewer').locator('.layer-list__eye');
  await eyes.nth(1).click();
  await eyes.nth(2).click();
  await page.waitForTimeout(500);
  await page.mouse.move(10, 10);
  await shoot(page, 'gerber-viewer');
  await page.getByTestId('gerber-flip').click();
  await page.waitForTimeout(500);
  await shoot(page.locator('.viewer__main'), 'gerber-viewer-bottom');
  await page.getByTestId('gerber-flip').click();

  // The generated drawings: as SVG for the documentation.
  await page.locator('[data-testid^="tab-"]', { hasText: 'Fabrication' }).first().click();
  await page.getByTestId('output-2').getByRole('button').click();
  await page.getByTestId('output-format').selectOption('svg');
  await page.getByTestId('output-generate-one').click();
  await expect(page.getByTestId('output-2').locator('[data-status="done"]')).toBeVisible();
  const add = async (content: string): Promise<void> => {
    await page.getByTestId('outputs-add').selectOption('svg');
    await page.getByTestId('output-content').selectOption(content);
    await page.getByTestId('output-generate-one').click();
    await expect(list.locator('li').last().locator('[data-status="done"]')).toBeVisible();
  };
  await add('schematic');
  await add('assembly');
  await add('board');
  const files = filesBelow(generated);
  const pick = (pattern: RegExp): string => {
    const found = files.find((file) => pattern.test(file));
    if (found === undefined) {
      throw new Error(`no file matching ${String(pattern)} in ${files.join(', ')}`);
    }
    return join(generated, found);
  };
  await shootSvg(page, pick(/drill-map\.svg$/u), 'output-drill-map');
  await shootSvg(page, pick(/schematic-1\.svg$/u), 'output-schematic');
  await shootSvg(page, pick(/assembly-1\.svg$/u), 'output-assembly-top');
  await shootSvg(page, pick(/assembly-2\.svg$/u), 'output-assembly-bottom');
  await shootSvg(page, pick(/layers\.svg$/u), 'output-board-print');
  await close();
});

test('3d view', async () => {
  test.setTimeout(300_000);
  freshProjects();
  const kicad = '/tmp/ElectronicWerk/KiCad/blinky';
  rmSync(kicad, { recursive: true, force: true });
  cpSync(KICAD_FIXTURE, kicad, { recursive: true });
  const { app, page, close } = await start({ settings: { theme: 'light' } });
  await page.setViewportSize({ width: 1500, height: 900 });
  await queueDialogAnswer(app, join(kicad, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-name').fill('Blinky');
  await page.getByTestId('kicad-import-start').click();
  await page
    .getByTestId('import-report')
    .getByRole('button', { name: 'Close', exact: true })
    .last()
    .click();
  await page
    .locator('[data-testid^="tree-doc:"]')
    .filter({ hasText: /^Blinky$/u })
    .dblclick();
  await settle(page);
  await expect(page.getByTestId('viewer')).toHaveAttribute('data-kind', 'pcb');

  // The 3D view next to the layout, with every model loaded.
  await page.getByTestId('viewer-open-3d').click();
  const canvas = page.locator('[data-testid="view3d-canvas"][data-ready="true"]');
  await expect(canvas).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId('view3d-models')).toHaveText(/^(\d+) of \1 models$/u, {
    timeout: 120_000,
  });
  const still = async (): Promise<void> => {
    await page.mouse.move(5, 300);
    await page.waitForTimeout(800);
  };
  await still();
  await shoot(page, 'view3d');
  const view = page.locator('.view3d');

  // Views and projection.
  await page.getByTestId('view3d-views').click();
  await shoot(page.locator('.workspace__main'), 'view3d-views');
  await page.getByTestId('view3d-view-top').click();
  await page.getByTestId('view3d-projection').click();
  await still();
  await shoot(view, 'view3d-top');
  await page.getByTestId('view3d-views').click();
  await page.getByTestId('view3d-view-bottom').click();
  await still();
  await shoot(view, 'view3d-bottom');
  await page.getByTestId('view3d-projection').click();

  // What is shown.
  await page.getByTestId('view3d-view-iso').click();
  await page.getByTestId('view3d-show').click();
  await page.getByTestId('view3d-show-mask').uncheck();
  await page.getByTestId('view3d-show-silk').uncheck();
  await still();
  await shoot(view, 'view3d-show');
  await page.getByTestId('view3d-show-mask').check();
  await page.getByTestId('view3d-show-silk').check();
  await page.getByTestId('view3d-show').click();

  // A section through the SOIC and the pin header.
  await page.getByTestId('view3d-section').click();
  await page.getByTestId('view3d-section-y').check();
  await page.getByTestId('view3d-section-position').fill('0.55');
  await still();
  await shoot(view, 'view3d-section');
  await page.getByTestId('view3d-section-off').check();
  await page.getByTestId('view3d-section').click();

  // Measuring in the top view.
  await page.getByTestId('view3d-views').click();
  await page.getByTestId('view3d-view-top').click();
  await page.getByTestId('view3d-projection').click();
  await page.getByTestId('view3d-measure').click();
  const box = await canvas.boundingBox();
  const cx = (box?.x ?? 0) + (box?.width ?? 0) / 2;
  const cy = (box?.y ?? 0) + (box?.height ?? 0) / 2;
  await page.mouse.click(cx - 220, cy + 120);
  await page.mouse.click(cx + 180, cy - 90);
  await expect(page.getByTestId('view3d-measure-label')).toBeVisible();
  await still();
  await shoot(view, 'view3d-measure');
  await page.keyboard.press('Escape');
  await page.getByTestId('view3d-projection').click();

  // A part selected in the layout editor is selected in 3D too.
  await page.getByTestId('view3d-view-iso').click();
  await page.locator('[role="tab"]', { hasText: /^Blinky$/u }).click();
  await settle(page);
  const board = readBoardJson('Blinky');
  const u1 = board.footprints.find((f) => f.designator === 'U1');
  const [ux, uy] = await toScreen(viewerCanvas(page), u1?.at ?? [0, 0]);
  await page.mouse.click(ux, uy);
  await page.locator('[role="tab"]', { hasText: 'Blinky (3D)' }).click();
  await expect(canvas).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId('view3d-selected')).toContainText('U1');
  await expect(page.getByTestId('view3d-models')).toHaveText(/^(\d+) of \1 models$/u, {
    timeout: 120_000,
  });
  await still();
  await shoot(page, 'view3d-selection');

  // Export.
  await page.getByTestId('view3d-export').click();
  await shoot(page.locator('.workspace__main'), 'view3d-export');
  await page.getByTestId('view3d-export').click();

  // Its own window.
  const opened = app.waitForEvent('window');
  await page.getByTestId('view3d-detach').click();
  const second = await opened;
  await second.setViewportSize({ width: 1100, height: 700 });
  const secondCanvas = second.locator('[data-testid="view3d-canvas"][data-ready="true"]');
  await expect(secondCanvas).toBeVisible({ timeout: 60_000 });
  await expect(second.getByTestId('view3d-models')).toHaveText(/^(\d+) of \1 models$/u, {
    timeout: 120_000,
  });
  await second.mouse.move(5, 300);
  await second.waitForTimeout(800);
  await shoot(second, 'view3d-window');
  await second.close();

  // A 3D model in the output job.
  await page.getByTestId('toolbar-outputs').click();
  await expect(page.getByTestId('output-job')).toBeVisible();
  await page.getByTestId('outputs-add').selectOption('step');
  await expect(page.getByTestId('output-model3d-format')).toBeVisible();
  await page.getByTestId('output-generate-one').click();
  const list = page.getByTestId('outputs-list');
  await expect(list.locator('li').last().locator('[data-status="done"]')).toBeVisible({
    timeout: 60_000,
  });
  await page.mouse.move(10, 10);
  await shoot(page.locator('.outputs__settings'), 'outputs-3d');
  await close();
});

test('online parts', async () => {
  test.setTimeout(240_000);
  freshProjects();
  const { app, page, close } = await start({
    settings: { theme: 'light' },
    libraryFeed: demoLibraryRelease(),
  });
  await page.setViewportSize({ width: 1400, height: 860 });
  // A base library, so the import finds a duplicate.
  await page.getByTestId('library').click();
  await page.getByTestId('library-download').click();
  await expect(page.getByTestId('library-manager')).toHaveAttribute('data-installed', 'true');
  await page.getByTestId('library-close').click();
  await createProjectWithWizard(page, 'Timer', 'empty');

  // The online part libraries.
  await page.keyboard.press('ControlOrMeta+Shift+P');
  await page.getByTestId('command-input').fill('parts online');
  await page.keyboard.press('Enter');
  await page.getByTestId('online-query').fill('NE555');
  await page.mouse.move(5, 5);
  await shoot(page.getByTestId('online-parts'), 'parts-online');

  // A SnapEDA download: the footprint is in the base library already.
  const parts = resolve(import.meta.dirname, '../../../../crates/ew-core/tests/fixtures/parts');
  const downloads = '/tmp/ElectronicWerk/Downloads';
  rmSync(downloads, { recursive: true, force: true });
  mkdirSync(downloads, { recursive: true });
  cpSync(join(parts, 'NE555-DEMO.zip'), join(downloads, 'NE555-DEMO.zip'));
  cpSync(join(parts, 'ul_OPAMP-DEMO.zip'), join(downloads, 'ul_OPAMP-DEMO.zip'));
  await queueDialogAnswer(app, join(downloads, 'NE555-DEMO.zip'));
  await page.getByTestId('online-import-file').click();
  const importer = page.getByTestId('download-import');
  await expect(importer.getByTestId('duplicate-exact')).toBeVisible();
  await shoot(importer, 'parts-import');
  await page.getByTestId('download-import-start').click();
  await expect(importer).toBeHidden();

  // An Ultra Librarian download with its own footprint and 3D model.
  await page.keyboard.press('ControlOrMeta+Shift+P');
  await page.getByTestId('command-input').fill('parts online');
  await page.keyboard.press('Enter');
  await queueDialogAnswer(app, join(downloads, 'ul_OPAMP-DEMO.zip'));
  await page.getByTestId('online-import-file').click();
  await expect(importer.getByTestId('import-model')).toBeVisible();
  await shoot(importer, 'parts-import-ul');
  await page.getByTestId('download-import-start').click();
  await expect(importer).toBeHidden();
  await expect(page.getByTestId('notice').last()).toContainText('Imported');
  await shoot(page.getByTestId('notices'), 'parts-imported');

  // The accounts in the settings.
  await page.getByTestId('open-settings').click();
  await shoot(page.getByTestId('settings-providers'), 'parts-settings');
  await close();
});

// --- GitHub (M11) -------------------------------------------------------------

/** Closes the notices (they would cover the panels). */
async function clearNotices(page: Page): Promise<void> {
  const close = page.locator('.toast__close');
  while ((await close.count()) > 0) {
    await close.first().click();
  }
}

/** A repository with one commit (for the "Open from GitHub" list). */
function sampleRepo(github: FakeGithub, name: string, description: string, secret: boolean): void {
  const dir = join(PROJECTS, '..', 'Sources', name);
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, 'README.md'), `# ${name}\n`);
  const git = (...args: string[]): void => {
    execFileSync('git', args, { cwd: dir });
  };
  git('init', '--initial-branch=main');
  git('add', '.');
  git('-c', 'user.name=Octo Maker', '-c', 'user.email=octo@example.com', 'commit', '-m', 'Start');
  github.addRepo({ owner: 'octo-maker', name, private: secret, description }, dir);
}

test('github', async () => {
  test.setTimeout(240_000);
  freshProjects();
  const github = await startFakeGithub();
  try {
    const { page, close } = await start({
      settings: { theme: 'light' },
      env: {
        EW_GITHUB_API: github.api,
        EW_GITHUB_WEB: github.url,
        EW_GITHUB_CLIENT_ID: 'Iv1.docs-client',
      },
    });
    await page.setViewportSize({ width: 1400, height: 900 });

    // Sign in with the device flow.
    await page.getByTestId('open-settings').click();
    await page.getByTestId('github-sign-in-open').click();
    await page.getByTestId('github-device-start').click();
    await expect(page.getByTestId('github-device-code')).toBeVisible();
    await page.mouse.move(5, 5);
    await shoot(page.getByTestId('github-sign-in'), 'github-sign-in');
    github.approveDevice();
    await expect(page.getByTestId('github-sign-in')).toBeHidden();
    await expect(page.getByTestId('github-account')).toContainText('octo-maker');
    await shoot(page.getByTestId('settings-github'), 'github-settings');
    await page.keyboard.press('Escape');

    // A new project on GitHub.
    await page.getByTestId('new-project').click();
    await page.getByTestId('wizard-name').fill('Bench Supply');
    await page.getByTestId('wizard-storage-github').click();
    await page.getByTestId('repo-target-description').fill('Adjustable 0–30 V bench supply');
    await page.mouse.move(5, 5);
    await shoot(page.getByTestId('wizard'), 'wizard-github');
    await page.getByTestId('wizard-next').click();
    await page.getByTestId('template-demo').click();
    await page.getByTestId('wizard-create').click();
    await expect(page.getByText('The project is on GitHub: octo-maker/Bench-Supply')).toBeVisible();

    // Work, save, commit.
    await page.getByTestId('tree-project').click();
    const revision = page.getByTestId('project-revision');
    await revision.fill('B');
    await revision.press('Enter');
    await page.getByTestId('toolbar-save').click();
    await expect(page.getByTestId('dirty-indicator')).toHaveAttribute('data-dirty', 'false');
    const folder = join(PROJECTS, 'Bench Supply');
    writeFileSync(join(folder, 'NOTES.md'), '# Notes\n\nOrder 10 boards after the review.\n');
    // Coming back to the window reads the git state again.
    await page.evaluate(() => window.dispatchEvent(new Event('focus')));
    await page.getByTestId('toolbar-git').click();
    await expect(page.getByTestId('toolbar-git-count')).toHaveText('2');
    await page.getByTestId('git-message').fill('Revision B: notes for the order');
    await clearNotices(page);
    await page.mouse.move(5, 5);
    await shoot(page, 'github-source-control');
    await shoot(page.getByTestId('toolbar-git'), 'github-toolbar');
    await page.getByTestId('git-commit-push').click();
    await expect(page.getByTestId('git-no-changes')).toBeVisible();

    // Someone else changes the same file on GitHub: a conflict.
    const other = join(PROJECTS, '..', 'Other');
    rmSync(other, { recursive: true, force: true });
    mkdirSync(other, { recursive: true });
    const authed = `${github.url.replace('http://', `http://x-access-token:${github.token}@`)}/octo-maker/Bench-Supply.git`;
    const run = (cwd: string, ...args: string[]): Promise<void> =>
      new Promise((done, fail) => {
        const env = { ...process.env, GIT_TERMINAL_PROMPT: '0' };
        execFile('git', ['-c', 'credential.helper=', ...args], { cwd, env }, (error) => {
          if (error === null) {
            done();
          } else {
            fail(new Error(error.message));
          }
        });
      });
    await run(other, 'clone', authed, 'copy');
    writeFileSync(join(other, 'copy', 'NOTES.md'), '# Notes\n\nOrder 25 boards.\n');
    await run(
      join(other, 'copy'),
      '-c',
      'user.name=Ada',
      '-c',
      'user.email=ada@example.com',
      'commit',
      '-am',
      'More boards',
    );
    await run(join(other, 'copy'), 'push', 'origin', 'main');
    writeFileSync(join(folder, 'NOTES.md'), '# Notes\n\nOrder 12 boards after the review.\n');
    await page.evaluate(() => window.dispatchEvent(new Event('focus')));
    await page.getByTestId('git-message').fill('Two more boards');
    await page.getByTestId('git-commit').click();
    await page.getByTestId('git-fetch').click();
    await expect(page.getByTestId('git-sync')).toContainText('Commits to pull: 1');
    await page.getByTestId('git-pull').click();
    await expect(page.getByTestId('git-problem')).toBeVisible();
    await clearNotices(page);
    await page.mouse.move(5, 5);
    await shoot(page.getByTestId('source-control'), 'github-conflict');

    // A new branch.
    await page.getByTestId('git-branch').selectOption('::new');
    await page.getByTestId('git-new-branch-name').fill('rev-c');
    await shoot(page.getByTestId('git-new-branch'), 'github-new-branch');
    await page.getByTestId('toolbar-close').click();

    // Open from GitHub.
    sampleRepo(github, 'led-matrix', '8×8 LED matrix with a shift register driver', false);
    sampleRepo(github, 'sensor-node', 'Battery powered sensor node', true);
    await page.getByTestId('open-github').click();
    await page.locator('[data-repo="octo-maker/Bench-Supply"]').click();
    await page.mouse.move(5, 5);
    await shoot(page.getByTestId('github-clone'), 'github-clone');
    await page.keyboard.press('Escape');
    await close();
    rmSync(other, { recursive: true, force: true });
    rmSync(join(PROJECTS, '..', 'Sources'), { recursive: true, force: true });
  } finally {
    await github.close();
  }
});

// --- Scripts and AI agents (M12) ---------------------------------------------

/** The newest `ewerk` binary built by cargo. */
function ewerkBinary(): string {
  const target = resolve(import.meta.dirname, '../../../../target');
  const name = process.platform === 'win32' ? 'ewerk.exe' : 'ewerk';
  const found = ['release', 'debug']
    .map((profile) => join(target, profile, name))
    .filter((path) => existsSync(path))
    .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs)[0];
  if (found === undefined) {
    throw new Error('build the command-line tool first: cargo build -p ew-cli');
  }
  return found;
}

test('automation', async () => {
  test.setTimeout(240_000);
  freshProjects();
  // The cookbook's status LED, built with the command-line tool.
  const project = join(PROJECTS, 'Status LED');
  const ewerk = (...args: string[]): void => {
    execFileSync(ewerkBinary(), args, {
      env: { ...process.env, EW_LIBRARY_DIR: join(PROJECTS, '..', 'no-library') },
    });
  };
  ewerk('project', 'new', project, '--width', '30', '--height', '20');
  ewerk(
    'script',
    resolve(import.meta.dirname, '../../../../examples/automation/status-led.jsonl'),
    '--project',
    project,
  );

  const { app, page, close } = await start({ settings: { theme: 'light' } });
  await page.setViewportSize({ width: 1400, height: 860 });
  await page.getByTestId('open-settings').click();
  await shoot(page.getByTestId('settings-automation'), 'automation-settings');
  await page.keyboard.press('Escape');

  await queueDialogAnswer(app, join(project, 'Status LED.ewproj'));
  await page.getByTestId('open-project').click();
  await page.getByTestId('workspace').waitFor();
  const sheet = await openDocument(page, 'Root');
  // The three parts, in the middle of the view.
  const parts: Point = [55_880_000, 180_340_000];
  await zoomAt(page, parts, 6);
  const [x, y] = await toScreen(sheet, parts);
  const box = await sheet.boundingBox();
  await page.mouse.move(x, y);
  await page.mouse.down({ button: 'middle' });
  await page.mouse.move(
    (box?.x ?? 0) + (box?.width ?? 0) / 2,
    (box?.y ?? 0) + (box?.height ?? 0) / 2,
    {
      steps: 5,
    },
  );
  await page.mouse.up({ button: 'middle' });
  await settle(page);
  await page.mouse.move(5, 5);
  await shoot(sheet, 'automation-schematic');
  const board = await openDocument(page, 'Status LED');
  await page.mouse.move(5, 5);
  await shoot(board, 'automation-board');
  await close();
});
