import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';

import { DOCS_URL, PROJECT_URL } from '@shared/links';

import type { Command } from './commands';
import { CommandPalette } from './components/CommandPalette';
import { ProjectDialogs } from './components/ProjectDialogs';
import { ProjectWorkspace } from './components/ProjectWorkspace';
import { SettingsDialog } from './components/SettingsDialog';
import { StartScreen } from './components/StartScreen';
import { StatusBar } from './components/StatusBar';
import { UpdateDialog, type UpdateDialogState } from './components/UpdateDialog';
import { useProject } from './project/ProjectProvider';
import { useAppState } from './state';

export function App(): ReactNode {
  const { updateSettings, t } = useAppState();
  const {
    project,
    newProject,
    openProject,
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
          case 'new-project':
            newProject();
            break;
          case 'open-project':
            void openProject();
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

  const commands = useMemo<Command[]>(
    () => [
      { id: 'new-project', label: 'commands.newProject', run: newProject },
      {
        id: 'open-project',
        label: 'commands.openProject',
        run: () => {
          void openProject();
        },
      },
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
      addDocument,
      archive,
      checkForUpdates,
      closeProject,
      newProject,
      openProject,
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
      <main className="app__content" data-view={project === null ? 'start' : 'project'}>
        {project === null ? <StartScreen /> : <ProjectWorkspace projectView={projectView} />}
      </main>
      <ProjectDialogs />
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
