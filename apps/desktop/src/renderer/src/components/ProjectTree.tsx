import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from 'react';

import type { MessageKey } from '@shared/i18n';
import { baseName, type DocumentKind } from '@shared/project';

import { useProject } from '../project/ProjectProvider';
import {
  buildTree,
  expandableKeys,
  selectionKey,
  visibleNodes,
  type Badge,
  type GroupId,
  type TreeNode,
  type TreeTarget,
} from '../project/tree';
import { useAppState } from '../state';
import { isViewable, useOpenDocument } from '../viewer/open-context';
import { Icon, type IconName } from './Icon';

const GROUP_LABEL: Readonly<Record<GroupId, MessageKey>> = {
  schematics: 'tree.schematics',
  boards: 'tree.boards',
  libraries: 'tree.libraries',
  rules: 'tree.rules',
  outputJobs: 'tree.outputJobs',
  otherFiles: 'tree.otherFiles',
  generatedFiles: 'tree.generatedFiles',
};

const KIND_ICON: Readonly<Record<DocumentKind, IconName>> = {
  schematic: 'schematic',
  pcb: 'board',
  library: 'library',
  rules: 'rules',
  outputJob: 'output',
};

const GROUP_ICON: Readonly<Record<GroupId, IconName>> = {
  schematics: 'schematic',
  boards: 'board',
  libraries: 'library',
  rules: 'rules',
  outputJobs: 'output',
  otherFiles: 'folder',
  generatedFiles: 'output',
};

const BADGE_LABEL: Readonly<Record<Badge, MessageKey>> = {
  top: 'tree.topSheet',
  unplaced: 'tree.unplaced',
  modified: 'tree.modified',
};

/** Which document a group's "add" action creates, and its default name. */
const GROUP_ADD: Partial<
  Readonly<Record<GroupId, { kind: DocumentKind; label: MessageKey; name: MessageKey }>>
> = {
  schematics: { kind: 'schematic', label: 'tree.addSheet', name: 'tree.newSheet' },
  boards: { kind: 'pcb', label: 'tree.addBoard', name: 'tree.newBoard' },
  libraries: { kind: 'library', label: 'tree.addLibrary', name: 'tree.newLibrary' },
  rules: { kind: 'rules', label: 'tree.addRules', name: 'tree.newRules' },
  outputJobs: { kind: 'outputJob', label: 'tree.addOutputJob', name: 'tree.newOutputJob' },
};

interface MenuAction {
  readonly id: string;
  readonly label: string;
  readonly icon: IconName;
  readonly danger?: boolean;
  readonly run: () => void;
}

interface ProjectTreeProps {
  readonly selected: string;
  readonly onSelect: (target: TreeTarget) => void;
}

