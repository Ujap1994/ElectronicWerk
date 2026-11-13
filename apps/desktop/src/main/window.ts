import { BrowserWindow, nativeTheme } from 'electron';

import { paths } from './paths';
import { APP_ORIGIN } from './urls';

/** Background colours matching the UI theme tokens (avoids a white flash). */
const BACKGROUND = { light: '#f6f7f9', dark: '#15171c' } as const;

function webPreferences(isPackaged: boolean): Electron.WebPreferences {
  return {
    preload: paths.preload,
    contextIsolation: true,
    sandbox: true,
    nodeIntegration: false,
    nodeIntegrationInWorker: false,
    webSecurity: true,
    allowRunningInsecureContent: false,
    spellcheck: false,
    devTools: !isPackaged,
  };
}

/**
 * A window with one view of the open project (the 3D view): the same app
 * page, told by its URL fragment what to show.
 */
export function createViewWindow(
  devUrl: string | undefined,
  isPackaged: boolean,
  fragment: string,
  title: string,
): BrowserWindow {
  const window = new BrowserWindow({
    width: 1100,
    height: 760,
    minWidth: 480,
    minHeight: 360,
    show: false,
    title,
    ...(process.platform === 'darwin' ? {} : { icon: paths.windowIcon }),
    backgroundColor: nativeTheme.shouldUseDarkColors ? BACKGROUND.dark : BACKGROUND.light,
    webPreferences: webPreferences(isPackaged),
  });
  window.once('ready-to-show', () => {
    window.show();
  });
  const base = devUrl ?? `${APP_ORIGIN}/index.html`;
  void window.loadURL(`${base}#${fragment}`);
  return window;
}

export function createMainWindow(devUrl: string | undefined, isPackaged: boolean): BrowserWindow {
  const window = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 960,
    minHeight: 600,
    show: false,
    title: 'ElectronicWerk',
    // macOS takes the icon from the app bundle; Windows and Linux need it here.
    ...(process.platform === 'darwin' ? {} : { icon: paths.windowIcon }),
    backgroundColor: nativeTheme.shouldUseDarkColors ? BACKGROUND.dark : BACKGROUND.light,
    webPreferences: webPreferences(isPackaged),
  });

  window.once('ready-to-show', () => {
    window.show();
  });

  if (devUrl === undefined) {
    void window.loadURL(`${APP_ORIGIN}/index.html`);
  } else {
    void window.loadURL(devUrl);
  }
  return window;
}
