import { useEffect, useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import type { ProbeResult } from '@shared/project';
import type { RecentProject } from '@shared/recent';

import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import { Icon } from './Icon';

interface Action {
  readonly id: string;
  readonly title: MessageKey;
  readonly hint: MessageKey;
  /** Version that brings the action, for actions not available yet. */
  readonly availableIn: string | null;
  readonly icon: ReactNode;
}

const ACTIONS: readonly Action[] = [
  {
    id: 'new-project',
    title: 'start.newProject',
    hint: 'start.newProjectHint',
    availableIn: null,
    icon: <path d="M12 5v14M5 12h14" />,
  },
  {
    id: 'open-project',
    title: 'start.openProject',
    hint: 'start.openProjectHint',
    availableIn: null,
    icon: <path d="M3 7h6l2 2h10v10H3zM3 7V5h6" />,
  },
  {
    id: 'import-kicad',
    title: 'start.importKicad',
    hint: 'start.importKicadHint',
    availableIn: null,
    icon: <path d="M12 4v11m0 0-4-4m4 4 4-4M4 20h16" />,
  },
  {
    id: 'library',
    title: 'library.open',
    hint: 'library.openHint',
    availableIn: null,
    icon: <path d="M4 4h4v16H4zM10 4h4v16h-4zM16 5l3.5-1 2.5 15.5-3.5 1z" />,
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

/** Recent projects, each checked with the engine (exists? unsaved work?). */
function RecentProjects(): ReactNode {
  const { t, engine, engineStatus, snapshot } = useAppState();
  const { openProject } = useProject();
  const [recent, setRecent] = useState<readonly RecentProject[]>([]);
  const [probes, setProbes] = useState<Readonly<Record<string, ProbeResult>>>({});

  useEffect(() => {
    const unsubscribe = window.ew.projects.onRecentChanged((state) => {
      setRecent(state.projects);
    });
    void window.ew.projects.recent().then((state) => {
      setRecent(state.projects);
    });
    return unsubscribe;
  }, []);

  useEffect(() => {
    if (engineStatus.state !== 'running' || recent.length === 0) {
      return undefined;
    }
    let cancelled = false;
    void engine.whenConnected().then(async () => {
      const entries = await Promise.all(
        recent.map(async (project) => {
          const probe = await engine
            .call<ProbeResult>('project.probe', { path: project.manifestPath })
            .catch((): ProbeResult => ({ exists: true }));
          return [project.manifestPath, probe] as const;
        }),
      );
      if (!cancelled) {
        setProbes(Object.fromEntries(entries));
      }
    });
    return () => {
      cancelled = true;
    };
  }, [engine, engineStatus.state, engineStatus.generation, recent]);

  if (recent.length === 0) {
    return <p className="start__empty">{t('start.recentEmpty')}</p>;
  }
  const dateFormat = new Intl.DateTimeFormat(snapshot.language, {
    dateStyle: 'medium',
    timeStyle: 'short',
  });
  return (
    <>
      <ul className="recent" data-testid="recent-list">
        {recent.map((project) => {
          const probe = probes[project.manifestPath];
          const missing = probe?.exists === false;
          return (
            <li key={project.manifestPath} className="recent__item" data-missing={missing}>
              <button
                type="button"
                className="recent__open"
                disabled={missing}
                data-testid="recent-open"
                onClick={() => {
                  void openProject(project.manifestPath);
                }}
              >
                <Icon name="project" size={20} />
                <span className="recent__text">
                  <span className="recent__name">{probe?.name ?? project.name}</span>
                  <span className="recent__path">{project.manifestPath}</span>
                </span>
                {missing && <span className="pill pill--muted">{t('start.recentMissing')}</span>}
                {probe?.hasRecovery === true && (
                  <span className="pill pill--warning" data-testid="recent-recovery">
                    {t('start.recentRecovery')}
                  </span>
                )}
                <span className="recent__date">
                  {t('start.recentOpened', { date: dateFormat.format(new Date(project.openedAt)) })}
                </span>
              </button>
              <button
                type="button"
                className="icon-button recent__remove"
                aria-label={t('start.recentRemove', { name: project.name })}
                title={t('start.recentRemove', { name: project.name })}
                onClick={() => {
                  void window.ew.projects.removeRecent(project.manifestPath);
                }}
              >
                <Icon name="close" size={14} />
              </button>
            </li>
          );
        })}
      </ul>
      <button
        type="button"
        className="button button--small start__clear"
        onClick={() => {
          void window.ew.projects.clearRecent();
        }}
      >
        {t('start.recentClear')}
      </button>
    </>
  );
}

export function StartScreen({ onOpenLibrary }: { readonly onOpenLibrary: () => void }): ReactNode {
  const { t } = useAppState();
  const { newProject, openProject, startKicadImport } = useProject();
  const run: Readonly<Record<string, () => void>> = {
    library: onOpenLibrary,
    'new-project': newProject,
    'open-project': () => {
      void openProject();
    },
    'import-kicad': () => {
      void startKicadImport();
    },
  };
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
            <button
              type="button"
              className="action-card"
              disabled={action.availableIn !== null}
              data-testid={action.id}
              onClick={run[action.id]}
            >
              <svg className="action-card__icon" viewBox="0 0 24 24" aria-hidden="true">
                {action.icon}
              </svg>
              <span className="action-card__title">{t(action.title)}</span>
              <span className="action-card__hint">{t(action.hint)}</span>
              {action.availableIn !== null && (
                <span className="action-card__badge">
                  {t('start.comingIn', { version: action.availableIn })}
                </span>
              )}
            </button>
          </li>
        ))}
      </ul>

      <section className="start__recent" aria-labelledby="recent-title">
        <h2 id="recent-title" className="start__section-title">
          {t('start.recentTitle')}
        </h2>
        <RecentProjects />
      </section>
    </section>
  );
}
