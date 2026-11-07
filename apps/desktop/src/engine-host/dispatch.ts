import type { EngineReply } from '@shared/engine';

/** Functions exported by the native addon (`crates/ew-node`). */
export interface NativeEngine {
  dispatch: (request: string) => string;
  engineVersion: () => string;
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
      | { result: unknown }
      | { error: { code: 'invalidRequest' | 'methodNotFound' | 'invalidParams'; message: string } };
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
