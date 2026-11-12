/**
 * The library editor tab of a project library: the list of its symbols,
 * footprints and components on the left, the editor of the chosen item in
 * the middle (drawing, pin or pad table, checks) and its properties on the
 * right. Every change is an engine transaction, so undo and redo work.
 */
import { useCallback, useMemo, useRef, useState, type ReactNode, type RefObject } from 'react';

import type { MessageKey } from '@shared/i18n';
import type { DocumentSummary, Units } from '@shared/project';

import { Icon } from '../components/Icon';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import type { ViewerController } from '../viewer/controller';
import { ChecksPanel } from './ChecksPanel';
import { ComponentEditor } from './ComponentEditor';
import { useLibraryItem, useLibraryList } from './data';
import { DeleteDialog, NameDialog } from './dialogs';
import { ItemCanvas } from './ItemCanvas';
import type { ItemEditor } from './item-editor';
import { ModelAlign } from './ModelAlign';
import { FootprintProps, FootprintSelection, SymbolProps, SymbolSelection } from './properties';
import { PadTable, PinTable } from './tables';
import {
  bodyOf,
  type Body,
  type ComponentDef,
  type Finding,
  type FootprintDef,
  type ItemRef,
  type ItemSummary,
  type LibraryKind,
  type LibraryList,
  type SymbolDef,
} from './types';
import { WizardDialog } from './WizardDialog';

const KINDS: readonly LibraryKind[] = ['symbol', 'footprint', 'component'];

function itemsOf(list: LibraryList | null, kind: LibraryKind): readonly ItemSummary[] {
  if (list === null) {
    return [];
  }
  switch (kind) {
    case 'symbol':
      return list.symbols;
    case 'footprint':
      return list.footprints;
    case 'component':
      return list.components;
  }
}

type Dialog =
  | { readonly type: 'new' }
  | { readonly type: 'duplicate'; readonly name: string }
  | { readonly type: 'rename'; readonly name: string }
  | { readonly type: 'delete'; readonly name: string }
  | { readonly type: 'wizard' };

type Handles = { controller: ViewerController; editor: ItemEditor } | null;

/** The engine calls shared by the item editors. */
function useItemActions(
  doc: string,
  kind: LibraryKind,
  name: string,
): {
  setProps: (props: Record<string, unknown>) => void;
  handles: RefObject<Handles>;
  setHandles: (handles: Handles) => void;
  run: (edit: Record<string, unknown>) => Promise<boolean>;
  select: (refs: ItemRef[]) => void;
} {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  const handlesRef = useRef<Handles>(null);
  const setHandles = useCallback((next: Handles) => {
    handlesRef.current = next;
  }, []);
  return {
    handles: handlesRef,
    setHandles,
    setProps: (props) => {
      engine.call('libedit.setProps', { doc, kind, name, props }).catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      });
    },
    run: (edit) => handlesRef.current?.editor.run(edit) ?? Promise.resolve(false),
    select: (refs) => {
      handlesRef.current?.editor.select(refs, true);
    },
  };
}

function letter(index: number): string {
  return index < 26 ? String.fromCharCode(65 + index) : String(index + 1);
}

