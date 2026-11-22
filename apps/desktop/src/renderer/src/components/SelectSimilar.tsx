/**
 * "Select similar": the properties of the selected object to compare by,
 * and the button that selects every object of its type that shares them.
 */
import { useState, type ReactNode } from 'react';

import { formatLength, type Units } from '@shared/project';

import { useAppState } from '../state';
import type { ViewerController } from '../viewer/controller';
import { fieldLabel } from '../viewer/labels';
import { Icon } from './Icon';

interface SimilarField {
  readonly key: string;
  readonly value: string | number | boolean;
  readonly length?: boolean;
  readonly text?: string;
}

/** Fields compared by default: what makes objects alike in practice. */
const DEFAULTS = new Set(['layer', 'width', 'diameter', 'drill', 'footprint', 'symbol', 'value']);

export function SelectSimilar({
  doc,
  id,
  units,
  controller,
}: {
  readonly doc: string;
  readonly id: string;
  readonly units: Units;
  readonly controller: ViewerController;
}): ReactNode {
  const { t, engine, snapshot } = useAppState();
  const [fields, setFields] = useState<readonly SimilarField[] | null>(null);
  const [chosen, setChosen] = useState<ReadonlySet<string>>(() => new Set());

  const open = (): void => {
    if (fields !== null) {
      setFields(null);
      return;
    }
    void engine.call<{ fields: SimilarField[] }>('document.similar', { doc, id }).then(
      (reply) => {
        setFields(reply.fields);
        setChosen(new Set(reply.fields.filter((f) => DEFAULTS.has(f.key)).map((f) => f.key)));
      },
      () => undefined,
    );
  };
  const shown = (field: SimilarField): string => {
    if (field.text !== undefined) {
      return field.text;
    }
    if (field.length === true && typeof field.value === 'number') {
      return formatLength(field.value, units, snapshot.language);
    }
    if (typeof field.value === 'boolean') {
      return field.value ? t('inspector.yes') : t('inspector.no');
    }
    return String(field.value);
  };
  const select = (): void => {
    void engine.call<{ ids: string[] }>('document.similar', { doc, id, keys: [...chosen] }).then(
      (reply) => {
        controller.selectIds(reply.ids);
        setFields(null);
      },
      () => undefined,
    );
  };

  return (
    <div className="select-similar">
      <button
        type="button"
        className="button button--small"
        aria-expanded={fields !== null}
        data-testid="select-similar"
        onClick={open}
      >
        <Icon name="duplicate" size={14} />
        {t('inspector.similar.open')}
      </button>
      {fields !== null && (
        <div className="select-similar__body" data-testid="select-similar-panel">
          <p className="field__hint">{t('inspector.similar.hint')}</p>
          {fields.map((field) => (
            <label key={field.key} className="checkbox select-similar__field">
              <input
                type="checkbox"
                checked={chosen.has(field.key)}
                data-testid={`similar-${field.key}`}
                onChange={(event) => {
                  const next = new Set(chosen);
                  if (event.target.checked) {
                    next.add(field.key);
                  } else {
                    next.delete(field.key);
                  }
                  setChosen(next);
                }}
              />
              <span>{fieldLabel(t, field.key)}</span>
              <span className="select-similar__value">{shown(field)}</span>
            </label>
          ))}
          <button
            type="button"
            className="button button--primary button--small"
            data-testid="similar-select"
            onClick={select}
          >
            {t('inspector.similar.select')}
          </button>
        </div>
      )}
    </div>
  );
}
