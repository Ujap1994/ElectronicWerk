/**
 * The design rules: net classes, which nets belong to them, the board's
 * manufacturing minimums, custom rules in the query language and the
 * severities of the DRC checks. Everything is stored in the project's
 * rules document.
 */
import { useEffect, useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import type { Units } from '@shared/project';

import { Modal } from '../components/Modal';
import { CheckInput, LengthInput, TextInput } from '../libedit/fields';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';

type Severity = 'error' | 'warning' | 'ignore';

interface NetClass {
  name: string;
  description?: string;
  clearance: number;
  trackWidth: number;
  viaDiameter: number;
  viaDrill: number;
  diffPairWidth: number;
  diffPairGap: number;
}

interface Constraints {
  minClearance: number;
  minTrackWidth: number;
  minViaDiameter: number;
  minViaDrill: number;
  minAnnularRing: number;
  minHole: number;
  minHoleToHole: number;
  minCopperToEdge: number;
  minSilkWidth: number;
}

type RuleConstraints = Partial<{
  clearance: number;
  trackWidthMin: number;
  trackWidthMax: number;
  trackWidth: number;
  viaDiameterMin: number;
  viaDrillMin: number;
  annularRingMin: number;
  holeMin: number;
  holeToHoleMin: number;
  edgeClearance: number;
}>;

interface CustomRule {
  name: string;
  enabled?: boolean;
  condition?: string;
  constraints: RuleConstraints;
}

interface RulesProps {
  name: string;
  constraints: Constraints;
  netClasses: NetClass[];
  assignments: { pattern: string; class: string }[];
  erc: unknown;
  custom: CustomRule[];
  drc: { severities?: Record<string, Severity> };
}

interface RulesReply {
  readonly rules: RulesProps;
  readonly checks: readonly { code: string; default: Severity; severity: Severity }[];
  readonly nets: Readonly<Record<string, readonly string[]>>;
}

type Tab = 'classes' | 'constraints' | 'custom' | 'checks';

const CONSTRAINTS: readonly (keyof Constraints)[] = [
  'minClearance',
  'minTrackWidth',
  'minViaDiameter',
  'minViaDrill',
  'minAnnularRing',
  'minHole',
  'minHoleToHole',
  'minCopperToEdge',
  'minSilkWidth',
];

const CLASS_FIELDS: readonly (keyof Omit<NetClass, 'name' | 'description'>)[] = [
  'clearance',
  'trackWidth',
  'viaDiameter',
  'viaDrill',
  'diffPairWidth',
  'diffPairGap',
];

const RULE_FIELDS: readonly (keyof RuleConstraints)[] = [
  'clearance',
  'trackWidthMin',
  'trackWidthMax',
  'trackWidth',
  'viaDiameterMin',
  'viaDrillMin',
  'annularRingMin',
  'holeMin',
  'holeToHoleMin',
  'edgeClearance',
];

/** A condition field that checks its syntax with the engine. */
function ConditionInput({
  value,
  index,
  onSave,
}: {
  readonly value: string;
  readonly index: number;
  readonly onSave: (value: string) => void;
}): ReactNode {
  const { t, engine } = useAppState();
  const [problem, setProblem] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    engine
      .call<{ ok: boolean; message?: string; position?: number }>('pcb.validateRule', {
        condition: value,
      })
      .then((reply) => {
        if (!cancelled) {
          setProblem(
            reply.ok
              ? null
              : t('board.rules.syntax', {
                  message: reply.message ?? '',
                  position: reply.position ?? 0,
                }),
          );
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [engine, t, value]);
  return (
    <div className="board-rule__condition">
      <TextInput
        label={t('board.rules.condition')}
        value={value}
        placeholder={t('board.rules.example')}
        testId={`rule-condition-${index.toString()}`}
        onSave={onSave}
      />
      {problem !== null && (
        <p
          className="field__hint field__hint--error"
          data-testid={`rule-error-${index.toString()}`}
        >
          {problem}
        </p>
      )}
    </div>
  );
}

/** The design rules editor. */
export function RulesDialog({
  doc,
  units,
  onClose,
}: {
  readonly doc: string;
  readonly units: Units;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  const [reply, setReply] = useState<RulesReply | null>(null);
  const [rules, setRules] = useState<RulesProps | null>(null);
  const [tab, setTab] = useState<Tab>('classes');

  useEffect(() => {
    let cancelled = false;
    engine
      .call<RulesReply>('pcb.rules', { doc })
      .then((loaded) => {
        if (!cancelled) {
          setReply(loaded);
          setRules(structuredClone(loaded.rules));
        }
      })
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      });
    return () => {
      cancelled = true;
    };
  }, [engine, notify, t, doc]);

  const save = (): void => {
    if (rules === null) {
      return;
    }
    engine
      .call('pcb.setRules', { props: rules })
      .then(onClose)
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      });
  };
  const patch = (next: Partial<RulesProps>): void => {
    if (rules !== null) {
      setRules({ ...rules, ...next });
    }
  };

  const classes = rules?.netClasses ?? [];
  const severities = rules?.drc.severities ?? {};
  const severityOf = (code: string, fallback: Severity): Severity => severities[code] ?? fallback;

  return (
    <Modal
      title={t('board.rules.title')}
      onClose={onClose}
      testId="rules-dialog"
      className="dialog--wide"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('board.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={rules === null}
            data-testid="rules-save"
            onClick={save}
          >
            {t('board.rules.save')}
          </button>
        </>
      }
    >
      <div className="segmented" role="tablist">
        {(['classes', 'constraints', 'custom', 'checks'] as const).map((value) => (
          <button
            key={value}
            type="button"
            role="tab"
            aria-selected={tab === value}
            className={
              tab === value ? 'segmented__item segmented__item--active' : 'segmented__item'
            }
            data-testid={`rules-tab-${value}`}
            onClick={() => {
              setTab(value);
            }}
          >
            {t(`board.rules.tabs.${value}`)}
          </button>
        ))}
      </div>
      {rules === null || reply === null ? (
        <p className="dialog__text dialog__text--muted">{t('board.rules.loading')}</p>
      ) : tab === 'classes' ? (
        <>
          <div className="board-table__scroll">
            <table className="board-table" data-testid="net-classes">
              <thead>
                <tr>
                  <th>{t('board.rules.className')}</th>
                  {CLASS_FIELDS.map((field) => (
                    <th key={field}>{t(`board.rules.classFields.${field}`)}</th>
                  ))}
                  <th>{t('board.rules.nets')}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {classes.map((cls, index) => (
                  <tr key={`${cls.name}-${index.toString()}`}>
                    <td>
                      <TextInput
                        bare
                        label={t('board.rules.className')}
                        value={cls.name}
                        disabled={index === 0}
                        testId={`class-name-${index.toString()}`}
                        onSave={(name) => {
                          const next = [...classes];
                          next[index] = { ...cls, name };
                          patch({ netClasses: next });
                        }}
                      />
                    </td>
                    {CLASS_FIELDS.map((field) => (
                      <td key={field}>
                        <LengthInput
                          bare
                          label={t(`board.rules.classFields.${field}`)}
                          value={cls[field]}
                          units={units}
                          min={0}
                          testId={`class-${field}-${index.toString()}`}
                          onSave={(value) => {
                            const next = [...classes];
                            next[index] = { ...cls, [field]: value };
                            patch({ netClasses: next });
                          }}
                        />
                      </td>
                    ))}
                    <td
                      className="board-table__nets"
                      title={(reply.nets[cls.name] ?? []).join(', ')}
                    >
                      {(reply.nets[cls.name] ?? []).length}
                    </td>
                    <td>
                      {index > 0 && (
                        <button
                          type="button"
                          className="button button--small button--quiet"
                          aria-label={t('board.rules.remove')}
                          onClick={() => {
                            patch({ netClasses: classes.filter((_, i) => i !== index) });
                          }}
                        >
                          ×
                        </button>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <button
            type="button"
            className="button button--small"
            data-testid="class-add"
            onClick={() => {
              const base = classes[0];
              if (base !== undefined) {
                patch({
                  netClasses: [
                    ...classes,
                    { ...base, name: `Class${(classes.length + 1).toString()}` },
                  ],
                });
              }
            }}
          >
            {t('board.rules.addClass')}
          </button>
          <h3 className="dialog__section">{t('board.rules.assignments')}</h3>
          <p className="field__hint">{t('board.rules.assignmentsHint')}</p>
          {rules.assignments.map((assignment, index) => (
            <div key={`assignment-${index.toString()}`} className="board-form__row">
              <TextInput
                bare
                label={t('board.rules.pattern')}
                value={assignment.pattern}
                testId={`assignment-pattern-${index.toString()}`}
                onSave={(pattern) => {
                  const next = [...rules.assignments];
                  next[index] = { ...assignment, pattern };
                  patch({ assignments: next });
                }}
              />
              <select
                className="field__control"
                aria-label={t('board.rules.className')}
                value={assignment.class}
                data-testid={`assignment-class-${index.toString()}`}
                onChange={(event) => {
                  const next = [...rules.assignments];
                  next[index] = { ...assignment, class: event.target.value };
                  patch({ assignments: next });
                }}
              >
                {classes.map((cls) => (
                  <option key={cls.name} value={cls.name}>
                    {cls.name}
                  </option>
                ))}
              </select>
              <button
                type="button"
                className="button button--small button--quiet"
                aria-label={t('board.rules.remove')}
                onClick={() => {
                  patch({ assignments: rules.assignments.filter((_, i) => i !== index) });
                }}
              >
                ×
              </button>
            </div>
          ))}
          <button
            type="button"
            className="button button--small"
            data-testid="assignment-add"
            onClick={() => {
              patch({
                assignments: [
                  ...rules.assignments,
                  { pattern: '', class: classes.at(-1)?.name ?? 'Default' },
                ],
              });
            }}
          >
            {t('board.rules.addAssignment')}
          </button>
        </>
      ) : tab === 'constraints' ? (
        <div className="board-form board-form--grid">
          <p className="field__hint">{t('board.rules.constraintsHint')}</p>
          {CONSTRAINTS.map((key) => (
            <LengthInput
              key={key}
              label={t(`board.rules.constraints.${key}`)}
              value={rules.constraints[key]}
              units={units}
              min={0}
              testId={`constraint-${key}`}
              onSave={(value) => {
                patch({ constraints: { ...rules.constraints, [key]: value } });
              }}
            />
          ))}
        </div>
      ) : tab === 'custom' ? (
        <>
          <p className="field__hint">{t('board.rules.customHint')}</p>
          {rules.custom.map((rule, index) => (
            <fieldset
              key={`rule-${index.toString()}`}
              className="board-rule"
              data-testid={`rule-${index.toString()}`}
            >
              <div className="board-form__row">
                <TextInput
                  label={t('board.rules.ruleName')}
                  value={rule.name}
                  testId={`rule-name-${index.toString()}`}
                  onSave={(name) => {
                    const next = [...rules.custom];
                    next[index] = { ...rule, name };
                    patch({ custom: next });
                  }}
                />
                <CheckInput
                  label={t('board.rules.enabled')}
                  checked={rule.enabled !== false}
                  onSave={(enabled) => {
                    const next = [...rules.custom];
                    next[index] = { ...rule, enabled };
                    patch({ custom: next });
                  }}
                />
                <button
                  type="button"
                  className="button button--small button--quiet"
                  aria-label={t('board.rules.remove')}
                  onClick={() => {
                    patch({ custom: rules.custom.filter((_, i) => i !== index) });
                  }}
                >
                  ×
                </button>
              </div>
              <ConditionInput
                value={rule.condition ?? ''}
                index={index}
                onSave={(condition) => {
                  const next = [...rules.custom];
                  next[index] = { ...rule, condition };
                  patch({ custom: next });
                }}
              />
              <div className="board-form board-form--grid">
                {RULE_FIELDS.map((field) => {
                  const value = rule.constraints[field];
                  return (
                    <div key={field} className="board-rule__field">
                      <CheckInput
                        label={t(`board.rules.ruleFields.${field}`)}
                        checked={value !== undefined}
                        testId={`rule-${field}-${index.toString()}`}
                        onSave={(on) => {
                          const next = [...rules.custom];
                          const constraints: RuleConstraints = on
                            ? { ...rule.constraints, [field]: rules.constraints.minClearance }
                            : Object.fromEntries(
                                Object.entries(rule.constraints).filter(([k]) => k !== field),
                              );
                          next[index] = { ...rule, constraints };
                          patch({ custom: next });
                        }}
                      />
                      {value !== undefined && (
                        <LengthInput
                          bare
                          label={t(`board.rules.ruleFields.${field}`)}
                          value={value}
                          units={units}
                          min={0}
                          testId={`rule-value-${field}-${index.toString()}`}
                          onSave={(v) => {
                            const next = [...rules.custom];
                            next[index] = {
                              ...rule,
                              constraints: { ...rule.constraints, [field]: v },
                            };
                            patch({ custom: next });
                          }}
                        />
                      )}
                    </div>
                  );
                })}
              </div>
            </fieldset>
          ))}
          <button
            type="button"
            className="button button--small"
            data-testid="rule-add"
            onClick={() => {
              patch({
                custom: [
                  ...rules.custom,
                  {
                    name: t('board.rules.newRule', { n: rules.custom.length + 1 }),
                    condition: '',
                    constraints: {},
                  },
                ],
              });
            }}
          >
            {t('board.rules.addRule')}
          </button>
          <details className="board-rule__help">
            <summary>{t('board.rules.helpTitle')}</summary>
            <p>{t('board.rules.help')}</p>
          </details>
        </>
      ) : (
        <div className="board-table__scroll">
          <table className="board-table" data-testid="drc-checks">
            <tbody>
              {reply.checks.map((check) => (
                <tr key={check.code}>
                  <td>{t(`board.drc.checks.${check.code}` as MessageKey)}</td>
                  <td>
                    <select
                      className="field__control"
                      aria-label={t(`board.drc.checks.${check.code}` as MessageKey)}
                      value={severityOf(check.code, check.default)}
                      data-testid={`check-${check.code}`}
                      onChange={(event) => {
                        const value = event.target.value as Severity;
                        const next: Record<string, Severity> =
                          value === check.default
                            ? Object.fromEntries(
                                Object.entries(severities).filter(([k]) => k !== check.code),
                              )
                            : { ...severities, [check.code]: value };
                        patch({ drc: { severities: next } });
                      }}
                    >
                      {(['error', 'warning', 'ignore'] as const).map((s) => (
                        <option key={s} value={s}>
                          {t(`board.drc.severity.${s}`)}
                        </option>
                      ))}
                    </select>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </Modal>
  );
}
