import { mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';

import type { SignInMethod, TokenStorage } from '@shared/github';

/** The sign-in kept on this computer. */
export interface StoredToken {
  readonly method: SignInMethod;
  readonly token: string;
  /** When the token expires (ms since 1970; `null`: it does not expire). */
  readonly expiresAt: number | null;
  /** Renews an expiring token (GitHub App only). */
  readonly refreshToken: string | null;
  /** When the refresh token expires (ms since 1970). */
  readonly refreshExpiresAt: number | null;
}

/** A personal access token (it does not expire for ElectronicWerk). */
export function personalToken(token: string): StoredToken {
  return { method: 'token', token, expiresAt: null, refreshToken: null, refreshExpiresAt: null };
}

function parseStored(text: string): StoredToken | null {
  if (text === '') {
    return null;
  }
  // Versions up to 0.14 kept a bare personal access token.
  if (!text.startsWith('{')) {
    return personalToken(text);
  }
  try {
    const data = JSON.parse(text) as Record<string, unknown>;
    const number = (value: unknown): number | null => (typeof value === 'number' ? value : null);
    const token = data['token'];
    const method = data['method'];
    if (typeof token !== 'string' || token === '' || (method !== 'app' && method !== 'token')) {
      return null;
    }
    const refresh = data['refreshToken'];
    return {
      method,
      token,
      expiresAt: number(data['expiresAt']),
      refreshToken: typeof refresh === 'string' && refresh !== '' ? refresh : null,
      refreshExpiresAt: number(data['refreshExpiresAt']),
    };
  } catch {
    return null;
  }
}

/** The part of Electron's `safeStorage` the store uses. */
export interface SecretBox {
  isEncryptionAvailable: () => boolean;
  encryptString: (text: string) => Buffer;
  decryptString: (data: Buffer) => string;
  /** Linux only: the keyring in use (`basic_text`: none). */
  getSelectedStorageBackend?: () => string;
}

/**
 * Keeps the GitHub token, encrypted by the operating system: the macOS
 * keychain, Windows DPAPI or the Linux secret service. Without encryption
 * the token is only kept in memory until the program ends.
 */
export class TokenStore {
  private readonly file: string;
  private readonly box: SecretBox;
  private readonly platform: NodeJS.Platform;
  /** The token, if encryption is unavailable (never written to disk). */
  private memory: StoredToken | null = null;

  constructor(file: string, box: SecretBox, platform: NodeJS.Platform = process.platform) {
    this.file = file;
    this.box = box;
    this.platform = platform;
  }

  /** How the token is protected. */
  storage(): TokenStorage {
    if (!this.box.isEncryptionAvailable()) {
      return 'unavailable';
    }
    // Without a keyring, Linux encrypts with a fixed key: as good as plain text.
    if (this.platform === 'linux' && this.box.getSelectedStorageBackend?.() === 'basic_text') {
      return 'plain';
    }
    return 'keychain';
  }

  get(): StoredToken | null {
    if (this.memory !== null) {
      return this.memory;
    }
    if (!this.box.isEncryptionAvailable()) {
      return null;
    }
    try {
      return parseStored(this.box.decryptString(readFileSync(this.file)));
    } catch {
      // Missing, or encrypted by another user or computer.
      return null;
    }
  }

  set(token: StoredToken): void {
    if (!this.box.isEncryptionAvailable()) {
      this.memory = token;
      return;
    }
    mkdirSync(dirname(this.file), { recursive: true });
    const temporary = `${this.file}.tmp`;
    writeFileSync(temporary, this.box.encryptString(JSON.stringify(token)), { mode: 0o600 });
    renameSync(temporary, this.file);
    this.memory = null;
  }

  clear(): void {
    this.memory = null;
    rmSync(this.file, { force: true });
  }
}

/** A stand-in for the keychain in automated tests (not secure). */
export const TEST_SECRET_BOX: SecretBox = {
  isEncryptionAvailable: () => true,
  encryptString: (text) => Buffer.from(`test:${text}`, 'utf8'),
  decryptString: (data) => {
    const text = data.toString('utf8');
    if (!text.startsWith('test:')) {
      throw new Error('not a test secret');
    }
    return text.slice(5);
  },
};
