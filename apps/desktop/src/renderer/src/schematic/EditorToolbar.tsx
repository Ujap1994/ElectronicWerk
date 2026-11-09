import { useSyncExternalStore, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';

import { Icon, type IconName } from '../components/Icon';
import { useAppState } from '../state';
import type { SchematicEditor, Tool } from './editor';

interface ToolSpec {
  readonly tool: Tool;
  readonly icon: IconName;
  readonly key: string;
}

/** The drawing tools in toolbar order, with their keys. */
const TOOLS: readonly ToolSpec[] = [
  { tool: 'select', icon: 'pointer', key: 'Esc' },
  { tool: 'symbol', icon: 'chip', key: 'A' },
  { tool: 'wire', icon: 'wire', key: 'W' },
  { tool: 'bus', icon: 'bus', key: 'B' },
  { tool: 'label', icon: 'label', key: 'L' },
  { tool: 'globalLabel', icon: 'globalLabel', key: 'Shift+L' },
  { tool: 'hierarchicalLabel', icon: 'hierLabel', key: 'H' },
  { tool: 'power', icon: 'ground', key: 'P' },
  { tool: 'noConnect', icon: 'noConnect', key: 'Q' },
  { tool: 'junction', icon: 'junction', key: 'J' },
  { tool: 'text', icon: 'text', key: 'T' },
  { tool: 'sheet', icon: 'sheet', key: 'S' },
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

export interface EditorCommands {
  readonly chooseSymbol: () => void;
  readonly annotate: () => void;
  readonly erc: () => void;
  readonly find: () => void;
  readonly exportMenu: () => void;
  readonly grid: () => void;
  readonly guide: () => void;
}

/** The drawing tools (a column at the left edge of the drawing). */
export function ToolPalette({
  editor,
  commands,
}: {
  readonly editor: SchematicEditor;
  readonly commands: EditorCommands;
}): ReactNode {
  const { t } = useAppState();
  const state = useSyncExternalStore(editor.subscribe, editor.snapshot);
  return (
    <div
      className="tool-palette"
      role="toolbar"
      aria-orientation="vertical"
      aria-label={t('schematic.toolbar')}
      data-testid="tool-palette"
    >
      {TOOLS.map(({ tool, icon, key }) => {
        const label = t(`schematic.tools.${tool}` as MessageKey);
        return (
          <Button
            key={tool}
            icon={icon}
            label={label}
            tip={`${label} (${key})\n${t(`schematic.tips.${tool}` as MessageKey)}`}
            pressed={state.tool === tool}
            testId={`tool-${tool}`}
            onClick={() => {
              if (tool === 'symbol') {
                commands.chooseSymbol();
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

/** Edit actions and schematic commands in the view toolbar. */
export function EditActions({
  editor,
  commands,
  hasSelection,
}: {
  readonly editor: SchematicEditor;
  readonly commands: EditorCommands;
  readonly hasSelection: boolean;
}): ReactNode {
  const { t, appInfo } = useAppState();
  const state = useSyncExternalStore(editor.subscribe, editor.snapshot);
  const mod = appInfo.platform === 'darwin' ? '⌘' : 'Ctrl+';
  return (
    <>
      <span className="viewer-tool__separator" />
      <Button
        icon="rotate"
        label={t('schematic.actions.rotate')}
        tip={`${t('schematic.actions.rotate')} (R)`}
        disabled={!hasSelection}
        testId="action-rotate"
        onClick={() => {
          editor.rotateSelection();
        }}
      />
      <Button
        icon="mirrorH"
        label={t('schematic.actions.mirrorHorizontal')}
        tip={`${t('schematic.actions.mirrorHorizontal')} (Y)`}
        disabled={!hasSelection}
        testId="action-mirror-h"
        onClick={() => {
          editor.mirrorSelection('horizontal');
        }}
      />
      <Button
        icon="mirrorV"
        label={t('schematic.actions.mirrorVertical')}
        tip={`${t('schematic.actions.mirrorVertical')} (X)`}
        disabled={!hasSelection}
        testId="action-mirror-v"
        onClick={() => {
          editor.mirrorSelection('vertical');
        }}
      />
      <Button
        icon="trash"
        label={t('schematic.actions.delete')}
        tip={`${t('schematic.actions.delete')} (Del)`}
        disabled={!hasSelection}
        testId="action-delete"
        onClick={() => {
          editor.deleteSelection();
        }}
      />
      <span className="viewer-tool__separator" />
      <Button
        icon="annotate"
        label={t('schematic.actions.annotate')}
        tip={t('schematic.actions.annotate')}
        testId="action-annotate"
        onClick={commands.annotate}
      />
      <Button
        icon="erc"
        label={t('schematic.actions.erc')}
        tip={t('schematic.actions.erc')}
        testId="action-erc"
        onClick={commands.erc}
      />
      <Button
        icon="search"
        label={t('schematic.actions.find')}
        tip={`${t('schematic.actions.find')} (${mod}F)`}
        testId="action-find"
        onClick={commands.find}
      />
      <Button
        icon="output"
        label={t('schematic.actions.export')}
        tip={t('schematic.actions.export')}
        testId="action-export"
        onClick={commands.exportMenu}
      />
      <Button
        icon="grid"
        label={t('schematic.actions.grid')}
        tip={t('schematic.actions.grid')}
        pressed={state.snap}
        testId="action-grid"
        onClick={commands.grid}
      />
      <Button
        icon="guide"
        label={t('schematic.actions.guide')}
        tip={t('schematic.actions.guide')}
        testId="action-guide"
        onClick={commands.guide}
      />
    </>
  );
}