/** Unit and body style of the symbol editor (toolbar). */
function UnitBar({
  def,
  body,
  run,
  onBody,
}: {
  readonly def: SymbolDef;
  readonly body: Body;
  readonly run: (edit: Record<string, unknown>) => Promise<boolean>;
  readonly onBody: (body: Body) => void;
}): ReactNode {
  const { t } = useAppState();
  const unit = def.units[body.unit - 1];
  const hasAlternate = unit?.alternate !== undefined;
  return (
    <span className="unit-bar" data-testid="unit-bar">
      <label className="item-canvas__field">
        <span>{t('libedit.units.label')}</span>
        <select
          value={body.unit}
          data-testid="unit-select"
          onChange={(event) => {
            onBody({ unit: Number(event.target.value), alternate: false });
          }}
        >
          {def.units.map((entry, index) => (
            <option key={letter(index)} value={index + 1}>
              {t('libedit.units.name', { name: entry.name ?? letter(index) })}
            </option>
          ))}
        </select>
      </label>
      <button
        type="button"
        className="viewer-tool"
        title={t('libedit.units.addCopy')}
        aria-label={t('libedit.units.addCopy')}
        data-testid="unit-add"
        onClick={() => {
          void run({ op: 'addUnit', copy: true });
        }}
      >
        <Icon name="plus" size={18} />
      </button>
      <button
        type="button"
        className="viewer-tool"
        title={t('libedit.units.remove')}
        aria-label={t('libedit.units.remove')}
        disabled={def.units.length < 2}
        data-testid="unit-remove"
        onClick={() => {
          void run({ op: 'removeUnit' });
        }}
      >
        <Icon name="trash" size={18} />
      </button>
      <span className="viewer-tool__separator" />
      <span className="segmented" role="group" aria-label={t('libedit.units.body')}>
        <button
          type="button"
          className={`segmented__item${body.alternate ? '' : ' segmented__item--active'}`}
          aria-pressed={!body.alternate}
          data-testid="body-normal"
          onClick={() => {
            onBody({ unit: body.unit, alternate: false });
          }}
        >
          {t('libedit.units.normal')}
        </button>
        <button
          type="button"
          className={`segmented__item${body.alternate ? ' segmented__item--active' : ''}`}
          aria-pressed={body.alternate}
          title={hasAlternate ? undefined : t('libedit.units.addAlternate')}
          data-testid="body-alternate"
          onClick={() => {
            if (hasAlternate) {
              onBody({ unit: body.unit, alternate: true });
            } else {
              void run({ op: 'addAlternate' });
            }
          }}
        >
          {hasAlternate ? t('libedit.units.alternate') : `+ ${t('libedit.units.alternate')}`}
        </button>
      </span>
      {hasAlternate && body.alternate && (
        <button
          type="button"
          className="viewer-tool"
          title={t('libedit.units.removeAlternate')}
          aria-label={t('libedit.units.removeAlternate')}
          data-testid="alternate-remove"
          onClick={() => {
            void run({ op: 'removeAlternate' }).then((ok) => {
              if (ok) {
                onBody({ unit: body.unit, alternate: false });
              }
            });
          }}
        >
          <Icon name="close" size={18} />
        </button>
      )}
    </span>
  );
}

/** The finding's pin or pad, selected in the drawing (on its body). */
function showFinding(
  finding: Finding,
  body: Body | null,
  setBody: ((body: Body) => void) | null,
  select: (refs: ItemRef[]) => void,
): void {
  const target = finding.target;
  if (target === undefined) {
    return;
  }
  if (setBody !== null && body !== null && target.unit !== undefined) {
    const wanted = { unit: target.unit, alternate: target.alternate === true };
    if (wanted.unit !== body.unit || wanted.alternate !== body.alternate) {
      setBody(wanted);
      return;
    }
  }
  if (target.index !== undefined && (target.kind === 'pin' || target.kind === 'pad')) {
    select([{ kind: target.kind, index: target.index }]);
  }
}

function BottomTabs({
  tab,
  tableLabel,
  findings,
  onTab,
}: {
  readonly tab: 'table' | 'checks';
  readonly tableLabel: string;
  readonly findings: readonly Finding[];
  readonly onTab: (tab: 'table' | 'checks') => void;
}): ReactNode {
  const { t } = useAppState();
  const problems = findings.filter((finding) => finding.severity !== 'info').length;
  return (
    <div className="libedit-item__tabs" role="tablist" aria-label={t('libedit.table.label')}>
      <button
        type="button"
        role="tab"
        className="tabs__tab"
        aria-selected={tab === 'table'}
        data-testid="tab-table"
        onClick={() => {
          onTab('table');
        }}
      >
        <Icon name="table" size={16} />
        <span>{tableLabel}</span>
      </button>
      <button
        type="button"
        role="tab"
        className="tabs__tab"
        aria-selected={tab === 'checks'}
        data-testid="tab-checks"
        onClick={() => {
          onTab('checks');
        }}
      >
        <Icon name={problems > 0 ? 'warning' : 'check'} size={16} />
        <span>{t('libedit.table.checks', { count: findings.length })}</span>
      </button>
    </div>
  );
}

