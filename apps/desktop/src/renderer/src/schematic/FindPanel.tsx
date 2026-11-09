import { useEffect, useState, type ReactNode } from 'react';

import { Icon } from '../components/Icon';
import { Modal } from '../components/Modal';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import type { Point } from './editor';

export interface FindHit {
  readonly doc: string;
  readonly id: string;
  readonly field: string;
  readonly text: string;
  readonly at: Point;
}

/** Find and replace across every sheet (floating over the drawing). */
export function FindPanel({
  onShow,
  onClose,
}: {
  readonly onShow: (hit: FindHit) => void;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine } = useAppState();
  const { project, notify } = useProject();
  const [query, setQuery] = useState('');
  const [replacement, setReplacement] = useState('');
  const [matchCase, setMatchCase] = useState(false);
  const [wholeWord, setWholeWord] = useState(false);
  const [found, setFound] = useState<{ key: string; hits: readonly FindHit[] } | null>(null);
  const options = { query, matchCase, wholeWord };
  const key = JSON.stringify(options);

  useEffect(() => {
    if (query === '') {
      return undefined;
    }
    let cancelled = false;
    const timer = setTimeout(() => {
      void engine.call<FindHit[]>('schematic.find', { query, matchCase, wholeWord }).then(
        (hits) => {
          if (!cancelled) {
            setFound({ key, hits });
          }
        },
        () => undefined,
      );
    }, 120);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [engine, query, matchCase, wholeWord, key]);

  const hits = found?.key === key && query !== '' ? found.hits : [];
  const fieldLabel = (field: string): string =>
    field.startsWith('property:')
      ? t('schematic.find.fields.property', { name: field.slice('property:'.length) })
      : t(`schematic.find.fields.${field as 'designator'}`);
  const sheetName = (id: string): string =>
    project?.documents.find((document) => document.id === id)?.name ?? '';

  return (
    <section className="find" aria-label={t('schematic.find.title')} data-testid="find-panel">
      <header className="find__header">
        <h2 className="find__title">{t('schematic.find.title')}</h2>
        <button
          type="button"
          className="button button--small button--quiet"
          aria-label={t('schematic.find.close')}
          title={t('schematic.find.close')}
          data-testid="find-close"
          onClick={onClose}
        >
          <Icon name="close" size={16} />
        </button>
      </header>
      <input
        className="field__control"
        type="search"
        value={query}
        placeholder={t('schematic.find.find')}
        aria-label={t('schematic.find.find')}
        data-testid="find-query"
        autoFocus
        onChange={(event) => {
          setQuery(event.target.value);
        }}
        onKeyDown={(event) => {
          if (event.key === 'Escape') {
            onClose();
          } else if (event.key === 'Enter' && hits[0] !== undefined) {
            onShow(hits[0]);
          }
        }}
      />
      <div className="find__options">
        <label className="checkbox">
          <input
            type="checkbox"
            checked={matchCase}
            onChange={(event) => {
              setMatchCase(event.target.checked);
            }}
          />
          {t('schematic.find.matchCase')}
        </label>
        <label className="checkbox">
          <input
            type="checkbox"
            checked={wholeWord}
            onChange={(event) => {
              setWholeWord(event.target.checked);
            }}
          />
          {t('schematic.find.wholeWord')}
        </label>
      </div>
      <p className="find__count" data-testid="find-count">
        {query === ''
          ? ''
          : hits.length === 0
            ? t('schematic.find.none')
            : t('schematic.find.results', { count: hits.length })}
      </p>
      <ul className="find__hits" data-testid="find-hits">
        {hits.map((hit) => (
          <li key={`${hit.doc}-${hit.id}-${hit.field}`}>
            <button
              type="button"
              className="find__hit"
              onClick={() => {
                onShow(hit);
              }}
            >
              <span className="find__text">{hit.text}</span>
              <span className="find__where">
                {fieldLabel(hit.field)} · {sheetName(hit.doc)}
              </span>
            </button>
          </li>
        ))}
      </ul>
      <div className="find__replace">
        <input
          className="field__control"
          value={replacement}
          placeholder={t('schematic.find.replace')}
          aria-label={t('schematic.find.replace')}
          data-testid="find-replacement"
          onChange={(event) => {
            setReplacement(event.target.value);
          }}
        />
        <button
          type="button"
          className="button button--small"
          disabled={hits.length === 0}
          data-testid="find-replace-all"
          onClick={() => {
            engine
              .call<{ changes: number }>('schematic.replace', { ...options, with: replacement })
              .then((result) => {
                notify(t('schematic.find.replaced', { count: result.changes }));
              })
              .catch((error: unknown) => {
                notify(errorMessage(t, error), 'error');
              });
          }}
        >
          {t('schematic.find.replaceAll')}
        </button>
      </div>
    </section>
  );
}

/** Numbering designators (all sheets or this one). */
export function AnnotateDialog({
  doc,
  onClose,
}: {
  readonly doc: string;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  const [allSheets, setAllSheets] = useState(true);
  const [reset, setReset] = useState(false);
  const [order, setOrder] = useState<'rows' | 'columns'>('rows');
  const [bySheet, setBySheet] = useState(false);
  const run = (): void => {
    engine
      .call<{ changes: number }>('schematic.annotate', {
        sheet: allSheets ? null : doc,
        reset,
        order,
        bySheet,
      })
      .then((result) => {
        notify(
          result.changes === 0
            ? t('schematic.annotate.nothing')
            : t('schematic.annotate.done', { count: result.changes }),
        );
        onClose();
      })
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      });
  };
  const radio = (
    name: string,
    checked: boolean,
    label: string,
    onChange: () => void,
    testId: string,
  ): ReactNode => (
    <label className="checkbox">
      <input type="radio" name={name} checked={checked} data-testid={testId} onChange={onChange} />
      {label}
    </label>
  );
  return (
    <Modal
      title={t('schematic.annotate.title')}
      onClose={onClose}
      testId="annotate-dialog"
      footer={
        <button
          type="button"
          className="button button--primary"
          data-testid="annotate-run"
          onClick={run}
        >
          {t('schematic.annotate.run')}
        </button>
      }
    >
      <fieldset className="field--group">
        <legend>{t('schematic.annotate.scope')}</legend>
        {radio(
          'scope',
          allSheets,
          t('schematic.annotate.all'),
          () => {
            setAllSheets(true);
          },
          'annotate-all',
        )}
        {radio(
          'scope',
          !allSheets,
          t('schematic.annotate.sheet'),
          () => {
            setAllSheets(false);
          },
          'annotate-sheet',
        )}
      </fieldset>
      <fieldset className="field--group">
        <legend>{t('schematic.annotate.mode')}</legend>
        {radio(
          'mode',
          !reset,
          t('schematic.annotate.unannotated'),
          () => {
            setReset(false);
          },
          'annotate-unannotated',
        )}
        {radio(
          'mode',
          reset,
          t('schematic.annotate.reset'),
          () => {
            setReset(true);
          },
          'annotate-reset',
        )}
      </fieldset>
      <fieldset className="field--group">
        <legend>{t('schematic.annotate.order')}</legend>
        {radio(
          'order',
          order === 'rows',
          t('schematic.annotate.rows'),
          () => {
            setOrder('rows');
          },
          'annotate-rows',
        )}
        {radio(
          'order',
          order === 'columns',
          t('schematic.annotate.columns'),
          () => {
            setOrder('columns');
          },
          'annotate-columns',
        )}
      </fieldset>
      <label className="checkbox">
        <input
          type="checkbox"
          checked={bySheet}
          onChange={(event) => {
            setBySheet(event.target.checked);
          }}
        />
        {t('schematic.annotate.bySheet')}
      </label>
    </Modal>
  );
}
