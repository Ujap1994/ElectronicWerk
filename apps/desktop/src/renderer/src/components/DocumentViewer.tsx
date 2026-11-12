import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from 'react';

import { formatLength, isEngineEvent, type DocumentSummary, type Units } from '@shared/project';

import { BoardOverlay } from '../board/BoardOverlay';
import { BoardActions, BoardToolPalette } from '../board/BoardToolbar';
import { DrcPanel } from '../board/DrcPanel';
import type { BoardEditor } from '../board/editor';
import { useBoardEditor, useSchematicCrossProbe } from '../board/useBoardEditor';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { SchematicEditor, type Point } from '../schematic/editor';
import { EditActions, ToolPalette, type EditorCommands } from '../schematic/EditorToolbar';
import { EditorOverlay } from '../schematic/EditorOverlay';
import { ErcPanel, ErcSettingsDialog, runErc, useErcUpdates } from '../schematic/ErcPanel';
import { AnnotateDialog, FindPanel } from '../schematic/FindPanel';
import { ExportMenu, Guide, GridAssistant } from '../schematic/Panels';
import { NetNavigator } from '../schematic/NetNavigator';
import {
  activeSheet,
  ercState,
  highlightedNet,
  netNavigation,
  requestFocus,
  takeFocus,
} from '../schematic/stores';
import { SymbolChooser } from '../schematic/SymbolChooser';
import { useAppState } from '../state';
import { useDarkScheme } from '../use-dark-scheme';
import { zoomPercent } from '../viewer/camera';
import { ViewerController, type ViewMemory } from '../viewer/controller';
import { loadCanvasFont } from '../viewer/glyph-atlas';
import { typeLabel } from '../viewer/labels';
import { useOpenDocument } from '../viewer/open-context';
import type { SceneReply } from '../viewer/types';
import { Icon, type IconName } from './Icon';
import { Inspector } from './Inspector';
import { LayerPanel } from './LayerPanel';

/** View settings per document, kept while the project is open. */
const memories = new Map<string, ViewMemory>();

/** Sheets whose guide was closed in this session. */
const guideDismissed = new Set<string>();

/** Forgets the view settings (when a project closes). */
export function forgetViewMemories(): void {
  memories.clear();
}

/** CSS pixels per millimetre at 100 % zoom (96 dpi). */
const CSS_PX_PER_MM = 96 / 25.4;

function ToolButton({
  icon,
  label,
  pressed,
  testId,
  onClick,
}: {
  readonly icon: IconName;
  readonly label: string;
  readonly pressed?: boolean;
  readonly testId: string;
  readonly onClick: () => void;
}): ReactNode {
  return (
    <button
      type="button"
      className="viewer-tool"
      title={label}
      aria-label={label}
      aria-pressed={pressed}
      data-testid={testId}
      onClick={onClick}
    >
      <Icon name={icon} size={18} />
    </button>
  );
}

function Overlay({ controller }: { readonly controller: ViewerController }): ReactNode {
  const view = useSyncExternalStore(controller.subscribeView, controller.view);
  const overlay = view.overlay;
  if (overlay === null) {
    return null;
  }
  return (
    <svg className="viewer__overlay" aria-hidden="true">
      {overlay.kind === 'box' ? (
        <rect
          className="viewer__box"
          data-crossing={overlay.crossing}
          x={Math.min(overlay.x0, overlay.x1)}
          y={Math.min(overlay.y0, overlay.y1)}
          width={Math.abs(overlay.x1 - overlay.x0)}
          height={Math.abs(overlay.y1 - overlay.y0)}
        />
      ) : (
        <polygon className="viewer__lasso" points={overlay.points.join(' ')} />
      )}
    </svg>
  );
}

