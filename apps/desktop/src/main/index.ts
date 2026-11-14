import { writeFile } from 'node:fs/promises';
import { isAbsolute, join } from 'node:path';

import {
  app,
  BrowserWindow,
  dialog,
  ipcMain,
  Menu,
  nativeImage,
  nativeTheme,
  shell,
} from 'electron';

import {
  IPC,
  isExportKind,
  isFolderPurpose,
  isImportKind,
  isModelFormat,
  isSyncMessage,
  type AppInfo,
  type MenuAction,
  type SettingsSnapshot,
  type WindowState,
} from '@shared/api';
import { createTranslator, resolveLanguage, type Translate } from '@shared/i18n';
import type { RecentState } from '@shared/recent';
import type { Settings } from '@shared/settings';

import {
  chooseArchiveFile,
  chooseExportFile,
  chooseDownload,
  chooseImageFile,
  chooseModelFile,
  chooseFolder,
  chooseGerbers,
  chooseImport,
  chooseProjectFile,
  GERBER_EXTENSIONS,
  queueDialogAnswer,
} from './dialogs';
import { EngineManager } from './engine';
import { writeExport } from './export';
import { buildMenu } from './menu';
import { projectFromArgv } from './open-path';
import { LIBRARY_TAG } from '@shared/library';
import { isProviderId, isProviderPage, isProviderUrl, providerUrl } from '@shared/providers';

import { LibraryDownloader } from './library';
import {
  devServerUrl,
  paths,
  testHooksEnabled,
  testLibraryFeed,
  testUpdateFeed,
  updateCheckDelayMs,
} from './paths';
import { handleAppScheme, registerAppScheme } from './protocol';
import { RecentStore } from './recent-store';
import { createSenderCheck, hardenApp, openExternalSafely } from './security';
import { SettingsStore } from './settings-store';
import { runSmokeTest, SMOKE_TEST_FLAG } from './smoke-test';
import { UpdateService } from './updates';
import { ProviderWindows } from './providers';
import { registerGithubIpc, type GithubIpc } from './github/ipc';
import { createMainWindow, createViewWindow } from './window';

// Tests and screenshot automation run with an isolated profile. This must
// happen before the single-instance lock, which lives in the profile folder.
const userDataOverride = process.env['EW_USER_DATA_DIR'];
if (userDataOverride !== undefined && userDataOverride !== '') {
  app.setPath('userData', userDataOverride);
}

app.setName('ElectronicWerk');

const NO_PROJECT: WindowState = {
  projectOpen: false,
  projectName: null,
  manifestPath: null,
  dirty: false,
  canUndo: false,
  canRedo: false,
};

/** Output files that may be opened with their application. */
const OPENABLE_OUTPUTS = new Set<string>([
  'pdf',
  'svg',
  'csv',
  'xlsx',
  'html',
  'zip',
  'xml',
  'd356',
  'net',
  'gbrjob',
  ...GERBER_EXTENSIONS,
]);

function isOpenableOutput(path: string): boolean {
  const dot = path.lastIndexOf('.');
  return dot > 0 && OPENABLE_OUTPUTS.has(path.slice(dot + 1).toLowerCase());
}

function isWindowState(value: unknown): value is WindowState {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const state = value as Record<string, unknown>;
  return (
    typeof state['projectOpen'] === 'boolean' &&
    typeof state['dirty'] === 'boolean' &&
    typeof state['canUndo'] === 'boolean' &&
    typeof state['canRedo'] === 'boolean' &&
    (state['projectName'] === null || typeof state['projectName'] === 'string') &&
    (state['manifestPath'] === null || typeof state['manifestPath'] === 'string')
  );
}

