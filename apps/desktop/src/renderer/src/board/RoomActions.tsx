/**
 * Places the selected parts into the selected room (select a room and the
 * parts, for example the parts selected in the schematic by cross-probing).
 */
import type { ReactNode } from 'react';

import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import type { SelectedObject, ViewerController } from '../viewer/controller';

export function RoomActions({
  doc,
  selection,
  controller,
}: {
  readonly doc: string;
  readonly selection: readonly SelectedObject[];
  readonly controller: ViewerController;
}): ReactNode {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  const rooms = selection.filter((entry) => entry.type === 'room' && entry.child === null);
  const parts = [
    ...new Set(
      selection
        .filter((entry) => entry.type === 'footprint' || entry.child?.kind === 'pad')
        .map((entry) => entry.id),
    ),
  ];
  const room = rooms[0];
  if (rooms.length !== 1 || room === undefined || parts.length === 0) {
    return null;
  }
  const place = (): void => {
    engine
      .call<{ placed: string[]; noRoom: string[]; select: string[] }>('pcb.placeInRoom', {
        doc,
        room: room.id,
        ids: parts,
      })
      .then(
        (reply) => {
          controller.selectAfterReload(reply.select);
          if (reply.noRoom.length > 0) {
            notify(t('board.rooms.noRoom', { parts: reply.noRoom.join(', ') }), 'info');
          }
        },
        (error: unknown) => {
          notify(errorMessage(t, error), 'error');
        },
      );
  };
  return (
    <div className="group-actions">
      <button
        type="button"
        className="button button--small"
        data-testid="room-place"
        onClick={place}
      >
        {t('board.rooms.place', { count: parts.length })}
      </button>
    </div>
  );
}
