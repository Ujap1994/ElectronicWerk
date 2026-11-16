/**
 * Channels: schematic sheets used more than once. The layout of one use
 * (placement, and routing between its parts) is copied to the others.
 */
import { useEffect, useState, type ReactNode } from 'react';

import { Modal } from '../components/Modal';
import { CheckInput, SelectInput } from '../libedit/fields';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';

interface Channel {
  readonly sheet: string;
  readonly name: string;
  readonly instances: readonly {
    readonly key: string;
    readonly name: string;
    readonly designators: readonly string[];
  }[];
}

interface CopyReply {
  readonly moved: number;
  readonly tracks: number;
  readonly vias: number;
  readonly skipped: readonly string[];
}

export function ChannelsDialog({
  doc,
  onClose,
}: {
  readonly doc: string;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  const [channels, setChannels] = useState<readonly Channel[] | null>(null);
  const [sheet, setSheet] = useState('');
  const [from, setFrom] = useState('');
  const [targets, setTargets] = useState<readonly string[]>([]);
  const [routing, setRouting] = useState(true);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    engine
      .call<Channel[]>('pcb.channels', { doc })
      .then((list) => {
        if (cancelled) {
          return;
        }
        setChannels(list);
        const first = list[0];
        if (first !== undefined) {
          setSheet(first.sheet);
          setFrom(first.instances[0]?.key ?? '');
          setTargets(first.instances.slice(1).map((i) => i.key));
        }
      })
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      });
    return () => {
      cancelled = true;
    };
  }, [engine, doc, notify, t]);

  const channel = channels?.find((c) => c.sheet === sheet);
  const choose = (next: string): void => {
    const found = channels?.find((c) => c.sheet === next);
    setSheet(next);
    setFrom(found?.instances[0]?.key ?? '');
    setTargets(found?.instances.slice(1).map((i) => i.key) ?? []);
  };

  const copy = (): void => {
    setBusy(true);
    engine
      .call<CopyReply>('pcb.copyChannel', {
        doc,
        sheet,
        from,
        to: targets,
        routing,
        apply: true,
      })
      .then((reply) => {
        notify(
          reply.skipped.length > 0
            ? t('board.channels.copiedSkipped', {
                moved: reply.moved,
                tracks: reply.tracks,
                skipped: reply.skipped.join(', '),
              })
            : t('board.channels.copied', { moved: reply.moved, tracks: reply.tracks }),
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

  return (
    <Modal
      title={t('board.channels.title')}
      onClose={onClose}
      testId="channels-dialog"
      className="dialog--wide"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('board.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={busy || channel === undefined || targets.length === 0}
            data-testid="channels-copy"
            onClick={copy}
          >
            {t('board.channels.copy')}
          </button>
        </>
      }
    >
      <p className="field__hint">{t('board.channels.hint')}</p>
      {channels === null ? (
        <p className="dialog__text dialog__text--muted">{t('board.rules.loading')}</p>
      ) : channels.length === 0 ? (
        <p className="field__hint" data-testid="channels-none">
          {t('board.channels.none')}
        </p>
      ) : (
        <div className="board-form">
          <SelectInput
            label={t('board.channels.sheet')}
            value={sheet}
            testId="channels-sheet"
            options={channels.map(
              (c) =>
                [
                  c.sheet,
                  t('board.channels.sheetOption', { name: c.name, count: c.instances.length }),
                ] as const,
            )}
            onSave={choose}
          />
          {channel !== undefined && (
            <table className="board-table" data-testid="channels-instances">
              <thead>
                <tr>
                  <th>{t('board.channels.instance')}</th>
                  <th>{t('board.channels.parts')}</th>
                  <th>{t('board.channels.source')}</th>
                  <th>{t('board.channels.target')}</th>
                </tr>
              </thead>
              <tbody>
                {channel.instances.map((instance, index) => (
                  <tr key={instance.key}>
                    <td>{instance.name}</td>
                    <td className="board-table__nets" title={instance.designators.join(', ')}>
                      {instance.designators.join(', ')}
                    </td>
                    <td>
                      <input
                        type="radio"
                        name="channel-source"
                        aria-label={t('board.channels.source')}
                        checked={from === instance.key}
                        data-testid={`channels-from-${index.toString()}`}
                        onChange={() => {
                          setFrom(instance.key);
                          setTargets(targets.filter((k) => k !== instance.key));
                        }}
                      />
                    </td>
                    <td>
                      <input
                        type="checkbox"
                        aria-label={t('board.channels.target')}
                        disabled={from === instance.key}
                        checked={targets.includes(instance.key)}
                        data-testid={`channels-to-${index.toString()}`}
                        onChange={(event) => {
                          setTargets(
                            event.target.checked
                              ? [...targets, instance.key]
                              : targets.filter((k) => k !== instance.key),
                          );
                        }}
                      />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <CheckInput
            label={t('board.channels.routing')}
            checked={routing}
            testId="channels-routing"
            onSave={setRouting}
          />
        </div>
      )}
    </Modal>
  );
}
