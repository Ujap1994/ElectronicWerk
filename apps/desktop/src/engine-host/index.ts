/**
 * Engine host: runs inside an Electron `utilityProcess`, loads the native
 * Rust engine and serves requests that arrive over MessagePorts.
 */
import { createRequire } from 'node:module';

import {
  HOST_EXIT_FATAL,
  type EngineInfo,
  type EngineReply,
  type HostToMainMessage,
  type MainToHostMessage,
} from '@shared/engine';

import { findAddon } from './addon';
import { handleRequest, takeEvents, type NativeEngine } from './dispatch';

const parentPort = process.parentPort;

function send(message: HostToMainMessage): void {
  parentPort.postMessage(message);
}

function loadEngine(): NativeEngine {
  const directory = process.env['EW_NATIVE_DIR'] ?? '';
  const file = findAddon(directory);
  if (file === undefined) {
    throw new Error(`native engine not found in "${directory}"`);
  }
  const require = createRequire(import.meta.url);
  return require(file) as NativeEngine;
}

function start(): void {
  let engine: NativeEngine;
  let info: EngineInfo;
  try {
    engine = loadEngine();
    const reply = JSON.parse(engine.dispatch('{"method":"engine.info"}')) as { result: EngineInfo };
    info = reply.result;
  } catch (error) {
    send({ type: 'fatal', message: error instanceof Error ? error.message : String(error) });
    process.exit(HOST_EXIT_FATAL);
  }

  // Every window's port; engine events go to all of them.
  const ports = new Set<Electron.MessagePortMain>();

  parentPort.on(
    'message',
    (event: { data: MainToHostMessage; ports: Electron.MessagePortMain[] }) => {
      if (event.data.type === 'test-crash') {
        process.exit(70);
      }
      // The only other message is `connect`, which carries the new port.
      const [port] = event.ports;
      if (port === undefined) {
        return;
      }
      ports.add(port);
      port.on('close', () => {
        ports.delete(port);
      });
      port.on('message', (message) => {
        const reply: EngineReply | null = handleRequest(engine, message.data);
        if (reply !== null) {
          port.postMessage(reply);
        }
        for (const engineEvent of takeEvents(engine)) {
          for (const target of ports) {
            target.postMessage({ event: engineEvent });
          }
        }
      });
      port.start();
    },
  );

  send({ type: 'ready', info });
}

start();
