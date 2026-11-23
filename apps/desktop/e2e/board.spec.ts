/**
 * The layout editor in the running app: the design rules check with the
 * online update, zone filling, design rules with a custom rule, board
 * presets, routing, zones, updating the board from the schematic and the
 * DXF import.
 */
import { cpSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

import { expect, test, type Page } from '@playwright/test';

import { APP_DIR, launchApp, queueDialogAnswer, type LaunchedApp } from './fixtures';
import { settle, toScreen, viewerCanvas, type Point } from './viewer-helpers';

const FIXTURE = resolve(APP_DIR, '../../crates/ew-kicad/tests/fixtures/blinky');
const MM = 1_000_000;

interface Board {
  footprints: {
    designator: string;
    at: Point;
    rotation?: number;
    side?: string;
    pads: { number: string; at: Point; net?: string }[];
  }[];
  tracks?: unknown[];
  vias?: unknown[];
  zones?: unknown[];
  shapes?: { layer: string; geometry: Record<string, unknown> }[];
}

let launched: LaunchedApp | undefined;

test.afterEach(async () => {
  await launched?.close();
  launched = undefined;
});

function readBoard(projectDir: string): Board {
  const dir = join(projectDir, 'pcb');
  const file = readdirSync(dir).find((name) => !name.startsWith('.'));
  return JSON.parse(readFileSync(join(dir, file ?? ''), 'utf8')) as Board;
}

async function importAs(launchedApp: LaunchedApp, name: string): Promise<void> {
  const { app, page } = launchedApp;
  await queueDialogAnswer(app, join(FIXTURE, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-name').fill(name);
  await page.getByTestId('kicad-import-start').click();
  const report = page.getByTestId('import-report');
  await expect(report).toBeVisible();
  await report.getByRole('button', { name: 'Close', exact: true }).last().click();
  await expect(page.getByTestId('workspace')).toBeVisible();
}

async function openBoard(page: Page, name: string): Promise<void> {
  await page.locator('[data-testid^="tree-doc:"]', { hasText: name }).first().dblclick();
  await settle(page);
  await expect(page.getByTestId('viewer')).toHaveAttribute('data-kind', 'pcb');
}

/** Two board points within 1 µm. */
function sameP(a: Point, b: Point): boolean {
  return Math.abs(a[0] - b[0]) < 1_000 && Math.abs(a[1] - b[1]) < 1_000;
}

/** Waits until the live route ends at a screen point (the router answered). */
async function previewReaches(page: Page, x: number, y: number): Promise<void> {
  const preview = page.getByTestId('route-preview');
  const canvasBox = await viewerCanvas(page).boundingBox();
  const [left, top] = [canvasBox?.x ?? 0, canvasBox?.y ?? 0];
  await expect
    .poll(async () => {
      const points = ((await preview.getAttribute('points')) ?? '').trim().split(/\s+/u);
      const [px, py] = (points.at(-1) ?? '').split(',').map(Number);
      return Math.hypot((px ?? 0) + left - x, (py ?? 0) + top - y);
    })
    .toBeLessThan(6);
}

async function save(page: Page): Promise<void> {
  await page.getByTestId('toolbar-save').click();
  await expect(page.getByTestId('dirty-indicator')).not.toHaveText('Unsaved changes');
}

test('checks, fills, rules and presets of a board', async () => {
  launched = await launchApp();
  const { page } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importAs(launched, 'Layout Checks');
  await openBoard(page, 'Layout Checks');
  await expect(page.getByTestId('board-tool-palette')).toBeVisible();
  await expect(page.getByTestId('board-layer')).toHaveValue('cu.top');

  // The design rules check runs and updates online.
  await page.getByTestId('board-drc').click();
  const panel = page.getByTestId('drc-panel');
  await expect(panel).toBeVisible();
  await expect(page.getByTestId('drc-summary')).toContainText('errors');

  // Filling the zones leaves no unfilled zone.
  await page.getByTestId('board-fill').click();
  await expect(page.getByTestId('drc-finding-zone-unfilled')).toHaveCount(0);

  // A custom rule: every track at least 5 mm wide.
  await page.getByTestId('board-rules').click();
  const rules = page.getByTestId('rules-dialog');
  await expect(rules).toBeVisible();
  await expect(page.getByTestId('class-name-0')).toHaveValue('Default');
  await page.getByTestId('rules-tab-custom').click();
  await page.getByTestId('rule-add').click();
  const condition = page.getByTestId('rule-condition-0');
  await condition.fill('A.net ==');
  await condition.press('Enter');
  await expect(page.getByTestId('rule-error-0')).toBeVisible();
  await condition.fill("A.type == 'track'");
  await condition.press('Enter');
  await expect(page.getByTestId('rule-error-0')).toHaveCount(0);
  await page.getByTestId('rule-trackWidthMin-0').check();
  const width = page.getByTestId('rule-value-trackWidthMin-0');
  await width.fill('5');
  await width.press('Enter');
  await page.getByTestId('rules-tab-checks').click();
  await page.getByTestId('check-dangling-via').selectOption('ignore');
  await page.getByTestId('rules-save').click();
  await expect(rules).toBeHidden();
  await expect(page.getByTestId('drc-finding-track-width').first()).toBeVisible();
  await expect(page.getByTestId('drc-finding-track-width').first()).toContainText('rule Rule 1');
  await expect(page.getByTestId('drc-finding-dangling-via')).toHaveCount(0);

  // Accepting a finding hides it from the list.
  const before = await page.getByTestId('drc-finding-track-width').count();
  await page.getByTestId('drc-exclude-track-width').first().click();
  await expect(page.getByTestId('drc-finding-track-width')).toHaveCount(before - 1);
  await page.getByTestId('drc-show-excluded').check();
  await expect(page.getByTestId('drc-finding-track-width')).toHaveCount(before);

  // A preset turns the four-layer board into a two-layer one.
  await page.getByTestId('board-setup').click();
  await expect(page.getByTestId('setup-dialog')).toBeVisible();
  await page.getByTestId('preset-standard-2l').click();
  await expect(page.getByTestId('setup-dialog')).toBeHidden();
  await expect(page.getByTestId('board-layer').locator('option')).toHaveCount(2);
  await page.getByTestId('board-setup').click();
  await page.getByTestId('setup-tab-stackup').click();
  await expect(page.getByTestId('stackup-count')).toHaveValue('2');
  await page.getByTestId('setup-dialog').getByRole('button', { name: 'Cancel' }).click();
  await page.getByTestId('toolbar-undo').click();
  await expect(page.getByTestId('board-layer').locator('option')).toHaveCount(4);
});

test('routes with a blind via once the design rules allow it', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importAs(launched, 'Blind Vias');
  const project = join(projectsDir, 'Blind Vias');
  const board = readBoard(project);
  await openBoard(page, 'Blind Vias');
  const canvas = viewerCanvas(page);

  // Blind and buried vias are off until the design rules allow them.
  await page.getByTestId('board-rules').click();
  const rules = page.getByTestId('rules-dialog');
  await expect(rules).toBeVisible();
  await page.getByTestId('rules-tab-constraints').click();
  await expect(page.getByTestId('constraint-allowBlindBuriedVias')).not.toBeChecked();
  await page.getByTestId('constraint-allowBlindBuriedVias').check();
  await page.getByTestId('rules-save').click();
  await expect(rules).toBeHidden();
  await page.getByTestId('board-via-type').selectOption('blind');

  // Route from a top pad, change to the first inner layer and go on there.
  const fp = board.footprints.find(
    (f) => (f.side ?? 'top') === 'top' && !f.rotation && f.pads.some((p) => p.net !== undefined),
  );
  const pad = fp?.pads.find((p) => p.net !== undefined);
  const start: Point = [(fp?.at[0] ?? 0) + (pad?.at[0] ?? 0), (fp?.at[1] ?? 0) + (pad?.at[1] ?? 0)];
  await canvas.focus();
  await page.keyboard.press('x');
  const [sx, sy] = await toScreen(canvas, start);
  await page.mouse.move(sx, sy);
  await page.mouse.click(sx, sy);
  await expect(page.getByTestId('route-status')).toBeVisible();
  const corner: Point = [start[0] + 2.54 * MM, start[1] - 2.54 * MM];
  const [cx, cy] = await toScreen(canvas, corner);
  await page.mouse.move(cx, cy, { steps: 5 });
  await previewReaches(page, cx, cy);
  await page.mouse.click(cx, cy);
  await page.getByTestId('board-layer').selectOption('cu.in1');
  await expect(page.getByTestId('board-layer')).toHaveValue('cu.in1');
  const end: Point = [corner[0] + 2.54 * MM, corner[1]];
  const [ex, ey] = await toScreen(canvas, end);
  await page.mouse.move(ex, ey, { steps: 5 });
  await previewReaches(page, ex, ey);
  await page.mouse.click(ex, ey);
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('route-status')).toHaveCount(0);
  await save(page);

  const vias = (readBoard(project).vias ?? []) as { from?: string; to?: string }[];
  const blind = vias.filter((v) => (v.from ?? 'cu.top') === 'cu.top' && v.to === 'cu.in1');
  expect(blind).toHaveLength(1);
});

test('computes impedance sizes in the beginner mode and takes them by hand in the advanced mode', async () => {
  launched = await launchApp();
  const { page } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importAs(launched, 'Impedance');
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Impedance' }).first().dblclick();
  await settle(page);

  // A net class for the USB data lines.
  await page.getByTestId('board-rules').click();
  const rules = page.getByTestId('rules-dialog');
  await expect(rules).toBeVisible();
  await expect(page.getByTestId('class-name-0')).toHaveValue('Default');
  await page.getByTestId('class-add').click();
  const usb = (await page.locator('[data-testid^="class-name-"]').count()) - 1;
  const name = page.getByTestId(`class-name-${usb.toString()}`);
  await name.fill('USB');
  await name.press('Enter');

  // Beginner (the default): choose the interface, the sizes are computed.
  await page.getByTestId('rules-tab-impedance').click();
  await expect(page.getByTestId('impedance-beginner')).toBeVisible();
  await page.getByTestId(`impedance-class-${usb.toString()}`).selectOption('usb2');
  const layers = page.getByTestId('impedance-layers-0');
  await expect(layers).toBeVisible();
  await expect(page.getByTestId('impedance-status-0-0')).toHaveText('Computed');
  await expect(page.getByTestId('impedance-value-0-0')).toContainText('Ω');
  await expect(layers.locator('input')).toHaveCount(0);
  const top = (await page.getByTestId('impedance-width-0-0').textContent()) ?? '';
  const gap = (await page.getByTestId('impedance-gap-0-0').textContent()) ?? '';
  // An inner layer between two planes needs another width.
  await expect(page.getByTestId('impedance-width-0-1')).not.toHaveText(top);

  // The class takes the top layer's sizes.
  await page.getByTestId('rules-tab-classes').click();
  await expect(page.getByTestId(`class-diffPairWidth-${usb.toString()}`)).toHaveValue(
    top.replace(' mm', ''),
  );
  await expect(page.getByTestId(`class-diffPairGap-${usb.toString()}`)).toHaveValue(
    gap.replace(' mm', ''),
  );
  await page.getByTestId('rules-save').click();
  await expect(rules).toBeHidden();

  // Advanced: the profile, its target and the sizes per layer are editable.
  await page.getByTestId('open-settings').click();
  await page.getByTestId('setting-work-mode').selectOption('advanced');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('settings-dialog')).toBeHidden();
  await page.getByTestId('board-rules').click();
  await page.getByTestId('rules-tab-impedance').click();
  await expect(page.getByTestId('impedance-advanced')).toBeVisible();
  await expect(page.getByTestId('profile-name-0')).toHaveValue('USB 2.0');
  await expect(page.getByTestId('profile-target-0')).toHaveValue('90');
  const width = page.getByTestId('impedance-width-0-0');
  await width.fill('0.5');
  await width.press('Enter');
  await expect(page.getByTestId('impedance-status-0-0')).toHaveText('Outside the tolerance');
  await page.getByTestId('impedance-reset-0-0').click();
  await expect(page.getByTestId('impedance-status-0-0')).toHaveText('Computed');
  await expect(page.getByTestId('impedance-width-0-0')).toHaveValue(top.replace(' mm', ''));

  // The calculator.
  const calc = page.getByTestId('calc-width');
  await calc.fill('0.3');
  await calc.press('Enter');
  await expect(page.getByTestId('calc-result')).toContainText('Ω');
  await rules.getByRole('button', { name: 'Cancel' }).click();
});

