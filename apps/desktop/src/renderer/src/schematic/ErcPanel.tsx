import { useEffect, useState, useSyncExternalStore, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import { isEngineEvent } from '@shared/project';

import { Icon } from '../components/Icon';
import { Modal } from '../components/Modal';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { ercState, type ErcIssue, type ErcReport } from './stores';

type Severity = 'error' | 'warning' | 'ignore';

/** Runs the ERC and stores its report. */
export async function runErc(call: <T>(method: string) => Promise<T>): Promise<void> {
  ercState.set({ ...ercState.get(), running: true });
  try {
    const report = await call<ErcReport>('schematic.erc');
    ercState.set({ ...ercState.get(), report, running: false, time: Date.now() });
  } catch {
    ercState.set({ ...ercState.get(), running: false });
  }
}

/** Keeps an open ERC panel up to date after every change of the project. */
export function useErcUpdates(): void {
  const { engine } = useAppState();
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const off = engine.onEvent((event) => {
      if (!isEngineEvent(event) || !ercState.get().open) {
        return;
      }
      clearTimeout(timer);
      timer = setTimeout(() => {
        void runErc((method) => engine.call(method));
      }, 300);
    });
    return () => {
      clearTimeout(timer);
      off();
    };
  }, [engine]);
}

/** The translated message of a finding. */
export function issueMessage(t: ReturnType<typeof useAppState>['t'], issue: ErcIssue): string {
  const args: Record<string, string> = { ...issue.args };
  for (const key of ['kind', 'otherKind']) {
    const kind = args[key];
    if (kind !== undefined) {
      args[key] = t(`schematic.erc.pinKinds.${kind}` as MessageKey);
    }
  }
  const key = `schematic.erc.messages.${issue.code}` as MessageKey;
  const text = t(key, args);
  return text === key ? issue.message : text;
}

