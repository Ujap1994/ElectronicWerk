import { useState, useSyncExternalStore, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';

import { Icon, type IconName } from '../components/Icon';
import { useAppState } from '../state';
import type { BoardEditor, BoardTool, RouteMode, ViaChoice } from './editor';
import {
  crossProbing,
  drcState,
  netInspector,
  routingCorners,
  routingMode,
  routingVia,
  setRoutingCorners,
  setRoutingMode,
  setRoutingVia,
  type BoardCommands,
} from './stores';
import { shortcutLabel } from '../shortcuts';
import { DIMENSION_KINDS } from './dimension';

const VIA_CHOICES: readonly ViaChoice[] = ['through', 'blind', 'micro'];
const ROUTE_MODES: readonly RouteMode[] = ['walkaround', 'shove'];

/** Whether routes walk around the tracks of other nets or push them aside. */
function RouteModeSelect({ editor }: { readonly editor: BoardEditor }): ReactNode {
  const { t } = useAppState();
  const mode = useSyncExternalStore(routingMode.subscribe, routingMode.get);
  return (
    <label className="board-layer" title={t('board.actions.routeModeTip')}>
      <span className="board-layer__label">{t('board.actions.routeMode')}</span>
      <select
        aria-label={t('board.actions.routeMode')}
        value={mode}
        data-testid="board-route-mode"
        onChange={(event) => {
          const value = event.target.value;
          if (value === 'walkaround' || value === 'shove') {
            setRoutingMode(value);
            editor.refreshRoute();
          }
        }}
      >
        {ROUTE_MODES.map((choice) => (
          <option key={choice} value={choice}>
            {t(`board.routeModes.${choice}`)}
          </option>
        ))}
      </select>
    </label>
  );
}

/** The via type placed when a route changes layers. */
function ViaSelect(): ReactNode {
  const { t } = useAppState();
  const via = useSyncExternalStore(routingVia.subscribe, routingVia.get);
  return (
    <label className="board-layer" title={t('board.actions.viaTypeTip')}>
      <span className="board-layer__label">{t('board.actions.viaType')}</span>
      <select
        aria-label={t('board.actions.viaType')}
        value={via}
        data-testid="board-via-type"
        onChange={(event) => {
          const value = event.target.value;
          if (value === 'through' || value === 'blind' || value === 'micro') {
            setRoutingVia(value);
          }
        }}
      >
        {VIA_CHOICES.map((choice) => (
          <option key={choice} value={choice}>
            {t(`board.viaTypes.${choice}`)}
          </option>
        ))}
      </select>
    </label>
  );
}

interface ToolSpec {
  readonly tool: BoardTool;
  readonly icon: IconName;
}

/** The layout tools in toolbar order. */
const TOOLS: readonly ToolSpec[] = [
  { tool: 'select', icon: 'pointer' },
  { tool: 'footprint', icon: 'chip' },
  { tool: 'route', icon: 'track' },
  { tool: 'pair', icon: 'pair' },
  { tool: 'via', icon: 'via' },
  { tool: 'zone', icon: 'zone' },
  { tool: 'keepout', icon: 'keepout' },
  { tool: 'outline', icon: 'outline' },
  { tool: 'text', icon: 'text' },
  { tool: 'dimension', icon: 'dimension' },
  { tool: 'room', icon: 'room' },
  { tool: 'highlight', icon: 'highlight' },
];

function Button({
  icon,
  label,
  tip,
  pressed,
  disabled = false,
  testId,
  onClick,
}: {
  readonly icon: IconName;
  readonly label: string;
  readonly tip: string;
  readonly pressed?: boolean;
  readonly disabled?: boolean;
  readonly testId: string;
  readonly onClick: () => void;
}): ReactNode {
  return (
    <button
      type="button"
      className="viewer-tool"
      title={tip}
      aria-label={label}
      aria-pressed={pressed}
      disabled={disabled}
      data-testid={testId}
      onClick={onClick}
    >
      <Icon name={icon} size={18} />
    </button>
  );
}

/** The layout tools (a column at the left edge of the board). */
export function BoardToolPalette({
  editor,
  commands,
}: {
  readonly editor: BoardEditor;
  readonly commands: BoardCommands;
}): ReactNode {
  const { t, appInfo } = useAppState();
  const mac = appInfo.platform === 'darwin';
  const state = useSyncExternalStore(editor.subscribe, editor.snapshot);
  return (
    <div
      className="tool-palette"
      role="toolbar"
      aria-orientation="vertical"
      aria-label={t('board.toolbar')}
      data-testid="board-tool-palette"
    >
      {TOOLS.map(({ tool, icon }) => {
        const label = t(`board.tools.${tool}` as MessageKey);
        const key = tool === 'select' ? 'Esc' : shortcutLabel(`board.${tool}`, mac);
        return (
          <Button
            key={tool}
            icon={icon}
            label={label}
            tip={`${label} (${key})\n${t(`board.tips.${tool}` as MessageKey)}`}
            pressed={state.tool === tool}
            testId={`board-tool-${tool}`}
            onClick={() => {
              if (tool === 'footprint') {
                commands.placeFootprint();
              } else {
                editor.setTool(tool);
              }
            }}
          />
        );
      })}
    </div>
  );
}

/** The kind of the next dimension (shown while the dimension tool is on). */
function DimensionSelect({ editor }: { readonly editor: BoardEditor }): ReactNode {
  const { t } = useAppState();
  const state = useSyncExternalStore(editor.subscribe, editor.snapshot);
  if (state.tool !== 'dimension') {
    return null;
  }
  return (
    <label className="board-layer" title={t('board.dimensions.kindTip')}>
      <span className="board-layer__label">{t('board.dimensions.kind')}</span>
      <select
        aria-label={t('board.dimensions.kind')}
        value={state.dimension}
        data-testid="board-dimension-kind"
        onChange={(event) => {
          const kind = DIMENSION_KINDS.find((k) => k === event.target.value);
          if (kind !== undefined) {
            editor.setDimensionKind(kind);
          }
        }}
      >
        {DIMENSION_KINDS.map((kind) => (
          <option key={kind} value={kind}>
            {t(`board.dimensions.kinds.${kind}`)}
          </option>
        ))}
      </select>
    </label>
  );
}

/** The active copper layer (Page Up / Page Down). */
function LayerSelect({ editor }: { readonly editor: BoardEditor }): ReactNode {
  const { t } = useAppState();
  const state = useSyncExternalStore(editor.subscribe, editor.snapshot);
  const current = state.route?.layer ?? state.layer;
  return (
    <label className="board-layer" title={`${t('board.activeLayer')} (PgUp/PgDn)`}>
      <span className={`board-layer__swatch board-layer__swatch--${current.replace('.', '-')}`} />
      <select
        aria-label={t('board.activeLayer')}
        value={current}
        data-testid="board-layer"
        onChange={(event) => {
          editor.setLayer(event.target.value);
        }}
      >
        {state.layers.map((layer) => (
          <option key={layer} value={layer}>
            {layerName(t, layer)}
          </option>
        ))}
      </select>
    </label>
  );
}

/** A readable name of a copper layer key. */
export function layerName(t: ReturnType<typeof useAppState>['t'], layer: string): string {
  if (layer === 'cu.top') {
    return t('board.layers.top');
  }
  if (layer === 'cu.bottom') {
    return t('board.layers.bottom');
  }
  const inner = /^cu\.in(\d+)$/u.exec(layer);
  return inner === null ? layer : t('board.layers.inner', { n: inner[1] ?? '' });
}

/** Arrangement commands for several footprints. */
function ArrangeMenu({
  editor,
  onClose,
}: {
  readonly editor: BoardEditor;
  readonly onClose: () => void;
}): ReactNode {
  const { t } = useAppState();
  const items: readonly [string, () => void][] = [
    [
      'left',
      () => {
        editor.align('left');
      },
    ],
    [
      'right',
      () => {
        editor.align('right');
      },
    ],
    [
      'top',
      () => {
        editor.align('top');
      },
    ],
    [
      'bottom',
      () => {
        editor.align('bottom');
      },
    ],
    [
      'centerX',
      () => {
        editor.align('centerX');
      },
    ],
    [
      'centerY',
      () => {
        editor.align('centerY');
      },
    ],
    [
      'distributeX',
      () => {
        editor.distribute('x');
      },
    ],
    [
      'distributeY',
      () => {
        editor.distribute('y');
      },
    ],
  ];
  return (
    <div className="popover board-arrange" role="menu" data-testid="board-arrange-menu">
      {items.map(([key, run]) => (
        <button
          key={key}
          type="button"
          role="menuitem"
          className="popover__item"
          data-testid={`arrange-${key}`}
          onClick={() => {
            run();
            onClose();
          }}
        >
          {t(`board.arrange.${key}` as MessageKey)}
        </button>
      ))}
    </div>
  );
}

/** Edit actions and board commands in the view toolbar. */
export function BoardActions({
  editor,
  commands,
  hasSelection,
}: {
  readonly editor: BoardEditor;
  readonly commands: BoardCommands;
  readonly hasSelection: boolean;
}): ReactNode {
  const { t } = useAppState();
  const drc = useSyncExternalStore(drcState.subscribe, drcState.get);
  const netsOpen = useSyncExternalStore(netInspector.subscribe, netInspector.get);
  const probing = useSyncExternalStore(crossProbing.subscribe, crossProbing.get);
  const corners = useSyncExternalStore(routingCorners.subscribe, routingCorners.get);
  const [arrange, setArrange] = useState(false);
  return (
    <>
      <span className="viewer-tool__separator" />
      <DimensionSelect editor={editor} />
      <LayerSelect editor={editor} />
      <Button
        icon="corner90"
        label={t('board.actions.corners')}
        tip={t('board.actions.cornersTip')}
        pressed={corners === '90'}
        testId="board-corners"
        onClick={() => {
          setRoutingCorners(corners === '90' ? '45' : '90');
          editor.refreshRoute();
        }}
      />
      <RouteModeSelect editor={editor} />
      <ViaSelect />
      <Button
        icon="rotate"
        label={t('board.actions.rotate')}
        tip={`${t('board.actions.rotate')} (R)`}
        disabled={!hasSelection}
        testId="board-rotate"
        onClick={() => {
          editor.rotateSelection();
        }}
      />
      <Button
        icon="flip"
        label={t('board.actions.flip')}
        tip={`${t('board.actions.flip')} (Shift+F)`}
        disabled={!hasSelection}
        testId="board-flip-side"
        onClick={() => {
          editor.flipSelection();
        }}
      />
      <Button
        icon="lock"
        label={t('board.actions.lock')}
        tip={`${t('board.actions.lock')} (L / Shift+L)`}
        disabled={!hasSelection}
        testId="board-lock"
        onClick={() => {
          editor.lockSelection(true);
        }}
      />
      <span className="board-arrange-anchor">
        <Button
          icon="alignLeft"
          label={t('board.actions.arrange')}
          tip={t('board.actions.arrange')}
          disabled={!hasSelection}
          pressed={arrange}
          testId="board-arrange"
          onClick={() => {
            setArrange(!arrange);
          }}
        />
        {arrange && (
          <ArrangeMenu
            editor={editor}
            onClose={() => {
              setArrange(false);
            }}
          />
        )}
      </span>
      <Button
        icon="trash"
        label={t('board.actions.delete')}
        tip={`${t('board.actions.delete')} (Del)`}
        disabled={!hasSelection}
        testId="board-delete"
        onClick={() => {
          editor.deleteSelection();
        }}
      />
      <span className="viewer-tool__separator" />
      <Button
        icon="sync"
        label={t('board.actions.update')}
        tip={t('board.actions.update')}
        testId="board-update"
        onClick={commands.update}
      />
      <Button
        icon="fill"
        label={t('board.actions.fill')}
        tip={`${t('board.actions.fill')} (Shift+B)`}
        testId="board-fill"
        onClick={commands.fillZones}
      />
      <Button
        icon="erc"
        label={t('board.actions.drc')}
        tip={t('board.actions.drc')}
        pressed={drc.open}
        testId="board-drc"
        onClick={commands.drc}
      />
      <Button
        icon="table"
        label={t('board.nets.open')}
        tip={t('board.nets.openTip')}
        pressed={netsOpen}
        testId="board-nets"
        onClick={() => {
          netInspector.set(!netInspector.get());
        }}
      />
      <Button
        icon="ratsnest"
        label={t('board.actions.ratsnest')}
        tip={t('board.actions.ratsnest')}
        pressed={drc.ratsnest}
        testId="board-ratsnest"
        onClick={() => {
          drcState.set({ ...drcState.get(), ratsnest: !drc.ratsnest });
        }}
      />
      <Button
        icon="schematic"
        label={t('board.actions.crossProbe')}
        tip={t('board.actions.crossProbe')}
        pressed={probing}
        testId="board-cross-probe"
        onClick={() => {
          crossProbing.set(!probing);
        }}
      />
      <Button
        icon="setup"
        label={t('board.actions.setup')}
        tip={t('board.actions.setup')}
        testId="board-setup"
        onClick={commands.setup}
      />
      <Button
        icon="rules"
        label={t('board.actions.rules')}
        tip={t('board.actions.rules')}
        testId="board-rules"
        onClick={commands.rules}
      />
      <Button
        icon="importFile"
        label={t('board.actions.dxf')}
        tip={t('board.actions.dxf')}
        testId="board-dxf"
        onClick={commands.importDxf}
      />
      <Button
        icon="image"
        label={t('board.picture.open')}
        tip={t('board.picture.openTip')}
        testId="board-picture"
        onClick={commands.picture}
      />
      <Button
        icon="layers"
        label={t('board.actions.gerberRef')}
        tip={t('board.actions.gerberRefTip')}
        testId="board-gerber-ref"
        onClick={commands.gerberReference}
      />
      <Button
        icon="channels"
        label={t('board.actions.channels')}
        tip={t('board.actions.channelsTip')}
        testId="board-channels"
        onClick={commands.channels}
      />
      <Button
        icon="stitch"
        label={t('board.actions.stitch')}
        tip={t('board.actions.stitchTip')}
        testId="board-stitch"
        onClick={commands.stitch}
      />
      <Button
        icon="teardrop"
        label={t('board.actions.teardrops')}
        tip={t('board.actions.teardropsTip')}
        testId="board-teardrops"
        onClick={commands.teardrops}
      />
      <Button
        icon="length"
        label={t('board.actions.lengths')}
        tip={t('board.actions.lengthsTip')}
        testId="board-lengths"
        onClick={commands.lengths}
      />
      <Button
        icon="track"
        label={t('board.actions.autorouter')}
        tip={t('board.actions.autorouterTip')}
        testId="board-autorouter"
        onClick={commands.autorouter}
      />
    </>
  );
}
