import type { ReactNode } from 'react';

import { useAppState } from '../state';
import { useEngineInfo } from '../use-engine-info';

interface StatusBarProps {
  readonly onOpenSettings: () => void;
}

export function StatusBar({ onOpenSettings }: StatusBarProps): ReactNode {
  const { t, engineStatus, appInfo } = useAppState();
  const info = useEngineInfo();

  let engineText: string;
  switch (engineStatus.state) {
    case 'running':
      engineText =
        info === null
          ? t('engine.starting')
          : t('engine.running', { version: info.version, threads: info.threads });
      break;
    case 'starting':
      engineText = t('engine.starting');
      break;
    case 'restarting':
      engineText = t('engine.restarting');
      break;
    case 'failed':
      engineText = t('engine.failed', { error: engineStatus.error ?? '' });
      break;
    case 'stopped':
      engineText = t('engine.stopped');
      break;
  }
  const ready = engineStatus.state === 'running' && info !== null;

  return (
    <footer className="status-bar">
      <span
        className="status-bar__engine"
        data-testid="engine-status"
        data-state={ready ? 'ready' : engineStatus.state}
      >
        <span className="status-bar__dot" aria-hidden="true" />
        {engineText}
      </span>
      <span className="status-bar__spacer" />
      <span className="status-bar__version">
        {t('status.appVersion', { version: appInfo.version })}
      </span>
      <button
        type="button"
        className="status-bar__button"
        onClick={onOpenSettings}
        aria-label={t('settings.open')}
        title={t('settings.open')}
        data-testid="open-settings"
      >
        <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true">
          <path
            fill="currentColor"
            d="M19.14 12.94a7.5 7.5 0 0 0 0-1.88l2.03-1.58a.5.5 0 0 0 .12-.64l-1.92-3.32a.5.5 0 0 0-.6-.22l-2.39.96a7.4 7.4 0 0 0-1.63-.94l-.36-2.54A.5.5 0 0 0 13.9 2h-3.84a.5.5 0 0 0-.49.42l-.36 2.54c-.59.24-1.13.56-1.63.94l-2.39-.96a.5.5 0 0 0-.6.22L2.67 8.48a.5.5 0 0 0 .12.64l2.03 1.58a7.5 7.5 0 0 0 0 1.88l-2.03 1.58a.5.5 0 0 0-.12.64l1.92 3.32c.13.22.39.3.6.22l2.39-.96c.5.38 1.04.7 1.63.94l.36 2.54c.04.24.25.42.49.42h3.84c.24 0 .45-.18.49-.42l.36-2.54c.59-.24 1.13-.56 1.63-.94l2.39.96c.22.08.47 0 .6-.22l1.92-3.32a.5.5 0 0 0-.12-.64zM12 15.5A3.5 3.5 0 1 1 12 8.5a3.5 3.5 0 0 1 0 7z"
          />
        </svg>
      </button>
    </footer>
  );
}
