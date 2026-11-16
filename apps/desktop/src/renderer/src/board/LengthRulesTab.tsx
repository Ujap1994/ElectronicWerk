/**
 * The lengths tab of the design rules: the largest skew of the
 * differential pairs of every net class, and length groups (nets whose
 * routed lengths must match within a tolerance).
 */
import type { ReactNode } from 'react';

import type { Units } from '@shared/project';

import { LengthInput, TextInput } from '../libedit/fields';
import { useAppState } from '../state';
import type { LengthGroup, NetClass, RulesProps } from './rules-types';

export function LengthRulesTab({
  units,
  rules,
  update,
}: {
  readonly units: Units;
  readonly rules: RulesProps;
  readonly update: (change: (rules: RulesProps) => RulesProps) => void;
}): ReactNode {
  const { t } = useAppState();
  const groups = rules.lengthGroups ?? [];
  const setGroup = (index: number, next: Partial<LengthGroup>): void => {
    update((current) => ({
      ...current,
      lengthGroups: (current.lengthGroups ?? []).map((g, i) =>
        i === index ? { ...g, ...next } : g,
      ),
    }));
  };
  return (
    <div data-testid="length-rules">
      <h3 className="dialog__section">{t('board.rules.lengths.skewTitle')}</h3>
      <p className="field__hint">{t('board.rules.lengths.skewHint')}</p>
      <div className="board-table__scroll">
        <table className="board-table" data-testid="skew-classes">
          <thead>
            <tr>
              <th>{t('board.rules.className')}</th>
              <th>{t('board.rules.lengths.maxSkew')}</th>
            </tr>
          </thead>
          <tbody>
            {rules.netClasses.map((cls, index) => (
              <tr key={cls.name}>
                <td>{cls.name}</td>
                <td>
                  <LengthInput
                    bare
                    label={t('board.rules.lengths.maxSkew')}
                    value={cls.maxSkew ?? 0}
                    units={units}
                    min={0}
                    testId={`class-maxSkew-${index.toString()}`}
                    onSave={(maxSkew) => {
                      update((current) => ({
                        ...current,
                        netClasses: current.netClasses.map((c) => {
                          if (c.name !== cls.name) {
                            return c;
                          }
                          const next: NetClass = { ...c, maxSkew };
                          if (maxSkew === 0) {
                            delete next.maxSkew;
                          }
                          return next;
                        }),
                      }));
                    }}
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <h3 className="dialog__section">{t('board.rules.lengths.groupsTitle')}</h3>
      <p className="field__hint">{t('board.rules.lengths.groupsHint')}</p>
      {groups.length > 0 && (
        <div className="board-table__scroll">
          <table className="board-table" data-testid="length-groups">
            <thead>
              <tr>
                <th>{t('board.rules.lengths.name')}</th>
                <th>{t('board.rules.lengths.nets')}</th>
                <th>{t('board.rules.lengths.tolerance')}</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {groups.map((group, index) => (
                <tr key={`group-${index.toString()}`}>
                  <td>
                    <TextInput
                      bare
                      label={t('board.rules.lengths.name')}
                      value={group.name}
                      testId={`group-name-${index.toString()}`}
                      onSave={(name) => {
                        setGroup(index, { name });
                      }}
                    />
                  </td>
                  <td>
                    <TextInput
                      bare
                      label={t('board.rules.lengths.nets')}
                      value={group.nets}
                      placeholder={t('board.rules.lengths.netsExample')}
                      testId={`group-nets-${index.toString()}`}
                      onSave={(nets) => {
                        setGroup(index, { nets });
                      }}
                    />
                  </td>
                  <td>
                    <LengthInput
                      bare
                      label={t('board.rules.lengths.tolerance')}
                      value={group.tolerance}
                      units={units}
                      min={0}
                      testId={`group-tolerance-${index.toString()}`}
                      onSave={(tolerance) => {
                        setGroup(index, { tolerance });
                      }}
                    />
                  </td>
                  <td>
                    <button
                      type="button"
                      className="button button--small button--quiet"
                      aria-label={t('board.rules.remove')}
                      data-testid={`group-remove-${index.toString()}`}
                      onClick={() => {
                        update((current) => ({
                          ...current,
                          lengthGroups: (current.lengthGroups ?? []).filter((_, i) => i !== index),
                        }));
                      }}
                    >
                      ×
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <button
        type="button"
        className="button button--small board-add"
        data-testid="group-add"
        onClick={() => {
          update((current) => {
            const list = current.lengthGroups ?? [];
            return {
              ...current,
              lengthGroups: [
                ...list,
                {
                  name: t('board.rules.lengths.newGroup', { n: list.length + 1 }),
                  nets: '',
                  tolerance: 500_000,
                },
              ],
            };
          });
        }}
      >
        {t('board.rules.lengths.addGroup')}
      </button>
    </div>
  );
}
