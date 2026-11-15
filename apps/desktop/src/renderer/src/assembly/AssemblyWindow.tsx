/**
 * The assembly assistant in its own window (a second monitor at the
 * workbench, full screen). It closes when the project closes.
 */
import { useEffect, useState, type ReactNode } from 'react';

import { useAppState } from '../state';
import { AssemblyView } from './AssemblyView';

/** The board this window shows, from its URL fragment. */
export function assemblyFromHash(hash: string): { doc: string; name: string } | null {
  const params = new URLSearchParams(hash.replace(/^#/u, ''));
  const doc = params.get('assembly');
  if (doc === null || !/^[0-9a-f-]{36}$/u.test(doc)) {
    return null;
  }
  return { doc, name: params.get('name') ?? '' };
}

export function AssemblyWindow({
  doc,
  name,
}: {
  readonly doc: string;
  readonly name: string;
}): ReactNode {
  const { engine, t, engineStatus } = useAppState();
  const [closed, setClosed] = useState(false);

  useEffect(() => {
    document.title = t('assembly.windowTitle', { name });
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
      <div className="view3d-window view3d-window--closed" data-testid="assembly-window-closed">
        {t('assembly.closed')}
      </div>
    );
  }
  return (
    <div className="view3d-window" data-testid="assembly-window">
      {engineStatus.state === 'running' && <AssemblyView doc={doc} name={name} detached />}
    </div>
  );
}
