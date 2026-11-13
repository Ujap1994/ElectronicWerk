import { createContext, use, useEffect, useMemo, useState, type ReactNode } from 'react';

import type { DocumentSummary } from '@shared/project';

import { ConfirmDialog } from '../components/ConfirmDialog';
import { Icon } from '../components/Icon';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import {
  isGerberFile,
  joinOutput,
  moved,
  newOutput,
  OUTPUT_KINDS,
  splitPath,
  type OutputJobProps,
  type OutputKind,
  type OutputResult,
  type OutputSpec,
  type Preset,
  type RunReport,
} from './model';
import { kindLabel, OutputSettings, TextOption } from './OutputSettings';

/** Opens a Gerber viewer tab for a set of files. */
export const OpenGerberContext = createContext<
  ((name: string, paths: readonly string[]) => void) | null
>(null);

export function useOpenGerber(): ((name: string, paths: readonly string[]) => void) | null {
  return use(OpenGerberContext);
}

function asJob(value: Record<string, unknown> | null): OutputJobProps | null {
  if (value === null || !Array.isArray(value['outputs'])) {
    return null;
  }
  return {
    name: typeof value['name'] === 'string' ? value['name'] : '',
    directory: typeof value['directory'] === 'string' ? value['directory'] : '',
    // Empty options and the default "enabled" are left out of the file.
    outputs: (value['outputs'] as Partial<OutputSpec>[]).map((output) => ({
      ...(output as OutputSpec),
      enabled: output.enabled ?? true,
      options: output.options ?? {},
    })),
  };
}

/** The last run of every job, kept while the project is open. */
const lastReports = new Map<string, RunReport>();

/** Forgets the runs (when a project closes). */
export function forgetOutputRuns(): void {
  lastReports.clear();
}

function randomId(): string {
  return crypto.randomUUID();
}

function StatusBadge({ result }: { readonly result: OutputResult | undefined }): ReactNode {
  const { t } = useAppState();
  if (result === undefined) {
    return null;
  }
  return (
    <span
      className={`outputs__status outputs__status--${result.status}`}
      title={result.message ?? t(`outputs.status.${result.status}`)}
      data-testid={`output-status-${result.id}`}
      data-status={result.status}
    >
      {t(`outputs.status.${result.status}`)}
    </span>
  );
}

