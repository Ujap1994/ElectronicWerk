/**
 * The system editor (M18): the boards of a system, the connectors that
 * mate directly, the cables, the connection check across the boards and
 * the system in 3D.
 */
import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';

import type { MessageKey } from '@shared/i18n';
import type { DocumentSummary } from '@shared/project';

import { Icon } from '../components/Icon';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { Board3dView } from '../view3d/Board3dView';
import { NumberField } from '../view3d/MechanicalPanel';
import {
  PAIRINGS,
  shortName,
  type Cable,
  type ConnectorInfo,
  type ConnectorRef,
  type Link,
  type SystemBoardInfo,
  type SystemCheck,
  type SystemFinding,
  type SystemInfo,
  type Wire,
} from './types';

type Tab = 'boards' | 'connections' | 'cables' | 'check' | 'view3d';
const TABS: readonly Tab[] = ['boards', 'connections', 'cables', 'check', 'view3d'];

type Translate = ReturnType<typeof useAppState>['t'];

/** Millimetres with at most three decimals. */
function fromNm(nm: number | undefined): number {
  return Math.round((nm ?? 0) / 1000) / 1000;
}

/** The translated message of a finding. */
export function systemFindingMessage(t: Translate, finding: SystemFinding): string {
  const variant = finding.args['variant'];
  const key = (
    variant === undefined
      ? `system.messages.${finding.code}`
      : `system.messages.${finding.code}-${variant}`
  ) as MessageKey;
  const text = t(key, finding.args);
  if (text === key) {
    return finding.message;
  }
  const cable = finding.args['cable'];
  return cable === undefined ||
    cable === '' ||
    finding.code.startsWith('system-cable') ||
    finding.code === 'system-connector'
    ? text
    : `${text}${t('system.viaCable', { cable })}`;
}

function boardName(info: SystemInfo, id: string): string {
  return info.boards.find((b) => b.id === id)?.name ?? '?';
}

/** A connector chooser: a board and one of its connectors. */
function ConnectorPicker({
  info,
  value,
  connectors,
  onChange,
  testId,
  allowFree = false,
}: {
  readonly info: SystemInfo;
  readonly value: ConnectorRef | undefined;
  readonly connectors: (board: string) => readonly ConnectorInfo[];
  readonly onChange: (value: ConnectorRef | undefined) => void;
  readonly testId: string;
  readonly allowFree?: boolean;
}): ReactNode {
  const { t } = useAppState();
  const list = value === undefined ? [] : connectors(value.board);
  return (
    <span className="system__picker">
      <select
        className="input"
        value={value?.board ?? ''}
        data-testid={`${testId}-board`}
        aria-label={t('system.link.boardA')}
        onChange={(event) => {
          const board = event.target.value;
          if (board === '') {
            onChange(undefined);
            return;
          }
          const first = connectors(board)[0];
          onChange({ board, designator: first?.designator ?? '' });
        }}
      >
        {(allowFree || value === undefined) && (
          <option value="">{allowFree ? t('system.cable.free') : t('system.link.choose')}</option>
        )}
        {info.boards.map((b) => (
          <option key={b.id} value={b.id}>
            {b.name}
          </option>
        ))}
      </select>
      {value !== undefined && (
        <select
          className="input"
          value={value.designator}
          data-testid={`${testId}-connector`}
          aria-label={t('system.link.connector')}
          onChange={(event) => {
            onChange({ board: value.board, designator: event.target.value });
          }}
        >
          {!list.some((c) => c.designator === value.designator) && (
            <option value={value.designator}>{value.designator || '—'}</option>
          )}
          {list.map((c) => (
            <option key={c.designator} value={c.designator}>
              {c.designator} · {c.value} ({c.pins})
            </option>
          ))}
        </select>
      )}
    </span>
  );
}

function Status({ status }: { readonly status: string }): ReactNode {
  const { t } = useAppState();
  const bad = !['ok', 'unused'].includes(status);
  return (
    <span className={`system__status system__status--${bad ? 'bad' : status}`}>
      {t(`system.pins.statuses.${status}` as MessageKey)}
    </span>
  );
}