function main(): void {
  const devUrl = devServerUrl();
  const isMac = process.platform === 'darwin';
  hardenApp(devUrl);
  registerAppScheme();
  // The 2D views need WebGL 2. Without a usable GPU (virtual machines, remote
  // desktops, blocklisted drivers) Chromium only offers its software
  // renderer when this switch is set. The app shows only its own content.
  app.commandLine.appendSwitch('enable-unsafe-swiftshader');

  let mainWindow: BrowserWindow | null = null;
  let providerWindows: ProviderWindows | null = null;
  /** Windows with a view of the open project (3D views), by board. */
  const viewWindows = new Map<string, BrowserWindow>();
  const closeViewWindows = (): void => {
    for (const window of viewWindows.values()) {
      window.close();
    }
    viewWindows.clear();
  };
  let settingsStore: SettingsStore | null = null;
  let recentStore: RecentStore | null = null;
  let windowState: WindowState = NO_PROJECT;
  let closeConfirmed = false;
  let t: Translate = createTranslator('en');
  // A project handed over by the OS before the window could take it.
  let pendingOpen: string | null = projectFromArgv(process.argv, process.cwd());
  const isTrustedSender = createSenderCheck(devUrl);

  const snapshot = (settings: Settings): SettingsSnapshot => ({
    settings,
    language: resolveLanguage(settings.language, app.getPreferredSystemLanguages()),
  });

  const sendToWindows = (channel: string, payload: unknown): void => {
    for (const window of BrowserWindow.getAllWindows()) {
      window.webContents.send(channel, payload);
    }
  };

  const openPath = (path: string): void => {
    if (mainWindow === null || mainWindow.webContents.isLoading()) {
      pendingOpen = path;
      return;
    }
    if (mainWindow.isMinimized()) {
      mainWindow.restore();
    }
    mainWindow.focus();
    mainWindow.webContents.send(IPC.openPath, path);
  };

  // macOS hands over files (Finder, Dock) with this event, also at launch.
  app.on('open-file', (event, path) => {
    event.preventDefault();
    openPath(path);
  });

  const engine = new EngineManager({
    hostPath: paths.engineHost,
    nativeDir: paths.nativeDir(),
    env: {
      EW_LIBRARY_DIR: join(app.getPath('userData'), 'library'),
      EW_OCCT_DIR: paths.occtDir(),
    },
    onStatus: (status) => {
      sendToWindows(IPC.engineStatusChanged, status);
    },
  });

  const rebuildMenu = (): void => {
    Menu.setApplicationMenu(
      buildMenu({
        t,
        isMac,
        isPackaged: app.isPackaged,
        state: windowState,
        recent: recentStore?.get().projects ?? [],
        onAction: (action: MenuAction) => {
          mainWindow?.webContents.send(IPC.menuAction, action);
        },
        onOpenRecent: openPath,
        onClearRecent: () => {
          const state = recentStore?.clear();
          if (state !== undefined) {
            recentChanged(state);
          }
        },
        onAbout: () => {
          void dialog.showMessageBox({
            type: 'info',
            icon: nativeImage.createFromPath(paths.windowIcon),
            title: t('menu.about'),
            message: `${t('app.name')} ${app.getVersion()}`,
            detail: t('app.tagline'),
          });
        },
      }),
    );
  };

  const recentChanged = (state: RecentState): void => {
    rebuildMenu();
    sendToWindows(IPC.recentChanged, state);
  };

  const applySettings = (settings: Settings): void => {
    nativeTheme.themeSource = settings.theme;
    t = createTranslator(snapshot(settings).language);
    rebuildMenu();
    app.setAboutPanelOptions({
      applicationName: t('app.name'),
      applicationVersion: app.getVersion(),
      credits: t('app.tagline'),
    });
  };

  const applyWindowState = (state: WindowState): void => {
    const previous = windowState;
    windowState = state;
    // Views of another (or no) project close.
    if (!state.projectOpen || previous.manifestPath !== state.manifestPath) {
      closeViewWindows();
    }
    const window = mainWindow;
    if (window !== null) {
      const name = state.projectOpen && state.projectName !== null ? state.projectName : null;
      const dirtyMark = state.dirty && !isMac ? '● ' : '';
      window.setTitle(name === null ? 'ElectronicWerk' : `${dirtyMark}${name} — ElectronicWerk`);
      if (isMac) {
        window.setDocumentEdited(state.dirty);
        window.setRepresentedFilename(state.manifestPath ?? '');
      }
    }
    if (
      previous.projectOpen !== state.projectOpen ||
      previous.canUndo !== state.canUndo ||
      previous.canRedo !== state.canRedo
    ) {
      rebuildMenu();
    }
  };

  app.on('second-instance', (_event, argv, workingDirectory) => {
    const project = projectFromArgv(argv, workingDirectory);
    if (project !== null) {
      openPath(project);
    } else if (mainWindow !== null) {
      if (mainWindow.isMinimized()) {
        mainWindow.restore();
      }
      mainWindow.focus();
    }
  });

  // Closing the window ends the program on every platform (including macOS).
  app.on('window-all-closed', () => {
    app.quit();
  });

  app.on('before-quit', () => {
    if (closeConfirmed || !windowState.projectOpen) {
      engine.stop();
    }
  });

  app.on('will-quit', () => {
    engine.stop();
  });

  void app.whenReady().then(() => {
    handleAppScheme(paths.renderer);
    if (isMac && !app.isPackaged) {
      app.dock?.setIcon(paths.devDockIcon);
    }
    const store = new SettingsStore(app.getPath('userData'));
    settingsStore = store;
    recentStore = new RecentStore(app.getPath('userData'));
    applySettings(store.get());
    const recent = recentStore;

    ipcMain.handle(IPC.appInfo, (event): AppInfo | null => {
      if (!isTrustedSender(event.senderFrame)) {
        return null;
      }
      return {
        name: 'ElectronicWerk',
        version: app.getVersion(),
        platform: process.platform,
        arch: process.arch,
        isPackaged: app.isPackaged,
        versions: {
          electron: process.versions.electron,
          chrome: process.versions.chrome,
          node: process.versions.node,
        },
        systemLanguages: app.getPreferredSystemLanguages(),
      };
    });

    ipcMain.handle(IPC.settingsGet, (event): SettingsSnapshot | null =>
      isTrustedSender(event.senderFrame) ? snapshot(store.get()) : null,
    );

    ipcMain.handle(IPC.settingsSet, (event, patch: unknown): SettingsSnapshot | null => {
      if (!isTrustedSender(event.senderFrame)) {
        return null;
      }
      const updated = store.update(patch);
      applySettings(updated);
      const result = snapshot(updated);
      sendToWindows(IPC.settingsChanged, result);
      return result;
    });

    ipcMain.handle(IPC.engineStatus, (event) =>
      isTrustedSender(event.senderFrame) ? engine.getStatus() : null,
    );

    ipcMain.on(IPC.engineRequestPort, (event) => {
      if (isTrustedSender(event.senderFrame)) {
        engine.connect(event.sender);
      }
    });

    ipcMain.on(IPC.openExternal, (event, url: unknown) => {
      if (isTrustedSender(event.senderFrame) && typeof url === 'string') {
        openExternalSafely(url);
      }
    });

    // --- Projects -------------------------------------------------------

    ipcMain.handle(IPC.dialogOpenProject, (event) =>
      isTrustedSender(event.senderFrame) ? chooseProjectFile(mainWindow, t) : null,
    );

    ipcMain.handle(IPC.dialogChooseFolder, (event, defaultPath: unknown, purpose: unknown) =>
      isTrustedSender(event.senderFrame)
        ? chooseFolder(
            mainWindow,
            t,
            typeof defaultPath === 'string' ? defaultPath : '',
            isFolderPurpose(purpose) ? purpose : 'location',
          )
        : null,
    );

    ipcMain.handle(IPC.dialogChooseGerbers, (event) =>
      isTrustedSender(event.senderFrame) ? chooseGerbers(mainWindow, t) : null,
    );

    ipcMain.handle(IPC.dialogChooseImport, (event, kind: unknown) =>
      isTrustedSender(event.senderFrame) && isImportKind(kind)
        ? chooseImport(mainWindow, t, kind)
        : null,
    );

    ipcMain.handle(
      IPC.exportSave,
      async (event, kind: unknown, defaultPath: unknown, content: unknown) => {
        if (
          !isTrustedSender(event.senderFrame) ||
          !isExportKind(kind) ||
          typeof defaultPath !== 'string' ||
          typeof content !== 'string'
        ) {
          return null;
        }
        const path = await chooseExportFile(mainWindow, t, kind, defaultPath);
        if (path === null) {
          return null;
        }
        await writeExport(kind, path, content);
        return path;
      },
    );

    ipcMain.handle(IPC.dialogSaveArchive, (event, defaultPath: unknown) =>
      isTrustedSender(event.senderFrame)
        ? chooseArchiveFile(mainWindow, t, typeof defaultPath === 'string' ? defaultPath : '')
        : null,
    );

    ipcMain.handle(IPC.projectsLocation, (event) => {
      if (!isTrustedSender(event.senderFrame)) {
        return null;
      }
      const override = process.env['EW_PROJECTS_DIR'];
      return testHooksEnabled() && override !== undefined && override !== ''
        ? override
        : join(app.getPath('documents'), 'ElectronicWerk');
    });

    ipcMain.handle(IPC.recentGet, (event) =>
      isTrustedSender(event.senderFrame) ? recent.get() : null,
    );

    const updateRecent = (state: RecentState): RecentState => {
      recentChanged(state);
      return state;
    };

    ipcMain.handle(IPC.recentAdd, (event, manifestPath: unknown, name: unknown) => {
      if (!isTrustedSender(event.senderFrame) || typeof manifestPath !== 'string') {
        return null;
      }
      return updateRecent(recent.add(manifestPath, typeof name === 'string' ? name : ''));
    });

    ipcMain.handle(IPC.recentRemove, (event, manifestPath: unknown) =>
      isTrustedSender(event.senderFrame) && typeof manifestPath === 'string'
        ? updateRecent(recent.remove(manifestPath))
        : null,
    );

    ipcMain.handle(IPC.recentClear, (event) =>
      isTrustedSender(event.senderFrame) ? updateRecent(recent.clear()) : null,
    );

    ipcMain.handle(IPC.recentSetLocation, (event, folder: unknown) =>
      isTrustedSender(event.senderFrame) && typeof folder === 'string'
        ? updateRecent(recent.setLastLocation(folder))
        : null,
    );

    // Opens generated outputs with their application; only document,
    // table and manufacturing file types, never programs.
    ipcMain.on(IPC.openFile, (event, path: unknown) => {
      if (
        isTrustedSender(event.senderFrame) &&
        typeof path === 'string' &&
        isAbsolute(path) &&
        isOpenableOutput(path)
      ) {
        void shell.openPath(path);
      }
    });

    ipcMain.on(IPC.showItem, (event, path: unknown) => {
      if (isTrustedSender(event.senderFrame) && typeof path === 'string' && isAbsolute(path)) {
        shell.showItemInFolder(path);
      }
    });

    ipcMain.handle(IPC.pendingOpen, (event) => {
      if (!isTrustedSender(event.senderFrame)) {
        return null;
      }
      const path = pendingOpen;
      pendingOpen = null;
      return path;
    });

    ipcMain.on(IPC.windowState, (event, state: unknown) => {
      // Only the main window has a project to report.
      if (
        isTrustedSender(event.senderFrame) &&
        event.sender === mainWindow?.webContents &&
        isWindowState(state)
      ) {
        applyWindowState(state);
      }
    });

    // --- 3D view --------------------------------------------------------

    ipcMain.handle(IPC.dialogChooseModel, (event, format: unknown, defaultPath: unknown) => {
      if (
        !isTrustedSender(event.senderFrame) ||
        !isModelFormat(format) ||
        typeof defaultPath !== 'string'
      ) {
        return null;
      }
      return chooseModelFile(BrowserWindow.fromWebContents(event.sender), t, format, defaultPath);
    });

    ipcMain.handle(IPC.exportSaveImage, async (event, defaultPath: unknown, data: unknown) => {
      if (
        !isTrustedSender(event.senderFrame) ||
        typeof defaultPath !== 'string' ||
        !(data instanceof Uint8Array) ||
        !isPng(data)
      ) {
        return null;
      }
      const path = await chooseImageFile(
        BrowserWindow.fromWebContents(event.sender),
        t,
        defaultPath,
      );
      if (path === null) {
        return null;
      }
      await writeFile(path, data);
      return path;
    });

    ipcMain.on(IPC.view3dOpen, (event, doc: unknown, name: unknown) => {
      if (
        !isTrustedSender(event.senderFrame) ||
        typeof doc !== 'string' ||
        !/^[0-9a-f-]{36}$/u.test(doc) ||
        typeof name !== 'string' ||
        name.length > 200 ||
        !windowState.projectOpen
      ) {
        return;
      }
      const open = viewWindows.get(doc);
      if (open !== undefined && !open.isDestroyed()) {
        open.focus();
        return;
      }
      const fragment = `view3d=${encodeURIComponent(doc)}&name=${encodeURIComponent(name)}`;
      const window = createViewWindow(
        devUrl,
        app.isPackaged,
        fragment,
        t('view3d.windowTitle', { name }),
      );
      viewWindows.set(doc, window);
      window.on('closed', () => {
        if (viewWindows.get(doc) === window) {
          viewWindows.delete(doc);
        }
      });
    });

    // --- Online part libraries ------------------------------------------

    const providerBase = process.env['EW_PROVIDER_BASE'];
    const providers = new ProviderWindows({
      downloadsDir: join(app.getPath('userData'), 'downloads'),
      isPackaged: app.isPackaged,
      ...(testHooksEnabled() && providerBase !== undefined && providerBase !== ''
        ? { base: providerBase }
        : {}),
      onDownload: (download) => {
        mainWindow?.webContents.send(IPC.providersDownload, download);
        if (mainWindow !== null && !mainWindow.isDestroyed() && download.importable) {
          mainWindow.focus();
        }
      },
    });
    providerWindows = providers;

    ipcMain.on(IPC.providersOpen, (event, id: unknown, page: unknown, query: unknown) => {
      if (
        isTrustedSender(event.senderFrame) &&
        isProviderId(id) &&
        isProviderPage(page) &&
        typeof query === 'string' &&
        query.length <= 200
      ) {
        providers.open(id, page, query);
      }
    });

    ipcMain.on(IPC.providersOpenExternal, (event, id: unknown, page: unknown, query: unknown) => {
      if (
        isTrustedSender(event.senderFrame) &&
        isProviderId(id) &&
        isProviderPage(page) &&
        typeof query === 'string' &&
        query.length <= 200
      ) {
        const url = providerUrl(id, page, query);
        if (isProviderUrl(url)) {
          void shell.openExternal(url);
        }
      }
    });

    ipcMain.handle(IPC.providersSignOut, async (event, id: unknown) => {
      if (isTrustedSender(event.senderFrame) && isProviderId(id)) {
        await providers.signOut(id);
      }
    });

    ipcMain.handle(IPC.providersStatus, (event) =>
      isTrustedSender(event.senderFrame) ? providers.status() : [],
    );

    ipcMain.handle(IPC.dialogChooseDownload, (event) =>
      isTrustedSender(event.senderFrame)
        ? chooseDownload(BrowserWindow.fromWebContents(event.sender), t)
        : null,
    );

    // --- GitHub ---------------------------------------------------------

    const github: GithubIpc = registerGithubIpc({
      userData: app.getPath('userData'),
      appVersion: app.getVersion(),
      testHooks: testHooksEnabled(),
      isTrustedSender,
      openProject: () => (windowState.projectOpen ? windowState.manifestPath : null),
      broadcast: sendToWindows,
    });

    // The selection travels between the windows.
    ipcMain.on(IPC.syncPublish, (event, message: unknown) => {
      if (!isTrustedSender(event.senderFrame) || !isSyncMessage(message)) {
        return;
      }
      if (JSON.stringify(message).length > 256_000) {
        return;
      }
      for (const window of BrowserWindow.getAllWindows()) {
        if (window.webContents !== event.sender && !window.isDestroyed()) {
          window.webContents.send(IPC.syncMessage, message);
        }
      }
    });

    ipcMain.on(IPC.closeConfirmed, (event) => {
      if (isTrustedSender(event.senderFrame)) {
        closeConfirmed = true;
        mainWindow?.close();
      }
    });

    // --- Library packs --------------------------------------------------

    const feedDir = testLibraryFeed();
    const library = new LibraryDownloader({
      dir: join(app.getPath('userData'), 'library'),
      appVersion: app.getVersion(),
      ...(feedDir === undefined ? {} : { feedDir }),
    });

    ipcMain.handle(IPC.libraryLatest, (event) =>
      isTrustedSender(event.senderFrame) ? library.latest() : null,
    );

    ipcMain.handle(IPC.libraryRelease, (event, tag: unknown) =>
      isTrustedSender(event.senderFrame) && typeof tag === 'string' && LIBRARY_TAG.test(tag)
        ? library.release(tag)
        : null,
    );

    ipcMain.handle(IPC.libraryDownload, (event, tag: unknown, file: unknown) => {
      if (
        !isTrustedSender(event.senderFrame) ||
        typeof tag !== 'string' ||
        typeof file !== 'string'
      ) {
        return null;
      }
      const sender = event.sender;
      return library.download(tag, file, (progress) => {
        if (!sender.isDestroyed()) {
          sender.send(IPC.libraryProgress, progress);
        }
      });
    });

    // --- Updates --------------------------------------------------------

    const updates = new UpdateService({
      currentVersion: app.getVersion(),
      getSkippedVersion: () => store.get().updates.skippedVersion,
      feedFile: testUpdateFeed(),
    });

    ipcMain.handle(IPC.updatesCheck, (event, manual: unknown) =>
      isTrustedSender(event.senderFrame) ? updates.check(manual === true) : null,
    );

    ipcMain.handle(IPC.updatesPending, (event) =>
      isTrustedSender(event.senderFrame) ? updates.getPending() : null,
    );

    if (store.get().updates.checkOnStartup && !process.argv.includes(SMOKE_TEST_FLAG)) {
      setTimeout(() => {
        void updates.check(false).then((result) => {
          if (result.status === 'available') {
            sendToWindows(IPC.updatesAvailable, result);
          }
        });
      }, updateCheckDelayMs());
    }

    engine.start();
    mainWindow = openMainWindow();
    if (process.argv.includes(SMOKE_TEST_FLAG)) {
      runSmokeTest(mainWindow);
    }

    if (testHooksEnabled()) {
      // Exposed for end-to-end tests only (see e2e/).
      Object.assign(globalThis, {
        __ewTest: {
          engine,
          settings: () => settingsStore?.get(),
          queueDialogAnswer,
          windowState: () => windowState,
          windowTitle: () => mainWindow?.getTitle() ?? null,
          viewWindows: () => [...viewWindows.keys()],
          providerWindows: () => providerWindows?.openIds() ?? [],
          githubPages: () => [...github.openedPages],
        },
      });
    }
  });

  function openMainWindow(): BrowserWindow {
    const window = createMainWindow(devUrl, app.isPackaged);
    // While a project is open, the UI decides: it asks about unsaved changes
    // and closes the project cleanly, then confirms.
    window.on('close', (event) => {
      if (closeConfirmed || !windowState.projectOpen) {
        return;
      }
      if (window.webContents.isCrashed()) {
        return;
      }
      event.preventDefault();
      window.webContents.send(IPC.closeRequested);
    });
    window.on('unresponsive', () => {
      // A hung UI must never trap the user: allow closing without asking.
      closeConfirmed = true;
    });
    window.on('closed', () => {
      mainWindow = null;
      // The views belong to the main window: the program ends with it.
      closeViewWindows();
      providerWindows?.closeAll();
    });
    return window;
  }
}

/** `true` for PNG data (the signature). */
function isPng(data: Uint8Array): boolean {
  const signature = [137, 80, 78, 71, 13, 10, 26, 10];
  return data.length > 8 && signature.every((byte, index) => data[index] === byte);
}

// Only one instance may run. A second launch focuses the existing window.
if (app.requestSingleInstanceLock()) {
  main();
} else {
  app.quit();
}
