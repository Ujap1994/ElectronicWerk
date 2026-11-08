import { useSyncExternalStore, type ReactNode } from 'react';

import type { ViewerController } from '../viewer/controller';
import { layerLabel } from '../viewer/labels';
import { useAppState } from '../state';
import { Icon } from './Icon';

/** Layers of the open document: visibility, active layer and opacity. */
export function LayerPanel({ controller }: { readonly controller: ViewerController }): ReactNode {
  const { t } = useAppState();
  const snapshot = useSyncExternalStore(controller.subscribe, controller.snapshot);
  const { layers, active } = snapshot;
  if (layers.length === 0) {
    return null;
  }
  const board = snapshot.kind === 'pcb';
  const activeLayer = active === null ? undefined : layers[active];
  return (
    <section className="viewer-panel" aria-label={board ? t('viewer.layers') : t('viewer.display')}>
      <header className="viewer-panel__header">
        <Icon name="layers" size={16} />
        <h2 className="viewer-panel__title">{board ? t('viewer.layers') : t('viewer.display')}</h2>
        <div className="viewer-panel__actions">
          <button
            type="button"
            className="button button--small"
            data-testid="layers-show-all"
            onClick={() => {
              controller.setAllVisible(true);
            }}
          >
            {t('viewer.showAll')}
          </button>
          {board && (
            <button
              type="button"
              className="button button--small"
              data-testid="layers-only-active"
              onClick={() => {
                controller.setAllVisible(false);
              }}
            >
              {t('viewer.onlyActive')}
            </button>
          )}
        </div>
      </header>
      <ul className="layer-list" data-testid="layer-list">
        {layers.map((layer) => {
          const name = layerLabel(t, layer.key, layer.name);
          const isActive = layer.index === active;
          return (
            <li
              key={layer.key}
              className="layer-list__row"
              data-active={isActive}
              data-visible={layer.visible}
              data-empty={layer.empty}
              data-testid={`layer-${layer.key}`}
            >
              <button
                type="button"
                className="layer-list__eye"
                aria-pressed={layer.visible}
                aria-label={t('viewer.toggleLayer', { name })}
                title={t('viewer.toggleLayer', { name })}
                data-testid={`layer-toggle-${layer.key}`}
                onClick={() => {
                  controller.setLayerVisible(layer.index, !layer.visible);
                }}
              >
                <Icon name={layer.visible ? 'eye' : 'eyeOff'} size={16} />
              </button>
              <span
                className="layer-list__swatch"
                style={{ backgroundColor: layer.color, opacity: layer.visible ? 1 : 0.35 }}
                aria-hidden="true"
              />
              <button
                type="button"
                className="layer-list__name"
                data-empty={layer.empty}
                aria-pressed={isActive}
                title={t('viewer.activate', { name })}
                data-testid={`layer-activate-${layer.key}`}
                onClick={() => {
                  controller.setActive(isActive ? null : layer.index);
                }}
              >
                {name}
              </button>
              {isActive && <span className="layer-list__badge">{t('viewer.active')}</span>}
              {!isActive && layer.empty && (
                <span className="layer-list__badge layer-list__badge--muted">
                  {t('viewer.emptyLayer')}
                </span>
              )}
            </li>
          );
        })}
      </ul>
      {activeLayer !== undefined && (
        <label className="viewer-panel__opacity">
          <span>
            {t('viewer.opacity', { name: layerLabel(t, activeLayer.key, activeLayer.name) })}
          </span>
          <input
            type="range"
            min={5}
            max={100}
            step={5}
            value={Math.round(activeLayer.opacity * 100)}
            data-testid="layer-opacity"
            onChange={(event) => {
              controller.setLayerOpacity(activeLayer.index, Number(event.target.value) / 100);
            }}
          />
          <output>{Math.round(activeLayer.opacity * 100)}&#8239;%</output>
        </label>
      )}
    </section>
  );
}
