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
    openExternal: (url) => {
      ipcRenderer.send(IPC.openExternal, url);
    },
  },
  updates: {
    check: (manual) => ipcRenderer.invoke(IPC.updatesCheck, manual),
    pending: () => ipcRenderer.invoke(IPC.updatesPending),
    onAvailable: (listener) => subscribe(IPC.updatesAvailable, listener),
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
  dialogs: {
    openProject: () => ipcRenderer.invoke(IPC.dialogOpenProject),
    chooseFolder: (defaultPath, purpose) =>
      ipcRenderer.invoke(IPC.dialogChooseFolder, defaultPath, purpose),
    saveArchive: (defaultPath) => ipcRenderer.invoke(IPC.dialogSaveArchive, defaultPath),
    chooseImport: (kind) => ipcRenderer.invoke(IPC.dialogChooseImport, kind),
    chooseGerbers: () => ipcRenderer.invoke(IPC.dialogChooseGerbers),
    saveExport: (kind, defaultPath, content) =>
      ipcRenderer.invoke(IPC.exportSave, kind, defaultPath, content),
    chooseModelFile: (format, defaultPath) =>
      ipcRenderer.invoke(IPC.dialogChooseModel, format, defaultPath),
    saveImage: (defaultPath, data) => ipcRenderer.invoke(IPC.exportSaveImage, defaultPath, data),
  },
  view3d: {
    open: (doc, name) => {
      ipcRenderer.send(IPC.view3dOpen, doc, name);
    },
  },
  providers: {
    open: (id, page, query) => {
      ipcRenderer.send(IPC.providersOpen, id, page, query ?? '');
    },
    openExternal: (id, page, query) => {
      ipcRenderer.send(IPC.providersOpenExternal, id, page, query ?? '');
    },
    signOut: (id) => ipcRenderer.invoke(IPC.providersSignOut, id),
    status: () => ipcRenderer.invoke(IPC.providersStatus),
    onDownload: (listener) => subscribe(IPC.providersDownload, listener),
    chooseDownload: () => ipcRenderer.invoke(IPC.dialogChooseDownload),
  },
  sync: {
    publish: (message) => {
      ipcRenderer.send(IPC.syncPublish, message);
    },
    onMessage: (listener) => subscribe(IPC.syncMessage, listener),
  },
  projects: {
    defaultLocation: () => ipcRenderer.invoke(IPC.projectsLocation),
    recent: () => ipcRenderer.invoke(IPC.recentGet),
    addRecent: (manifestPath, name) => ipcRenderer.invoke(IPC.recentAdd, manifestPath, name),
    removeRecent: (manifestPath) => ipcRenderer.invoke(IPC.recentRemove, manifestPath),
    clearRecent: () => ipcRenderer.invoke(IPC.recentClear),
    setLastLocation: (folder) => ipcRenderer.invoke(IPC.recentSetLocation, folder),
    onRecentChanged: (listener) => subscribe(IPC.recentChanged, listener),
    showInFolder: (path) => {
      ipcRenderer.send(IPC.showItem, path);
    },
    openFile: (path) => {
      ipcRenderer.send(IPC.openFile, path);
    },
    takePendingOpen: () => ipcRenderer.invoke(IPC.pendingOpen),
    onOpenPath: (listener) => subscribe(IPC.openPath, listener),
  },
  library: {
    latest: () => ipcRenderer.invoke(IPC.libraryLatest),
    release: (tag) => ipcRenderer.invoke(IPC.libraryRelease, tag),
    download: (tag, file) => ipcRenderer.invoke(IPC.libraryDownload, tag, file),
    onProgress: (listener) => subscribe(IPC.libraryProgress, listener),
  },
  window: {
    setState: (state) => {
      ipcRenderer.send(IPC.windowState, state);
    },
    onCloseRequested: (listener) => subscribe(IPC.closeRequested, listener),
    confirmClose: () => {
      ipcRenderer.send(IPC.closeConfirmed);
    },
  },
};

// MessagePorts cannot cross the context bridge as values; hand them over to
// the page with window.postMessage instead (see Electron's MessagePort guide).
ipcRenderer.on(IPC.enginePort, (event) => {
  window.postMessage({ type: ENGINE_PORT_MESSAGE }, '*', event.ports);
});

contextBridge.exposeInMainWorld('ew', api);
