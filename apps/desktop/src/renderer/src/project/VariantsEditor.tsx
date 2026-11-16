/**
 * Assembly variants of the project: the same board with some parts left
 * out, added or changed. Each variant lists only the parts that differ from
 * the design; output jobs are made for a variant.
 */
import { useEffect, useState, type ReactNode } from 'react';

import type { ProjectSnapshot, Variant, VariantPart } from '@shared/project';

import { TextInput } from '../libedit/fields';
import { useAppState } from '../state';
import { useProject } from './ProjectProvider';

interface DesignPart {
  readonly designator: string;
  readonly value: string;
  readonly dnp: boolean;
}

/** One part per row from the bill of materials preview. */
function useDesignParts(): DesignPart[] | null {
  const { engine } = useAppState();
  const [parts, setParts] = useState<DesignPart[] | null>(null);
  useEffect(() => {
    let cancelled = false;
    engine
      .call<{ rows: (string | number)[][] }>('outputs.bom', {
        options: {
          columns: [{ field: 'designators' }, { field: 'value' }, { field: 'dnp' }],
          groupBy: [],
          includeDnp: true,
        },
      })
      .then((reply) => {
        if (!cancelled) {
          setParts(
            reply.rows.map(([designator, value, dnp]) => ({
              designator: String(designator ?? ''),
              value: String(value ?? ''),
              dnp: dnp === 'DNP',
            })),
          );
        }
      })
      .catch(() => {
        if (!cancelled) {
          setParts([]);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [engine]);
  return parts;
}

type Fitting = '' | 'fitted' | 'left';

/** A change of one part; `undefined` takes the design's own again. */
interface PartPatch {
  readonly fitted?: boolean | undefined;
  readonly value?: string | undefined;
}

function fitting(part: VariantPart | undefined): Fitting {
  if (part?.fitted === true) {
    return 'fitted';
  }
  return part?.fitted === false ? 'left' : '';
}

/** A part without changes is dropped from the variant. */
function cleaned(part: VariantPart): VariantPart | null {
  const empty =
    part.fitted === undefined &&
    (part.value === undefined || part.value === '') &&
    Object.keys(part.properties ?? {}).length === 0;
  return empty ? null : part;
}

export function VariantsEditor({ project }: { readonly project: ProjectSnapshot }): ReactNode {
  const { t } = useAppState();
  const { setProperties } = useProject();
  // Edits apply here at once, so quick edits build on each other before
  // the project reports them back.
  const saved = project.variants;
  const [variants, setVariants] = useState<readonly Variant[]>(saved ?? []);
  const [seen, setSeen] = useState(saved);
  if (seen !== saved) {
    setSeen(saved);
    setVariants(saved ?? []);
  }
  const [open, setOpen] = useState<number | null>(null);
  const parts = useDesignParts();

  const save = (next: readonly Variant[]): void => {
    setVariants(next);
    void setProperties({ variants: next });
  };
  const change = (name: string, update: (variant: Variant) => Variant): void => {
    save(variants.map((v) => (v.name === name ? update(v) : v)));
  };
  const setPart = (name: string, designator: string, patch: PartPatch): void => {
    change(name, (variant) => {
      const current: VariantPart = variant.parts?.[designator] ?? {};
      const merged = { ...current, ...patch };
      const kept = cleaned(
        Object.fromEntries(Object.entries(merged).filter(([, value]) => value !== undefined)),
      );
      const others = Object.entries(variant.parts ?? {}).filter(([key]) => key !== designator);
      const parts: Record<string, VariantPart> = Object.fromEntries(
        kept === null ? others : [...others, [designator, kept]],
      );
      return { ...variant, parts };
    });
  };

  return (
    <section className="panel" data-testid="variants">
      <h3 className="panel__title">{t('variants.title')}</h3>
      <p className="field__hint">{t('variants.hint')}</p>
      {variants.map((variant, index) => (
        // eslint-disable-next-line @eslint-react/no-array-index-key -- a renamed variant keeps its row
        <div key={index} className="variant" data-testid={`variant-${index.toString()}`}>
          <div className="board-form__row">
            <TextInput
              bare
              label={t('variants.name')}
              value={variant.name}
              testId={`variant-name-${index.toString()}`}
              onSave={(name) => {
                const trimmed = name.trim();
                if (trimmed !== '' && !variants.some((v) => v.name === trimmed)) {
                  change(variant.name, (v) => ({ ...v, name: trimmed }));
                }
              }}
            />
            <TextInput
              bare
              label={t('variants.description')}
              value={variant.description ?? ''}
              placeholder={t('variants.description')}
              testId={`variant-description-${index.toString()}`}
              onSave={(description) => {
                change(variant.name, (v) => ({ ...v, description }));
              }}
            />
            <span className="field__hint">
              {t('variants.changes', { count: Object.keys(variant.parts ?? {}).length })}
            </span>
            <button
              type="button"
              className="button button--small"
              data-testid={`variant-edit-${index.toString()}`}
              onClick={() => {
                setOpen(open === index ? null : index);
              }}
            >
              {open === index ? t('variants.close') : t('variants.edit')}
            </button>
            <button
              type="button"
              className="button button--small button--quiet"
              aria-label={t('variants.remove')}
              data-testid={`variant-remove-${index.toString()}`}
              onClick={() => {
                save(variants.filter((v) => v.name !== variant.name));
                setOpen(null);
              }}
            >
              ×
            </button>
          </div>
          {open === index && (
            <div className="board-table__scroll">
              {parts === null ? (
                <p className="field__hint">{t('variants.loading')}</p>
              ) : (
                <table className="board-table" data-testid={`variant-parts-${index.toString()}`}>
                  <thead>
                    <tr>
                      <th>{t('variants.designator')}</th>
                      <th>{t('variants.designValue')}</th>
                      <th>{t('variants.fitting')}</th>
                      <th>{t('variants.value')}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {parts.map((part) => {
                      const own = variant.parts?.[part.designator];
                      return (
                        <tr key={part.designator}>
                          <td>{part.designator}</td>
                          <td>
                            {part.value}
                            {part.dnp ? ` · ${t('variants.dnp')}` : ''}
                          </td>
                          <td>
                            <select
                              className="field__control"
                              aria-label={t('variants.fitting')}
                              value={fitting(own)}
                              data-testid={`variant-fitting-${part.designator}`}
                              onChange={(event) => {
                                const value = event.target.value as Fitting;
                                setPart(variant.name, part.designator, {
                                  fitted: value === '' ? undefined : value === 'fitted',
                                });
                              }}
                            >
                              <option value="">{t('variants.asDesigned')}</option>
                              <option value="fitted">{t('variants.fitted')}</option>
                              <option value="left">{t('variants.left')}</option>
                            </select>
                          </td>
                          <td>
                            <TextInput
                              bare
                              label={t('variants.value')}
                              value={own?.value ?? ''}
                              placeholder={part.value}
                              testId={`variant-value-${part.designator}`}
                              onSave={(value) => {
                                setPart(variant.name, part.designator, {
                                  value: value.trim() === '' ? undefined : value.trim(),
                                });
                              }}
                            />
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              )}
            </div>
          )}
        </div>
      ))}
      <button
        type="button"
        className="button button--small board-add"
        data-testid="variant-add"
        onClick={() => {
          let n = variants.length + 1;
          while (variants.some((v) => v.name === t('variants.newName', { n }))) {
            n += 1;
          }
          const name = t('variants.newName', { n });
          save([...variants, { name }]);
          setOpen(variants.length);
        }}
      >
        {t('variants.add')}
      </button>
    </section>
  );
}
