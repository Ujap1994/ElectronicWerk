import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

import {
  applySettingsPatch,
  DEFAULT_SETTINGS,
  sanitizeSettings,
  type Settings,
} from '@shared/settings';

/** Loads and saves the user settings as `settings.json` in a directory. */
export class SettingsStore {
  private current: Settings;
  private readonly file: string;

  constructor(directory: string) {
    this.file = join(directory, 'settings.json');
    this.current = this.load();
  }

  get(): Settings {
    return this.current;
  }

  /** Applies an untrusted patch, saves it and returns the new settings. */
  update(patch: unknown): Settings {
    this.current = applySettingsPatch(this.current, patch);
    this.save();
    return this.current;
  }

  private load(): Settings {
    try {
      return sanitizeSettings(JSON.parse(readFileSync(this.file, 'utf8')));
    } catch {
      // Missing or corrupt file: start with defaults.
      return DEFAULT_SETTINGS;
    }
  }

  private save(): void {
    // Write to a temporary file first, then rename: never a half-written file.
    mkdirSync(dirname(this.file), { recursive: true });
    const temporary = `${this.file}.tmp`;
    writeFileSync(temporary, `${JSON.stringify(this.current, null, 2)}\n`, 'utf8');
    renameSync(temporary, this.file);
  }
}
