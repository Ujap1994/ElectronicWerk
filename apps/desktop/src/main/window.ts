import { BrowserWindow, nativeTheme } from 'electron';

import { paths } from './paths';
import { APP_ORIGIN } from './urls';

/** Background colours matching the UI theme tokens (avoids a white flash). */
const BACKGROUND = { light: '#f6f7f9', dark: '#15171c' } as const;

export function createMainWindow(devUrl: string | undefined, isPackaged: boolean): BrowserWindow {
  const window = new BrowserWindow({
    width: 1280,
    height: 800,
    minWidth: 960,
    minHeight: 600,
    show: false,
    title: 'ElectronicWerk',
    backgroundColor: nativeTheme.shouldUseDarkColors ? BACKGROUND.dark : BACKGROUND.light,
    webPreferences: {
      preload: paths.preload,
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
      nodeIntegrationInWorker: false,
      webSecurity: true,
      allowRunningInsecureContent: false,
      spellcheck: false,
      devTools: !isPackaged,
    },
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
