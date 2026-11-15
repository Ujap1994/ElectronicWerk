/**
 * Gerber and Excellon files as a reference drawing: each file becomes
 * filled areas on a user layer of the board, for tracing a board from its
 * manufacturing files or comparing the design with them.
 */
import { useState, type ReactNode } from 'react';

import { Modal } from '../components/Modal';
import { CheckInput, SelectInput } from '../libedit/fields';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { layerLabel } from '../viewer/labels';

interface PreviewFile {
  readonly path: string;
  readonly name: string;
  readonly kind: 'gerber' | 'drill';
  readonly function: string | null;
  readonly objects: number;
  readonly bounds: readonly [number, number, number, number] | null;
  readonly layer: string | null;
}

/** Layers a reference drawing can go to. */
const TARGETS: readonly string[] = [
  ...Array.from({ length: 12 }, (_, i) => `user.${String(i + 1)}`),
  ...Array.from({ length: 4 }, (_, i) => `mech.${String(i + 1)}`),
];

export function GerberReferenceDialog({
  doc,
  onClose,
}: {
  readonly doc: string;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine, snapshot } = useAppState();
  const { notify } = useProject();
  const [files, setFiles] = useState<readonly PreviewFile[]>([]);
  const [layers, setLayers] = useState<Readonly<Record<string, string>>>({});
  const [skipped, setSkipped] = useState<ReadonlySet<string>>(() => new Set());
  const [toOrigin, setToOrigin] = useState(true);
  const [replace, setReplace] = useState(true);
  const [busy, setBusy] = useState(false);

  const fail = (error: unknown): void => {
    notify(errorMessage(t, error), 'error');
  };
  const mm = (nm: number): string =>
    new Intl.NumberFormat(snapshot.language, { maximumFractionDigits: 2 }).format(nm / 1e6);

  const choose = async (): Promise<void> => {
    try {
      const paths = await window.ew.dialogs.chooseGerbers();
      if (paths === null || paths.length === 0) {
        return;
      }
      setBusy(true);
      const reply = await engine.call<{ files: PreviewFile[] }>('pcb.gerberPreview', {
        doc,
        paths,
      });
      setFiles(reply.files);
      setLayers(Object.fromEntries(reply.files.map((file) => [file.path, file.layer ?? 'user.1'])));
      setSkipped(new Set());
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  const chosen = files.filter((file) => !skipped.has(file.path));
  const run = async (): Promise<void> => {
    setBusy(true);
    try {
      const reply = await engine.call<{ shapes: number }>('pcb.importGerbers', {
        doc,
        files: chosen.map((file) => ({ path: file.path, layer: layers[file.path] ?? 'user.1' })),
        origin: toOrigin ? [0, 0] : null,
        replace,
      });
      notify(t('board.gerberRef.done', { shapes: reply.shapes, files: chosen.length }));
      onClose();
    } catch (error) {
      fail(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Modal
      title={t('board.gerberRef.title')}
      onClose={onClose}
      testId="gerber-ref-dialog"
      className="dialog--wide"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('board.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={chosen.length === 0 || busy}
            data-testid="gerber-ref-import"
            onClick={() => {
              void run();
            }}
          >
            {t('board.gerberRef.import')}
          </button>
        </>
      }
    >
      <p className="dialog__text">{t('board.gerberRef.intro')}</p>
      <div className="board-form">
        <div className="board-form__row">
          <button
            type="button"
            className="button"
            disabled={busy}
            data-testid="gerber-ref-choose"
            onClick={() => {
              void choose();
            }}
          >
            {t('board.gerberRef.choose')}
          </button>
          {files.length === 0 && (
            <span className="board-form__path">{t('board.gerberRef.noFiles')}</span>
          )}
        </div>
        {files.length > 0 && (
          <table className="gerber-ref__files" data-testid="gerber-ref-files">
            <thead>
              <tr>
                <th />
                <th>{t('board.gerberRef.file')}</th>
                <th>{t('board.gerberRef.content')}</th>
                <th>{t('board.gerberRef.size')}</th>
                <th>{t('board.gerberRef.layer')}</th>
              </tr>
            </thead>
            <tbody>
              {files.map((file) => (
                <tr key={file.path} data-file={file.name}>
                  <td>
                    <input
                      type="checkbox"
                      aria-label={file.name}
                      checked={!skipped.has(file.path)}
                      onChange={(event) => {
                        const next = new Set(skipped);
                        if (event.target.checked) {
                          next.delete(file.path);
                        } else {
                          next.add(file.path);
                        }
                        setSkipped(next);
                      }}
                    />
                  </td>
                  <td className="gerber-ref__name">{file.name}</td>
                  <td>
                    {file.function ??
                      (file.kind === 'drill'
                        ? t('board.gerberRef.drill')
                        : t('board.gerberRef.unknown'))}
                  </td>
                  <td>
                    {file.bounds === null
                      ? '–'
                      : `${mm(file.bounds[2] - file.bounds[0])} × ${mm(file.bounds[3] - file.bounds[1])} mm`}
                  </td>
                  <td>
                    <SelectInput
                      bare
                      ariaLabel={t('board.gerberRef.layer')}
                      value={layers[file.path] ?? 'user.1'}
                      testId={`gerber-ref-layer-${file.name}`}
                      options={TARGETS.map((layer) => [layer, layerLabel(t, layer)] as const)}
                      onSave={(value) => {
                        setLayers({ ...layers, [file.path]: value });
                      }}
                    />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <CheckInput
          label={t('board.gerberRef.toOrigin')}
          checked={toOrigin}
          testId="gerber-ref-origin"
          onSave={setToOrigin}
        />
        <CheckInput
          label={t('board.gerberRef.replace')}
          checked={replace}
          testId="gerber-ref-replace"
          onSave={setReplace}
        />
        <p className="field__hint">{t('board.gerberRef.hint')}</p>
      </div>
    </Modal>
  );
}
