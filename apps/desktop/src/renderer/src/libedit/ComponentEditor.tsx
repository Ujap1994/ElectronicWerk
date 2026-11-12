/**
 * The component editor: symbol, footprint, value, manufacturer data,
 * suppliers and fields of an orderable part, with a preview and the
 * pin-to-pad check.
 */
import { useState, type ReactNode } from 'react';

import { Icon } from '../components/Icon';
import { SymbolPreview } from '../schematic/SymbolShapes';
import { useAppState } from '../state';
import { useLibraryItem } from './data';
import { findingText } from './labels';
import { TextInput } from './fields';
import { FootprintSvg } from './FootprintSvg';
import { FieldsEditor, FootprintField } from './properties';
import type {
  ComponentDef,
  Finding,
  FootprintDef,
  LibraryKind,
  SupplierPart,
  SymbolDef,
  Usage,
} from './types';

type SetProps = (props: Record<string, unknown>) => void;

const collator = new Intl.Collator(undefined, { numeric: true });

/** Pins (normal bodies of all units) and pads by number. */
function pinMap(
  symbol: SymbolDef | null,
  footprint: FootprintDef | null,
): { number: string; pin: string | null; pad: boolean }[] {
  const pins = new Map<string, string>();
  for (const unit of symbol?.units ?? []) {
    for (const pin of unit.pins ?? []) {
      if (!pins.has(pin.number)) {
        pins.set(pin.number, pin.name ?? '');
      }
    }
  }
  const pads = new Set(
    (footprint?.pads ?? []).map((pad) => pad.number).filter((number) => number !== ''),
  );
  const numbers = [...new Set([...pins.keys(), ...pads])].sort(collator.compare);
  return numbers.map((number) => ({
    number,
    pin: pins.get(number) ?? null,
    pad: pads.has(number),
  }));
}

