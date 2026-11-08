import { mkdtempSync } from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

import { addonCandidates, findAddon } from './addon';
import { handleRequest, takeEvents, type NativeEngine } from './dispatch';

const fakeEngine = (reply: string | Error): NativeEngine => ({
  dispatch: () => {
    if (reply instanceof Error) {
      throw reply;
    }
    return reply;
  },
  takeEvents: () => '[]',
  engineVersion: () => '0.0.0',
});

describe('handleRequest (unit)', () => {
  const engine = fakeEngine('{"result":{"ok":true}}');

  it('ignores messages without a numeric id', () => {
    expect(handleRequest(engine, null)).toBeNull();
    expect(handleRequest(engine, 'x')).toBeNull();
    expect(handleRequest(engine, { method: 'engine.info' })).toBeNull();
    expect(handleRequest(engine, { id: 1.5, method: 'engine.info' })).toBeNull();
  });

  it('rejects invalid method names before they reach native code', () => {
    for (const method of [42, '', 'Engine.info', '../x', 'a'.repeat(200)]) {
      expect(handleRequest(engine, { id: 1, method })).toEqual({
        id: 1,
        error: { code: 'invalidRequest', message: 'invalid method name' },
      });
    }
  });

  it('forwards results', () => {
    expect(handleRequest(engine, { id: 7, method: 'engine.info' })).toEqual({
      id: 7,
      result: { ok: true },
    });
  });

  it('reports native exceptions as hostError', () => {
    const failing = fakeEngine(new Error('boom'));
    expect(handleRequest(failing, { id: 3, method: 'engine.info' })).toEqual({
      id: 3,
      error: { code: 'hostError', message: 'boom' },
    });
  });
});

const nativeDir = resolve(import.meta.dirname, '../../native');

describe('addonCandidates', () => {
  it('prefers the exact build, then a universal build', () => {
    expect(addonCandidates('/n', 'darwin', 'arm64')).toEqual([
      join('/n', 'ew_node.darwin-arm64.node'),
      join('/n', 'ew_node.darwin-universal.node'),
    ]);
  });
});

describe('takeEvents', () => {
  it('parses the event list and survives garbage', () => {
    const engine = (events: string): NativeEngine => ({
      ...fakeEngine('{}'),
      takeEvents: () => events,
    });
    expect(takeEvents(engine('[{"type":"project"}]'))).toEqual([{ type: 'project' }]);
    expect(takeEvents(engine('{}'))).toEqual([]);
    expect(takeEvents(engine('nope'))).toEqual([]);
  });
});

describe('handleRequest (native engine)', () => {
  const addon = findAddon(nativeDir);

  it('has a built native engine (run `npm run build:native`)', () => {
    expect(addon, nativeDir).toBeDefined();
  });

  it('round-trips through the real Rust engine', () => {
    const engine = createRequire(import.meta.url)(addon ?? '') as NativeEngine;
    const info = handleRequest(engine, { id: 1, method: 'engine.info' });
    expect(info).toMatchObject({ id: 1, result: { name: 'ElectronicWerk Engine' } });

    const ping = handleRequest(engine, { id: 2, method: 'engine.ping', params: [1, 'a'] });
    expect(ping).toEqual({ id: 2, result: { pong: true, echo: [1, 'a'] } });

    const missing = handleRequest(engine, { id: 3, method: 'no.such' });
    expect(missing).toMatchObject({ id: 3, error: { code: 'methodNotFound' } });
  });

  it('creates, edits and saves a project through the real engine', () => {
    const engine = createRequire(import.meta.url)(addon ?? '') as NativeEngine;
    const folder = join(mkdtempSync(join(tmpdir(), 'ew-host-')), 'Demo');
    const created = handleRequest(engine, {
      id: 1,
      method: 'project.create',
      params: { path: folder, name: 'Demo', template: 'board-2l' },
    });
    expect(created).toMatchObject({ id: 1, result: { name: 'Demo' } });
    expect(takeEvents(engine)).toEqual([
      { type: 'project', action: 'opened', path: join(folder, 'Demo.ewproj') },
    ]);
    const edited = handleRequest(engine, {
      id: 2,
      method: 'project.setProperties',
      params: { revision: 'B' },
    });
    expect(edited).toMatchObject({ result: { dirty: true, undoLabel: 'set-project-properties' } });
    const events = takeEvents(engine);
    expect(events).toHaveLength(1);
    expect(events[0]).toMatchObject({ type: 'changed', changeSet: { cause: 'commit' } });
    expect(handleRequest(engine, { id: 3, method: 'project.save' })).toMatchObject({
      result: { history: { dirty: false } },
    });
    expect(handleRequest(engine, { id: 4, method: 'project.close' })).toMatchObject({
      result: { closed: true },
    });
    takeEvents(engine);
  });
});
