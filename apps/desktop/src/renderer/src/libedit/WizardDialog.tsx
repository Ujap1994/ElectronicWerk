/**
 * The footprint wizard: package dimensions in, an IPC-7351 land pattern
 * out (`libedit.wizard` for the live preview, `libedit.create` to add it).
 */
import { useEffect, useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import { formatSize, type Units } from '@shared/project';

import { Modal } from '../components/Modal';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { findingText } from './labels';
import { CheckInput, LengthInput, NumberInput, SelectInput } from './fields';
import { FootprintSvg } from './FootprintSvg';
import type { FootprintDef, Finding } from './types';

export type WizardKind = 'chip' | 'gullWing' | 'noLead' | 'bga' | 'header';

/** Wizard parameters as the engine takes them. */
export type WizardParams = { readonly kind: WizardKind } & Readonly<Record<string, unknown>>;

interface Preset {
  readonly name: string;
  readonly wizard: WizardParams;
}

const FAMILIES: readonly WizardKind[] = ['chip', 'gullWing', 'noLead', 'bga', 'header'];

type Field =
  | { readonly key: string; readonly type: 'length' }
  | { readonly key: string; readonly type: 'count' }
  | { readonly key: string; readonly type: 'flag' };

const length = (key: string): Field => ({ key, type: 'length' });
const count = (key: string): Field => ({ key, type: 'count' });

/** The inputs of each package family, in datasheet order. */
const FIELDS: Readonly<Record<WizardKind, readonly Field[]>> = {
  chip: [length('bodyLength'), length('bodyWidth'), length('terminal')],
  gullWing: [
    count('pins'),
    { key: 'quad', type: 'flag' },
    length('pitch'),
    length('leadSpan'),
    length('leadLength'),
    length('leadWidth'),
    length('bodyWidth'),
    length('bodyLength'),
  ],
  noLead: [
    count('pins'),
    { key: 'quad', type: 'flag' },
    length('pitch'),
    length('bodyWidth'),
    length('bodyLength'),
    length('terminalLength'),
    length('terminalWidth'),
  ],
  bga: [
    count('rows'),
    count('columns'),
    length('pitch'),
    length('ball'),
    length('bodyWidth'),
    length('bodyLength'),
  ],
  header: [count('rows'), count('pinsPerRow'), length('pitch'), length('drill'), length('pad')],
};

interface Preview {
  readonly name: string;
  readonly def: FootprintDef;
  readonly findings: readonly Finding[];
}

/** Size of the courtyard (from its graphics). */
function courtyardSize(def: FootprintDef): [number, number] | null {
  const xs: number[] = [];
  const ys: number[] = [];
  for (const graphic of def.graphics ?? []) {
    if (!graphic.layer.startsWith('courtyard')) {
      continue;
    }
    const g = graphic.geometry;
    const points =
      'rect' in g
        ? [g.rect.start, g.rect.end]
        : 'polygon' in g
          ? g.polygon.points
          : 'polyline' in g
            ? g.polyline.points
            : 'line' in g
              ? [g.line.start, g.line.end]
              : [];
    for (const p of points) {
      xs.push(p[0]);
      ys.push(p[1]);
    }
  }
  if (xs.length === 0) {
    return null;
  }
  return [Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys)];
}

