/**
 * Design blocks: the selected part of the sheet saved with its board
 * layout, and blocks placed again (with their layout as a group on the
 * board).
 */
import { useEffect, useState, type ReactNode } from 'react';

import { Modal } from '../components/Modal';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';

/** A block as `block.list` describes it. */
export interface BlockSummary {
  readonly name: string;
  readonly description: string;
  readonly file: string;
  readonly parts: number;
  readonly designators: readonly string[];
  readonly layout: boolean;
  readonly footprints: number;
  readonly tracks: number;
  readonly vias: number;
}

interface PlaceResult {
  readonly select: readonly string[];
  readonly parts: number;
  readonly library: { readonly added: number; readonly existing: number };
  readonly board: null | {
    readonly footprints: number;
    readonly tracks: number;
    readonly vias: number;
    readonly skipped: number;
  };
}

export function BlocksDialog({
  doc,
  selection,
  onPlaced,
  onClose,
}: {
  readonly doc: string;
  /** The objects selected when the dialog opened. */
  readonly selection: readonly string[];
  readonly onPlaced: (select: readonly string[]) => void;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine } = useAppState();
  const { notify } = useProject();
  const [blocks, setBlocks] = useState<readonly BlockSummary[] | null>(null);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [busy, setBusy] = useState(false);
  const [deleting, setDeleting] = useState<string | null>(null);

  // Counted up to read the block folder again.
  const [version, setVersion] = useState(0);
  useEffect(() => {
    let current = true;
    engine
      .call<BlockSummary[]>('block.list', {})
      .then((list) => {
        if (current) {
          setBlocks(list);
        }
      })
      .catch((error: unknown) => {
        if (current) {
          setBlocks([]);
          notify(errorMessage(t, error), 'error');
        }
      });
    return () => {
      current = false;
    };
  }, [engine, notify, t, version]);
  const load = (): void => {
    setVersion((v) => v + 1);
  };

  const trimmed = name.trim();
  const replaces = blocks?.some((b) => b.name.toLowerCase() === trimmed.toLowerCase()) ?? false;
  const save = (): void => {
    setBusy(true);
    engine
      .call<BlockSummary>('block.save', { doc, ids: selection, name: trimmed, description })
      .then((block) => {
        notify(
          block.layout
            ? t('schematic.blocks.savedLayout', {
                name: block.name,
                parts: block.parts,
                tracks: block.tracks,
              })
            : t('schematic.blocks.saved', { name: block.name, parts: block.parts }),
        );
        setName('');
        setDescription('');
        load();
      })
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      })
      .finally(() => {
        setBusy(false);
      });
  };
  const place = (block: BlockSummary): void => {
    setBusy(true);
    engine
      .call<PlaceResult>('block.place', { name: block.name, doc })
      .then((result) => {
        notify(
          result.board === null
            ? t('schematic.blocks.placed', { name: block.name, parts: result.parts })
            : t('schematic.blocks.placedBoard', {
                name: block.name,
                parts: result.parts,
                footprints: result.board.footprints,
                tracks: result.board.tracks,
                vias: result.board.vias,
              }),
        );
        onPlaced(result.select);
        onClose();
      })
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
        setBusy(false);
      });
  };
  const remove = (block: BlockSummary): void => {
    engine
      .call('block.delete', { name: block.name })
      .then(() => {
        setDeleting(null);
        load();
      })
      .catch((error: unknown) => {
        notify(errorMessage(t, error), 'error');
      });
  };

  return (
    <Modal title={t('schematic.blocks.title')} onClose={onClose} testId="blocks-dialog">
      <section className="blocks__save">
        <h3 className="blocks__heading">{t('schematic.blocks.saveHeading')}</h3>
        <p className="field__hint">
          {selection.length === 0
            ? t('schematic.blocks.nothingSelected')
            : t('schematic.blocks.selected', { count: selection.length })}
        </p>
        <label className="field">
          <span className="field__label">{t('schematic.blocks.name')}</span>
          <input
            className="field__control"
            value={name}
            data-testid="block-name"
            onChange={(event) => {
              setName(event.target.value);
            }}
          />
        </label>
        <label className="field">
          <span className="field__label">{t('schematic.blocks.description')}</span>
          <input
            className="field__control"
            value={description}
            data-testid="block-description"
            onChange={(event) => {
              setDescription(event.target.value);
            }}
          />
        </label>
        <button
          type="button"
          className="button button--primary"
          disabled={busy || selection.length === 0 || trimmed === ''}
          data-testid="block-save"
          onClick={save}
        >
          {replaces ? t('schematic.blocks.replace') : t('schematic.blocks.save')}
        </button>
      </section>
      <section className="blocks__library">
        <h3 className="blocks__heading">{t('schematic.blocks.listHeading')}</h3>
        {blocks !== null && blocks.length === 0 && (
          <p className="field__hint" data-testid="blocks-empty">
            {t('schematic.blocks.empty')}
          </p>
        )}
        <ul className="blocks__list" data-testid="blocks-list">
          {(blocks ?? []).map((block, index) => (
            <li key={block.file} className="blocks__item" data-testid={`block-item-${index}`}>
              <div className="blocks__text">
                <strong className="blocks__name">{block.name}</strong>
                {block.description !== '' && (
                  <span className="blocks__description">{block.description}</span>
                )}
                <span className="blocks__counts">
                  {block.layout
                    ? t('schematic.blocks.countsLayout', {
                        parts: block.parts,
                        tracks: block.tracks,
                        vias: block.vias,
                      })
                    : t('schematic.blocks.counts', { parts: block.parts })}
                </span>
              </div>
              <div className="blocks__actions">
                <button
                  type="button"
                  className="button button--primary"
                  disabled={busy}
                  data-testid={`block-place-${index}`}
                  onClick={() => {
                    place(block);
                  }}
                >
                  {t('schematic.blocks.place')}
                </button>
                <button
                  type="button"
                  className={deleting === block.name ? 'button button--danger' : 'button'}
                  data-testid={`block-delete-${index}`}
                  onClick={() => {
                    if (deleting === block.name) {
                      remove(block);
                    } else {
                      setDeleting(block.name);
                    }
                  }}
                >
                  {deleting === block.name
                    ? t('schematic.blocks.deleteConfirm')
                    : t('schematic.blocks.delete')}
                </button>
              </div>
            </li>
          ))}
        </ul>
      </section>
    </Modal>
  );
}
