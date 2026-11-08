import { useEffect, useId, useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import {
  checkName,
  joinPath,
  parseLength,
  sanitizeFolderName,
  toUnits,
  unitSymbol,
  UNITS,
  type BoardOptions,
  type TemplateId,
  type TemplateInfo,
  type Units,
} from '@shared/project';

import { useProject } from '../project/ProjectProvider';
import { useAppState } from '../state';
import { Icon } from './Icon';
import { Modal } from './Modal';

const TEMPLATE_TEXT: Readonly<Record<TemplateId, readonly [MessageKey, MessageKey]>> = {
  empty: ['wizard.templateEmpty', 'wizard.templateEmptyHint'],
  'board-2l': ['wizard.templateBoard2l', 'wizard.templateBoard2lHint'],
  'board-4l': ['wizard.templateBoard4l', 'wizard.templateBoard4lHint'],
  'arduino-shield': ['wizard.templateArduino', 'wizard.templateArduinoHint'],
  demo: ['wizard.templateDemo', 'wizard.templateDemoHint'],
};

const LAYER_COUNTS = [1, 2, 4, 6, 8, 10, 12, 14, 16] as const;
const MM = 1_000_000;

/** A small drawing of what a template contains. */
function TemplatePreview({ id }: { readonly id: TemplateId }): ReactNode {
  switch (id) {
    case 'empty':
      return (
        <svg className="template-preview" viewBox="0 0 64 48" aria-hidden="true">
          <rect x="10" y="6" width="44" height="36" rx="2" className="tp-sheet" />
          <path d="M16 18h12M16 24h20M16 30h8" className="tp-line" />
          <rect x="38" y="32" width="14" height="8" className="tp-line" />
        </svg>
      );
    case 'arduino-shield':
      return (
        <svg className="template-preview" viewBox="0 0 64 48" aria-hidden="true">
          <path d="M6 8h48l4 4v24l-4 4H6z" className="tp-board" />
          <circle cx="18" cy="38" r="2.2" className="tp-hole" />
          <circle cx="19" cy="10" r="2.2" className="tp-hole" />
          <circle cx="52" cy="33" r="2.2" className="tp-hole" />
          <circle cx="52" cy="17" r="2.2" className="tp-hole" />
          <path d="M24 9h26M14 39h30" className="tp-pins" />
        </svg>
      );
    case 'demo':
      return (
        <svg className="template-preview" viewBox="0 0 64 48" aria-hidden="true">
          <rect x="6" y="6" width="52" height="36" rx="2" className="tp-board" />
          <rect x="14" y="14" width="10" height="8" className="tp-part" />
          <rect x="34" y="12" width="14" height="14" className="tp-part" />
          <rect x="18" y="30" width="6" height="4" className="tp-part" />
          <path d="M24 18h10M41 26v8H24" className="tp-track" />
        </svg>
      );
    case 'board-2l':
    case 'board-4l':
      return (
        <svg className="template-preview" viewBox="0 0 64 48" aria-hidden="true">
          {id === 'board-4l' && (
            <>
              <rect x="16" y="2" width="38" height="38" rx="2" className="tp-layer" />
              <rect x="13" y="5" width="38" height="38" rx="2" className="tp-layer" />
            </>
          )}
          <rect x="10" y="8" width="38" height="38" rx="2" className="tp-board" />
          <path d="M16 20h12v12h14" className="tp-track" />
        </svg>
      );
  }
}

interface BoardFields {
  readonly width: string;
  readonly height: string;
  readonly layers: number;
  readonly thickness: string;
}

function fieldsFor(board: BoardOptions, units: Units): BoardFields {
  return {
    width: toUnits(board.width, units),
    height: toUnits(board.height, units),
    layers: board.layers,
    thickness: toUnits(board.thickness, 'mm'),
  };
}

type BoardCheck =
  | { readonly ok: true; readonly board: BoardOptions }
  | { readonly ok: false; readonly field: keyof BoardFields; readonly message: MessageKey };

function checkBoard(fields: BoardFields, units: Units): BoardCheck {
  const width = parseLength(fields.width, units);
  const height = parseLength(fields.height, units);
  const thickness = parseLength(fields.thickness, 'mm');
  if (width === null) {
    return { ok: false, field: 'width', message: 'wizard.invalidNumber' };
  }
  if (height === null) {
    return { ok: false, field: 'height', message: 'wizard.invalidNumber' };
  }
  if (thickness === null) {
    return { ok: false, field: 'thickness', message: 'wizard.invalidNumber' };
  }
  if (width < 5 * MM || width > 2000 * MM) {
    return { ok: false, field: 'width', message: 'wizard.sizeRange' };
  }
  if (height < 5 * MM || height > 2000 * MM) {
    return { ok: false, field: 'height', message: 'wizard.sizeRange' };
  }
  if (thickness < 0.2 * MM || thickness > 10 * MM) {
    return { ok: false, field: 'thickness', message: 'wizard.thicknessRange' };
  }
  return { ok: true, board: { width, height, layers: fields.layers, thickness } };
}

const NAME_ERRORS = {
  empty: 'wizard.errorEmpty',
  tooLong: 'wizard.errorTooLong',
  invalid: 'wizard.errorInvalid',
} as const satisfies Record<string, MessageKey>;

/** The new-project wizard: project, template, board. */
export function NewProjectWizard(): ReactNode {
  const { t, appInfo } = useAppState();
  const { templates, createProject, closeWizard, busy } = useProject();
  const formId = useId();
  const [step, setStep] = useState(0);
  const [name, setName] = useState('');
  const [location, setLocation] = useState('');
  const [touched, setTouched] = useState(false);
  const [templateId, setTemplateId] = useState<TemplateId>('board-2l');
  const [units, setUnits] = useState<Units>('mm');
  const [board, setBoard] = useState<BoardFields | null>(null);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([window.ew.projects.recent(), window.ew.projects.defaultLocation()]).then(
      ([recent, fallback]) => {
        if (!cancelled) {
          setLocation((current) => (current === '' ? (recent.lastLocation ?? fallback) : current));
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);

  const template: TemplateInfo | undefined = templates.find((entry) => entry.id === templateId);
  const templateBoard = template?.board ?? null;
  const fields = board ?? (templateBoard === null ? null : fieldsFor(templateBoard, units));
  const nameProblem = checkName(name);
  const locationMissing = location.trim() === '';
  const boardCheck = fields === null ? null : checkBoard(fields, units);
  const folder = joinPath(location.trim(), sanitizeFolderName(name), appInfo.platform);

  const chooseTemplate = (id: TemplateId): void => {
    setTemplateId(id);
    setBoard(null);
  };

  const changeUnits = (next: Units): void => {
    if (fields !== null && boardCheck?.ok === true) {
      setBoard({
        ...fields,
        width: toUnits(boardCheck.board.width, next),
        height: toUnits(boardCheck.board.height, next),
      });
    }
    setUnits(next);
  };

  const updateBoard = (patch: Partial<BoardFields>): void => {
    if (fields !== null) {
      setBoard({ ...fields, ...patch });
    }
  };

  const steps: readonly MessageKey[] = [
    'wizard.stepProject',
    'wizard.stepTemplate',
    'wizard.stepBoard',
  ];
  const projectValid = nameProblem === null && !locationMissing;
  const canCreate = projectValid && (boardCheck === null || boardCheck.ok) && busy === null;

  const next = (): void => {
    if (step === 0 && !projectValid) {
      setTouched(true);
      return;
    }
    setStep((current) => Math.min(current + 1, steps.length - 1));
  };

  const create = (): void => {
    if (!canCreate) {
      setTouched(true);
      return;
    }
    void createProject({
      location: location.trim(),
      name,
      template: templateId,
      board: boardCheck?.ok === true ? boardCheck.board : null,
      units,
    });
  };

  const boardError = (field: keyof BoardFields): string | null =>
    boardCheck !== null && !boardCheck.ok && boardCheck.field === field
      ? t(boardCheck.message)
      : null;

  return (
    <Modal
      title={t('wizard.title')}
      onClose={closeWizard}
      testId="wizard"
      className="dialog--wizard"
      footer={
        <>
          <button type="button" className="button dialog__footer-start" onClick={closeWizard}>
            {t('wizard.cancel')}
          </button>
          {step > 0 && (
            <button
              type="button"
              className="button"
              data-testid="wizard-back"
              onClick={() => {
                setStep(step - 1);
              }}
            >
              {t('wizard.back')}
            </button>
          )}
          {step < steps.length - 1 && (
            <button type="button" className="button" data-testid="wizard-next" onClick={next}>
              {t('wizard.next')}
            </button>
          )}
          <button
            type="button"
            className="button button--primary"
            data-testid="wizard-create"
            disabled={!canCreate}
            onClick={create}
          >
            {t('wizard.create')}
          </button>
        </>
      }
    >
      <ol
        className="steps"
        aria-label={t('wizard.step', { current: step + 1, total: steps.length })}
      >
        {steps.map((label, index) => (
          <li key={label}>
            <button
              type="button"
              className="steps__item"
              aria-current={index === step ? 'step' : undefined}
              data-done={index < step}
              disabled={index > 0 && !projectValid}
              onClick={() => {
                setStep(index);
              }}
            >
              <span className="steps__number">{index + 1}</span>
              {t(label)}
            </button>
          </li>
        ))}
      </ol>

      {step === 0 && (
        <div className="wizard__page" data-testid="wizard-step-project">
          <label className="field">
            <span className="field__label">{t('wizard.name')}</span>
            <input
              className="field__control"
              type="text"
              value={name}
              placeholder={t('wizard.namePlaceholder')}
              data-testid="wizard-name"
              autoFocus
              aria-invalid={touched && nameProblem !== null}
              onChange={(event) => {
                setName(event.target.value);
              }}
              onKeyDown={(event) => {
                if (event.key === 'Enter') {
                  next();
                }
              }}
            />
            {touched && nameProblem !== null && (
              <span className="field__error">{t(NAME_ERRORS[nameProblem])}</span>
            )}
          </label>
          <div className="field">
            <label className="field__label" htmlFor={`${formId}-location`}>
              {t('wizard.location')}
            </label>
            <div className="field__row">
              <input
                id={`${formId}-location`}
                className="field__control field__control--grow"
                type="text"
                value={location}
                data-testid="wizard-location"
                onChange={(event) => {
                  setLocation(event.target.value);
                }}
              />
              <button
                type="button"
                className="button"
                data-testid="wizard-choose"
                onClick={() => {
                  void window.ew.dialogs.chooseFolder(location, 'location').then((chosen) => {
                    if (chosen !== null) {
                      setLocation(chosen);
                    }
                  });
                }}
              >
                {t('wizard.choose')}
              </button>
            </div>
            {touched && locationMissing ? (
              <span className="field__error">{t('wizard.errorLocation')}</span>
            ) : (
              <span className="field__hint" data-testid="wizard-folder">
                {t('wizard.folderPreview', { path: folder })}
              </span>
            )}
          </div>
          <fieldset className="field field--group">
            <legend className="field__label">{t('wizard.storage')}</legend>
            <div className="choice-grid choice-grid--two">
              <label className="choice" data-selected="true">
                <input type="radio" name={`${formId}-storage`} checked readOnly />
                <Icon name="computer" size={22} />
                <span className="choice__title">{t('wizard.local')}</span>
                <span className="choice__hint">{t('wizard.localHint')}</span>
              </label>
              <label className="choice" data-disabled="true">
                <input type="radio" name={`${formId}-storage`} disabled />
                <Icon name="github" size={22} />
                <span className="choice__title">{t('wizard.github')}</span>
                <span className="choice__hint">{t('wizard.githubHint')}</span>
              </label>
            </div>
          </fieldset>
        </div>
      )}

      {step === 1 && (
        <div className="choice-grid" role="radiogroup" data-testid="wizard-step-template">
          {templates.map((entry) => {
            const [title, hint] = TEMPLATE_TEXT[entry.id];
            const selected = entry.id === templateId;
            return (
              <label
                key={entry.id}
                className="choice choice--template"
                data-selected={selected}
                data-testid={`template-${entry.id}`}
              >
                <input
                  type="radio"
                  name={`${formId}-template`}
                  checked={selected}
                  onChange={() => {
                    chooseTemplate(entry.id);
                  }}
                />
                <TemplatePreview id={entry.id} />
                <span className="choice__title">{t(title)}</span>
                <span className="choice__hint">{t(hint)}</span>
              </label>
            );
          })}
        </div>
      )}

      {step === 2 && (
        <div className="wizard__page" data-testid="wizard-step-board">
          {fields === null ? (
            <p className="dialog__text dialog__text--muted">{t('wizard.noBoard')}</p>
          ) : (
            <>
              <div className="field-grid">
                <label className="field">
                  <span className="field__label">
                    {t('wizard.width')} ({unitSymbol(units)})
                  </span>
                  <input
                    className="field__control"
                    inputMode="decimal"
                    value={fields.width}
                    disabled={template?.fixedSize === true}
                    data-testid="board-width"
                    onChange={(event) => {
                      updateBoard({ width: event.target.value });
                    }}
                  />
                  {boardError('width') !== null && (
                    <span className="field__error">{boardError('width')}</span>
                  )}
                </label>
                <label className="field">
                  <span className="field__label">
                    {t('wizard.height')} ({unitSymbol(units)})
                  </span>
                  <input
                    className="field__control"
                    inputMode="decimal"
                    value={fields.height}
                    disabled={template?.fixedSize === true}
                    data-testid="board-height"
                    onChange={(event) => {
                      updateBoard({ height: event.target.value });
                    }}
                  />
                  {boardError('height') !== null && (
                    <span className="field__error">{boardError('height')}</span>
                  )}
                </label>
                <label className="field">
                  <span className="field__label">{t('wizard.layers')}</span>
                  <select
                    className="field__control"
                    value={fields.layers}
                    data-testid="board-layers"
                    onChange={(event) => {
                      updateBoard({ layers: Number(event.target.value) });
                    }}
                  >
                    {LAYER_COUNTS.map((count) => (
                      <option key={count} value={count}>
                        {count}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="field">
                  <span className="field__label">
                    {t('wizard.thickness')} ({unitSymbol('mm')})
                  </span>
                  <input
                    className="field__control"
                    inputMode="decimal"
                    value={fields.thickness}
                    data-testid="board-thickness"
                    onChange={(event) => {
                      updateBoard({ thickness: event.target.value });
                    }}
                  />
                  {boardError('thickness') !== null && (
                    <span className="field__error">{boardError('thickness')}</span>
                  )}
                </label>
              </div>
              {template?.fixedSize === true && (
                <p className="field__hint">{t('wizard.fixedSize')}</p>
              )}
            </>
          )}
          <label className="field">
            <span className="field__label">{t('wizard.units')}</span>
            <select
              className="field__control"
              value={units}
              data-testid="board-units"
              onChange={(event) => {
                changeUnits(UNITS.find((unit) => unit === event.target.value) ?? 'mm');
              }}
            >
              {UNITS.map((unit) => (
                <option key={unit} value={unit}>
                  {t(`units.${unit}`)}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}
    </Modal>
  );
}
