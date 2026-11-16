/**
 * Stitching vias: a grid of vias in a copper zone, or a via fence along
 * the selected tracks. In the advanced working mode the pitch and the via
 * size can be set.
 */
import { useEffect, useState, type ReactNode } from 'react';

import type { Units } from '@shared/project';

import { Modal } from '../components/Modal';
import { CheckInput, LengthInput, SelectInput } from '../libedit/fields';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { layerName } from './BoardToolbar';

type Mode = 'zone' | 'fence';

interface BoardJson {
  readonly nets: readonly { readonly id: string; readonly name: string }[];
  readonly zones?: readonly {
    readonly id: string;
    readonly name?: string;
    readonly net?: string;
    readonly layers: readonly string[];
    readonly keepout?: unknown;
    readonly teardrop?: boolean;
  }[];
}

export function StitchDialog({
  doc,
  units,
  zone,
  tracks,
  onClose,
}: {
  readonly doc: string;
  readonly units: Units;
  /** The selected zone, if one is selected. */
  readonly zone: string | null;
  /** The selected tracks. */
  readonly tracks: readonly string[];
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine, snapshot } = useAppState();
  const { notify } = useProject();
  const advanced = snapshot.settings.workMode === 'advanced';
  const [board, setBoard] = useState<BoardJson | null>(null);
  const [mode, setMode] = useState<Mode>(tracks.length > 0 && zone === null ? 'fence' : 'zone');
  const [zoneId, setZoneId] = useState(zone ?? '');
  const [net, setNet] = useState('');
  const [zonePitch, setZonePitch] = useState(3_000_000);
  const [fencePitch, setFencePitch] = useState(2_000_000);
  const [staggered, setStaggered] = useState(true);
  const [ownSize, setOwnSize] = useState(false);
  const [diameter, setDiameter] = useState(600_000);
  const [drill, setDrill] = useState(300_000);
  const [ownGap, setOwnGap] = useState(false);
  const [gap, setGap] = useState(300_000);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    engine
      .call<BoardJson>('document.get', { doc })
      .then((loaded) => {
        if (cancelled) {
          return;
        }
        setBoard(loaded);
        const pours = (loaded.zones ?? []).filter(
          (z) => z.net !== undefined && z.keepout === undefined && z.teardrop !== true,
        );
        setZoneId((current) => (current === '' ? (pours[0]?.id ?? '') : current));
        const ground =
          loaded.nets.find((n) => /^(GND|VSS|AGND|DGND)$/iu.test(n.name)) ??
          loaded.nets.find((n) => /GND/iu.test(n.name));
        setNet(ground?.id ?? loaded.nets[0]?.id ?? '');
      })
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      });
    return () => {
      cancelled = true;
    };
  }, [engine, doc, notify, t]);

  const pours = (board?.zones ?? []).filter(
    (z) => z.net !== undefined && z.keepout === undefined && z.teardrop !== true,
  );
  const netName = (id: string | undefined): string =>
    board?.nets.find((n) => n.id === id)?.name ?? '';
  const ready = mode === 'zone' ? zoneId !== '' : tracks.length > 0 && net !== '';

  const run = (remove: boolean): void => {
    setBusy(true);
    const size = ownSize ? { diameter, drill } : {};
    const params =
      mode === 'zone'
        ? { zone: zoneId, pitch: zonePitch, staggered, ...size }
        : { tracks, net, pitch: fencePitch, ...(ownGap ? { gap } : {}), ...size };
    engine
      .call<{ added: number; removed: number }>('pcb.stitch', {
        doc,
        ...params,
        apply: true,
        ...(remove ? { remove: true } : {}),
      })
      .then((reply) => {
        notify(
          remove
            ? t('board.stitch.removed', { count: reply.removed })
            : reply.added === 0
              ? t('board.stitch.none')
              : t('board.stitch.added', { count: reply.added }),
          !remove && reply.added === 0 ? 'error' : 'info',
        );
        if (remove || reply.added > 0) {
          onClose();
        }
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
      title={t('board.stitch.title')}
      onClose={onClose}
      testId="stitch-dialog"
      className="dialog--wide"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('board.cancel')}
          </button>
          <button
            type="button"
            className="button"
            disabled={busy || !ready}
            data-testid="stitch-remove"
            onClick={() => {
              run(true);
            }}
          >
            {t('board.stitch.remove')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={busy || !ready}
            data-testid="stitch-add"
            onClick={() => {
              run(false);
            }}
          >
            {t('board.stitch.add')}
          </button>
        </>
      }
    >
      <div className="segmented" role="tablist">
        {(['zone', 'fence'] as const).map((value) => (
          <button
            key={value}
            type="button"
            role="tab"
            aria-selected={mode === value}
            className={
              mode === value ? 'segmented__item segmented__item--active' : 'segmented__item'
            }
            data-testid={`stitch-mode-${value}`}
            onClick={() => {
              setMode(value);
            }}
          >
            {t(`board.stitch.modes.${value}`)}
          </button>
        ))}
      </div>
      <p className="field__hint">
        {mode === 'zone' ? t('board.stitch.zoneHint') : t('board.stitch.fenceHint')}
      </p>
      {board === null ? (
        <p className="dialog__text dialog__text--muted">{t('board.rules.loading')}</p>
      ) : (
        <div className="board-form board-form--grid">
          {mode === 'zone' ? (
            pours.length === 0 ? (
              <p className="field__hint">{t('board.stitch.noZones')}</p>
            ) : (
              <SelectInput
                label={t('board.stitch.zone')}
                value={zoneId}
                testId="stitch-zone"
                options={pours.map(
                  (z) =>
                    [
                      z.id,
                      t('board.stitch.zoneOption', {
                        name: z.name ?? netName(z.net),
                        net: netName(z.net),
                        layers: z.layers.map((l) => layerName(t, l)).join(', '),
                      }),
                    ] as const,
                )}
                onSave={setZoneId}
              />
            )
          ) : (
            <>
              <p className="field__hint" data-testid="stitch-tracks">
                {tracks.length === 0
                  ? t('board.stitch.selectTracks')
                  : t('board.stitch.tracks', { count: tracks.length })}
              </p>
              <SelectInput
                label={t('board.stitch.net')}
                value={net}
                testId="stitch-net"
                options={board.nets.map((n) => [n.id, n.name] as const)}
                onSave={setNet}
              />
            </>
          )}
          {advanced && (
            <>
              <LengthInput
                label={t('board.stitch.pitch')}
                value={mode === 'zone' ? zonePitch : fencePitch}
                units={units}
                min={100_000}
                testId="stitch-pitch"
                onSave={mode === 'zone' ? setZonePitch : setFencePitch}
              />
              {mode === 'zone' ? (
                <CheckInput
                  label={t('board.stitch.staggered')}
                  checked={staggered}
                  testId="stitch-staggered"
                  onSave={setStaggered}
                />
              ) : (
                <>
                  <CheckInput
                    label={t('board.stitch.ownGap')}
                    checked={ownGap}
                    testId="stitch-own-gap"
                    onSave={setOwnGap}
                  />
                  {ownGap && (
                    <LengthInput
                      label={t('board.stitch.gap')}
                      value={gap}
                      units={units}
                      min={0}
                      testId="stitch-gap"
                      onSave={setGap}
                    />
                  )}
                </>
              )}
              <CheckInput
                label={t('board.stitch.ownSize')}
                checked={ownSize}
                testId="stitch-own-size"
                onSave={setOwnSize}
              />
              {ownSize && (
                <>
                  <LengthInput
                    label={t('board.stitch.diameter')}
                    value={diameter}
                    units={units}
                    min={1}
                    testId="stitch-diameter"
                    onSave={setDiameter}
                  />
                  <LengthInput
                    label={t('board.stitch.drill')}
                    value={drill}
                    units={units}
                    min={1}
                    testId="stitch-drill"
                    onSave={setDrill}
                  />
                </>
              )}
            </>
          )}
        </div>
      )}
    </Modal>
  );
}
