import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from 'react';

import { DOCS_URL, PROJECT_URL } from '@shared/links';

import type { Command } from './commands';
import { CommandPalette } from './components/CommandPalette';
import { ProjectDialogs } from './components/ProjectDialogs';
import { ProjectWorkspace } from './components/ProjectWorkspace';
import { SettingsDialog } from './components/SettingsDialog';
import { StartScreen } from './components/StartScreen';
import { StatusBar } from './components/StatusBar';
import { UpdateDialog, type UpdateDialogState } from './components/UpdateDialog';
import { LibraryManager } from './library/LibraryManager';
import { useProject } from './project/ProjectProvider';
import { activeBoard } from './board/stores';
import { outputCommands } from './outputs/stores';
import { open3dCommand } from './view3d/stores';
import { PartsHost } from './parts/PartsHost';
import { openOnlineParts } from './parts/stores';
import { activeSheet } from './schematic/stores';
import { useAppState } from './state';

export function App(): ReactNode {
  const { updateSettings, t } = useAppState();
  const {
    project,
    newProject,
    openProject,
    startKicadImport,
    importNetlist,
    save,
    saveAs,
    saveCopy,
    archive,
    closeProject,
    undo,
    redo,
    validate,
    addDocument,
    showInFolder,
  } = useProject();
  const [projectView, setProjectView] = useState(0);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [libraryOpen, setLibraryOpen] = useState(false);
  const openLibrary = useCallback(() => {
    setLibraryOpen(true);
  }, []);
  const closeLibrary = useCallback(() => {
    setLibraryOpen(false);
  }, []);
  const [update, setUpdate] = useState<UpdateDialogState | null>(null);

  const checkForUpdates = useCallback(() => {
    setSettingsOpen(false);
    setUpdate({ kind: 'checking' });
    void window.ew.updates.check(true).then((result) => {
      setUpdate({ kind: 'result', result });
    });
  }, []);

  // Menu commands from the main process.
  useEffect(
    () =>
      window.ew.app.onMenuAction((action) => {
        switch (action) {
          case 'settings':
            setSettingsOpen(true);
            break;
          case 'check-updates':
            checkForUpdates();
            break;
          case 'command-palette':
            setPaletteOpen(true);
            break;
          case 'library':
            setLibraryOpen(true);
            break;
          case 'new-project':
            newProject();
            break;
          case 'open-project':
            void openProject();
            break;
          case 'import-kicad':
            void startKicadImport();
            break;
          case 'save':
            void save();
            break;
          case 'save-as':
            void saveAs();
            break;
          case 'save-copy':
            void saveCopy();
            break;
          case 'archive':
            void archive();
            break;
          case 'close-project':
            void closeProject();
            break;
          case 'undo':
            undo();
            break;
          case 'redo':
            redo();
            break;
          case 'validate':
            validate();
            break;
          case 'project-properties':
            setProjectView((count) => count + 1);
            break;
        }
      }),
    [
      startKicadImport,
      archive,
      checkForUpdates,
      closeProject,
      newProject,
      openProject,
      redo,
      save,
      saveAs,
      saveCopy,
      undo,
      validate,
    ],
  );

  // The automatic startup check reports a new release (now or before the UI was ready).
  useEffect(() => {
    const unsubscribe = window.ew.updates.onAvailable((result) => {
      setUpdate({ kind: 'result', result });
    });
    void window.ew.updates.pending().then((result) => {
      if (result?.status === 'available') {
        setUpdate({ kind: 'result', result });
      }
    });
    return unsubscribe;
  }, []);

  // Alternative palette shortcut (Cmd/Ctrl+Shift+P); Cmd/Ctrl+K is a menu accelerator.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === 'p') {
        event.preventDefault();
        setPaletteOpen(true);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
    };
  }, []);

  const sheet = useSyncExternalStore(activeSheet.subscribe, activeSheet.get);
  const boardCommands = useSyncExternalStore(activeBoard.subscribe, activeBoard.get);
  const outputs = useSyncExternalStore(outputCommands.subscribe, outputCommands.get);
  const open3d = useSyncExternalStore(open3dCommand.subscribe, open3dCommand.get);
  const commands = useMemo<Command[]>(
    () => [
      ...(sheet === null
        ? []
        : ([
            { id: 'add-symbol', label: 'commands.addSymbol', run: sheet.chooseSymbol },
            { id: 'run-erc', label: 'commands.runErc', run: sheet.erc },
            { id: 'annotate', label: 'commands.annotate', run: sheet.annotate },
            { id: 'find-replace', label: 'commands.findReplace', run: sheet.find },
            { id: 'export-schematic', label: 'commands.exportSchematic', run: sheet.exportMenu },
            { id: 'grid-assistant', label: 'commands.gridAssistant', run: sheet.grid },
            { id: 'schematic-guide', label: 'commands.schematicGuide', run: sheet.guide },
          ] satisfies Command[])),
      ...(boardCommands === null
        ? []
        : ([
            { id: 'run-drc', label: 'commands.runDrc', run: boardCommands.drc },
            { id: 'fill-zones', label: 'commands.fillZones', run: boardCommands.fillZones },
            { id: 'update-board', label: 'commands.updateBoard', run: boardCommands.update },
            { id: 'board-setup', label: 'commands.boardSetup', run: boardCommands.setup },
            { id: 'design-rules', label: 'commands.designRules', run: boardCommands.rules },
            { id: 'import-dxf', label: 'commands.importDxf', run: boardCommands.importDxf },
            {
              id: 'place-footprint',
              label: 'commands.placeFootprint',
              run: boardCommands.placeFootprint,
            },
          ] satisfies Command[])),
      ...(outputs === null
        ? []
        : ([
            { id: 'generate-outputs', label: 'commands.generateOutputs', run: outputs.openJob },
            { id: 'gerber-viewer', label: 'commands.gerberViewer', run: outputs.openGerbers },
          ] satisfies Command[])),
      ...(open3d === null
        ? []
        : ([{ id: 'view-3d', label: 'commands.view3d', run: open3d }] satisfies Command[])),
      ...(project === null
        ? []
        : ([
            {
              id: 'online-parts',
              label: 'commands.onlineParts',
              run: () => {
                openOnlineParts();
              },
            },
          ] satisfies Command[])),
      { id: 'new-project', label: 'commands.newProject', run: newProject },
      {
        id: 'open-project',
        label: 'commands.openProject',
        run: () => {
          void openProject();
        },
      },
      {
        id: 'import-kicad',
        label: 'commands.importKicad',
        run: () => {
          void startKicadImport();
        },
      },
      {
        id: 'open-library',
        label: 'commands.openLibrary',
        run: openLibrary,
      },
      ...(project?.documents.some((document) => document.kind === 'pcb') === true
        ? [
            {
              id: 'import-netlist',
              label: 'commands.importNetlist',
              run: () => {
                void importNetlist();
              },
            } as const,
          ]
        : []),
      ...(project === null
        ? []
        : ([
            {
              id: 'save-project',
              label: 'commands.saveProject',
              run: () => {
                void save();
              },
            },
            {
              id: 'save-project-as',
              label: 'commands.saveProjectAs',
              run: () => {
                void saveAs();
              },
            },
            {
              id: 'save-copy',
              label: 'commands.saveCopy',
              run: () => {
                void saveCopy();
              },
            },
            {
              id: 'archive-project',
              label: 'commands.archiveProject',
              run: () => {
                void archive();
              },
            },
            { id: 'undo', label: 'commands.undo', run: undo },
            { id: 'redo', label: 'commands.redo', run: redo },
            { id: 'validate-project', label: 'commands.validateProject', run: validate },
            {
              id: 'project-properties',
              label: 'commands.projectProperties',
              run: () => {
                setProjectView((count) => count + 1);
              },
            },
            {
              id: 'add-sheet',
              label: 'commands.addSheet',
              run: () => {
                void addDocument(
                  'schematic',
                  t('tree.newSheet'),
                  project.rootSchematic ?? undefined,
                );
              },
            },
            {
              id: 'show-project-folder',
              label: 'commands.showProjectFolder',
              run: () => {
                showInFolder();
              },
            },
            {
              id: 'close-project',
              label: 'commands.closeProject',
              run: () => {
                void closeProject();
              },
            },
          ] satisfies Command[])),
      {
        id: 'open-settings',
        label: 'commands.openSettings',
        run: () => {
          setSettingsOpen(true);
        },
      },
      { id: 'check-updates', label: 'commands.checkUpdates', run: checkForUpdates },
      {
        id: 'language-de',
        label: 'commands.languageGerman',
        run: () => {
          updateSettings({ language: 'de' });
        },
      },
      {
        id: 'language-en',
        label: 'commands.languageEnglish',
        run: () => {
          updateSettings({ language: 'en' });
        },
      },
      {
        id: 'language-system',
        label: 'commands.languageSystem',
        run: () => {
          updateSettings({ language: 'system' });
        },
      },
      {
        id: 'theme-light',
        label: 'commands.themeLight',
        run: () => {
          updateSettings({ theme: 'light' });
        },
      },
      {
        id: 'theme-dark',
        label: 'commands.themeDark',
        run: () => {
          updateSettings({ theme: 'dark' });
        },
      },
      {
        id: 'theme-system',
        label: 'commands.themeSystem',
        run: () => {
          updateSettings({ theme: 'system' });
        },
      },
      {
        id: 'documentation',
        label: 'commands.documentation',
        run: () => {
          window.ew.app.openExternal(DOCS_URL);
        },
      },
      {
        id: 'report-issue',
        label: 'commands.reportIssue',
        run: () => {
          window.ew.app.openExternal(`${PROJECT_URL}/issues`);
        },
      },
    ],
    [
      sheet,
      boardCommands,
      addDocument,
      archive,
      checkForUpdates,
      closeProject,
      importNetlist,
      newProject,
      openLibrary,
      openProject,
      outputs,
      open3d,
      startKicadImport,
      project,
      redo,
      save,
      saveAs,
      saveCopy,
      showInFolder,
      t,
      undo,
      updateSettings,
      validate,
    ],
  );

  return (
    <div className="app">
      <main
        className="app__content"
        data-view={libraryOpen ? 'library' : project === null ? 'start' : 'project'}
      >
        <div className="app__view" inert={libraryOpen}>
          {project === null ? (
            <StartScreen onOpenLibrary={openLibrary} />
          ) : (
            <ProjectWorkspace projectView={projectView} onOpenLibrary={openLibrary} />
          )}
        </div>
        {libraryOpen && <LibraryManager onClose={closeLibrary} />}
      </main>
      <ProjectDialogs />
      <PartsHost />
      <StatusBar
        onOpenSettings={() => {
          setSettingsOpen(true);
        }}
        onOpenPalette={() => {
          setPaletteOpen(true);
        }}
      />
      {settingsOpen && (
        <SettingsDialog
          onClose={() => {
            setSettingsOpen(false);
          }}
          onCheckUpdates={checkForUpdates}
        />
      )}
      {paletteOpen && (
        <CommandPalette
          commands={commands}
          onClose={() => {
            setPaletteOpen(false);
          }}
        />
      )}
      {update !== null && (
        <UpdateDialog
          state={update}
          onClose={() => {
            setUpdate(null);
          }}
        />
      )}
    </div>
  );
}