export function WizardDialog({
  doc,
  units,
  taken,
  onCreated,
  onClose,
}: {
  readonly doc: string;
  readonly units: Units;
  /** Footprint names already in the library. */
  readonly taken: readonly string[];
  readonly onCreated: (name: string) => void;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine, snapshot: settings } = useAppState();
  const { notify } = useProject();
  const [presets, setPresets] = useState<readonly Preset[]>([]);
  const [preset, setPreset] = useState('');
  const [wizard, setWizard] = useState<WizardParams | null>(null);
  const [name, setName] = useState<string | null>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [problem, setProblem] = useState<string | null>(null);

  useEffect(() => {
    void engine.call<Preset[]>('libedit.presets').then((list) => {
      setPresets(list);
      const first = list.find((entry) => entry.name === 'SOIC-8') ?? list[0];
      if (first !== undefined) {
        setPreset(first.name);
        setWizard(first.wizard);
      }
    });
  }, [engine]);

  // The live preview.
  useEffect(() => {
    if (wizard === null) {
      return undefined;
    }
    let cancelled = false;
    const timer = setTimeout(() => {
      engine.call<Preview>('libedit.wizard', { wizard }).then(
        (reply) => {
          if (!cancelled) {
            setPreview(reply);
            setProblem(null);
          }
        },
        (error: unknown) => {
          if (!cancelled) {
            setPreview(null);
            setProblem(error instanceof Error ? error.message : String(error));
          }
        },
      );
    }, 120);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [engine, wizard]);

  const finalName = (name ?? preview?.name ?? '').trim();
  const nameTaken = taken.includes(finalName);
  const set = (key: string, value: unknown): void => {
    if (wizard !== null) {
      setWizard({ ...wizard, [key]: value });
      setPreset('');
    }
  };
  const family = wizard?.kind ?? 'gullWing';
  const courtyard = preview === null ? null : courtyardSize(preview.def);
  const exposed = wizard?.['exposedPad'] as readonly [number, number] | null | undefined;

  return (
    <Modal
      title={t('libedit.wizard.title')}
      onClose={onClose}
      testId="fpwizard-dialog"
      className="dialog--wide libedit-wizard"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('tree.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={wizard === null || preview === null || finalName === '' || nameTaken}
            data-testid="fpwizard-create"
            onClick={() => {
              engine
                .call<{ name: string }>('libedit.create', {
                  doc,
                  kind: 'footprint',
                  name: finalName,
                  wizard,
                })
                .then(
                  (reply) => {
                    onCreated(reply.name);
                  },
                  (error: unknown) => {
                    notify(errorMessage(t, error), 'error');
                  },
                );
            }}
          >
            {t('libedit.wizard.create')}
          </button>
        </>
      }
    >
      <p className="dialog__text dialog__text--muted">{t('libedit.wizard.intro')}</p>
      <div className="libedit-wizard__body">
        <div className="libedit-wizard__form">
          <label className="field field--compact">
            <span className="field__label">{t('libedit.wizard.preset')}</span>
            <select
              className="field__control"
              value={preset}
              data-testid="fpwizard-preset"
              onChange={(event) => {
                const found = presets.find((entry) => entry.name === event.target.value);
                setPreset(event.target.value);
                if (found !== undefined) {
                  setWizard(found.wizard);
                  setName(null);
                }
              }}
            >
              <option value="">{t('libedit.wizard.custom')}</option>
              {FAMILIES.map((kind) => (
                <optgroup key={kind} label={t(`libedit.wizard.families.${kind}`)}>
                  {presets
                    .filter((entry) => entry.wizard.kind === kind)
                    .map((entry) => (
                      <option key={entry.name} value={entry.name}>
                        {entry.name}
                      </option>
                    ))}
                </optgroup>
              ))}
            </select>
          </label>
          <SelectInput
            label={t('libedit.wizard.family')}
            value={family}
            options={FAMILIES.map((kind) => [kind, t(`libedit.wizard.families.${kind}`)] as const)}
            testId="fpwizard-family"
            onSave={(kind) => {
              const first = presets.find((entry) => entry.wizard.kind === kind);
              if (first !== undefined) {
                setWizard(first.wizard);
                setPreset(first.name);
                setName(null);
              }
            }}
          />
          {wizard !== null && (
            <div className="field-grid field-grid--two">
              {FIELDS[family].map((field) => {
                const label = t(`libedit.wizard.fields.${field.key}` as MessageKey);
                const value = wizard[field.key];
                if (field.type === 'flag') {
                  return (
                    <CheckInput
                      key={field.key}
                      label={label}
                      checked={value === true}
                      testId={`fpwizard-${field.key}`}
                      onSave={(next) => {
                        set(field.key, next);
                      }}
                    />
                  );
                }
                if (field.type === 'count') {
                  return (
                    <NumberInput
                      key={field.key}
                      label={label}
                      value={typeof value === 'number' ? value : 1}
                      integer
                      min={1}
                      max={2000}
                      testId={`fpwizard-${field.key}`}
                      onSave={(next) => {
                        set(field.key, next);
                      }}
                    />
                  );
                }
                return (
                  <LengthInput
                    key={field.key}
                    label={label}
                    value={typeof value === 'number' ? value : 0}
                    units={units}
                    min={1}
                    testId={`fpwizard-${field.key}`}
                    onSave={(next) => {
                      set(field.key, next);
                    }}
                  />
                );
              })}
              {family === 'noLead' && (
                <>
                  <CheckInput
                    label={t('libedit.wizard.fields.exposedPad')}
                    checked={exposed !== null && exposed !== undefined}
                    testId="fpwizard-exposedPad"
                    onSave={(on) => {
                      const body = Number(wizard['bodyWidth'] ?? 3_000_000);
                      const side = Math.round((body * 0.6) / 10_000) * 10_000;
                      set('exposedPad', on ? [side, side] : null);
                    }}
                  />
                  {exposed !== null && exposed !== undefined && (
                    <>
                      <LengthInput
                        label={t('libedit.wizard.fields.exposedWidth')}
                        value={exposed[0]}
                        units={units}
                        min={1}
                        onSave={(w) => {
                          set('exposedPad', [w, exposed[1]]);
                        }}
                      />
                      <LengthInput
                        label={t('libedit.wizard.fields.exposedLength')}
                        value={exposed[1]}
                        units={units}
                        min={1}
                        onSave={(h) => {
                          set('exposedPad', [exposed[0], h]);
                        }}
                      />
                    </>
                  )}
                </>
              )}
              {family !== 'header' && (
                <SelectInput
                  label={t('libedit.wizard.density')}
                  value={
                    (wizard['density'] as 'least' | 'nominal' | 'most' | undefined) ?? 'nominal'
                  }
                  options={(['least', 'nominal', 'most'] as const).map(
                    (density) => [density, t(`libedit.wizard.densities.${density}`)] as const,
                  )}
                  testId="fpwizard-density"
                  onSave={(density) => {
                    set('density', density);
                  }}
                />
              )}
            </div>
          )}
          <label className="field field--compact">
            <span className="field__label">{t('libedit.wizard.name')}</span>
            <input
              className="field__control"
              value={name ?? preview?.name ?? ''}
              spellCheck={false}
              data-testid="fpwizard-name"
              aria-invalid={nameTaken}
              onChange={(event) => {
                setName(event.target.value);
              }}
            />
            {nameTaken && <span className="field__error">{t('libedit.dialogs.nameTaken')}</span>}
          </label>
        </div>
        <div className="libedit-wizard__preview" data-testid="fpwizard-preview">
          {preview !== null && (
            <>
              <FootprintSvg def={preview.def} label={t('libedit.wizard.preview')} />
              <p className="libedit-wizard__facts">
                {t('libedit.wizard.pads', { count: preview.def.pads?.length ?? 0 })}
                {courtyard !== null &&
                  ` · ${t('libedit.wizard.courtyard', {
                    size: formatSize(courtyard[0], courtyard[1], units, settings.language),
                  })}`}
              </p>
              {preview.findings.length > 0 && (
                <ul className="issues">
                  {preview.findings.map((finding) => (
                    <li
                      key={finding.code}
                      className={`issues__item issues__item--${finding.severity}`}
                    >
                      <span className="issues__message">{findingText(t, finding)}</span>
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
          {problem !== null && (
            <p className="callout callout--warning" role="alert">
              {t('libedit.wizard.invalid', { message: problem })}
            </p>
          )}
        </div>
      </div>
    </Modal>
  );
}
