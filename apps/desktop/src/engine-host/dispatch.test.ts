import { existsSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join, resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

import { handleRequest, type NativeEngine } from './dispatch';

const fakeEngine = (reply: string | Error): NativeEngine => ({
  dispatch: () => {
    if (reply instanceof Error) {
      throw reply;
    }
    return reply;
  },
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
const addon = join(nativeDir, `ew_node.${process.platform}-${process.arch}.node`);

describe('handleRequest (native engine)', () => {
  it('has a built native engine (run `npm run build:native`)', () => {
    expect(existsSync(addon), addon).toBe(true);
  });

  it('round-trips through the real Rust engine', () => {
    const engine = createRequire(import.meta.url)(addon) as NativeEngine;
    const info = handleRequest(engine, { id: 1, method: 'engine.info' });
    expect(info).toMatchObject({ id: 1, result: { name: 'ElectronicWerk Engine' } });

    const ping = handleRequest(engine, { id: 2, method: 'engine.ping', params: [1, 'a'] });
    expect(ping).toEqual({ id: 2, result: { pong: true, echo: [1, 'a'] } });

    const missing = handleRequest(engine, { id: 3, method: 'no.such' });
    expect(missing).toMatchObject({ id: 3, error: { code: 'methodNotFound' } });
  });
});