test('routes a differential pair with both tracks together', async () => {
  launched = await launchApp();
  const { app, page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  // The blinky board with /TRIG and /OUT renamed to a pair: they end on
  // the neighbouring pins 2 and 3 of U1.
  const copy = mkdtempSync(join(tmpdir(), 'ew-pair-'));
  cpSync(FIXTURE, copy, { recursive: true });
  const pcbFile = join(copy, 'blinky.kicad_pcb');
  writeFileSync(
    pcbFile,
    readFileSync(pcbFile, 'utf8').replaceAll('"/TRIG"', '"SIG_P"').replaceAll('"/OUT"', '"SIG_N"'),
  );
  await queueDialogAnswer(app, join(copy, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-name').fill('Pair');
  await page.getByTestId('kicad-import-start').click();
  await page
    .getByTestId('import-report')
    .getByRole('button', { name: 'Close', exact: true })
    .last()
    .click();
  const project = join(projectsDir, 'Pair');
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Pair' }).first().dblclick();
  await settle(page);
  const canvas = viewerCanvas(page);
  interface Track {
    start: Point;
    end: Point;
    net?: string;
  }
  const board = readBoard(project) as Board & { nets: { id: string; name: string }[] };
  const netId = (name: string): string => board.nets.find((n) => n.name === name)?.id ?? '';
  const before = (board.tracks ?? []) as Track[];

  const u1 = board.footprints.find((f) => f.designator === 'U1');
  const padAt = (number: string): Point => {
    const pad = u1?.pads.find((p) => p.number === number);
    return [(u1?.at[0] ?? 0) + (pad?.at[0] ?? 0), (u1?.at[1] ?? 0) + (pad?.at[1] ?? 0)];
  };

  // Not on a pair (pin 7, /DIS): a hint.
  await canvas.focus();
  await page.keyboard.press('6');
  const [dx, dy] = await toScreen(canvas, padAt('7'));
  await page.mouse.click(dx, dy);
  await expect(page.getByTestId('editor-hint')).toContainText('No differential pair here');

  // From U1 pin 2 (SIG_P) to free space on the left: both tracks follow.
  const start = padAt('2');
  const [sx, sy] = await toScreen(canvas, start);
  await page.mouse.click(sx, sy);
  await expect(page.getByTestId('route-status')).toContainText('Routing the pair SIG_P / SIG_N');
  const [tx, ty] = await toScreen(canvas, [112.5 * MM, start[1] - 1.25 * MM]);
  await page.mouse.move(tx, ty, { steps: 6 });
  await expect
    .poll(async () => {
      const points = (await page.getByTestId('pair-preview').getAttribute('points')) ?? '';
      return points.trim().split(/\s+/u).length;
    })
    .toBeGreaterThan(2);
  await page.mouse.click(tx, ty);
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Route/u);
  await save(page);

  const added = ((readBoard(project).tracks ?? []) as Track[]).filter(
    (t) => !before.some((b) => sameP(b.start, t.start) && sameP(b.end, t.end)),
  );
  const mine = added.filter((t) => t.net === netId('SIG_P'));
  const theirs = added.filter((t) => t.net === netId('SIG_N'));
  expect(mine.length).toBeGreaterThan(1);
  expect(theirs.length).toBe(mine.length);
  // The coupled horizontal parts run side by side.
  const level = (tracks: Track[]): number[] =>
    tracks.filter((t) => t.start[1] === t.end[1]).map((t) => t.start[1]);
  const gap = Math.min(...level(mine).flatMap((a) => level(theirs).map((b) => Math.abs(a - b))));
  expect(gap).toBeGreaterThan(0.3 * MM);
  expect(gap).toBeLessThan(1 * MM);
});

test('reports the skew of a pair and matches it with meanders', async () => {
  launched = await launchApp();
  const { app, page } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  // The blinky board with /TRIG and /OUT renamed to a pair of different lengths.
  const copy = mkdtempSync(join(tmpdir(), 'ew-skew-'));
  cpSync(FIXTURE, copy, { recursive: true });
  const pcbFile = join(copy, 'blinky.kicad_pcb');
  writeFileSync(
    pcbFile,
    readFileSync(pcbFile, 'utf8').replaceAll('"/TRIG"', '"SIG_P"').replaceAll('"/OUT"', '"SIG_N"'),
  );
  await queueDialogAnswer(app, join(copy, 'blinky.kicad_pro'));
  await page.getByTestId('import-kicad').click();
  await page.getByTestId('kicad-import-name').fill('Skew');
  await page.getByTestId('kicad-import-start').click();
  await page
    .getByTestId('import-report')
    .getByRole('button', { name: 'Close', exact: true })
    .last()
    .click();
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Skew' }).first().dblclick();
  await settle(page);

  // At most 0.1 mm skew for the Default class.
  await page.getByTestId('board-rules').click();
  const rules = page.getByTestId('rules-dialog');
  await expect(page.getByTestId('class-name-0')).toHaveValue('Default');
  await page.getByTestId('rules-tab-lengths').click();
  const skew = page.getByTestId('class-maxSkew-0');
  await skew.fill('0.1');
  await skew.press('Enter');
  await page.getByTestId('group-add').click();
  await expect(page.getByTestId('group-name-0')).toHaveValue('Group 1');
  await page.getByTestId('group-remove-0').click();
  await page.getByTestId('rules-save').click();
  await expect(rules).toBeHidden();

  // The lengths: too much skew, then matched with meanders.
  await page.getByTestId('board-lengths').click();
  const dialog = page.getByTestId('lengths-dialog');
  await expect(dialog).toBeVisible();
  await expect(page.getByTestId('lengths-pair-0')).toContainText('SIG_P');
  await expect(page.getByTestId('lengths-pair-status-0')).toHaveText('Too much skew');
  await page.getByTestId('lengths-match').click();
  await expect(page.getByTestId('lengths-pair-status-0')).toHaveText('OK');
  await expect(page.getByTestId('lengths-match')).toBeDisabled();
  await dialog.getByRole('button', { name: 'Close', exact: true }).last().click();
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Match lengths/u);
});

test('adds and removes teardrops', async () => {
  launched = await launchApp();
  const { page } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importAs(launched, 'Teardrops');
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Teardrops' }).first().dblclick();
  await settle(page);

  await page.getByTestId('board-teardrops').click();
  const dialog = page.getByTestId('teardrop-dialog');
  await expect(dialog).toBeVisible();
  // The beginner mode shows no settings.
  await expect(page.getByTestId('teardrop-settings')).toHaveCount(0);
  await page.getByTestId('teardrops-add').click();
  await expect(dialog).toBeHidden();
  await expect(page.getByText(/teardrops added/u)).toBeVisible();
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Add teardrops/u);

  // The advanced mode shows the shape.
  await page.getByTestId('open-settings').click();
  await page.getByTestId('setting-work-mode').selectOption('advanced');
  await page.keyboard.press('Escape');
  await page.getByTestId('board-teardrops').click();
  await expect(page.getByTestId('teardrop-settings')).toBeVisible();
  await page.getByTestId('teardrops-curved').uncheck();
  await page.getByTestId('teardrops-remove').click();
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Remove teardrops/u);
});

