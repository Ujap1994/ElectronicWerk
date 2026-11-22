/**
 * Group and ungroup the selected board objects (Ctrl+G, Ctrl+Shift+G in
 * the layout editor).
 */
import type { ReactNode } from 'react';

import { Icon } from '../components/Icon';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import type { ViewerController } from '../viewer/controller';

export function GroupActions({
  doc,
  ids,
  controller,
}: {
  readonly doc: string;
  /** The selected top-level objects. */
  readonly ids: readonly string[];
  readonly controller: ViewerController;
}): ReactNode {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  const groups = controller.currentScene?.meta.groups ?? [];
  const chosen = new Set(ids);
  const touched = groups.filter((group) => group.some((id) => chosen.has(id)));
  // The selection is exactly one group already: nothing to group.
  const oneGroup =
    touched.length === 1 &&
    touched[0]?.length === ids.length &&
    touched[0].every((id) => chosen.has(id));
  const run = (op: 'group' | 'ungroup'): void => {
    engine.call('pcb.edit', { doc, op, ids }).catch((error: unknown) => {
      notify(errorMessage(t, error), 'error');
    });
  };
  if (ids.length < 2 && touched.length === 0) {
    return null;
  }
  return (
    <div className="group-actions">
      {ids.length >= 2 && !oneGroup && (
        <button
          type="button"
          className="button button--small"
          title={t('board.groups.groupTip')}
          data-testid="board-group"
          onClick={() => {
            run('group');
          }}
        >
          <Icon name="duplicate" size={14} />
          {t('board.groups.group')}
        </button>
      )}
      {touched.length > 0 && (
        <button
          type="button"
          className="button button--small"
          title={t('board.groups.ungroupTip')}
          data-testid="board-ungroup"
          onClick={() => {
            run('ungroup');
          }}
        >
          <Icon name="split" size={14} />
          {t('board.groups.ungroup')}
        </button>
      )}
    </div>
  );
}
