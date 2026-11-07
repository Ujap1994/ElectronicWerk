import { useEffect, useState, type ReactNode } from 'react';

import { SettingsDialog } from './components/SettingsDialog';
import { StartScreen } from './components/StartScreen';
import { StatusBar } from './components/StatusBar';

export function App(): ReactNode {
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(
    () =>
      window.ew.app.onMenuAction((action) => {
        if (action === 'settings') {
          setSettingsOpen(true);
        }
      }),
    [],
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
      />
      {settingsOpen && (
        <SettingsDialog
          onClose={() => {
            setSettingsOpen(false);
          }}
        />
      )}
    </div>
  );
}