test('fences a selected track with ground vias', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importAs(launched, 'Fence');
  await page.locator('[data-testid^="tree-doc:"]', { hasText: 'Fence' }).first().dblclick();
  await settle(page);
  const board = readBoard(join(projectsDir, 'Fence')) as unknown as {
    nets: { id: string; name: string }[];
    tracks: { net?: string; layer: string; start: Point; end: Point }[];
    vias: unknown[];
  };
  const out = board.nets.find((n) => n.name === '/OUT')?.id;
  const track = board.tracks.find(
    (t) => t.net === out && t.layer === 'cu.top' && t.start[0] === t.end[0],
  );
  expect(track).toBeDefined();
  const canvas = viewerCanvas(page);
  const middle: Point = [
    ((track?.start[0] ?? 0) + (track?.end[0] ?? 0)) / 2,
    ((track?.start[1] ?? 0) + (track?.end[1] ?? 0)) / 2,
  ];
  const [x, y] = await toScreen(canvas, middle);
  await page.mouse.click(x, y);

  await page.getByTestId('board-stitch').click();
  const dialog = page.getByTestId('stitch-dialog');
  await expect(dialog).toBeVisible();
  await expect(page.getByTestId('stitch-mode-fence')).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByTestId('stitch-tracks')).toHaveText('1 tracks selected');
  await expect(page.getByTestId('stitch-net').locator('option:checked')).toHaveText('GND');
  await page.getByTestId('stitch-add').click();
  await expect(dialog).toBeHidden();
  await expect(page.getByText(/vias added/u)).toBeVisible();
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Add stitching vias/u);
  await save(page);
  const after = readBoard(join(projectsDir, 'Fence')) as { vias?: unknown[] };
  expect((after.vias ?? []).length).toBeGreaterThan(board.vias.length);
});

