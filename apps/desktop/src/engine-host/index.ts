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
import { handleDatasheetRequest, isDatasheetMethod } from './datasheets';
import { cacheStepMesh, handleMeshRequest, isMeshMethod } from './meshes';

/** Checks whose reply may name STEP files to tessellate first. */
const CHECKS = new Set([
  'mechanical.check',
  'mechanical.section',
  'mechanical.outline',
  'mechanical.areas',
  'pcb.drc',
  'system.check',
]);

/** The STEP files a check reply asks for (`tessellate`). */
function filesToTessellate(reply: EngineReply | null): string[] {
  if (reply === null || !('result' in reply)) {
    return [];
  }
  const files = (reply.result as { tessellate?: unknown } | null)?.tessellate;
  return Array.isArray(files) ? files.filter((f): f is string => typeof f === 'string') : [];
}

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
  const flush = (): void => {
    for (const engineEvent of takeEvents(engine)) {
      for (const target of ports) {
        target.postMessage({ event: engineEvent });
      }
    }
  };
  // Background work (library builds) produces events between requests.
  setInterval(flush, 250).unref();

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
        const data = message.data as { id?: unknown; method?: unknown; params?: unknown } | null;
        if (data !== null && typeof data.id === 'number' && isDatasheetMethod(data.method)) {
          port.postMessage(handleDatasheetRequest(data.id, String(data.method), data.params));
          return;
        }
        if (data !== null && typeof data.id === 'number' && isMeshMethod(data.method)) {
          void handleMeshRequest(data.id, String(data.method), data.params).then((reply) => {
            port.postMessage(reply);
          });
          return;
        }
        const reply: EngineReply | null = handleRequest(engine, message.data);
        const files = CHECKS.has(String(data?.method)) ? filesToTessellate(reply) : [];
        if (files.length > 0) {
          // The enclosure check needs meshes of curved STEP files: make
          // them, then check once more.
          void Promise.all(files.map(cacheStepMesh)).then((made) => {
            const again = made.some(Boolean) ? handleRequest(engine, message.data) : reply;
            if (again !== null) {
              port.postMessage(again);
            }
            flush();
          });
          return;
        }
        if (reply !== null) {
          port.postMessage(reply);
        }
        flush();
      });
      port.start();
    },
  );

  send({ type: 'ready', info });
}

start();
