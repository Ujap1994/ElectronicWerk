import { useEffect, useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';

import { Icon } from '../components/Icon';
import { useAppState } from '../state';
import { layerLabel } from '../viewer/labels';
import {
  boardLayers,
  OUTPUT_KINDS,
  option,
  withOption,
  type BomColumn,
  type BomPreview,
  type OutputKind,
  type OutputSpec,
} from './model';

type Change = (spec: OutputSpec) => void;

/** A text field that commits on Enter or blur. */
export function TextOption({
  label,
  value,
  hint,
  testId,
  onCommit,
}: {
  readonly label: string;
  readonly value: string;
  readonly hint?: string;
  readonly testId: string;
  readonly onCommit: (value: string) => void;
}): ReactNode {
  const [draft, setDraft] = useState(value);
  const [shown, setShown] = useState(value);
  if (shown !== value) {
    setShown(value);
    setDraft(value);
  }
  const commit = (): void => {
    if (draft !== value) {
      onCommit(draft);
    }
  };
  return (
    <label className="field">
      <span className="field__label">{label}</span>
      <input
        className="field__control"
        type="text"
        value={draft}
        data-testid={testId}
        onChange={(event) => {
          setDraft(event.target.value);
        }}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            commit();
          } else if (event.key === 'Escape') {
            setDraft(value);
          }
        }}
      />
      {hint !== undefined && <span className="field__hint">{hint}</span>}
    </label>
  );
}

function Check({
  label,
  checked,
  testId,
  onChange,
}: {
  readonly label: string;
  readonly checked: boolean;
  readonly testId: string;
  readonly onChange: (checked: boolean) => void;
}): ReactNode {
  return (
    <label className="checkbox">
      <input
        type="checkbox"
        checked={checked}
        data-testid={testId}
        onChange={(event) => {
          onChange(event.target.checked);
        }}
      />
      <span>{label}</span>
    </label>
  );
}

function Choice({
  label,
  value,
  options,
  testId,
  onChange,
}: {
  readonly label: string;
  readonly value: string;
  readonly options: readonly (readonly [string, string])[];
  readonly testId: string;
  readonly onChange: (value: string) => void;
}): ReactNode {
  return (
    <label className="field">
      <span className="field__label">{label}</span>
      <select
        className="field__control"
        value={value}
        data-testid={testId}
        onChange={(event) => {
          onChange(event.target.value);
        }}
      >
        {options.map(([id, text]) => (
          <option key={id} value={id}>
            {text}
          </option>
        ))}
      </select>
    </label>
  );
}

/** A check box per board layer. */
function LayerChoice({
  layers,
  selected,
  testId,
  onChange,
}: {
  readonly layers: readonly string[];
  readonly selected: readonly string[];
  readonly testId: string;
  readonly onChange: (layers: string[]) => void;
}): ReactNode {
  const { t } = useAppState();
  return (
    <div className="outputs__layers" data-testid={testId}>
      {layers.map((layer) => (
        <Check
          key={layer}
          label={layerLabel(t, layer)}
          checked={selected.includes(layer)}
          testId={`${testId}-${layer}`}
          onChange={(on) => {
            const next = on
              ? layers.filter((l) => l === layer || selected.includes(l))
              : selected.filter((l) => l !== layer);
            onChange(next);
          }}
        />
      ))}
    </div>
  );
}

const FIELD_LABELS: Readonly<Record<string, MessageKey>> = {
  item: 'outputs.bom.fields.item',
  quantity: 'outputs.bom.fields.quantity',
  designators: 'outputs.bom.fields.designators',
  value: 'outputs.bom.fields.value',
  footprint: 'outputs.bom.fields.footprint',
  symbol: 'outputs.bom.fields.symbol',
  description: 'outputs.bom.fields.description',
  side: 'outputs.bom.fields.side',
  mounting: 'outputs.bom.fields.mounting',
  dnp: 'outputs.bom.fields.dnp',
};

