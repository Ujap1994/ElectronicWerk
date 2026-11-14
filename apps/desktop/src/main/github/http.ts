import type { GitHttpRequest, GitHttpResponse, HttpClient } from 'isomorphic-git';

import type { Fetch } from './api';

/**
 * isomorphic-git's HTTP client on top of `fetch`. The app passes Electron's
 * `net.fetch`, which uses the system's proxy settings and certificates.
 */
export function fetchHttpClient(fetchImpl: Fetch): HttpClient {
  return {
    async request(request: GitHttpRequest): Promise<GitHttpResponse> {
      const method = request.method ?? 'GET';
      const body = request.body === undefined ? undefined : await collect(request.body);
      const response = await fetchImpl(request.url, {
        method,
        headers: request.headers ?? {},
        ...(body === undefined ? {} : { body }),
        ...(request.signal === undefined ? {} : { signal: request.signal }),
      });
      const headers: Record<string, string> = {};
      response.headers.forEach((value, key) => {
        headers[key] = value;
      });
      return {
        url: response.url === '' ? request.url : response.url,
        method,
        statusCode: response.status,
        statusMessage: response.statusText,
        headers,
        body: chunks(response),
      };
    },
  };
}

async function collect(body: AsyncIterableIterator<Uint8Array>): Promise<Uint8Array<ArrayBuffer>> {
  const parts: Uint8Array[] = [];
  let size = 0;
  for await (const part of body) {
    parts.push(part);
    size += part.byteLength;
  }
  const result = new Uint8Array(size);
  let offset = 0;
  for (const part of parts) {
    result.set(part, offset);
    offset += part.byteLength;
  }
  return result;
}

async function* chunks(response: Response): AsyncIterableIterator<Uint8Array> {
  if (response.body === null) {
    return;
  }
  const reader = response.body.getReader();
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) {
        return;
      }
      yield value;
    }
  } finally {
    reader.releaseLock();
  }
}
