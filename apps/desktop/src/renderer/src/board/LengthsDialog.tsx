/**
 * Lengths of a board: the skew of every differential pair and the lengths
 * of the length groups, with their rules. **Match lengths** makes every
 * net that breaks a rule long enough with meanders; in the advanced
 * working mode the meander shape can be set and a selected track tuned to
 * a length of its own.
 */
import { useCallback, useEffect, useState, type ReactNode } from 'react';

import { formatLength, type Units } from '@shared/project';

import { Modal } from '../components/Modal';
import { CheckInput, LengthInput } from '../libedit/fields';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';

interface NetLength {
  readonly net: string;
  readonly name: string;
  readonly length: number;
}

interface LengthReport {
  readonly nets: readonly NetLength[];
  readonly pairs: readonly {
    readonly name: string;
    readonly positive: NetLength;
    readonly negative: NetLength;
    readonly skew: number;
    readonly maxSkew: number;
    readonly ok: boolean;
  }[];
  readonly groups: readonly {
    readonly name: string;
    readonly tolerance: number;
    readonly longest: number;
    readonly nets: readonly NetLength[];
    readonly ok: boolean;
  }[];
}

interface MatchReply {
  readonly nets: readonly {
    readonly name: string;
    readonly wanted: number;
    readonly gained: number;
    readonly reached: boolean;
  }[];
}

/** The length a net should get: its partner's, or its group's longest. */
function targetOf(report: LengthReport, net: string): number | null {
  for (const pair of report.pairs) {
    if (pair.positive.net === net) {
      return pair.negative.length;
    }
    if (pair.negative.net === net) {
      return pair.positive.length;
    }
  }
  const group = report.groups.find((g) => g.nets.some((n) => n.net === net));
  return group === undefined ? null : group.longest;
}