test('selects similar tracks and changes their width together', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importAs(launched, 'Similar');
  await openBoard(page, 'Similar');
  const board = readBoard(join(projectsDir, 'Similar')) as unknown as {
    tracks: { id: string; layer: string; width: number; start: Point; end: Point }[];
  };
  // The longest track on top: its middle is free of pads.
  const length = (t: { start: Point; end: Point }): number =>
    Math.hypot(t.end[0] - t.start[0], t.end[1] - t.start[1]);
  const track = board.tracks
    .filter((t) => t.layer === 'cu.top')
    .sort((a, b) => length(b) - length(a))[0];
  expect(track).toBeDefined();
  const alike = board.tracks.filter(
    (t) => t.layer === track?.layer && t.width === track.width,
  ).length;
  expect(alike).toBeGreaterThan(1);
  const canvas = viewerCanvas(page);
  const middle: Point = [
    ((track?.start[0] ?? 0) + (track?.end[0] ?? 0)) / 2,
    ((track?.start[1] ?? 0) + (track?.end[1] ?? 0)) / 2,
  ];
  const [x, y] = await toScreen(canvas, middle);
  await page.mouse.click(x, y);
  await expect(page.getByTestId('inspector-title')).toContainText('Track');

  // Same layer and width: the defaults.
  await page.getByTestId('select-similar').click();
  await expect(page.getByTestId('similar-layer')).toBeChecked();
  await expect(page.getByTestId('similar-width')).toBeChecked();
  await page.getByTestId('similar-select').click();
  await expect(page.getByTestId('inspector-title')).toHaveText(`${String(alike)} objects selected`);

  // One width for all of them, in one step.
  const width = page.getByTestId('board-prop-width');
  await width.fill('0.4');
  await width.press('Enter');
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Change properties/u);
  await save(page);
  const after = readBoard(join(projectsDir, 'Similar')) as unknown as {
    tracks: { layer: string; width: number }[];
  };
  expect(after.tracks.filter((t) => t.width === 400_000).length).toBe(alike);

  // The net inspector: every named net, filtered, sorted, a class for some.
  const named = (
    readBoard(join(projectsDir, 'Similar')) as unknown as {
      nets: { name: string }[];
    }
  ).nets.filter((n) => n.name !== '');
  await page.getByTestId('board-nets').click();
  const rows = page.getByTestId('net-row');
  await expect(rows).toHaveCount(named.length);
  await page.getByTestId('net-filter').fill('GND');
  await expect(rows.first()).toHaveAttribute('data-name', /GND/u);
  await page.getByTestId('net-filter').fill('');
  await page.getByTestId('net-sort-length').click();
  await expect(page.getByTestId('net-sort-length')).toContainText('↓');
  await rows.first().click();
  await expect(page.getByTestId('net-chip')).toBeVisible();
  await rows.first().locator('input[type="checkbox"]').check();
  await page.getByTestId('net-set-class').selectOption({ index: 1 });
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Change properties/u);

  // A colour for the first net: its dot in the table, saved in the project.
  const first = rows.first();
  const colored = String(await first.getAttribute('data-name'));
  await first.locator('input[type="checkbox"]').check();
  await page.getByTestId('net-tick-color-1e88e5').click();
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute(
    'title',
    /Change project properties/u,
  );
  await expect(first.locator('.net-inspector__dot')).toHaveAttribute('data-color', '#1e88e5');
  await expect(page.getByTestId('net-colors-shown')).toBeChecked();
  if (process.env['EW_LOOK'] !== undefined) {
    await page.screenshot({ path: process.env['EW_LOOK'] });
    await first.click();
    const box = await canvas.boundingBox();
    await page.mouse.click((box?.x ?? 0) + (box?.width ?? 0) - 12, (box?.y ?? 0) + 12);
    await page.screenshot({ path: process.env['EW_LOOK'].replace('.png', '-2.png') });
  }
  await save(page);
  const manifest = JSON.parse(
    readFileSync(join(projectsDir, 'Similar', 'Similar.ewproj'), 'utf8'),
  ) as { settings: { netColors?: Record<string, string> } };
  expect(manifest.settings.netColors).toEqual({ [colored]: '#1e88e5' });
  await page.getByTestId('net-inspector-close').click();
  await expect(page.getByTestId('net-inspector')).toHaveCount(0);
});

