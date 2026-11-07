import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';

import { PROJECT_URL } from '@shared/links';

import type { Command } from './commands';
import { CommandPalette } from './components/CommandPalette';
import { SettingsDialog } from './components/SettingsDialog';
import { StartScreen } from './components/StartScreen';
import { StatusBar } from './components/StatusBar';
import { UpdateDialog, type UpdateDialogState } from './components/UpdateDialog';
import { useAppState } from './state';

export function App(): ReactNode {
  const { updateSettings } = useAppState();
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
        if (action === 'settings') {
          setSettingsOpen(true);
        } else if (action === 'check-updates') {
          checkForUpdates();
        } else if (action === 'command-palette') {
          setPaletteOpen(true);
        }
      }),
    [checkForUpdates],
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
          window.ew.app.openExternal(`${PROJECT_URL}#readme`);
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
    [checkForUpdates, updateSettings],
  );

  return (
    <div className="app">
      <main className="app__content">
        <StartScreen />
      </main>
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
