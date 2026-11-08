import { useEffect, useId, useState, type ReactNode } from 'react';

import type { MessageKey, Translate } from '@shared/i18n';
import {
  checkName,
  IMPORT_CODES,
  joinPath,
  sanitizeFolderName,
  type ImportFinding,
} from '@shared/project';

import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import { Icon } from './Icon';
import { Modal } from './Modal';

const NAME_ERRORS = {
  empty: 'wizard.errorEmpty',
  tooLong: 'wizard.errorTooLong',
  invalid: 'wizard.errorInvalid',
} as const satisfies Record<string, MessageKey>;

/** Report count keys (from the engine) and their labels. */
const COUNT_LABELS: Readonly<Record<string, MessageKey>> = {
  sheets: 'import.kinds.sheets',
  symbols: 'import.kinds.symbols',
  wires: 'import.kinds.wires',
  labels: 'import.kinds.labels',
  junctions: 'import.kinds.junctions',
  'sheet texts and graphics': 'import.kinds.sheetGraphics',
  footprints: 'import.kinds.footprints',
  pads: 'import.kinds.pads',
  tracks: 'import.kinds.tracks',
  vias: 'import.kinds.vias',
  zones: 'import.kinds.zones',
  nets: 'import.kinds.nets',
  'board graphics': 'import.kinds.boardGraphics',
  'library symbols': 'import.kinds.librarySymbols',
  'library footprints': 'import.kinds.libraryFootprints',
};

/** Translated title of a finding (the engine's English message is the detail). */
export function findingTitle(t: Translate, finding: ImportFinding): string {
  return IMPORT_CODES.has(finding.code)
    ? t(`import.codes.${finding.code}` as MessageKey)
    : finding.code;
}