test('drags a track while its 45° neighbours keep their direction', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importAs(launched, 'Drag');
  const project = join(projectsDir, 'Drag');
  const board = readBoard(project);
  await openBoard(page, 'Drag');
  const canvas = viewerCanvas(page);

  // A route from R3: diagonal, then straight on.
  const r3 = board.footprints.find((f) => f.designator === 'R3');
  const pad = r3?.pads.find((p) => p.number === '2');
  const origin: Point = [
    (r3?.at[0] ?? 0) + (pad?.at[0] ?? 0),
    (r3?.at[1] ?? 0) + (pad?.at[1] ?? 0),
  ];
  const corner: Point = [origin[0] + 2.54 * MM, origin[1] - 2.54 * MM];
  const end: Point = [corner[0] + 2.54 * MM, corner[1]];
  await canvas.focus();
  await page.keyboard.press('x');
  const [ox, oy] = await toScreen(canvas, origin);
  await page.mouse.move(ox, oy);
  await page.mouse.click(ox, oy);
  const [cx, cy] = await toScreen(canvas, corner);
  await page.mouse.move(cx, cy, { steps: 5 });
  await previewReaches(page, cx, cy);
  await page.mouse.click(cx, cy);
  const [ex, ey] = await toScreen(canvas, end);
  await page.mouse.move(ex, ey, { steps: 5 });
  await previewReaches(page, ex, ey);
  await page.mouse.click(ex, ey);
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('route-status')).toHaveCount(0);
  // Still routing: the hint says how to get back to the select tool.
  await expect(page.getByTestId('editor-hint')).toHaveText(/Esc again: back to the select tool/u);
  await expect(page.getByTestId('editor-hint')).toHaveClass(/viewer__hint--next/u);
  // The route is in the board (and selected) once the view has reloaded.
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Route/u);
  await settle(page);
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('board-tool-select')).toHaveAttribute('aria-pressed', 'true');
  // Escape ends the net highlight and then the selection of the new route.
  await page.keyboard.press('Escape');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('inspector')).not.toContainText('objects selected');

  // The straight part and the diagonal before it, as saved.
  await save(page);
  interface Track {
    id: string;
    start: Point;
    end: Point;
  }
  const before = (readBoard(project).tracks ?? []) as Track[];
  const known = new Set(((board.tracks ?? []) as Track[]).map((t) => t.id));
  const added = before.filter((t) => !known.has(t.id));
  // A straight track with a 45° diagonal in front of its left end.
  const isDiagonal = (t: Track): boolean =>
    Math.abs(Math.abs(t.end[0] - t.start[0]) - Math.abs(t.end[1] - t.start[1])) < 2 &&
    t.end[0] !== t.start[0];
  const leftOf = (t: Track): Point => (t.start[0] < t.end[0] ? t.start : t.end);
  let straight: Track | undefined;
  let diagonal: Track | undefined;
  for (const t of added.filter((x) => x.start[1] === x.end[1] && x.start[0] !== x.end[0])) {
    const left = leftOf(t);
    const before = added.find(
      (d) => d.id !== t.id && isDiagonal(d) && (sameP(d.start, left) || sameP(d.end, left)),
    );
    if (before !== undefined) {
      straight = t;
      diagonal = before;
    }
  }
  if (straight === undefined || diagonal === undefined) {
    throw new Error(`no diagonal and straight track in the new route: ${JSON.stringify(added)}`);
  }

  // Drag the straight part down; the diagonal follows along its own line (a
  // preview shows it during the drag).
  const middle: Point = [(straight.start[0] + straight.end[0]) / 2, straight.start[1]];
  const [mx, my] = await toScreen(canvas, middle);
  const [tx, ty] = await toScreen(canvas, [middle[0], middle[1] - 1.25 * MM]);
  await page.mouse.move(mx, my);
  await page.mouse.down();
  await page.mouse.move(tx, ty, { steps: 6 });
  // A horizontal preview line has no height, so count instead of "visible".
  await expect(page.getByTestId('edit-preview')).not.toHaveCount(0);
  await page.mouse.up();
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /drag/iu);
  await save(page);

  const after = new Map(((readBoard(project).tracks ?? []) as Track[]).map((t) => [t.id, t]));
  const moved = after.get(straight.id);
  const follower = after.get(diagonal.id);
  expect(moved?.start[1]).toBe(straight.start[1] - 1.25 * MM);
  expect(moved?.end[1]).toBe(straight.start[1] - 1.25 * MM);
  // The diagonal still runs at 45° and ends where the straight part starts.
  const origin0: Point = [0, 0];
  const a: Point = follower?.start ?? origin0;
  const b: Point = follower?.end ?? origin0;
  expect(Math.abs(Math.abs(b[0] - a[0]) - Math.abs(b[1] - a[1]))).toBeLessThan(2);
  const newLeft = (moved?.start[0] ?? 0) < (moved?.end[0] ?? 0) ? moved?.start : moved?.end;
  expect(sameP(a, newLeft ?? origin0) || sameP(b, newLeft ?? origin0)).toBe(true);
});

test('groups parts so that one click selects them all', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importAs(launched, 'Grouped');
  await openBoard(page, 'Grouped');
  const board = readBoard(join(projectsDir, 'Grouped'));
  const part = (name: string): Board['footprints'][number] => {
    const found = board.footprints.find((fp) => fp.designator === name);
    if (found === undefined) {
      throw new Error(`no part ${name}`);
    }
    return found;
  };
  const canvas = viewerCanvas(page);
  const clearSelection = async (): Promise<void> => {
    const box = await canvas.boundingBox();
    await page.mouse.click((box?.x ?? 0) + (box?.width ?? 0) - 12, (box?.y ?? 0) + 12);
  };
  const [x1, y1] = await toScreen(canvas, part('R1').at);
  const [x2, y2] = await toScreen(canvas, part('R2').at);
  await page.mouse.click(x1, y1);
  await page.keyboard.down('Shift');
  await page.mouse.click(x2, y2);
  await page.keyboard.up('Shift');
  await expect(page.getByTestId('inspector-title')).toHaveText('2 objects selected');
  const version = String(await canvas.getAttribute('data-version'));
  await page.keyboard.press('Control+g');
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Group/u);
  await expect(canvas).not.toHaveAttribute('data-version', version);

  // A click on one of them selects the group.
  await clearSelection();
  await expect(page.getByTestId('inspector-title')).toBeHidden();
  await page.mouse.click(x1, y1);
  await expect(page.getByTestId('inspector-title')).toHaveText('2 objects selected');
  await save(page);
  const grouped = readBoard(join(projectsDir, 'Grouped')) as unknown as {
    groups?: { members: string[] }[];
  };
  expect(grouped.groups?.length).toBe(1);
  expect(grouped.groups?.[0]?.members.length).toBe(2);

  // Ungroup: a click selects one part again.
  const grouped2 = String(await canvas.getAttribute('data-version'));
  await page.getByTestId('board-ungroup').click();
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Ungroup/u);
  await expect(canvas).not.toHaveAttribute('data-version', grouped2);
  await save(page);
  const ungrouped = readBoard(join(projectsDir, 'Grouped')) as unknown as { groups?: unknown[] };
  expect(ungrouped.groups).toBeUndefined();
  await clearSelection();
  await page.mouse.click(x1, y1);
  await expect(page.getByTestId('inspector-title')).toContainText('R1');
});

test('editor keys can be changed in the settings', async () => {
  launched = await launchApp();
  const { page } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importAs(launched, 'Keys');
  await openBoard(page, 'Keys');
  const route = page.getByTestId('board-tool-route');
  await expect(route).toHaveAttribute('title', /\(X\)/u);

  await page.getByTestId('open-settings').click();
  await page.getByTestId('shortcut-settings').locator('summary').click();
  const key = page.getByTestId('shortcut-key-board.route');
  await expect(key).toHaveText('X');
  // A key of another command is refused.
  await key.click();
  await expect(key).toHaveText('Press a key …');
  await page.keyboard.press('v');
  await expect(page.getByTestId('shortcut-message')).toContainText('Place a via');
  await expect(key).toHaveText('X');
  // A reserved key too.
  await key.click();
  await page.keyboard.press('ControlOrMeta+z');
  await expect(page.getByTestId('shortcut-message')).toContainText('reserved');
  // W is free in the layout editor.
  await key.click();
  await page.keyboard.press('w');
  await expect(key).toHaveText('W');
  await expect(page.getByTestId('shortcut-settings').locator('summary')).toContainText('1 changed');
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('settings-dialog')).toBeHidden();

  // The palette shows the new key, and the key picks the tool.
  await expect(route).toHaveAttribute('title', /\(W\)/u);
  const canvas = viewerCanvas(page);
  await canvas.focus();
  await page.keyboard.press('x');
  await expect(route).toHaveAttribute('aria-pressed', 'false');
  await page.keyboard.press('w');
  await expect(route).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('Escape');

  // Back to the defaults.
  await page.getByTestId('open-settings').click();
  await page.getByTestId('shortcut-settings').locator('summary').click();
  await page.getByTestId('shortcuts-reset-all').click();
  await expect(page.getByTestId('shortcut-key-board.route')).toHaveText('X');
  await page.keyboard.press('Escape');
  await expect(route).toHaveAttribute('title', /\(X\)/u);
});

