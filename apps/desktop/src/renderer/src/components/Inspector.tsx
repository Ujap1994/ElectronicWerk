import { useEffect, useState, useSyncExternalStore, type ReactNode } from 'react';

import type { Units } from '@shared/project';

import { BOARD_EDITABLE, BoardProperties, MAX_EDITED } from '../board/BoardProperties';
import { SelectSimilar } from './SelectSimilar';
import { GroupActions } from '../board/GroupActions';
import { NetColor } from './NetColor';
import { EDITABLE_TYPES, PropertyEditor } from '../schematic/PropertyEditor';
import { useAppState } from '../state';
import type { SelectedObject, ViewerController } from '../viewer/controller';
import { fieldLabel, formatRow, typeLabel } from '../viewer/labels';
import type { ObjectInfo } from '../viewer/types';

interface Loaded {
  readonly key: string;
  readonly version: number;
  readonly info: ObjectInfo | null;
}

/** Read-only properties of the selected object. */
export function Inspector({
  controller,
  doc,
  units,
  editable = false,
  boardLayers = null,
}: {
  readonly controller: ViewerController;
  readonly doc: string;
  readonly units: Units;
  /** Schematic objects can be edited here. */
  readonly editable?: boolean;
  /** Board objects can be edited here (the copper layers of the board). */
  readonly boardLayers?: readonly string[] | null;
}): ReactNode {
  const { t, engine, snapshot: settings } = useAppState();
  const snapshot = useSyncExternalStore(controller.subscribe, controller.snapshot);
  const { selection, version } = snapshot;
  const single: SelectedObject | undefined = selection.length === 1 ? selection[0] : undefined;
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const key = single?.key ?? null;
  const id = single?.id ?? null;
  const childKind = single?.child?.kind ?? null;
  const childIndex = single?.child?.index ?? null;

  useEffect(() => {
    if (key === null || id === null) {
      return undefined;
    }
    let cancelled = false;
    const params =
      childKind === null || childIndex === null
        ? { doc, id }
        : { doc, id, child: { kind: childKind, index: childIndex } };
    engine
      .call<ObjectInfo>('view.object', params)
      .then(
        (info) => info,
        () => null,
      )
      .then((info) => {
        if (!cancelled) {
          setLoaded({ key, version, info });
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [engine, doc, key, id, childKind, childIndex, version]);

  const ctx = { t, units, locale: settings.language };
  // Layers that drawings and dimensions can go on.
  const drawingLayers = (controller.currentScene?.meta.layers ?? [])
    .map((layer) => layer.key)
    .filter((key) => /^(user|mech|fab|silk)\./u.test(key));
  let body: ReactNode;
  if (selection.length === 0) {
    body = <p className="viewer-panel__hint">{t('inspector.empty')}</p>;
  } else if (single === undefined) {
    // Several objects of one type (whole objects, not pads) are edited together.
    const first = selection[0];
    const sameType =
      first !== undefined &&
      selection.every((entry) => entry.type === first.type && entry.child === null)
        ? first.type
        : null;
    const counts = new Map<string, number>();
    for (const entry of selection) {
      counts.set(entry.type, (counts.get(entry.type) ?? 0) + 1);
    }
    body = (
      <>
        <p className="inspector__title" data-testid="inspector-title">
          {t('inspector.multiple', { count: selection.length })}
        </p>
        <dl className="inspector__rows">
          {[...counts.entries()].map(([type, count]) => (
            <div key={type} className="inspector__row">
              <dt className="inspector__label">{typeLabel(t, type)}</dt>
              <dd className="inspector__value">{count}</dd>
            </div>
          ))}
        </dl>
        {boardLayers !== null && (
          <GroupActions
            doc={doc}
            ids={selection.filter((entry) => entry.child === null).map((entry) => entry.id)}
            controller={controller}
          />
        )}
        {boardLayers !== null &&
          sameType !== null &&
          BOARD_EDITABLE.has(sameType) &&
          selection.length <= MAX_EDITED && (
            <BoardProperties
              doc={doc}
              ids={selection.map((entry) => entry.id)}
              type={sameType}
              version={version}
              units={units}
              layers={boardLayers}
              drawingLayers={drawingLayers}
            />
          )}
      </>
    );
  } else if (loaded?.key !== single.key) {
    body = <p className="viewer-panel__hint">{t('inspector.loading')}</p>;
  } else if (loaded.info === null) {
    body = <p className="viewer-panel__hint">{t('inspector.missing')}</p>;
  } else {
    const info = loaded.info;
    body = (
      <>
        <p className="inspector__title" data-testid="inspector-title">
          <span className="inspector__type">{typeLabel(t, info.type)}</span>
          {info.title !== '' && <span className="inspector__name">{info.title}</span>}
        </p>
        {editable && single.child === null && EDITABLE_TYPES.has(single.type) && (
          <PropertyEditor key={single.id} doc={doc} id={single.id} version={version} />
        )}
        {boardLayers !== null && single.child === null && BOARD_EDITABLE.has(single.type) && (
          <BoardProperties
            key={single.id}
            doc={doc}
            ids={[single.id]}
            type={single.type}
            version={version}
            units={units}
            layers={boardLayers}
            drawingLayers={drawingLayers}
          />
        )}
        {single.child === null && (
          <SelectSimilar doc={doc} id={single.id} units={units} controller={controller} />
        )}
        {boardLayers !== null && single.child === null && (
          <GroupActions doc={doc} ids={[single.id]} controller={controller} />
        )}
        <NetColor
          doc={doc}
          id={single.id}
          type={single.type}
          net={snapshot.kind === 'pcb' ? single.net : undefined}
        />
        <dl className="inspector__rows" data-testid="inspector-rows">
          {info.rows.map((row) => (
            <div key={row.key} className="inspector__row" data-testid={`inspector-${row.key}`}>
              <dt className="inspector__label">{fieldLabel(t, row.key)}</dt>
              <dd className="inspector__value">{formatRow(ctx, row)}</dd>
            </div>
          ))}
        </dl>
      </>
    );
  }

  return (
    <section
      className="viewer-panel inspector"
      aria-label={t('inspector.title')}
      data-testid="inspector"
    >
      <header className="viewer-panel__header">
        <h2 className="viewer-panel__title">{t('inspector.title')}</h2>
        {!editable && boardLayers === null && (
          <span className="viewer-panel__note">{t('viewer.readOnly')}</span>
        )}
      </header>
      {body}
    </section>
  );
}
