import { isLanguage, type LanguageSetting } from './i18n';

export type ThemeSetting = 'system' | 'light' | 'dark';

/** User settings, persisted as JSON in the user data directory. */
export interface Settings {
  readonly schemaVersion: 1;
  readonly language: LanguageSetting;
  readonly theme: ThemeSetting;
  readonly updates: {
    readonly checkOnStartup: boolean;
    /** A release the user chose to skip (for example `0.2.0`), or `null`. */
    readonly skippedVersion: string | null;
  };
}

/** A partial update; nested objects may be partial too. */
export interface SettingsPatch {
  readonly language?: LanguageSetting;
  readonly theme?: ThemeSetting;
  readonly updates?: Partial<Settings['updates']>;
}

export const DEFAULT_SETTINGS: Settings = {
  schemaVersion: 1,
  language: 'system',
  theme: 'system',
  updates: { checkOnStartup: true, skippedVersion: null },
};

const THEMES: readonly ThemeSetting[] = ['system', 'light', 'dark'];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isLanguageSetting(value: unknown): value is LanguageSetting {
  return value === 'system' || isLanguage(value);
}

function isThemeSetting(value: unknown): value is ThemeSetting {
  return typeof value === 'string' && (THEMES as readonly string[]).includes(value);
}

/**
 * Turns untrusted input (a settings file, an IPC message) into valid
 * settings. Invalid or missing values fall back to the defaults; unknown
 * fields are dropped.
 */
export function sanitizeSettings(raw: unknown): Settings {
  const input = isRecord(raw) ? raw : {};
  const updates = isRecord(input['updates']) ? input['updates'] : {};
  const skipped = updates['skippedVersion'];
  return {
    schemaVersion: 1,
    language: isLanguageSetting(input['language']) ? input['language'] : DEFAULT_SETTINGS.language,
    theme: isThemeSetting(input['theme']) ? input['theme'] : DEFAULT_SETTINGS.theme,
    updates: {
      checkOnStartup:
        typeof updates['checkOnStartup'] === 'boolean'
          ? updates['checkOnStartup']
          : DEFAULT_SETTINGS.updates.checkOnStartup,
      skippedVersion:
        typeof skipped === 'string' && skipped.length > 0 && skipped.length <= 64 ? skipped : null,
    },
  };
}

/** Applies a patch and validates the result. */
export function applySettingsPatch(current: Settings, patch: unknown): Settings {
  const input = isRecord(patch) ? patch : {};
  const updates = isRecord(input['updates']) ? input['updates'] : {};
  return sanitizeSettings({
    ...current,
    ...input,
    updates: { ...current.updates, ...updates },
  });
}
