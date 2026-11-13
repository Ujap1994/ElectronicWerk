import { app, session, shell, type WebFrameMain } from 'electron';

import { providerContents } from './providers';
import { APP_ORIGIN, isAllowedExternalUrl, originOf } from './urls';

/** Origins the UI may be loaded from. */
function trustedOrigins(devUrl: string | undefined): string[] {
  const devOrigin = devUrl === undefined ? null : originOf(devUrl);
  return devOrigin === null ? [APP_ORIGIN] : [APP_ORIGIN, devOrigin];
}

/** Opens an allow-listed URL in the default browser; ignores everything else. */
export function openExternalSafely(url: string): void {
  if (isAllowedExternalUrl(url)) {
    void shell.openExternal(url);
  }
}

/** Creates a check that accepts IPC only from our own UI frames. */
export function createSenderCheck(
  devUrl: string | undefined,
): (frame: WebFrameMain | null) => boolean {
  const origins = trustedOrigins(devUrl);
  return (frame) => {
    if (frame === null) {
      return false;
    }
    const origin = originOf(frame.url);
    return origin !== null && origins.includes(origin);
  };
}

/**
 * Applies the Electron security checklist to every web contents: no new
 * windows, no navigation away from the app, no webviews, no permissions.
 * Must be called before the app is ready.
 */
export function hardenApp(devUrl: string | undefined): void {
  app.enableSandbox();
  const origins = trustedOrigins(devUrl);

  app.on('web-contents-created', (_event, contents) => {
    contents.setWindowOpenHandler(({ url }) => {
      openExternalSafely(url);
      return { action: 'deny' };
    });
    contents.on('will-navigate', (event, url) => {
      // Provider windows browse the provider's website (their own rules).
      if (providerContents.has(contents)) {
        return;
      }
      const origin = originOf(url);
      if (origin === null || !origins.includes(origin)) {
        event.preventDefault();
      }
    });
    contents.on('will-attach-webview', (event) => {
      event.preventDefault();
    });
  });

  void app.whenReady().then(() => {
    const ses = session.defaultSession;
    ses.setPermissionRequestHandler((_contents, _permission, callback) => {
      callback(false);
    });
    ses.setPermissionCheckHandler(() => false);
    if (devUrl !== undefined) {
      // Development only: Vite's hot reload injects an inline preamble script
      // and styles and talks over a websocket. Production keeps the strict
      // policy from protocol.ts.
      ses.webRequest.onHeadersReceived((details, callback) => {
        callback({
          responseHeaders: {
            ...details.responseHeaders,
            'Content-Security-Policy': [
              "default-src 'self'; script-src 'self' 'unsafe-inline'; " +
                "style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; " +
                "connect-src 'self' ws:; object-src 'none'",
            ],
          },
        });
      });
    }
  });
}
