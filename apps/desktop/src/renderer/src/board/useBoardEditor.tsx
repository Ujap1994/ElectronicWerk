/**
 * Puts the layout editor into the document view of a board: the editor
 * itself, its commands (also for the command palette), the dialogs, the
 * online DRC and cross-probing with the schematic.
 */
import { useEffect, useMemo, useRef, useState, type ReactNode, type RefObject } from 'react';

import type { DocumentSummary, Units } from '@shared/project';

import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import type { ViewerController } from '../viewer/controller';
import {
  FootprintChooser,
  DxfDialog,
  SyncDialog,
  ZoneDialog,
  newZone,
  type ZoneJson,
} from './dialogs';
import { runDrc, useDrcUpdates } from './DrcPanel';
import { BoardEditor, type Point } from './editor';
import { RulesDialog } from './RulesDialog';
import { SetupDialog } from './SetupDialog';
import {
  activeBoard,
  crossProbing,
  crossSelection,
  drcState,
  type BoardCommands,
  type DrcFinding,
} from './stores';

type Dialog =
  | { kind: 'chooser' }
  | { kind: 'zone'; zone: ZoneJson; isNew: boolean }
  | { kind: 'sync' }
  | { kind: 'setup' }
  | { kind: 'rules' }
  | { kind: 'dxf' }
  | null;

export interface BoardUi {
  readonly editor: BoardEditor | null;
  readonly commands: BoardCommands;
  readonly dialogs: ReactNode;
  /** Shows a DRC finding: zooms to it and selects its objects. */
  readonly showFinding: (finding: DrcFinding) => void;
  /** Opens the rules dialog on the DRC checks. */
  readonly openRules: () => void;
}

/** The ids of the selected objects of a type. */
function selectedOfType(controller: ViewerController, type: string): string[] {
  return controller
    .snapshot()
    .selection.filter((s) => s.type === type)
    .map((s) => s.id);
}

