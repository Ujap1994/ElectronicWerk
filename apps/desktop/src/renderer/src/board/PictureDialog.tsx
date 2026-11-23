/**
 * A logo, picture or QR code on the board: the picture becomes black and
 * white pixels (threshold, invert), the pixels filled polygons on a layer
 * (`pcb.addImage`, `pcb.addQrCode`). The preview shows the pixels that go
 * onto the board. On bottom layers the picture is mirrored, so that it
 * reads right from below.
 */
import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react';

import { formatLength, formatSize, parseLength, unitSymbol, type Units } from '@shared/project';

import { Modal } from '../components/Modal';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import { layerLabel } from '../viewer/labels';

export type PictureTab = 'image' | 'qr';

const LAYERS = ['silk.top', 'silk.bottom', 'cu.top', 'cu.bottom', 'mask.top', 'mask.bottom'];
/** Pixel sizes (nm) of the detail levels. */
const DETAIL = { fine: 25_000, normal: 50_000, coarse: 100_000 } as const;
type Detail = keyof typeof DETAIL;
/** The most pixels across. */
const MAX_COLUMNS = 1000;

interface Loaded {
  readonly name: string;
  readonly image: HTMLImageElement;
}

/** Black-and-white rows of a picture: dark pixels are set. */
export function pictureRows(
  image: CanvasImageSource & { readonly width: number; readonly height: number },
  columns: number,
  threshold: number,
  invert: boolean,
  mirror: boolean,
): string[] {
  const rows = Math.max(1, Math.round((columns * image.height) / Math.max(1, image.width)));
  const canvas = document.createElement('canvas');
  canvas.width = columns;
  canvas.height = rows;
  const context = canvas.getContext('2d', { willReadFrequently: true });
  if (context === null) {
    return [];
  }
  // Transparent parts count as white.
  context.fillStyle = '#ffffff';
  context.fillRect(0, 0, columns, rows);
  context.imageSmoothingQuality = 'high';
  context.drawImage(image, 0, 0, columns, rows);
  const data = context.getImageData(0, 0, columns, rows).data;
  const out: string[] = [];
  for (let y = 0; y < rows; y += 1) {
    let row = '';
    for (let x = 0; x < columns; x += 1) {
      const i = (y * columns + (mirror ? columns - 1 - x : x)) * 4;
      const light =
        0.299 * (data[i] ?? 255) + 0.587 * (data[i + 1] ?? 255) + 0.114 * (data[i + 2] ?? 255);
      row += light < threshold !== invert ? '#' : '.';
    }
    out.push(row);
  }
  return out;
}

/** Draws rows of pixels: set ones in the colour of the layer's ink. */
function Preview({
  rows,
  testId,
}: {
  readonly rows: readonly string[];
  readonly testId: string;
}): ReactNode {
  const ref = useRef<HTMLCanvasElement>(null);
  const width = rows[0]?.length ?? 0;
  useEffect(() => {
    const canvas = ref.current;
    const context = canvas?.getContext('2d');
    if (canvas === null || context === undefined || context === null || width === 0) {
      return;
    }
    canvas.width = width;
    canvas.height = rows.length;
    const image = context.createImageData(width, rows.length);
    rows.forEach((row, y) => {
      for (let x = 0; x < width; x += 1) {
        const set = row[x] === '#';
        const i = (y * width + x) * 4;
        // Dark ink on a light ground, like the printout.
        const value = set ? 24 : 236;
        image.data[i] = value;
        image.data[i + 1] = value;
        image.data[i + 2] = value;
        image.data[i + 3] = 255;
      }
    });
    context.putImageData(image, 0, 0);
  }, [rows, width]);
  return (
    <canvas
      ref={ref}
      className="picture__preview"
      data-testid={testId}
      data-columns={width}
      data-rows={rows.length}
    />
  );
}

