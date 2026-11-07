import { isEngineReply, type EngineErrorBody, type EngineErrorCode } from '@shared/engine';

/** Error raised when an engine call fails. */
export class EngineError extends Error {
  readonly code: EngineErrorCode;

  constructor(body: EngineErrorBody) {
    super(body.message);
    this.name = 'EngineError';
    this.code = body.code;
  }
}

interface Pending {
  readonly resolve: (value: unknown) => void;
  readonly reject: (reason: EngineError) => void;
  readonly timer: ReturnType<typeof setTimeout>;
}

/** Minimal MessagePort surface (browser and Node implementations both fit). */
export interface PortLike {
  onmessage: ((event: MessageEvent) => void) | null;
  postMessage: (message: unknown) => void;
  close: () => void;
}

/**
 * Promise-based client for the engine MessagePort. When the engine restarts,
 * `attach` is called with a new port; calls still waiting on the old port are
 * rejected with `disconnected`.
 */
export class EngineClient {
  private port: PortLike | null = null;
  private nextId = 1;
  private readonly pending = new Map<number, Pending>();
  private readonly timeoutMs: number;

  constructor(options: { timeoutMs?: number } = {}) {
    this.timeoutMs = options.timeoutMs ?? 10_000;
  }

  get connected(): boolean {
    return this.port !== null;
  }

  attach(port: PortLike): void {
    this.detach();
    this.port = port;
    port.onmessage = (event: MessageEvent) => {
      this.receive(event.data);
    };
  }

  detach(): void {
    if (this.port !== null) {
      this.port.onmessage = null;
      this.port.close();
      this.port = null;
    }
    for (const [id, pending] of this.pending) {
      clearTimeout(pending.timer);
      pending.reject(new EngineError({ code: 'disconnected', message: 'engine disconnected' }));
      this.pending.delete(id);
    }
  }

  call<T>(method: string, params?: unknown): Promise<T> {
    const port = this.port;
    if (port === null) {
      return Promise.reject(
        new EngineError({ code: 'disconnected', message: 'engine not connected' }),
      );
    }
    const id = this.nextId;
    this.nextId += 1;
    return new Promise<T>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new EngineError({ code: 'timeout', message: `${method} timed out` }));
      }, this.timeoutMs);
      this.pending.set(id, { resolve: resolve as (value: unknown) => void, reject, timer });
      port.postMessage(params === undefined ? { id, method } : { id, method, params });
    });
  }

  private receive(data: unknown): void {
    if (!isEngineReply(data)) {
      return;
    }
    const pending = this.pending.get(data.id);
    if (pending === undefined) {
      return;
    }
    this.pending.delete(data.id);
    clearTimeout(pending.timer);
    if ('result' in data) {
      pending.resolve(data.result);
    } else {
      pending.reject(new EngineError(data.error));
    }
  }
}
