/**
 * The panel of an output job: copies of the board on one sheet with rails,
 * tabs with mouse bites or V-score lines, fiducials and tooling holes. The
 * board outputs of the job (Gerber, drill, drill map, pick and place) are
 * made for the panel. The preview is drawn by the engine
 * (`outputs.panelPreview`).
 */
import { useEffect, useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import { formatLength, type Units } from '@shared/project';

import { LengthInput, NumberInput } from '../libedit/fields';
import { useAppState } from '../state';
import {
  PANEL_DEFAULTS,
  type PanelPreview,
  type PanelSettings,
  type Rails,
  type Separation,
} from './model';

const SEPARATIONS: readonly Separation[] = ['mouseBites', 'vScore'];
const RAILS: readonly Rails[] = ['topBottom', 'leftRight', 'all', 'none'];

const WARNINGS: Readonly<Record<string, MessageKey>> = {
  toolingHolesNeedRails: 'outputs.panel.warning.toolingHolesNeedRails',
  fiducialsNeedRails: 'outputs.panel.warning.fiducialsNeedRails',
  tabsOnBoundingBox: 'outputs.panel.warning.tabsOnBoundingBox',
  noOutline: 'outputs.panel.warning.noOutline',
  notRectangular: 'outputs.panel.warning.notRectangular',
};

/** The preview of a panel, drawn again when the settings change. */
function usePanelPreview(
  settings: PanelSettings | null,
  board: string | null,
): { readonly preview: PanelPreview | null; readonly failure: string | null } {
  const { engine } = useAppState();
  const [state, setState] = useState<{
    readonly preview: PanelPreview | null;
    readonly failure: string | null;
  }>({ preview: null, failure: null });
  // The settings and the board as one value, so that equal settings do not
  // draw the panel again.
  const key = settings === null ? null : JSON.stringify({ settings, board });
  useEffect(() => {
    if (key === null) {
      return undefined;
    }
    let cancelled = false;
    // Typing in a field changes the settings several times; draw the last.
    const timer = setTimeout(() => {
      const params = JSON.parse(key) as { settings: PanelSettings; board: string | null };
      engine
        .call<PanelPreview>('outputs.panelPreview', {
          settings: params.settings,
          ...(params.board === null ? {} : { board: params.board }),
        })
        .then(
          (preview) => {
            if (!cancelled) {
              setState({ preview, failure: null });
            }
          },
          (reason: unknown) => {
            if (!cancelled) {
              setState({
                preview: null,
                failure: reason instanceof Error ? reason.message : String(reason),
              });
            }
          },
        );
    }, 150);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [engine, key]);
  return state;
}

export function PanelForm({
  panel,
  boards,
  units,
  onChange,
}: {
  readonly panel: PanelSettings | null;
  readonly boards: readonly { readonly id: string; readonly name: string }[];
  readonly units: Units;
  readonly onChange: (panel: PanelSettings | null) => void;
}): ReactNode {
  const { t, snapshot } = useAppState();
  const [board, setBoard] = useState<string | null>(null);
  const shownBoard = boards.some((b) => b.id === board) ? board : null;
  const { preview, failure } = usePanelPreview(panel, shownBoard);
  const set = <K extends keyof PanelSettings>(key: K, value: PanelSettings[K]): void => {
    if (panel !== null) {
      onChange({ ...panel, [key]: value });
    }
  };
  const length = (mm: number): string => formatLength(mm * 1e6, units, snapshot.language);
  const bites = panel?.separation !== 'vScore';
  const size = preview?.size ?? null;
  const svg = preview?.svg ?? null;
  const notes =
    preview === null
      ? []
      : [
          ...(preview.problem === null ? [] : [{ ...preview.problem, error: true }]),
          ...preview.warnings.map((warning) => ({ ...warning, error: false })),
        ];

  return (
    <div className="outputs__form" data-testid="panel-settings">
      <p className="field__hint">{t('outputs.panel.hint')}</p>
      <label className="checkbox">
        <input
          type="checkbox"
          checked={panel !== null}
          data-testid="panel-enabled"
          onChange={(event) => {
            onChange(event.target.checked ? PANEL_DEFAULTS : null);
          }}
        />
        <span>{t('outputs.panel.enabled')}</span>
      </label>
      {panel !== null && (
        <>
          <div className="panel-form__grid">
            <NumberInput
              label={t('outputs.panel.columns')}
              value={panel.columns}
              integer
              min={1}
              max={50}
              testId="panel-columns"
              onSave={(value) => {
                set('columns', value);
              }}
            />
            <NumberInput
              label={t('outputs.panel.rows')}
              value={panel.rows}
              integer
              min={1}
              max={50}
              testId="panel-rows"
              onSave={(value) => {
                set('rows', value);
              }}
            />
          </div>
          <fieldset className="field field--group">
            <legend className="field__label">{t('outputs.panel.separation')}</legend>
            <div className="choice-grid choice-grid--two" role="radiogroup">
              {SEPARATIONS.map((separation) => (
                <label
                  key={separation}
                  className="choice"
                  data-selected={panel.separation === separation}
                  data-testid={`panel-separation-${separation}`}
                >
                  <input
                    type="radio"
                    name="panel-separation"
                    checked={panel.separation === separation}
                    onChange={() => {
                      set('separation', separation);
                    }}
                  />
                  <span className="choice__title">{t(`outputs.panel.${separation}`)}</span>
                  <span className="choice__hint">{t(`outputs.panel.${separation}Hint`)}</span>
                </label>
              ))}
            </div>
          </fieldset>
          {bites && (
            <div className="panel-form__grid">
              <LengthInput
                label={t('outputs.panel.gap')}
                value={panel.gap}
                units={units}
                min={0}
                testId="panel-gap"
                onSave={(value) => {
                  set('gap', value);
                }}
              />
              <NumberInput
                label={t('outputs.panel.tabs')}
                value={panel.tabs}
                integer
                min={1}
                max={10}
                testId="panel-tabs"
                onSave={(value) => {
                  set('tabs', value);
                }}
              />
              <LengthInput
                label={t('outputs.panel.tabWidth')}
                value={panel.tabWidth}
                units={units}
                min={1}
                testId="panel-tab-width"
                onSave={(value) => {
                  set('tabWidth', value);
                }}
              />
            </div>
          )}
          <div className="panel-form__grid">
            <label className="field field--compact">
              <span className="field__label">{t('outputs.panel.rails')}</span>
              <select
                className="field__control"
                value={panel.rails}
                data-testid="panel-rails"
                onChange={(event) => {
                  set('rails', event.target.value as Rails);
                }}
              >
                {RAILS.map((rails) => (
                  <option key={rails} value={rails}>
                    {t(`outputs.panel.railSides.${rails}`)}
                  </option>
                ))}
              </select>
            </label>
            {panel.rails !== 'none' && (
              <LengthInput
                label={t('outputs.panel.railWidth')}
                value={panel.railWidth}
                units={units}
                min={0}
                testId="panel-rail-width"
                onSave={(value) => {
                  set('railWidth', value);
                }}
              />
            )}
          </div>
          <label className="checkbox">
            <input
              type="checkbox"
              checked={panel.fiducials}
              data-testid="panel-fiducials"
              onChange={(event) => {
                set('fiducials', event.target.checked);
              }}
            />
            <span>{t('outputs.panel.fiducials')}</span>
          </label>
          <label className="checkbox">
            <input
              type="checkbox"
              checked={panel.toolingHoles}
              data-testid="panel-tooling-holes"
              onChange={(event) => {
                set('toolingHoles', event.target.checked);
              }}
            />
            <span>{t('outputs.panel.toolingHoles')}</span>
          </label>
          <section className="panel-form__preview" data-testid="panel-preview">
            <div className="panel-form__preview-head">
              <h4 className="outputs__subtitle">{t('outputs.panel.preview')}</h4>
              {boards.length > 1 && (
                <select
                  className="field__control"
                  value={shownBoard ?? ''}
                  aria-label={t('outputs.board')}
                  data-testid="panel-board"
                  onChange={(event) => {
                    setBoard(event.target.value === '' ? null : event.target.value);
                  }}
                >
                  <option value="">{t('outputs.firstBoard')}</option>
                  {boards.map((b) => (
                    <option key={b.id} value={b.id}>
                      {b.name}
                    </option>
                  ))}
                </select>
              )}
            </div>
            {size !== null && (
              <p className="panel-form__size" data-testid="panel-size">
                {t('outputs.panel.size', {
                  width: length(size[0]),
                  height: length(size[1]),
                  boards: preview?.boards ?? 0,
                })}
              </p>
            )}
            {notes.map((note) => {
              const message = WARNINGS[note.code];
              return (
                <p
                  key={note.code}
                  className={`panel-form__warning${note.error ? ' panel-form__warning--error' : ''}`}
                  data-testid="panel-warning"
                  data-code={note.code}
                >
                  {message === undefined ? note.message : t(message)}
                </p>
              );
            })}
            {failure !== null && (
              <p className="panel-form__warning panel-form__warning--error">{failure}</p>
            )}
            {svg !== null && (
              <img
                className="panel-form__image"
                alt={t('outputs.panel.preview')}
                data-testid="panel-image"
                src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`}
              />
            )}
          </section>
        </>
      )}
    </div>
  );
}
