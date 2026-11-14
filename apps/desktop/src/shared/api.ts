import type { LibraryDownloadProgress, LibraryRelease } from './library';
import type { EngineStatus } from './engine';
import type { Language } from './i18n';
import type { RecentState } from './recent';
import type { Settings, SettingsPatch } from './settings';
import type { UpdateCheckResult } from './update';
import type { ProviderDownload, ProviderId, ProviderPage, ProviderStatus } from './providers';
import type {
  DeviceResult,
  DeviceStart,
  GitBranches,
  GitCommit,
  GithubAccount,
  GithubRepo,
  GithubStatus,
  GitResult,
  GitStatus,
  PublishTarget,
  PullResult,
} from './github';

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
  dialogChooseImport: 'dialog:choose-import',
  dialogChooseGerbers: 'dialog:choose-gerbers',
  exportSave: 'export:save',
  projectsLocation: 'projects:default-location',
  recentGet: 'recent:get',
  recentAdd: 'recent:add',
  recentRemove: 'recent:remove',
  recentClear: 'recent:clear',
  recentSetLocation: 'recent:set-location',
  recentChanged: 'recent:changed',
  showItem: 'shell:show-item',
  openFile: 'shell:open-file',
  windowState: 'window:state',
  closeRequested: 'window:close-requested',
  closeConfirmed: 'window:close-confirmed',
  openPath: 'app:open-path',
  pendingOpen: 'app:pending-open',
  libraryLatest: 'library:latest',
  libraryRelease: 'library:release',
  libraryDownload: 'library:download',
  libraryProgress: 'library:download-progress',
  dialogChooseModel: 'dialog:choose-model',
  exportSaveImage: 'export:save-image',
  view3dOpen: 'view3d:open',
  syncPublish: 'sync:publish',
  syncMessage: 'sync:message',
  providersOpen: 'providers:open',
  providersOpenExternal: 'providers:open-external',
  providersSignOut: 'providers:sign-out',
  providersStatus: 'providers:status',
  providersDownload: 'providers:download',
  dialogChooseDownload: 'dialog:choose-download',
  githubStatus: 'github:status',
  githubChanged: 'github:changed',
  githubSignInToken: 'github:sign-in-token',
  githubDeviceStart: 'github:device-start',
  githubDeviceWait: 'github:device-wait',
  githubDeviceCancel: 'github:device-cancel',
  githubSignOut: 'github:sign-out',
  githubRepos: 'github:repos',
  githubOpenPage: 'github:open-page',
  gitStatus: 'git:status',
  gitPublish: 'git:publish',
  gitCommit: 'git:commit',
  gitPush: 'git:push',
  gitFetch: 'git:fetch',
  gitPull: 'git:pull',
  gitBranches: 'git:branches',
  gitCheckout: 'git:checkout',
  gitLog: 'git:log',
  gitClone: 'git:clone',
} as const;

/** 3D model file formats the 3D view exports. */
export type ModelFormat = 'step' | 'glb' | 'vrml';

export function isModelFormat(value: unknown): value is ModelFormat {
  return value === 'step' || value === 'glb' || value === 'vrml';
}

/**
 * A message shared between the app's windows: the selection, or a new
 * window asking for the current state.
 */
export interface SyncMessage {
  readonly topic: 'selection' | 'request';
  readonly payload: unknown;
}

export function isSyncMessage(value: unknown): value is SyncMessage {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const message = value as Record<string, unknown>;
  return (
    (message['topic'] === 'selection' || message['topic'] === 'request') && 'payload' in message
  );
}

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
  | 'import-kicad'
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
  | 'command-palette'
  | 'library';

/** Why a folder is chosen (sets the dialog title). */
export type FolderPurpose = 'location' | 'saveAs' | 'saveCopy' | 'library';

/** What an export writes. */
export type ExportKind = 'netlist' | 'csv' | 'bom' | 'svg' | 'pdf';

export function isExportKind(value: unknown): value is ExportKind {
  return (
    value === 'netlist' || value === 'csv' || value === 'bom' || value === 'svg' || value === 'pdf'
  );
}

/** What kind of file an import dialog asks for. */
export type ImportKind =
  'kicadProject' | 'netlist' | 'kicadLibraryFile' | 'kicadLibraryFolder' | 'dxf';

export function isImportKind(value: unknown): value is ImportKind {
  return (
    value === 'kicadProject' ||
    value === 'netlist' ||
    value === 'kicadLibraryFile' ||
    value === 'kicadLibraryFolder' ||
    value === 'dxf'
  );
}