test('puts a logo and a QR code onto the silkscreen', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importAs(launched, 'Logo');
  await openBoard(page, 'Logo');
  const before = (readBoard(join(projectsDir, 'Logo')).shapes ?? []).length;
  // A ring with a bar: one region with a hole.
  const folder = mkdtempSync(join(tmpdir(), 'ew-logo-'));
  const logo = join(folder, 'logo.svg');
  writeFileSync(
    logo,
    '<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 200 100">' +
      '<rect width="200" height="100" fill="white"/>' +
      '<circle cx="50" cy="50" r="40" fill="none" stroke="black" stroke-width="16"/>' +
      '<rect x="110" y="20" width="80" height="60" fill="black"/></svg>',
  );

  await page.getByTestId('board-picture').click();
  const dialog = page.getByTestId('picture-dialog');
  await expect(dialog).toBeVisible();
  await expect(dialog.getByTestId('picture-add')).toBeDisabled();
  await dialog.getByTestId('picture-file').setInputFiles(logo);
  const preview = dialog.getByTestId('picture-preview');
  await expect(preview).toBeVisible();
  // 15 mm at 0.05 mm per pixel: 300 × 150 pixels.
  await expect(preview).toHaveAttribute('data-columns', '300');
  await expect(preview).toHaveAttribute('data-rows', '150');
  await expect(dialog.getByTestId('picture-size')).toContainText('15 × 7.5 mm');
  await dialog.getByTestId('picture-layer').selectOption('silk.bottom');
  await expect(dialog.getByText(/mirrored/u)).toBeVisible();
  await dialog.getByTestId('picture-layer').selectOption('silk.top');
  if (process.env['EW_LOOK'] !== undefined) {
    await page.screenshot({ path: process.env['EW_LOOK'] });
  }
  await dialog.getByTestId('picture-add').click();
  await expect(dialog).toBeHidden();
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Add picture/u);

  // A QR code next to it.
  await page.getByTestId('board-picture').click();
  await dialog.getByTestId('picture-tab-qr').click();
  await dialog.getByTestId('picture-text').fill('https://example.com/board');
  await expect(preview).toBeVisible();
  await dialog.getByTestId('picture-width').fill('10');
  const shown = String(await viewerCanvas(page).getAttribute('data-version'));
  await dialog.getByTestId('picture-add').click();
  await expect(viewerCanvas(page)).not.toHaveAttribute('data-version', shown);
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Add QR code/u);
  if (process.env['EW_LOOK'] !== undefined) {
    await page.screenshot({ path: process.env['EW_LOOK'].replace('.png', '-board.png') });
  }
  await save(page);
  const after = readBoard(join(projectsDir, 'Logo')) as unknown as {
    shapes: { layer: string; filled?: boolean }[];
    groups?: { members: string[] }[];
  };
  const added = after.shapes.length - before;
  // The logo is two regions (ring, bar); the QR code many.
  expect(added).toBeGreaterThan(10);
  expect(
    after.shapes.filter((shape) => shape.layer === 'silk.top' && shape.filled === true).length,
  ).toBe(added);
  expect(after.groups?.length).toBe(2);
});

test('measures the board with dimensions and a leader', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importAs(launched, 'Measured');
  await openBoard(page, 'Measured');
  const board = readBoard(join(projectsDir, 'Measured'));
  const at = (name: string): Point => {
    const found = board.footprints.find((fp) => fp.designator === name);
    if (found === undefined) {
      throw new Error(`no part ${name}`);
    }
    return found.at;
  };
  const canvas = viewerCanvas(page);
  const click = async (point: Point): Promise<void> => {
    const [x, y] = await toScreen(canvas, point);
    await page.mouse.move(x, y);
    await page.mouse.click(x, y);
  };
  // Horizontal: from R1 to D1, the line 5 mm above.
  await page.getByTestId('board-tool-dimension').click();
  await page.getByTestId('board-dimension-kind').selectOption('horizontal');
  const r1 = at('R1');
  const d1 = at('D1');
  await click(r1);
  await click(d1);
  const [px, py] = await toScreen(canvas, [d1[0], r1[1] + 5 * MM]);
  await page.mouse.move(px, py);
  await expect(page.getByTestId('dimension-preview')).toBeVisible();
  await page.mouse.click(px, py);
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Add dimension/u);

  // A leader with a note.
  await page.getByTestId('board-dimension-kind').selectOption('leader');
  await click(at('U1'));
  await click([at('U1')[0] + 6 * MM, at('U1')[1] + 6 * MM]);
  await page.getByTestId('board-prompt-input').fill('Timer');
  await page.getByTestId('board-prompt-input').press('Enter');
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Add dimension/u);
  await page.keyboard.press('Escape');
  await save(page);
  const saved = readBoard(join(projectsDir, 'Measured')) as unknown as {
    dimensions: { kind: string; text?: string; offset?: number; start: Point; end: Point }[];
  };
  // Blinky comes with one dimension of its own.
  expect(saved.dimensions.filter((d) => d.kind === 'leader')).toHaveLength(1);
  const horizontal = saved.dimensions.find(
    (d) => d.kind === 'horizontal' && d.start[0] === r1[0] && d.start[1] === r1[1],
  );
  expect(horizontal?.start).toEqual(r1);
  expect(Math.abs((horizontal?.offset ?? 0) - 5 * MM)).toBeLessThanOrEqual(MM);
  expect(saved.dimensions.find((d) => d.kind === 'leader')?.text).toBe('Timer');
});

