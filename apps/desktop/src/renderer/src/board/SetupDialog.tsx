/**
 * Board setup: presets for common board classes, the layer stack-up and a
 * rectangular board outline.
 */
import { useEffect, useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import type { Units } from '@shared/project';

import { Modal } from '../components/Modal';
import { LengthInput, NumberInput, SelectInput, TextInput } from '../libedit/fields';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { layerName } from './BoardToolbar';
import { standardStackup, thicknessOf, type Stackup, type StackupLayer } from './stackup';

interface Preset {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly layers: number;
  readonly thickness: number;
}

interface BoardProps {
  name: string;
  stackup: Stackup;
  origin: [number, number];
  drcExclusions: string[];
}

interface ShapeJson {
  id: string;
  layer: string;
  geometry: Record<string, Record<string, unknown>>;
}

type Tab = 'presets' | 'stackup' | 'outline';

/** The board setup dialog. */
export function SetupDialog({
  doc,
  units,
  onClose,
}: {
  readonly doc: string;
  readonly units: Units;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine, snapshot } = useAppState();
  const { notify } = useProject();
  const [tab, setTab] = useState<Tab>('presets');
  const [presets, setPresets] = useState<readonly Preset[]>([]);
  const [props, setProps] = useState<BoardProps | null>(null);
  const [outline, setOutline] = useState<{ shapes: ShapeJson[]; width: number; height: number }>({
    shapes: [],
    width: 100_000_000,
    height: 80_000_000,
  });
  const fail = (error: unknown): void => {
    notify(errorMessage(t, error), 'error');
  };

  useEffect(() => {
    const report = (error: unknown): void => {
      notify(errorMessage(t, error), 'error');
    };
    let cancelled = false;
    Promise.all([
      engine.call<Preset[]>('pcb.presets'),
      engine.call<BoardProps>('document.properties', { doc }),
      engine.call<{ shapes?: ShapeJson[] }>('document.get', { doc }),
    ])
      .then(([list, loaded, board]) => {
        if (cancelled) {
          return;
        }
        setPresets(list);
        setProps(loaded);
        const shapes = (board.shapes ?? []).filter((s) => s.layer === 'outline');
        const rect = shapes.length === 1 ? shapes[0]?.geometry['rect'] : undefined;
        if (rect !== undefined) {
          const [x0, y0] = rect['start'] as [number, number];
          const [x1, y1] = rect['end'] as [number, number];
          setOutline({ shapes, width: Math.abs(x1 - x0), height: Math.abs(y1 - y0) });
        } else {
          setOutline((current) => ({ ...current, shapes }));
        }
      })
      .catch(report);
    return () => {
      cancelled = true;
    };
  }, [engine, doc, notify, t]);

  const applyPreset = (preset: Preset): void => {
    engine.call('pcb.applyPreset', { doc, preset: preset.id }).then(onClose).catch(fail);
  };
  const saveStackup = (): void => {
    if (props === null) {
      return;
    }
    engine
      .call('document.setProperties', { doc, props: { stackup: props.stackup } })
      .then(onClose)
      .catch(fail);
  };
  const saveOutline = (): void => {
    engine
      .call('edit.apply', {
        label: 'board-outline',
        changes: [
          ...outline.shapes.map((shape) => ({ op: 'delete', doc, id: shape.id })),
          {
            op: 'insert',
            doc,
            object: {
              type: 'boardShape',
              id: crypto.randomUUID(),
              layer: 'outline',
              geometry: { rect: { start: [0, 0], end: [outline.width, outline.height] } },
              width: 100_000,
            },
          },
        ],
      })
      .then(onClose)
      .catch(fail);
  };

  const stackup = props?.stackup ?? null;
  const copperCount = stackup?.layers.filter((l) => l.type === 'copper').length ?? 2;
  const mm = (nm: number): string =>
    new Intl.NumberFormat(snapshot.language, { maximumFractionDigits: 3 }).format(nm / 1e6);
  const setLayer = (index: number, layer: StackupLayer): void => {
    if (props === null || stackup === null) {
      return;
    }
    const layers = [...stackup.layers];
    layers[index] = layer;
    setProps({ ...props, stackup: { ...stackup, layers } });
  };

  return (
    <Modal
      title={t('board.setup.title')}
      onClose={onClose}
      testId="setup-dialog"
      className="dialog--wide"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('board.cancel')}
          </button>
          {tab === 'stackup' && (
            <button
              type="button"
              className="button button--primary"
              disabled={props === null}
              data-testid="stackup-save"
              onClick={saveStackup}
            >
              {t('board.setup.saveStackup')}
            </button>
          )}
          {tab === 'outline' && (
            <button
              type="button"
              className="button button--primary"
              data-testid="outline-save"
              onClick={saveOutline}
            >
              {t('board.setup.saveOutline')}
            </button>
          )}
        </>
      }
    >
      <div className="segmented" role="tablist">
        {(['presets', 'stackup', 'outline'] as const).map((value) => (
          <button
            key={value}
            type="button"
            role="tab"
            aria-selected={tab === value}
            className={
              tab === value ? 'segmented__item segmented__item--active' : 'segmented__item'
            }
            data-testid={`setup-tab-${value}`}
            onClick={() => {
              setTab(value);
            }}
          >
            {t(`board.setup.tabs.${value}`)}
          </button>
        ))}
      </div>
      {tab === 'presets' && (
        <>
          <p className="field__hint">{t('board.setup.presetsHint')}</p>
          <ul className="board-presets" data-testid="presets">
            {presets.map((preset) => (
              <li key={preset.id}>
                <button
                  type="button"
                  className="board-presets__item"
                  data-testid={`preset-${preset.id}`}
                  onClick={() => {
                    applyPreset(preset);
                  }}
                >
                  <strong>{t(`board.presets.${preset.id}.name` as MessageKey)}</strong>
                  <span>{t(`board.presets.${preset.id}.description` as MessageKey)}</span>
                  <span className="board-presets__meta">
                    {t('board.setup.presetMeta', {
                      layers: preset.layers,
                      thickness: mm(preset.thickness),
                    })}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
      {tab === 'stackup' && props !== null && stackup !== null && (
        <>
          <div className="board-form__row">
            <SelectInput
              label={t('board.setup.copperLayers')}
              value={copperCount.toString()}
              testId="stackup-count"
              options={['1', '2', '4', '6', '8', '10'].map((n) => [n, n])}
              onSave={(value) => {
                setProps({
                  ...props,
                  stackup: standardStackup(Number(value), thicknessOf(stackup), stackup.finish),
                });
              }}
            />
            <LengthInput
              label={t('board.setup.thickness')}
              value={thicknessOf(stackup)}
              units={units}
              min={100_000}
              testId="stackup-thickness"
              onSave={(value) => {
                setProps({
                  ...props,
                  stackup: standardStackup(copperCount, value, stackup.finish),
                });
              }}
            />
            <TextInput
              label={t('board.setup.finish')}
              value={stackup.finish ?? ''}
              onSave={(finish) => {
                setProps({ ...props, stackup: { ...stackup, finish } });
              }}
            />
          </div>
          <div className="board-table__scroll">
            <table className="board-table" data-testid="stackup-table">
              <thead>
                <tr>
                  <th>{t('board.setup.layer')}</th>
                  <th>{t('board.setup.material')}</th>
                  <th>{t('board.setup.layerThickness')}</th>
                  <th>{t('board.setup.epsilon')}</th>
                </tr>
              </thead>
              <tbody>
                {stackup.layers.map((layer, index) => (
                  // eslint-disable-next-line @eslint-react/no-array-index-key -- the stack is positional
                  <tr key={index} data-type={layer.type}>
                    <td>
                      {layer.type === 'copper'
                        ? layerName(t, layer.layer)
                        : layer.type === 'dielectric'
                          ? t(`board.setup.dielectric.${layer.kind}`)
                          : t(`board.setup.kinds.${layer.type}`, {
                              side: t(`board.setup.sides.${layer.side}` as MessageKey),
                            })}
                    </td>
                    <td>
                      {layer.type === 'dielectric' ? (
                        <TextInput
                          bare
                          label={t('board.setup.material')}
                          value={layer.material}
                          onSave={(material) => {
                            setLayer(index, { ...layer, material });
                          }}
                        />
                      ) : layer.type === 'copper' ? (
                        t('board.setup.copper')
                      ) : (
                        (layer.color ?? '')
                      )}
                    </td>
                    <td>
                      {layer.type === 'silkscreen' ? (
                        '–'
                      ) : (
                        <LengthInput
                          bare
                          label={t('board.setup.layerThickness')}
                          value={layer.thickness}
                          units={units}
                          min={0}
                          testId={`stackup-thickness-${index.toString()}`}
                          onSave={(thickness) => {
                            setLayer(index, { ...layer, thickness });
                          }}
                        />
                      )}
                    </td>
                    <td>
                      {layer.type === 'dielectric' ? (
                        <NumberInput
                          bare
                          label={t('board.setup.epsilon')}
                          value={layer.epsilonR}
                          min={1}
                          onSave={(epsilonR) => {
                            setLayer(index, { ...layer, epsilonR });
                          }}
                        />
                      ) : (
                        ''
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="field__hint">
            {t('board.setup.total', { thickness: mm(thicknessOf(stackup)) })}
          </p>
        </>
      )}
      {tab === 'outline' && (
        <div className="board-form">
          <p className="field__hint">{t('board.setup.outlineHint')}</p>
          <LengthInput
            label={t('board.setup.width')}
            value={outline.width}
            units={units}
            min={1_000_000}
            testId="outline-width"
            onSave={(width) => {
              setOutline({ ...outline, width });
            }}
          />
          <LengthInput
            label={t('board.setup.height')}
            value={outline.height}
            units={units}
            min={1_000_000}
            testId="outline-height"
            onSave={(height) => {
              setOutline({ ...outline, height });
            }}
          />
          {outline.shapes.length > 0 && (
            <p className="field__hint">
              {t('board.setup.replaces', { count: outline.shapes.length })}
            </p>
          )}
        </div>
      )}
    </Modal>
  );
}