function PinTable({
  pairs,
  testId,
}: {
  readonly pairs: SystemCheck['connections'][number]['pairs'];
  readonly testId: string;
}): ReactNode {
  const { t } = useAppState();
  return (
    <table className="system__table" data-testid={testId}>
      <thead>
        <tr>
          <th>{t('system.pins.pinA')}</th>
          <th>{t('system.pins.signalA')}</th>
          <th>{t('system.pins.pinB')}</th>
          <th>{t('system.pins.signalB')}</th>
          <th>{t('system.pins.status')}</th>
        </tr>
      </thead>
      <tbody>
        {pairs.map((p, i) => (
          <tr key={`${p.a ?? ''}-${p.b ?? ''}-${String(i)}`} data-status={p.status}>
            <td>{p.a ?? '—'}</td>
            <td>{shortName(p.aNet)}</td>
            <td>{p.b ?? '—'}</td>
            <td>{shortName(p.bNet)}</td>
            <td>
              <Status status={p.status} />
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

export function SystemEditor({ document }: { readonly document: DocumentSummary }): ReactNode {
  const { engine, t } = useAppState();
  const { project, notify } = useProject();
  const doc = document.id;
  const [tab, setTab] = useState<Tab>('boards');
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [check, setCheck] = useState<SystemCheck | null>(null);
  const [checking, setChecking] = useState(false);
  const [connectorLists, setConnectorLists] = useState<Record<string, readonly ConnectorInfo[]>>(
    {},
  );
  const [selectedLink, setSelectedLink] = useState<string | null>(null);
  const [selectedCable, setSelectedCable] = useState<string | null>(null);
  const [menu, setMenu] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [showAccepted, setShowAccepted] = useState(false);
  const runRef = useRef<() => void>(() => undefined);

  const flash = useCallback((text: string) => {
    setMessage(text);
    setTimeout(() => {
      setMessage((current) => (current === text ? null : current));
    }, 8000);
  }, []);

  const call = useCallback(
    async <T,>(method: string, params: Record<string, unknown>): Promise<T | null> => {
      try {
        return await engine.call<T>(method, { doc, ...params }, 300_000);
      } catch (error) {
        notify(errorMessage(t, error), 'error');
        return null;
      }
    },
    [engine, doc, notify, t],
  );

  // The system and its check, again after every change of the project.
  useEffect(() => {
    let cancelled = false;
    const stale = (): boolean => cancelled;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = async (): Promise<void> => {
      try {
        const next = await engine.call<SystemInfo>('system.info', { doc });
        if (cancelled) {
          return;
        }
        setInfo(next);
        setChecking(true);
        const checked = await engine.call<SystemCheck>('system.check', { doc }, 300_000);
        if (!stale()) {
          setCheck(checked);
        }
      } catch {
        // The document was removed.
      } finally {
        if (!cancelled) {
          setChecking(false);
        }
      }
    };
    void load();
    runRef.current = () => {
      void load();
    };
    const off = engine.onEvent((event) => {
      if ((event as { type?: string } | null)?.type === 'changed') {
        clearTimeout(timer);
        timer = setTimeout(() => {
          void load();
        }, 300);
      }
    });
    return () => {
      cancelled = true;
      clearTimeout(timer);
      off();
    };
  }, [engine, doc]);

  // Connector lists of the boards (for the pickers).
  useEffect(() => {
    if (info === null) {
      return;
    }
    let cancelled = false;
    void Promise.all(
      info.boards
        .filter((b) => b.loaded)
        .map(async (b) => {
          try {
            const reply = await engine.call<{ connectors: ConnectorInfo[] }>('system.connectors', {
              doc,
              board: b.id,
            });
            return [b.id, reply.connectors] as const;
          } catch {
            return [b.id, []] as const;
          }
        }),
    ).then((entries) => {
      if (!cancelled) {
        const lists: Record<string, readonly ConnectorInfo[]> = {};
        for (const [id, list] of entries) {
          lists[id] = list;
        }
        setConnectorLists(lists);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [engine, doc, info]);

  const connectors = useCallback(
    (board: string): readonly ConnectorInfo[] => connectorLists[board] ?? [],
    [connectorLists],
  );

  const localBoards = useMemo(
    () => (project?.documents ?? []).filter((d) => d.kind === 'pcb'),
    [project],
  );

  if (info === null) {
    return <div className="system" data-testid="system-editor" />;
  }

  const findings = check?.findings.filter((f) => showAccepted || !f.excluded) ?? [];
  const errors = check?.errors ?? 0;
  const warnings = check?.warnings ?? 0;
  const link = info.links.find((l) => l.id === selectedLink) ?? info.links[0] ?? null;
  const cable = info.cables.find((c) => c.id === selectedCable) ?? info.cables[0] ?? null;
  const report = (id: string): SystemCheck['connections'][number] | undefined =>
    check?.connections.find((c) => c.id === id);
  const problems = (id: string): number =>
    check?.findings.filter((f) => f.connection === id && !f.excluded).length ?? 0;

  const addLocal = async (boardDoc: string): Promise<void> => {
    setMenu(false);
    await call('system.addBoard', { source: { kind: 'local', doc: boardDoc } });
  };
  const addOther = async (): Promise<void> => {
    setMenu(false);
    const path = await window.ew.dialogs.openProject();
    if (path !== null) {
      await call('system.addBoard', { source: { kind: 'project', path } });
    }
  };
  const addReference = async (): Promise<void> => {
    setMenu(false);
    const path = await window.ew.dialogs.chooseImport('kicadProject');
    if (path !== null) {
      await call('system.importReference', { source: path });
    }
  };
  const writeOutputs = async (): Promise<void> => {
    const reply = await call<{ files: string[] }>('system.outputs', {});
    if (reply !== null && reply.files.length > 0) {
      const first = reply.files[0] ?? '';
      const folder = first.slice(0, Math.max(first.lastIndexOf('/'), first.lastIndexOf('\\')));
      flash(t('system.outputsDone', { count: reply.files.length, path: folder }));
    }
  };
  const updateBoard = (board: SystemBoardInfo, fields: Record<string, unknown>): void => {
    void call('system.updateBoard', { id: board.id, ...fields });
  };
  const updateCable = (target: Cable, fields: Record<string, unknown>): void => {
    void call('system.updateCable', { id: target.id, ...fields });
  };
  const setWires = (target: Cable, wires: readonly Wire[]): void => {
    updateCable(target, { wires });
  };
  const rename = async (finding: SystemFinding, side: 0 | 1): Promise<void> => {
    const pin = finding.pins[side];
    if (pin === undefined) {
      return;
    }
    const from = side === 0 ? finding.args['signalA'] : finding.args['signalB'];
    const to = side === 0 ? finding.args['signalB'] : finding.args['signalA'];
    const reply = await call<{ renamed: number; others: { board: string }[] }>(
      'system.renameSignal',
      { board: pin.board, net: from, to },
    );
    if (reply !== null) {
      const others =
        reply.others.length > 0
          ? ` · ${t('system.findings.renameOthers', { boards: reply.others.map((o) => o.board).join(', ') })}`
          : '';
      flash(`${t('system.findings.renamed', { count: reply.renamed })}${others}`);
    }
  };

  const tabLabel = (key: Tab): string => t(`system.tabs.${key}`);

  return (
    <div className="system" data-testid="system-editor">
      <div className="system__toolbar" role="toolbar" aria-label={t('system.toolbar')}>
        <h2 className="system__title">{info.name}</h2>
        <button
          type="button"
          className={`system__summary system__summary--${errors > 0 ? 'error' : warnings > 0 ? 'warning' : 'ok'}`}
          data-testid="system-summary"
          onClick={() => {
            setTab('check');
          }}
        >
          <Icon name={errors > 0 ? 'error' : warnings > 0 ? 'warning' : 'check'} size={14} />
          {checking
            ? t('system.checking')
            : errors + warnings === 0
              ? t('system.summaryClean')
              : t('system.summary', { errors, warnings })}
        </button>
        <span className="system__spacer" />
        <span className="system__menu">
          <button
            type="button"
            className="button"
            title={t('system.addBoardTip')}
            aria-expanded={menu}
            data-testid="system-add-board"
            onClick={() => {
              setMenu(!menu);
            }}
          >
            <Icon name="plus" size={16} />
            {t('system.addBoard')}
          </button>
          {menu && (
            <div className="popover system__popover" role="menu" data-testid="system-add-menu">
              <div className="popover__label">{t('system.thisProject')}</div>
              {localBoards.map((b) => (
                <button
                  key={b.id}
                  type="button"
                  role="menuitem"
                  className="popover__item"
                  data-testid="system-add-local"
                  onClick={() => {
                    void addLocal(b.id);
                  }}
                >
                  <Icon name="board" size={14} />
                  {b.name}
                </button>
              ))}
              <button
                type="button"
                role="menuitem"
                className="popover__item"
                data-testid="system-add-other"
                onClick={() => {
                  void addOther();
                }}
              >
                <Icon name="folder" size={14} />
                {t('system.otherProject')}
              </button>
              <button
                type="button"
                role="menuitem"
                className="popover__item"
                data-testid="system-add-reference"
                onClick={() => {
                  void addReference();
                }}
              >
                <Icon name="download" size={14} />
                {t('system.reference')}
              </button>
            </div>
          )}
        </span>
        <button
          type="button"
          className="button"
          data-testid="system-check"
          onClick={() => {
            runRef.current();
          }}
        >
          <Icon name="erc" size={16} />
          {t('system.check')}
        </button>
        <button
          type="button"
          className="button"
          title={t('system.outputsTip', {
            folder: info.name.toLowerCase().replace(/[^a-z0-9]+/gu, '-'),
          })}
          data-testid="system-outputs"
          onClick={() => {
            void writeOutputs();
          }}
        >
          <Icon name="output" size={16} />
          {t('system.outputs')}
        </button>
      </div>
      <div className="system__tabs" role="tablist">
        {TABS.map((key) => (
          <button
            key={key}
            type="button"
            role="tab"
            aria-selected={tab === key}
            className="system__tab"
            data-testid={`system-tab-${key}`}
            onClick={() => {
              setTab(key);
            }}
          >
            {tabLabel(key)}
            {key === 'check' && errors + warnings > 0 && (
              <span className="system__count">{errors + warnings}</span>
            )}
          </button>
        ))}
      </div>
      {message !== null && (
        <p className="system__message" role="status" data-testid="system-message">
          {message}
        </p>
      )}
      <div className="system__body">
        {tab === 'boards' &&
          (info.boards.length === 0 ? (
            <p className="field__hint">{t('system.noBoards')}</p>
          ) : (
            <table className="system__table" data-testid="system-boards">
              <thead>
                <tr>
                  <th>{t('system.board.name')}</th>
                  <th>{t('system.board.source')}</th>
                  <th>{t('system.board.quantity')}</th>
                  <th>{t('system.board.position')}</th>
                  <th>{t('system.board.rotation')}</th>
                  <th>{t('system.board.flipped')}</th>
                  <th>{t('system.board.shown')}</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {info.boards.map((b) => (
                  <tr key={b.id} data-testid="system-board" data-name={b.name}>
                    <td>
                      <input
                        key={`${b.id}:${b.name}`}
                        className="input"
                        defaultValue={b.name}
                        aria-label={t('system.board.name')}
                        onBlur={(event) => {
                          const name = event.target.value.trim();
                          if (name !== '' && name !== b.name) {
                            updateBoard(b, { name });
                          }
                        }}
                      />
                    </td>
                    <td>
                      {b.error !== undefined ? (
                        <span className="system__error" title={b.error}>
                          <Icon name="warning" size={14} />
                          {t('system.board.error', { error: b.error })}
                        </span>
                      ) : b.source.kind === 'local' ? (
                        t('system.board.local', { board: b.board ?? '' })
                      ) : (
                        t('system.board.other', { path: b.source.path, board: b.board ?? '' })
                      )}
                    </td>
                    <td>
                      <NumberField
                        key={`${b.id}:q${String(b.quantity)}`}
                        value={b.quantity}
                        label={t('system.board.quantity')}
                        step={1}
                        testId="system-board-quantity"
                        onCommit={(quantity) => {
                          updateBoard(b, { quantity: Math.max(1, Math.round(quantity)) });
                        }}
                      />
                    </td>
                    <td className="system__triple">
                      {([0, 1, 2] as const).map((axis) => (
                        <NumberField
                          key={`${b.id}:${String(axis)}:${String(b.position[axis])}`}
                          value={fromNm(b.position[axis])}
                          label={`${t('system.board.position')} ${'xyz'[axis] ?? ''}`}
                          step={0.5}
                          testId={`system-board-${'xyz'[axis] ?? ''}`}
                          onCommit={(mm) => {
                            const position = [...b.position];
                            position[axis] = Math.round(mm * 1e6);
                            updateBoard(b, { position });
                          }}
                        />
                      ))}
                    </td>
                    <td>
                      <NumberField
                        key={`${b.id}:rz${String(b.rotation[2])}`}
                        value={b.rotation[2]}
                        label={t('system.board.rotation')}
                        step={90}
                        testId="system-board-rz"
                        onCommit={(degrees) => {
                          updateBoard(b, { rotation: [b.rotation[0], b.rotation[1], degrees] });
                        }}
                      />
                    </td>
                    <td>
                      <input
                        type="checkbox"
                        checked={Math.round(b.rotation[0]) === 180}
                        aria-label={t('system.board.flipped')}
                        onChange={(event) => {
                          updateBoard(b, {
                            rotation: [
                              event.target.checked ? 180 : 0,
                              b.rotation[1],
                              b.rotation[2],
                            ],
                          });
                        }}
                      />
                    </td>
                    <td>
                      <input
                        type="checkbox"
                        checked={!b.hidden}
                        aria-label={t('system.board.shown')}
                        onChange={(event) => {
                          updateBoard(b, { hidden: !event.target.checked });
                        }}
                      />
                    </td>
                    <td>
                      <button
                        type="button"
                        className="icon-button"
                        title={t('system.board.remove', { name: b.name })}
                        aria-label={t('system.board.remove', { name: b.name })}
                        data-testid="system-board-remove"
                        onClick={() => {
                          void call('system.removeBoard', { id: b.id });
                        }}
                      >
                        <Icon name="trash" size={16} />
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ))}
        {tab === 'connections' && (
          <div className="system__split">
            <div className="system__list">
              <button
                type="button"
                className="button"
                title={t('system.link.addTip')}
                disabled={info.boards.length < 2}
                data-testid="system-add-link"
                onClick={() => {
                  const [first, second] = info.boards;
                  if (first === undefined || second === undefined) {
                    return;
                  }
                  void call<{ id: string }>('system.addLink', {
                    a: { board: first.id, designator: connectors(first.id)[0]?.designator ?? '' },
                    b: { board: second.id, designator: connectors(second.id)[0]?.designator ?? '' },
                  }).then((reply) => {
                    if (reply !== null) {
                      setSelectedLink(reply.id);
                    }
                  });
                }}
              >
                <Icon name="plus" size={16} />
                {t('system.link.add')}
              </button>
              {info.links.length === 0 && <p className="field__hint">{t('system.link.none')}</p>}
              <ul className="system__items" role="listbox">
                {info.links.map((l) => (
                  <li
                    key={l.id}
                    role="option"
                    aria-selected={link?.id === l.id}
                    className="system__item"
                    data-testid="system-link"
                    onClick={() => {
                      setSelectedLink(l.id);
                    }}
                  >
                    <Icon name={problems(l.id) > 0 ? 'warning' : 'check'} size={14} />
                    {boardName(info, l.a.board)} {l.a.designator} ↔ {boardName(info, l.b.board)}{' '}
                    {l.b.designator}
                  </li>
                ))}
              </ul>
            </div>
            {link !== null && (
              <LinkEditor
                key={link.id}
                info={info}
                link={link}
                connectors={connectors}
                pairs={report(link.id)?.pairs ?? []}
                call={call}
                flash={flash}
              />
            )}
          </div>
        )}
        {tab === 'cables' && (
          <div className="system__split">
            <div className="system__list">
              <button
                type="button"
                className="button"
                data-testid="system-add-cable"
                onClick={() => {
                  const [first, second] = info.boards;
                  const end = (b: SystemBoardInfo | undefined): Record<string, unknown> =>
                    b === undefined
                      ? {}
                      : {
                          connector: {
                            board: b.id,
                            designator: connectors(b.id)[0]?.designator ?? '',
                          },
                        };
                  void call<{ id: string }>('system.addCable', {
                    ends: [end(first), end(second)],
                  }).then((reply) => {
                    if (reply !== null) {
                      setSelectedCable(reply.id);
                    }
                  });
                }}
              >
                <Icon name="plus" size={16} />
                {t('system.cable.add')}
              </button>
              {info.cables.length === 0 && <p className="field__hint">{t('system.cable.none')}</p>}
              <ul className="system__items" role="listbox">
                {info.cables.map((c) => (
                  <li
                    key={c.id}
                    role="option"
                    aria-selected={cable?.id === c.id}
                    className="system__item"
                    data-testid="system-cable"
                    onClick={() => {
                      setSelectedCable(c.id);
                    }}
                  >
                    <Icon name={problems(c.id) > 0 ? 'warning' : 'check'} size={14} />
                    {c.name} {c.part !== undefined && c.part !== '' ? `· ${c.part}` : ''}
                  </li>
                ))}
              </ul>
            </div>
            {cable !== null && (
              <CableEditor
                key={cable.id}
                info={info}
                cable={cable}
                connectors={connectors}
                pairs={report(cable.id)?.pairs ?? []}
                onUpdate={(fields) => {
                  updateCable(cable, fields);
                }}
                onWires={(wires) => {
                  setWires(cable, wires);
                }}
                call={call}
              />
            )}
          </div>
        )}
        {tab === 'check' && (
          <div className="system__check">
            <label className="checkbox">
              <input
                type="checkbox"
                checked={showAccepted}
                onChange={(event) => {
                  setShowAccepted(event.target.checked);
                }}
              />
              {t('system.findings.showAccepted')}
            </label>
            {findings.length === 0 ? (
              <p className="field__hint" data-testid="system-findings-none">
                {t('system.findings.none')}
              </p>
            ) : (
              <ul className="system__findings" data-testid="system-findings">
                {findings.map((f) => {
                  const boardA = f.pins[0] === undefined ? '' : boardName(info, f.pins[0].board);
                  const boardB = f.pins[1] === undefined ? '' : boardName(info, f.pins[1].board);
                  const renamable =
                    f.code === 'system-mismatch' &&
                    f.args['variant'] === 'pin' &&
                    f.pins.length === 2;
                  return (
                    <li
                      key={f.key}
                      className={`system__finding system__finding--${f.severity}`}
                      data-testid="system-finding"
                      data-code={f.code}
                    >
                      <Icon name={f.severity === 'error' ? 'error' : 'warning'} size={14} />
                      <span className="system__finding-text">{systemFindingMessage(t, f)}</span>
                      {renamable && (
                        <>
                          <button
                            type="button"
                            className="button button--small"
                            title={t('system.findings.renameTip', { board: boardB })}
                            data-testid="system-rename"
                            onClick={() => {
                              void rename(f, 1);
                            }}
                          >
                            {t('system.findings.rename', {
                              signal: f.args['signalA'] ?? '',
                              board: boardB,
                            })}
                          </button>
                          <button
                            type="button"
                            className="button button--small"
                            title={t('system.findings.renameTip', { board: boardA })}
                            onClick={() => {
                              void rename(f, 0);
                            }}
                          >
                            {t('system.findings.rename', {
                              signal: f.args['signalB'] ?? '',
                              board: boardA,
                            })}
                          </button>
                        </>
                      )}
                      <button
                        type="button"
                        className="button button--small"
                        data-testid="system-accept"
                        onClick={() => {
                          void call('system.exclude', { keys: [f.key], excluded: !f.excluded });
                        }}
                      >
                        {f.excluded ? t('system.findings.unaccept') : t('system.findings.accept')}
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}
            <Aliases info={info} call={call} />
          </div>
        )}
        {tab === 'view3d' && (
          <div className="system__view3d">
            <Board3dView doc={doc} name={info.name} system />
          </div>
        )}
      </div>
    </div>
  );
}

type Call = <T>(method: string, params: Record<string, unknown>) => Promise<T | null>;

function LinkEditor({
  info,
  link,
  connectors,
  pairs,
  call,
  flash,
}: {
  readonly info: SystemInfo;
  readonly link: Link;
  readonly connectors: (board: string) => readonly ConnectorInfo[];
  readonly pairs: SystemCheck['connections'][number]['pairs'];
  readonly call: Call;
  readonly flash: (text: string) => void;
}): ReactNode {
  const { t } = useAppState();
  const update = (fields: Record<string, unknown>): void => {
    void call('system.updateLink', { id: link.id, ...fields });
  };
  return (
    <div className="system__editor" data-testid="system-link-editor">
      <div className="system__row">
        <span className="system__label">{t('system.link.boardA')}</span>
        <ConnectorPicker
          info={info}
          value={link.a}
          connectors={connectors}
          testId="system-link-a"
          onChange={(a) => {
            if (a !== undefined) {
              update({ a });
            }
          }}
        />
      </div>
      <div className="system__row">
        <span className="system__label">{t('system.link.boardB')}</span>
        <ConnectorPicker
          info={info}
          value={link.b}
          connectors={connectors}
          testId="system-link-b"
          onChange={(b) => {
            if (b !== undefined) {
              update({ b });
            }
          }}
        />
      </div>
      <div className="system__row">
        <span className="system__label">{t('system.link.pairing')}</span>
        <select
          className="input"
          value={link.pairing ?? 'number'}
          data-testid="system-link-pairing"
          onChange={(event) => {
            update({ pairing: event.target.value });
          }}
        >
          {PAIRINGS.map((p) => (
            <option key={p} value={p}>
              {t(`system.link.pairings.${p}`)}
            </option>
          ))}
        </select>
      </div>
      <div className="system__actions">
        <button
          type="button"
          className="button"
          title={t('system.link.stackTip')}
          data-testid="system-link-stack"
          onClick={() => {
            void call<{ gap: number }>('system.stack', { link: link.id }).then((reply) => {
              if (reply !== null) {
                flash(
                  t('system.link.stacked', {
                    board: boardName(info, link.b.board),
                    gap: Math.round(reply.gap * 100) / 100,
                  }),
                );
              }
            });
          }}
        >
          <Icon name="layers" size={16} />
          {t('system.link.stack')}
        </button>
        <button
          type="button"
          className="button button--danger"
          data-testid="system-link-remove"
          onClick={() => {
            void call('system.removeLink', { id: link.id });
          }}
        >
          <Icon name="trash" size={16} />
          {t('system.link.remove')}
        </button>
      </div>
      <PinTable pairs={pairs} testId="system-link-pins" />
    </div>
  );
}

function CableEditor({
  info,
  cable,
  connectors,
  pairs,
  onUpdate,
  onWires,
  call,
}: {
  readonly info: SystemInfo;
  readonly cable: Cable;
  readonly connectors: (board: string) => readonly ConnectorInfo[];
  readonly pairs: SystemCheck['connections'][number]['pairs'];
  readonly onUpdate: (fields: Record<string, unknown>) => void;
  readonly onWires: (wires: readonly Wire[]) => void;
  readonly call: Call;
}): ReactNode {
  const { t } = useAppState();
  const setWire = (index: number, fields: Partial<Wire>): void => {
    onWires(cable.wires.map((w, i) => (i === index ? { ...w, ...fields } : w)));
  };
  const text = (
    value: string | undefined,
    label: MessageKey,
    commit: (value: string) => void,
    testId?: string,
  ): ReactNode => (
    <input
      key={`${label}:${value ?? ''}`}
      className="input"
      defaultValue={value ?? ''}
      aria-label={t(label)}
      data-testid={testId}
      onBlur={(event) => {
        if (event.target.value !== (value ?? '')) {
          commit(event.target.value);
        }
      }}
    />
  );
  const [first, second] = cable.ends;
  return (
    <div className="system__editor" data-testid="system-cable-editor">
      <div className="system__row">
        <label className="system__label">{t('system.cable.name')}</label>
        {text(cable.name, 'system.cable.name', (name) => {
          onUpdate({ name });
        })}
        <label className="system__label">{t('system.cable.part')}</label>
        {text(cable.part, 'system.cable.part', (part) => {
          onUpdate({ part });
        })}
        <label className="system__label">{t('system.cable.length')}</label>
        <NumberField
          key={`len:${String(cable.length ?? 0)}`}
          value={fromNm(cable.length)}
          label={t('system.cable.length')}
          step={10}
          testId="system-cable-length"
          onCommit={(mm) => {
            onUpdate({ length: Math.max(0, Math.round(mm * 1e6)) });
          }}
        />
      </div>
      <h3 className="system__subtitle">{t('system.cable.ends')}</h3>
      <table className="system__table">
        <thead>
          <tr>
            <th>{t('system.cable.endLabel')}</th>
            <th>{t('system.cable.endConnector')}</th>
            <th>{t('system.cable.housing')}</th>
          </tr>
        </thead>
        <tbody>
          {cable.ends.map((end, i) => (
            <tr key={end.id} data-testid="system-cable-end">
              <td>
                {text(end.label, 'system.cable.endLabel', (label) => {
                  onUpdate({ ends: cable.ends.map((e, j) => (j === i ? { ...e, label } : e)) });
                })}
              </td>
              <td>
                <ConnectorPicker
                  info={info}
                  value={end.connector}
                  connectors={connectors}
                  allowFree
                  testId={`system-cable-end-${String(i)}`}
                  onChange={(connector) => {
                    onUpdate({
                      ends: cable.ends.map((e, j) =>
                        j === i ? { ...e, connector: connector ?? null } : e,
                      ),
                    });
                  }}
                />
              </td>
              <td>
                {text(end.housing, 'system.cable.housing', (housing) => {
                  onUpdate({ ends: cable.ends.map((e, j) => (j === i ? { ...e, housing } : e)) });
                })}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <div className="system__actions">
        <button
          type="button"
          className="button"
          onClick={() => {
            onUpdate({ ends: [...cable.ends, { label: `X${String(cable.ends.length + 1)}` }] });
          }}
        >
          <Icon name="plus" size={16} />
          {t('system.cable.addEnd')}
        </button>
      </div>
      <h3 className="system__subtitle">{t('system.cable.wires')}</h3>
      <div className="system__actions">
        <button
          type="button"
          className="button"
          title={t('system.cable.wireAllTip')}
          disabled={first?.connector === undefined || second === undefined}
          data-testid="system-cable-wire-all"
          onClick={() => {
            if (first !== undefined && second !== undefined) {
              void call('system.wireCable', { cable: cable.id, from: first.id, to: second.id });
            }
          }}
        >
          <Icon name="wire" size={16} />
          {t('system.cable.wireAll')}
        </button>
        <button
          type="button"
          className="button"
          disabled={first === undefined || second === undefined}
          onClick={() => {
            if (first !== undefined && second !== undefined) {
              const n = String(cable.wires.length + 1);
              onWires([
                ...cable.wires,
                {
                  id: crypto.randomUUID(),
                  from: { end: first.id, pin: n },
                  to: { end: second.id, pin: n },
                },
              ]);
            }
          }}
        >
          <Icon name="plus" size={16} />
          {t('system.cable.addWire')}
        </button>
        <button
          type="button"
          className="button button--danger"
          data-testid="system-cable-remove"
          onClick={() => {
            void call('system.removeCable', { id: cable.id });
          }}
        >
          <Icon name="trash" size={16} />
          {t('system.cable.remove')}
        </button>
      </div>
      <table className="system__table" data-testid="system-wires">
        <thead>
          <tr>
            <th>#</th>
            <th>{t('system.cable.from')}</th>
            <th>{t('system.cable.pin')}</th>
            <th>{t('system.cable.to')}</th>
            <th>{t('system.cable.pin')}</th>
            <th>{t('system.cable.signal')}</th>
            <th>{t('system.cable.color')}</th>
            <th>{t('system.cable.gauge')}</th>
            <th>{t('system.pins.status')}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {cable.wires.map((w, i) => {
            const pair = pairs.find((p) => p.wire === w.id);
            const endSelect = (value: string, onChange: (end: string) => void): ReactNode => (
              <select
                className="input"
                value={value}
                onChange={(event) => {
                  onChange(event.target.value);
                }}
              >
                {cable.ends.map((e) => (
                  <option key={e.id} value={e.id}>
                    {e.label}
                  </option>
                ))}
              </select>
            );
            return (
              <tr key={w.id} data-testid="system-wire">
                <td>{i + 1}</td>
                <td>
                  {endSelect(w.from.end, (end) => {
                    setWire(i, { from: { ...w.from, end } });
                  })}
                </td>
                <td>
                  {text(w.from.pin, 'system.cable.pin', (pin) => {
                    setWire(i, { from: { ...w.from, pin } });
                  })}
                </td>
                <td>
                  {endSelect(w.to.end, (end) => {
                    setWire(i, { to: { ...w.to, end } });
                  })}
                </td>
                <td>
                  {text(w.to.pin, 'system.cable.pin', (pin) => {
                    setWire(i, { to: { ...w.to, pin } });
                  })}
                </td>
                <td>
                  {w.signal !== undefined && w.signal !== ''
                    ? w.signal
                    : shortName(pair?.aNet ?? pair?.bNet)}
                </td>
                <td>
                  {text(w.color, 'system.cable.color', (color) => {
                    setWire(i, { color });
                  })}
                </td>
                <td>
                  {text(w.gauge, 'system.cable.gauge', (gauge) => {
                    setWire(i, { gauge });
                  })}
                </td>
                <td>{pair !== undefined && <Status status={pair.status} />}</td>
                <td>
                  <button
                    type="button"
                    className="icon-button"
                    title={t('system.cable.removeWire')}
                    aria-label={t('system.cable.removeWire')}
                    onClick={() => {
                      onWires(cable.wires.filter((_, j) => j !== i));
                    }}
                  >
                    <Icon name="trash" size={14} />
                  </button>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function Aliases({ info, call }: { readonly info: SystemInfo; readonly call: Call }): ReactNode {
  const { t } = useAppState();
  const current = info.aliases.map((group) => group.join(' = ')).join('\n');
  const [draft, setDraft] = useState<string | null>(null);
  return (
    <section className="system__aliases">
      <h3 className="system__subtitle">{t('system.findings.aliases')}</h3>
      <p className="field__hint">{t('system.findings.aliasesHint')}</p>
      <textarea
        className="input"
        rows={3}
        value={draft ?? current}
        data-testid="system-aliases"
        onChange={(event) => {
          setDraft(event.target.value);
        }}
      />
      <button
        type="button"
        className="button"
        disabled={draft === null || draft === current}
        data-testid="system-aliases-save"
        onClick={() => {
          const aliases = (draft ?? '')
            .split('\n')
            .map((line) =>
              line
                .split('=')
                .map((s) => s.trim())
                .filter((s) => s !== ''),
            )
            .filter((group) => group.length >= 2);
          void call('system.setAliases', { aliases }).then(() => {
            setDraft(null);
          });
        }}
      >
        {t('system.findings.saveAliases')}
      </button>
    </section>
  );
}