/** The results of the electrical rules check, below the drawing. */
export function ErcPanel({
  doc,
  onShow,
  onAnnotate,
  onSnap,
  onSettings,
}: {
  readonly doc: string;
  /** Shows a finding (on this or another sheet). */
  readonly onShow: (issue: ErcIssue) => void;
  readonly onAnnotate: () => void;
  readonly onSnap: () => void;
  readonly onSettings: () => void;
}): ReactNode {
  const { t, engine } = useAppState();
  const { project } = useProject();
  const state = useSyncExternalStore(ercState.subscribe, ercState.get);
  const [show, setShow] = useState({ error: true, warning: true });
  if (!state.open) {
    return null;
  }
  const report = state.report;
  const issues = report?.issues.filter((issue) => show[issue.severity]) ?? [];
  const sheetName = (id: string): string =>
    project?.documents.find((document) => document.id === id)?.name ?? '';
  const codes = new Set(report?.issues.map((issue) => issue.code) ?? []);
  return (
    <section className="erc" aria-label={t('schematic.erc.title')} data-testid="erc-panel">
      <header className="erc__header">
        <h2 className="erc__title">{t('schematic.erc.title')}</h2>
        {report !== null && (
          <span className="erc__summary" data-testid="erc-summary">
            {t('schematic.erc.summary', { errors: report.errors, warnings: report.warnings })}
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
          {t('schematic.erc.errors')}
        </label>
        <label className="checkbox erc__filter">
          <input
            type="checkbox"
            checked={show.warning}
            onChange={(event) => {
              setShow({ ...show, warning: event.target.checked });
            }}
          />
          {t('schematic.erc.warnings')}
        </label>
        <span className="erc__spacer" />
        {codes.has('unannotated') && (
          <button type="button" className="button button--small" onClick={onAnnotate}>
            {t('schematic.erc.fixAnnotate')}
          </button>
        )}
        {codes.has('off-grid') && (
          <button type="button" className="button button--small" onClick={onSnap}>
            {t('schematic.erc.fixSnap')}
          </button>
        )}
        <button
          type="button"
          className="button button--small"
          disabled={state.running}
          data-testid="erc-run"
          onClick={() => {
            void runErc((method) => engine.call(method));
          }}
        >
          {state.running ? t('schematic.erc.running') : t('schematic.erc.run')}
        </button>
        <button
          type="button"
          className="button button--small button--quiet"
          title={t('schematic.erc.settings')}
          aria-label={t('schematic.erc.settings')}
          data-testid="erc-settings"
          onClick={onSettings}
        >
          <Icon name="rules" size={16} />
        </button>
        <button
          type="button"
          className="button button--small button--quiet"
          title={t('schematic.erc.close')}
          aria-label={t('schematic.erc.close')}
          data-testid="erc-close"
          onClick={() => {
            ercState.set({ ...state, open: false });
          }}
        >
          <Icon name="close" size={16} />
        </button>
      </header>
      {report === null ? (
        <p className="erc__note">{t('schematic.erc.notRun')}</p>
      ) : report.issues.length === 0 ? (
        <p className="erc__note erc__note--ok" data-testid="erc-clean">
          {t('schematic.erc.clean', { nets: report.nets, pins: report.pins })}
        </p>
      ) : (
        <ul className="erc__list" data-testid="erc-list">
          {issues.map((issue, index) => (
            <li key={`${issue.code}-${issue.sheet}-${issue.at.join(',')}-${index.toString()}`}>
              <button
                type="button"
                className="erc__issue"
                data-severity={issue.severity}
                data-current={issue.sheet === doc}
                data-testid={`erc-issue-${issue.code}`}
                title={t('schematic.erc.zoom', { sheet: sheetName(issue.sheet) })}
                onClick={() => {
                  onShow(issue);
                }}
              >
                <Icon name={issue.severity === 'error' ? 'error' : 'warning'} size={16} />
                <span className="erc__message">{issueMessage(t, issue)}</span>
                <span className="erc__where">{issue.path}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

interface SettingsReply {
  readonly checks: readonly { code: string; default: Severity; value: Severity }[];
  readonly kinds: readonly string[];
  readonly defaultMatrix: readonly (readonly Severity[])[];
  readonly matrix: readonly (readonly Severity[])[];
}

const NEXT: Readonly<Record<Severity, Severity>> = {
  ignore: 'warning',
  warning: 'error',
  error: 'ignore',
};

/** Check severities and the pin type matrix (stored in the rules document). */
export function ErcSettingsDialog({ onClose }: { readonly onClose: () => void }): ReactNode {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  const [loaded, setLoaded] = useState<SettingsReply | null>(null);
  const [severities, setSeverities] = useState<Record<string, Severity>>({});
  const [matrix, setMatrix] = useState<Severity[][]>([]);

  useEffect(() => {
    let cancelled = false;
    void engine.call<SettingsReply>('schematic.ercSettings').then(
      (reply) => {
        if (!cancelled) {
          setLoaded(reply);
          setSeverities(Object.fromEntries(reply.checks.map((c) => [c.code, c.value])));
          setMatrix(reply.matrix.map((row) => [...row]));
        }
      },
      (error: unknown) => {
        notify(errorMessage(t, error), 'error');
      },
    );
    return () => {
      cancelled = true;
    };
  }, [engine, notify, t]);

  const save = (): void => {
    engine
      .call('schematic.setErcSettings', { severities, matrix })
      .then(onClose)
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      });
  };
  const kinds = loaded?.kinds ?? [];
  const label = (severity: Severity): string =>
    severity === 'ignore' ? t('schematic.erc.ok') : t(`schematic.erc.severity.${severity}`);
  return (
    <Modal
      title={t('schematic.erc.settingsTitle')}
      onClose={onClose}
      testId="erc-settings-dialog"
      className="dialog--wide"
      footer={
        <>
          <button
            type="button"
            className="button dialog__footer-start"
            disabled={loaded === null}
            onClick={() => {
              if (loaded !== null) {
                setSeverities(Object.fromEntries(loaded.checks.map((c) => [c.code, c.default])));
                setMatrix(loaded.defaultMatrix.map((row) => [...row]));
              }
            }}
          >
            {t('schematic.erc.reset')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={loaded === null}
            data-testid="erc-settings-save"
            onClick={save}
          >
            {t('schematic.erc.save')}
          </button>
        </>
      }
    >
      {loaded !== null && (
        <div className="erc-settings">
          <fieldset className="field--group">
            <legend>{t('schematic.erc.severities')}</legend>
            <div className="erc-settings__checks">
              {loaded.checks.map(({ code }) => (
                <label key={code} className="erc-settings__check">
                  <span>{t(`schematic.erc.checks.${code}` as MessageKey)}</span>
                  <select
                    className="field__control"
                    value={severities[code] ?? 'error'}
                    data-testid={`erc-severity-${code}`}
                    onChange={(event) => {
                      setSeverities({ ...severities, [code]: event.target.value as Severity });
                    }}
                  >
                    {(['error', 'warning', 'ignore'] as const).map((severity) => (
                      <option key={severity} value={severity}>
                        {t(`schematic.erc.severity.${severity}`)}
                      </option>
                    ))}
                  </select>
                </label>
              ))}
            </div>
          </fieldset>
          <fieldset className="field--group">
            <legend>{t('schematic.erc.matrix')}</legend>
            <p className="field__hint">{t('schematic.erc.matrixHint')}</p>
            <table className="erc-matrix" data-testid="erc-matrix">
              <thead>
                <tr>
                  <th />
                  {kinds.map((kind) => (
                    <th
                      key={kind}
                      scope="col"
                      title={t(`schematic.erc.pinKinds.${kind}` as MessageKey)}
                    >
                      {t(`schematic.erc.pinShort.${kind}` as MessageKey)}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {kinds.map((row, r) => (
                  <tr key={row}>
                    <th scope="row">{t(`schematic.erc.pinKinds.${row}` as MessageKey)}</th>
                    {kinds.map((column, c) => {
                      const severity = matrix[r]?.[c] ?? 'ignore';
                      // The matrix is symmetric: only the lower half is editable.
                      if (c > r) {
                        return (
                          <td
                            key={column}
                            className="erc-matrix__mirror"
                            data-severity={severity}
                          />
                        );
                      }
                      const name = t('schematic.erc.cell', {
                        row: t(`schematic.erc.pinKinds.${row}` as MessageKey),
                        column: t(`schematic.erc.pinKinds.${column}` as MessageKey),
                        severity: label(severity),
                      });
                      return (
                        <td key={column}>
                          <button
                            type="button"
                            className="erc-matrix__cell"
                            data-severity={severity}
                            title={name}
                            aria-label={name}
                            data-testid={`erc-cell-${row}-${column}`}
                            onClick={() => {
                              const next = NEXT[severity];
                              const copy = matrix.map((line) => [...line]);
                              const a = copy[r];
                              const b = copy[c];
                              if (a !== undefined && b !== undefined) {
                                a[c] = next;
                                b[r] = next;
                                setMatrix(copy);
                              }
                            }}
                          >
                            {severity === 'error' ? 'E' : severity === 'warning' ? 'W' : '·'}
                          </button>
                        </td>
                      );
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
          </fieldset>
        </div>
      )}
    </Modal>
  );
}