function Suppliers({
  suppliers,
  onSave,
}: {
  readonly suppliers: readonly SupplierPart[];
  readonly onSave: (suppliers: SupplierPart[]) => void;
}): ReactNode {
  const { t } = useAppState();
  const [supplier, setSupplier] = useState('');
  const change = (index: number, patch: Partial<SupplierPart>): void => {
    onSave(
      suppliers.map((entry, i) => {
        if (i !== index) {
          return entry;
        }
        const next = { ...entry, ...patch };
        // An empty link is removed.
        return next.url === '' ? { supplier: next.supplier, partNumber: next.partNumber } : next;
      }),
    );
  };
  return (
    <fieldset className="field--group" data-testid="component-suppliers">
      <legend>{t('libedit.component.suppliers')}</legend>
      {suppliers.length > 0 && (
        <table className="table libedit-suppliers">
          <thead>
            <tr>
              <th>{t('libedit.component.supplier')}</th>
              <th>{t('libedit.component.partNumber')}</th>
              <th>{t('libedit.component.url')}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {suppliers.map((entry, index) => (
              // Suppliers are kept in order; the index is the identity.
              // eslint-disable-next-line @eslint-react/no-array-index-key -- see above
              <tr key={index}>
                <td>
                  <TextInput
                    bare
                    label={t('libedit.component.supplier')}
                    value={entry.supplier}
                    onSave={(value) => {
                      change(index, { supplier: value });
                    }}
                  />
                </td>
                <td>
                  <TextInput
                    bare
                    label={t('libedit.component.partNumber')}
                    value={entry.partNumber}
                    testId={`supplier-pn-${String(index)}`}
                    onSave={(value) => {
                      change(index, { partNumber: value });
                    }}
                  />
                </td>
                <td>
                  <TextInput
                    bare
                    label={t('libedit.component.url')}
                    value={entry.url ?? ''}
                    onSave={(value) => {
                      change(index, { url: value });
                    }}
                  />
                </td>
                <td>
                  <button
                    type="button"
                    className="button button--quiet button--small"
                    aria-label={t('libedit.component.removeSupplier', { name: entry.supplier })}
                    title={t('libedit.component.removeSupplier', { name: entry.supplier })}
                    onClick={() => {
                      onSave(suppliers.filter((_, i) => i !== index));
                    }}
                  >
                    <Icon name="trash" size={14} />
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      <form
        className="inspector-edit__add"
        onSubmit={(event) => {
          event.preventDefault();
          if (supplier.trim() !== '') {
            onSave([...suppliers, { supplier: supplier.trim(), partNumber: '' }]);
            setSupplier('');
          }
        }}
      >
        <input
          className="field__control"
          value={supplier}
          placeholder={t('libedit.component.supplier')}
          aria-label={t('libedit.component.supplier')}
          data-testid="supplier-new"
          onChange={(event) => {
            setSupplier(event.target.value);
          }}
        />
        <button
          type="submit"
          className="button button--small"
          disabled={supplier.trim() === ''}
          data-testid="supplier-add"
        >
          {t('libedit.component.addSupplier')}
        </button>
      </form>
    </fieldset>
  );
}

export function ComponentEditor({
  doc,
  def,
  usage,
  findings,
  symbols,
  footprints,
  setProps,
  onOpen,
  onRename,
}: {
  readonly doc: string;
  readonly def: ComponentDef;
  readonly usage: Usage;
  readonly findings: readonly Finding[];
  readonly symbols: readonly string[];
  readonly footprints: readonly string[];
  readonly setProps: SetProps;
  readonly onOpen: (kind: LibraryKind, name: string) => void;
  readonly onRename: () => void;
}): ReactNode {
  const { t } = useAppState();
  const symbol = useLibraryItem<SymbolDef>(doc, 'symbol', def.symbol === '' ? null : def.symbol);
  const footprint = useLibraryItem<FootprintDef>(
    doc,
    'footprint',
    def.footprint === undefined || def.footprint === '' ? null : def.footprint,
  );
  const map = pinMap(symbol?.def ?? null, footprint?.def ?? null);
  const text = (key: keyof ComponentDef, label: string, testId?: string): ReactNode => (
    <TextInput
      label={label}
      value={typeof def[key] === 'string' ? def[key] : ''}
      {...(testId === undefined ? {} : { testId })}
      onSave={(value) => {
        setProps({ [key]: value });
      }}
    />
  );
  const used = usage.placed + usage.board + usage.symbols + usage.components > 0;
  return (
    <div className="component-editor" data-testid="component-editor">
      <div className="component-editor__form">
        <div className="libedit-props__name">
          <h3 className="libedit-props__value" data-testid="item-name">
            {def.name}
          </h3>
          <button
            type="button"
            className="button button--small button--quiet"
            data-testid="item-rename"
            onClick={onRename}
          >
            <Icon name="edit" size={14} />
            {t('libedit.list.rename')}
          </button>
        </div>
        <p className="dialog__text--muted">{t('libedit.component.intro')}</p>
        <p className="libedit-props__usage" data-testid="item-usage">
          {used ? t('libedit.props.usage', { ...usage }) : t('libedit.props.unused')}
        </p>
        <label className="field field--compact">
          <span className="field__label">{t('libedit.component.symbol')}</span>
          <span className="field__row">
            <select
              className="field__control"
              value={def.symbol}
              data-testid="component-symbol"
              onChange={(event) => {
                setProps({ symbol: event.target.value });
              }}
            >
              {!symbols.includes(def.symbol) && (
                <option value={def.symbol}>
                  {def.symbol === '' ? t('libedit.component.none') : def.symbol}
                </option>
              )}
              {symbols.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </select>
            <button
              type="button"
              className="button button--small button--quiet"
              title={t('libedit.component.openSymbol')}
              aria-label={t('libedit.component.openSymbol')}
              disabled={!symbols.includes(def.symbol)}
              onClick={() => {
                onOpen('symbol', def.symbol);
              }}
            >
              <Icon name="reveal" size={14} />
            </button>
          </span>
        </label>
        <label className="field field--compact">
          <span className="field__label">{t('libedit.component.footprint')}</span>
          <span className="field__row">
            <FootprintField
              value={def.footprint ?? ''}
              footprints={footprints}
              testId="component-footprint"
              onSave={(value) => {
                setProps({ footprint: value === '' ? null : value });
              }}
            />
            <button
              type="button"
              className="button button--small button--quiet"
              title={t('libedit.component.openFootprint')}
              aria-label={t('libedit.component.openFootprint')}
              disabled={def.footprint === undefined || !footprints.includes(def.footprint)}
              onClick={() => {
                if (def.footprint !== undefined) {
                  onOpen('footprint', def.footprint);
                }
              }}
            >
              <Icon name="reveal" size={14} />
            </button>
          </span>
        </label>
        <div className="field-grid field-grid--two">
          {text('value', t('libedit.component.value'), 'component-value')}
          {text('manufacturer', t('libedit.component.manufacturer'), 'component-manufacturer')}
          {text('mpn', t('libedit.component.mpn'), 'component-mpn')}
          {text('datasheet', t('libedit.component.datasheet'))}
        </div>
        <TextInput
          label={t('libedit.props.description')}
          value={def.description ?? ''}
          multiline
          testId="prop-description"
          onSave={(description) => {
            setProps({ description });
          }}
        />
        {text('keywords', t('libedit.props.keywords'))}
        <Suppliers
          suppliers={def.suppliers ?? []}
          onSave={(suppliers) => {
            setProps({ suppliers });
          }}
        />
        <FieldsEditor
          values={def.properties ?? {}}
          testId="component-fields"
          onSave={(patch) => {
            setProps({ properties: patch });
          }}
        />
      </div>
      <aside className="component-editor__side">
        <h4 className="libedit-props__title">{t('libedit.component.preview')}</h4>
        <div className="component-editor__previews">
          {symbol !== null && (
            <SymbolPreview def={symbol.def} label={t('libedit.component.symbol')} />
          )}
          {footprint !== null && (
            <FootprintSvg
              def={footprint.def}
              label={t('libedit.component.footprint')}
              testId="component-footprint-preview"
            />
          )}
        </div>
        {findings.length > 0 && (
          <ul className="issues" data-testid="component-findings">
            {findings.map((finding, index) => (
              <li
                // eslint-disable-next-line @eslint-react/no-array-index-key -- stable order
                key={index}
                className={`issues__item issues__item--${finding.severity}`}
                data-testid={`finding-${finding.code}`}
              >
                <span className="issues__message">{findingText(t, finding)}</span>
              </li>
            ))}
          </ul>
        )}
        <h4 className="libedit-props__title">{t('libedit.component.pinMap')}</h4>
        <p className="libedit-props__muted">{t('libedit.component.pinMapHint')}</p>
        <div className="item-table__scroll">
          <table className="table" data-testid="component-pin-map">
            <thead>
              <tr>
                <th>{t('libedit.table.number')}</th>
                <th>{t('libedit.component.pin')}</th>
                <th>{t('libedit.component.pad')}</th>
              </tr>
            </thead>
            <tbody>
              {map.map((row) => (
                <tr key={row.number} data-ok={row.pin !== null && row.pad}>
                  <td>{row.number}</td>
                  <td>{row.pin ?? <em>{t('libedit.component.missing')}</em>}</td>
                  <td>{row.pad ? '✓' : <em>{t('libedit.component.missing')}</em>}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </aside>
    </div>
  );
}
