/**
 * The steps of the part generator: the part's names, the package with its
 * dimensions, the pins of the package and the result with previews. Every
 * value read from the datasheet has a button that shows its place.
 */
import { useState, type ReactNode } from 'react';

import {
  FAMILIES,
  PIN_TYPES,
  type Dimension,
  type Draft,
  type Family,
  type Note,
  type PinType,
  type Source,
} from '@shared/datasheet';
import type { MessageKey } from '@shared/i18n';

import { CheckInput, SelectInput, TextInput } from '../libedit/fields';
import { useAppState } from '../state';
import {
  addPin,
  dimensionKeys,
  mainKeys,
  missingPads,
  mm,
  parseNumbers,
  pinRows,
  removePin,
  setDimension,
  setPackage,
  setPin,
  setPinNumbers,
} from './model';

/** Translates a note of the engine by its code. */
export function useNote(): (note: Note) => string {
  const { t } = useAppState();
  const keys: Readonly<Record<string, MessageKey>> = {
    pinout: 'partgen.notes.pinout',
    'no-numbers': 'partgen.notes.noNumbers',
    scanned: 'partgen.notes.scanned',
    'pin-no-pad': 'partgen.notes.pinNoPad',
    'pad-exposed': 'partgen.notes.padExposed',
    'pad-no-pin': 'partgen.notes.padNoPin',
  };
  return (note) => {
    const key = keys[note.code];
    return key === undefined ? note.message : t(key, note.args ?? {});
  };
}

export function SourceButton({
  source,
  onShow,
  testId,
}: {
  readonly source: Source | undefined;
  readonly onShow: (source: Source) => void;
  readonly testId?: string;
}): ReactNode {
  const { t } = useAppState();
  if (source === undefined) {
    return null;
  }
  const label = t('partgen.source', { page: source.page });
  return (
    <button
      type="button"
      className="partgen-source"
      title={label}
      aria-label={label}
      data-testid={testId}
      onClick={() => {
        onShow(source);
      }}
    >
      {t('partgen.page', { page: source.page })}
    </button>
  );
}

/**
 * Help from the user's own AI agent through the MCP server: the draft is
 * saved as a file, the agent corrects it, the app loads it again. No API
 * key is stored in the app.
 */
function AgentHelp({
  draft,
  onChange,
}: {
  readonly draft: Draft;
  readonly onChange: (draft: Draft) => void;
}): ReactNode {
  const { engine, t } = useAppState();
  const [file, setFile] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  return (
    <details className="partgen-agent" data-testid="partgen-agent">
      <summary>{t('partgen.ai.title')}</summary>
      <p className="field__hint">{t('partgen.ai.intro')}</p>
      <div className="partgen-agent__actions">
        <button
          type="button"
          className="button button--small"
          data-testid="partgen-agent-save"
          onClick={() => {
            void engine
              .call<{ path: string }>('part.draftSave', { draft })
              .then((saved) => {
                setFile(saved.path);
                setStatus(t('partgen.ai.saved', { path: saved.path }));
              })
              .catch((error: unknown) => {
                setStatus(error instanceof Error ? error.message : String(error));
              });
          }}
        >
          {t('partgen.ai.save')}
        </button>
        <button
          type="button"
          className="button button--small"
          disabled={file === null}
          data-testid="partgen-agent-copy"
          onClick={() => {
            const request = t('partgen.ai.request', {
              datasheet: draft.datasheet ?? '',
              draft: file ?? '',
            });
            void navigator.clipboard.writeText(request).then(() => {
              setStatus(t('partgen.ai.copied'));
            });
          }}
        >
          {t('partgen.ai.copy')}
        </button>
        <button
          type="button"
          className="button button--small"
          disabled={file === null}
          data-testid="partgen-agent-load"
          onClick={() => {
            void engine
              .call<Draft>('part.draftLoad', { path: file })
              .then((loaded) => {
                onChange(loaded);
                setStatus(t('partgen.ai.loaded'));
              })
              .catch((error: unknown) => {
                setStatus(error instanceof Error ? error.message : String(error));
              });
          }}
        >
          {t('partgen.ai.load')}
        </button>
      </div>
      {status !== null && (
        <p className="field__hint" data-testid="partgen-agent-status">
          {status}
        </p>
      )}
    </details>
  );
}

