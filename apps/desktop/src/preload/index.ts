/// <reference lib="dom" />
/**
 * Preload script: the only bridge between the sandboxed UI and the main
 * process. It exposes a small, typed API as `window.ew` and nothing else.
 */
import { contextBridge, ipcRenderer, type IpcRendererEvent } from 'electron';

import { ENGINE_PORT_MESSAGE, IPC, type EwApi, type Unsubscribe } from '@shared/api';
import type { EngineStatus } from '@shared/engine';

/**
 * Subscribes to a main-process event. The payload type is fixed by the
 * `EwApi` signature that each caller implements.
 */
function subscribe(channel: string, listener: (payload: never) => void): Unsubscribe {
  const handler = (_event: IpcRendererEvent, payload: unknown): void => {
    listener(payload as never);
  };
  ipcRenderer.on(channel, handler);
  return () => {
    ipcRenderer.removeListener(channel, handler);
  };
}

const api: EwApi = {
  app: {
    info: () => ipcRenderer.invoke(IPC.appInfo),
    onMenuAction: (listener) => subscribe(IPC.menuAction, listener),
  },
  settings: {
    get: () => ipcRenderer.invoke(IPC.settingsGet),
    set: (patch) => ipcRenderer.invoke(IPC.settingsSet, patch),
    onChanged: (listener) => subscribe(IPC.settingsChanged, listener),
  },
  engine: {
    status: () => ipcRenderer.invoke(IPC.engineStatus) as Promise<EngineStatus>,
    onStatus: (listener) => subscribe(IPC.engineStatusChanged, listener),
    requestPort: () => {
      ipcRenderer.send(IPC.engineRequestPort);
    },
  },
};

// MessagePorts cannot cross the context bridge as values; hand them over to
// the page with window.postMessage instead (see Electron's MessagePort guide).
ipcRenderer.on(IPC.enginePort, (event) => {
  window.postMessage({ type: ENGINE_PORT_MESSAGE }, '*', event.ports);
});

contextBridge.exposeInMainWorld('ew', api);
