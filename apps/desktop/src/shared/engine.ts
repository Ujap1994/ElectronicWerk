/**
 * Types shared by the main process, the engine host and the renderer for
 * talking to the Rust engine.
 */

/** Information reported by the `engine.info` method. */
export interface EngineInfo {
  readonly name: string;
  readonly version: string;
  readonly os: string;
  readonly arch: string;
  readonly threads: number;
  readonly debug: boolean;
}

export type EngineState = 'starting' | 'running' | 'restarting' | 'failed' | 'stopped';

/** Engine lifecycle status, broadcast by the main process. */
export interface EngineStatus {
  readonly state: EngineState;
  /** Increases every time a new engine process starts. */
  readonly generation: number;
  /** Number of automatic restarts after a crash. */
  readonly restarts: number;
  readonly info: EngineInfo | null;
  readonly error: string | null;
}

/** A request sent over the engine MessagePort. */
export interface EngineRequest {
  readonly id: number;
  readonly method: string;
  readonly params?: unknown;
}

/** Error codes of the engine API (see crates/ew-core/src/error.rs). */
export type EngineApiErrorCode =
  | 'invalidRequest'
  | 'methodNotFound'
  | 'invalidParams'
  | 'noProject'
  | 'projectOpen'
  | 'unsavedChanges'
  | 'recoveryPending'
  | 'conflict'
  | 'notFound'
  | 'alreadyExists'
  | 'ioError'
  | 'formatError'
  | 'newerFormat'
  | 'internal';

/** Engine errors plus problems outside the engine (host and transport). */
export type EngineErrorCode = EngineApiErrorCode | 'hostError' | 'disconnected' | 'timeout';

export interface EngineErrorBody {
  readonly code: EngineErrorCode;
  readonly message: string;
}

/** A reply sent over the engine MessagePort. */
export type EngineReply =
  | { readonly id: number; readonly result: unknown }
  | { readonly id: number; readonly error: EngineErrorBody };

/** Messages from the engine host to the main process (`parentPort`). */
export type HostToMainMessage =
  | { readonly type: 'ready'; readonly info: EngineInfo }
  | { readonly type: 'fatal'; readonly message: string };

/** Messages from the main process to the engine host. */
export type MainToHostMessage = { readonly type: 'connect' } | { readonly type: 'test-crash' };

/** Exit code the host uses for unrecoverable setup errors (no restart). */
export const HOST_EXIT_FATAL = 78;

/** An event pushed by the engine host to every window. */
export interface EngineEventMessage {
  readonly event: unknown;
}

export function isEngineEventMessage(value: unknown): value is EngineEventMessage {
  return typeof value === 'object' && value !== null && 'event' in value && !('id' in value);
}

export function isEngineReply(value: unknown): value is EngineReply {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const record = value as Record<string, unknown>;
  return typeof record['id'] === 'number' && ('result' in record || 'error' in record);
}
