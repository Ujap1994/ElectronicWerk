import type { DocumentKind, DocumentSummary, FileEntry, ProjectSnapshot } from '@shared/project';

export type GroupId =
  'schematics' | 'boards' | 'libraries' | 'rules' | 'outputJobs' | 'otherFiles' | 'generatedFiles';

export const GROUP_OF_KIND: Readonly<Record<DocumentKind, GroupId>> = {
  schematic: 'schematics',
  pcb: 'boards',
  library: 'libraries',
  rules: 'rules',
  outputJob: 'outputJobs',
};

export type Badge = 'top' | 'unplaced' | 'modified';

/** What a tree node stands for (also the selection target). */
export type TreeTarget =
  | { readonly type: 'project' }
  | { readonly type: 'group'; readonly group: GroupId }
  | { readonly type: 'document'; readonly document: DocumentSummary }
  | { readonly type: 'file'; readonly file: FileEntry }
  | { readonly type: 'git' };

export interface TreeNode {
  /** Unique key (a path of ids, as a sheet can be placed more than once). */
  readonly key: string;
  readonly target: TreeTarget;
  readonly badges: readonly Badge[];
  readonly children: readonly TreeNode[];
}

/** Selection key of a target: documents are selected by id wherever they appear. */
export function selectionKey(target: TreeTarget): string {
  switch (target.type) {
    case 'project':
      return 'project';
    case 'group':
      return `group:${target.group}`;
    case 'document':
      return `doc:${target.document.id}`;
    case 'file':
      return `file:${target.file.path}`;
    case 'git':
      return 'git';
  }
}

function documentNode(
  document: DocumentSummary,
  parentKey: string,
  extraBadges: readonly Badge[],
  byId: ReadonlyMap<string, DocumentSummary>,
  ancestors: ReadonlySet<string>,
): TreeNode {
  const key = `${parentKey}/${document.id}`;
  const nextAncestors = new Set(ancestors).add(document.id);
  const children = (document.stats.children ?? [])
    .map((id) => byId.get(id))
    .filter((child): child is DocumentSummary => child !== undefined)
    // A sheet that includes itself is shown once, not forever.
    .filter((child) => !nextAncestors.has(child.id))
    .map((child) => documentNode(child, key, [], byId, nextAncestors));
  return {
    key,
    target: { type: 'document', document },
    badges: document.dirty ? [...extraBadges, 'modified'] : extraBadges,
    children,
  };
}

function group(id: GroupId, children: readonly TreeNode[]): TreeNode {
  return { key: `group:${id}`, target: { type: 'group', group: id }, badges: [], children };
}

/**
 * Builds the project tree: schematic hierarchy, documents by kind, files
 * and source control (`gitChanges`: files changed since the last commit).
 */
export function buildTree(
  project: ProjectSnapshot,
  files: readonly FileEntry[],
  gitChanges = 0,
): TreeNode {
  const byId = new Map(project.documents.map((document) => [document.id, document]));
  const schematics = project.documents.filter((document) => document.kind === 'schematic');
  const placed = new Set(schematics.flatMap((document) => document.stats.children ?? []));

  const schematicNodes: TreeNode[] = [];
  const root = project.rootSchematic === null ? undefined : byId.get(project.rootSchematic);
  if (root !== undefined) {
    schematicNodes.push(documentNode(root, 'group:schematics', ['top'], byId, new Set()));
  }
  for (const sheet of schematics) {
    if (sheet.id !== root?.id && !placed.has(sheet.id)) {
      schematicNodes.push(documentNode(sheet, 'group:schematics', ['unplaced'], byId, new Set()));
    }
  }

  const ofKind = (kind: DocumentKind): TreeNode[] =>
    project.documents
      .filter((document) => document.kind === kind)
      .map((document) =>
        documentNode(document, `group:${GROUP_OF_KIND[kind]}`, [], byId, new Set()),
      );

  const fileNodes = (category: FileEntry['category'], groupId: GroupId): TreeNode[] =>
    files
      .filter((file) => file.category === category)
      .map((file) => ({
        key: `group:${groupId}/${file.path}`,
        target: { type: 'file', file },
        badges: [],
        children: [],
      }));

  return {
    key: 'project',
    target: { type: 'project' },
    badges: [],
    children: [
      group('schematics', schematicNodes),
      group('boards', ofKind('pcb')),
      group('libraries', ofKind('library')),
      group('rules', ofKind('rules')),
      group('outputJobs', ofKind('outputJob')),
      group('otherFiles', fileNodes('other', 'otherFiles')),
      group('generatedFiles', fileNodes('generated', 'generatedFiles')),
      {
        key: 'git',
        target: { type: 'git' },
        badges: gitChanges > 0 ? ['modified'] : [],
        children: [],
      },
    ],
  };
}

/** Nodes in display order, skipping children of collapsed nodes. */
export function visibleNodes(
  root: TreeNode,
  expanded: ReadonlySet<string>,
): { readonly node: TreeNode; readonly depth: number; readonly parent: TreeNode | null }[] {
  const result: { node: TreeNode; depth: number; parent: TreeNode | null }[] = [];
  const walk = (node: TreeNode, depth: number, parent: TreeNode | null): void => {
    result.push({ node, depth, parent });
    if (expanded.has(node.key)) {
      for (const child of node.children) {
        walk(child, depth + 1, node);
      }
    }
  };
  walk(root, 0, null);
  return result;
}

/** Keys of every node that has children (to expand everything). */
export function expandableKeys(root: TreeNode): string[] {
  const keys: string[] = [];
  const walk = (node: TreeNode): void => {
    if (node.children.length > 0) {
      keys.push(node.key);
      node.children.forEach(walk);
    }
  };
  walk(root);
  return keys;
}