/** The label of a BOM field. */
export function useFieldLabel(): (field: string) => string {
  const { t } = useAppState();
  return (field) => {
    const key = FIELD_LABELS[field];
    return key === undefined ? field : t(key);
  };
}

function BomSettings({
  spec,
  onChange,
}: {
  readonly spec: OutputSpec;
  readonly onChange: Change;
}): ReactNode {
  const { t, engine } = useAppState();
  const fieldLabel = useFieldLabel();
  const columns = option<readonly BomColumn[]>(spec, 'columns');
  const groupBy = option<readonly string[]>(spec, 'groupBy');
  const includeDnp = option<boolean>(spec, 'includeDnp');
  const separator = option<string>(spec, 'separator');
  const [preview, setPreview] = useState<BomPreview | null>(null);
  const optionsKey = JSON.stringify({ columns, groupBy, includeDnp, separator });
  useEffect(() => {
    let cancelled = false;
    const options = JSON.parse(optionsKey) as Record<string, unknown>;
    void engine
      .call<BomPreview>('outputs.bom', { options })
      .then((result) => {
        if (!cancelled) {
          setPreview(result);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setPreview(null);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [engine, optionsKey]);
  const fields = preview?.fields ?? [];
  const setColumns = (next: readonly BomColumn[]): void => {
    onChange(withOption(spec, 'columns', next));
  };
  return (
    <>
      <Choice
        label={t('outputs.bom.format')}
        value={option<string>(spec, 'format')}
        options={[
          ['csv', t('outputs.bom.csv')],
          ['xlsx', t('outputs.bom.xlsx')],
          ['html', t('outputs.bom.html')],
        ]}
        testId="output-bom-format"
        onChange={(value) => {
          onChange(withOption(spec, 'format', value));
        }}
      />
      <fieldset className="field field--group">
        <legend className="field__label">{t('outputs.bom.columns')}</legend>
        <table className="table outputs__columns" data-testid="output-bom-columns">
          <thead>
            <tr>
              <th>{t('outputs.bom.field')}</th>
              <th>{t('outputs.bom.heading')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {columns.map((column, index) => (
              <tr key={`${column.field}-${String(index)}`}>
                <td>{fieldLabel(column.field)}</td>
                <td>
                  <input
                    className="field__control field__control--table"
                    type="text"
                    defaultValue={column.label ?? ''}
                    placeholder={fieldLabel(column.field)}
                    aria-label={t('outputs.bom.heading')}
                    onBlur={(event) => {
                      const label = event.target.value.trim();
                      if (label !== (column.label ?? '')) {
                        setColumns(
                          columns.map((c, i) =>
                            i === index
                              ? label === ''
                                ? { field: c.field }
                                : { field: c.field, label }
                              : c,
                          ),
                        );
                      }
                    }}
                  />
                </td>
                <td className="outputs__row-actions">
                  <button
                    type="button"
                    className="icon-button"
                    title={t('outputs.moveUp')}
                    aria-label={t('outputs.moveUp')}
                    disabled={index === 0}
                    onClick={() => {
                      const next = [...columns];
                      const [entry] = next.splice(index, 1);
                      if (entry !== undefined) {
                        next.splice(index - 1, 0, entry);
                        setColumns(next);
                      }
                    }}
                  >
                    <Icon name="chevron" size={14} className="icon--up" />
                  </button>
                  <button
                    type="button"
                    className="icon-button"
                    title={t('outputs.bom.removeColumn')}
                    aria-label={t('outputs.bom.removeColumn')}
                    onClick={() => {
                      setColumns(columns.filter((_, i) => i !== index));
                    }}
                  >
                    <Icon name="trash" size={14} />
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        <select
          className="field__control"
          value=""
          data-testid="output-bom-add-column"
          aria-label={t('outputs.bom.addColumn')}
          onChange={(event) => {
            if (event.target.value !== '') {
              setColumns([...columns, { field: event.target.value }]);
            }
          }}
        >
          <option value="">{t('outputs.bom.addColumn')}</option>
          {fields.map((field) => (
            <option key={field} value={field}>
              {fieldLabel(field)}
            </option>
          ))}
        </select>
      </fieldset>
      <fieldset className="field field--group">
        <legend className="field__label">{t('outputs.bom.groupBy')}</legend>
        <div className="outputs__layers">
          {fields
            .filter((field) => field !== 'item' && field !== 'quantity' && field !== 'designators')
            .map((field) => (
              <Check
                key={field}
                label={fieldLabel(field)}
                checked={groupBy.includes(field)}
                testId={`output-bom-group-${field}`}
                onChange={(on) => {
                  onChange(
                    withOption(
                      spec,
                      'groupBy',
                      on ? [...groupBy, field] : groupBy.filter((g) => g !== field),
                    ),
                  );
                }}
              />
            ))}
        </div>
        {groupBy.length === 0 && <span className="field__hint">{t('outputs.bom.noGrouping')}</span>}
      </fieldset>
      <Check
        label={t('outputs.bom.includeDnp')}
        checked={includeDnp}
        testId="output-bom-dnp"
        onChange={(on) => {
          onChange(withOption(spec, 'includeDnp', on));
        }}
      />
      <Choice
        label={t('outputs.bom.separator')}
        value={separator}
        options={[
          [',', ', (comma)'],
          [';', '; (semicolon)'],
          ['\t', 'Tab'],
        ]}
        testId="output-bom-separator"
        onChange={(value) => {
          onChange(withOption(spec, 'separator', value));
        }}
      />
      {preview !== null && (
        <section className="outputs__preview" data-testid="output-bom-preview">
          <h4 className="outputs__subtitle">
            {t('outputs.bom.preview')}{' '}
            <span className="field__hint">
              {t('outputs.bom.previewCount', {
                lines: preview.rows.length,
                parts: preview.parts,
              })}
            </span>
          </h4>
          <div className="outputs__preview-table">
            <table className="table">
              <thead>
                <tr>
                  {preview.headers.map((header, index) => (
                    <th key={`${header}-${String(index)}`}>{header}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {preview.rows.slice(0, 50).map((row) => (
                  <tr key={row.join('\u0000')}>
                    {row.map((cell, column) => (
                      <td key={`${preview.headers[column] ?? ''}:${String(column)}`}>{cell}</td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>
      )}
    </>
  );
}

const KIND_LABELS: Readonly<Record<OutputKind, MessageKey>> = {
  gerber: 'outputKinds.gerber',
  drill: 'outputKinds.drill',
  drillMap: 'outputKinds.drillMap',
  bom: 'outputKinds.bom',
  pickAndPlace: 'outputKinds.pickAndPlace',
  pdf: 'outputKinds.pdf',
  svg: 'outputKinds.svg',
  step: 'outputKinds.step',
  ipc356: 'outputKinds.ipc356',
  ipc2581: 'outputKinds.ipc2581',
  netlist: 'outputKinds.netlist',
  archive: 'outputKinds.archive',
};

/** The translated name of an output kind. */
export function kindLabel(t: (key: MessageKey) => string, kind: OutputKind): string {
  return t(KIND_LABELS[kind]);
}

/** The settings of one output. */
export function OutputSettings({
  spec,
  copperCount,
  boards,
  onChange,
}: {
  readonly spec: OutputSpec;
  readonly copperCount: number;
  readonly boards: readonly { readonly id: string; readonly name: string }[];
  readonly onChange: Change;
}): ReactNode {
  const { t } = useAppState();
  const set = (key: string, value: unknown): void => {
    onChange(withOption(spec, key, value));
  };
  const check = (key: string, label: MessageKey): ReactNode => (
    <Check
      label={t(label)}
      checked={option<boolean>(spec, key)}
      testId={`output-option-${key}`}
      onChange={(on) => {
        set(key, on);
      }}
    />
  );
  const layers = boardLayers(copperCount);
  const usesBoard = !['bom', 'netlist', 'step'].includes(spec.kind);
  const content = option<string>(spec, 'content', 'schematic');
  const showsBoard =
    usesBoard && !((spec.kind === 'pdf' || spec.kind === 'svg') && content === 'schematic');
  const folder = option<string | undefined>(spec, 'folder', undefined);
  return (
    <div className="outputs__form" data-testid="output-settings">
      <TextOption
        label={t('outputs.name')}
        value={spec.name}
        testId="output-name"
        onCommit={(name) => {
          if (name.trim() !== '') {
            onChange({ ...spec, name: name.trim() });
          }
        }}
      />
      <div className="field">
        <span className="field__label">{t('outputs.kind')}</span>
        <span>{kindLabel(t, spec.kind)}</span>
      </div>
      {showsBoard && boards.length > 1 && (
        <Choice
          label={t('outputs.board')}
          value={spec.source ?? ''}
          options={[['', t('outputs.firstBoard')], ...boards.map((b) => [b.id, b.name] as const)]}
          testId="output-board"
          onChange={(value) => {
            onChange({ ...spec, source: value === '' ? null : value });
          }}
        />
      )}
      {spec.kind === 'gerber' && (
        <>
          <fieldset className="field field--group">
            <legend className="field__label">{t('outputs.gerber.layers')}</legend>
            <Check
              label={t('outputs.gerber.layersDefault')}
              checked={option<readonly string[]>(spec, 'layers').length === 0}
              testId="output-gerber-default-layers"
              onChange={(on) => {
                set('layers', on ? [] : layers.filter((l) => !l.startsWith('adhesive')));
              }}
            />
            {option<readonly string[]>(spec, 'layers').length > 0 && (
              <LayerChoice
                layers={layers}
                selected={option<readonly string[]>(spec, 'layers')}
                testId="output-gerber-layers"
                onChange={(next) => {
                  set('layers', next);
                }}
              />
            )}
          </fieldset>
          {check('protelExtensions', 'outputs.gerber.protel')}
          {check('x2', 'outputs.gerber.x2')}
          {check('jobFile', 'outputs.gerber.jobFile')}
          {check('tentVias', 'outputs.gerber.tentVias')}
          {check('silkValues', 'outputs.gerber.silkValues')}
        </>
      )}
      {spec.kind === 'drill' && check('merge', 'outputs.drill.merge')}
      {spec.kind === 'drillMap' && (
        <Choice
          label={t('outputs.drawing.format')}
          value={option<string>(spec, 'format')}
          options={[
            ['pdf', t('outputs.drawing.pdf')],
            ['svg', t('outputs.drawing.svg')],
          ]}
          testId="output-format"
          onChange={(value) => {
            set('format', value);
          }}
        />
      )}
      {spec.kind === 'bom' && <BomSettings spec={spec} onChange={onChange} />}
      {spec.kind === 'pickAndPlace' && (
        <>
          <Choice
            label={t('outputs.placement.style')}
            value={option<string>(spec, 'style')}
            options={[
              ['generic', t('outputs.placement.styleGeneric')],
              ['jlcpcb', t('outputs.placement.styleJlcpcb')],
              ['kicad', t('outputs.placement.styleKicad')],
            ]}
            testId="output-placement-style"
            onChange={(value) => {
              set('style', value);
            }}
          />
          <Choice
            label={t('outputs.placement.unit')}
            value={option<string>(spec, 'unit')}
            options={[
              ['mm', t('outputs.placement.mm')],
              ['inch', t('outputs.placement.inch')],
            ]}
            testId="output-placement-unit"
            onChange={(value) => {
              set('unit', value);
            }}
          />
          {check('boardOrigin', 'outputs.placement.boardOrigin')}
          {check('includeDnp', 'outputs.placement.includeDnp')}
          {check('splitSides', 'outputs.placement.splitSides')}
          {check('mirrorBottomX', 'outputs.placement.mirrorBottom')}
        </>
      )}
      {(spec.kind === 'pdf' || spec.kind === 'svg') && (
        <>
          <Choice
            label={t('outputs.drawing.content')}
            value={content}
            options={[
              ['schematic', t('outputs.drawing.contentSchematic')],
              ['board', t('outputs.drawing.contentBoard')],
              ['assembly', t('outputs.drawing.contentAssembly')],
            ]}
            testId="output-content"
            onChange={(value) => {
              set('content', value);
            }}
          />
          {content === 'board' && (
            <>
              <fieldset className="field field--group">
                <legend className="field__label">{t('outputs.gerber.layers')}</legend>
                <LayerChoice
                  layers={layers}
                  selected={option<readonly string[]>(spec, 'layers')}
                  testId="output-print-layers"
                  onChange={(next) => {
                    set('layers', next);
                  }}
                />
              </fieldset>
              {check('pagePerLayer', 'outputs.drawing.pagePerLayer')}
              {check('mirror', 'outputs.drawing.mirror')}
            </>
          )}
          {content === 'assembly' && check('values', 'outputs.drawing.values')}
          {content !== 'assembly' && (
            <Choice
              label={t('outputs.drawing.color')}
              value={option<string>(spec, 'color')}
              options={[
                ['color', t('outputs.drawing.colorColor')],
                ['monochrome', t('outputs.drawing.colorMonochrome')],
              ]}
              testId="output-color"
              onChange={(value) => {
                set('color', value);
              }}
            />
          )}
          <Choice
            label={t('outputs.drawing.paper')}
            value={option<string>(spec, 'paper')}
            options={[
              ['fit', t('outputs.drawing.paperFit')],
              ['a4', t('outputs.drawing.paperA4')],
              ['a3', t('outputs.drawing.paperA3')],
              ['letter', t('outputs.drawing.paperLetter')],
            ]}
            testId="output-paper"
            onChange={(value) => {
              set('paper', value);
            }}
          />
        </>
      )}
      {spec.kind === 'ipc356' && check('tentVias', 'outputs.netlist.tentVias')}
      {spec.kind === 'netlist' && (
        <Choice
          label={t('outputs.netlist.format')}
          value={option<string>(spec, 'format')}
          options={[
            ['kicad', t('outputs.netlist.kicad')],
            ['csv', t('outputs.netlist.csv')],
          ]}
          testId="output-netlist-format"
          onChange={(value) => {
            set('format', value);
          }}
        />
      )}
      {spec.kind === 'archive' && (
        <>
          <fieldset className="field field--group">
            <legend className="field__label">{t('outputs.archive.include')}</legend>
            <div className="outputs__layers">
              {OUTPUT_KINDS.filter((k) => k !== 'archive' && k !== 'step').map((kind) => {
                const include = option<readonly string[]>(spec, 'include');
                return (
                  <Check
                    key={kind}
                    label={kindLabel(t, kind)}
                    checked={include.includes(kind)}
                    testId={`output-archive-${kind}`}
                    onChange={(on) => {
                      set('include', on ? [...include, kind] : include.filter((k) => k !== kind));
                    }}
                  />
                );
              })}
            </div>
          </fieldset>
          <TextOption
            label={t('outputs.archive.file')}
            value={option<string>(spec, 'file')}
            hint={t('outputs.archive.fileHint')}
            testId="output-archive-file"
            onCommit={(value) => {
              set('file', value.trim());
            }}
          />
        </>
      )}
      {spec.kind === 'step' && (
        <p className="callout callout--info">
          <Icon name="info" />
          <span>{t('outputs.step')}</span>
        </p>
      )}
      {folder !== undefined && (
        <TextOption
          label={t('outputs.subfolder')}
          value={folder}
          hint={t('outputs.subfolderHint')}
          testId="output-folder"
          onCommit={(value) => {
            set('folder', value.trim().replace(/^\/+|\/+$/gu, ''));
          }}
        />
      )}
    </div>
  );
}