function SymbolEditorView({
  doc,
  name,
  units,
  footprints,
  onRename,
  onOpen,
}: {
  readonly doc: string;
  readonly name: string;
  readonly units: Units;
  readonly footprints: readonly string[];
  readonly onRename: () => void;
  readonly onOpen: (kind: LibraryKind, name: string) => void;
}): ReactNode {
  const { t } = useAppState();
  const item = useLibraryItem<SymbolDef>(doc, 'symbol', name);
  const actions = useItemActions(doc, 'symbol', name);
  const [wanted, setWanted] = useState<Body>({ unit: 1, alternate: false });
  const [selection, setSelection] = useState<readonly ItemRef[]>([]);
  const [tab, setTab] = useState<'table' | 'checks'>('table');
  const def = item?.def ?? null;
  // The body shown: the wanted one if it exists (after undo it may not).
  const unitCount = def?.units.length ?? 1;
  const unit = Math.min(Math.max(wanted.unit, 1), unitCount);
  const body: Body = {
    unit,
    alternate: wanted.alternate && def?.units[unit - 1]?.alternate !== undefined,
  };
  const setBody = useCallback((next: Body) => {
    setWanted(next);
    setSelection([]);
  }, []);
  if (def === null || item === null) {
    return <p className="libedit__empty">{t('libedit.loading')}</p>;
  }
  const pins = bodyOf(def, body).pins ?? [];
  return (
    <div className="libedit-item" data-testid="symbol-editor">
      <div className="libedit-item__main">
        <ItemCanvas
          doc={doc}
          kind="symbol"
          name={name}
          body={body}
          units={units}
          pads={[]}
          onBody={setBody}
          onSelection={setSelection}
          controllerRef={actions.setHandles}
          toolbar={<UnitBar def={def} body={body} run={actions.run} onBody={setBody} />}
        />
        <div className="libedit-item__bottom">
          <BottomTabs
            tab={tab}
            tableLabel={t('libedit.table.pins', { count: pins.length })}
            findings={item.findings}
            onTab={setTab}
          />
          {tab === 'table' ? (
            <PinTable
              pins={pins}
              units={units}
              selection={selection}
              defaultKind="passive"
              defaultLength={2_540_000}
              onSelect={actions.select}
              run={actions.run}
            />
          ) : (
            <ChecksPanel
              doc={doc}
              findings={item.findings}
              onShow={(finding) => {
                showFinding(finding, body, setBody, actions.select);
              }}
              onOpen={onOpen}
            />
          )}
        </div>
      </div>
      <aside className="libedit-item__side">
        <SymbolSelection
          def={def}
          body={body}
          selection={selection}
          units={units}
          run={actions.run}
        />
        <SymbolProps
          def={def}
          usage={item.usage}
          units={units}
          footprints={footprints}
          setProps={actions.setProps}
          onRename={onRename}
        />
      </aside>
    </div>
  );
}

