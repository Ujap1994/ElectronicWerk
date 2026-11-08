import type { EngineApiErrorCode, EngineReply } from '@shared/engine';

/** Functions exported by the native addon (`crates/ew-node`). */
export interface NativeEngine {
  dispatch: (request: string) => string;
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
    const request = params === undefined ? { method } : { method, params };
    const reply = JSON.parse(engine.dispatch(JSON.stringify(request))) as
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