export function PartStep({
  draft,
  onChange,
  onShow,
}: {
  readonly draft: Draft;
  readonly onChange: (draft: Draft) => void;
  readonly onShow: (source: Source) => void;
}): ReactNode {
  const { t } = useAppState();
  const note = useNote();
  return (
    <div className="partgen-step" data-testid="partgen-step-part">
      <div className="partgen-row">
        <TextInput
          label={t('partgen.part.name')}
          value={draft.name}
          testId="partgen-name"
          onSave={(name) => {
            onChange({ ...draft, name: name.trim() });
          }}
        />
        <SourceButton source={draft.source} onShow={onShow} testId="partgen-name-source" />
      </div>
      <TextInput
        label={t('partgen.part.manufacturer')}
        value={draft.manufacturer ?? ''}
        testId="partgen-manufacturer"
        onSave={(manufacturer) => {
          onChange({ ...draft, manufacturer });
        }}
      />
      <TextInput
        label={t('partgen.part.mpn')}
        value={draft.mpn ?? ''}
        testId="partgen-mpn"
        onSave={(mpn) => {
          onChange({ ...draft, mpn });
        }}
      />
      <TextInput
        label={t('partgen.part.description')}
        value={draft.description ?? ''}
        testId="partgen-description"
        onSave={(description) => {
          onChange({ ...draft, description });
        }}
      />
      <AgentHelp draft={draft} onChange={onChange} />
      {(draft.notes?.length ?? 0) > 0 && (
        <div className="partgen-notes" data-testid="partgen-notes">
          <h4 className="partgen-subtitle">{t('partgen.part.notes')}</h4>
          <ul>
            {draft.notes?.map((n, i) => (
              // eslint-disable-next-line @eslint-react/no-array-index-key -- notes have no id
              <li key={i}>{note(n)}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

function DimensionCell({
  value,
  editable,
  testId,
  onSave,
}: {
  readonly value: number | undefined;
  readonly editable: boolean;
  readonly testId: string;
  readonly onSave: (value: number | null) => void;
}): ReactNode {
  const { t } = useAppState();
  if (!editable) {
    return <span data-testid={testId}>{mm(value)}</span>;
  }
  return (
    <TextInput
      bare
      ariaLabel={t('partgen.package.dimensions')}
      value={mm(value)}
      testId={testId}
      onSave={(text) => {
        const trimmed = text.trim().replace(',', '.');
        const parsed = Number(trimmed);
        onSave(trimmed === '' || !Number.isFinite(parsed) ? null : parsed);
      }}
    />
  );
}

export function PackageStep({
  draft,
  pkg,
  advanced,
  showAll,
  onShowAll,
  onSelect,
  onChange,
  onShow,
}: {
  readonly draft: Draft;
  readonly pkg: string | null;
  readonly advanced: boolean;
  readonly showAll: boolean;
  readonly onShowAll: (all: boolean) => void;
  readonly onSelect: (name: string) => void;
  readonly onChange: (draft: Draft) => void;
  readonly onShow: (source: Source) => void;
}): ReactNode {
  const { t } = useAppState();
  const current = draft.packages.find((p) => p.name === pkg) ?? null;
  const familyOptions = FAMILIES.map(
    (family) => [family, t(`partgen.families.${family}`)] as const,
  );
  const keys =
    current === null
      ? []
      : advanced || showAll
        ? dimensionKeys(current.family, current.exposedPad ?? false)
        : mainKeys(current.family);
  const dims: Readonly<Record<string, Dimension>> = current?.dimensions ?? {};
  const fromTable = Object.values(dims).filter((d) => d.source !== undefined).length;
  const standard = Object.values(dims).length - fromTable;
  return (
    <div className="partgen-step" data-testid="partgen-step-package">
      {draft.packages.length === 0 && <p className="field__hint">{t('partgen.package.none')}</p>}
      <div className="partgen-packages" role="radiogroup" aria-label={t('partgen.package.choose')}>
        {draft.packages.map((p) => {
          const pins = draft.pins.filter((pin) => (pin.numbers[p.name]?.length ?? 0) > 0).length;
          return (
            <label
              key={p.name}
              className={`partgen-package${p.name === pkg ? ' partgen-package--active' : ''}`}
              data-testid={`partgen-package-${p.name}`}
            >
              <input
                type="radio"
                name="partgen-package"
                checked={p.name === pkg}
                onChange={() => {
                  onSelect(p.name);
                }}
              />
              <span className="partgen-package__name">{p.name}</span>
              <span className="field__hint">
                {t(`partgen.families.${p.family}`)}
                {' · '}
                {t('partgen.pins.count', { count: pins, package: p.name })}
              </span>
            </label>
          );
        })}
      </div>
      {current !== null && (
        <>
          {(advanced || current.family === 'other') && (
            <SelectInput<Family>
              label={t('partgen.package.family')}
              value={current.family}
              options={familyOptions}
              testId="partgen-family"
              onSave={(family) => {
                onChange(setPackage(draft, current.name, { family }));
              }}
            />
          )}
          {current.family === 'other' && (
            <p className="field__hint">{t('partgen.package.other')}</p>
          )}
          {advanced && (
            <CheckInput
              label={t('partgen.package.exposedPad')}
              checked={current.exposedPad ?? false}
              testId="partgen-exposed-pad"
              onSave={(exposedPad) => {
                onChange(setPackage(draft, current.name, { exposedPad }));
              }}
            />
          )}
          {keys.length > 0 && (
            <>
              <h4 className="partgen-subtitle">
                {t('partgen.package.dimensions')}
                {current.drawing !== undefined && (
                  <button
                    type="button"
                    className="button button--small button--quiet"
                    data-testid="partgen-drawing"
                    onClick={() => {
                      if (current.drawing !== undefined) {
                        onShow(current.drawing);
                      }
                    }}
                  >
                    {t('partgen.package.drawing')}
                  </button>
                )}
              </h4>
              <p className="field__hint" data-testid="partgen-dimension-summary">
                {t('partgen.package.fromTable', { count: fromTable, standard })}
              </p>
              <table className="board-table partgen-dimensions" data-testid="partgen-dimensions">
                <thead>
                  <tr>
                    <th>{t('partgen.package.symbol')}</th>
                    <th>{t('partgen.package.meaning')}</th>
                    <th>{t('partgen.package.min')}</th>
                    <th>{t('partgen.package.nom')}</th>
                    <th>{t('partgen.package.max')}</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {keys.map((key) => {
                    const d = dims[key];
                    const meaning = (
                      [
                        'e',
                        'D',
                        'E',
                        'E1',
                        'D1',
                        'D2',
                        'E2',
                        'b',
                        'b1',
                        'b2',
                        'L',
                        'A',
                        'A1',
                        'c',
                      ] as const
                    ).find((k) => k === key);
                    return (
                      <tr key={key} data-testid={`partgen-dim-${key}`}>
                        <td className="partgen-dimensions__symbol">{key}</td>
                        <td>{meaning === undefined ? '' : t(`partgen.dims.${meaning}`)}</td>
                        {(['min', 'nom', 'max'] as const).map((slot) => (
                          <td key={slot}>
                            <DimensionCell
                              value={d?.[slot]}
                              editable={advanced}
                              testId={`partgen-dim-${key}-${slot}`}
                              onSave={(value) => {
                                onChange(setDimension(draft, current.name, key, slot, value));
                              }}
                            />
                          </td>
                        ))}
                        <td>
                          {d?.source === undefined ? (
                            <span className="partgen-standard" title={t('partgen.standardHint')}>
                              {t('partgen.standard')}
                            </span>
                          ) : (
                            <SourceButton
                              source={d.source}
                              onShow={onShow}
                              testId={`partgen-dim-${key}-source`}
                            />
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
              {!advanced && (
                <button
                  type="button"
                  className="button button--small button--quiet"
                  data-testid="partgen-all-dimensions"
                  onClick={() => {
                    onShowAll(!showAll);
                  }}
                >
                  {showAll ? t('partgen.package.main') : t('partgen.package.all')}
                </button>
              )}
            </>
          )}
        </>
      )}
    </div>
  );
}

export function PinsStep({
  draft,
  pkg,
  onChange,
  onShow,
}: {
  readonly draft: Draft;
  readonly pkg: string;
  readonly onChange: (draft: Draft) => void;
  readonly onShow: (source: Source) => void;
}): ReactNode {
  const { t } = useAppState();
  const rows = pinRows(draft, pkg);
  const current = draft.packages.find((p) => p.name === pkg);
  const missing = current === undefined ? [] : missingPads(draft, current);
  const typeOptions = PIN_TYPES.map((kind) => [kind, t(`partgen.types.${kind}`)] as const);
  return (
    <div className="partgen-step" data-testid="partgen-step-pins">
      {rows.length === 0 && <p className="field__hint">{t('partgen.package.noPins')}</p>}
      <div className="board-table__scroll partgen-pins-scroll">
        <table className="board-table partgen-pins" data-testid="partgen-pins">
          <thead>
            <tr>
              <th>{t('partgen.pins.pad')}</th>
              <th>{t('partgen.pins.name')}</th>
              <th>{t('partgen.pins.type')}</th>
              <th>{t('partgen.pins.function')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.index} data-testid={`partgen-pin-${String(row.index)}`}>
                <td className="partgen-pins__pad">
                  <TextInput
                    bare
                    ariaLabel={t('partgen.pins.pad')}
                    value={row.numbers.join(', ')}
                    testId={`partgen-pin-pad-${String(row.index)}`}
                    onSave={(text) => {
                      onChange(setPinNumbers(draft, row.index, pkg, parseNumbers(text)));
                    }}
                  />
                </td>
                <td>
                  <TextInput
                    bare
                    ariaLabel={t('partgen.pins.name')}
                    value={row.pin.name}
                    testId={`partgen-pin-name-${String(row.index)}`}
                    onSave={(name) => {
                      onChange(setPin(draft, row.index, { name: name.trim() }));
                    }}
                  />
                </td>
                <td>
                  <SelectInput<PinType>
                    bare
                    ariaLabel={t('partgen.pins.type')}
                    value={row.pin.kind ?? 'unspecified'}
                    options={typeOptions}
                    testId={`partgen-pin-type-${String(row.index)}`}
                    onSave={(kind) => {
                      onChange(setPin(draft, row.index, { kind }));
                    }}
                  />
                </td>
                <td className="partgen-pins__function" title={row.pin.function ?? ''}>
                  {row.pin.function ?? ''}
                </td>
                <td className="partgen-pins__actions">
                  <SourceButton
                    source={row.pin.source}
                    onShow={onShow}
                    testId={`partgen-pin-source-${String(row.index)}`}
                  />
                  <button
                    type="button"
                    className="button button--small button--quiet"
                    aria-label={t('partgen.pins.remove', { name: row.pin.name })}
                    title={t('partgen.pins.remove', { name: row.pin.name })}
                    onClick={() => {
                      onChange(removePin(draft, row.index));
                    }}
                  >
                    ×
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {missing.length > 0 && (
        <p className="field__hint" data-testid="partgen-missing">
          {t('partgen.pins.missing', { pads: missing.join(', ') })}
        </p>
      )}
      <button
        type="button"
        className="button button--small board-add"
        data-testid="partgen-add-pin"
        onClick={() => {
          onChange(addPin(draft, pkg, t('partgen.pins.newName'), 'unspecified'));
        }}
      >
        {t('partgen.pins.add')}
      </button>
    </div>
  );
}
