import { createContext, use, useEffect, useMemo, useState, type ReactNode } from 'react';

import { ENGINE_PORT_MESSAGE, type AppInfo, type SettingsSnapshot } from '@shared/api';
import type { EngineStatus } from '@shared/engine';
import { createTranslator, type Translate } from '@shared/i18n';
import type { SettingsPatch } from '@shared/settings';

import { EngineClient } from './engine-client';

export interface AppState {
  readonly appInfo: AppInfo;
  readonly snapshot: SettingsSnapshot;
  readonly engineStatus: EngineStatus;
  readonly engine: EngineClient;
  readonly t: Translate;
  readonly updateSettings: (patch: SettingsPatch) => void;
}

const AppStateContext = createContext<AppState | null>(null);

export function useAppState(): AppState {
  const state = use(AppStateContext);
  if (state === null) {
    throw new Error('useAppState must be used inside <AppStateProvider>');
  }
  return state;
}

export function useT(): Translate {
  return useAppState().t;
}

const engine = new EngineClient();

// The preload script hands over a fresh engine port after every (re)start.
window.addEventListener('message', (event: MessageEvent<unknown>) => {
  const data = event.data as { type?: unknown } | null;
  const [port] = event.ports;
  if (event.source === window && data?.type === ENGINE_PORT_MESSAGE && port !== undefined) {
    engine.attach(port);
  }
});

interface ProviderProps {
  readonly initial: {
    readonly appInfo: AppInfo;
    readonly snapshot: SettingsSnapshot;
    readonly engineStatus: EngineStatus;
  };
  readonly children: ReactNode;
}

export function AppStateProvider({ initial, children }: ProviderProps): ReactNode {
  const [snapshot, setSnapshot] = useState(initial.snapshot);
  const [engineStatus, setEngineStatus] = useState(initial.engineStatus);

  useEffect(() => window.ew.settings.onChanged(setSnapshot), []);

  useEffect(() => {
    const unsubscribe = window.ew.engine.onStatus((status) => {
      setEngineStatus(status);
      if (status.state === 'running') {
        window.ew.engine.requestPort();
      } else {
        engine.detach();
      }
    });
    if (initial.engineStatus.state === 'running') {
      window.ew.engine.requestPort();
    }
    return unsubscribe;
  }, [initial.engineStatus.state]);

  useEffect(() => {
    document.documentElement.lang = snapshot.language;
  }, [snapshot.language]);

  const value = useMemo<AppState>(
    () => ({
      appInfo: initial.appInfo,
      snapshot,
      engineStatus,
      engine,
      t: createTranslator(snapshot.language),
      updateSettings: (patch) => {
        void window.ew.settings.set(patch).then(setSnapshot);
      },
    }),
    [initial.appInfo, snapshot, engineStatus],
  );

  return <AppStateContext value={value}>{children}</AppStateContext>;
}
