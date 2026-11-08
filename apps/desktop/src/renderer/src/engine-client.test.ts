import { afterEach, describe, expect, it, vi } from 'vitest';

import { EngineClient, EngineError, type PortLike } from './engine-client';

/** A fake port that records requests and lets the test reply. */
class FakePort implements PortLike {
  onmessage: ((event: MessageEvent) => void) | null = null;
  readonly sent: { id: number; method: string; params?: unknown }[] = [];
  closed = false;

  postMessage(message: unknown): void {
    this.sent.push(message as { id: number; method: string; params?: unknown });
  }

  close(): void {
    this.closed = true;
  }

  reply(data: unknown): void {
    this.onmessage?.({ data } as MessageEvent);
  }
}

afterEach(() => {
  vi.useRealTimers();
});

describe('EngineClient', () => {
  it('rejects calls while disconnected', async () => {
    const client = new EngineClient();
    await expect(client.call('engine.info')).rejects.toMatchObject({ code: 'disconnected' });
  });

  it('resolves results by request id', async () => {
    const client = new EngineClient();
    const port = new FakePort();
    client.attach(port);
    const first = client.call<string>('a');
    const second = client.call<string>('b', { x: 1 });
    expect(port.sent).toEqual([
      { id: 1, method: 'a' },
      { id: 2, method: 'b', params: { x: 1 } },
    ]);
    port.reply({ id: 2, result: 'two' });
    port.reply({ id: 1, result: 'one' });
    await expect(first).resolves.toBe('one');
    await expect(second).resolves.toBe('two');
  });

  it('turns error replies into EngineError', async () => {
    const client = new EngineClient();
    const port = new FakePort();
    client.attach(port);
    const call = client.call('nope');
    port.reply({ id: 1, error: { code: 'methodNotFound', message: 'unknown method' } });
    const error = await call.catch((reason: unknown) => reason);
    expect(error).toBeInstanceOf(EngineError);
    expect(error).toMatchObject({ code: 'methodNotFound', message: 'unknown method' });
  });

  it('ignores unrelated or malformed messages', async () => {
    const client = new EngineClient();
    const port = new FakePort();
    client.attach(port);
    const call = client.call('a');
    port.reply('garbage');
    port.reply({ id: 99, result: 'stray' });
    port.reply({ id: 1, result: 'ok' });
    await expect(call).resolves.toBe('ok');
  });

  it('rejects pending calls when a new port replaces the old one', async () => {
    const client = new EngineClient();
    const oldPort = new FakePort();
    client.attach(oldPort);
    const call = client.call('slow');
    client.attach(new FakePort());
    await expect(call).rejects.toMatchObject({ code: 'disconnected' });
    expect(oldPort.closed).toBe(true);
    expect(client.connected).toBe(true);
  });

  it('times out', async () => {
    vi.useFakeTimers();
    const client = new EngineClient({ timeoutMs: 50 });
    client.attach(new FakePort());
    const call = client.call('slow');
    vi.advanceTimersByTime(51);
    await expect(call).rejects.toMatchObject({ code: 'timeout' });
  });

  it('forwards engine events to listeners', () => {
    const client = new EngineClient();
    const port = new FakePort();
    client.attach(port);
    const seen: unknown[] = [];
    const unsubscribe = client.onEvent((event) => seen.push(event));
    port.reply({ event: { type: 'project', action: 'opened' } });
    unsubscribe();
    port.reply({ event: { type: 'project', action: 'closed' } });
    expect(seen).toEqual([{ type: 'project', action: 'opened' }]);
  });

  it('waits for a connection', async () => {
    const client = new EngineClient();
    let connected = false;
    const waiting = client.whenConnected().then(() => {
      connected = true;
    });
    await Promise.resolve();
    expect(connected).toBe(false);
    client.attach(new FakePort());
    await waiting;
    expect(connected).toBe(true);
    await expect(client.whenConnected()).resolves.toBeUndefined();
  });

  it('accepts a per-call timeout', async () => {
    vi.useFakeTimers();
    const client = new EngineClient({ timeoutMs: 10 });
    client.attach(new FakePort());
    const slow = client.call('project.open', { path: '/x' }, 1_000);
    vi.advanceTimersByTime(500);
    const fast = client.call('engine.info');
    vi.advanceTimersByTime(20);
    await expect(fast).rejects.toMatchObject({ code: 'timeout' });
    vi.advanceTimersByTime(1_000);
    await expect(slow).rejects.toMatchObject({ code: 'timeout' });
  });
});
