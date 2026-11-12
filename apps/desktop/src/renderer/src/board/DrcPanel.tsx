import { useEffect, useState, useSyncExternalStore, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import { isEngineEvent } from '@shared/project';

import { Icon } from '../components/Icon';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { drcState, type DrcFinding, type DrcReport } from './stores';

type Call = <T>(method: string, params?: unknown) => Promise<T>;

/** Runs the design rules check of a board and stores its report. */
export async function runDrc(call: Call, doc: string): Promise<void> {
  drcState.set({ ...drcState.get(), running: true, doc });
  try {
    const report = await call<DrcReport>('pcb.drc', { doc });
    if (drcState.get().doc === doc) {
      drcState.set({ ...drcState.get(), report, running: false });
    }
  } catch {
    drcState.set({ ...drcState.get(), running: false });
  }
}

/**
 * Keeps the report of the shown board current: the check runs again
 * shortly after every change of the project (online DRC).
 */
export function useDrcUpdates(doc: string | null): void {
  const { engine } = useAppState();
  useEffect(() => {
    if (doc === null) {
      return undefined;
    }
    let timer: ReturnType<typeof setTimeout> | undefined;
    const run = (): void => {
      void runDrc((method, params) => engine.call(method, params), doc);
    };
    if (drcState.get().doc !== doc || drcState.get().report === null) {
      drcState.set({ ...drcState.get(), doc, report: null });
      run();
    }
    const off = engine.onEvent((event) => {
      if (!isEngineEvent(event) || !drcState.get().online) {
        return;
      }
      clearTimeout(timer);
      timer = setTimeout(run, 250);
    });
    return () => {
      clearTimeout(timer);
      off();
    };
  }, [engine, doc]);
}

/** The translated message of a finding. */
export function findingMessage(
  t: ReturnType<typeof useAppState>['t'],
  finding: DrcFinding,
): string {
  const key = `board.drc.messages.${finding.code}` as MessageKey;
  const text = t(key, finding.args);
  return text === key ? finding.message : text;
}

/** The results of the design rules check, below the board. */
export function DrcPanel({
  doc,
  onShow,
  onSettings,
}: {
  readonly doc: string;
  readonly onShow: (finding: DrcFinding) => void;
  readonly onSettings: () => void;
}): ReactNode {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  const state = useSyncExternalStore(drcState.subscribe, drcState.get);
  const [show, setShow] = useState({ error: true, warning: true, excluded: false });
  if (!state.open) {
    return null;
  }
  const report = state.doc === doc ? state.report : null;
  const findings =
    report?.findings.filter((f) => (f.excluded ? show.excluded : show[f.severity])) ?? [];
  const exclude = (finding: DrcFinding, excluded: boolean): void => {
    engine.call('pcb.exclude', { doc, keys: [finding.key], excluded }).catch((error: unknown) => {
      notify(errorMessage(t, error), 'error');
    });
  };
  return (
    <section className="erc drc" aria-label={t('board.drc.title')} data-testid="drc-panel">
      <header className="erc__header">
        <h2 className="erc__title">{t('board.drc.title')}</h2>
        {report !== null && (
          <span className="erc__summary" data-testid="drc-summary">
            {t('board.drc.summary', {
              errors: report.counts.errors,
              warnings: report.counts.warnings,
              unrouted: report.counts.unrouted,
            })}
          </span>
        )}
        <label className="checkbox erc__filter">
          <input
            type="checkbox"
            checked={show.error}
            onChange={(event) => {
              setShow({ ...show, error: event.target.checked });
            }}
          />
          {t('board.drc.errors')}
        </label>
        <label className="checkbox erc__filter">
          <input
            type="checkbox"
            checked={show.warning}
            onChange={(event) => {
              setShow({ ...show, warning: event.target.checked });
            }}
          />
          {t('board.drc.warnings')}
        </label>
        <label className="checkbox erc__filter">
          <input
            type="checkbox"
            checked={show.excluded}
            data-testid="drc-show-excluded"
            onChange={(event) => {
              setShow({ ...show, excluded: event.target.checked });
            }}
          />
          {t('board.drc.excluded', { count: report?.counts.excluded ?? 0 })}
        </label>
        <label className="checkbox erc__filter" title={t('board.drc.onlineHint')}>
          <input
            type="checkbox"
            checked={state.online}
            data-testid="drc-online"
            onChange={(event) => {
              drcState.set({ ...drcState.get(), online: event.target.checked });
            }}
          />
          {t('board.drc.online')}
        </label>
        <span className="erc__spacer" />
        <button
          type="button"
          className="button button--small"
          disabled={state.running}
          data-testid="drc-run"
          onClick={() => {
            void runDrc((method, params) => engine.call(method, params), doc);
          }}
        >
          {state.running ? t('board.drc.running') : t('board.drc.run')}
        </button>
        <button
          type="button"
          className="button button--small button--quiet"
          title={t('board.drc.settings')}
          aria-label={t('board.drc.settings')}
          data-testid="drc-settings"
          onClick={onSettings}
        >
          <Icon name="rules" size={16} />
        </button>
        <button
          type="button"
          className="button button--small button--quiet"
          title={t('board.drc.close')}
          aria-label={t('board.drc.close')}
          data-testid="drc-close"
          onClick={() => {
            drcState.set({ ...state, open: false });
          }}
        >
          <Icon name="close" size={16} />
        </button>
      </header>
      {report === null ? (
        <p className="erc__note">{t('board.drc.notRun')}</p>
      ) : report.findings.length === 0 ? (
        <p className="erc__note erc__note--ok" data-testid="drc-clean">
          {t('board.drc.clean')}
        </p>
      ) : (
        <ul className="erc__list" data-testid="drc-list">
          {findings.map((finding) => (
            <li key={finding.key} className="drc__row">
              <button
                type="button"
                className="button button--small button--quiet drc__exclude"
                title={finding.excluded ? t('board.drc.include') : t('board.drc.exclude')}
                aria-label={finding.excluded ? t('board.drc.include') : t('board.drc.exclude')}
                data-testid={`drc-exclude-${finding.code}`}
                onClick={() => {
                  exclude(finding, !finding.excluded);
                }}
              >
                <Icon name={finding.excluded ? 'eye' : 'eyeOff'} size={14} />
              </button>
              <button
                type="button"
                className="erc__issue"
                data-severity={finding.severity}
                data-excluded={finding.excluded}
                data-testid={`drc-finding-${finding.code}`}
                title={t('board.drc.zoom')}
                onClick={() => {
                  onShow(finding);
                }}
              >
                <Icon name={finding.severity === 'error' ? 'error' : 'warning'} size={16} />
                <span className="erc__message">{findingMessage(t, finding)}</span>
                <span className="erc__where">{finding.layer ?? ''}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