export function isFolderPurpose(value: unknown): value is FolderPurpose {
  return value === 'location' || value === 'saveAs' || value === 'saveCopy' || value === 'library';
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
    /** Lets the user pick a file (or folder) to import. */
    chooseImport: (kind: ImportKind) => Promise<string | null>;
    /** Lets the user pick Gerber and drill files for the Gerber viewer. */
    chooseGerbers: () => Promise<readonly string[] | null>;
    /**
     * Asks where to save an export and writes it: text as it is, a print
     * document (HTML) as PDF. Returns the path, or `null` if cancelled.
     */
    saveExport: (kind: ExportKind, defaultPath: string, content: string) => Promise<string | null>;
    /** Asks where to save a 3D model (the engine writes it). */
    chooseModelFile: (format: ModelFormat, defaultPath: string) => Promise<string | null>;
    /** Asks where to save a PNG image and writes it. */
    saveImage: (defaultPath: string, data: Uint8Array) => Promise<string | null>;
  };
  readonly view3d: {
    /** Shows the 3D view of a board in its own window. */
    open: (doc: string, name: string) => void;
  };
  readonly providers: {
    /** Opens a provider's website in its provider window. */
    open: (id: ProviderId, page: ProviderPage, query?: string) => void;
    /** Opens a provider's website in the user's browser. */
    openExternal: (id: ProviderId, page: ProviderPage, query?: string) => void;
    /** Deletes the provider window's sign-in and stored data. */
    signOut: (id: ProviderId) => Promise<void>;
    status: () => Promise<readonly ProviderStatus[]>;
    /** A file downloaded in a provider window. */
    onDownload: (listener: (download: ProviderDownload) => void) => Unsubscribe;
    /** Lets the user pick a downloaded part file to import. */
    chooseDownload: () => Promise<string | null>;
  };
  readonly github: {
    /** The account (checked with GitHub once per start). */
    status: () => Promise<GithubStatus>;
    /** Signs in with a personal access token. */
    signInWithToken: (token: string) => Promise<GitResult<GithubAccount>>;
    /** Starts the device flow: the user enters the code on GitHub. */
    deviceStart: () => Promise<GitResult<DeviceStart>>;
    /** Waits for the device flow to end. */
    deviceWait: () => Promise<DeviceResult>;
    deviceCancel: () => void;
    signOut: () => Promise<void>;
    /** The account's repositories, newest first. */
    repos: () => Promise<GitResult<readonly GithubRepo[]>>;
    /** Opens a page of a repository (`owner/name`) or the device sign-in in the browser. */
    openPage: (page: 'repo' | 'device' | 'tokens', fullName?: string) => void;
    /** The account changed (sign-in or sign-out in any window). */
    onChanged: (listener: (status: GithubStatus) => void) => Unsubscribe;
  };
  /** Git for the open project (`manifestPath` must be the open project). */
  readonly git: {
    status: (manifestPath: string) => Promise<GitResult<GitStatus>>;
    /** Puts the project on GitHub (new or existing empty repository). */
    publish: (manifestPath: string, target: PublishTarget) => Promise<GitResult<GitStatus>>;
    /** Commits the changed files (all without `paths`). */
    commit: (
      manifestPath: string,
      message: string,
      paths?: readonly string[],
    ) => Promise<GitResult<GitStatus>>;
    push: (manifestPath: string) => Promise<GitResult<GitStatus>>;
    fetch: (manifestPath: string) => Promise<GitResult<GitStatus>>;
    pull: (manifestPath: string) => Promise<GitResult<PullResult>>;
    branches: (manifestPath: string) => Promise<GitResult<GitBranches>>;
    checkout: (
      manifestPath: string,
      branch: string,
      create: boolean,
    ) => Promise<GitResult<GitStatus>>;
    log: (manifestPath: string) => Promise<GitResult<readonly GitCommit[]>>;
    /** Clones a repository into a folder; returns the project manifest. */
    clone: (fullName: string, parent: string) => Promise<GitResult<string>>;
  };
  readonly sync: {
    /** Sends a message to the app's other windows. */
    publish: (message: SyncMessage) => void;
    onMessage: (listener: (message: SyncMessage) => void) => Unsubscribe;
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
    /** Opens a generated output file (PDF, SVG, CSV, …) with its application. */
    openFile: (path: string) => void;
    /** A project passed on the command line or by the OS, taken once. */
    takePendingOpen: () => Promise<string | null>;
    /** Projects opened from the "Open Recent" menu, the OS or a second launch. */
    onOpenPath: (listener: (path: string) => void) => Unsubscribe;
  };
  readonly library: {
    /** The newest library release on GitHub (`null`: none published yet). */
    latest: () => Promise<LibraryRelease | null>;
    /** A library release by tag. */
    release: (tag: string) => Promise<LibraryRelease>;
    /** Downloads a pack and returns its local path (checked against the manifest). */
    download: (tag: string, file: string) => Promise<string>;
    onProgress: (listener: (progress: LibraryDownloadProgress) => void) => Unsubscribe;
  };
  readonly window: {
    setState: (state: WindowState) => void;
    /** The user wants to close the window while a project is open. */
    onCloseRequested: (listener: () => void) => Unsubscribe;
    /** The project is saved or closed: the window may close now. */
    confirmClose: () => void;
  };
}
