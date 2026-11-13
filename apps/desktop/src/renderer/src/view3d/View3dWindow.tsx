/**
 * The 3D view in its own window (for a second monitor). The window shows
 * the board of the project the main window has open; it closes when the
 * project closes.
 */
import { useEffect, useState, type ReactNode } from 'react';

import { useAppState } from '../state';
import { Board3dView } from './Board3dView';

/** The board this window shows, from its URL fragment. */
export function viewFromHash(hash: string): { doc: string; name: string } | null {
  const params = new URLSearchParams(hash.replace(/^#/u, ''));
  const doc = params.get('view3d');
  if (doc === null || !/^[0-9a-f-]{36}$/u.test(doc)) {
    return null;
  }
  return { doc, name: params.get('name') ?? '' };
}

export function View3dWindow({
  doc,
  name,
}: {
  readonly doc: string;
  readonly name: string;
}): ReactNode {
  const { engine, t, engineStatus } = useAppState();
  const [closed, setClosed] = useState(false);

  useEffect(() => {
    document.title = t('view3d.windowTitle', { name });
  }, [t, name]);

  useEffect(
    () =>
      engine.onEvent((event) => {
        const record = event as { type?: string; action?: string } | null;
        if (record?.type === 'project' && record.action === 'closed') {
          setClosed(true);
        }
      }),
    [engine],
  );

  if (closed) {
    return (
      <div className="view3d-window view3d-window--closed" data-testid="view3d-window-closed">
        {t('view3d.closed')}
      </div>
    );
  }
  return (
    <div className="view3d-window" data-testid="view3d-window">
      {engineStatus.state === 'running' && <Board3dView doc={doc} name={name} detached />}
    </div>
  );
}
