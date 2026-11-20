/**
 * The part generator (M16, ADR-0017): a PDF datasheet on the left, the
 * part read from it on the right in four steps — part, package, pins and
 * result. Everything read is shown for checking before the symbol,
 * footprint, 3D model and component are made in a project library.
 */
import { useEffect, useMemo, useRef, useState, useSyncExternalStore, type ReactNode } from 'react';

import type {
  DatasheetInfo,
  Draft,
  Duplicates,
  Generated,
  PartOptions,
  PartPreview,
  Source,
} from '@shared/datasheet';

import { Icon } from '../components/Icon';
import { FootprintSvg } from '../libedit/FootprintSvg';
import { SelectInput, TextInput, NumberInput } from '../libedit/fields';
import type { FootprintDef } from '../libedit/types';
import { ModelPreview } from '../library/ModelPreview';
import { useProject } from '../project/ProjectProvider';
import { Store } from '../schematic/stores';
import { useAppState } from '../state';
import { useOpenDocument } from '../viewer/open-context';
import { DatasheetView } from './DatasheetView';
import { firstPackage } from './model';
import { PackageStep, PartStep, PinsStep, useNote } from './steps';
import { SymbolView, type SymbolJson } from './SymbolView';

/** What the generator was opened with. */
export interface PartGeneratorRequest {
  /** A datasheet to read at once. */
  readonly path: string | null;
  /** The library to store the part in. */
  readonly doc: string | null;
}

/** The open generator, if any. */
export const partGenerator = new Store<PartGeneratorRequest | null>(null);

/** Opens the generator, with a datasheet or with the file dialog. */
export function openPartGenerator(path: string | null = null, doc: string | null = null): void {
  partGenerator.set({ path, doc });
}

type Step = 'part' | 'package' | 'pins' | 'result';
const STEPS: readonly Step[] = ['part', 'package', 'pins', 'result'];

type Loaded =
  | { readonly kind: 'empty' }
  | { readonly kind: 'reading' }
  | { readonly kind: 'failed'; readonly message: string }
  | {
      readonly kind: 'ready';
      readonly info: DatasheetInfo;
      readonly data: Uint8Array;
    };

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** Renders the generator while it is open (inside the project workspace). */
export function PartGeneratorHost(): ReactNode {
  const request = useSyncExternalStore(partGenerator.subscribe, partGenerator.get);
  if (request === null) {
    return null;
  }
  return (
    <PartGenerator
      key={`${request.path ?? ''}:${request.doc ?? ''}`}
      request={request}
      onClose={() => {
        partGenerator.set(null);
      }}
    />
  );
}

