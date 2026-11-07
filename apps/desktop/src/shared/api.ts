import type { EngineStatus } from './engine';
import type { Language } from './i18n';
import type { Settings, SettingsPatch } from './settings';
import type { UpdateCheckResult } from './update';

/** IPC channel names (renderer ↔ main). */
export const IPC = {
  appInfo: 'app:info',
  settingsGet: 'settings:get',
  settingsSet: 'settings:set',
  settingsChanged: 'settings:changed',
  engineStatus: 'engine:status',
  engineStatusChanged: 'engine:status-changed',
  engineRequestPort: 'engine:request-port',
  enginePort: 'engine:port',
  menuAction: 'menu:action',
  openExternal: 'app:open-external',
  updatesCheck: 'updates:check',
  updatesPending: 'updates:pending',
  updatesAvailable: 'updates:available',
} as const;

/** `window.postMessage` tag used by the preload to hand over the engine port. */
export const ENGINE_PORT_MESSAGE = 'ew:engine-port';

export interface AppInfo {
  readonly name: string;
  readonly version: string;
  readonly platform: string;
  readonly arch: string;
  readonly isPackaged: boolean;
  readonly versions: {
    readonly electron: string;
    readonly chrome: string;
    readonly node: string;
  };
  /** Preferred system languages, most preferred first (BCP 47 tags). */
  readonly systemLanguages: readonly string[];
}

/** Settings together with the language actually in use. */
export interface SettingsSnapshot {
  readonly settings: Settings;
  readonly language: Language;
}

/** Menu commands forwarded to the renderer. */
export type MenuAction =
  'settings' | 'new-project' | 'open-project' | 'check-updates' | 'command-palette';

export type Unsubscribe = () => void;

/** The API the preload script exposes as `window.ew`. */
export interface EwApi {
  readonly app: {
    info: () => Promise<AppInfo>;
    onMenuAction: (listener: (action: MenuAction) => void) => Unsubscribe;
    /** Opens an allow-listed link (this project on GitHub) in the browser. */
    openExternal: (url: string) => void;
  };
  readonly updates: {
    /** Checks GitHub for a newer release. `manual` also reports skipped versions. */
    check: (manual: boolean) => Promise<UpdateCheckResult>;
    /** Result of the automatic startup check, if it found an update. */
    pending: () => Promise<UpdateCheckResult | null>;
    onAvailable: (listener: (result: UpdateCheckResult) => void) => Unsubscribe;
  };
  readonly settings: {
    get: () => Promise<SettingsSnapshot>;
    set: (patch: SettingsPatch) => Promise<SettingsSnapshot>;
    onChanged: (listener: (snapshot: SettingsSnapshot) => void) => Unsubscribe;
  };
  readonly engine: {
    status: () => Promise<EngineStatus>;
    onStatus: (listener: (status: EngineStatus) => void) => Unsubscribe;
    /** Asks the main process to send a fresh engine port (see ENGINE_PORT_MESSAGE). */
    requestPort: () => void;
  };
}
