import { useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';

import { useAppState } from '../state';

interface Action {
  readonly id: string;
  readonly title: MessageKey;
  readonly hint: MessageKey;
  readonly availableIn: string;
  readonly icon: ReactNode;
}

const ACTIONS: readonly Action[] = [
  {
    id: 'new-project',
    title: 'start.newProject',
    hint: 'start.newProjectHint',
    availableIn: 'v0.2',
    icon: <path d="M12 5v14M5 12h14" />,
  },
  {
    id: 'open-project',
    title: 'start.openProject',
    hint: 'start.openProjectHint',
    availableIn: 'v0.2',
    icon: <path d="M3 7h6l2 2h10v10H3zM3 7V5h6" />,
  },
  {
    id: 'import-kicad',
    title: 'start.importKicad',
    hint: 'start.importKicadHint',
    availableIn: 'v0.4',
    icon: <path d="M12 4v11m0 0-4-4m4 4 4-4M4 20h16" />,
  },
];

function RestartNotice(): ReactNode {
  const { t, engineStatus } = useAppState();
  const [dismissed, setDismissed] = useState(0);
  if (engineStatus.restarts === 0 || dismissed === engineStatus.restarts) {
    return null;
  }
  return (
    <div className="notice" role="status" data-testid="engine-restarted">
      <span>{t('engine.restartedNotice')}</span>
      <button
        type="button"
        className="notice__close"
        aria-label={t('settings.close')}
        onClick={() => {
          setDismissed(engineStatus.restarts);
        }}
      >
        ×
      </button>
    </div>
  );
}

export function StartScreen(): ReactNode {
  const { t } = useAppState();
  return (
    <section className="start" aria-labelledby="start-title">
      <RestartNotice />
      <header className="start__header">
        <img className="start__logo" src="./favicon.svg" alt="" width={64} height={64} />
        <div>
          <h1 id="start-title" className="start__title">
            {t('start.title')}
          </h1>
          <p className="start__subtitle">{t('start.subtitle')}</p>
        </div>
      </header>

      <ul className="start__actions">
        {ACTIONS.map((action) => (
          <li key={action.id}>
            <button type="button" className="action-card" disabled data-testid={action.id}>
              <svg className="action-card__icon" viewBox="0 0 24 24" aria-hidden="true">
                {action.icon}
              </svg>
              <span className="action-card__title">{t(action.title)}</span>
              <span className="action-card__hint">{t(action.hint)}</span>
              <span className="action-card__badge">
                {t('start.comingIn', { version: action.availableIn })}
              </span>
            </button>
          </li>
        ))}
      </ul>

      <section className="start__recent" aria-labelledby="recent-title">
        <h2 id="recent-title" className="start__section-title">
          {t('start.recentTitle')}
        </h2>
        <p className="start__empty">{t('start.recentEmpty')}</p>
      </section>
    </section>
  );
}
