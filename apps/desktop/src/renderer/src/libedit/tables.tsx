/**
 * The pin table of the symbol editor and the pad table of the footprint
 * editor. Rows follow the drawing's selection both ways. A change in a
 * selected row applies to every selected row (bulk edit) for the fields
 * that make sense together (type, style, length, size …); numbers, names
 * and positions always change one row.
 */
import { useRef, useState, type MouseEvent, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import type { Units } from '@shared/project';

import { Icon } from '../components/Icon';
import { Modal } from '../components/Modal';
import { useAppState } from '../state';
import { LengthInput, NumberInput, SelectInput, TextInput } from './fields';
import { defaultPad } from './item-editor';
import { parsePinList, placePastedPins, selectedIndices } from './pinlist';
import { SHAPE_KEYS, shapeKey, shapeOf } from './shapes';
import {
  DIRECTIONS,
  PAD_KINDS,
  PIN_KINDS,
  PIN_SHAPES,
  type ItemKind,
  type ItemRef,
  type Pad,
  type Pin,
  type PinKind,
} from './types';

type Run = (edit: Record<string, unknown>) => Promise<boolean>;

/** Row selection with Ctrl/⌘ (toggle) and Shift (range). */
function useRows(
  kind: ItemKind,
  selection: readonly ItemRef[],
  onSelect: (refs: ItemRef[]) => void,
): {
  selected: Set<number>;
  click: (event: MouseEvent, index: number) => void;
  toggle: (index: number) => void;
  all: (count: number) => void;
} {
  const anchorRef = useRef<number | null>(null);
  const selected = new Set(selectedIndices(selection, kind));
  const refs = (indices: Iterable<number>): ItemRef[] =>
    [...indices].sort((a, b) => a - b).map((index) => ({ kind, index }));
  return {
    selected,
    click: (event, index) => {
      const target = event.target as HTMLElement;
      if (target.closest('input, select, textarea, button') !== null) {
        return;
      }
      if (event.shiftKey && anchorRef.current !== null) {
        const [a, b] = [Math.min(anchorRef.current, index), Math.max(anchorRef.current, index)];
        onSelect(refs(Array.from({ length: b - a + 1 }, (_, i) => a + i)));
        return;
      }
      anchorRef.current = index;
      if (event.ctrlKey || event.metaKey) {
        const next = new Set(selected);
        if (next.has(index)) {
          next.delete(index);
        } else {
          next.add(index);
        }
        onSelect(refs(next));
      } else {
        onSelect(refs([index]));
      }
    },
    toggle: (index) => {
      anchorRef.current = index;
      const next = new Set(selected);
      if (next.has(index)) {
        next.delete(index);
      } else {
        next.add(index);
      }
      onSelect(refs(next));
    },
    all: (count) => {
      onSelect(selected.size === count ? [] : refs(Array.from({ length: count }, (_, i) => i)));
    },
  };
}

/** The renumber control of the bulk bar. */
function Renumber({
  indices,
  op,
  run,
}: {
  readonly indices: readonly number[];
  readonly op: 'renumber';
  readonly run: Run;
}): ReactNode {
  const { t } = useAppState();
  const [start, setStart] = useState('1');
  const [step, setStep] = useState(1);
  return (
    <form
      className="item-table__renumber"
      title={t('libedit.table.renumberHint')}
      onSubmit={(event) => {
        event.preventDefault();
        if (start.trim() !== '') {
          void run({ op, indices, start: start.trim(), step });
        }
      }}
    >
      <label className="field--inline">
        <span>{t('libedit.table.renumberStart')}</span>
        <input
          className="field__control item-table__short"
          value={start}
          data-testid="renumber-start"
          onChange={(event) => {
            setStart(event.target.value);
          }}
        />
      </label>
      <NumberInput
        label={t('libedit.table.renumberStep')}
        bare
        ariaLabel={t('libedit.table.renumberStep')}
        value={step}
        integer
        onSave={setStep}
      />
      <button type="submit" className="button button--small" data-testid="renumber">
        {t('libedit.table.renumber')}
      </button>
    </form>
  );
}

/** The dialog that adds pins from a pasted list. */
function PasteDialog({
  existing,
  kind,
  length,
  run,
  onClose,
}: {
  readonly existing: readonly Pin[];
  readonly kind: PinKind;
  readonly length: number;
  readonly run: Run;
  readonly onClose: () => void;
}): ReactNode {
  const { t } = useAppState();
  const [text, setText] = useState('');
  const parsed = parsePinList(text, kind);
  const count = parsed.pins.length;
  return (
    <Modal
      title={t('libedit.paste.title')}
      onClose={onClose}
      testId="paste-pins-dialog"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('tree.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={count === 0}
            data-testid="paste-pins-add"
            onClick={() => {
              void run({
                op: 'addPins',
                pins: placePastedPins(parsed.pins, existing, length),
              }).then((ok) => {
                if (ok) {
                  onClose();
                }
              });
            }}
          >
            {t('libedit.paste.add', { count })}
          </button>
        </>
      }
    >
      <p className="dialog__text dialog__text--muted">{t('libedit.paste.hint')}</p>
      <label className="field">
        <span className="field__label">{t('libedit.paste.text')}</span>
        <textarea
          className="field__control item-table__paste"
          value={text}
          rows={10}
          spellCheck={false}
          placeholder={'1\tVCC\tW\n2\tGND\tW\n3\tSDA\tB'}
          data-testid="paste-pins-text"
          onChange={(event) => {
            setText(event.target.value);
          }}
        />
      </label>
      {count > 0 && <p className="dialog__text">{t('libedit.paste.preview', { count })}</p>}
      {parsed.unknown.map(({ line, kind: unknownKind }) => (
        <p key={line} className="callout callout--warning">
          {t('libedit.paste.unknownKind', { line, kind: unknownKind })}
        </p>
      ))}
    </Modal>
  );
}

/** The pins of the shown body. */
export function PinTable({
  pins,
  units,
  selection,
  defaultKind,
  defaultLength,
  onSelect,
  run,
}: {
  readonly pins: readonly Pin[];
  readonly units: Units;
  readonly selection: readonly ItemRef[];
  readonly defaultKind: PinKind;
  readonly defaultLength: number;
  readonly onSelect: (refs: ItemRef[]) => void;
  readonly run: Run;
}): ReactNode {
  const { t } = useAppState();
  const rows = useRows('pin', selection, onSelect);
  const [pasting, setPasting] = useState(false);
  const selected = [...rows.selected].sort((a, b) => a - b);
  const patch = (index: number, fields: Partial<Pin>, bulk: boolean): void => {
    const indices = bulk && rows.selected.has(index) ? selected : [index];
    void run({ op: 'setPins', indices, patch: fields });
  };
  const kinds = PIN_KINDS.map(
    (kind) => [kind, t(`schematic.erc.pinKinds.${kind}` as MessageKey)] as const,
  );
  const shapes = PIN_SHAPES.map(
    (shape) => [shape, t(`libedit.pinShapes.${shape}` as MessageKey)] as const,
  );
  const directions = DIRECTIONS.map(
    (direction) => [direction, t(`libedit.directions.${direction}` as MessageKey)] as const,
  );
  return (
    <div className="item-table" data-testid="pin-table">
      <div className="item-table__bar">
        <span className="item-table__count">
          {t('libedit.table.selected', { count: selected.length })}
        </span>
        {selected.length > 0 && <Renumber indices={selected} op="renumber" run={run} />}
        {selected.length > 0 && (
          <button
            type="button"
            className="button button--small button--quiet"
            data-testid="pin-table-delete"
            onClick={() => {
              void run({ op: 'delete', items: selected.map((index) => ({ kind: 'pin', index })) });
            }}
          >
            <Icon name="trash" size={14} />
            {t('libedit.actions.delete')}
          </button>
        )}
        <span className="item-table__spacer" />
        <button
          type="button"
          className="button button--small"
          data-testid="paste-pins"
          onClick={() => {
            setPasting(true);
          }}
        >
          <Icon name="paste" size={14} />
          {t('libedit.table.paste')}
        </button>
      </div>
      {pins.length === 0 ? (
        <p className="item-table__empty">{t('libedit.table.emptyPins')}</p>
      ) : (
        <div className="item-table__scroll">
          <table className="table item-table__table">
            <thead>
              <tr>
                <th>
                  <input
                    type="checkbox"
                    aria-label={t('libedit.table.selectAll')}
                    checked={selected.length === pins.length}
                    onChange={() => {
                      rows.all(pins.length);
                    }}
                  />
                </th>
                <th>{t('libedit.table.number')}</th>
                <th>{t('libedit.table.name')}</th>
                <th>{t('libedit.table.kind')}</th>
                <th>{t('libedit.table.shape')}</th>
                <th>{t('libedit.table.direction')}</th>
                <th>{t('libedit.table.length')}</th>
                <th>{t('libedit.table.x')}</th>
                <th>{t('libedit.table.y')}</th>
                <th>{t('libedit.table.hidden')}</th>
              </tr>
            </thead>
            <tbody>
              {pins.map((pin, index) => (
                <tr
                  // Rows are addressed by index, like the engine's edits.
                  // eslint-disable-next-line @eslint-react/no-array-index-key -- see above
                  key={index}
                  data-selected={rows.selected.has(index)}
                  data-testid={`pin-row-${String(index)}`}
                  onClick={(event) => {
                    rows.click(event, index);
                  }}
                >
                  <td>
                    <input
                      type="checkbox"
                      aria-label={t('libedit.table.selectRow', { name: pin.number })}
                      checked={rows.selected.has(index)}
                      onChange={() => {
                        rows.toggle(index);
                      }}
                    />
                  </td>
                  <td>
                    <TextInput
                      bare
                      label={t('libedit.table.number')}
                      value={pin.number}
                      testId={`pin-number-${String(index)}`}
                      onSave={(number) => {
                        patch(index, { number }, false);
                      }}
                    />
                  </td>
                  <td>
                    <TextInput
                      bare
                      label={t('libedit.table.name')}
                      value={pin.name ?? ''}
                      testId={`pin-name-${String(index)}`}
                      onSave={(name) => {
                        patch(index, { name }, false);
                      }}
                    />
                  </td>
                  <td>
                    <SelectInput
                      bare
                      label={t('libedit.table.kind')}
                      value={pin.kind ?? 'passive'}
                      options={kinds}
                      testId={`pin-kind-${String(index)}`}
                      onSave={(kind) => {
                        patch(index, { kind }, true);
                      }}
                    />
                  </td>
                  <td>
                    <SelectInput
                      bare
                      label={t('libedit.table.shape')}
                      value={pin.shape ?? 'line'}
                      options={shapes}
                      onSave={(shape) => {
                        patch(index, { shape }, true);
                      }}
                    />
                  </td>
                  <td>
                    <SelectInput
                      bare
                      label={t('libedit.table.direction')}
                      value={pin.direction ?? 'right'}
                      options={directions}
                      onSave={(direction) => {
                        patch(index, { direction }, true);
                      }}
                    />
                  </td>
                  <td>
                    <LengthInput
                      bare
                      label={t('libedit.table.length')}
                      value={pin.length}
                      units={units}
                      min={0}
                      testId={`pin-length-${String(index)}`}
                      onSave={(length) => {
                        patch(index, { length }, true);
                      }}
                    />
                  </td>
                  <td>
                    <LengthInput
                      bare
                      label={t('libedit.table.x')}
                      value={pin.at[0]}
                      units={units}
                      onSave={(x) => {
                        patch(index, { at: [x, pin.at[1]] }, false);
                      }}
                    />
                  </td>
                  <td>
                    <LengthInput
                      bare
                      label={t('libedit.table.y')}
                      value={pin.at[1]}
                      units={units}
                      onSave={(y) => {
                        patch(index, { at: [pin.at[0], y] }, false);
                      }}
                    />
                  </td>
                  <td>
                    <input
                      type="checkbox"
                      aria-label={t('libedit.table.hidden')}
                      checked={pin.hidden === true}
                      onChange={(event) => {
                        patch(index, { hidden: event.target.checked }, true);
                      }}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {pasting && (
        <PasteDialog
          existing={pins}
          kind={defaultKind}
          length={defaultLength}
          run={run}
          onClose={() => {
            setPasting(false);
          }}
        />
      )}
    </div>
  );
}

/** The dialog that adds a grid of pads. */
function ArrayDialog({
  template,
  units,
  run,
  onClose,
}: {
  readonly template: Pad;
  readonly units: Units;
  readonly run: Run;
  readonly onClose: () => void;
}): ReactNode {
  const { t } = useAppState();
  const [columns, setColumns] = useState(1);
  const [rows, setRows] = useState(8);
  const [pitchX, setPitchX] = useState(2_540_000);
  const [pitchY, setPitchY] = useState(2_540_000);
  const [start, setStart] = useState('1');
  const [at, setAt] = useState<[number, number]>([template.at[0], template.at[1]]);
  const count = columns * rows;
  return (
    <Modal
      title={t('libedit.array.title')}
      onClose={onClose}
      testId="pad-array-dialog"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('tree.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={count === 0 || start.trim() === ''}
            data-testid="pad-array-add"
            onClick={() => {
              void run({
                op: 'addPadArray',
                pad: { ...template, at },
                columns,
                rows,
                pitchX,
                pitchY,
                start: start.trim(),
              }).then((ok) => {
                if (ok) {
                  onClose();
                }
              });
            }}
          >
            {t('libedit.array.add', { count })}
          </button>
        </>
      }
    >
      <p className="dialog__text dialog__text--muted">{t('libedit.array.hint')}</p>
      <div className="field-grid">
        <NumberInput
          label={t('libedit.array.columns')}
          value={columns}
          integer
          min={1}
          max={100}
          testId="pad-array-columns"
          onSave={setColumns}
        />
        <NumberInput
          label={t('libedit.array.rows')}
          value={rows}
          integer
          min={1}
          max={100}
          testId="pad-array-rows"
          onSave={setRows}
        />
        <LengthInput
          label={t('libedit.array.pitchX')}
          value={pitchX}
          units={units}
          onSave={setPitchX}
        />
        <LengthInput
          label={t('libedit.array.pitchY')}
          value={pitchY}
          units={units}
          onSave={setPitchY}
        />
        <TextInput label={t('libedit.array.start')} value={start} onSave={setStart} />
        <span className="field field--compact">
          <span className="field__label">{t('libedit.array.at')}</span>
          <span className="field__row">
            <LengthInput
              bare
              ariaLabel="X"
              value={at[0]}
              units={units}
              onSave={(x) => {
                setAt([x, at[1]]);
              }}
            />
            <LengthInput
              bare
              ariaLabel="Y"
              value={at[1]}
              units={units}
              onSave={(y) => {
                setAt([at[0], y]);
              }}
            />
          </span>
        </span>
      </div>
    </Modal>
  );
}

/** Short layer list of a pad (`F.Cu`-like names are KiCad's; ours are words). */
function layerSummary(layers: readonly string[]): string {
  return layers.map((layer) => layer.replace('.top', '↑').replace('.bottom', '↓')).join(' ');
}

/** The pads of the footprint. */
export function PadTable({
  pads,
  units,
  selection,
  onSelect,
  run,
}: {
  readonly pads: readonly Pad[];
  readonly units: Units;
  readonly selection: readonly ItemRef[];
  readonly onSelect: (refs: ItemRef[]) => void;
  readonly run: Run;
}): ReactNode {
  const { t } = useAppState();
  const rows = useRows('pad', selection, onSelect);
  const [array, setArray] = useState(false);
  const selected = [...rows.selected].sort((a, b) => a - b);
  const patch = (index: number, fields: Record<string, unknown>, bulk: boolean): void => {
    const indices = bulk && rows.selected.has(index) ? selected : [index];
    void run({ op: 'setPads', indices, patch: fields });
  };
  const kinds = PAD_KINDS.map(
    (kind) => [kind, t(`libedit.padKinds.${kind}` as MessageKey)] as const,
  );
  const shapes = SHAPE_KEYS.map(
    (key) => [key, t(`libedit.padShapes.${key}` as MessageKey)] as const,
  );
  const template = pads[selected[0] ?? -1] ?? pads.at(-1) ?? defaultPad();
  return (
    <div className="item-table" data-testid="pad-table">
      <div className="item-table__bar">
        <span className="item-table__count">
          {t('libedit.table.selected', { count: selected.length })}
        </span>
        {selected.length > 0 && <Renumber indices={selected} op="renumber" run={run} />}
        {selected.length > 0 && (
          <button
            type="button"
            className="button button--small button--quiet"
            data-testid="pad-table-delete"
            onClick={() => {
              void run({ op: 'delete', items: selected.map((index) => ({ kind: 'pad', index })) });
            }}
          >
            <Icon name="trash" size={14} />
            {t('libedit.actions.delete')}
          </button>
        )}
        <span className="item-table__spacer" />
        <button
          type="button"
          className="button button--small"
          data-testid="pad-array"
          onClick={() => {
            setArray(true);
          }}
        >
          <Icon name="grid" size={14} />
          {t('libedit.table.array')}
        </button>
      </div>
      {pads.length === 0 ? (
        <p className="item-table__empty">{t('libedit.table.emptyPads')}</p>
      ) : (
        <div className="item-table__scroll">
          <table className="table item-table__table">
            <thead>
              <tr>
                <th>
                  <input
                    type="checkbox"
                    aria-label={t('libedit.table.selectAll')}
                    checked={selected.length === pads.length}
                    onChange={() => {
                      rows.all(pads.length);
                    }}
                  />
                </th>
                <th>{t('libedit.table.number')}</th>
                <th>{t('libedit.table.kind')}</th>
                <th>{t('libedit.table.shape')}</th>
                <th>{t('libedit.table.width')}</th>
                <th>{t('libedit.table.height')}</th>
                <th>{t('libedit.table.drill')}</th>
                <th>{t('libedit.table.rotation')}</th>
                <th>{t('libedit.table.x')}</th>
                <th>{t('libedit.table.y')}</th>
                <th>{t('libedit.table.layers')}</th>
              </tr>
            </thead>
            <tbody>
              {pads.map((pad, index) => {
                const kind = pad.kind ?? 'smd';
                return (
                  <tr
                    key={pad.id}
                    data-selected={rows.selected.has(index)}
                    data-testid={`pad-row-${String(index)}`}
                    onClick={(event) => {
                      rows.click(event, index);
                    }}
                  >
                    <td>
                      <input
                        type="checkbox"
                        aria-label={t('libedit.table.selectRow', { name: pad.number })}
                        checked={rows.selected.has(index)}
                        onChange={() => {
                          rows.toggle(index);
                        }}
                      />
                    </td>
                    <td>
                      <TextInput
                        bare
                        label={t('libedit.table.number')}
                        value={pad.number}
                        testId={`pad-number-${String(index)}`}
                        onSave={(number) => {
                          patch(index, { number }, false);
                        }}
                      />
                    </td>
                    <td>
                      <SelectInput
                        bare
                        label={t('libedit.table.kind')}
                        value={kind}
                        options={kinds}
                        testId={`pad-kind-${String(index)}`}
                        onSave={(next) => {
                          patch(index, { kind: next }, true);
                        }}
                      />
                    </td>
                    <td>
                      <SelectInput
                        bare
                        label={t('libedit.table.shape')}
                        value={shapeKey(pad.shape)}
                        options={shapes}
                        testId={`pad-shape-${String(index)}`}
                        onSave={(next) => {
                          patch(index, { shape: shapeOf(next, pad.size) }, true);
                        }}
                      />
                    </td>
                    <td>
                      <LengthInput
                        bare
                        label={t('libedit.table.width')}
                        value={pad.size[0]}
                        units={units}
                        min={1}
                        testId={`pad-width-${String(index)}`}
                        onSave={(w) => {
                          patch(index, { size: [w, pad.size[1]] }, true);
                        }}
                      />
                    </td>
                    <td>
                      <LengthInput
                        bare
                        label={t('libedit.table.height')}
                        value={pad.size[1]}
                        units={units}
                        min={1}
                        onSave={(h) => {
                          patch(index, { size: [pad.size[0], h] }, true);
                        }}
                      />
                    </td>
                    <td>
                      {pad.drill === undefined ? (
                        <span className="item-table__none">–</span>
                      ) : (
                        <LengthInput
                          bare
                          label={t('libedit.table.drill')}
                          value={pad.drill.diameter}
                          units={units}
                          min={1}
                          onSave={(diameter) => {
                            patch(index, { drill: { ...pad.drill, diameter } }, true);
                          }}
                        />
                      )}
                    </td>
                    <td>
                      <NumberInput
                        bare
                        label={t('libedit.table.rotation')}
                        value={pad.rotation ?? 0}
                        suffix="°"
                        onSave={(rotation) => {
                          patch(index, { rotation }, true);
                        }}
                      />
                    </td>
                    <td>
                      <LengthInput
                        bare
                        label={t('libedit.table.x')}
                        value={pad.at[0]}
                        units={units}
                        onSave={(x) => {
                          patch(index, { at: [x, pad.at[1]] }, false);
                        }}
                      />
                    </td>
                    <td>
                      <LengthInput
                        bare
                        label={t('libedit.table.y')}
                        value={pad.at[1]}
                        units={units}
                        onSave={(y) => {
                          patch(index, { at: [pad.at[0], y] }, false);
                        }}
                      />
                    </td>
                    <td className="item-table__layers">{layerSummary(pad.layers)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
      {array && (
        <ArrayDialog
          template={template}
          units={units}
          run={run}
          onClose={() => {
            setArray(false);
          }}
        />
      )}
    </div>
  );
}