function FootprintEditorView({
  doc,
  name,
  units,
  onRename,
  onOpen,
}: {
  readonly doc: string;
  readonly name: string;
  readonly units: Units;
  readonly onRename: () => void;
  readonly onOpen: (kind: LibraryKind, name: string) => void;
}): ReactNode {
  const { t } = useAppState();
  const item = useLibraryItem<FootprintDef>(doc, 'footprint', name);
  const actions = useItemActions(doc, 'footprint', name);
  const [selection, setSelection] = useState<readonly ItemRef[]>([]);
  const [tab, setTab] = useState<'table' | 'checks'>('table');
  const body = useMemo<Body>(() => ({ unit: 1, alternate: false }), []);
  const noBody = useCallback(() => undefined, []);
  if (item === null) {
    return <p className="libedit__empty">{t('libedit.loading')}</p>;
  }
  const def = item.def;
  const pads = def.pads ?? [];
  return (
    <div className="libedit-item" data-testid="footprint-editor">
      <div className="libedit-item__main">
        <ItemCanvas
          doc={doc}
          kind="footprint"
          name={name}
          body={body}
          units={units}
          pads={pads}
          onBody={noBody}
          onSelection={setSelection}
          controllerRef={actions.setHandles}
        />
        <div className="libedit-item__bottom">
          <BottomTabs
            tab={tab}
            tableLabel={t('libedit.table.pads', { count: pads.length })}
            findings={item.findings}
            onTab={setTab}
          />
          {tab === 'table' ? (
            <PadTable
              pads={pads}
              units={units}
              selection={selection}
              onSelect={actions.select}
              run={actions.run}
            />
          ) : (
            <ChecksPanel
              doc={doc}
              findings={item.findings}
              onShow={(finding) => {
                showFinding(finding, null, null, actions.select);
              }}
              onOpen={onOpen}
            />
          )}
        </div>
      </div>
      <aside className="libedit-item__side">
        <FootprintSelection def={def} selection={selection} units={units} run={actions.run} />
        <FootprintProps
          def={def}
          usage={item.usage}
          units={units}
          setProps={actions.setProps}
          run={actions.run}
          onRename={onRename}
          modelPreview={(index) => {
            const model = def.models?.[index];
            return model === undefined ? null : <ModelAlign def={def} model={model} />;
          }}
        />
      </aside>
    </div>
  );
}

function ComponentView({
  doc,
  name,
  list,
  onRename,
  onOpen,
}: {
  readonly doc: string;
  readonly name: string;
  readonly list: LibraryList;
  readonly onRename: () => void;
  readonly onOpen: (kind: LibraryKind, name: string) => void;
}): ReactNode {
  const { t } = useAppState();
  const item = useLibraryItem<ComponentDef>(doc, 'component', name);
  const actions = useItemActions(doc, 'component', name);
  if (item === null) {
    return <p className="libedit__empty">{t('libedit.loading')}</p>;
  }
  return (
    <ComponentEditor
      doc={doc}
      def={item.def}
      usage={item.usage}
      findings={item.findings}
      symbols={list.symbols.map((entry) => entry.name)}
      footprints={list.footprints.map((entry) => entry.name)}
      setProps={actions.setProps}
      onOpen={onOpen}
      onRename={onRename}
    />
  );
}

