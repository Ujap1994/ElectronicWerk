import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

import {
  addRecent,
  EMPTY_RECENT,
  removeRecent,
  sanitizeRecent,
  type RecentState,
} from '@shared/recent';

/** Persists the recently opened projects as `recent.json` in a directory. */
export class RecentStore {
  private current: RecentState;
  private readonly file: string;

  constructor(directory: string) {
    this.file = join(directory, 'recent.json');
    this.current = this.load();
  }

  get(): RecentState {
    return this.current;
  }

  add(manifestPath: string, name: string): RecentState {
    return this.set(
      addRecent(this.current, { manifestPath, name, openedAt: new Date().toISOString() }),
    );
  }

  remove(manifestPath: string): RecentState {
    return this.set(removeRecent(this.current, manifestPath));
  }

  clear(): RecentState {
    return this.set({ ...this.current, projects: [] });
  }

  setLastLocation(folder: string): RecentState {
    return this.set(sanitizeRecent({ ...this.current, lastLocation: folder }));
  }

  private set(state: RecentState): RecentState {
    this.current = state;
    try {
      mkdirSync(dirname(this.file), { recursive: true });
      const temporary = `${this.file}.tmp`;
      writeFileSync(temporary, `${JSON.stringify(state, null, 2)}\n`, 'utf8');
      renameSync(temporary, this.file);
    } catch {
      // The list is a convenience; a read-only profile must not break the app.
    }
    return state;
  }

  private load(): RecentState {
    try {
      return sanitizeRecent(JSON.parse(readFileSync(this.file, 'utf8')));
    } catch {
      return EMPTY_RECENT;
    }
  }
}