export function PictureDialog({
  doc,
  units,
  center,
  tab: initialTab,
  onAdded,
  onClose,
}: {
  readonly doc: string;
  readonly units: Units;
  /** Where the picture goes (the middle of the view). */
  readonly center: readonly [number, number];
  readonly tab: PictureTab;
  /** The new shapes, to select them. */
  readonly onAdded: (ids: readonly string[]) => void;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine, snapshot } = useAppState();
  const { notify } = useProject();
  const locale = snapshot.language;
  const [tab, setTab] = useState<PictureTab>(initialTab);
  const [layer, setLayer] = useState('silk.top');
  const [width, setWidth] = useState(units === 'mm' ? '15' : '600');
  const mirror = layer.endsWith('.bottom');

  // Picture.
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [threshold, setThreshold] = useState(128);
  const [invert, setInvert] = useState(false);
  const [detail, setDetail] = useState<Detail>('normal');
  // QR code.
  const [text, setText] = useState('https://');
  const [ecc, setEcc] = useState<'low' | 'medium' | 'quartile' | 'high'>('medium');
  const [light, setLight] = useState(true);
  const [qr, setQr] = useState<{ text: string; rows: string[] } | null>(null);

  const widthNm = parseLength(width, units);
  const validWidth = widthNm !== null && widthNm >= 1_000_000 && widthNm <= 500_000_000;
  const columns =
    widthNm === null ? 0 : Math.min(MAX_COLUMNS, Math.max(8, Math.round(widthNm / DETAIL[detail])));

  const imageRows = useMemo(
    () =>
      loaded === null || columns === 0
        ? []
        : pictureRows(loaded.image, columns, threshold, invert, mirror),
    [loaded, columns, threshold, invert, mirror],
  );

  useEffect(() => {
    if (tab !== 'qr' || text.trim() === '') {
      return undefined;
    }
    let cancelled = false;
    engine
      .call<{ rows: string[] }>('pcb.qrCode', { text, ecc, invert: light, quiet: light ? 2 : 0 })
      .then(
        (reply) => {
          if (!cancelled) {
            setQr({ text, rows: reply.rows });
          }
        },
        () => {
          if (!cancelled) {
            setQr(null);
          }
        },
      );
    return () => {
      cancelled = true;
    };
  }, [engine, tab, text, ecc, light]);

  const choose = (file: File): void => {
    const url = URL.createObjectURL(file);
    const image = new Image();
    image.src = url;
    image
      .decode()
      .then(() => {
        setLoaded({ name: file.name, image });
      })
      .catch(() => {
        notify(t('board.picture.unreadable', { name: file.name }), 'error');
      })
      .finally(() => {
        URL.revokeObjectURL(url);
      });
  };

  const qrRows = qr?.text === text ? qr.rows : [];
  const rows = tab === 'image' ? imageRows : qrRows;
  const ready = validWidth && rows.length > 0 && rows.some((row) => row.includes('#'));

  const add = (): void => {
    if (!ready) {
      return;
    }
    const pixel = Math.max(1000, Math.floor(widthNm / (rows[0]?.length ?? 1)));
    const call =
      tab === 'image'
        ? engine.call<{ select: string[] }>('pcb.addImage', {
            doc,
            layer,
            rows,
            pixel,
            center,
          })
        : engine.call<{ select: string[] }>('pcb.addQrCode', {
            doc,
            layer,
            text,
            size: widthNm,
            center,
            ecc,
            invert: light,
            quiet: light ? 2 : 0,
          });
    call.then(
      (reply) => {
        onAdded(reply.select);
        onClose();
      },
      (error: unknown) => {
        notify(errorMessage(t, error), 'error');
      },
    );
  };

  const height =
    widthNm === null || rows.length === 0
      ? null
      : Math.round((widthNm * rows.length) / Math.max(1, rows[0]?.length ?? 1));

  return (
    <Modal
      title={t('board.picture.title')}
      onClose={onClose}
      testId="picture-dialog"
      className="picture"
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            {t('board.cancel')}
          </button>
          <button
            type="button"
            className="button button--primary"
            disabled={!ready}
            data-testid="picture-add"
            onClick={add}
          >
            {t('board.picture.add')}
          </button>
        </>
      }
    >
      <div className="segmented" role="tablist">
        {(['image', 'qr'] as const).map((value) => (
          <button
            key={value}
            type="button"
            role="tab"
            aria-selected={tab === value}
            className={
              tab === value ? 'segmented__item segmented__item--active' : 'segmented__item'
            }
            data-testid={`picture-tab-${value}`}
            onClick={() => {
              setTab(value);
            }}
          >
            {t(`board.picture.tabs.${value}`)}
          </button>
        ))}
      </div>
      <div className="picture__body">
        <div className="picture__controls">
          {tab === 'image' ? (
            <>
              <p className="field__hint">{t('board.picture.imageHint')}</p>
              <label className="button picture__file">
                {loaded === null ? t('board.picture.choose') : loaded.name}
                <input
                  type="file"
                  accept="image/png,image/jpeg,image/gif,image/bmp,image/webp,image/svg+xml"
                  className="picture__input"
                  data-testid="picture-file"
                  onChange={(event) => {
                    const file = event.target.files?.[0];
                    if (file !== undefined) {
                      choose(file);
                    }
                  }}
                />
              </label>
              <label className="field">
                <span className="field__label">
                  {t('board.picture.threshold', { value: threshold })}
                </span>
                <input
                  type="range"
                  min={1}
                  max={254}
                  value={threshold}
                  data-testid="picture-threshold"
                  onChange={(event) => {
                    setThreshold(Number(event.target.value));
                  }}
                />
              </label>
              <label className="checkbox">
                <input
                  type="checkbox"
                  checked={invert}
                  data-testid="picture-invert"
                  onChange={(event) => {
                    setInvert(event.target.checked);
                  }}
                />
                <span>{t('board.picture.invert')}</span>
              </label>
              <label className="field">
                <span className="field__label">{t('board.picture.detail')}</span>
                <select
                  className="field__control"
                  value={detail}
                  data-testid="picture-detail"
                  onChange={(event) => {
                    const found = (Object.keys(DETAIL) as Detail[]).find(
                      (d) => d === event.target.value,
                    );
                    setDetail(found ?? 'normal');
                  }}
                >
                  {(Object.keys(DETAIL) as Detail[]).map((d) => (
                    <option key={d} value={d}>
                      {t(`board.picture.details.${d}`, {
                        size: formatLength(DETAIL[d], units, locale),
                      })}
                    </option>
                  ))}
                </select>
              </label>
            </>
          ) : (
            <>
              <p className="field__hint">{t('board.picture.qrHint')}</p>
              <label className="field">
                <span className="field__label">{t('board.picture.text')}</span>
                <input
                  className="field__control"
                  value={text}
                  data-testid="picture-text"
                  onChange={(event) => {
                    setText(event.target.value);
                  }}
                />
              </label>
              <label className="field">
                <span className="field__label">{t('board.picture.ecc')}</span>
                <select
                  className="field__control"
                  value={ecc}
                  data-testid="picture-ecc"
                  onChange={(event) => {
                    const found = (['low', 'medium', 'quartile', 'high'] as const).find(
                      (e) => e === event.target.value,
                    );
                    setEcc(found ?? 'medium');
                  }}
                >
                  {(['low', 'medium', 'quartile', 'high'] as const).map((e) => (
                    <option key={e} value={e}>
                      {t(`board.picture.eccs.${e}`)}
                    </option>
                  ))}
                </select>
              </label>
              <label className="checkbox">
                <input
                  type="checkbox"
                  checked={light}
                  data-testid="picture-light"
                  onChange={(event) => {
                    setLight(event.target.checked);
                  }}
                />
                <span>{t('board.picture.light')}</span>
              </label>
            </>
          )}
          <label className="field">
            <span className="field__label">{t('board.picture.layer')}</span>
            <select
              className="field__control"
              value={layer}
              data-testid="picture-layer"
              onChange={(event) => {
                setLayer(event.target.value);
              }}
            >
              {LAYERS.map((key) => (
                <option key={key} value={key}>
                  {layerLabel(t, key, null)}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            <span className="field__label">{t('board.picture.width')}</span>
            <span className="picture__width">
              <input
                className="field__control"
                value={width}
                aria-invalid={!validWidth}
                data-testid="picture-width"
                onChange={(event) => {
                  setWidth(event.target.value);
                }}
              />
              <span>{unitSymbol(units)}</span>
            </span>
          </label>
          {mirror && <p className="field__hint">{t('board.picture.mirrored')}</p>}
        </div>
        <div className="picture__side">
          {rows.length > 0 ? (
            <Preview rows={rows} testId="picture-preview" />
          ) : (
            <p className="picture__empty">
              {tab === 'image' ? t('board.picture.noImage') : t('board.picture.noText')}
            </p>
          )}
          {height !== null && widthNm !== null && (
            <p className="field__hint" data-testid="picture-size">
              {t('board.picture.size', {
                size: formatSize(widthNm, height, units, locale),
                columns: rows[0]?.length ?? 0,
                rows: rows.length,
              })}
            </p>
          )}
        </div>
      </div>
    </Modal>
  );
}
