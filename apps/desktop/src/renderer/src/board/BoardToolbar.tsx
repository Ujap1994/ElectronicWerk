import { useState, useSyncExternalStore, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';

import { Icon, type IconName } from '../components/Icon';
import { useAppState } from '../state';
import type { BoardEditor, BoardTool } from './editor';
import {
  crossProbing,
  drcState,
  routingCorners,
  setRoutingCorners,
  type BoardCommands,
} from './stores';

interface ToolSpec {
  readonly tool: BoardTool;
  readonly icon: IconName;
  readonly key: string;
}

/** The layout tools in toolbar order, with their keys. */
const TOOLS: readonly ToolSpec[] = [
  { tool: 'select', icon: 'pointer', key: 'Esc' },
  { tool: 'footprint', icon: 'chip', key: 'A' },
  { tool: 'route', icon: 'track', key: 'X' },
  { tool: 'via', icon: 'via', key: 'V' },
  { tool: 'zone', icon: 'zone', key: 'Z' },
  { tool: 'keepout', icon: 'keepout', key: 'K' },
  { tool: 'outline', icon: 'outline', key: 'O' },
  { tool: 'text', icon: 'text', key: 'T' },
  { tool: 'highlight', icon: 'highlight', key: 'N' },
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
  const { t } = useAppState();
  const state = useSyncExternalStore(editor.subscribe, editor.snapshot);
  return (
    <div
      className="tool-palette"
      role="toolbar"
      aria-orientation="vertical"
      aria-label={t('board.toolbar')}
      data-testid="board-tool-palette"
    >
      {TOOLS.map(({ tool, icon, key }) => {
        const label = t(`board.tools.${tool}` as MessageKey);
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
  const probing = useSyncExternalStore(crossProbing.subscribe, crossProbing.get);
  const corners = useSyncExternalStore(routingCorners.subscribe, routingCorners.get);
  const [arrange, setArrange] = useState(false);
  return (
    <>
      <span className="viewer-tool__separator" />
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
    </>
  );
}
