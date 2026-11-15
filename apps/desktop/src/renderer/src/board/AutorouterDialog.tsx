/**
 * Routing with an external autorouter (Freerouting and others): the board
 * goes out as a Specctra DSN file, the router's session file (SES) comes
 * back as tracks and vias.
 */
import { useState, type ReactNode } from 'react';

import { joinPath } from '@shared/project';

import { Icon } from '../components/Icon';
import { Modal } from '../components/Modal';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';

interface SessionReply {
  readonly tracks: number;
  readonly vias: number;
  readonly removedTracks: number;
  readonly removedVias: number;
  readonly unknownNets: readonly string[];
  readonly unknownLayers: readonly string[];
}

export function AutorouterDialog({
  doc,
  onClose,
}: {
  readonly doc: string;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine, appInfo } = useAppState();
  const { project, notify } = useProject();
  const [busy, setBusy] = useState(false);
  const [saved, setSaved] = useState<string | null>(null);
  const [result, setResult] = useState<SessionReply | null>(null);

  const fail = (error: unknown): void => {
    notify(errorMessage(t, error), 'error');
  };

  const exportDsn = async (): Promise<void> => {
    if (project === null) {
      return;
    }
    setBusy(true);
    try {
      const reply = await engine.call<{ file: string; text: string }>('pcb.exportSpecctra', {
        doc,
      });
      const path = await window.ew.dialogs.saveExport(
        'dsn',
        joinPath(project.path, reply.file, appInfo.platform),
        reply.text,
      );
      if (path !== null) {
        setSaved(path);
      }
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  const importSession = async (): Promise<void> => {
    setBusy(true);
    try {
      const path = await window.ew.dialogs.chooseImport('ses');
      if (path !== null) {
        const reply = await engine.call<SessionReply>('pcb.importSession', { doc, path });
        setResult(reply);
        notify(t('board.autorouter.imported', { tracks: reply.tracks, vias: reply.vias }));
      }
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title={t('board.autorouter.title')}
      onClose={onClose}
      testId="autorouter-dialog"
      footer={
        <button type="button" className="button" data-testid="autorouter-close" onClick={onClose}>
          {t('board.autorouter.close')}
        </button>
      }
    >
      <p className="dialog__text">{t('board.autorouter.intro')}</p>
      <ol className="autorouter__steps">
        <li>
          <div className="autorouter__step">
            <span>{t('board.autorouter.step1')}</span>
            <button
              type="button"
              className="button"
              disabled={busy}
              data-testid="autorouter-export"
              onClick={() => {
                void exportDsn();
              }}
            >
              <Icon name="download" size={16} />
              {t('board.autorouter.export')}
            </button>
          </div>
          {saved !== null && (
            <span className="field__hint" data-testid="autorouter-saved">
              {t('board.autorouter.saved', { path: saved })}
            </span>
          )}
        </li>
        <li>
          <div className="autorouter__step">
            <span>{t('board.autorouter.step2')}</span>
          </div>
        </li>
        <li>
          <div className="autorouter__step">
            <span>{t('board.autorouter.step3')}</span>
            <button
              type="button"
              className="button button--primary"
              disabled={busy}
              data-testid="autorouter-import"
              onClick={() => {
                void importSession();
              }}
            >
              <Icon name="importFile" size={16} />
              {t('board.autorouter.import')}
            </button>
          </div>
          {result !== null && (
            <span className="field__hint" data-testid="autorouter-result">
              {t('board.autorouter.result', {
                tracks: result.tracks,
                vias: result.vias,
                removedTracks: result.removedTracks,
                removedVias: result.removedVias,
              })}
            </span>
          )}
          {result !== null && result.unknownNets.length > 0 && (
            <span className="field__error" data-testid="autorouter-unknown-nets">
              {t('board.autorouter.unknownNets', { list: result.unknownNets.join(', ') })}
            </span>
          )}
          {result !== null && result.unknownLayers.length > 0 && (
            <span className="field__error">
              {t('board.autorouter.unknownLayers', { list: result.unknownLayers.join(', ') })}
            </span>
          )}
        </li>
      </ol>
      <p className="callout callout--info">
        <Icon name="info" />
        <span>{t('board.autorouter.hint')}</span>
      </p>
    </Modal>
  );
}
