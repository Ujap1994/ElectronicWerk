import {
  createContext,
  use,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';

import type { MessageKey } from '@shared/i18n';
import {
  dirName,
  isEngineEvent,
  joinPath,
  sanitizeFolderName,
  type ArchiveReport,
  type BoardOptions,
  type DocumentKind,
  type FileEntry,
  type ProjectSnapshot,
  type TemplateId,
  type TemplateInfo,
  type Units,
  type ValidationReport,
} from '@shared/project';

import { EngineError } from '../engine-client';
import { useAppState } from '../state';
import { errorMessage } from './labels';

/** Opening or saving a large project takes longer than a normal call. */
const LONG_CALL_MS = 120_000;
const NOTICE_MS = 6_000;

export interface Notice {
  readonly id: number;
  readonly text: string;
  readonly kind: 'info' | 'error';
}

export type CloseChoice = 'save' | 'discard' | 'cancel';

export interface NewProjectOptions {
  readonly location: string;
  readonly name: string;
  readonly template: TemplateId;
  readonly board: BoardOptions | null;
  readonly units: Units;
}

/** Dialogs the project flow needs (rendered by the workspace). */
export interface ProjectPrompts {
  readonly close: { readonly name: string; readonly answer: (choice: CloseChoice) => void } | null;
  readonly recovery: ProjectSnapshot | null;
  readonly remove: { readonly id: string; readonly name: string } | null;
  readonly validation: ValidationReport | 'running' | null;
  readonly wizard: boolean;
}

export interface ProjectApi {
  readonly project: ProjectSnapshot | null;
  readonly files: readonly FileEntry[];
  readonly templates: readonly TemplateInfo[];
  readonly busy: MessageKey | null;
  readonly notices: readonly Notice[];
  readonly prompts: ProjectPrompts;
  readonly dismissNotice: (id: number) => void;
  readonly newProject: () => void;
  readonly closeWizard: () => void;
  readonly createProject: (options: NewProjectOptions) => Promise<boolean>;
  readonly openProject: (manifestPath?: string) => Promise<void>;
  readonly save: () => Promise<boolean>;
  readonly saveAs: () => Promise<void>;
  readonly saveCopy: () => Promise<void>;
  readonly archive: () => Promise<void>;
  readonly closeProject: () => Promise<boolean>;
  readonly undo: () => void;
  readonly redo: () => void;
  readonly validate: () => void;
  readonly closeValidation: () => void;
  readonly answerRecovery: (restore: boolean) => Promise<void>;
  readonly setProperties: (patch: Readonly<Record<string, unknown>>) => Promise<boolean>;
  readonly setDocumentProperties: (
    doc: string,
    props: Readonly<Record<string, unknown>>,
  ) => Promise<boolean>;
  readonly addDocument: (
    kind: DocumentKind,
    name: string,
    parent?: string,
  ) => Promise<string | null>;
  readonly renameDocument: (doc: string, name: string) => Promise<boolean>;
  readonly requestRemove: (doc: string) => void;
  readonly answerRemove: (confirmed: boolean) => Promise<void>;
  readonly documentProperties: (doc: string) => Promise<Record<string, unknown> | null>;
  readonly resolveText: (text: string, doc?: string) => Promise<string>;
  readonly showInFolder: (relativePath?: string) => void;
}

const ProjectContext = createContext<ProjectApi | null>(null);

export function useProject(): ProjectApi {
  const api = use(ProjectContext);
  if (api === null) {
    throw new Error('useProject must be used inside <ProjectProvider>');
  }
  return api;
}

/** `true` if keyboard focus is in a text field (which has its own undo). */
function focusInTextField(): boolean {
  const element = document.activeElement;
  return (
    element instanceof HTMLTextAreaElement ||
    (element instanceof HTMLInputElement &&
      !['checkbox', 'radio', 'button', 'submit'].includes(element.type))
  );
}

const EMPTY_PROMPTS: ProjectPrompts = {
  close: null,
  recovery: null,
  remove: null,
  validation: null,
  wizard: false,
};

export function ProjectProvider({ children }: { readonly children: ReactNode }): ReactNode {
  const { engine, engineStatus, t, appInfo } = useAppState();
  const [project, setProject] = useState<ProjectSnapshot | null>(null);
  const [files, setFiles] = useState<readonly FileEntry[]>([]);
  const [templates, setTemplates] = useState<readonly TemplateInfo[]>([]);
  const [busy, setBusy] = useState<MessageKey | null>(null);
  const [notices, setNotices] = useState<readonly Notice[]>([]);
  const [prompts, setPrompts] = useState<ProjectPrompts>(EMPTY_PROMPTS);
  const nextNoticeRef = useRef(1);
  // The latest snapshot for callbacks that outlive a render (events, close).
  const projectRef = useRef<ProjectSnapshot | null>(null);
  const generationRef = useRef(engineStatus.generation);

  useEffect(() => {
    projectRef.current = project;
  }, [project]);

  const notify = useCallback((text: string, kind: Notice['kind'] = 'info') => {
    const id = nextNoticeRef.current;
    nextNoticeRef.current += 1;
    setNotices((current) => [...current.slice(-3), { id, text, kind }]);
    if (kind === 'info') {
      setTimeout(() => {
        setNotices((current) => current.filter((notice) => notice.id !== id));
      }, NOTICE_MS);
    }
  }, []);

  const dismissNotice = useCallback((id: number) => {
    setNotices((current) => current.filter((notice) => notice.id !== id));
  }, []);

  const fail = useCallback(
    (error: unknown) => {
      notify(errorMessage(t, error), 'error');
    },
    [notify, t],
  );

  /** Runs an engine call with a busy indicator; reports errors as notices. */
  const run = useCallback(
    async <T,>(label: MessageKey | null, task: () => Promise<T>): Promise<T | null> => {
      if (label !== null) {
        setBusy(label);
      }
      try {
        return await task();
      } catch (error) {
        fail(error);
        return null;
      } finally {
        if (label !== null) {
          setBusy(null);
        }
      }
    },
    [fail],
  );

  const refresh = useCallback(async () => {
    if (projectRef.current === null) {
      return;
    }
    try {
      const [snapshot, list] = await Promise.all([
        engine.call<ProjectSnapshot>('project.info'),
        engine.call<FileEntry[]>('project.files'),
      ]);
      setProject(snapshot);
      setFiles(list);
    } catch {
      // The engine may be restarting; the restart handler reloads.
    }
  }, [engine]);

  const adopt = useCallback(
    async (snapshot: ProjectSnapshot) => {
      setProject(snapshot);
      projectRef.current = snapshot;
      setFiles(await engine.call<FileEntry[]>('project.files').catch(() => []));
      void window.ew.projects.addRecent(snapshot.manifestPath, snapshot.name);
      if (snapshot.recovery !== null) {
        setPrompts((current) => ({ ...current, recovery: snapshot }));
      }
    },
    [engine],
  );

  // Engine events: every change refreshes the snapshot.
  useEffect(() => {
    let scheduled = false;
    return engine.onEvent((event) => {
      if (!isEngineEvent(event) || scheduled) {
        return;
      }
      scheduled = true;
      queueMicrotask(() => {
        scheduled = false;
        void refresh();
      });
    });
  }, [engine, refresh]);

  // Templates for the wizard (once the engine is connected).
  useEffect(() => {
    if (engineStatus.state !== 'running') {
      return;
    }
    let cancelled = false;
    void engine
      .whenConnected()
      .then(() => engine.call<TemplateInfo[]>('project.templates'))
      .then((list) => {
        if (!cancelled) {
          setTemplates(list);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [engine, engineStatus.state, engineStatus.generation]);

  // After an engine restart the project is reopened and the journal replayed,
  // so no work is lost.
  useEffect(() => {
    if (engineStatus.state !== 'running' || engineStatus.generation === generationRef.current) {
      return;
    }
    generationRef.current = engineStatus.generation;
    const open = projectRef.current;
    if (open === null) {
      return;
    }
    void (async () => {
      try {
        await engine.whenConnected();
        let snapshot = await engine.call<ProjectSnapshot>(
          'project.open',
          { path: open.manifestPath },
          LONG_CALL_MS,
        );
        if (snapshot.recovery !== null) {
          snapshot = await engine.call<ProjectSnapshot>('project.recover', undefined, LONG_CALL_MS);
        }
        setProject(snapshot);
        projectRef.current = snapshot;
        notify(t('project.restoredAfterRestart'));
      } catch (error) {
        fail(error);
      }
    })();
  }, [engine, engineStatus.state, engineStatus.generation, notify, fail, t]);

  // Window title, menus and the close prompt follow the project state.
  useEffect(() => {
    window.ew.window.setState({
      projectOpen: project !== null,
      projectName: project?.name ?? null,
      manifestPath: project?.manifestPath ?? null,
      dirty: project?.history.dirty ?? false,
      canUndo: project?.history.canUndo ?? false,
      canRedo: project?.history.canRedo ?? false,
    });
  }, [project]);

  const askClose = useCallback(
    (name: string) =>
      new Promise<CloseChoice>((resolve) => {
        setPrompts((current) => ({
          ...current,
          close: {
            name,
            answer: (choice) => {
              setPrompts((latest) => ({ ...latest, close: null }));
              resolve(choice);
            },
          },
        }));
      }),
    [],
  );

  const save = useCallback(async (): Promise<boolean> => {
    if (projectRef.current === null) {
      return false;
    }
    const result = await run('project.saving', () =>
      engine.call('project.save', undefined, LONG_CALL_MS),
    );
    if (result === null) {
      return false;
    }
    notify(t('project.saved'));
    await refresh();
    return true;
  }, [engine, notify, refresh, run, t]);

  /**
   * Closes the open project, asking about unsaved changes. Returns `false`
   * if the user cancelled. With `forWindow`, an unreachable engine does not
   * block closing (the autosave journal keeps the work).
   */
  const closeInternal = useCallback(
    async (forWindow: boolean): Promise<boolean> => {
      const current = projectRef.current;
      if (current === null) {
        return true;
      }
      let discard = false;
      if (current.history.dirty && current.recovery === null) {
        const choice = await askClose(current.name);
        if (choice === 'cancel') {
          return false;
        }
        if (choice === 'save' && !(await save())) {
          return false;
        }
        discard = choice === 'discard';
      }
      try {
        await engine.call('project.close', { discard });
      } catch (error) {
        if (!forWindow && !(error instanceof EngineError && error.code === 'noProject')) {
          fail(error);
        }
      }
      projectRef.current = null;
      setProject(null);
      setFiles([]);
      return true;
    },
    [askClose, engine, fail, save],
  );

  const closeProject = useCallback(() => closeInternal(false), [closeInternal]);

  const openProject = useCallback(
    async (manifestPath?: string) => {
      const path = manifestPath ?? (await window.ew.dialogs.openProject());
      if (path === null) {
        return;
      }
      if (projectRef.current?.manifestPath === path) {
        return;
      }
      if (!(await closeInternal(false))) {
        return;
      }
      const snapshot = await run('project.opening', () =>
        engine.call<ProjectSnapshot>('project.open', { path }, LONG_CALL_MS),
      );
      if (snapshot !== null) {
        await adopt(snapshot);
      }
    },
    [adopt, closeInternal, engine, run],
  );

  const newProject = useCallback(() => {
    void closeInternal(false).then((closed) => {
      if (closed) {
        setPrompts((current) => ({ ...current, wizard: true }));
      }
    });
  }, [closeInternal]);

  const closeWizard = useCallback(() => {
    setPrompts((current) => ({ ...current, wizard: false }));
  }, []);

  const createProject = useCallback(
    async (options: NewProjectOptions): Promise<boolean> => {
      const path = joinPath(options.location, sanitizeFolderName(options.name), appInfo.platform);
      const snapshot = await run('project.creating', () =>
        engine.call<ProjectSnapshot>(
          'project.create',
          {
            path,
            name: options.name.trim(),
            template: options.template,
            units: options.units,
            names: {
              rootSchematic: t('defaults.rootSheet'),
              library: t('defaults.library'),
              rules: t('defaults.rules'),
              outputJob: t('defaults.outputJob'),
              outputs: {
                gerber: t('defaults.outputGerber'),
                drill: t('defaults.outputDrill'),
                bom: t('defaults.outputBom'),
                pickAndPlace: t('defaults.outputPickAndPlace'),
                pdf: t('defaults.outputPdf'),
                step: t('defaults.outputStep'),
              },
            },
            ...(options.board === null ? {} : { board: options.board }),
          },
          LONG_CALL_MS,
        ),
      );
      if (snapshot === null) {
        return false;
      }
      void window.ew.projects.setLastLocation(options.location);
      setPrompts((current) => ({ ...current, wizard: false }));
      await adopt(snapshot);
      return true;
    },
    [adopt, appInfo.platform, engine, run, t],
  );

  const saveTo = useCallback(
    async (purpose: 'saveAs' | 'saveCopy') => {
      const current = projectRef.current;
      if (current === null) {
        return;
      }
      const folder = await window.ew.dialogs.chooseFolder(dirName(current.path), purpose);
      if (folder === null) {
        return;
      }
      const target = joinPath(folder, sanitizeFolderName(current.name), appInfo.platform);
      if (purpose === 'saveAs') {
        const snapshot = await run('project.saving', () =>
          engine.call<ProjectSnapshot>('project.saveAs', { path: target }, LONG_CALL_MS),
        );
        if (snapshot !== null) {
          await adopt(snapshot);
          notify(t('project.savedAs', { path: snapshot.path }));
        }
      } else {
        const result = await run('project.saving', () =>
          engine.call<{ manifestPath: string }>('project.saveCopy', { path: target }, LONG_CALL_MS),
        );
        if (result !== null) {
          notify(t('project.copySaved', { path: result.manifestPath }));
        }
      }
    },
    [adopt, appInfo.platform, engine, notify, run, t],
  );

  const saveAs = useCallback(() => saveTo('saveAs'), [saveTo]);
  const saveCopy = useCallback(() => saveTo('saveCopy'), [saveTo]);

  const archive = useCallback(async () => {
    const current = projectRef.current;
    if (current === null) {
      return;
    }
    const suggestion = joinPath(
      dirName(current.path),
      `${sanitizeFolderName(current.name)}.zip`,
      appInfo.platform,
    );
    const target = await window.ew.dialogs.saveArchive(suggestion);
    if (target === null) {
      return;
    }
    const report = await run('project.archiving', () =>
      engine.call<ArchiveReport>('project.archive', { path: target }, LONG_CALL_MS),
    );
    if (report !== null) {
      notify(t('project.archived', { path: report.path, files: report.files }));
    }
  }, [appInfo.platform, engine, notify, run, t]);

  const history = useCallback(
    (method: 'edit.undo' | 'edit.redo') => {
      if (focusInTextField()) {
        // Text fields keep their own undo history.
        // eslint-disable-next-line @typescript-eslint/no-deprecated -- the only way to trigger native text undo from a menu command
        document.execCommand(method === 'edit.undo' ? 'undo' : 'redo');
        return;
      }
      if (projectRef.current !== null) {
        void run(null, () => engine.call(method));
      }
    },
    [engine, run],
  );

  const undo = useCallback(() => {
    history('edit.undo');
  }, [history]);
  const redo = useCallback(() => {
    history('edit.redo');
  }, [history]);

  const validate = useCallback(() => {
    if (projectRef.current === null) {
      return;
    }
    setPrompts((current) => ({ ...current, validation: 'running' }));
    void run(null, () => engine.call<ValidationReport>('project.validate')).then((report) => {
      setPrompts((current) => ({ ...current, validation: report }));
    });
  }, [engine, run]);

  const closeValidation = useCallback(() => {
    setPrompts((current) => ({ ...current, validation: null }));
  }, []);

  const answerRecovery = useCallback(
    async (restore: boolean) => {
      setPrompts((current) => ({ ...current, recovery: null }));
      const snapshot = await run('project.opening', () =>
        engine.call<ProjectSnapshot>(
          restore ? 'project.recover' : 'project.discardRecovery',
          undefined,
          LONG_CALL_MS,
        ),
      );
      if (snapshot !== null) {
        setProject(snapshot);
        notify(t(restore ? 'recovery.restored' : 'recovery.discarded'));
      } else {
        await refresh();
      }
    },
    [engine, notify, refresh, run, t],
  );

  const edit = useCallback(
    async (method: string, params: unknown): Promise<boolean> =>
      (await run(null, () => engine.call(method, params))) !== null,
    [engine, run],
  );

  const setProperties = useCallback(
    (patch: Readonly<Record<string, unknown>>) => edit('project.setProperties', patch),
    [edit],
  );

  const setDocumentProperties = useCallback(
    (doc: string, props: Readonly<Record<string, unknown>>) =>
      edit('document.setProperties', { doc, props }),
    [edit],
  );

  const addDocument = useCallback(
    async (kind: DocumentKind, name: string, parent?: string): Promise<string | null> => {
      const result = await run(null, () =>
        engine.call<{ id: string }>('document.add', {
          kind,
          name,
          ...(parent === undefined ? {} : { parent }),
        }),
      );
      return result?.id ?? null;
    },
    [engine, run],
  );

  const renameDocument = useCallback(
    (doc: string, name: string) => edit('document.rename', { doc, name }),
    [edit],
  );

  const requestRemove = useCallback((doc: string) => {
    const document = projectRef.current?.documents.find((entry) => entry.id === doc);
    if (document !== undefined) {
      setPrompts((current) => ({ ...current, remove: { id: doc, name: document.name } }));
    }
  }, []);

  const answerRemove = useCallback(
    async (confirmed: boolean) => {
      const target = prompts.remove;
      setPrompts((current) => ({ ...current, remove: null }));
      if (confirmed && target !== null) {
        await edit('document.remove', { doc: target.id });
      }
    },
    [edit, prompts.remove],
  );

  const documentProperties = useCallback(
    async (doc: string) =>
      run(null, () => engine.call<Record<string, unknown>>('document.properties', { doc })),
    [engine, run],
  );

  const resolveText = useCallback(
    async (text: string, doc?: string): Promise<string> => {
      try {
        const result = await engine.call<{ text: string }>('project.resolveText', {
          text,
          ...(doc === undefined ? {} : { doc }),
        });
        return result.text;
      } catch {
        return text;
      }
    },
    [engine],
  );

  const showInFolder = useCallback(
    (relativePath?: string) => {
      const current = projectRef.current;
      if (current === null) {
        return;
      }
      const target =
        relativePath === undefined
          ? current.manifestPath
          : joinPath(
              current.path,
              appInfo.platform === 'win32' ? relativePath.replaceAll('/', '\\') : relativePath,
              appInfo.platform,
            );
      window.ew.projects.showInFolder(target);
    },
    [appInfo.platform],
  );

  // The main process asks before the window closes while a project is open.
  useEffect(
    () =>
      window.ew.window.onCloseRequested(() => {
        void closeInternal(true).then((closed) => {
          if (closed) {
            window.ew.window.confirmClose();
          }
        });
      }),
    [closeInternal],
  );

  // Projects handed over by the OS, a second launch or "Open Recent".
  useEffect(() => {
    const unsubscribe = window.ew.projects.onOpenPath((path) => {
      void openProject(path);
    });
    void window.ew.projects.takePendingOpen().then((path) => {
      if (path !== null) {
        void engine.whenConnected().then(() => openProject(path));
      }
    });
    return unsubscribe;
  }, [engine, openProject]);

  const value = useMemo<ProjectApi>(
    () => ({
      project,
      files,
      templates,
      busy,
      notices,
      prompts,
      dismissNotice,
      newProject,
      closeWizard,
      createProject,
      openProject,
      save,
      saveAs,
      saveCopy,
      archive,
      closeProject,
      undo,
      redo,
      validate,
      closeValidation,
      answerRecovery,
      setProperties,
      setDocumentProperties,
      addDocument,
      renameDocument,
      requestRemove,
      answerRemove,
      documentProperties,
      resolveText,
      showInFolder,
    }),
    [
      project,
      files,
      templates,
      busy,
      notices,
      prompts,
      dismissNotice,
      newProject,
      closeWizard,
      createProject,
      openProject,
      save,
      saveAs,
      saveCopy,
      archive,
      closeProject,
      undo,
      redo,
      validate,
      closeValidation,
      answerRecovery,
      setProperties,
      setDocumentProperties,
      addDocument,
      renameDocument,
      requestRemove,
      answerRemove,
      documentProperties,
      resolveText,
      showInFolder,
    ],
  );

  return <ProjectContext value={value}>{children}</ProjectContext>;
}
