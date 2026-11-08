import type { EngineStatus } from './engine';
import type { Language } from './i18n';
import type { RecentState } from './recent';
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
  dialogOpenProject: 'dialog:open-project',
  dialogChooseFolder: 'dialog:choose-folder',
  dialogSaveArchive: 'dialog:save-archive',
  projectsLocation: 'projects:default-location',
  recentGet: 'recent:get',
  recentAdd: 'recent:add',
  recentRemove: 'recent:remove',
  recentClear: 'recent:clear',
  recentSetLocation: 'recent:set-location',
  recentChanged: 'recent:changed',
  showItem: 'shell:show-item',
  windowState: 'window:state',
  closeRequested: 'window:close-requested',
  closeConfirmed: 'window:close-confirmed',
  openPath: 'app:open-path',
  pendingOpen: 'app:pending-open',
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
  | 'settings'
  | 'new-project'
  | 'open-project'
  | 'save'
  | 'save-as'
  | 'save-copy'
  | 'archive'
  | 'close-project'
  | 'undo'
  | 'redo'
  | 'project-properties'
  | 'validate'
  | 'check-updates'
  | 'command-palette';

/** Why a folder is chosen (sets the dialog title). */
export type FolderPurpose = 'location' | 'saveAs' | 'saveCopy';

export function isFolderPurpose(value: unknown): value is FolderPurpose {
  return value === 'location' || value === 'saveAs' || value === 'saveCopy';
}

/** What the window shows; drives the title, menus and the close prompt. */
export interface WindowState {
  readonly projectOpen: boolean;
  readonly projectName: string | null;
  readonly manifestPath: string | null;
  readonly dirty: boolean;
  readonly canUndo: boolean;
  readonly canRedo: boolean;
}

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
  readonly dialogs: {
    /** Lets the user pick a `.ewproj` file. */
    openProject: () => Promise<string | null>;
    /** Lets the user pick (or create) a folder. */
    chooseFolder: (defaultPath: string, purpose: FolderPurpose) => Promise<string | null>;
    /** Asks where to save a `.zip` archive. */
    saveArchive: (defaultPath: string) => Promise<string | null>;
  };
  readonly projects: {
    /** Default folder for new projects. */
    defaultLocation: () => Promise<string>;
    recent: () => Promise<RecentState>;
    addRecent: (manifestPath: string, name: string) => Promise<RecentState>;
    removeRecent: (manifestPath: string) => Promise<RecentState>;
    clearRecent: () => Promise<RecentState>;
    setLastLocation: (folder: string) => Promise<RecentState>;
    onRecentChanged: (listener: (state: RecentState) => void) => Unsubscribe;
    /** Reveals a file or folder in the system file manager. */
    showInFolder: (path: string) => void;
    /** A project passed on the command line or by the OS, taken once. */
    takePendingOpen: () => Promise<string | null>;
    /** Projects opened from the "Open Recent" menu, the OS or a second launch. */
    onOpenPath: (listener: (path: string) => void) => Unsubscribe;
  };
  readonly window: {
    setState: (state: WindowState) => void;
    /** The user wants to close the window while a project is open. */
    onCloseRequested: (listener: () => void) => Unsubscribe;
    /** The project is saved or closed: the window may close now. */
    confirmClose: () => void;
  };
}