export function PartGenerator({
  request,
  onClose,
}: {
  readonly request: PartGeneratorRequest;
  readonly onClose: () => void;
}): ReactNode {
  const { engine, t, snapshot } = useAppState();
  const advanced = snapshot.settings.workMode === 'advanced';
  const [path, setPath] = useState<string | null>(request.path);
  // The result of reading a path; another path means it is being read.
  const [result, setResult] = useState<{ readonly path: string; readonly loaded: Loaded } | null>(
    null,
  );
  const loaded: Loaded =
    path === null ? { kind: 'empty' } : result?.path === path ? result.loaded : { kind: 'reading' };
  const [draft, setDraft] = useState<Draft | null>(null);
  const [pkg, setPkg] = useState<string | null>(null);
  const [step, setStep] = useState<Step>('part');
  const [page, setPage] = useState(1);
  const [highlight, setHighlight] = useState<Source | null>(null);
  const [showAll, setShowAll] = useState(false);

  // Reading and recognising.
  useEffect(() => {
    if (path === null) {
      return undefined;
    }
    const run = { cancelled: false };
    Promise.all([
      engine.call<DatasheetInfo>('datasheet.read', { path }, 120_000),
      engine.call<{ data: Uint8Array }>('datasheet.file', { path }, 120_000),
      engine.call<Draft>('datasheet.extract', { path }, 120_000),
    ])
      .then(([info, file, recognised]) => {
        if (run.cancelled) {
          return;
        }
        setResult({ path, loaded: { kind: 'ready', info, data: file.data } });
        setDraft(recognised);
        setPkg(firstPackage(recognised));
        const at = recognised.source?.page;
        setPage(at ?? 1);
      })
      .catch((error: unknown) => {
        if (!run.cancelled) {
          setResult({ path, loaded: { kind: 'failed', message: message(error) } });
        }
      });
    return () => {
      run.cancelled = true;
    };
  }, [engine, path]);

  // A native modal dialog: in the top layer, the page behind is inert and
  // Escape closes it.
  const dialogRef = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = dialogRef.current;
    dialog?.showModal();
    dialog?.focus();
    return () => {
      dialog?.close();
    };
  }, []);

  const show = (source: Source): void => {
    setPage(source.page);
    setHighlight(source);
  };

  const choose = async (): Promise<void> => {
    const chosen = await window.ew.dialogs.chooseDatasheet();
    if (chosen !== null) {
      setPath(chosen);
    }
  };

  const fileName = path?.split(/[\\/]/u).at(-1) ?? '';
  const index = STEPS.indexOf(step);
  const ready = loaded.kind === 'ready' && draft !== null;
  return (
    <dialog
      ref={dialogRef}
      className="dialog partgen"
      aria-labelledby="partgen-title"
      tabIndex={-1}
      data-testid="partgen"
      onClose={onClose}
    >
      <header className="partgen__header">
        <Icon name="datasheet" />
        <h2 id="partgen-title">{t('partgen.title')}</h2>
        {fileName !== '' && (
          <span className="partgen__file" data-testid="partgen-file">
            {fileName}
          </span>
        )}
        <span className="toolbar__spacer" />
        <button
          type="button"
          className="icon-button"
          aria-label={t('partgen.close')}
          title={t('partgen.close')}
          data-testid="partgen-close"
          onClick={onClose}
        >
          <Icon name="close" />
        </button>
      </header>
      {path === null || loaded.kind === 'failed' ? (
        <div className="partgen__start">
          {loaded.kind === 'failed' && (
            <p className="field__error" data-testid="partgen-failed">
              {t('partgen.failed', { message: loaded.message })}
            </p>
          )}
          <p>{t('partgen.openHint')}</p>
          <button
            type="button"
            className="button button--primary"
            data-testid="partgen-open"
            onClick={() => {
              void choose();
            }}
          >
            {t('partgen.open')}
          </button>
          <p className="field__hint">{t('partgen.drop')}</p>
        </div>
      ) : !ready ? (
        <div className="partgen__start" data-testid="partgen-reading">
          <p>{t('partgen.reading')}</p>
          <p className="field__hint">{t('partgen.recognising')}</p>
        </div>
      ) : (
        <div className="partgen__body">
          <DatasheetView
            data={loaded.data}
            info={loaded.info}
            page={page}
            onPage={(next) => {
              setPage(next);
              setHighlight(null);
            }}
            highlight={highlight?.page === page ? (highlight.bbox ?? null) : null}
          />
          <aside className="partgen__panel">
            <nav className="partgen__steps" aria-label={t('partgen.title')}>
              {STEPS.map((s, i) => (
                <button
                  key={s}
                  type="button"
                  className="partgen__step"
                  aria-current={s === step ? 'step' : undefined}
                  data-testid={`partgen-tab-${s}`}
                  onClick={() => {
                    setStep(s);
                  }}
                >
                  <span className="partgen__step-number">{i + 1}</span>
                  {t(`partgen.steps.${s}`)}
                </button>
              ))}
            </nav>
            <div className="partgen__content">
              {step === 'part' && <PartStep draft={draft} onChange={setDraft} onShow={show} />}
              {step === 'package' && (
                <PackageStep
                  draft={draft}
                  pkg={pkg}
                  advanced={advanced}
                  showAll={showAll}
                  onShowAll={setShowAll}
                  onSelect={setPkg}
                  onChange={setDraft}
                  onShow={show}
                />
              )}
              {step === 'pins' && pkg !== null && (
                <PinsStep draft={draft} pkg={pkg} onChange={setDraft} onShow={show} />
              )}
              {step === 'result' && pkg !== null && (
                <ResultStep
                  draft={draft}
                  pkg={pkg}
                  advanced={advanced}
                  doc={request.doc}
                  onDone={onClose}
                />
              )}
            </div>
            <footer className="partgen__footer">
              <button
                type="button"
                className="button"
                disabled={index === 0}
                data-testid="partgen-back"
                onClick={() => {
                  setStep(STEPS[index - 1] ?? 'part');
                }}
              >
                {t('partgen.back')}
              </button>
              {index < STEPS.length - 1 && (
                <button
                  type="button"
                  className="button button--primary"
                  disabled={pkg === null && index >= 1}
                  data-testid="partgen-next"
                  onClick={() => {
                    setStep(STEPS[index + 1] ?? 'result');
                  }}
                >
                  {t('partgen.next')}
                </button>
              )}
            </footer>
          </aside>
        </div>
      )}
    </dialog>
  );
}

type PreviewState =
  | { readonly kind: 'loading' }
  | { readonly kind: 'failed'; readonly message: string }
  | { readonly kind: 'ready'; readonly preview: PartPreview; readonly model: string | null };

