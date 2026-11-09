import { useEffect, useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';

import { Icon } from '../components/Icon';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { useOpenDocument } from '../viewer/open-context';

/** Object types with editable properties. */
export const EDITABLE_TYPES = new Set(['symbol', 'label', 'powerPort', 'text', 'sheet']);

type Json = Record<string, unknown>;

const LABEL_KINDS = ['local', 'global', 'hierarchical'] as const;
const SHAPES = ['input', 'output', 'bidirectional', 'triState', 'passive'] as const;
const STYLES = ['bar', 'arrow', 'circle', 'wave', 'ground', 'signalGround', 'earth'] as const;

function text(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

/** A text field that saves on Enter or when it loses focus. */
function TextField({
  label,
  value,
  testId,
  multiline = false,
  onSave,
}: {
  readonly label: string;
  readonly value: string;
  readonly testId: string;
  readonly multiline?: boolean;
  readonly onSave: (value: string) => void;
}): ReactNode {
  const [draft, setDraft] = useState(value);
  const save = (): void => {
    if (draft !== value) {
      onSave(draft);
    }
  };
  return (
    <label className="field field--compact">
      <span className="field__label">{label}</span>
      {multiline ? (
        <textarea
          className="field__control"
          value={draft}
          rows={3}
          data-testid={testId}
          onChange={(event) => {
            setDraft(event.target.value);
          }}
          onBlur={save}
        />
      ) : (
        <input
          className="field__control"
          value={draft}
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
      )}
    </label>
  );
}

function Parameters({
  values,
  onSave,
}: {
  readonly values: Readonly<Record<string, string>>;
  readonly onSave: (patch: Readonly<Record<string, string | null>>) => void;
}): ReactNode {
  const { t } = useAppState();
  const [name, setName] = useState('');
  return (
    <fieldset className="field--group inspector-edit__parameters">
      <legend>{t('schematic.properties.parameters')}</legend>
      {Object.entries(values).map(([key, value]) => (
        <div key={key} className="inspector-edit__parameter">
          <TextField
            label={key}
            value={value}
            testId={`property-param-${key}`}
            onSave={(next) => {
              onSave({ [key]: next });
            }}
          />
          <button
            type="button"
            className="button button--quiet button--small"
            aria-label={t('schematic.properties.removeParameter', { name: key })}
            title={t('schematic.properties.removeParameter', { name: key })}
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
          if (key !== '' && !(key in values)) {
            onSave({ [key]: '' });
            setName('');
          }
        }}
      >
        <input
          className="field__control"
          value={name}
          placeholder={t('schematic.properties.parameterName')}
          aria-label={t('schematic.properties.parameterName')}
          data-testid="property-param-new"
          onChange={(event) => {
            setName(event.target.value);
          }}
        />
        <button type="submit" className="button button--small">
          {t('schematic.properties.addParameter')}
        </button>
      </form>
    </fieldset>
  );
}

/**
 * Editable properties of a schematic object: designator, value, footprint,
 * parameters and flags of a symbol, names of labels, power ports, texts
 * and sheets. Every change is one undo step.
 */
export function PropertyEditor({
  doc,
  id,
  version,
}: {
  readonly doc: string;
  readonly id: string;
  /** Scene version: the object is read again after every change. */
  readonly version: number;
}): ReactNode {
  const { t, engine } = useAppState();
  const { project, notify } = useProject();
  const openDocument = useOpenDocument();
  const [object, setObject] = useState<{ key: string; value: Json } | null>(null);
  const key = `${id}#${version.toString()}`;

  useEffect(() => {
    let cancelled = false;
    void engine.call<{ objects: Json[] }>('schematic.copy', { doc, ids: [id] }).then(
      (reply) => {
        const value = reply.objects[0];
        if (!cancelled && value !== undefined) {
          setObject({ key, value });
        }
      },
      () => undefined,
    );
    return () => {
      cancelled = true;
    };
  }, [engine, doc, id, key]);

  if (object?.key !== key) {
    return null;
  }
  const value = object.value;
  const type = text(value['type']);
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
  const field = (name: string, label: MessageKey, multiline = false): ReactNode => (
    <TextField
      key={`${key}-${name}`}
      label={t(label)}
      value={text(value[name])}
      multiline={multiline}
      testId={`property-${name}`}
      onSave={(next) => {
        patch({ [name]: next });
      }}
    />
  );
  const select = <T extends string>(
    name: string,
    label: MessageKey,
    options: readonly T[],
    optionLabel: (option: T) => string,
    fallback: T,
  ): ReactNode => (
    <label className="field field--compact">
      <span className="field__label">{t(label)}</span>
      <select
        className="field__control"
        value={text(value[name]) === '' ? fallback : text(value[name])}
        data-testid={`property-${name}`}
        onChange={(event) => {
          patch({ [name]: event.target.value });
        }}
      >
        {options.map((option) => (
          <option key={option} value={option}>
            {optionLabel(option)}
          </option>
        ))}
      </select>
    </label>
  );

  let body: ReactNode = null;
  if (type === 'symbol') {
    const flags = (value['flags'] ?? {}) as Readonly<Record<string, boolean>>;
    const properties = (value['properties'] ?? {}) as Readonly<Record<string, string>>;
    const flag = (name: string, label: MessageKey): ReactNode => (
      <label className="checkbox">
        <input
          type="checkbox"
          checked={flags[name] === true}
          data-testid={`property-flag-${name}`}
          onChange={(event) => {
            patch({ flags: { [name]: event.target.checked } });
          }}
        />
        {t(label)}
      </label>
    );
    body = (
      <>
        {field('designator', 'schematic.properties.designator')}
        {field('value', 'schematic.properties.value')}
        {field('footprint', 'schematic.properties.footprint')}
        <Parameters
          key={`${key}-parameters`}
          values={properties}
          onSave={(changes) => {
            patch({ properties: changes });
          }}
        />
        {flag('dnp', 'schematic.properties.dnp')}
        {flag('excludeFromBom', 'schematic.properties.excludeFromBom')}
        {flag('excludeFromBoard', 'schematic.properties.excludeFromBoard')}
      </>
    );
  } else if (type === 'label') {
    const kind = text(value['kind']) === '' ? 'local' : text(value['kind']);
    body = (
      <>
        {field('text', 'schematic.properties.name')}
        {select(
          'kind',
          'schematic.properties.kind',
          LABEL_KINDS,
          (k) => t(`schematic.properties.labelKinds.${k}`),
          'local',
        )}
        {kind !== 'local' &&
          select(
            'shape',
            'schematic.properties.shape',
            SHAPES,
            (s) => t(`schematic.properties.shapes.${s}`),
            'passive',
          )}
      </>
    );
  } else if (type === 'powerPort') {
    body = (
      <>
        {field('net', 'schematic.properties.net')}
        {select(
          'style',
          'schematic.properties.style',
          STYLES,
          (s) => t(`schematic.properties.styles.${s}`),
          'bar',
        )}
      </>
    );
  } else if (type === 'text') {
    body = field('text', 'schematic.properties.text', true);
  } else if (type === 'sheet') {
    const child = project?.documents.find((document) => document.id === text(value['schematic']));
    body = (
      <>
        {field('name', 'schematic.properties.name')}
        <div className="inspector-edit__buttons">
          <button
            type="button"
            className="button button--small"
            data-testid="property-sync-pins"
            onClick={() => {
              engine.call('schematic.syncSheetPins', { doc, id }).catch((error: unknown) => {
                notify(errorMessage(t, error), 'error');
              });
            }}
          >
            {t('schematic.properties.syncPins')}
          </button>
          {child !== undefined && openDocument !== null && (
            <button
              type="button"
              className="button button--small"
              data-testid="property-open-sheet"
              onClick={() => {
                openDocument(child);
              }}
            >
              {t('schematic.properties.openSheet')}
            </button>
          )}
        </div>
      </>
    );
  }
  return (
    <div className="inspector-edit" data-testid="property-editor">
      {body}
    </div>
  );
}
