/**
 * The 3D preview of a footprint's model while it is aligned: the model of
 * the base library with the footprint's offset, rotation and scale, on a
 * board as large as the footprint. Models outside the base library have
 * no preview (the engine cannot read them yet).
 */
import { useEffect, useMemo, useState, type ReactNode } from 'react';

import { ModelPreview } from '../library/ModelPreview';
import type { BoardSize, Placement } from '../library/model-view';
import type { LibraryStatus } from '../library/types';
import { useAppState } from '../state';
import type { FootprintDef, Model3d } from './types';

/** Footprint extent in millimetres (pads and graphics). */
function extent(def: FootprintDef): BoardSize {
  const xs: number[] = [];
  const ys: number[] = [];
  for (const pad of def.pads ?? []) {
    xs.push(pad.at[0] - pad.size[0] / 2, pad.at[0] + pad.size[0] / 2);
    ys.push(pad.at[1] - pad.size[1] / 2, pad.at[1] + pad.size[1] / 2);
  }
  if (xs.length === 0) {
    return { width: 5, depth: 5 };
  }
  const width = (Math.max(...xs) - Math.min(...xs)) / 1e6;
  const depth = (Math.max(...ys) - Math.min(...ys)) / 1e6;
  return { width: width + 2, depth: depth + 2 };
}

export function ModelAlign({
  def,
  model,
}: {
  readonly def: FootprintDef;
  readonly model: Model3d;
}): ReactNode {
  const { t, engine } = useAppState();
  const [found, setFound] = useState<{ path: string; id: number | null } | null>(null);
  const [release, setRelease] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void engine.call<{ id: number | null }>('library.model', { path: model.path }).then(
      (reply) => {
        if (!cancelled) {
          setFound({ path: model.path, id: reply.id });
        }
      },
      () => {
        if (!cancelled) {
          setFound({ path: model.path, id: null });
        }
      },
    );
    return () => {
      cancelled = true;
    };
  }, [engine, model.path]);

  useEffect(() => {
    void engine.call<LibraryStatus>('library.status').then(
      (status) => {
        setRelease(status.status?.release ?? null);
      },
      () => undefined,
    );
  }, [engine]);

  const [ox = 0, oy = 0, oz = 0] = model.offset ?? [];
  const [rx = 0, ry = 0, rz = 0] = model.rotation ?? [];
  const [sx = 1, sy = 1, sz = 1] = model.scale ?? [];
  const placement = useMemo<Placement>(
    () => ({ offset: [ox / 1e6, oy / 1e6, oz / 1e6], rotation: [rx, ry, rz], scale: [sx, sy, sz] }),
    [ox, oy, oz, rx, ry, rz, sx, sy, sz],
  );
  const board = useMemo(() => extent(def), [def]);

  if (found?.path !== model.path) {
    return null;
  }
  if (found.id === null) {
    return (
      <p className="libedit-props__muted" data-testid="model-not-found">
        {t('libedit.models.notFound')}
      </p>
    );
  }
  return (
    <div className="libedit-model__preview" data-testid="model-align">
      <ModelPreview modelId={found.id} placement={placement} board={board} release={release} />
    </div>
  );
}