function ResultStep({
  draft,
  pkg,
  advanced,
  doc,
  onDone,
}: {
  readonly draft: Draft;
  readonly pkg: string;
  readonly advanced: boolean;
  readonly doc: string | null;
  readonly onDone: () => void;
}): ReactNode {
  const { engine, t } = useAppState();
  const { project, notify } = useProject();
  const openDocument = useOpenDocument();
  const note = useNote();
  const [options, setOptions] = useState<PartOptions>({});
  const [answer, setAnswer] = useState<{
    readonly key: string;
    readonly state: PreviewState;
  } | null>(null);
  const [duplicates, setDuplicates] = useState<Duplicates | null>(null);
  const [anyway, setAnyway] = useState(false);
  const [busy, setBusy] = useState(false);
  const [unit, setUnit] = useState(0);
  const libraries = useMemo(
    () => project?.documents.filter((d) => d.kind === 'library') ?? [],
    [project],
  );
  const [target, setTarget] = useState<string | null>(doc ?? libraries[0]?.id ?? null);

  const requestKey = useMemo(() => JSON.stringify({ draft, pkg, options }), [draft, pkg, options]);
  const state: PreviewState = answer?.key === requestKey ? answer.state : { kind: 'loading' };

  useEffect(() => {
    const run = { cancelled: false };
    const params = { draft, package: pkg, options };
    void (async () => {
      try {
        const preview = await engine.call<PartPreview>('part.preview', { ...params, model: true });
        let model: string | null = null;
        if (preview.step !== undefined) {
          const written = await engine.call<{ file: string }>('datasheet.previewModel', {
            step: preview.step,
          });
          model = written.file;
        }
        if (!run.cancelled) {
          setAnswer({ key: requestKey, state: { kind: 'ready', preview, model } });
        }
        const found = await engine.call<Duplicates>('part.duplicates', params);
        if (!run.cancelled) {
          setDuplicates(found);
        }
      } catch (error) {
        if (!run.cancelled) {
          setAnswer({ key: requestKey, state: { kind: 'failed', message: message(error) } });
        }
      }
    })();
    return () => {
      run.cancelled = true;
    };
  }, [draft, engine, options, pkg, requestKey]);

  const generate = async (): Promise<void> => {
    setBusy(true);
    try {
      const made = await engine.call<Generated>('part.generate', {
        draft,
        package: pkg,
        options,
        ...(target === null ? {} : { doc: target }),
      });
      const library = libraries.find((d) => d.id === target);
      notify(
        t('partgen.result.done', {
          component: made.component,
          library: library?.name ?? '',
          symbol: made.symbol,
          footprint: made.footprint,
        }),
      );
      if (library !== undefined && openDocument !== null) {
        openDocument(library);
      }
      onDone();
    } catch (error) {
      notify(t('partgen.result.error', { message: message(error) }));
    } finally {
      setBusy(false);
    }
  };

  const match = duplicates?.matches[0];
  const blocked = match !== undefined && !anyway;
  const preview = state.kind === 'ready' ? state.preview : null;
  const units = preview?.symbol.units.length ?? 0;
  const errors = preview?.findings.filter((f) => f.finding.severity === 'error') ?? [];
  return (
    <div className="partgen-step" data-testid="partgen-step-result">
      {state.kind === 'loading' && <p className="field__hint">{t('partgen.result.loading')}</p>}
      {state.kind === 'failed' && (
        <p className="field__error" data-testid="partgen-preview-failed">
          {t('partgen.result.error', { message: state.message })}
        </p>
      )}
      {preview !== null && (
        <>
          <div className="partgen-previews">
            <figure className="partgen-preview" data-testid="partgen-symbol">
              <figcaption>
                {t('partgen.result.symbol')}
                {units > 1 && (
                  <select
                    className="field__control partgen-unit"
                    value={unit}
                    aria-label={t('partgen.result.symbol')}
                    onChange={(event) => {
                      setUnit(Number(event.target.value));
                    }}
                  >
                    {Array.from({ length: units }, (_, i) => (
                      <option key={i} value={i}>
                        {String.fromCharCode(65 + i)}
                      </option>
                    ))}
                  </select>
                )}
              </figcaption>
              <SymbolView def={preview.symbol as unknown as SymbolJson} unit={unit} />
            </figure>
            <figure className="partgen-preview" data-testid="partgen-footprint">
              <figcaption>{t('partgen.result.footprint')}</figcaption>
              <FootprintSvg
                def={preview.footprint as unknown as FootprintDef}
                label={preview.footprint.name}
              />
              <span className="field__hint">{preview.footprint.name}</span>
            </figure>
            <figure className="partgen-preview partgen-preview--3d" data-testid="partgen-model">
              <figcaption>{t('partgen.result.model')}</figcaption>
              {state.kind === 'ready' && state.model !== null && (
                <ModelPreview
                  modelId={null}
                  file={state.model}
                  placement={null}
                  board={null}
                  release={null}
                />
              )}
            </figure>
          </div>
          {preview.warnings.length > 0 && (
            <div className="partgen-notes" data-testid="partgen-warnings">
              <h4 className="partgen-subtitle">{t('partgen.result.warnings')}</h4>
              <ul>
                {preview.warnings.map((w, i) => (
                  // eslint-disable-next-line @eslint-react/no-array-index-key -- notes have no id
                  <li key={i}>{note(w)}</li>
                ))}
              </ul>
            </div>
          )}
          <div className="partgen-notes" data-testid="partgen-findings">
            <h4 className="partgen-subtitle">{t('partgen.result.checks')}</h4>
            {preview.findings.filter((f) => f.finding.severity !== 'info').length === 0 ? (
              <p className="field__hint">{t('partgen.result.noFindings')}</p>
            ) : (
              <ul>
                {preview.findings
                  .filter((f) => f.finding.severity !== 'info')
                  .map((f, i) => (
                    // eslint-disable-next-line @eslint-react/no-array-index-key -- findings have no id
                    <li key={i} className={`partgen-finding--${f.finding.severity}`}>
                      {`${t(`libedit.kind.${f.item}`)}: ${f.finding.message}`}
                    </li>
                  ))}
              </ul>
            )}
          </div>
        </>
      )}
      {match !== undefined && (
        <div className="partgen-duplicate" role="alert" data-testid="partgen-duplicate">
          <h4 className="partgen-subtitle partgen-duplicate__text">
            {t('partgen.duplicates.title')}
          </h4>
          <p className="partgen-duplicate__text">
            {t('partgen.duplicates.match', { name: match.name, library: match.library })}
          </p>
          {!anyway && (
            <div className="partgen-duplicate__actions">
              <button
                type="button"
                className="button button--primary"
                data-testid="partgen-use-existing"
                onClick={() => {
                  notify(
                    t('partgen.duplicates.used', { name: match.name, library: match.library }),
                  );
                  const library = libraries.find((d) => d.id === match.doc);
                  if (library !== undefined && openDocument !== null) {
                    openDocument(library);
                  }
                  onDone();
                }}
              >
                {t('partgen.duplicates.use', { name: match.name })}
              </button>
              <button
                type="button"
                className="button"
                data-testid="partgen-anyway"
                onClick={() => {
                  setAnyway(true);
                }}
              >
                {t('partgen.duplicates.anyway')}
              </button>
            </div>
          )}
        </div>
      )}
      {(duplicates?.related.length ?? 0) > 0 && (
        <p className="field__hint" data-testid="partgen-related">
          {t('partgen.duplicates.related', {
            list: (duplicates?.related ?? []).map((r) => `${r.name} (${r.library})`).join(', '),
          })}
        </p>
      )}
      <h4 className="partgen-subtitle">{t('partgen.result.options')}</h4>
      <TextInput
        label={t('partgen.result.componentName')}
        value={options.name ?? preview?.component.name ?? ''}
        testId="partgen-component-name"
        onSave={(name) => {
          setOptions({ ...options, name });
        }}
      />
      {advanced && (
        <>
          <TextInput
            label={t('partgen.result.footprintName')}
            value={options.footprint ?? preview?.footprint.name ?? ''}
            testId="partgen-footprint-name"
            onSave={(footprint) => {
              setOptions({ ...options, footprint });
            }}
          />
          <SelectInput
            label={t('partgen.result.density')}
            value={options.density ?? 'nominal'}
            options={(['least', 'nominal', 'most'] as const).map(
              (d) => [d, t(`partgen.densities.${d}`)] as const,
            )}
            testId="partgen-density"
            onSave={(density) => {
              setOptions({ ...options, density });
            }}
          />
          <NumberInput
            label={t('partgen.result.split')}
            value={options.splitAbove ?? 64}
            integer
            min={0}
            testId="partgen-split"
            onSave={(splitAbove) => {
              setOptions({ ...options, splitAbove });
            }}
          />
        </>
      )}
      {project === null ? (
        <p className="field__hint">{t('partgen.result.noProject')}</p>
      ) : libraries.length === 0 ? (
        <p className="field__hint">{t('partgen.result.noLibrary')}</p>
      ) : (
        <SelectInput
          label={t('partgen.result.library')}
          value={target ?? ''}
          options={libraries.map((d) => [d.id, d.name] as const)}
          testId="partgen-library"
          onSave={setTarget}
        />
      )}
      <button
        type="button"
        className="button button--primary partgen-generate"
        disabled={
          busy ||
          preview === null ||
          errors.length > 0 ||
          blocked ||
          target === null ||
          project === null
        }
        data-testid="partgen-generate"
        onClick={() => {
          void generate();
        }}
      >
        {busy ? t('partgen.result.generating') : t('partgen.result.generate')}
      </button>
    </div>
  );
}
