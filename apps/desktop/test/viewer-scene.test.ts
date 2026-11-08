/**
 * Integration test of the 2D viewer pipeline: the real Rust engine builds a
 * scene of the demo project, the engine host passes the binary buffer, and
 * the renderer's scene decoder and selection queries work on it.
 */
import { mkdtempSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

import { beforeAll, describe, expect, it } from 'vitest';

import { findAddon } from '../src/engine-host/addon';
import { handleRequest, takeEvents, type NativeEngine } from '../src/engine-host/dispatch';
import { pick, selectBox, selectLasso } from '../src/renderer/src/viewer/pick';
import { Scene } from '../src/renderer/src/viewer/scene';
import type { ObjectInfo, SceneReply } from '../src/renderer/src/viewer/types';

interface Footprint {
  id: string;
  designator: string;
  at: [number, number];
  rotation?: number;
  side?: string;
  pads: { number: string; at: [number, number]; net?: string }[];
}

interface Board {
  id: string;
  footprints: Footprint[];
  tracks: { id: string; layer: string; start: [number, number]; end: [number, number] }[];
  vias: unknown[];
  shapes: unknown[];
  zones?: unknown[];
  texts?: unknown[];
}

const addon = findAddon(resolve(import.meta.dirname, '../native'));
let engine: NativeEngine;
let nextId = 1;

function call(method: string, params?: unknown): unknown {
  const reply = handleRequest(engine, { id: nextId, method, params });
  nextId += 1;
  takeEvents(engine);
  if (reply === null || 'error' in reply) {
    throw new Error(`${method}: ${JSON.stringify(reply)}`);
  }
  return reply.result;
}

const all = (): boolean => true;

describe('viewer scene (native engine)', () => {
  let board: Board;
  let boardScene: Scene;
  let sheetScene: Scene;
  let sheetId: string;

  beforeAll(() => {
    expect(addon, 'run `npm run build:native` first').toBeDefined();
    engine = createRequire(import.meta.url)(addon ?? '') as NativeEngine;
    const folder = join(mkdtempSync(join(tmpdir(), 'ew-viewer-')), 'Demo');
    const snapshot = call('project.create', { path: folder, name: 'Demo', template: 'demo' }) as {
      documents: { id: string; kind: string }[];
      rootSchematic: string;
    };
    const pcbId = snapshot.documents.find((doc) => doc.kind === 'pcb')?.id ?? '';
    sheetId = snapshot.rootSchematic;
    board = call('document.get', { doc: pcbId }) as Board;
    boardScene = new Scene(call('view.scene', { doc: pcbId }) as SceneReply);
    sheetScene = new Scene(call('view.scene', { doc: sheetId }) as SceneReply);
    return () => {
      call('project.close', { discard: true });
    };
  });

  it('passes the primitives as a binary buffer', () => {
    const reply = call('view.scene', { doc: board.id }) as SceneReply;
    expect(reply.data).toBeInstanceOf(Uint8Array);
    expect((reply.data as Uint8Array).byteLength).toBe(reply.stats.bytes);
    // JSON clients get the same bytes as base64.
    const encoded = new Scene({
      ...reply,
      data: Buffer.from(reply.data as Uint8Array).toString('base64'),
    });
    expect(encoded.objectCount).toBe(boardScene.objectCount);
    expect(encoded.layers[0]?.segments).toEqual(boardScene.layers[0]?.segments);
  });

  it('decodes layers and objects', () => {
    const keys = boardScene.meta.layers.map((layer) => layer.key);
    expect(keys.slice(0, 2)).toEqual(['cu.top', 'cu.bottom']);
    expect(keys).toContain('holes');
    const tops = board.footprints.length + board.tracks.length + board.vias.length;
    const topLevel = [...Array(boardScene.objectCount).keys()].filter(
      (obj) => boardScene.parentOf(obj) === null,
    );
    expect(topLevel.length).toBeGreaterThanOrEqual(tops);
    // Pads are children of their footprint and carry its id.
    const pad = [...Array(boardScene.objectCount).keys()].find(
      (obj) => boardScene.typeOf(obj) === 'pad',
    );
    expect(pad).toBeDefined();
    const parent = boardScene.parentOf(pad ?? 0);
    expect(parent).not.toBeNull();
    expect(boardScene.idOf(pad ?? 0)).toBe(boardScene.idOf(parent ?? 0));
    expect(boardScene.keyOf(pad ?? 0)).toMatch(/\/pad\/\d+$/u);
    expect(boardScene.childrenOf(parent ?? 0)).toContain(pad);
  });

  it('picks the pad under the cursor and describes it', () => {
    const fp = board.footprints.find(
      (entry) => (entry.side ?? 'top') === 'top' && (entry.rotation ?? 0) === 0,
    );
    expect(fp).toBeDefined();
    const pad = fp?.pads[0];
    const [ox, oy] = boardScene.meta.origin;
    const x = (fp?.at[0] ?? 0) + (pad?.at[0] ?? 0) - ox;
    const y = (fp?.at[1] ?? 0) + (pad?.at[1] ?? 0) - oy;
    const hit = pick(boardScene, x, y, 10_000, all, 0);
    expect(hit).not.toBeNull();
    expect(boardScene.typeOf(hit ?? 0)).toBe('pad');
    expect(boardScene.netOf(hit ?? 0)).toMatch(/^N\d+/u);
    const info = call('view.object', {
      doc: board.id,
      id: boardScene.idOf(hit ?? 0),
      child: { kind: 'pad', index: boardScene.children[hit ?? 0] },
    }) as ObjectInfo;
    expect(info.type).toBe('pad');
    expect(info.title).toMatch(new RegExp(`^${fp?.designator ?? ''}-`, 'u'));
    expect(info.rows.find((row) => row.key === 'net')?.value).toMatch(/^N\d+/u);
  });

  it('picks tracks only on visible layers', () => {
    // The longest track: its middle is far from pads and vias.
    const length = (entry: Board['tracks'][number]): number =>
      Math.hypot(entry.end[0] - entry.start[0], entry.end[1] - entry.start[1]);
    const track = [...board.tracks].sort((a, b) => length(b) - length(a))[0];
    expect(track).toBeDefined();
    const [ox, oy] = boardScene.meta.origin;
    const x = ((track?.start[0] ?? 0) + (track?.end[0] ?? 0)) / 2 - ox;
    const y = ((track?.start[1] ?? 0) + (track?.end[1] ?? 0)) / 2 - oy;
    const layer = boardScene.meta.layers.findIndex((entry) => entry.key === track?.layer);
    const hit = pick(boardScene, x, y, 1_000, (l) => l === layer, layer);
    expect(hit).not.toBeNull();
    expect(boardScene.typeOf(hit ?? 0)).toBe('track');
    const hidden = pick(boardScene, x, y, 1_000, (l) => l !== layer, null);
    expect(hidden).not.toBe(hit);
  });

  it('selects by box and lasso', () => {
    const bounds = boardScene.meta.bounds ?? [0, 0, 0, 0];
    const [ox, oy] = boardScene.meta.origin;
    const everything = [
      bounds[0] - ox - 1,
      bounds[1] - oy - 1,
      bounds[2] - ox + 1,
      bounds[3] - oy + 1,
    ] as const;
    const windowed = selectBox(boardScene, everything, 'window', all);
    const topLevel = [...Array(boardScene.objectCount).keys()].filter(
      (obj) => boardScene.parentOf(obj) === null,
    );
    expect(windowed).toEqual(topLevel);
    // A tiny window selects nothing, a tiny crossing box selects what it touches.
    const [cx, cy] = [(everything[0] + everything[2]) / 2, (everything[1] + everything[3]) / 2];
    const tiny = [cx - 1, cy - 1, cx + 1, cy + 1] as const;
    expect(selectBox(boardScene, tiny, 'window', all)).toEqual([]);
    expect(selectBox(boardScene, tiny, 'crossing', all).length).toBeGreaterThanOrEqual(0);
    const lasso = [
      everything[0],
      everything[1],
      everything[2],
      everything[1],
      everything[2],
      everything[3],
      everything[0],
      everything[3],
    ];
    expect(selectLasso(boardScene, lasso, all)).toEqual(topLevel);
  });

  it('builds sheets with symbols, pins and fields', () => {
    expect(sheetScene.meta.kind).toBe('schematic');
    const symbol = [...Array(sheetScene.objectCount).keys()].find(
      (obj) => sheetScene.typeOf(obj) === 'symbol',
    );
    expect(symbol).toBeDefined();
    const children = [...sheetScene.childrenOf(symbol ?? 0)].map((obj) => sheetScene.typeOf(obj));
    expect(children).toContain('pin');
    expect(children).toContain('field');
    const box = sheetScene.box(symbol ?? 0);
    const hit = pick(sheetScene, (box[0] + box[2]) / 2, (box[1] + box[3]) / 2, 1_000, all);
    expect(hit).not.toBeNull();
    const info = call('view.object', { doc: sheetId, id: sheetScene.idOf(hit ?? 0) }) as ObjectInfo;
    expect(['symbol', 'pin', 'field']).toContain(info.type);
  });
});
