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
  // The route is in the board (and selected) once the view has reloaded.
  await expect(page.getByTestId('toolbar-undo')).toHaveAttribute('title', /Route/u);
  await settle(page);
  await canvas.focus();
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
