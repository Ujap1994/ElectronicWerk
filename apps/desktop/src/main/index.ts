import { app, BrowserWindow, dialog, ipcMain, Menu, nativeImage, nativeTheme } from 'electron';

import { IPC, type AppInfo, type MenuAction, type SettingsSnapshot } from '@shared/api';
import { createTranslator, resolveLanguage } from '@shared/i18n';
import type { Settings } from '@shared/settings';

import { EngineManager } from './engine';
import { buildMenu } from './menu';
import { devServerUrl, paths, testHooksEnabled, testUpdateFeed, updateCheckDelayMs } from './paths';
import { handleAppScheme, registerAppScheme } from './protocol';
import { createSenderCheck, hardenApp, openExternalSafely } from './security';
import { SettingsStore } from './settings-store';
import { runSmokeTest, SMOKE_TEST_FLAG } from './smoke-test';
import { UpdateService } from './updates';
import { createMainWindow } from './window';

// Tests and screenshot automation run with an isolated profile. This must
// happen before the single-instance lock, which lives in the profile folder.
const userDataOverride = process.env['EW_USER_DATA_DIR'];
if (userDataOverride !== undefined && userDataOverride !== '') {
  app.setPath('userData', userDataOverride);
}

app.setName('ElectronicWerk');

function main(): void {
  const devUrl = devServerUrl();
  const isMac = process.platform === 'darwin';
  hardenApp(devUrl);
  registerAppScheme();

  let mainWindow: BrowserWindow | null = null;
  let settingsStore: SettingsStore | null = null;
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

  const engine = new EngineManager({
    hostPath: paths.engineHost,
    nativeDir: paths.nativeDir(),
    onStatus: (status) => {
      sendToWindows(IPC.engineStatusChanged, status);
    },
  });

  const applySettings = (settings: Settings): void => {
    nativeTheme.themeSource = settings.theme;
    const { language } = snapshot(settings);
    const t = createTranslator(language);
    Menu.setApplicationMenu(
      buildMenu({
        t,
        isMac,
        isPackaged: app.isPackaged,
        onAction: (action: MenuAction) => {
          mainWindow?.webContents.send(IPC.menuAction, action);
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
    app.setAboutPanelOptions({
      applicationName: t('app.name'),
      applicationVersion: app.getVersion(),
      credits: t('app.tagline'),
    });
  };

  app.on('second-instance', () => {
    if (mainWindow !== null) {
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
    engine.stop();
  });

  void app.whenReady().then(() => {
    handleAppScheme(paths.renderer);
    if (isMac && !app.isPackaged) {
      app.dock?.setIcon(paths.devDockIcon);
    }
    const store = new SettingsStore(app.getPath('userData'));
    settingsStore = store;
    applySettings(store.get());

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
      Object.assign(globalThis, { __ewTest: { engine, settings: () => settingsStore?.get() } });
    }
  });

  function openMainWindow(): BrowserWindow {
    const window = createMainWindow(devUrl, app.isPackaged);
    window.on('closed', () => {
      mainWindow = null;
    });
    return window;
  }
}

// Only one instance may run. A second launch focuses the existing window.
if (app.requestSingleInstanceLock()) {
  main();
} else {
  app.quit();
}
