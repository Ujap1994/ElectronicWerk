/**
 * The side panel of the symbol and footprint editors: the item's own
 * properties (`libedit.setProps`), the selected pin, pad, graphic or text
 * (`update*` edits) and, for footprints, the courtyard and the 3D models.
 */
import { useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import type { Units } from '@shared/project';

import { Icon } from '../components/Icon';
import { useAppState } from '../state';
import { layerLabel as layerName } from '../viewer/labels';
import {
  CheckInput,
  LengthInput,
  LengthTuple,
  NumberInput,
  SelectInput,
  TextInput,
} from './fields';
import { SHAPE_KEYS, shapeKey, shapeOf } from './shapes';
import {
  DIRECTIONS,
  PAD_KINDS,
  PIN_KINDS,
  PIN_SHAPES,
  bodyOf,
  type Body,
  type BoardGraphic,
  type FootprintDef,
  type FootprintText,
  type Geometry,
  type ItemRef,
  type Point,
  type SymbolDef,
  type SymbolGraphic,
  type SymbolText,
  type Usage,
} from './types';

type Run = (edit: Record<string, unknown>) => Promise<boolean>;
type SetProps = (props: Record<string, unknown>) => void;

/** Edits the custom fields (name → value) of an item. */
export function FieldsEditor({
  values,
  testId,
  onSave,
}: {
  readonly values: Readonly<Record<string, string>>;
  readonly testId: string;
  readonly onSave: (patch: Record<string, string | null>) => void;
}): ReactNode {
  const { t } = useAppState();
  const [name, setName] = useState('');
  return (
    <fieldset className="field--group libedit-fields" data-testid={testId}>
      <legend>{t('libedit.props.fields')}</legend>
      {Object.entries(values).map(([key, value]) => (
        <div key={key} className="inspector-edit__parameter">
          <TextInput
            label={key}
            value={value}
            onSave={(next) => {
              onSave({ [key]: next });
            }}
          />
          <button
            type="button"
            className="button button--quiet button--small"
            aria-label={t('libedit.props.removeField', { name: key })}
            title={t('libedit.props.removeField', { name: key })}
            onClick={() => {
              onSave({ [key]: null });
            }}
          >
            <Icon name="trash" size={14} />
          </button>
        </div>
      ))}
      <form
        className="inspector-edit__add"
        onSubmit={(event) => {
          event.preventDefault();
          const key = name.trim();
          if (key !== '' && values[key] === undefined) {
            onSave({ [key]: '' });
            setName('');
          }
        }}
      >
        <input
          className="field__control"
          value={name}
          placeholder={t('libedit.props.fieldName')}
          aria-label={t('libedit.props.fieldName')}
          data-testid={`${testId}-new`}
          onChange={(event) => {
            setName(event.target.value);
          }}
        />
        <button type="submit" className="button button--small" disabled={name.trim() === ''}>
          {t('libedit.props.addField')}
        </button>
      </form>
    </fieldset>
  );
}

function UsageLine({ usage }: { readonly usage: Usage }): ReactNode {
  const { t } = useAppState();
  const used = usage.placed + usage.board + usage.symbols + usage.components > 0;
  return (
    <p className="libedit-props__usage" data-testid="item-usage">
      {used ? t('libedit.props.usage', { ...usage }) : t('libedit.props.unused')}
    </p>
  );
}

/** The properties of a symbol. */
export function SymbolProps({
  def,
  usage,
  units,
  footprints,
  setProps,
  onRename,
}: {
  readonly def: SymbolDef;
  readonly usage: Usage;
  readonly units: Units;
  readonly footprints: readonly string[];
  readonly setProps: SetProps;
  readonly onRename: () => void;
}): ReactNode {
  const { t } = useAppState();
  return (
    <section className="libedit-props" aria-label={t('libedit.props.title')}>
      <h3 className="libedit-props__title">{t('libedit.props.title')}</h3>
      <div className="libedit-props__name">
        <span className="libedit-props__value" data-testid="item-name">
          {def.name}
        </span>
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
      <UsageLine usage={usage} />
      <TextInput
        label={t('libedit.props.description')}
        value={def.description ?? ''}
        multiline
        testId="prop-description"
        onSave={(description) => {
          setProps({ description });
        }}
      />
      <TextInput
        label={t('libedit.props.keywords')}
        value={def.keywords ?? ''}
        onSave={(keywords) => {
          setProps({ keywords });
        }}
      />
      <div className="field-grid field-grid--two">
        <TextInput
          label={t('libedit.props.prefix')}
          value={def.prefix}
          testId="prop-prefix"
          onSave={(prefix) => {
            setProps({ prefix });
          }}
        />
        <TextInput
          label={t('libedit.props.value')}
          value={def.value ?? ''}
          onSave={(value) => {
            setProps({ value });
          }}
        />
      </div>
      <label className="field field--compact">
        <span className="field__label">{t('libedit.props.footprint')}</span>
        <FootprintField
          value={def.footprint ?? ''}
          footprints={footprints}
          testId="prop-footprint"
          onSave={(footprint) => {
            setProps({ footprint });
          }}
        />
      </label>
      <TextInput
        label={t('libedit.props.datasheet')}
        value={def.datasheet ?? ''}
        onSave={(datasheet) => {
          setProps({ datasheet });
        }}
      />
      <CheckInput
        label={t('libedit.props.power')}
        checked={def.power === true}
        testId="prop-power"
        onSave={(power) => {
          setProps({ power });
        }}
      />
      <CheckInput
        label={t('libedit.props.hideNumbers')}
        checked={def.pinText?.hideNumbers === true}
        onSave={(hideNumbers) => {
          setProps({ pinText: { hideNumbers } });
        }}
      />
      <CheckInput
        label={t('libedit.props.hideNames')}
        checked={def.pinText?.hideNames === true}
        onSave={(hideNames) => {
          setProps({ pinText: { hideNames } });
        }}
      />
      <LengthInput
        label={t('libedit.props.nameOffset')}
        value={def.pinText?.nameOffset ?? 508_000}
        units={units}
        min={0}
        onSave={(nameOffset) => {
          setProps({ pinText: { nameOffset } });
        }}
      />
      <FieldsEditor
        values={def.properties ?? {}}
        testId="symbol-fields"
        onSave={(patch) => {
          setProps({ properties: patch });
        }}
      />
    </section>
  );
}

/** A footprint name with suggestions from the project's libraries. */
export function FootprintField({
  value,
  footprints,
  testId,
  onSave,
}: {
  readonly value: string;
  readonly footprints: readonly string[];
  readonly testId: string;
  readonly onSave: (value: string) => void;
}): ReactNode {
  const [draft, setDraft] = useState(value);
  const [stored, setStored] = useState(value);
  if (stored !== value) {
    setStored(value);
    setDraft(value);
  }
  const listId = `${testId}-list`;
  const save = (): void => {
    if (draft.trim() !== value) {
      onSave(draft.trim());
    }
  };
  return (
    <>
      <input
        className="field__control"
        value={draft}
        list={listId}
        spellCheck={false}
        data-testid={testId}
        onChange={(event) => {
          setDraft(event.target.value);
        }}
        onBlur={save}
        onKeyDown={(event) => {
          if (event.key === 'Enter') {
            save();
          } else if (event.key === 'Escape') {
            setDraft(value);
          }
        }}
      />
      <datalist id={listId}>
        {footprints.map((name) => (
          <option key={name} value={name} />
        ))}
      </datalist>
    </>
  );
}

/** The properties of a footprint, its courtyard and its 3D models. */
export function FootprintProps({
  def,
  usage,
  units,
  setProps,
  run,
  onRename,
  modelPreview,
}: {
  readonly def: FootprintDef;
  readonly usage: Usage;
  readonly units: Units;
  readonly setProps: SetProps;
  readonly run: Run;
  readonly onRename: () => void;
  /** The 3D preview of the selected model. */
  readonly modelPreview: (index: number) => ReactNode;
}): ReactNode {
  const { t } = useAppState();
  const [margin, setMargin] = useState(250_000);
  const attributes = def.attributes ?? {};
  const mountings = (['smd', 'throughHole', 'other'] as const).map(
    (key) => [key, t(`libedit.props.mountings.${key}`)] as const,
  );
  return (
    <section className="libedit-props" aria-label={t('libedit.props.title')}>
      <h3 className="libedit-props__title">{t('libedit.props.title')}</h3>
      <div className="libedit-props__name">
        <span className="libedit-props__value" data-testid="item-name">
          {def.name}
        </span>
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
      <UsageLine usage={usage} />
      <TextInput
        label={t('libedit.props.description')}
        value={def.description ?? ''}
        multiline
        testId="prop-description"
        onSave={(description) => {
          setProps({ description });
        }}
      />
      <TextInput
        label={t('libedit.props.keywords')}
        value={def.keywords ?? ''}
        onSave={(keywords) => {
          setProps({ keywords });
        }}
      />
      <SelectInput
        label={t('libedit.props.mounting')}
        value={attributes.mounting ?? 'smd'}
        options={mountings}
        onSave={(mounting) => {
          setProps({ attributes: { mounting } });
        }}
      />
      <CheckInput
        label={t('libedit.props.excludeFromBom')}
        checked={attributes.excludeFromBom === true}
        onSave={(excludeFromBom) => {
          setProps({ attributes: { excludeFromBom } });
        }}
      />
      <CheckInput
        label={t('libedit.props.excludeFromPlacement')}
        checked={attributes.excludeFromPlacement === true}
        onSave={(excludeFromPlacement) => {
          setProps({ attributes: { excludeFromPlacement } });
        }}
      />
      <CheckInput
        label={t('libedit.props.dnp')}
        checked={attributes.dnp === true}
        onSave={(dnp) => {
          setProps({ attributes: { dnp } });
        }}
      />
      <fieldset className="field--group">
        <legend>{t('libedit.props.courtyard')}</legend>
        <div className="libedit-props__row">
          <LengthInput
            label={t('libedit.props.courtyardMargin')}
            value={margin}
            units={units}
            min={0}
            onSave={setMargin}
          />
          <button
            type="button"
            className="button button--small"
            data-testid="make-courtyard"
            onClick={() => {
              void run({ op: 'courtyard', margin });
            }}
          >
            {t('libedit.props.makeCourtyard')}
          </button>
        </div>
      </fieldset>
      <Models def={def} units={units} run={run} preview={modelPreview} />
    </section>
  );
}

/** The 3D models of a footprint: path and placement. */
function Models({
  def,
  units,
  run,
  preview,
}: {
  readonly def: FootprintDef;
  readonly units: Units;
  readonly run: Run;
  readonly preview: (index: number) => ReactNode;
}): ReactNode {
  const { t } = useAppState();
  const models = def.models ?? [];
  const [shown, setShown] = useState(0);
  const update = (index: number, patch: Record<string, unknown>): void => {
    const model = models[index];
    if (model !== undefined) {
      void run({ op: 'updateModel', index, model: { ...model, ...patch } });
    }
  };
  return (
    <fieldset className="field--group libedit-models" data-testid="item-models">
      <legend>{t('libedit.models.title')}</legend>
      {models.length === 0 && <p className="libedit-props__muted">{t('libedit.models.none')}</p>}
      {models.map((model, index) => (
        <div
          // Models are addressed by index, like the engine's edits.
          // eslint-disable-next-line @eslint-react/no-array-index-key -- see above
          key={index}
          className="libedit-model"
          data-testid={`model-${String(index)}`}
        >
          <TextInput
            label={t('libedit.models.path')}
            value={model.path}
            testId={`model-path-${String(index)}`}
            onSave={(path) => {
              update(index, { path });
            }}
          />
          <LengthTuple
            label={t('libedit.models.offset')}
            values={model.offset ?? [0, 0, 0]}
            units={units}
            testId={`model-offset-${String(index)}`}
            onSave={(offset) => {
              update(index, { offset });
            }}
          />
          <fieldset className="field field--compact field--tuple">
            <legend className="field__label">{t('libedit.models.rotation')}</legend>
            <span className="field__row">
              {(model.rotation ?? [0, 0, 0]).map((value, axis) => (
                <NumberInput
                  // eslint-disable-next-line @eslint-react/no-array-index-key -- fixed axes
                  key={axis}
                  bare
                  ariaLabel={`${t('libedit.models.rotation')} ${'XYZ'[axis] ?? ''}`}
                  value={value}
                  suffix="°"
                  testId={`model-rotation-${String(index)}-${String(axis)}`}
                  onSave={(next) => {
                    const rotation = [...(model.rotation ?? [0, 0, 0])];
                    rotation[axis] = next;
                    update(index, { rotation });
                  }}
                />
              ))}
            </span>
          </fieldset>
          <fieldset className="field field--compact field--tuple">
            <legend className="field__label">{t('libedit.models.scale')}</legend>
            <span className="field__row">
              {(model.scale ?? [1, 1, 1]).map((value, axis) => (
                <NumberInput
                  // eslint-disable-next-line @eslint-react/no-array-index-key -- fixed axes
                  key={axis}
                  bare
                  ariaLabel={`${t('libedit.models.scale')} ${'XYZ'[axis] ?? ''}`}
                  value={value}
                  min={0.0001}
                  onSave={(next) => {
                    const scale = [...(model.scale ?? [1, 1, 1])];
                    scale[axis] = next;
                    update(index, { scale });
                  }}
                />
              ))}
            </span>
          </fieldset>
          <div className="libedit-props__row">
            <CheckInput
              label={t('libedit.models.hidden')}
              checked={model.hidden === true}
              onSave={(hidden) => {
                update(index, { hidden });
              }}
            />
            <button
              type="button"
              className="button button--small"
              aria-pressed={shown === index}
              data-testid={`model-preview-${String(index)}`}
              onClick={() => {
                setShown(index);
              }}
            >
              <Icon name="cube" size={14} />
              {t('libedit.models.preview')}
            </button>
            <button
              type="button"
              className="button button--small button--quiet"
              aria-label={t('libedit.models.remove')}
              title={t('libedit.models.remove')}
              onClick={() => {
                void run({ op: 'delete', items: [{ kind: 'model', index }] });
              }}
            >
              <Icon name="trash" size={14} />
            </button>
          </div>
        </div>
      ))}
      {models.length > 0 && preview(Math.min(shown, models.length - 1))}
      <AddModel run={run} />
    </fieldset>
  );
}

function AddModel({ run }: { readonly run: Run }): ReactNode {
  const { t } = useAppState();
  const [path, setPath] = useState('');
  return (
    <form
      className="inspector-edit__add"
      onSubmit={(event) => {
        event.preventDefault();
        if (path.trim() !== '') {
          void run({ op: 'addModel', model: { path: path.trim() } }).then((ok) => {
            if (ok) {
              setPath('');
            }
          });
        }
      }}
    >
      <input
        className="field__control"
        value={path}
        placeholder={t('libedit.models.path')}
        title={t('libedit.models.pathHint')}
        aria-label={t('libedit.models.path')}
        spellCheck={false}
        data-testid="model-add-path"
        onChange={(event) => {
          setPath(event.target.value);
        }}
      />
      <button
        type="submit"
        className="button button--small"
        disabled={path.trim() === ''}
        data-testid="model-add"
      >
        {t('libedit.models.add')}
      </button>
    </form>
  );
}

// --- The selected item ---------------------------------------------------------

/** Editable points of a geometry, with names. */
function geometryPoints(geometry: Geometry): { label: MessageKey; point: Point }[] {
  if ('line' in geometry) {
    return [
      { label: 'libedit.props.start', point: geometry.line.start },
      { label: 'libedit.props.end', point: geometry.line.end },
    ];
  }
  if ('rect' in geometry) {
    return [
      { label: 'libedit.props.start', point: geometry.rect.start },
      { label: 'libedit.props.end', point: geometry.rect.end },
    ];
  }
  if ('circle' in geometry) {
    return [{ label: 'libedit.props.center', point: geometry.circle.center }];
  }
  if ('arc' in geometry) {
    return [
      { label: 'libedit.props.start', point: geometry.arc.start },
      { label: 'libedit.props.end', point: geometry.arc.end },
    ];
  }
  return [];
}

/** A geometry with one of its named points moved. */
function movePoint(geometry: Geometry, which: number, point: Point): Geometry {
  if ('line' in geometry) {
    return {
      line: which === 0 ? { ...geometry.line, start: point } : { ...geometry.line, end: point },
    };
  }
  if ('rect' in geometry) {
    return {
      rect: which === 0 ? { ...geometry.rect, start: point } : { ...geometry.rect, end: point },
    };
  }
  if ('circle' in geometry) {
    return { circle: { ...geometry.circle, center: point } };
  }
  if ('arc' in geometry) {
    // Keep the arc's bulge: move the middle point along.
    const { start, mid, end } = geometry.arc;
    const old = which === 0 ? start : end;
    const shift: Point = [(point[0] - old[0]) / 2, (point[1] - old[1]) / 2];
    const middle: Point = [Math.round(mid[0] + shift[0]), Math.round(mid[1] + shift[1])];
    return {
      arc: which === 0 ? { start: point, mid: middle, end } : { start, mid: middle, end: point },
    };
  }
  return geometry;
}

function GeometryFields({
  geometry,
  units,
  onSave,
}: {
  readonly geometry: Geometry;
  readonly units: Units;
  readonly onSave: (geometry: Geometry) => void;
}): ReactNode {
  const { t } = useAppState();
  return (
    <>
      {geometryPoints(geometry).map(({ label, point }, which) => (
        <LengthTuple
          key={label}
          label={t(label)}
          values={point}
          units={units}
          onSave={([x = 0, y = 0]) => {
            onSave(movePoint(geometry, which, [x, y]));
          }}
        />
      ))}
      {'circle' in geometry && (
        <LengthInput
          label={t('libedit.props.radius')}
          value={geometry.circle.radius}
          units={units}
          min={1}
          onSave={(radius) => {
            onSave({ circle: { ...geometry.circle, radius } });
          }}
        />
      )}
    </>
  );
}

/** The selected item of a symbol body. */
export function SymbolSelection({
  def,
  body,
  selection,
  units,
  run,
}: {
  readonly def: SymbolDef;
  readonly body: Body;
  readonly selection: readonly ItemRef[];
  readonly units: Units;
  readonly run: Run;
}): ReactNode {
  const { t } = useAppState();
  const content = bodyOf(def, body);
  const [ref] = selection;
  if (selection.length > 1) {
    return <SelectionNote text={t('libedit.props.multiple', { count: selection.length })} />;
  }
  if (ref === undefined) {
    return <SelectionNote text={t('libedit.props.nothing')} />;
  }
  if (ref.kind === 'pin') {
    const pin = content.pins?.[ref.index];
    if (pin === undefined) {
      return null;
    }
    const save = (patch: Record<string, unknown>): void => {
      void run({ op: 'setPins', indices: [ref.index], patch });
    };
    return (
      <section className="libedit-props" data-testid="selection-props">
        <h3 className="libedit-props__title">{t('libedit.props.pin', { number: pin.number })}</h3>
        <div className="field-grid field-grid--two">
          <TextInput
            label={t('libedit.table.number')}
            value={pin.number}
            testId="sel-pin-number"
            onSave={(number) => {
              save({ number });
            }}
          />
          <TextInput
            label={t('libedit.table.name')}
            value={pin.name ?? ''}
            testId="sel-pin-name"
            onSave={(name) => {
              save({ name });
            }}
          />
        </div>
        <SelectInput
          label={t('libedit.table.kind')}
          value={pin.kind ?? 'passive'}
          options={PIN_KINDS.map(
            (kind) => [kind, t(`schematic.erc.pinKinds.${kind}` as MessageKey)] as const,
          )}
          onSave={(kind) => {
            save({ kind });
          }}
        />
        <SelectInput
          label={t('libedit.table.shape')}
          value={pin.shape ?? 'line'}
          options={PIN_SHAPES.map(
            (shape) => [shape, t(`libedit.pinShapes.${shape}` as MessageKey)] as const,
          )}
          onSave={(shape) => {
            save({ shape });
          }}
        />
        <SelectInput
          label={t('libedit.table.direction')}
          value={pin.direction ?? 'right'}
          options={DIRECTIONS.map(
            (direction) => [direction, t(`libedit.directions.${direction}` as MessageKey)] as const,
          )}
          onSave={(direction) => {
            save({ direction });
          }}
        />
        <LengthInput
          label={t('libedit.table.length')}
          value={pin.length}
          units={units}
          min={0}
          onSave={(length) => {
            save({ length });
          }}
        />
        <LengthTuple
          label={t('libedit.props.position')}
          values={pin.at}
          units={units}
          onSave={([x = 0, y = 0]) => {
            save({ at: [x, y] });
          }}
        />
        <CheckInput
          label={t('libedit.table.hidden')}
          checked={pin.hidden === true}
          onSave={(hidden) => {
            save({ hidden });
          }}
        />
      </section>
    );
  }
  if (ref.kind === 'graphic') {
    const graphic = content.graphics?.[ref.index];
    if (graphic === undefined) {
      return null;
    }
    const save = (next: SymbolGraphic): void => {
      void run({ op: 'updateGraphic', index: ref.index, graphic: next });
    };
    const fills = (['none', 'solid', 'background'] as const).map(
      (fill) => [fill, t(`libedit.props.fills.${fill}`)] as const,
    );
    return (
      <section className="libedit-props" data-testid="selection-props">
        <h3 className="libedit-props__title">{t('libedit.props.graphic')}</h3>
        <GeometryFields
          geometry={graphic.geometry}
          units={units}
          onSave={(geometry) => {
            save({ ...graphic, geometry });
          }}
        />
        <LengthInput
          label={t('libedit.props.lineWidth')}
          value={graphic.stroke?.width ?? 0}
          units={units}
          min={0}
          onSave={(width) => {
            save({ ...graphic, stroke: { ...graphic.stroke, width } });
          }}
        />
        <SelectInput
          label={t('libedit.props.fill')}
          value={graphic.fill ?? 'none'}
          options={fills}
          testId="sel-graphic-fill"
          onSave={(fill) => {
            save({ ...graphic, fill });
          }}
        />
      </section>
    );
  }
  if (ref.kind === 'text') {
    const text = content.texts?.[ref.index];
    if (text === undefined) {
      return null;
    }
    const save = (next: SymbolText): void => {
      void run({ op: 'updateText', index: ref.index, text: next });
    };
    return (
      <section className="libedit-props" data-testid="selection-props">
        <h3 className="libedit-props__title">{t('libedit.props.text')}</h3>
        <TextInput
          label={t('libedit.props.textValue')}
          value={text.text}
          onSave={(value) => {
            save({ ...text, text: value });
          }}
        />
        <LengthTuple
          label={t('libedit.props.position')}
          values={text.at}
          units={units}
          onSave={([x = 0, y = 0]) => {
            save({ ...text, at: [x, y] });
          }}
        />
        <LengthInput
          label={t('libedit.props.textSize')}
          value={text.style?.size ?? 1_270_000}
          units={units}
          min={1}
          onSave={(size) => {
            save({ ...text, style: { ...text.style, size } });
          }}
        />
        <NumberInput
          label={t('libedit.table.rotation')}
          value={text.rotation ?? 0}
          suffix="°"
          onSave={(rotation) => {
            save({ ...text, rotation });
          }}
        />
      </section>
    );
  }
  return null;
}

function SelectionNote({ text }: { readonly text: string }): ReactNode {
  return (
    <p className="libedit-props__muted" data-testid="selection-note">
      {text}
    </p>
  );
}

/** Copper, mask and paste layers of one side, as check boxes. */
const PAD_LAYERS = [
  'cu.top',
  'cu.bottom',
  'cu.*',
  'mask.top',
  'mask.bottom',
  'paste.top',
  'paste.bottom',
] as const;

/** The selected item of a footprint. */
export function FootprintSelection({
  def,
  selection,
  units,
  run,
}: {
  readonly def: FootprintDef;
  readonly selection: readonly ItemRef[];
  readonly units: Units;
  readonly run: Run;
}): ReactNode {
  const { t } = useAppState();
  const [ref] = selection;
  if (selection.length > 1) {
    return <SelectionNote text={t('libedit.props.multiple', { count: selection.length })} />;
  }
  if (ref === undefined) {
    return <SelectionNote text={t('libedit.props.nothing')} />;
  }
  if (ref.kind === 'pad') {
    const pad = def.pads?.[ref.index];
    if (pad === undefined) {
      return null;
    }
    const patch = (fields: Record<string, unknown>): void => {
      void run({ op: 'setPads', indices: [ref.index], patch: fields });
    };
    const shape = pad.shape;
    return (
      <section className="libedit-props" data-testid="selection-props">
        <h3 className="libedit-props__title">{t('libedit.props.pad', { number: pad.number })}</h3>
        <TextInput
          label={t('libedit.table.number')}
          value={pad.number}
          testId="sel-pad-number"
          onSave={(number) => {
            patch({ number });
          }}
        />
        <SelectInput
          label={t('libedit.props.kind')}
          value={pad.kind ?? 'smd'}
          options={PAD_KINDS.map(
            (kind) => [kind, t(`libedit.padKinds.${kind}` as MessageKey)] as const,
          )}
          testId="sel-pad-kind"
          onSave={(kind) => {
            patch({ kind });
          }}
        />
        <SelectInput
          label={t('libedit.props.shape')}
          value={shapeKey(shape)}
          options={SHAPE_KEYS.map(
            (key) => [key, t(`libedit.padShapes.${key}` as MessageKey)] as const,
          )}
          testId="sel-pad-shape"
          onSave={(key) => {
            patch({ shape: shapeOf(key, pad.size) });
          }}
        />
        {typeof shape !== 'string' && 'roundRect' in shape && (
          <NumberInput
            label={t('libedit.props.ratio')}
            value={shape.roundRect.ratio / 10}
            min={0}
            max={50}
            suffix="%"
            onSave={(percent) => {
              patch({ shape: { roundRect: { ratio: Math.round(percent * 10) } } });
            }}
          />
        )}
        <LengthTuple
          label={t('libedit.table.size')}
          values={pad.size}
          units={units}
          min={1}
          testId="sel-pad-size"
          onSave={([w = 1, h = 1]) => {
            patch({ size: [w, h] });
          }}
        />
        {pad.drill !== undefined && (
          <div className="field-grid field-grid--two">
            <LengthInput
              label={t('libedit.table.drill')}
              value={pad.drill.diameter}
              units={units}
              min={1}
              testId="sel-pad-drill"
              onSave={(diameter) => {
                patch({ drill: { ...pad.drill, diameter } });
              }}
            />
            <LengthInput
              label={t('libedit.props.slot')}
              value={pad.drill.slotLength ?? 0}
              units={units}
              min={0}
              onSave={(slotLength) => {
                const drill = pad.drill ?? { diameter: 0 };
                patch({
                  drill: {
                    diameter: drill.diameter,
                    ...(drill.offset === undefined ? {} : { offset: drill.offset }),
                    ...(slotLength > 0 ? { slotLength } : {}),
                  },
                });
              }}
            />
          </div>
        )}
        <NumberInput
          label={t('libedit.table.rotation')}
          value={pad.rotation ?? 0}
          suffix="°"
          onSave={(rotation) => {
            patch({ rotation });
          }}
        />
        <LengthTuple
          label={t('libedit.props.position')}
          values={pad.at}
          units={units}
          onSave={([x = 0, y = 0]) => {
            patch({ at: [x, y] });
          }}
        />
        <fieldset className="field--group libedit-layers">
          <legend>{t('libedit.table.layers')}</legend>
          {PAD_LAYERS.map((layer) => (
            <CheckInput
              key={layer}
              label={layer}
              checked={pad.layers.includes(layer)}
              onSave={(on) => {
                const layers = on
                  ? [...pad.layers, layer]
                  : pad.layers.filter((entry) => entry !== layer);
                patch({ layers });
              }}
            />
          ))}
        </fieldset>
        <div className="field-grid field-grid--two">
          <LengthInput
            label={t('libedit.props.maskMargin')}
            value={pad.maskMargin ?? 0}
            units={units}
            onSave={(maskMargin) => {
              patch({ maskMargin: maskMargin === 0 ? null : maskMargin });
            }}
          />
          <LengthInput
            label={t('libedit.props.pasteMargin')}
            value={pad.pasteMargin ?? 0}
            units={units}
            onSave={(pasteMargin) => {
              patch({ pasteMargin: pasteMargin === 0 ? null : pasteMargin });
            }}
          />
        </div>
      </section>
    );
  }
  if (ref.kind === 'graphic') {
    const graphic = def.graphics?.[ref.index];
    if (graphic === undefined) {
      return null;
    }
    const save = (next: BoardGraphic): void => {
      void run({ op: 'updateGraphic', index: ref.index, graphic: next });
    };
    return (
      <section className="libedit-props" data-testid="selection-props">
        <h3 className="libedit-props__title">{t('libedit.props.graphic')}</h3>
        <LayerSelect
          value={graphic.layer}
          onSave={(layer) => {
            save({ ...graphic, layer });
          }}
        />
        <GeometryFields
          geometry={graphic.geometry}
          units={units}
          onSave={(geometry) => {
            save({ ...graphic, geometry });
          }}
        />
        <LengthInput
          label={t('libedit.props.lineWidth')}
          value={graphic.width}
          units={units}
          min={0}
          onSave={(width) => {
            save({ ...graphic, width });
          }}
        />
        <CheckInput
          label={t('libedit.props.filled')}
          checked={graphic.filled === true}
          onSave={(filled) => {
            save({ ...graphic, filled });
          }}
        />
      </section>
    );
  }
  if (ref.kind === 'text') {
    const text = def.texts?.[ref.index];
    if (text === undefined) {
      return null;
    }
    const save = (next: FootprintText): void => {
      void run({ op: 'updateText', index: ref.index, text: next });
    };
    const kind = text.kind ?? 'designator';
    return (
      <section className="libedit-props" data-testid="selection-props">
        <h3 className="libedit-props__title">
          {t(`libedit.props.textKinds.${kind}` as MessageKey)}
        </h3>
        {kind === 'user' && (
          <TextInput
            label={t('libedit.props.textValue')}
            value={text.text ?? ''}
            onSave={(value) => {
              save({ ...text, text: value });
            }}
          />
        )}
        <LayerSelect
          value={text.layer}
          onSave={(layer) => {
            save({ ...text, layer });
          }}
        />
        <LengthTuple
          label={t('libedit.props.position')}
          values={text.at}
          units={units}
          onSave={([x = 0, y = 0]) => {
            save({ ...text, at: [x, y] });
          }}
        />
        <div className="field-grid field-grid--two">
          <LengthInput
            label={t('libedit.props.textSize')}
            value={text.style.size ?? 1_000_000}
            units={units}
            min={1}
            onSave={(size) => {
              save({ ...text, style: { ...text.style, size } });
            }}
          />
          <LengthInput
            label={t('libedit.props.thickness')}
            value={text.style.thickness ?? 150_000}
            units={units}
            min={1}
            onSave={(thickness) => {
              save({ ...text, style: { ...text.style, thickness } });
            }}
          />
        </div>
        <NumberInput
          label={t('libedit.table.rotation')}
          value={text.rotation ?? 0}
          suffix="°"
          onSave={(rotation) => {
            save({ ...text, rotation });
          }}
        />
        <CheckInput
          label={t('libedit.props.visible')}
          checked={text.visible !== false}
          onSave={(visible) => {
            save({ ...text, visible });
          }}
        />
      </section>
    );
  }
  return null;
}

function LayerSelect({
  value,
  onSave,
}: {
  readonly value: string;
  readonly onSave: (layer: string) => void;
}): ReactNode {
  const { t } = useAppState();
  const layers = [
    'silk.top',
    'silk.bottom',
    'fab.top',
    'fab.bottom',
    'courtyard.top',
    'courtyard.bottom',
    'cu.top',
    'cu.bottom',
    'mask.top',
    'mask.bottom',
    'paste.top',
    'paste.bottom',
  ];
  const options = (layers.includes(value) ? layers : [value, ...layers]).map(
    (layer) => [layer, layerName(t, layer)] as const,
  );
  return <SelectInput label={t('libedit.layer')} value={value} options={options} onSave={onSave} />;
}
