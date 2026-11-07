import {
  MessageChannelMain,
  utilityProcess,
  type UtilityProcess,
  type WebContents,
} from 'electron';

import { IPC } from '@shared/api';
import {
  HOST_EXIT_FATAL,
  type EngineStatus,
  type HostToMainMessage,
  type MainToHostMessage,
} from '@shared/engine';

export interface EngineManagerOptions {
  /** Path of the bundled engine host script. */
  readonly hostPath: string;
  /** Directory that contains the native addon. */
  readonly nativeDir: string;
  /** Called on every status change. */
  readonly onStatus: (status: EngineStatus) => void;
  /** Maximum automatic restarts within `restartWindowMs`. */
  readonly maxRestarts?: number;
  readonly restartWindowMs?: number;
  readonly restartDelayMs?: number;
}

/**
 * Runs the Rust engine in a separate `utilityProcess`. If the engine crashes,
 * it is restarted automatically (with a limit), and every window gets a new
 * connection. The UI keeps running in the meantime.
 */
export class EngineManager {
  private child: UtilityProcess | null = null;
  private status: EngineStatus = {
    state: 'stopped',
    generation: 0,
    restarts: 0,
    info: null,
    error: null,
  };
  private crashTimes: number[] = [];
  private stopping = false;
  private restartTimer: NodeJS.Timeout | null = null;
  private readonly options: Required<EngineManagerOptions>;

  constructor(options: EngineManagerOptions) {
    this.options = {
      maxRestarts: 3,
      restartWindowMs: 60_000,
      restartDelayMs: 300,
      ...options,
    };
  }

  getStatus(): EngineStatus {
    return this.status;
  }

  start(): void {
    this.stopping = false;
    this.spawn(this.status.state === 'restarting' ? 'restarting' : 'starting');
  }

  /** Stops the engine for good (app quit). */
  stop(): void {
    this.stopping = true;
    if (this.restartTimer !== null) {
      clearTimeout(this.restartTimer);
      this.restartTimer = null;
    }
    this.child?.kill();
    this.child = null;
    this.setStatus({ state: 'stopped' });
  }

  /** Sends a fresh MessagePort to a window. Ignored while the engine is not running. */
  connect(target: WebContents): boolean {
    if (this.child === null || this.status.state !== 'running') {
      return false;
    }
    const { port1, port2 } = new MessageChannelMain();
    const message: MainToHostMessage = { type: 'connect' };
    this.child.postMessage(message, [port2]);
    target.postMessage(IPC.enginePort, null, [port1]);
    return true;
  }

  /** Test hook: makes the engine process crash. */
  crashForTest(): void {
    const message: MainToHostMessage = { type: 'test-crash' };
    this.child?.postMessage(message);
  }

  private spawn(state: 'starting' | 'restarting'): void {
    const child = utilityProcess.fork(this.options.hostPath, [], {
      serviceName: 'ElectronicWerk Engine',
      env: { ...process.env, EW_NATIVE_DIR: this.options.nativeDir },
      stdio: 'inherit',
    });
    this.child = child;
    this.setStatus({
      state,
      generation: this.status.generation + 1,
      info: null,
      error: null,
    });

    child.on('message', (message: HostToMainMessage) => {
      if (child !== this.child) {
        return;
      }
      if (message.type === 'ready') {
        this.setStatus({ state: 'running', info: message.info, error: null });
      } else {
        this.setStatus({ state: 'failed', error: message.message });
      }
    });

    child.on('exit', (code) => {
      if (child !== this.child) {
        return;
      }
      this.child = null;
      this.handleExit(code);
    });
  }

  private handleExit(code: number): void {
    if (this.stopping) {
      return;
    }
    if (code === HOST_EXIT_FATAL || this.status.state === 'failed') {
      this.setStatus({ state: 'failed', error: this.status.error ?? `exit code ${String(code)}` });
      return;
    }
    const now = Date.now();
    this.crashTimes = [...this.crashTimes, now].filter(
      (time) => now - time <= this.options.restartWindowMs,
    );
    if (this.crashTimes.length > this.options.maxRestarts) {
      this.setStatus({
        state: 'failed',
        error: `engine crashed ${String(this.crashTimes.length)} times in a row`,
      });
      return;
    }
    this.setStatus({ state: 'restarting', restarts: this.status.restarts + 1, info: null });
    this.restartTimer = setTimeout(() => {
      this.restartTimer = null;
      if (!this.stopping) {
        this.spawn('restarting');
      }
    }, this.options.restartDelayMs);
  }

  private setStatus(change: Partial<EngineStatus>): void {
    this.status = { ...this.status, ...change };
    this.options.onStatus(this.status);
  }
}