/** The layout editor of a board (`enabled` false for sheets). */
export function useBoardEditor({
  controller,
  document,
  enabled,
  units,
  canvasRef,
}: {
  readonly controller: ViewerController;
  readonly document: DocumentSummary;
  readonly enabled: boolean;
  readonly units: Units;
  readonly canvasRef: RefObject<HTMLCanvasElement | null>;
}): BoardUi {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  const doc = document.id;
  const [dialog, setDialog] = useState<Dialog>(null);

  const editor = useMemo(
    () =>
      enabled
        ? new BoardEditor(controller, {
            doc,
            call: (method, params) => engine.call(method, params),
            error: (error) => {
              notify(errorMessage(t, error), 'error');
            },
            chooseFootprint: () => {
              setDialog({ kind: 'chooser' });
            },
            newZone: (outline: readonly Point[], keepout: boolean, layer: string) => {
              setDialog({ kind: 'zone', zone: newZone(outline, keepout, layer), isNew: true });
            },
            editZone: (id: string) => {
              engine
                .call<{ zones?: ZoneJson[] }>('document.get', { doc })
                .then((board) => {
                  const zone = board.zones?.find((z) => z.id === id);
                  if (zone !== undefined) {
                    setDialog({ kind: 'zone', zone, isNew: false });
                  }
                })
                .catch((error: unknown) => {
                  notify(errorMessage(t, error), 'error');
                });
            },
            edited: () => undefined,
          })
        : null,
    [enabled, controller, doc, engine, notify, t],
  );

  // The online DRC (and with it the ratsnest) of the shown board.
  useDrcUpdates(enabled ? doc : null);

  // Copper layers of each new scene.
  useEffect(() => {
    if (editor === null) {
      return undefined;
    }
    const apply = (): void => {
      const layers = controller
        .snapshot()
        .layers.filter((layer) => layer.group === 'copper')
        .map((layer) => layer.key);
      editor.setLayers(layers);
    };
    apply();
    return controller.subscribe(apply);
  }, [controller, editor]);

  // Cross-probing: board selection → schematic, and back.
  const applyingRef = useRef(false);
  useEffect(() => {
    if (editor === null) {
      return undefined;
    }
    let last = '';
    const offController = controller.subscribe(() => {
      if (applyingRef.current || !crossProbing.get()) {
        return;
      }
      const footprints = selectedOfType(controller, 'footprint');
      const key = footprints.join(',');
      if (key === last) {
        return;
      }
      last = key;
      if (footprints.length === 0) {
        return;
      }
      engine
        .call<{ symbols: { doc: string; id: string }[] }>('pcb.crossProbe', { doc, footprints })
        .then((reply) => {
          crossSelection.set({ source: 'board', footprints, symbols: reply.symbols });
        })
        .catch(() => undefined);
    });
    const apply = (): void => {
      const selection = crossSelection.get();
      if (selection?.source !== 'schematic' || !crossProbing.get()) {
        return;
      }
      applyingRef.current = true;
      if (controller.currentScene === null) {
        controller.selectAfterReload(selection.footprints);
      } else {
        controller.selectIds(selection.footprints);
      }
      last = selection.footprints.join(',');
      applyingRef.current = false;
    };
    apply();
    const offStore = crossSelection.subscribe(apply);
    return () => {
      offController();
      offStore();
    };
  }, [controller, editor, engine, doc]);

  const commands: BoardCommands = useMemo(
    () => ({
      drc: () => {
        const state = drcState.get();
        drcState.set({ ...state, open: !state.open || state.doc !== doc });
        void runDrc((method, params) => engine.call(method, params), doc);
      },
      fillZones: () => {
        editor?.fillZones(true);
      },
      update: () => {
        setDialog({ kind: 'sync' });
      },
      setup: () => {
        setDialog({ kind: 'setup' });
      },
      rules: () => {
        setDialog({ kind: 'rules' });
      },
      importDxf: () => {
        setDialog({ kind: 'dxf' });
      },
      placeFootprint: () => {
        setDialog({ kind: 'chooser' });
      },
    }),
    [doc, editor, engine],
  );

  // The command palette offers the commands of the shown board.
  useEffect(() => {
    if (!enabled) {
      return undefined;
    }
    activeBoard.set(commands);
    return () => {
      activeBoard.set(null);
    };
  }, [enabled, commands]);

  const close = (): void => {
    setDialog(null);
    canvasRef.current?.focus();
  };
  const layers = editor?.snapshot().layers ?? [];
  const dialogs =
    dialog === null || editor === null ? null : dialog.kind === 'chooser' ? (
      <FootprintChooser
        onPick={(name) => {
          setDialog(null);
          editor.placeFootprint(name);
          canvasRef.current?.focus();
        }}
        onClose={close}
      />
    ) : dialog.kind === 'zone' ? (
      <ZoneDialog
        doc={doc}
        zone={dialog.zone}
        layers={layers}
        units={units}
        isNew={dialog.isNew}
        onClose={close}
      />
    ) : dialog.kind === 'sync' ? (
      <SyncDialog
        doc={doc}
        onClose={close}
        onApplied={(select) => {
          if (select.length > 0) {
            controller.selectAfterReload(select);
          }
        }}
      />
    ) : dialog.kind === 'setup' ? (
      <SetupDialog doc={doc} units={units} onClose={close} />
    ) : dialog.kind === 'rules' ? (
      <RulesDialog doc={doc} units={units} onClose={close} />
    ) : (
      <DxfDialog doc={doc} units={units} onClose={close} />
    );

  return {
    editor,
    commands,
    dialogs,
    showFinding: (finding) => {
      controller.focusOn(finding.at);
      controller.selectIds(finding.targets.map((target) => target.id));
      canvasRef.current?.focus();
    },
    openRules: () => {
      setDialog({ kind: 'rules' });
    },
  };
}

/**
 * Cross-probing on a schematic sheet: selected symbols select their
 * footprints on the board, and a board selection selects symbols here.
 */
export function useSchematicCrossProbe(
  controller: ViewerController,
  doc: string,
  enabled: boolean,
  boardDoc: string | null,
): void {
  const { engine } = useAppState();
  const applyingRef = useRef(false);
  useEffect(() => {
    if (!enabled) {
      return undefined;
    }
    let last = '';
    const offController = controller.subscribe(() => {
      if (applyingRef.current || !crossProbing.get() || boardDoc === null) {
        return;
      }
      const symbols = selectedOfType(controller, 'symbol');
      const key = symbols.join(',');
      if (key === last) {
        return;
      }
      last = key;
      if (symbols.length === 0) {
        return;
      }
      engine
        .call<{ footprints: string[] }>('pcb.crossProbe', { doc: boardDoc, symbols })
        .then((reply) => {
          crossSelection.set({
            source: 'schematic',
            footprints: reply.footprints,
            symbols: symbols.map((id) => ({ doc, id })),
          });
        })
        .catch(() => undefined);
    });
    const apply = (): void => {
      const selection = crossSelection.get();
      if (selection?.source !== 'board' || !crossProbing.get()) {
        return;
      }
      const ids = selection.symbols.filter((s) => s.doc === doc).map((s) => s.id);
      applyingRef.current = true;
      if (controller.currentScene === null) {
        controller.selectAfterReload(ids);
      } else {
        controller.selectIds(ids);
      }
      last = ids.join(',');
      applyingRef.current = false;
    };
    apply();
    const offStore = crossSelection.subscribe(apply);
    return () => {
      offController();
      offStore();
    };
  }, [controller, engine, doc, enabled, boardDoc]);
}
