/**
 * Library checks: the findings of the shown item (with a button that shows
 * the pin or pad), and on request the findings of the whole library.
 */
import { useState, type ReactNode } from 'react';

import { Icon, type IconName } from '../components/Icon';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { findingText } from './labels';
import type { Finding, LibraryKind, Severity } from './types';

const SEVERITY_ICON: Readonly<Record<Severity, IconName>> = {
  error: 'error',
  warning: 'warning',
  info: 'info',
};

function FindingRow({
  finding,
  onShow,
}: {
  readonly finding: Finding;
  readonly onShow: ((finding: Finding) => void) | null;
}): ReactNode {
  const { t } = useAppState();
  const target = finding.target;
  const showable =
    onShow !== null &&
    target !== undefined &&
    (target.index !== undefined || target.unit !== undefined);
  return (
    <li
      className={`issues__item issues__item--${finding.severity}`}
      data-testid={`finding-${finding.code}`}
    >
      <Icon name={SEVERITY_ICON[finding.severity]} size={16} />
      <span className="issues__message">{findingText(t, finding)}</span>
      <code className="issues__code">{finding.code}</code>
      {showable && (
        <button
          type="button"
          className="button button--small button--quiet"
          onClick={() => {
            onShow(finding);
          }}
        >
          {t('libedit.checks.show')}
        </button>
      )}
    </li>
  );
}

interface LibraryFindings {
  readonly kind: LibraryKind;
  readonly name: string;
  readonly findings: readonly Finding[];
}

export function ChecksPanel({
  doc,
  findings,
  onShow,
  onOpen,
}: {
  readonly doc: string;
  /** Findings of the shown item. */
  readonly findings: readonly Finding[];
  readonly onShow: (finding: Finding) => void;
  /** Opens another item of the library. */
  readonly onOpen: (kind: LibraryKind, name: string) => void;
}): ReactNode {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  const [all, setAll] = useState<readonly LibraryFindings[] | null>(null);
  const count = (severity: Severity): number =>
    findings.filter((finding) => finding.severity === severity).length;
  return (
    <div className="libedit-checks" data-testid="checks-panel">
      <div className="item-table__bar">
        <span className="item-table__count" data-testid="checks-summary">
          {t('libedit.checks.summary', {
            errors: count('error'),
            warnings: count('warning'),
            infos: count('info'),
          })}
        </span>
        <span className="item-table__spacer" />
        <button
          type="button"
          className="button button--small"
          data-testid="check-library"
          onClick={() => {
            engine
              .call<LibraryFindings[]>('libedit.check', { doc })
              .then(setAll, (error: unknown) => {
                notify(errorMessage(t, error), 'error');
              });
          }}
        >
          <Icon name="check" size={14} />
          {t('libedit.checks.run')}
        </button>
      </div>
      <div className="item-table__scroll">
        {findings.length === 0 ? (
          <p className="item-table__empty">{t('libedit.checks.none')}</p>
        ) : (
          <ul className="issues libedit-checks__list">
            {findings.map((finding, index) => (
              <FindingRow
                // Findings have no ids; their order is stable.
                // eslint-disable-next-line @eslint-react/no-array-index-key -- see above
                key={index}
                finding={finding}
                onShow={onShow}
              />
            ))}
          </ul>
        )}
        {all !== null && (
          <section className="libedit-checks__all" data-testid="library-findings">
            <h4 className="libedit-checks__title">{t('libedit.checks.title')}</h4>
            {all.length === 0 && <p className="item-table__empty">{t('libedit.checks.none')}</p>}
            {all.map((item) => (
              <details key={`${item.kind}:${item.name}`} className="libedit-checks__item">
                <summary>
                  <button
                    type="button"
                    className="link-button"
                    onClick={() => {
                      onOpen(item.kind, item.name);
                    }}
                  >
                    {t(`libedit.kind.${item.kind}`)}: {item.name}
                  </button>{' '}
                  <span className="pill pill--muted">{item.findings.length}</span>
                </summary>
                <ul className="issues">
                  {item.findings.map((finding, index) => (
                    // eslint-disable-next-line @eslint-react/no-array-index-key -- stable order
                    <FindingRow key={index} finding={finding} onShow={null} />
                  ))}
                </ul>
              </details>
            ))}
          </section>
        )}
      </div>
    </div>
  );
}
