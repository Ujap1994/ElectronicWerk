import { describe, expect, it } from 'vitest';

import type { DocumentSummary, FileEntry, ProjectSnapshot } from '@shared/project';

import { buildTree, expandableKeys, selectionKey, visibleNodes, type TreeNode } from './tree';

const doc = (
  id: string,
  kind: DocumentSummary['kind'],
  children: string[] = [],
  dirty = false,
): DocumentSummary => ({
  id,
  kind,
  name: id,
  path: `${kind}/${id}`,
  dirty,
  stats: { objects: 0, ...(kind === 'schematic' ? { children } : {}) },
});

function project(documents: DocumentSummary[], root: string | null): ProjectSnapshot {
  return {
    id: 'p',
    name: 'P',
    revision: 'A',
    description: '',
    variables: {},
    path: '/p',
    manifestPath: '/p/P.ewproj',
    rootSchematic: root,
    settings: { units: 'mm', schematicGrid: 1, boardGrid: 1 },
    documents,
    history: {
      dirty: false,
      canUndo: false,
      canRedo: false,
      undoLabel: null,
      redoLabel: null,
      undoCount: 0,
      redoCount: 0,
    },
    needsUpgrade: false,
    recovery: null,
    journalError: null,
  };
}

const labels = (node: TreeNode): unknown => ({
  key: selectionKey(node.target),
  badges: node.badges,
  children: node.children.map(labels),
});

describe('buildTree', () => {
  it('shows the sheet hierarchy, unplaced sheets and documents by kind', () => {
    const files: FileEntry[] = [
      { path: 'README.md', size: 1, category: 'other' },
      { path: 'outputs/generated/a.gbr', size: 1, category: 'generated' },
      { path: 'P.ewproj', size: 1, category: 'manifest' },
    ];
    const tree = buildTree(
      project(
        [
          doc('root', 'schematic', ['a', 'b']),
          doc('a', 'schematic', ['c'], true),
          doc('b', 'schematic'),
          doc('c', 'schematic'),
          doc('loose', 'schematic'),
          doc('board', 'pcb'),
          doc('rules', 'rules'),
        ],
        'root',
      ),
      files,
    );
    const [schematics, boards, libraries, rules, , other, generated] = tree.children;
    expect(labels(schematics!)).toEqual({
      key: 'group:schematics',
      badges: [],
      children: [
        {
          key: 'doc:root',
          badges: ['top'],
          children: [
            {
              key: 'doc:a',
              badges: ['modified'],
              children: [{ key: 'doc:c', badges: [], children: [] }],
            },
            { key: 'doc:b', badges: [], children: [] },
          ],
        },
        { key: 'doc:loose', badges: ['unplaced'], children: [] },
      ],
    });
    expect(boards!.children.map((n) => selectionKey(n.target))).toEqual(['doc:board']);
    expect(libraries!.children).toEqual([]);
    expect(rules!.children).toHaveLength(1);
    expect(other!.children.map((n) => selectionKey(n.target))).toEqual(['file:README.md']);
    expect(generated!.children).toHaveLength(1);
  });

  it('survives cycles and missing children', () => {
    const tree = buildTree(
      project([doc('a', 'schematic', ['b', 'ghost']), doc('b', 'schematic', ['a'])], 'a'),
      [],
    );
    const a = tree.children[0]!.children[0]!;
    expect(a.children.map((n) => selectionKey(n.target))).toEqual(['doc:b']);
    expect(a.children[0]!.children).toEqual([]);
  });

  it('lists visible nodes and expandable keys', () => {
    const tree = buildTree(project([doc('r', 'schematic', ['s']), doc('s', 'schematic')], 'r'), []);
    const keys = expandableKeys(tree);
    expect(keys).toContain('project');
    expect(keys).toContain('group:schematics');
    expect(keys).toContain('group:schematics/r');
    expect(visibleNodes(tree, new Set(['project'])).length).toBe(8);
    const all = visibleNodes(tree, new Set(keys));
    expect(all.find((entry) => entry.node.key === 'group:schematics/r/s')?.depth).toBe(3);
  });
});