export function ProjectTree({ selected, onSelect }: ProjectTreeProps): ReactNode {
  const { t } = useAppState();
  const {
    project,
    files,
    addDocument,
    renameDocument,
    requestRemove,
    showInFolder,
    importNetlist,
    importLibrary,
  } = useProject();
  const open = useOpenDocument();
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(() => new Set());
  const [renaming, setRenaming] = useState<string | null>(null);
  const [menu, setMenu] = useState<{
    readonly key: string;
    readonly x: number;
    readonly y: number;
  } | null>(null);
  const treeRef = useRef<HTMLUListElement>(null);

  const tree = useMemo(
    () => (project === null ? null : buildTree(project, files)),
    [project, files],
  );
  const expanded = useMemo(
    () => new Set(tree === null ? [] : expandableKeys(tree).filter((key) => !collapsed.has(key))),
    [tree, collapsed],
  );
  const rows = useMemo(() => (tree === null ? [] : visibleNodes(tree, expanded)), [tree, expanded]);

  // Close the context menu on Escape or a click elsewhere.
  useEffect(() => {
    if (menu === null) {
      return undefined;
    }
    const close = (event: Event): void => {
      if (event instanceof KeyboardEvent && event.key !== 'Escape') {
        return;
      }
      if (
        event instanceof MouseEvent &&
        event.target instanceof Element &&
        event.target.closest('.menu') !== null
      ) {
        return;
      }
      setMenu(null);
    };
    window.addEventListener('mousedown', close);
    window.addEventListener('keydown', close);
    return () => {
      window.removeEventListener('mousedown', close);
      window.removeEventListener('keydown', close);
    };
  }, [menu]);

  if (project === null || tree === null) {
    return null;
  }

  const label = (node: TreeNode): string => {
    switch (node.target.type) {
      case 'project':
        return project.name;
      case 'group':
        return t(GROUP_LABEL[node.target.group]);
      case 'document':
        return node.target.document.name;
      case 'file':
        return baseName(node.target.file.path);
    }
  };

  const icon = (node: TreeNode): IconName => {
    switch (node.target.type) {
      case 'project':
        return 'project';
      case 'group':
        return GROUP_ICON[node.target.group];
      case 'document':
        return KIND_ICON[node.target.document.kind];
      case 'file':
        return 'file';
    }
  };

  const add = (kind: DocumentKind, name: MessageKey, parent?: string): void => {
    void addDocument(kind, t(name), parent).then((id) => {
      if (id !== null) {
        setRenaming(id);
        onSelect({
          type: 'document',
          document: { id, kind, name: t(name), path: '', dirty: true, stats: { objects: 0 } },
        });
      }
    });
  };

  const actions = (node: TreeNode): MenuAction[] => {
    const target = node.target;
    const reveal = (path?: string): MenuAction => ({
      id: 'reveal',
      label: t('tree.showInFolder'),
      icon: 'reveal',
      run: () => {
        showInFolder(path);
      },
    });
    const addFor = (group: GroupId): MenuAction[] => {
      const spec = GROUP_ADD[group];
      if (spec === undefined) {
        return [];
      }
      // One board and one rule set per project for now.
      const single = spec.kind === 'pcb' || spec.kind === 'rules';
      if (single && project.documents.some((document) => document.kind === spec.kind)) {
        return [];
      }
      return [
        {
          id: `add-${group}`,
          label: t(spec.label),
          icon: 'plus',
          run: () => {
            add(spec.kind, spec.name);
          },
        },
      ];
    };
    const libraryImports = (doc?: string): MenuAction[] => [
      {
        id: 'import-library-file',
        label: t('libraryImport.file'),
        icon: 'library',
        run: () => {
          void importLibrary('kicadLibraryFile', doc);
        },
      },
      {
        id: 'import-library-folder',
        label: t('libraryImport.folder'),
        icon: 'folder',
        run: () => {
          void importLibrary('kicadLibraryFolder', doc);
        },
      },
    ];
    switch (target.type) {
      case 'project':
        return [
          ...addFor('schematics'),
          ...addFor('boards'),
          ...addFor('libraries'),
          ...addFor('outputJobs'),
          reveal(),
        ];
      case 'group':
        return [
          ...addFor(target.group),
          ...(target.group === 'libraries' ? libraryImports() : []),
          reveal(),
        ];
      case 'document': {
        const { document } = target;
        return [
          ...(open !== null && isViewable(document)
            ? [
                {
                  id: 'open',
                  label: t('viewer.open'),
                  icon: 'eye' as const,
                  run: () => {
                    open(document);
                  },
                },
              ]
            : []),
          ...(document.kind === 'pcb'
            ? [
                {
                  id: 'import-netlist',
                  label: t('tree.importNetlist'),
                  icon: 'output' as const,
                  run: () => {
                    void importNetlist(document.id);
                  },
                },
              ]
            : []),
          ...(document.kind === 'library' ? libraryImports(document.id) : []),
          ...(document.kind === 'schematic'
            ? [
                {
                  id: 'add-sub-sheet',
                  label: t('tree.addSubSheet'),
                  icon: 'plus' as const,
                  run: () => {
                    add('schematic', 'tree.newSheet', document.id);
                  },
                },
              ]
            : []),
          {
            id: 'rename',
            label: t('tree.rename'),
            icon: 'edit',
            run: () => {
              setRenaming(document.id);
            },
          },
          reveal(document.path),
          {
            id: 'remove',
            label: t('tree.remove'),
            icon: 'trash',
            danger: true,
            run: () => {
              requestRemove(document.id);
            },
          },
        ];
      }
      case 'file':
        return [reveal(target.file.path)];
    }
  };

  const toggle = (key: string): void => {
    setCollapsed((current) => {
      const next = new Set(current);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  };

  const selectedIndex = Math.max(
    rows.findIndex((row) => selectionKey(row.node.target) === selected),
    0,
  );

  const focusRow = (index: number): void => {
    const row = rows[index];
    if (row !== undefined) {
      onSelect(row.node.target);
      requestAnimationFrame(() => {
        treeRef.current?.querySelector<HTMLElement>(`[data-row="${String(index)}"]`)?.focus();
      });
    }
  };

  const onKeyDown = (event: ReactKeyboardEvent): void => {
    const row = rows[selectedIndex];
    if (row === undefined || renaming !== null) {
      return;
    }
    const { node, parent } = row;
    const hasChildren = node.children.length > 0;
    switch (event.key) {
      case 'ArrowDown':
        focusRow(Math.min(selectedIndex + 1, rows.length - 1));
        break;
      case 'ArrowUp':
        focusRow(Math.max(selectedIndex - 1, 0));
        break;
      case 'Home':
        focusRow(0);
        break;
      case 'End':
        focusRow(rows.length - 1);
        break;
      case 'ArrowRight':
        if (hasChildren && !expanded.has(node.key)) {
          toggle(node.key);
        } else if (hasChildren) {
          focusRow(selectedIndex + 1);
        }
        break;
      case 'ArrowLeft':
        if (hasChildren && expanded.has(node.key)) {
          toggle(node.key);
        } else if (parent !== null) {
          focusRow(rows.findIndex((entry) => entry.node === parent));
        }
        break;
      case 'Enter':
        if (node.target.type === 'document' && open !== null && isViewable(node.target.document)) {
          open(node.target.document);
        } else if (hasChildren) {
          toggle(node.key);
        }
        break;
      case 'F2':
        if (node.target.type === 'document') {
          setRenaming(node.target.document.id);
        }
        break;
      case 'Delete':
      case 'Backspace':
        if (node.target.type === 'document') {
          requestRemove(node.target.document.id);
        }
        break;
      case 'ContextMenu': {
        const element = treeRef.current?.querySelector(`[data-row="${String(selectedIndex)}"]`);
        const rect = element?.getBoundingClientRect();
        setMenu({ key: node.key, x: (rect?.left ?? 0) + 24, y: rect?.bottom ?? 0 });
        break;
      }
      default:
        return;
    }
    event.preventDefault();
  };

  const menuRow = menu === null ? undefined : rows.find((row) => row.node.key === menu.key);
  const menuActions = menuRow === undefined ? [] : actions(menuRow.node);

  return (
    <nav className="tree" aria-label={t('tree.label')}>
      <ul
        ref={treeRef}
        className="tree__list"
        role="tree"
        aria-label={t('tree.label')}
        onKeyDown={onKeyDown}
      >
        {rows.map(({ node, depth }, index) => {
          const key = selectionKey(node.target);
          const isSelected = key === selected;
          const hasChildren = node.children.length > 0;
          const isOpen = expanded.has(node.key);
          const renameTarget =
            node.target.type === 'document' && node.target.document.id === renaming
              ? node.target.document
              : null;
          const isEmptyGroup = node.target.type === 'group' && !hasChildren;
          return (
            <li
              key={node.key}
              role="treeitem"
              aria-level={depth + 1}
              aria-selected={isSelected}
              aria-expanded={hasChildren ? isOpen : undefined}
              tabIndex={index === selectedIndex ? 0 : -1}
              className="tree__row"
              data-row={index}
              data-testid={`tree-${key}`}
              data-selected={isSelected}
              style={{ paddingLeft: `${String(4 + depth * 14)}px` }}
              onClick={() => {
                onSelect(node.target);
              }}
              onDoubleClick={() => {
                if (
                  node.target.type === 'document' &&
                  open !== null &&
                  isViewable(node.target.document)
                ) {
                  open(node.target.document);
                } else if (hasChildren) {
                  toggle(node.key);
                }
              }}
              onContextMenu={(event) => {
                event.preventDefault();
                onSelect(node.target);
                setMenu({ key: node.key, x: event.clientX, y: event.clientY });
              }}
            >
              <span
                className="tree__twisty"
                data-open={isOpen}
                data-hidden={!hasChildren}
                aria-hidden="true"
                onClick={(event) => {
                  event.stopPropagation();
                  toggle(node.key);
                }}
              >
                <Icon name="chevron" size={14} />
              </span>
              <Icon name={icon(node)} size={16} className="tree__icon" />
              {renameTarget !== null ? (
                <RenameInput
                  initial={renameTarget.name}
                  label={t('tree.renameLabel', { name: renameTarget.name })}
                  onDone={(name) => {
                    setRenaming(null);
                    if (name !== null) {
                      void renameDocument(renameTarget.id, name);
                    }
                    treeRef.current
                      ?.querySelector<HTMLElement>(`[data-row="${String(index)}"]`)
                      ?.focus();
                  }}
                />
              ) : (
                <span className="tree__label" data-muted={isEmptyGroup}>
                  {label(node)}
                </span>
              )}
              {node.badges.map((badge) => (
                <span
                  key={badge}
                  className={`tree__badge tree__badge--${badge}`}
                  title={t(BADGE_LABEL[badge])}
                >
                  {badge === 'modified' ? '●' : t(BADGE_LABEL[badge])}
                </span>
              ))}
              {node.target.type === 'group' &&
                node.target.group !== 'otherFiles' &&
                node.target.group !== 'generatedFiles' && (
                  <span className="tree__count">{node.children.length}</span>
                )}
              <button
                type="button"
                className="tree__more"
                tabIndex={-1}
                aria-label={t('tree.actions', { name: label(node) })}
                title={t('tree.actions', { name: label(node) })}
                data-testid={`tree-actions-${key}`}
                onClick={(event) => {
                  event.stopPropagation();
                  onSelect(node.target);
                  const rect = event.currentTarget.getBoundingClientRect();
                  setMenu({ key: node.key, x: rect.left, y: rect.bottom + 2 });
                }}
              >
                <Icon name="more" size={16} />
              </button>
            </li>
          );
        })}
      </ul>
      {menu !== null && menuActions.length > 0 && (
        <div
          className="menu"
          role="menu"
          style={{ left: `${String(menu.x)}px`, top: `${String(menu.y)}px` }}
          data-testid="tree-menu"
        >
          {menuActions.map((action) => (
            <button
              key={action.id}
              type="button"
              role="menuitem"
              className="menu__item"
              data-danger={action.danger === true}
              data-testid={`menu-${action.id}`}
              onClick={() => {
                setMenu(null);
                action.run();
              }}
            >
              <Icon name={action.icon} size={16} />
              {action.label}
            </button>
          ))}
        </div>
      )}
    </nav>
  );
}

function RenameInput({
  initial,
  label,
  onDone,
}: {
  readonly initial: string;
  readonly label: string;
  readonly onDone: (name: string | null) => void;
}): ReactNode {
  const [value, setValue] = useState(initial);
  const ref = useRef<HTMLInputElement>(null);
  const doneRef = useRef(false);
  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);
  const finish = (commit: boolean): void => {
    if (doneRef.current) {
      return;
    }
    doneRef.current = true;
    const name = value.trim();
    onDone(commit && name !== '' && name !== initial ? name : null);
  };
  return (
    <input
      ref={ref}
      className="tree__rename"
      value={value}
      aria-label={label}
      data-testid="tree-rename"
      onChange={(event) => {
        setValue(event.target.value);
      }}
      onClick={(event) => {
        event.stopPropagation();
      }}
      onKeyDown={(event) => {
        event.stopPropagation();
        if (event.key === 'Enter') {
          finish(true);
        } else if (event.key === 'Escape') {
          event.preventDefault();
          finish(false);
        }
      }}
      onBlur={() => {
        finish(true);
      }}
    />
  );
}
