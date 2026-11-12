/**
 * The layout editor in the running app: the design rules check with the
 * online update, zone filling, design rules with a custom rule, board
 * presets, routing, zones, updating the board from the schematic and the
 * DXF import.
 */
import { mkdtempSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
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