function BoardStatus({ editor }: { readonly editor: BoardEditor }): ReactNode {
  const { t } = useAppState();
  const state = useSyncExternalStore(editor.subscribe, editor.snapshot);
  const route = state.route;
  return (
    <>
      {state.hint !== null && (
        <span className="viewer__hint" data-testid="editor-hint">
          {t(state.hint)}
        </span>
      )}
      {route !== null && (
        <span className="viewer__hint" data-testid="route-status">
          {t('board.routeStatus', {
            net: route.netName === '' ? t('board.noNet') : route.netName,
            width: (route.width / 1e6).toFixed(3),
          })}
        </span>
      )}
      {state.net !== null && route === null && (
        <button
          type="button"
          className="net-chip"
          title={t('schematic.clearNet')}
          data-testid="net-chip"
          onClick={() => {
            editor.showNet(null);
          }}
        >
          {t('schematic.net', { name: state.net })} ×
        </button>
      )}
    </>
  );
}

function EditorStatus({ editor }: { readonly editor: SchematicEditor }): ReactNode {
  const { t } = useAppState();
  const state = useSyncExternalStore(editor.subscribe, editor.snapshot);
  return (
    <>
      {state.hint !== null && (
        <span className="viewer__hint" data-testid="editor-hint">
          {t(state.hint)}
        </span>
      )}
      {state.net !== null && (
        <>
          <button
            type="button"
            className="net-chip"
            title={t('schematic.clearNet')}
            data-testid="net-chip"
            onClick={() => {
              editor.clearHighlight();
            }}
          >
            {t('schematic.net', { name: state.net })} ×
          </button>
          <button
            type="button"
            className="link-button"
            title={t('schematic.navigator.hint')}
            data-testid="net-places"
            onClick={() => {
              const { net, places } = highlightedNet.get();
              if (net !== null) {
                netNavigation.set({ net, places, current: 0 });
              }
            }}
          >
            {t('schematic.navigator.open')}
          </button>
        </>
      )}
    </>
  );
}

function Footer({
  controller,
  editor,
  board,
  units,
}: {
  readonly controller: ViewerController;
  readonly editor: SchematicEditor | null;
  readonly board: BoardEditor | null;
  readonly units: Units;
}): ReactNode {
  const { t, snapshot } = useAppState();
  const view = useSyncExternalStore(controller.subscribeView, controller.view);
  const locale = snapshot.language;
  const length = (nm: number): string => formatLength(nm, units, locale);
  const percent = new Intl.NumberFormat(locale, { maximumSignificantDigits: 3 }).format(
    zoomPercent(view.scale, CSS_PX_PER_MM),
  );
  return (
    <footer className="viewer__footer" data-testid="viewer-footer">
      <span className="viewer__cursor" data-testid="viewer-cursor">
        {view.cursor === null
          ? ' '
          : t('viewer.cursor', { x: length(view.cursor[0]), y: length(view.cursor[1]) })}
      </span>
      <span className="viewer__hover" data-testid="viewer-hover">
        {view.hover === null
          ? ''
          : view.hover.net === null
            ? typeLabel(t, view.hover.type)
            : `${typeLabel(t, view.hover.type)} · ${view.hover.net}`}
      </span>
      {editor !== null && <EditorStatus editor={editor} />}
      {board !== null && <BoardStatus editor={board} />}
      <span className="viewer__spacer" />
      {view.grid > 0 && <span>{t('viewer.grid', { size: length(view.grid) })}</span>}
      <span data-testid="viewer-zoom">{t('viewer.zoom', { percent })}</span>
    </footer>
  );
}