test('pushes a track of another net aside in push-and-shove mode', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importAs(launched, 'Shove');
  const project = join(projectsDir, 'Shove');
  await openBoard(page, 'Shove');
  const canvas = viewerCanvas(page);
  interface Track {
    id: string;
    start: Point;
    end: Point;
    net?: string;
  }
  const before = (readBoard(project).tracks ?? []) as Track[];
  // The vertical part of /OUT, which ends on R3 pad 1.
  const out = before.find(
    (t) => t.start[0] === t.end[0] && Math.abs(t.start[0] - 129_087_500) < 1_000,
  );
  expect(out).toBeDefined();

  await page.getByTestId('board-route-mode').selectOption('shove');
  await canvas.focus();
  await page.keyboard.press('x');
  // From free space on the right across the track (points on the 1.25 mm
  // grid): the track bends around the end of the new one.
  const from: Point = [132.5 * MM, 100 * MM];
  const to: Point = [128.75 * MM, 100 * MM];
  const [fx, fy] = await toScreen(canvas, from);
  await page.mouse.move(fx, fy);
  await page.mouse.click(fx, fy);
  await expect(page.getByTestId('route-status')).toBeVisible();
  const [tx, ty] = await toScreen(canvas, to);
  await page.mouse.move(tx, ty, { steps: 6 });
  await previewReaches(page, tx, ty);
  // The pushed track is drawn in its new place.
  await expect(page.getByTestId('edit-preview')).not.toHaveCount(0);
  await page.mouse.click(tx, ty);
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Route/u);
  await save(page);

  const after = (readBoard(project).tracks ?? []) as Track[];
  expect(after.some((t) => t.id === out?.id)).toBe(false);
  const bent = after.filter(
    (t) =>
      t.net === out?.net &&
      [t.start, t.end].some((p) => p[0] < 128.7 * MM && p[1] > 99 * MM && p[1] < 101 * MM),
  );
  expect(bent.length).toBeGreaterThan(0);
});

