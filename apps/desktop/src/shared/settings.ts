import { isLanguage, type LanguageSetting } from './i18n';
import { sanitizeShortcuts } from './shortcuts';

export type ThemeSetting = 'system' | 'light' | 'dark';

/**
 * How much the app decides for the user. `beginner` computes and applies
 * engineering values (for example track widths for an impedance) from a few
 * choices; `advanced` shows every value and lets the user set it.
 */
export type WorkMode = 'beginner' | 'advanced';

/** How far one notch of the mouse wheel zooms the 2D views. */
export type WheelZoomSetting = 'fine' | 'normal' | 'coarse';

/** User settings, persisted as JSON in the user data directory. */
export interface Settings {
  readonly schemaVersion: 1;
  readonly language: LanguageSetting;
  readonly theme: ThemeSetting;
  readonly workMode: WorkMode;
  readonly wheelZoom: WheelZoomSetting;
  /** Keys the user gave to editor commands (command id → key). */
  readonly shortcuts: Readonly<Record<string, string>>;
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
  readonly workMode?: WorkMode;
  readonly wheelZoom?: WheelZoomSetting;
  /** Replaces all of the user's keys. */
  readonly shortcuts?: Readonly<Record<string, string>>;
  readonly updates?: Partial<Settings['updates']>;
}

export const DEFAULT_SETTINGS: Settings = {
  schemaVersion: 1,
  language: 'system',
  theme: 'system',
  workMode: 'beginner',
  wheelZoom: 'normal',
  shortcuts: {},
  updates: { checkOnStartup: true, skippedVersion: null },
};

const THEMES: readonly ThemeSetting[] = ['system', 'light', 'dark'];
const WORK_MODES: readonly WorkMode[] = ['beginner', 'advanced'];
export const WHEEL_ZOOM_SETTINGS: readonly WheelZoomSetting[] = ['fine', 'normal', 'coarse'];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isLanguageSetting(value: unknown): value is LanguageSetting {
  return value === 'system' || isLanguage(value);
}

function isThemeSetting(value: unknown): value is ThemeSetting {
  return typeof value === 'string' && (THEMES as readonly string[]).includes(value);
}

function isWorkMode(value: unknown): value is WorkMode {
  return typeof value === 'string' && (WORK_MODES as readonly string[]).includes(value);
}

function isWheelZoom(value: unknown): value is WheelZoomSetting {
  return typeof value === 'string' && (WHEEL_ZOOM_SETTINGS as readonly string[]).includes(value);
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
    workMode: isWorkMode(input['workMode']) ? input['workMode'] : DEFAULT_SETTINGS.workMode,
    wheelZoom: isWheelZoom(input['wheelZoom']) ? input['wheelZoom'] : DEFAULT_SETTINGS.wheelZoom,
    shortcuts: sanitizeShortcuts(input['shortcuts']),
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