export function LengthsDialog({
  doc,
  units,
  trackId,
  onClose,
}: {
  readonly doc: string;
  readonly units: Units;
  /** The selected track, if exactly one is selected. */
  readonly trackId: string | null;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine, snapshot } = useAppState();
  const { notify } = useProject();
  const advanced = snapshot.settings.workMode === 'advanced';
  const language = snapshot.settings.language;
  const length = (nm: number): string => formatLength(nm, units, language);
  const [report, setReport] = useState<LengthReport | null>(null);
  const [own, setOwn] = useState(false);
  const [spacing, setSpacing] = useState(600_000);
  const [amplitude, setAmplitude] = useState(1_800_000);
  const [target, setTarget] = useState<number | null>(null);
  const [busy, setBusy] = useState(false);
  const [track, setTrack] = useState<{ readonly id: string; readonly net: string } | null>(null);

  // The net of the selected track.
  useEffect(() => {
    if (trackId === null) {
      return undefined;
    }
    let cancelled = false;
    engine
      .call<{ net?: string }>('document.object', { doc, id: trackId })
      .then((object) => {
        if (!cancelled && object.net !== undefined) {
          setTrack({ id: trackId, net: object.net });
        }
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [engine, doc, trackId]);

  const load = useCallback(() => {
    engine
      .call<LengthReport>('pcb.lengths', { doc })
      .then(setReport)
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      });
  }, [engine, doc, notify, t]);
  useEffect(load, [load]);

  const shape = own ? { spacing, amplitude } : {};
  const trackNet = track === null ? undefined : report?.nets.find((n) => n.net === track.net);
  const trackTarget =
    target ?? (report === null || track === null ? null : targetOf(report, track.net));

  const match = (): void => {
    setBusy(true);
    engine
      .call<MatchReply>('pcb.matchLengths', { doc, apply: true, ...shape })
      .then((reply) => {
        const missed = reply.nets.filter((n) => !n.reached).map((n) => n.name);
        notify(
          missed.length > 0
            ? t('board.lengths.notReached', { nets: missed.join(', ') })
            : t('board.lengths.matched', { count: reply.nets.length }),
          missed.length > 0 ? 'error' : 'info',
        );
        load();
      })
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      })
      .finally(() => {
        setBusy(false);
      });
  };

  const tune = (): void => {
    if (track === null || trackTarget === null) {
      return;
    }
    setBusy(true);
    engine
      .call<{ reached: boolean; length: number }>('pcb.tuneLength', {
        doc,
        track: track.id,
        length: trackTarget,
        apply: true,
        ...shape,
      })
      .then((reply) => {
        notify(
          reply.reached
            ? t('board.lengths.tuned', { length: length(reply.length) })
            : t('board.lengths.tunedPartly', { length: length(reply.length) }),
          reply.reached ? 'info' : 'error',
        );
        onClose();
      })
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      })
      .finally(() => {
        setBusy(false);
      });
  };

  const broken =
    report === null ? 0 : [...report.pairs, ...report.groups].filter((x) => !x.ok).length;

  return (
    <Modal
      title={t('board.lengths.title')}
      onClose={onClose}
      testId="lengths-dialog"
      className="dialog--wide dialog--rules"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('board.lengths.close')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={busy || broken === 0}
            data-testid="lengths-match"
            onClick={match}
          >
            {t('board.lengths.match')}
          </button>
        </>
      }
    >
      <p className="field__hint">{t('board.lengths.hint')}</p>
      {report === null ? (
        <p className="dialog__text dialog__text--muted">{t('board.rules.loading')}</p>
      ) : (
        <>
          <h3 className="dialog__section">{t('board.lengths.pairs')}</h3>
          {report.pairs.length === 0 ? (
            <p className="field__hint">{t('board.lengths.noPairs')}</p>
          ) : (
            <div className="board-table__scroll">
              <table className="board-table lengths-table" data-testid="lengths-pairs">
                <thead>
                  <tr>
                    <th>{t('board.lengths.pair')}</th>
                    <th>{t('board.lengths.positive')}</th>
                    <th>{t('board.lengths.negative')}</th>
                    <th>{t('board.lengths.skew')}</th>
                    <th>{t('board.lengths.allowed')}</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {report.pairs.map((pair, index) => (
                    <tr key={pair.name} data-testid={`lengths-pair-${index.toString()}`}>
                      <td>{pair.name}</td>
                      <td>
                        {pair.positive.name} · {length(pair.positive.length)}
                      </td>
                      <td>
                        {pair.negative.name} · {length(pair.negative.length)}
                      </td>
                      <td>{length(pair.skew)}</td>
                      <td>
                        {pair.maxSkew > 0 ? length(pair.maxSkew) : t('board.lengths.unchecked')}
                      </td>
                      <td>
                        <span
                          className={
                            pair.ok ? 'impedance-status' : 'impedance-status impedance-status--bad'
                          }
                          data-testid={`lengths-pair-status-${index.toString()}`}
                        >
                          {pair.ok ? t('board.lengths.ok') : t('board.lengths.tooMuchSkew')}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          <h3 className="dialog__section">{t('board.lengths.groups')}</h3>
          {report.groups.length === 0 ? (
            <p className="field__hint">{t('board.lengths.noGroups')}</p>
          ) : (
            report.groups.map((group, gi) => (
              <div
                key={group.name}
                className="board-table__scroll"
                data-testid={`lengths-group-${gi.toString()}`}
              >
                <table className="board-table lengths-table">
                  <thead>
                    <tr>
                      <th>
                        {t('board.lengths.groupTitle', {
                          name: group.name,
                          tolerance: length(group.tolerance),
                        })}
                      </th>
                      <th>{t('board.lengths.length')}</th>
                      <th>{t('board.lengths.shorter')}</th>
                      <th />
                    </tr>
                  </thead>
                  <tbody>
                    {group.nets.map((net) => {
                      const short = group.longest - net.length;
                      const ok = short <= group.tolerance + 0.5;
                      return (
                        <tr key={net.net}>
                          <td>{net.name}</td>
                          <td>{length(net.length)}</td>
                          <td>{short > 0.5 ? length(short) : '—'}</td>
                          <td>
                            <span
                              className={
                                ok ? 'impedance-status' : 'impedance-status impedance-status--bad'
                              }
                            >
                              {ok ? t('board.lengths.ok') : t('board.lengths.tooShort')}
                            </span>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            ))
          )}

          {advanced && (
            <>
              <h3 className="dialog__section">{t('board.lengths.meanders')}</h3>
              <div className="board-form board-form--grid">
                <CheckInput
                  label={t('board.lengths.ownShape')}
                  checked={own}
                  testId="lengths-own-shape"
                  onSave={setOwn}
                />
                {own && (
                  <>
                    <LengthInput
                      label={t('board.lengths.spacing')}
                      value={spacing}
                      units={units}
                      min={1}
                      testId="lengths-spacing"
                      onSave={setSpacing}
                    />
                    <LengthInput
                      label={t('board.lengths.amplitude')}
                      value={amplitude}
                      units={units}
                      min={1}
                      testId="lengths-amplitude"
                      onSave={setAmplitude}
                    />
                  </>
                )}
              </div>
              <h3 className="dialog__section">{t('board.lengths.tuneTitle')}</h3>
              {track === null || trackNet === undefined ? (
                <p className="field__hint">{t('board.lengths.selectTrack')}</p>
              ) : (
                <div className="board-form board-form--grid" data-testid="lengths-tune">
                  <p className="field__hint">
                    {t('board.lengths.selected', {
                      net: trackNet.name,
                      length: length(trackNet.length),
                    })}
                  </p>
                  <LengthInput
                    label={t('board.lengths.target')}
                    value={trackTarget ?? trackNet.length}
                    units={units}
                    min={0}
                    testId="lengths-target"
                    onSave={setTarget}
                  />
                  <button
                    type="button"
                    className="button"
                    disabled={busy || (trackTarget ?? 0) <= trackNet.length}
                    data-testid="lengths-tune-apply"
                    onClick={tune}
                  >
                    {t('board.lengths.tune')}
                  </button>
                </div>
              )}
            </>
          )}
        </>
      )}
    </Modal>
  );
}
