/**
 * Editable properties of a selected board object in the inspector:
 * designator, value, rotation and lock of footprints, width and layer of
 * tracks, sizes of vias and the text of board texts.
 */
import { useEffect, useState, type ReactNode } from 'react';

import type { Units } from '@shared/project';

import { CheckInput, LengthInput, NumberInput, SelectInput, TextInput } from '../libedit/fields';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { layerName } from './BoardToolbar';

/** Object types with editable properties. */
export const BOARD_EDITABLE = new Set(['footprint', 'track', 'via', 'boardText']);

type Json = Record<string, unknown>;

function text(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function number(value: unknown): number {
  return typeof value === 'number' ? value : 0;
}

export function BoardProperties({
  doc,
  id,
  type,
  version,
  units,
  layers,
}: {
  readonly doc: string;
  readonly id: string;
  readonly type: string;
  /** Scene version: reloads after every change. */
  readonly version: number;
  readonly units: Units;
  /** Copper layers of the board. */
  readonly layers: readonly string[];
}): ReactNode {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  const [object, setObject] = useState<Json | null>(null);

  useEffect(() => {
    let cancelled = false;
    engine
      .call<Json>('document.object', { doc, id })
      .then((loaded) => {
        if (!cancelled) {
          setObject(loaded);
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [engine, doc, id, version]);

  if (object === null) {
    return null;
  }
  const patch = (fields: Json): void => {
    engine
      .call('edit.apply', {
        label: 'edit-properties',
        changes: [{ op: 'patch', doc, id, fields }],
      })
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      });
  };
  const flip = (): void => {
    engine.call('pcb.edit', { doc, op: 'flip', ids: [id] }).catch((error: unknown) => {
      notify(errorMessage(t, error), 'error');
    });
  };
  const locked = (
    <CheckInput
      label={t('board.props.locked')}
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
          <TextInput
            label={t('board.props.designator')}
            value={text(object['designator'])}
            testId="board-prop-designator"
            onSave={(value) => {
              patch({ designator: value });
            }}
          />
          <TextInput
            label={t('board.props.value')}
            value={text(object['value'])}
            testId="board-prop-value"
            onSave={(value) => {
              patch({ value });
            }}
          />
          <NumberInput
            label={t('board.props.rotation')}
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
            label={t('board.props.width')}
            value={number(object['width'])}
            units={units}
            min={1}
            testId="board-prop-width"
            onSave={(value) => {
              patch({ width: value });
            }}
          />
          <SelectInput
            label={t('board.props.layer')}
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
            label={t('board.props.diameter')}
            value={number(object['diameter'])}
            units={units}
            min={1}
            testId="board-prop-diameter"
            onSave={(value) => {
              patch({ diameter: value });
            }}
          />
          <LengthInput
            label={t('board.props.drill')}
            value={number(object['drill'])}
            units={units}
            min={1}
            testId="board-prop-drill"
            onSave={(value) => {
              patch({ drill: value });
            }}
          />
          {locked}
        </>
      )}
      {type === 'boardText' && (
        <TextInput
          label={t('board.props.text')}
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
