import type { EngineApiErrorCode, EngineReply } from '@shared/engine';

/** Functions exported by the native addon (`crates/ew-node`). */
export interface NativeEngine {
  dispatch: (request: string) => string;
  /** Like `dispatch`, but `view.scene` returns its buffer separately. */
  dispatchBinary: (request: string) => { reply: string; data?: Uint8Array | null };
  /** Events produced since the last call, as a JSON array. */
  takeEvents: () => string;
  engineVersion: () => string;
}

/** Collects the pending engine events (never throws). */
export function takeEvents(engine: NativeEngine): unknown[] {
  try {
    const events: unknown = JSON.parse(engine.takeEvents());
    return Array.isArray(events) ? events : [];
  } catch {
    return [];
  }
}

const METHOD_PATTERN = /^[a-z][\w.]{0,127}$/u;

/** Methods whose reply carries a binary buffer (`data`). */
const BINARY_METHODS = new Set(['view.scene', 'library.preview', 'gerber.scene']);

/**
 * Validates one request from the UI, forwards it to the native engine and
 * returns the reply. Returns `null` for messages without a usable id.
 */
export function handleRequest(engine: NativeEngine, data: unknown): EngineReply | null {
  if (typeof data !== 'object' || data === null) {
    return null;
  }
  const { id, method, params } = data as { id?: unknown; method?: unknown; params?: unknown };
  if (typeof id !== 'number' || !Number.isSafeInteger(id)) {
    return null;
  }
  if (typeof method !== 'string' || !METHOD_PATTERN.test(method)) {
    return { id, error: { code: 'invalidRequest', message: 'invalid method name' } };
  }
  try {
    const request = JSON.stringify(params === undefined ? { method } : { method, params });
    if (BINARY_METHODS.has(method)) {
      // The scene's primitives travel as a binary buffer next to the JSON
      // metadata; the renderer hands them to the GPU without parsing.
      const { reply: json, data } = engine.dispatchBinary(request);
      const reply = JSON.parse(json) as
        | { result: Record<string, unknown> }
        | { error: { code: EngineApiErrorCode; message: string } };
      return 'result' in reply
        ? { id, result: { ...reply.result, data: data ?? null } }
        : { id, error: reply.error };
    }
    const reply = JSON.parse(engine.dispatch(request)) as
      { result: unknown } | { error: { code: EngineApiErrorCode; message: string } };
    return 'result' in reply ? { id, result: reply.result } : { id, error: reply.error };
  } catch (error) {
    return {
      id,
      error: {
        code: 'hostError',
        message: error instanceof Error ? error.message : String(error),
      },
    };
  }
}
