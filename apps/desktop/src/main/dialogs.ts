import { dialog, type BrowserWindow } from 'electron';

import type { ExportKind, FolderPurpose, ImportKind } from '@shared/api';
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
  dxf: {
    title: 'dialogs.importDxfTitle',
    filter: 'dialogs.dxfFilter',
    extensions: ['dxf'],
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

/** Extensions of Gerber and drill files. */
export const GERBER_EXTENSIONS = [
  'gbr',
  'ger',
  'gtl',
  'gbl',
  'gts',
  'gbs',
  'gto',
  'gbo',
  'gtp',
  'gbp',
  'gta',
  'gba',
  'gm1',
  'gko',
  'gml',
  'g1',
  'g2',
  'g3',
  'g4',
  'g5',
  'g6',
  'drl',
  'xln',
  'exc',
  'txt',
] as const;

/** Asks for Gerber and drill files (several at once). */
export async function chooseGerbers(
  window: BrowserWindow | null,
  t: Translate,
): Promise<readonly string[] | null> {
  const stub = takeQueued();
  if (stub !== null) {
    return stub.answer === null ? null : stub.answer.split('\n').filter((p) => p !== '');
  }
  const options: Electron.OpenDialogOptions = {
    title: t('dialogs.gerberTitle'),
    properties: ['openFile', 'multiSelections'],
    filters: [
      { name: t('dialogs.gerberFilter'), extensions: [...GERBER_EXTENSIONS] },
      { name: t('dialogs.allFiles'), extensions: ['*'] },
    ],
  };
  const result =
    window === null
      ? await dialog.showOpenDialog(options)
      : await dialog.showOpenDialog(window, options);
  return result.canceled || result.filePaths.length === 0 ? null : result.filePaths;
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

const EXPORT_FILTERS = {
  netlist: ['dialogs.exportNetlist', 'net'],
  csv: ['dialogs.exportCsv', 'csv'],
  bom: ['dialogs.exportCsv', 'csv'],
  svg: ['dialogs.exportSvg', 'svg'],
  pdf: ['dialogs.exportPdf', 'pdf'],
} as const satisfies Record<ExportKind, readonly [Parameters<Translate>[0], string]>;

export async function chooseExportFile(
  window: BrowserWindow | null,
  t: Translate,
  kind: ExportKind,
  defaultPath: string,
): Promise<string | null> {
  const stub = takeQueued();
  if (stub !== null) {
    return stub.answer;
  }
  const [filter, extension] = EXPORT_FILTERS[kind];
  const options: Electron.SaveDialogOptions = {
    title: t('dialogs.exportTitle'),
    defaultPath,
    filters: [{ name: t(filter), extensions: [extension] }],
  };
  const result =
    window === null
      ? await dialog.showSaveDialog(options)
      : await dialog.showSaveDialog(window, options);
  return result.canceled || result.filePath === '' ? null : result.filePath;
}