/** The editor of an output job: outputs, their settings and the results of a run. */
export function OutputJobEditor({ document }: { readonly document: DocumentSummary }): ReactNode {
  const { t, engine, appInfo } = useAppState();
  const { project, documentProperties, setDocumentProperties, showInFolder, notify } = useProject();
  const openGerber = useOpenGerber();
  const [job, setJob] = useState<OutputJobProps | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [presets, setPresets] = useState<readonly Preset[]>([]);
  const [preset, setPreset] = useState<Preset | null>(null);
  const [report, setReport] = useState<RunReport | null>(
    () => lastReports.get(document.id) ?? null,
  );
  const remember = (next: RunReport | null): void => {
    if (next === null) {
      lastReports.delete(document.id);
    } else {
      lastReports.set(document.id, next);
    }
    setReport(next);
  };
  const [running, setRunning] = useState(false);
  const [copperCount, setCopperCount] = useState(2);

  useEffect(() => {
    let cancelled = false;
    void documentProperties(document.id).then((props) => {
      if (!cancelled) {
        setJob(asJob(props));
      }
    });
    return () => {
      cancelled = true;
    };
  }, [document.id, documentProperties, project]);

  useEffect(() => {
    let cancelled = false;
    void engine
      .call<Preset[]>('outputs.presets')
      .then((list) => {
        if (!cancelled) {
          setPresets(list);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [engine]);

  const boards = useMemo(
    () =>
      (project?.documents ?? [])
        .filter((d) => d.kind === 'pcb')
        .map((d) => ({ id: d.id, name: d.name })),
    [project],
  );
  const firstBoard = boards[0]?.id ?? null;
  useEffect(() => {
    if (firstBoard === null) {
      return undefined;
    }
    let cancelled = false;
    void documentProperties(firstBoard).then((props) => {
      const layers = (props?.['stackup'] as { layers?: { type?: string }[] } | undefined)?.layers;
      if (!cancelled && Array.isArray(layers)) {
        setCopperCount(Math.max(1, layers.filter((l) => l.type === 'copper').length));
      }
    });
    return () => {
      cancelled = true;
    };
  }, [firstBoard, documentProperties]);

  if (job === null) {
    return <div className="outputs outputs--loading" data-testid="output-job" />;
  }
  const save = (outputs: readonly OutputSpec[]): void => {
    setJob({ ...job, outputs });
    void setDocumentProperties(document.id, { outputs });
  };
  const current = job.outputs.find((o) => o.id === selected) ?? job.outputs[0] ?? null;
  const results = new Map((report?.results ?? []).map((r) => [r.id, r]));
  const separator = appInfo.platform === 'win32' ? '\\' : '/';

  const run = async (only?: readonly string[]): Promise<void> => {
    setRunning(true);
    try {
      const reply = await engine.call<RunReport>(
        'outputs.run',
        {
          job: document.id,
          ...(only === undefined ? {} : { outputs: only }),
          labels: {
            file: t('viewer.titleFile'),
            size: t('viewer.titleSize'),
            date: t('viewer.titleDate'),
            revision: t('viewer.titleRevision'),
          },
        },
        600_000,
      );
      remember(reply);
      const failed = reply.results.filter((r) => r.status === 'failed').length;
      if (failed > 0) {
        notify(t('outputs.failedToast', { count: failed }), 'error');
      } else {
        notify(t('outputs.doneToast', { count: reply.files.length }));
      }
    } catch (reason) {
      notify(errorMessage(t, reason), 'error');
    } finally {
      setRunning(false);
    }
  };

  const viewGerbers = async (): Promise<void> => {
    if (openGerber === null) {
      return;
    }
    let paths: string[];
    if (report !== null) {
      paths = report.files
        .map((f) => f.path)
        .filter(isGerberFile)
        .map((p) => joinOutput(report.directory, p, separator));
    } else {
      try {
        const listed = await engine.call<{ files: string[] }>('outputs.files', {
          job: document.id,
        });
        paths = listed.files;
      } catch (reason) {
        notify(errorMessage(t, reason), 'error');
        return;
      }
    }
    if (paths.length === 0) {
      notify(t('outputs.viewer.empty'));
      return;
    }
    openGerber(job.name, paths);
  };

  const summary =
    report === null
      ? null
      : t('outputs.resultSummary', {
          done: report.results.filter((r) => r.status === 'done').length,
          skipped: report.results.filter((r) => r.status === 'skipped').length,
          failed: report.results.filter((r) => r.status === 'failed').length,
          ms: report.elapsedMs,
        });

  return (
    <div className="outputs" data-testid="output-job">
      <header className="outputs__toolbar" role="toolbar" aria-label={t('outputs.toolbar')}>
        <h2 className="outputs__title">
          <Icon name="output" size={20} />
          {job.name}
        </h2>
        <button
          type="button"
          className="button button--primary"
          title={t('outputs.generateAllTip')}
          disabled={running}
          data-testid="outputs-generate"
          onClick={() => {
            void run();
          }}
        >
          {running ? t('outputs.generating') : t('outputs.generateAll')}
        </button>
        <button
          type="button"
          className="button"
          title={t('outputs.gerberViewerTip')}
          data-testid="outputs-gerber-viewer"
          onClick={() => {
            void viewGerbers();
          }}
        >
          <Icon name="layers" size={16} /> {t('outputs.gerberViewer')}
        </button>
        <button
          type="button"
          className="button"
          data-testid="outputs-show-folder"
          onClick={() => {
            if (report === null) {
              showInFolder(job.directory);
            } else {
              window.ew.projects.showInFolder(report.directory);
            }
          }}
        >
          <Icon name="reveal" size={16} /> {t('outputs.showFolder')}
        </button>
        <span className="outputs__spacer" />
        <select
          className="field__control"
          value=""
          aria-label={t('outputs.preset')}
          title={t('outputs.preset')}
          data-testid="outputs-preset"
          onChange={(event) => {
            setPreset(presets.find((p) => p.id === event.target.value) ?? null);
          }}
        >
          <option value="">{t('outputs.presetChoose')}</option>
          {presets.map((p) => (
            <option key={p.id} value={p.id} title={p.description}>
              {p.name}
            </option>
          ))}
        </select>
      </header>
      <div className="outputs__body">
        <section className="outputs__list" aria-label={t('outputs.outputs')}>
          <div className="outputs__folder">
            <TextOption
              label={t('outputs.folder')}
              value={job.directory}
              hint={t('outputs.folderHint')}
              testId="outputs-directory"
              onCommit={(directory) => {
                setJob({ ...job, directory });
                void setDocumentProperties(document.id, { directory: directory.trim() });
              }}
            />
          </div>
          <h3 className="outputs__subtitle">{t('outputs.outputs')}</h3>
          <ul className="outputs__items" data-testid="outputs-list">
            {job.outputs.map((spec, index) => (
              <li
                key={spec.id}
                className={`outputs__item${spec.id === current?.id ? ' outputs__item--selected' : ''}`}
                data-testid={`output-${String(index)}`}
                data-kind={spec.kind}
              >
                <input
                  type="checkbox"
                  checked={spec.enabled}
                  aria-label={t('outputs.enabled', { name: spec.name })}
                  data-testid={`output-enabled-${String(index)}`}
                  onChange={(event) => {
                    save(
                      job.outputs.map((o) =>
                        o.id === spec.id ? { ...o, enabled: event.target.checked } : o,
                      ),
                    );
                  }}
                />
                <button
                  type="button"
                  className="outputs__item-name"
                  onClick={() => {
                    setSelected(spec.id);
                  }}
                >
                  <span>{spec.name}</span>
                  {kindLabel(t, spec.kind).toLowerCase() !== spec.name.toLowerCase() && (
                    <span className="outputs__kind">{kindLabel(t, spec.kind)}</span>
                  )}
                </button>
                <StatusBadge result={results.get(spec.id)} />
              </li>
            ))}
          </ul>
          <div className="outputs__list-actions">
            <select
              className="field__control"
              value=""
              aria-label={t('outputs.addOutput')}
              data-testid="outputs-add"
              onChange={(event) => {
                const kind = event.target.value as OutputKind | '';
                if (kind === '') {
                  return;
                }
                const spec = newOutput(kind, kindLabel(t, kind), randomId());
                save([...job.outputs, spec]);
                setSelected(spec.id);
              }}
            >
              <option value="">{t('outputs.addOutput')}</option>
              {OUTPUT_KINDS.map((kind) => (
                <option key={kind} value={kind}>
                  {kindLabel(t, kind)}
                </option>
              ))}
            </select>
            {current !== null && (
              <>
                <button
                  type="button"
                  className="icon-button"
                  title={t('outputs.moveUp')}
                  aria-label={t('outputs.moveUp')}
                  onClick={() => {
                    save(moved(job.outputs, job.outputs.indexOf(current), -1));
                  }}
                >
                  <Icon name="chevron" size={16} className="icon--up" />
                </button>
                <button
                  type="button"
                  className="icon-button"
                  title={t('outputs.moveDown')}
                  aria-label={t('outputs.moveDown')}
                  onClick={() => {
                    save(moved(job.outputs, job.outputs.indexOf(current), 1));
                  }}
                >
                  <Icon name="chevron" size={16} className="icon--down" />
                </button>
                <button
                  type="button"
                  className="icon-button"
                  title={t('outputs.remove')}
                  aria-label={t('outputs.remove')}
                  data-testid="outputs-remove"
                  onClick={() => {
                    save(job.outputs.filter((o) => o.id !== current.id));
                    setSelected(null);
                  }}
                >
                  <Icon name="trash" size={16} />
                </button>
              </>
            )}
          </div>
        </section>
        <section className="outputs__settings" aria-label={t('outputs.settings')}>
          {current === null ? (
            <p className="field__hint">{t('outputs.noSelection')}</p>
          ) : (
            <>
              <div className="outputs__settings-header">
                <h3 className="outputs__subtitle">{t('outputs.settings')}</h3>
                <button
                  type="button"
                  className="button button--small"
                  disabled={running}
                  data-testid="output-generate-one"
                  onClick={() => {
                    void run([current.id]);
                  }}
                >
                  {t('outputs.generateOne')}
                </button>
              </div>
              <OutputSettings
                key={current.id}
                spec={current}
                copperCount={copperCount}
                boards={boards}
                onChange={(next) => {
                  save(job.outputs.map((o) => (o.id === next.id ? next : o)));
                }}
              />
            </>
          )}
        </section>
      </div>
      <section
        className="outputs__results"
        aria-label={t('outputs.results')}
        data-testid="outputs-results"
      >
        <h3 className="outputs__subtitle">
          {t('outputs.results')}
          {summary !== null && <span className="field__hint"> {summary}</span>}
        </h3>
        {report === null ? (
          <p className="field__hint">{t('outputs.noResults')}</p>
        ) : (
          <ul className="outputs__files">
            {report.results
              .filter((r) => r.status !== 'skipped' || r.message !== 'not selected')
              .map((result) => (
                <li key={result.id} className="outputs__result">
                  <div className="outputs__result-head">
                    <StatusBadge result={result} />
                    <strong>{result.name}</strong>
                    {result.message !== undefined && (
                      <span className="field__hint">{result.message}</span>
                    )}
                  </div>
                  {result.files.map((path) => {
                    const { folder, file } = splitPath(path);
                    const absolute = joinOutput(report.directory, path, separator);
                    return (
                      <div key={path} className="outputs__file">
                        <Icon name="file" size={14} />
                        <span className="outputs__file-name" title={path}>
                          {folder !== '' && <span className="field__hint">{folder}/</span>}
                          {file}
                        </span>
                        <button
                          type="button"
                          className="button button--small"
                          onClick={() => {
                            window.ew.projects.openFile(absolute);
                          }}
                        >
                          {t('outputs.open')}
                        </button>
                        <button
                          type="button"
                          className="icon-button"
                          title={t('outputs.showFolder')}
                          aria-label={t('outputs.showFolder')}
                          onClick={() => {
                            window.ew.projects.showInFolder(absolute);
                          }}
                        >
                          <Icon name="reveal" size={14} />
                        </button>
                      </div>
                    );
                  })}
                </li>
              ))}
          </ul>
        )}
      </section>
      {preset !== null && (
        <ConfirmDialog
          title={t('outputs.presetConfirmTitle', { preset: preset.name })}
          message={t('outputs.presetConfirm', { job: job.name, preset: preset.name })}
          confirmLabel={t('outputs.presetApply')}
          cancelLabel={t('board.cancel')}
          testId="outputs-preset-confirm"
          onAnswer={(confirmed) => {
            const chosen = preset;
            setPreset(null);
            if (confirmed) {
              void engine
                .call('outputs.applyPreset', { job: document.id, preset: chosen.id })
                .then(() => {
                  setSelected(null);
                  remember(null);
                })
                .catch((reason: unknown) => {
                  notify(errorMessage(t, reason), 'error');
                });
            }
          }}
        />
      )}
    </div>
  );
}