function ItemList({
  kind,
  items,
  selected,
  query,
  onQuery,
  onSelect,
}: {
  readonly kind: LibraryKind;
  readonly items: readonly ItemSummary[];
  readonly selected: string | null;
  readonly query: string;
  readonly onQuery: (query: string) => void;
  readonly onSelect: (name: string) => void;
}): ReactNode {
  const { t } = useAppState();
  const needle = query.trim().toLowerCase();
  const shown =
    needle === ''
      ? items
      : items.filter(
          (item) =>
            item.name.toLowerCase().includes(needle) ||
            item.description.toLowerCase().includes(needle),
        );
  const empty: Readonly<Record<LibraryKind, MessageKey>> = {
    symbol: 'libedit.list.emptySymbols',
    footprint: 'libedit.list.emptyFootprints',
    component: 'libedit.list.emptyComponents',
  };
  return (
    <>
      <input
        type="search"
        className="field__control libedit__search"
        value={query}
        placeholder={t('libedit.list.search')}
        aria-label={t('libedit.list.search')}
        data-testid="libedit-search"
        onChange={(event) => {
          onQuery(event.target.value);
        }}
      />
      {shown.length === 0 && (
        <p className="libedit__empty">
          {items.length === 0 ? t(empty[kind]) : t('libedit.list.noMatch', { query })}
        </p>
      )}
      <ul className="libedit__items" aria-label={t('libedit.list.label')}>
        {shown.map((item) => {
          const facts =
            kind === 'symbol'
              ? [
                  t('libedit.list.pins', { count: item.pins ?? 0 }),
                  (item.units ?? 1) > 1
                    ? t('libedit.list.units', { count: item.units ?? 1 })
                    : null,
                  item.alternate === true ? t('libedit.list.alternate') : null,
                  item.power === true ? t('libedit.list.power') : null,
                ]
              : kind === 'footprint'
                ? [t('libedit.list.pads', { count: item.pads ?? 0 })]
                : [item.symbol ?? null, item.footprint ?? null, item.mpn ?? null];
          const problems = item.checks.errors + item.checks.warnings;
          return (
            <li key={item.name}>
              <button
                type="button"
                className="libedit__item"
                aria-current={item.name === selected}
                data-testid={`libedit-item-${item.name}`}
                onClick={() => {
                  onSelect(item.name);
                }}
              >
                <span className="libedit__item-name">{item.name}</span>
                {problems > 0 && (
                  <span
                    className={`pill ${item.checks.errors > 0 ? 'pill--error' : 'pill--warning'}`}
                    title={t('libedit.list.findings', {
                      errors: item.checks.errors,
                      warnings: item.checks.warnings,
                    })}
                  >
                    {problems}
                  </span>
                )}
                <span className="libedit__item-facts">
                  {facts.filter((fact) => fact !== null && fact !== '').join(' · ')}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    </>
  );
}

export function LibraryEditor({
  document,
  units,
}: {
  readonly document: DocumentSummary;
  readonly units: Units;
}): ReactNode {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  const doc = document.id;
  const list = useLibraryList(doc);
  const [kind, setKind] = useState<LibraryKind>('symbol');
  const [chosen, setChosen] = useState<Readonly<Record<LibraryKind, string | null>>>({
    symbol: null,
    footprint: null,
    component: null,
  });
  const [query, setQuery] = useState('');
  const [dialog, setDialog] = useState<Dialog | null>(null);
  const items = itemsOf(list, kind);
  const wanted = chosen[kind];
  const name =
    wanted !== null && items.some((item) => item.name === wanted)
      ? wanted
      : (items[0]?.name ?? null);
  const taken = items.map((item) => item.name);
  const usageItem = useLibraryItem<unknown>(
    doc,
    kind,
    dialog?.type === 'delete' ? dialog.name : null,
  );

  const choose = (nextKind: LibraryKind, next: string): void => {
    setKind(nextKind);
    setChosen((current) => ({ ...current, [nextKind]: next }));
  };
  const fail = (error: unknown): void => {
    notify(errorMessage(t, error), 'error');
  };
  const create = (newName: string, params: Record<string, unknown>): void => {
    engine
      .call<{ name: string }>('libedit.create', { doc, kind, name: newName, ...params })
      .then((reply) => {
        setDialog(null);
        choose(kind, reply.name);
      }, fail);
  };
  const rename = (old: string): void => {
    setDialog({ type: 'rename', name: old });
  };

  let content: ReactNode;
  if (list === null) {
    content = <p className="libedit__empty">{t('libedit.loading')}</p>;
  } else if (name === null) {
    content = <p className="libedit__empty">{t('libedit.none')}</p>;
  } else if (kind === 'symbol') {
    content = (
      <SymbolEditorView
        key={name}
        doc={doc}
        name={name}
        units={units}
        footprints={list.footprints.map((entry) => entry.name)}
        onRename={() => {
          rename(name);
        }}
        onOpen={choose}
      />
    );
  } else if (kind === 'footprint') {
    content = (
      <FootprintEditorView
        key={name}
        doc={doc}
        name={name}
        units={units}
        onRename={() => {
          rename(name);
        }}
        onOpen={choose}
      />
    );
  } else {
    content = (
      <ComponentView
        key={name}
        doc={doc}
        name={name}
        list={list}
        onRename={() => {
          rename(name);
        }}
        onOpen={choose}
      />
    );
  }

  return (
    <div className="libedit" data-testid="library-editor" data-kind={kind}>
      <aside className="libedit__list" aria-label={t('libedit.list.label')}>
        <div className="segmented libedit__kinds" role="tablist">
          {KINDS.map((entry) => (
            <button
              key={entry}
              type="button"
              role="tab"
              aria-selected={kind === entry}
              className={`segmented__item${kind === entry ? ' segmented__item--active' : ''}`}
              data-testid={`libedit-kind-${entry}`}
              onClick={() => {
                setKind(entry);
              }}
            >
              {t(`libedit.kinds.${entry}`)}
              <span className="libedit__count">{itemsOf(list, entry).length}</span>
            </button>
          ))}
        </div>
        <div className="libedit__actions">
          <button
            type="button"
            className="button button--small"
            data-testid="libedit-new"
            onClick={() => {
              setDialog({ type: 'new' });
            }}
          >
            <Icon name="plus" size={14} />
            {t('libedit.list.new')}
          </button>
          {kind === 'footprint' && (
            <button
              type="button"
              className="button button--small"
              data-testid="libedit-wizard"
              onClick={() => {
                setDialog({ type: 'wizard' });
              }}
            >
              <Icon name="wand" size={14} />
              {t('libedit.list.wizard')}
            </button>
          )}
          <button
            type="button"
            className="button button--small button--quiet"
            disabled={name === null}
            title={t('libedit.list.duplicate')}
            aria-label={t('libedit.list.duplicate')}
            data-testid="libedit-duplicate"
            onClick={() => {
              if (name !== null) {
                setDialog({ type: 'duplicate', name });
              }
            }}
          >
            <Icon name="duplicate" size={14} />
          </button>
          <button
            type="button"
            className="button button--small button--quiet"
            disabled={name === null}
            title={t('libedit.list.delete')}
            aria-label={t('libedit.list.delete')}
            data-testid="libedit-delete"
            onClick={() => {
              if (name !== null) {
                setDialog({ type: 'delete', name });
              }
            }}
          >
            <Icon name="trash" size={14} />
          </button>
        </div>
        {list !== null && (
          <ItemList
            kind={kind}
            items={items}
            selected={name}
            query={query}
            onQuery={setQuery}
            onSelect={(next) => {
              choose(kind, next);
            }}
          />
        )}
      </aside>
      <div className="libedit__main">{content}</div>
      {dialog?.type === 'new' && (
        <NameDialog
          title={t('libedit.dialogs.newTitle', { kind: t(`libedit.kind.${kind}`) })}
          initial=""
          taken={taken}
          confirm={t('libedit.dialogs.create')}
          onSubmit={(newName) => {
            create(newName, {});
          }}
          onClose={() => {
            setDialog(null);
          }}
        />
      )}
      {dialog?.type === 'duplicate' && (
        <NameDialog
          title={t('libedit.dialogs.duplicateTitle', { name: dialog.name })}
          initial={`${dialog.name}_copy`}
          taken={taken}
          confirm={t('libedit.dialogs.create')}
          onSubmit={(newName) => {
            create(newName, { copyOf: dialog.name });
          }}
          onClose={() => {
            setDialog(null);
          }}
        />
      )}
      {dialog?.type === 'rename' && (
        <NameDialog
          title={t('libedit.dialogs.renameTitle', { name: dialog.name })}
          initial={dialog.name}
          hint={t('libedit.dialogs.renameHint')}
          taken={taken}
          confirm={t('libedit.list.rename')}
          onSubmit={(newName) => {
            engine
              .call<{ name: string; changes: number }>('libedit.rename', {
                doc,
                kind,
                name: dialog.name,
                new: newName,
              })
              .then((reply) => {
                setDialog(null);
                choose(kind, reply.name);
                if (reply.changes > 1) {
                  notify(
                    t('libedit.dialogs.renamed', { name: reply.name, changes: reply.changes }),
                  );
                }
              }, fail);
          }}
          onClose={() => {
            setDialog(null);
          }}
        />
      )}
      {dialog?.type === 'delete' && (
        <DeleteDialog
          name={dialog.name}
          usage={usageItem?.usage ?? null}
          onConfirm={() => {
            engine
              .call('libedit.delete', { doc, kind, name: dialog.name, force: true })
              .then(() => {
                setDialog(null);
              }, fail);
          }}
          onClose={() => {
            setDialog(null);
          }}
        />
      )}
      {dialog?.type === 'wizard' && (
        <WizardDialog
          doc={doc}
          units={units}
          taken={list?.footprints.map((entry) => entry.name) ?? []}
          onCreated={(created) => {
            setDialog(null);
            choose('footprint', created);
          }}
          onClose={() => {
            setDialog(null);
          }}
        />
      )}
    </div>
  );
}