/** The 2D view of a sheet or board, with layer panel and inspector. */
export function DocumentViewer({
  document,
  units,
}: {
  readonly document: DocumentSummary;
  readonly units: Units;
}): ReactNode {
  const { t, engine } = useAppState();
  const { project, notify } = useProject();
  const openDocument = useOpenDocument();
  const controller = useMemo(() => new ViewerController(), []);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const areaRef = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const snapshot = useSyncExternalStore(controller.subscribe, controller.snapshot);
  const dark = useDarkScheme();
  const doc = document.id;
  const sheet = document.kind === 'schematic';
  const [chooser, setChooser] = useState(false);
  const [finding, setFinding] = useState(false);
  const [annotating, setAnnotating] = useState(false);
  const [ercSettings, setErcSettings] = useState(false);
  const [popover, setPopover] = useState<'export' | 'grid' | null>(null);
  const empty = (document.stats.symbols ?? 0) === 0 && (document.stats.wires ?? 0) === 0;
  const [guide, setGuide] = useState(sheet && empty && !guideDismissed.has(doc));
  useErcUpdates();

  const editor = useMemo(
    () =>
      sheet
        ? new SchematicEditor(controller, {
            doc,
            call: (method, params) => engine.call(method, params),
            error: (error) => {
              notify(errorMessage(t, error), 'error');
            },
            chooseSymbol: () => {
              setChooser(true);
            },
            find: () => {
              setFinding(true);
            },
            highlight: (net, sheets, places) => {
              highlightedNet.set({ net, sheets, places });
            },
            navigate: (net, places, current) => {
              netNavigation.set({ net, places, current });
            },
          })
        : null,
    [sheet, controller, doc, engine, notify, t],
  );

  const boardUi = useBoardEditor({
    controller,
    document,
    enabled: document.kind === 'pcb',
    units,
    canvasRef,
  });
  const boardEditor = boardUi.editor;
  const firstBoard = project?.documents.find((entry) => entry.kind === 'pcb')?.id ?? null;
  useSchematicCrossProbe(controller, doc, sheet, firstBoard);

  useEffect(() => {
    controller.setInteraction(editor ?? boardEditor);
    return () => {
      controller.setInteraction(null);
    };
  }, [controller, editor, boardEditor]);

  // A net highlighted on another sheet shows here too.
  useEffect(() => {
    if (editor === null) {
      return undefined;
    }
    const apply = (): void => {
      const { net, sheets } = highlightedNet.get();
      editor.showNet(net, sheets);
    };
    apply();
    return highlightedNet.subscribe(apply);
  }, [editor]);

  // The command palette offers the commands of the shown sheet. The
  // registered object stays the same; it calls the latest commands.
  const commandsRef = useRef<EditorCommands | null>(null);
  useEffect(() => {
    if (!sheet) {
      return undefined;
    }
    const call = (name: keyof EditorCommands) => (): void => {
      commandsRef.current?.[name]();
    };
    activeSheet.set({
      chooseSymbol: call('chooseSymbol'),
      annotate: call('annotate'),
      erc: call('erc'),
      find: call('find'),
      exportMenu: call('exportMenu'),
      grid: call('grid'),
      guide: call('guide'),
    });
    return () => {
      activeSheet.set(null);
    };
  }, [sheet]);

  /** Shows a point and selects objects, on this or another sheet. */
  const showTarget = (target: { doc: string; at: Point; ids: readonly string[] }): void => {
    if (target.doc === doc) {
      controller.focusOn(target.at);
      controller.selectIds(target.ids);
      canvasRef.current?.focus();
      return;
    }
    const other = project?.documents.find((entry) => entry.id === target.doc);
    if (other !== undefined && openDocument !== null) {
      requestFocus({ doc: target.doc, at: target.at, ids: target.ids });
      openDocument(other);
    }
  };

  const commands: EditorCommands = {
    chooseSymbol: () => {
      setChooser(true);
    },
    annotate: () => {
      setAnnotating(true);
    },
    erc: () => {
      ercState.set({ ...ercState.get(), open: true });
      void runErc((method) => engine.call(method));
    },
    find: () => {
      setFinding(true);
    },
    exportMenu: () => {
      setPopover(popover === 'export' ? null : 'export');
    },
    grid: () => {
      setPopover(popover === 'grid' ? null : 'grid');
    },
    guide: () => {
      guideDismissed.delete(doc);
      setGuide(!guide);
    },
  };
  useEffect(() => {
    commandsRef.current = commands;
  });

  useLayoutEffect(() => {
    const canvas = canvasRef.current;
    const area = areaRef.current;
    if (canvas === null || area === null) {
      return undefined;
    }
    controller.attach(canvas);
    const observer = new ResizeObserver(() => {
      const rect = area.getBoundingClientRect();
      controller.resize(rect.width, rect.height, window.devicePixelRatio);
    });
    observer.observe(area);
    return () => {
      observer.disconnect();
      const memory = controller.exportMemory();
      if (memory !== null) {
        memories.set(doc, memory);
      }
      controller.detach();
    };
  }, [controller, doc]);

  useEffect(() => {
    controller.setTheme(dark ? 'dark' : 'light');
  }, [controller, dark]);

  const labels = useMemo(
    () => ({
      file: t('viewer.titleFile'),
      size: t('viewer.titleSize'),
      date: t('viewer.titleDate'),
      revision: t('viewer.titleRevision'),
    }),
    [t],
  );

  // Load the scene, and reload it after every change of the project.
  useEffect(() => {
    let cancelled = false;
    let request = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = async (): Promise<void> => {
      request += 1;
      const mine = request;
      try {
        await loadCanvasFont();
        await engine.whenConnected();
        const reply = await engine.call<SceneReply>('view.scene', { doc, labels }, 120_000);
        if (!cancelled && mine === request) {
          const focus = takeFocus(doc);
          if (focus !== null) {
            controller.selectAfterReload(focus.ids);
            controller.focusAfterReload(focus.at);
          }
          controller.setScene(reply, memories.get(doc) ?? null);
          setError(null);
        }
      } catch (reason) {
        if (!cancelled && mine === request) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      } finally {
        if (!cancelled && mine === request) {
          setLoading(false);
        }
      }
    };
    void load();
    const off = engine.onEvent((event) => {
      if (!isEngineEvent(event)) {
        return;
      }
      clearTimeout(timer);
      timer = setTimeout(() => {
        void load();
      }, 30);
    });
    return () => {
      cancelled = true;
      clearTimeout(timer);
      off();
    };
  }, [engine, controller, doc, labels]);

  const board = snapshot.kind === 'pcb';
  return (
    <div className="viewer" data-testid="viewer" data-kind={snapshot.kind ?? ''}>
      <div className="viewer__main">
        <div className="viewer__toolbar" role="toolbar" aria-label={t('viewer.toolbar')}>
          <ToolButton
            icon="zoomIn"
            label={t('viewer.zoomIn')}
            testId="viewer-zoom-in"
            onClick={() => {
              controller.zoomBy(1.5);
            }}
          />
          <ToolButton
            icon="zoomOut"
            label={t('viewer.zoomOut')}
            testId="viewer-zoom-out"
            onClick={() => {
              controller.zoomBy(1 / 1.5);
            }}
          />
          <ToolButton
            icon="fit"
            label={`${t('viewer.fitAll')} (Home)`}
            testId="viewer-fit"
            onClick={() => {
              controller.fitAll();
            }}
          />
          <ToolButton
            icon="focus"
            label={`${t('viewer.fitSelection')} (F)`}
            testId="viewer-fit-selection"
            onClick={() => {
              controller.fitSelection();
            }}
          />
          <span className="viewer-tool__separator" />
          <ToolButton
            icon="boxSelect"
            label={t('viewer.boxSelect')}
            pressed={snapshot.tool === 'box'}
            testId="viewer-tool-box"
            onClick={() => {
              controller.setTool('box');
            }}
          />
          <ToolButton
            icon="lasso"
            label={`${t('viewer.lassoSelect')} (Alt)`}
            pressed={snapshot.tool === 'lasso'}
            testId="viewer-tool-lasso"
            onClick={() => {
              controller.setTool('lasso');
            }}
          />
          {editor !== null && (
            <EditActions
              editor={editor}
              commands={commands}
              hasSelection={snapshot.selection.length > 0}
            />
          )}
          {boardEditor !== null && (
            <BoardActions
              editor={boardEditor}
              commands={boardUi.commands}
              hasSelection={snapshot.selection.length > 0}
            />
          )}
          {board && (
            <>
              <span className="viewer-tool__separator" />
              <ToolButton
                icon="contrast"
                label={t('viewer.highContrast')}
                pressed={snapshot.highContrast}
                testId="viewer-high-contrast"
                onClick={() => {
                  controller.setHighContrast(!snapshot.highContrast);
                }}
              />
              <ToolButton
                icon="flip"
                label={`${t('viewer.flip')} (B)`}
                pressed={snapshot.flipped}
                testId="viewer-flip"
                onClick={() => {
                  controller.toggleFlip();
                }}
              />
            </>
          )}
        </div>
        {popover === 'export' && (
          <ExportMenu
            controller={controller}
            docName={document.name}
            onClose={() => {
              setPopover(null);
            }}
          />
        )}
        {popover === 'grid' && editor !== null && (
          <GridAssistant
            doc={doc}
            editor={editor}
            units={units}
            onClose={() => {
              setPopover(null);
            }}
          />
        )}
        <div className="viewer__area" ref={areaRef}>
          <canvas
            ref={canvasRef}
            className="viewer__canvas"
            tabIndex={0}
            aria-label={t('viewer.canvas', { name: document.name })}
            data-testid="viewer-canvas"
            data-ready={snapshot.status === 'ready' && !loading}
            data-version={snapshot.version}
          />
          <Overlay controller={controller} />
          {editor !== null && <EditorOverlay controller={controller} editor={editor} />}
          {editor !== null && <ToolPalette editor={editor} commands={commands} />}
          {boardEditor !== null && (
            <BoardOverlay controller={controller} editor={boardEditor} doc={doc} />
          )}
          {boardEditor !== null && (
            <BoardToolPalette editor={boardEditor} commands={boardUi.commands} />
          )}
          {editor !== null && guide && (
            <Guide
              doc={doc}
              onClose={() => {
                guideDismissed.add(doc);
                setGuide(false);
              }}
            />
          )}
          {editor !== null && !finding && (
            <NetNavigator
              onShow={(place) => {
                showTarget({ doc: place.doc, at: place.at, ids: [place.id] });
              }}
            />
          )}
          {finding && (
            <FindPanel
              onShow={(hit) => {
                showTarget({ doc: hit.doc, at: hit.at, ids: [hit.id] });
              }}
              onClose={() => {
                setFinding(false);
                canvasRef.current?.focus();
              }}
            />
          )}
          {snapshot.status === 'unavailable' && (
            <p className="viewer__message callout callout--error" role="alert">
              {t('viewer.unavailable')}
            </p>
          )}
          {error !== null && (
            <p
              className="viewer__message callout callout--error"
              role="alert"
              data-testid="viewer-error"
            >
              {t('viewer.failed', { message: error })}
            </p>
          )}
          {loading && error === null && (
            <p className="viewer__message viewer__message--quiet" data-testid="viewer-loading">
              {t('viewer.loading', { name: document.name })}
            </p>
          )}
        </div>
        {editor !== null && (
          <ErcPanel
            doc={doc}
            onShow={(issue) => {
              showTarget({ doc: issue.sheet, at: issue.at, ids: issue.items });
            }}
            onAnnotate={() => {
              setAnnotating(true);
            }}
            onSnap={() => {
              engine.call('schematic.snapToGrid', { doc }).catch((reason: unknown) => {
                notify(errorMessage(t, reason), 'error');
              });
            }}
            onSettings={() => {
              setErcSettings(true);
            }}
          />
        )}
        {boardEditor !== null && (
          <DrcPanel doc={doc} onShow={boardUi.showFinding} onSettings={boardUi.openRules} />
        )}
        <Footer controller={controller} editor={editor} board={boardEditor} units={units} />
      </div>
      <aside className="viewer__side">
        <LayerPanel controller={controller} />
        <Inspector
          controller={controller}
          doc={doc}
          units={units}
          editable={sheet}
          boardLayers={
            boardEditor === null
              ? null
              : snapshot.layers
                  .filter((layer) => layer.group === 'copper')
                  .map((layer) => layer.key)
          }
        />
      </aside>
      {chooser && editor !== null && (
        <SymbolChooser
          onPick={(name, component) => {
            setChooser(false);
            editor.placeSymbol(name, component ?? null);
            canvasRef.current?.focus();
          }}
          onClose={() => {
            setChooser(false);
          }}
        />
      )}
      {annotating && (
        <AnnotateDialog
          doc={doc}
          onClose={() => {
            setAnnotating(false);
          }}
        />
      )}
      {boardUi.dialogs}
      {ercSettings && (
        <ErcSettingsDialog
          onClose={() => {
            setErcSettings(false);
          }}
        />
      )}
    </div>
  );
}
