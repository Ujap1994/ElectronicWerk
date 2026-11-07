import { pathToFileURL } from 'node:url';

import { net, protocol } from 'electron';

import { APP_HOST, APP_SCHEME, resolveAppPath } from './urls';

/** Content Security Policy for the production UI. */
export const PRODUCTION_CSP = [
  "default-src 'none'",
  "script-src 'self'",
  "style-src 'self'",
  "img-src 'self' data: blob:",
  "font-src 'self' data:",
  "connect-src 'self'",
  "worker-src 'self' blob:",
  "base-uri 'none'",
  "form-action 'none'",
  "frame-ancestors 'none'",
  "object-src 'none'",
].join('; ');

/** Must run before the app is ready. */
export function registerAppScheme(): void {
  protocol.registerSchemesAsPrivileged([
    {
      scheme: APP_SCHEME,
      privileges: { standard: true, secure: true, supportFetchAPI: true, codeCache: true },
    },
  ]);
}

/** Serves the built UI from `rendererDir` under `app://bundle/`. */
export function handleAppScheme(rendererDir: string): void {
  protocol.handle(APP_SCHEME, async (request) => {
    const url = new URL(request.url);
    const file = url.host === APP_HOST ? resolveAppPath(rendererDir, url.pathname) : null;
    if (file === null) {
      return new Response('Not found', { status: 404 });
    }
    const upstream = await net.fetch(pathToFileURL(file).toString());
    const headers = new Headers(upstream.headers);
    headers.set('Content-Security-Policy', PRODUCTION_CSP);
    headers.set('X-Content-Type-Options', 'nosniff');
    headers.set('Cross-Origin-Opener-Policy', 'same-origin');
    return new Response(upstream.body, { status: upstream.status, headers });
  });
}
