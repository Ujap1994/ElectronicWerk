/**
 * Editable properties of the selected board objects in the inspector:
 * designator, value, rotation and lock of footprints, width and layer of
 * tracks, sizes of vias and the text of board texts. With several objects
 * of one type, a change applies to all of them (one undo step); fields whose
 * values differ say so.
 */
import { useEffect, useState, type ReactNode } from 'react';

import type { Units } from '@shared/project';

import { CheckInput, LengthInput, NumberInput, SelectInput, TextInput } from '../libedit/fields';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { layerLabel } from '../viewer/labels';
import { layerName } from './BoardToolbar';
import { viaTypeOf } from './editor';

/** Object types with editable properties. */
export const BOARD_EDITABLE = new Set([
  'footprint',
  'track',
  'via',
  'boardText',
  'dimension',
  'room',
]);

type Json = Record<string, unknown>;

function text(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function number(value: unknown): number {
  return typeof value === 'number' ? value : 0;
}

const NO_LAYERS: readonly string[] = [];

/** IPC-4761 via fills, type I to VII. */
const VIA_FILLS = [
  'tented',
  'tentedCovered',
  'plugged',
  'pluggedCovered',
  'filled',
  'filledCovered',
  'filledCapped',
] as const;

/** At most this many objects are edited together. */
export const MAX_EDITED = 2000;

export function BoardProperties({
  doc,
  ids,
  type,
  version,
  units,
  layers,
  drawingLayers = NO_LAYERS,
}: {
  readonly doc: string;
  /** The selected objects (all of `type`). */
  readonly ids: readonly string[];
  readonly type: string;
  /** Scene version: reloads after every change. */
  readonly version: number;
  readonly units: Units;
  /** Copper layers of the board. */
  readonly layers: readonly string[];
  /** Drawing layers of the board (user, mechanical, fabrication, silkscreen). */
  readonly drawingLayers?: readonly string[];
}): ReactNode {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  const [objects, setObjects] = useState<readonly Json[] | null>(null);
  const key = ids.join(',');

  useEffect(() => {
    let cancelled = false;
    void Promise.all(
      key.split(',').map((id) => engine.call<Json>('document.object', { doc, id })),
    ).then(
      (loaded) => {
        if (!cancelled) {
          setObjects(loaded);
        }
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [engine, doc, key, version]);

  const object = objects?.[0];
  if (objects === null || object === undefined) {
    return null;
  }
  const many = objects.length > 1;
  /** The label, marked when the objects differ in this field. */
  const label = (text: string, field: string): string =>
    many && objects.some((o) => JSON.stringify(o[field]) !== JSON.stringify(object[field]))
      ? `${text} · ${t('board.props.mixed')}`
      : text;
  // Vias store their layers only when they differ from top and bottom.
  const viaFrom = text(object['from']) || 'cu.top';
  const viaTo = text(object['to']) || 'cu.bottom';
  const patch = (fields: Json): void => {
    engine
      .call('edit.apply', {
        label: 'edit-properties',
        changes: ids.map((id) => ({ op: 'patch', doc, id, fields })),
      })
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      });
  };
  const flip = (): void => {
    engine.call('pcb.edit', { doc, op: 'flip', ids }).catch((error: unknown) => {
      notify(errorMessage(t, error), 'error');
    });
  };
  const locked = (
    <CheckInput
      label={label(t('board.props.locked'), 'locked')}
      checked={object['locked'] === true}
      testId="board-prop-locked"
      onSave={(value) => {
        patch({ locked: value });
      }}
    />
  );
  return (
    <div className="property-editor board-properties" data-testid="board-properties">
      {type === 'footprint' && (
        <>
          {!many && (
            <TextInput
              label={t('board.props.designator')}
              value={text(object['designator'])}
              testId="board-prop-designator"
              onSave={(value) => {
                patch({ designator: value });
              }}
            />
          )}
          <TextInput
            label={label(t('board.props.value'), 'value')}
            value={text(object['value'])}
            testId="board-prop-value"
            onSave={(value) => {
              patch({ value });
            }}
          />
          <NumberInput
            label={label(t('board.props.rotation'), 'rotation')}
            value={number(object['rotation'])}
            suffix="°"
            testId="board-prop-rotation"
            onSave={(value) => {
              patch({ rotation: ((value % 360) + 360) % 360 });
            }}
          />
          <div className="board-properties__side">
            <span>
              {object['side'] === 'bottom' ? t('board.props.bottom') : t('board.props.top')}
            </span>
            <button
              type="button"
              className="button button--small"
              data-testid="board-prop-flip"
              onClick={flip}
            >
              {t('board.actions.flip')}
            </button>
          </div>
          {locked}
        </>
      )}
      {type === 'track' && (
        <>
          <LengthInput
            label={label(t('board.props.width'), 'width')}
            value={number(object['width'])}
            units={units}
            min={1}
            testId="board-prop-width"
            onSave={(value) => {
              patch({ width: value });
            }}
          />
          <SelectInput
            label={label(t('board.props.layer'), 'layer')}
            value={text(object['layer'])}
            testId="board-prop-layer"
            options={layers.map((layer) => [layer, layerName(t, layer)])}
            onSave={(value) => {
              patch({ layer: value });
            }}
          />
          {locked}
        </>
      )}
      {type === 'via' && (
        <>
          <LengthInput
            label={label(t('board.props.diameter'), 'diameter')}
            value={number(object['diameter'])}
            units={units}
            min={1}
            testId="board-prop-diameter"
            onSave={(value) => {
              patch({ diameter: value });
            }}
          />
          <LengthInput
            label={label(t('board.props.drill'), 'drill')}
            value={number(object['drill'])}
            units={units}
            min={1}
            testId="board-prop-drill"
            onSave={(value) => {
              patch({ drill: value });
            }}
          />
          <SelectInput
            label={label(t('board.props.from'), 'from')}
            value={viaFrom}
            testId="board-prop-via-from"
            options={layers.map((layer) => [layer, layerName(t, layer)])}
            onSave={(value) => {
              patch({ from: value });
            }}
          />
          <SelectInput
            label={label(t('board.props.to'), 'to')}
            value={viaTo}
            testId="board-prop-via-to"
            options={layers.map((layer) => [layer, layerName(t, layer)])}
            onSave={(value) => {
              patch({ to: value });
            }}
          />
          <CheckInput
            label={label(t('board.props.micro'), 'micro')}
            checked={object['micro'] === true}
            testId="board-prop-via-micro"
            onSave={(micro) => {
              patch({ micro });
            }}
          />
          <p className="field__hint" data-testid="board-prop-via-type">
            {t('board.props.viaType')}:{' '}
            {t(`board.viaTypes.${viaTypeOf(viaFrom, viaTo, object['micro'] === true)}`)}
          </p>
          <SelectInput
            label={label(t('board.props.viaFill'), 'fill')}
            value={text(object['fill'])}
            testId="board-prop-via-fill"
            options={[
              ['', t('board.viaFills.board')],
              ...VIA_FILLS.map((fill) => [fill, t(`board.viaFills.${fill}`)] as const),
            ]}
            onSave={(value) => {
              patch({ fill: value === '' ? null : value });
            }}
          />
          {locked}
        </>
      )}
      {type === 'room' && (
        <>
          <TextInput
            label={label(t('board.rooms.name'), 'name')}
            value={text(object['name'])}
            testId="board-prop-room-name"
            onSave={(value) => {
              if (value.trim() !== '') {
                patch({ name: value.trim() });
              }
            }}
          />
          <p className="field__hint">{t('board.rooms.nameHint')}</p>
          {locked}
        </>
      )}
      {type === 'dimension' && (
        <>
          <TextInput
            label={label(
              object['kind'] === 'leader' ? t('board.dimensions.note') : t('board.dimensions.text'),
              'text',
            )}
            value={text(object['text'])}
            testId="board-prop-dimension-text"
            onSave={(value) => {
              patch({ text: value });
            }}
          />
          {object['kind'] !== 'leader' && (
            <>
              <p className="field__hint">{t('board.dimensions.textHint')}</p>
              <SelectInput
                label={label(t('board.dimensions.units'), 'units')}
                value={text(object['units']) || 'project'}
                testId="board-prop-dimension-units"
                options={[
                  ['project', t('board.dimensions.projectUnits')],
                  ['mm', 'mm'],
                  ['mil', 'mil'],
                  ['inch', 'in'],
                ]}
                onSave={(value) => {
                  patch({ units: value === 'project' ? null : value });
                }}
              />
              <SelectInput
                label={label(t('board.dimensions.precision'), 'precision')}
                value={
                  typeof object['precision'] === 'number' ? String(object['precision']) : 'auto'
                }
                testId="board-prop-dimension-precision"
                options={[
                  ['auto', t('board.dimensions.precisionAuto')],
                  ...['0', '1', '2', '3', '4'].map((n) => [n, n] as const),
                ]}
                onSave={(value) => {
                  patch({ precision: value === 'auto' ? null : Number(value) });
                }}
              />
              <CheckInput
                label={label(t('board.dimensions.hideUnits'), 'hideUnits')}
                checked={object['hideUnits'] === true}
                testId="board-prop-dimension-hide-units"
                onSave={(value) => {
                  patch({ hideUnits: value });
                }}
              />
              {(object['kind'] === 'aligned' ||
                object['kind'] === 'horizontal' ||
                object['kind'] === 'vertical') && (
                <LengthInput
                  label={label(t('board.dimensions.offset'), 'offset')}
                  value={number(object['offset'])}
                  units={units}
                  testId="board-prop-dimension-offset"
                  onSave={(value) => {
                    patch({ offset: value });
                  }}
                />
              )}
            </>
          )}
          <LengthInput
            label={label(t('board.dimensions.textSize'), 'style')}
            value={number((object['style'] as Json | undefined)?.['size'])}
            units={units}
            min={100_000}
            testId="board-prop-dimension-size"
            onSave={(value) => {
              patch({ style: { ...(object['style'] as Json | undefined), size: value } });
            }}
          />
          <LengthInput
            label={label(t('board.dimensions.lineWidth'), 'width')}
            value={number(object['width'])}
            units={units}
            min={10_000}
            testId="board-prop-dimension-width"
            onSave={(value) => {
              patch({ width: value });
            }}
          />
          <SelectInput
            label={label(t('board.props.layer'), 'layer')}
            value={text(object['layer'])}
            testId="board-prop-dimension-layer"
            options={(drawingLayers.includes(text(object['layer']))
              ? drawingLayers
              : [text(object['layer']), ...drawingLayers]
            ).map((layer) => [layer, layerLabel(t, layer)])}
            onSave={(value) => {
              patch({ layer: value });
            }}
          />
          {locked}
        </>
      )}
      {type === 'boardText' && (
        <TextInput
          label={label(t('board.props.text'), 'text')}
          value={text(object['text'])}
          testId="board-prop-text"
          onSave={(value) => {
            patch({ text: value });
          }}
        />
      )}
    </div>
  );
}