test('routes tracks, draws zones, updates from the schematic and imports DXF', async () => {
  launched = await launchApp();
  const { app, page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importAs(launched, 'Layout Edit');
  const project = join(projectsDir, 'Layout Edit');
  const board = readBoard(project);
  await openBoard(page, 'Layout Edit');
  const canvas = viewerCanvas(page);
  const tracks = board.tracks?.length ?? 0;

  // Route from a pad with a net to a free spot below it.
  const fp = board.footprints.find(
    (f) => (f.side ?? 'top') === 'top' && !f.rotation && f.pads.some((p) => p.net !== undefined),
  );
  expect(fp).toBeDefined();
  const pad = fp?.pads.find((p) => p.net !== undefined);
  const start: Point = [(fp?.at[0] ?? 0) + (pad?.at[0] ?? 0), (fp?.at[1] ?? 0) + (pad?.at[1] ?? 0)];
  await canvas.focus();
  await page.keyboard.press('x');
  await expect(page.getByTestId('board-tool-route')).toHaveAttribute('aria-pressed', 'true');
  const [sx, sy] = await toScreen(canvas, start);
  await page.mouse.move(sx, sy);
  await page.mouse.click(sx, sy);
  await expect(page.getByTestId('route-status')).toBeVisible();
  const target: Point = [start[0] + 2.54 * MM, start[1] - 2.54 * MM];
  const [tx, ty] = await toScreen(canvas, target);
  await page.mouse.move(tx, ty, { steps: 5 });
  await expect(page.getByTestId('route-preview')).toBeVisible();
  // By default the route only bends by 45° (never at right angles).
  await expect(page.getByTestId('board-corners')).toHaveAttribute('aria-pressed', 'false');
  await expect(page.getByTestId('route-status')).toContainText('45°');
  const preview = ((await page.getByTestId('route-preview').getAttribute('points')) ?? '')
    .trim()
    .split(/\s+/u)
    .map((pair) => pair.split(',').map(Number) as [number, number]);
  for (let i = 2; i < preview.length; i += 1) {
    const [a, b, c] = [preview[i - 2], preview[i - 1], preview[i]];
    if (a === undefined || b === undefined || c === undefined) {
      continue;
    }
    const d1 = Math.atan2(b[1] - a[1], b[0] - a[0]);
    const d2 = Math.atan2(c[1] - b[1], c[0] - b[0]);
    const bend = Math.abs(((d2 - d1 + 3 * Math.PI) % (2 * Math.PI)) - Math.PI);
    expect(bend).toBeLessThan(Math.PI / 4 + 0.05);
  }
  await page.getByTestId('board-corners').click();
  await expect(page.getByTestId('board-corners')).toHaveAttribute('aria-pressed', 'true');
  await page.getByTestId('board-corners').click();
  await expect(page.getByTestId('board-corners')).toHaveAttribute('aria-pressed', 'false');
  await page.mouse.move(tx, ty);
  await page.mouse.click(tx, ty);
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('route-status')).toHaveCount(0);
  await save(page);
  expect(readBoard(project).tracks?.length ?? 0).toBeGreaterThan(tracks);

  // A zone on the bottom layer, with its own dialog.
  await canvas.focus();
  await page.keyboard.press('PageDown');
  await expect(page.getByTestId('board-layer')).toHaveValue('cu.in1');
  await page.getByTestId('board-layer').selectOption('cu.bottom');
  await canvas.focus();
  await page.keyboard.press('z');
  const box = await canvas.boundingBox();
  if (box === null) {
    throw new Error('no canvas');
  }
  const corners: [number, number][] = [
    [box.x + box.width * 0.3, box.y + box.height * 0.3],
    [box.x + box.width * 0.6, box.y + box.height * 0.3],
    [box.x + box.width * 0.6, box.y + box.height * 0.6],
  ];
  for (const [x, y] of corners) {
    await page.mouse.click(x, y);
  }
  await expect(page.getByTestId('polygon-preview')).toBeVisible();
  await page.keyboard.press('Enter');
  const zone = page.getByTestId('zone-dialog');
  await expect(zone).toBeVisible();
  await page.getByTestId('zone-name').fill('Test zone');
  await page.getByTestId('zone-name').press('Enter');
  await expect(page.getByTestId('zone-layer-cu.bottom')).toBeChecked();
  await page.getByTestId('zone-save').click();
  await expect(zone).toBeHidden();
  await save(page);
  expect(readBoard(project).zones?.length ?? 0).toBe((board.zones?.length ?? 0) + 1);

  // Schematic and board agree after the import; the dialog says so.
  await page.getByTestId('board-update').click();
  const sync = page.getByTestId('sync-dialog');
  await expect(sync).toBeVisible();
  await expect(page.getByTestId('sync-none').or(page.getByTestId('sync-list'))).toBeVisible();
  await page.getByTestId('sync-back').click();
  await expect(page.getByTestId('sync-none').or(page.getByTestId('sync-list'))).toBeVisible();
  await sync.getByRole('button', { name: 'Cancel' }).click();

  // A DXF outline replaces the drawn one.
  const dir = mkdtempSync(join(tmpdir(), 'ew-dxf-'));
  const dxf = join(dir, 'outline.dxf');
  writeFileSync(
    dxf,
    [
      '0',
      'SECTION',
      '2',
      'HEADER',
      '9',
      '$INSUNITS',
      '70',
      '4',
      '0',
      'ENDSEC',
      '0',
      'SECTION',
      '2',
      'ENTITIES',
      '0',
      'LWPOLYLINE',
      '8',
      'EDGE',
      '90',
      '4',
      '70',
      '1',
      '10',
      '0',
      '20',
      '0',
      '10',
      '70',
      '20',
      '0',
      '10',
      '70',
      '20',
      '50',
      '10',
      '0',
      '20',
      '50',
      '0',
      'ENDSEC',
      '0',
      'EOF',
      '',
    ].join('\n'),
  );
  await queueDialogAnswer(app, dxf);
  await page.getByTestId('board-dxf').click();
  await page.getByTestId('dxf-choose').click();
  await expect(page.getByTestId('dxf-summary')).toContainText('1 shapes');
  await expect(page.getByTestId('dxf-summary')).toContainText('70 × 50');
  await page.getByTestId('dxf-import').click();
  await expect(page.getByTestId('dxf-dialog')).toBeHidden();
  await save(page);
  const outlines = (readBoard(project).shapes ?? []).filter((s) => s.layer === 'outline');
  expect(outlines).toHaveLength(1);
  expect(Object.keys(outlines[0]?.geometry ?? {})).toEqual(['polygon']);

  // An external autorouter: the board goes out as DSN, the session comes
  // back as tracks and vias of the routed net.
  const dsn = join(project, 'Layout Edit.dsn');
  await queueDialogAnswer(app, dsn);
  await page.getByTestId('board-autorouter').click();
  const router = page.getByTestId('autorouter-dialog');
  await expect(router).toBeVisible();
  await page.getByTestId('autorouter-export').click();
  await expect(page.getByTestId('autorouter-saved')).toContainText('Layout Edit.dsn');
  const text = readFileSync(dsn, 'utf8');
  expect(text).toContain('(pcb ');
  const net = /\(net (\S+) \(pins \S+ \S+/u.exec(text)?.[1] ?? '';
  expect(net).not.toBe('');
  const ses = join(project, 'Layout Edit.ses');
  writeFileSync(
    ses,
    `(session s (routes (resolution um 10) (network_out (net ${net} ` +
      '(wire (path F.Cu 2500 0 0 50000 0)) (via "Via[0-1]_600:300_um" 50000 0) ' +
      '(wire (path B.Cu 2500 50000 0 50000 -50000))) (net NOWHERE (wire (path F.Cu 10 0 0 1 1))))))',
  );
  const before = readBoard(project);
  await queueDialogAnswer(app, ses);
  await page.getByTestId('autorouter-import').click();
  await expect(page.getByTestId('autorouter-result')).toContainText('2 tracks and 1 vias imported');
  await expect(page.getByTestId('autorouter-unknown-nets')).toContainText('NOWHERE');
  await page.getByTestId('autorouter-close').click();
  await expect(router).toBeHidden();
  await save(page);
  const after = readBoard(project);
  expect(after.vias?.length ?? 0).toBeGreaterThanOrEqual(1);
  expect(after.tracks?.length ?? 0).toBeLessThanOrEqual((before.tracks?.length ?? 0) + 2);
  await page.getByTestId('toolbar-undo').click();
  await save(page);
  expect(readBoard(project).vias?.length ?? 0).toBe(before.vias?.length ?? 0);

  // Gerber and drill files as a reference drawing on user layers.
  const gerber = join(project, 'old-F_Cu.gbr');
  writeFileSync(
    gerber,
    '%TF.FileFunction,Copper,L1,Top*%\n%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.500000*%\n' +
      'D10*X0Y0D02*X20000000Y0D01*X20000000Y10000000D01*\nM02*\n',
  );
  const drills = join(project, 'old.drl');
  writeFileSync(drills, 'M48\nMETRIC\nT1C1.000\n%\nG90\nT1\nX5.0Y5.0\nX15.0Y5.0\nM30\n');
  const otherUser = (shapes: Board['shapes']): number =>
    (shapes ?? []).filter((s) => s.layer.startsWith('user.') && s.layer !== 'user.9').length;
  const userBefore = otherUser(readBoard(project).shapes);
  await queueDialogAnswer(app, `${gerber}\n${drills}`);
  await page.getByTestId('board-gerber-ref').click();
  await page.getByTestId('gerber-ref-choose').click();
  const rows = page.getByTestId('gerber-ref-files').locator('tbody tr');
  await expect(rows).toHaveCount(2);
  await expect(rows.first()).toContainText('Copper,L1,Top');
  await expect(rows.nth(1)).toContainText('Plated');
  await page.getByTestId('gerber-ref-layer-old.drl').selectOption('user.9');
  await page.getByTestId('gerber-ref-import').click();
  await expect(page.getByTestId('gerber-ref-dialog')).toBeHidden();
  await save(page);
  const shapes = readBoard(project).shapes ?? [];
  expect(shapes.filter((s) => s.layer === 'user.9')).toHaveLength(2);
  // The copper drawing went to the suggested free user layer: one area.
  expect(otherUser(shapes)).toBe(userBefore + 1);
});

test('cross-probes between schematic and board', async () => {
  launched = await launchApp();
  const { page, projectsDir } = launched;
  await page.setViewportSize({ width: 1280, height: 800 });
  await importAs(launched, 'Cross Probe');
  const dir = join(projectsDir, 'Cross Probe', 'schematics');
  const sheets = readdirSync(dir)
    .filter((name) => !name.startsWith('.'))
    .map(
      (name) =>
        JSON.parse(readFileSync(join(dir, name), 'utf8')) as {
          name: string;
          symbols: { designator: string; at: Point; symbol: string }[];
        },
    );
  const sheet = sheets.find((s) => s.symbols.some((sym) => /^R\d/u.test(sym.designator)));
  const symbol = sheet?.symbols.find((sym) => /^R\d/u.test(sym.designator));
  expect(symbol).toBeDefined();

  // Open the board first, then the sheet with the resistor.
  await openBoard(page, 'Cross Probe');
  await page
    .locator('[data-testid^="tree-doc:"]', { hasText: sheet?.name ?? '' })
    .first()
    .dblclick();
  await settle(page);
  await expect(page.getByTestId('viewer')).toHaveAttribute('data-kind', 'schematic');
  const [x, y] = await toScreen(viewerCanvas(page), symbol?.at ?? [0, 0]);
  await page.mouse.click(x, y);
  await expect(page.getByTestId('inspector-title')).toContainText(symbol?.designator ?? '');

  // The board selects the footprint of that resistor.
  await page.locator('.tabs__tab', { hasText: 'Cross Probe' }).click();
  await page.locator('[data-testid="viewer-canvas"][data-ready="true"]').waitFor();
  await expect(page.getByTestId('viewer')).toHaveAttribute('data-kind', 'pcb');
  await expect(page.getByTestId('inspector-title')).toContainText(symbol?.designator ?? '');
});
