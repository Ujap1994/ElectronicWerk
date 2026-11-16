/**
 * Teardrops: adds (or replaces) the teardrops of the board where tracks end
 * in pads and vias, or removes them. In the advanced working mode their
 * shape can be set; the settings are kept in the design rules.
 */
import { useEffect, useState, type ReactNode } from 'react';

import type { Units } from '@shared/project';

import { Modal } from '../components/Modal';
import { CheckInput, LengthInput, NumberInput } from '../libedit/fields';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { DEFAULT_TEARDROPS, type RulesReply, type TeardropSettings } from './rules-types';

export function TeardropDialog({
  doc,
  units,
  onClose,
}: {
  readonly doc: string;
  readonly units: Units;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine, snapshot } = useAppState();
  const { notify } = useProject();
  const advanced = snapshot.settings.workMode === 'advanced';
  const [settings, setSettings] = useState<TeardropSettings | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    engine
      .call<RulesReply>('pcb.rules', { doc })
      .then((reply) => {
        if (!cancelled) {
          setSettings({ ...DEFAULT_TEARDROPS, ...reply.rules.teardrops });
        }
      })
      .catch(() => {
        if (!cancelled) {
          setSettings(DEFAULT_TEARDROPS);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [engine, doc]);

  const run = (remove: boolean): void => {
    if (settings === null) {
      return;
    }
    setBusy(true);
    engine
      .call<{ added: number; removed: number }>('pcb.teardrops', {
        doc,
        ...(remove ? { remove: true } : advanced ? { settings } : {}),
      })
      .then((reply) => {
        notify(
          remove
            ? t('board.teardrops.removed', { count: reply.removed })
            : t('board.teardrops.added', { count: reply.added }),
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
  const change = (next: Partial<TeardropSettings>): void => {
    if (settings !== null) {
      setSettings({ ...settings, ...next });
    }
  };

  return (
    <Modal
      title={t('board.teardrops.title')}
      onClose={onClose}
      testId="teardrop-dialog"
      className="dialog--wide"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('board.cancel')}
          </button>
          <button
            type="button"
            className="button"
            disabled={busy || settings === null}
            data-testid="teardrops-remove"
            onClick={() => {
              run(true);
            }}
          >
            {t('board.teardrops.remove')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={busy || settings === null}
            data-testid="teardrops-add"
            onClick={() => {
              run(false);
            }}
          >
            {t('board.teardrops.add')}
          </button>
        </>
      }
    >
      <p className="field__hint">{t('board.teardrops.hint')}</p>
      {settings !== null && advanced && (
        <div className="board-form board-form--grid" data-testid="teardrop-settings">
          <CheckInput
            label={t('board.teardrops.pads')}
            checked={settings.pads}
            testId="teardrops-pads"
            onSave={(pads) => {
              change({ pads });
            }}
          />
          <CheckInput
            label={t('board.teardrops.vias')}
            checked={settings.vias}
            testId="teardrops-vias"
            onSave={(vias) => {
              change({ vias });
            }}
          />
          <CheckInput
            label={t('board.teardrops.curved')}
            checked={settings.curved}
            testId="teardrops-curved"
            onSave={(curved) => {
              change({ curved });
            }}
          />
          <NumberInput
            label={t('board.teardrops.lengthRatio')}
            value={settings.lengthRatio}
            integer
            min={10}
            max={200}
            suffix="%"
            testId="teardrops-length-ratio"
            onSave={(lengthRatio) => {
              change({ lengthRatio });
            }}
          />
          <LengthInput
            label={t('board.teardrops.maxLength')}
            value={settings.maxLength}
            units={units}
            min={1}
            testId="teardrops-max-length"
            onSave={(maxLength) => {
              change({ maxLength });
            }}
          />
          <NumberInput
            label={t('board.teardrops.widthRatio')}
            value={settings.widthRatio}
            integer
            min={10}
            max={100}
            suffix="%"
            testId="teardrops-width-ratio"
            onSave={(widthRatio) => {
              change({ widthRatio });
            }}
          />
          <LengthInput
            label={t('board.teardrops.maxWidth')}
            value={settings.maxWidth}
            units={units}
            min={1}
            testId="teardrops-max-width"
            onSave={(maxWidth) => {
              change({ maxWidth });
            }}
          />
        </div>
      )}
      <p className="field__hint">{t('board.teardrops.refill')}</p>
    </Modal>
  );
}
