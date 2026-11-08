import { dialog, type BrowserWindow } from 'electron';

import type { FolderPurpose, ImportKind } from '@shared/api';
import type { Translate } from '@shared/i18n';

import { testHooksEnabled } from './paths';

/** Answers queued by end-to-end tests instead of showing native dialogs. */
const queued: (string | null)[] = [];

/** Test hook: the next dialog returns `answer` (a path, or `null` for "cancel"). */
export function queueDialogAnswer(answer: string | null): void {
  queued.push(answer);
}

function takeQueued(): { readonly answer: string | null } | null {
  if (!testHooksEnabled() || queued.length === 0) {
    return null;
  }
  return { answer: queued.shift() ?? null };
}

export async function chooseProjectFile(
  window: BrowserWindow | null,
  t: Translate,
): Promise<string | null> {
  const stub = takeQueued();
  if (stub !== null) {
    return stub.answer;
  }
  const options: Electron.OpenDialogOptions = {
    title: t('dialogs.openProjectTitle'),
    properties: ['openFile'],
    filters: [{ name: t('dialogs.projectFilter'), extensions: ['ewproj'] }],
  };
  const result =
    window === null
      ? await dialog.showOpenDialog(options)
      : await dialog.showOpenDialog(window, options);
  return result.canceled ? null : (result.filePaths[0] ?? null);
}

const FOLDER_TITLES = {
  location: 'dialogs.chooseFolderTitle',
  saveAs: 'dialogs.saveAsTitle',
  saveCopy: 'dialogs.saveCopyTitle',
  library: 'dialogs.chooseLibraryTitle',
} as const satisfies Record<FolderPurpose, Parameters<Translate>[0]>;

export async function chooseFolder(
  window: BrowserWindow | null,
  t: Translate,
  defaultPath: string,
  purpose: FolderPurpose,
): Promise<string | null> {
  const stub = takeQueued();
  if (stub !== null) {
    return stub.answer;
  }
  const options: Electron.OpenDialogOptions = {
    title: t(FOLDER_TITLES[purpose]),
    defaultPath,
    properties: ['openDirectory', 'createDirectory', 'promptToCreate'],
  };
  const result =
    window === null
      ? await dialog.showOpenDialog(options)
      : await dialog.showOpenDialog(window, options);
  return result.canceled ? null : (result.filePaths[0] ?? null);
}

const IMPORT_DIALOGS = {
  kicadProject: {
    title: 'dialogs.importKicadTitle',
    filter: 'dialogs.kicadProjectFilter',
    extensions: ['kicad_pro', 'kicad_sch', 'kicad_pcb'],
  },
  netlist: {
    title: 'dialogs.importNetlistTitle',
    filter: 'dialogs.netlistFilter',
    extensions: ['net', 'xml', 'csv', 'txt'],
  },
  kicadLibraryFile: {
    title: 'dialogs.importLibraryTitle',
    filter: 'dialogs.kicadLibraryFilter',
    extensions: ['kicad_sym', 'kicad_mod'],
  },
  kicadLibraryFolder: {
    title: 'dialogs.importLibraryFolderTitle',
    filter: null,
    extensions: [],
  },
} as const;

/** Asks for a file or folder to import. */
export async function chooseImport(
  window: BrowserWindow | null,
  t: Translate,
  kind: ImportKind,
): Promise<string | null> {
  const stub = takeQueued();
  if (stub !== null) {
    return stub.answer;
  }
  const spec = IMPORT_DIALOGS[kind];
  const options: Electron.OpenDialogOptions =
    spec.filter === null
      ? { title: t(spec.title), properties: ['openDirectory'] }
      : {
          title: t(spec.title),
          properties: ['openFile'],
          filters: [{ name: t(spec.filter), extensions: [...spec.extensions] }],
        };
  const result =
    window === null
      ? await dialog.showOpenDialog(options)
      : await dialog.showOpenDialog(window, options);
  return result.canceled ? null : (result.filePaths[0] ?? null);
}

export async function chooseArchiveFile(
  window: BrowserWindow | null,
  t: Translate,
  defaultPath: string,
): Promise<string | null> {
  const stub = takeQueued();
  if (stub !== null) {
    return stub.answer;
  }
  const options: Electron.SaveDialogOptions = {
    title: t('dialogs.archiveTitle'),
    defaultPath,
    filters: [{ name: t('dialogs.archiveFilter'), extensions: ['zip'] }],
  };
  const result =
    window === null
      ? await dialog.showSaveDialog(options)
      : await dialog.showSaveDialog(window, options);
  return result.canceled || result.filePath === '' ? null : result.filePath;
}