function KicadImportDialog(): ReactNode {
  const { t, appInfo } = useAppState();
  const { prompts, runKicadImport, closeKicadImport } = useProject();
  const formId = useId();
  const prompt = prompts.kicadImport;
  const [name, setName] = useState(prompt?.probe.name ?? '');
  const [location, setLocation] = useState('');
  const [touched, setTouched] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([window.ew.projects.recent(), window.ew.projects.defaultLocation()]).then(
      ([recent, fallback]) => {
        if (!cancelled) {
          setLocation((current) => (current === '' ? (recent.lastLocation ?? fallback) : current));
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);

  if (prompt === null) {
    return null;
  }
  const nameProblem = checkName(name);
  const valid = nameProblem === null && location.trim() !== '';
  const target = joinPath(location.trim(), sanitizeFolderName(name), appInfo.platform);
  const parts = [
    prompt.probe.schematic ? t('import.schematic') : null,
    prompt.probe.board ? t('import.board') : null,
    prompt.probe.project ? t('import.projectFile') : null,
  ].filter((part) => part !== null);
  const submit = (): void => {
    setTouched(true);
    if (valid) {
      void runKicadImport({ source: prompt.source, name: name.trim(), location: location.trim() });
    }
  };
  return (
    <Modal
      title={t('import.title')}
      testId="kicad-import-dialog"
      onClose={closeKicadImport}
      footer={
        <>
          <button type="button" className="button" onClick={closeKicadImport}>
            {t('wizard.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            data-testid="kicad-import-start"
            disabled={touched && !valid}
            onClick={submit}
          >
            {t('import.start')}
          </button>
        </>
      }
    >
      <p className="dialog__text dialog__text--muted">{t('import.intro')}</p>
      <div className="field">
        <span className="field__label">{t('import.source')}</span>
        <code className="path" data-testid="kicad-import-source">
          {prompt.source}
        </code>
        <span className="field__hint">
          {t('import.contains')}: {parts.join(', ')}
        </span>
      </div>
      <label className="field">
        <span className="field__label">{t('import.name')}</span>
        <input
          className="field__control"
          value={name}
          data-testid="kicad-import-name"
          onChange={(event) => {
            setName(event.target.value);
          }}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              submit();
            }
          }}
        />
        {touched && nameProblem !== null && (
          <span className="field__error">{t(NAME_ERRORS[nameProblem])}</span>
        )}
      </label>
      <div className="field">
        <label className="field__label" htmlFor={`${formId}-location`}>
          {t('import.location')}
        </label>
        <div className="field__row">
          <input
            id={`${formId}-location`}
            className="field__control field__control--grow"
            value={location}
            data-testid="kicad-import-location"
            onChange={(event) => {
              setLocation(event.target.value);
            }}
          />
          <button
            type="button"
            className="button"
            onClick={() => {
              void window.ew.dialogs.chooseFolder(location, 'location').then((chosen) => {
                if (chosen !== null) {
                  setLocation(chosen);
                }
              });
            }}
          >
            {t('import.change')}
          </button>
        </div>
        <span className="field__hint" data-testid="kicad-import-target">
          {t('import.target', { path: target })}
        </span>
      </div>
    </Modal>
  );
}

function ImportReportDialog(): ReactNode {
  const { t } = useAppState();
  const { prompts, closeImportReport, showInFolder } = useProject();
  const prompt = prompts.importReport;
  if (prompt === null) {
    return null;
  }
  const { report } = prompt;
  const total = (severity: ImportFinding['severity']): number =>
    report.findings
      .filter((finding) => finding.severity === severity)
      .reduce((sum, finding) => sum + finding.count, 0);
  const counts = Object.entries(report.counts);
  const severityLabel = {
    error: t('import.severity.error'),
    warning: t('import.severity.warning'),
    info: t('import.severity.info'),
  } as const;
  const source = report.source.split(/[\\/]/u).pop() ?? report.source;
  return (
    <Modal
      title={t('import.reportTitle')}
      testId="import-report"
      className="dialog--wide"
      onClose={closeImportReport}
      footer={
        <>
          <button
            type="button"
            className="button dialog__footer-start"
            data-testid="import-report-show"
            onClick={() => {
              showInFolder(prompt.reportFile);
            }}
          >
            {t('import.showReport')}
          </button>
          <button type="button" className="button button--primary" onClick={closeImportReport}>
            {t('validation.close')}
          </button>
        </>
      }
    >
      <p className="dialog__text">{t('import.reportIntro', { source })}</p>
      <h3 className="dialog__section">{t('import.imported')}</h3>
      <dl className="import-counts" data-testid="import-counts">
        {counts.map(([key, count]) => {
          const label = COUNT_LABELS[key];
          return (
            <div key={key} className="import-counts__row">
              <dt>{label === undefined ? key : t(label)}</dt>
              <dd className="import-counts__value">{new Intl.NumberFormat().format(count)}</dd>
            </div>
          );
        })}
      </dl>
      <h3 className="dialog__section">{t('import.findings')}</h3>
      <p className="dialog__text" data-testid="import-summary">
        {report.findings.length === 0
          ? t('import.noFindings')
          : t('import.summary', {
              errors: total('error'),
              warnings: total('warning'),
              notes: total('info'),
            })}
      </p>
      {report.findings.length > 0 && (
        <ul className="issues">
          {report.findings.map((finding) => (
            <li
              key={`${finding.severity}-${finding.code}-${finding.file ?? ''}-${finding.message}`}
              className={`issues__item issues__item--${finding.severity}`}
            >
              <Icon
                name={
                  finding.severity === 'error'
                    ? 'error'
                    : finding.severity === 'warning'
                      ? 'warning'
                      : 'info'
                }
              />
              <div>
                <div className="issues__head">
                  <strong>{severityLabel[finding.severity]}</strong>
                  <span>{findingTitle(t, finding)}</span>
                  {finding.count > 1 && (
                    <span className="issues__count">
                      {t('import.times', { count: finding.count })}
                    </span>
                  )}
                </div>
                <div className="issues__message">
                  {finding.file !== undefined && <code>{finding.file}</code>} {finding.message}
                </div>
                <code className="issues__code">{finding.code}</code>
              </div>
            </li>
          ))}
        </ul>
      )}
    </Modal>
  );
}

const DIFFERENCES_SHOWN = 30;

function NetlistDialog(): ReactNode {
  const { t } = useAppState();
  const { prompts, applyNetlist, closeNetlist } = useProject();
  const prompt = prompts.netlist;
  if (prompt === null) {
    return null;
  }
  const { result } = prompt;
  const { comparison } = result;
  const equivalent =
    comparison.matchingNets === comparison.nets &&
    comparison.differences.length === 0 &&
    comparison.missingComponents.length === 0;
  const file = prompt.source.split(/[\\/]/u).pop() ?? prompt.source;
  return (
    <Modal
      title={t('netlist.title')}
      testId="netlist-dialog"
      className="dialog--wide"
      onClose={closeNetlist}
      footer={
        <>
          <button type="button" className="button" onClick={closeNetlist}>
            {t('validation.close')}
          </button>
          {result.changedPads + result.newNets > 0 && (
            <button
              type="button"
              className="button button--primary"
              data-testid="netlist-apply"
              onClick={() => {
                void applyNetlist();
              }}
            >
              {t('netlist.apply')}
            </button>
          )}
        </>
      }
    >
      <p className="dialog__text">
        <strong>{t(`netlist.format.${result.format}`)}</strong> <code>{file}</code> ·{' '}
        {t('netlist.contents', { nets: result.nets, components: result.components })}
      </p>
      <p
        className={`callout ${equivalent ? 'callout--info' : 'callout--warning'}`}
        data-testid="netlist-summary"
      >
        <Icon name={equivalent ? 'check' : 'warning'} />
        <span>
          {equivalent
            ? t('netlist.equivalent')
            : t('netlist.matching', {
                matching: comparison.matchingNets,
                nets: comparison.nets,
                same: comparison.sameNames,
              })}
        </span>
      </p>
      {comparison.differences.length > 0 && (
        <>
          <h3 className="dialog__section">{t('netlist.differences')}</h3>
          <table className="netlist-table" data-testid="netlist-differences">
            <thead>
              <tr>
                <th>{t('netlist.pin')}</th>
                <th>{t('netlist.inNetlist')}</th>
                <th>{t('netlist.onBoard')}</th>
              </tr>
            </thead>
            <tbody>
              {comparison.differences.slice(0, DIFFERENCES_SHOWN).map((difference) => (
                <tr key={`${difference.reference}-${difference.pin}`}>
                  <td>
                    {difference.reference}-{difference.pin}
                  </td>
                  <td>{difference.expected ?? t('netlist.none')}</td>
                  <td>{difference.actual ?? t('netlist.none')}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {comparison.differences.length > DIFFERENCES_SHOWN && (
            <p className="dialog__text dialog__text--muted">
              {t('netlist.more', { count: comparison.differences.length - DIFFERENCES_SHOWN })}
            </p>
          )}
        </>
      )}
      {result.missingFootprints.length > 0 && (
        <p className="dialog__text">
          {t('netlist.missingFootprints', { list: result.missingFootprints.join(', ') })}
        </p>
      )}
      {result.extraFootprints.length > 0 && (
        <p className="dialog__text dialog__text--muted">
          {t('netlist.extraFootprints', { list: result.extraFootprints.join(', ') })}
        </p>
      )}
      <p className="dialog__text" data-testid="netlist-changes">
        {result.changedPads + result.newNets > 0
          ? t('netlist.changes', { pads: result.changedPads, nets: result.newNets })
          : t('netlist.noChanges')}
      </p>
    </Modal>
  );
}

/** The dialogs of the import flows. */
export function ImportDialogs(): ReactNode {
  const { prompts } = useProject();
  return (
    <>
      {prompts.kicadImport !== null && <KicadImportDialog key={prompts.kicadImport.source} />}
      <ImportReportDialog />
      <NetlistDialog />
    </>
  );
}
