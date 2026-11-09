import { useEffect, useState, useSyncExternalStore, type ReactNode } from 'react';

import type { ExportKind } from '@shared/api';
import type { MessageKey } from '@shared/i18n';
import { formatLength, isEngineEvent, joinPath, type Units } from '@shared/project';

import { Icon } from '../components/Icon';
import { useProject } from '../project/ProjectProvider';
import { errorMessage } from '../project/labels';
import { useAppState } from '../state';
import type { ViewerController } from '../viewer/controller';
import { Scene } from '../viewer/scene';
import type { SceneReply } from '../viewer/types';
import type { SchematicEditor } from './editor';
import { ercState, type ErcReport } from './stores';
import { printDocument, sceneToSvg } from './svg';

/** Schematic grid sizes offered by the grid assistant (nm). */
const GRID_SIZES = [635_000, 1_270_000, 2_540_000, 500_000, 1_000_000, 2_500_000];

function safeName(name: string): string {
  return name.replace(/[\\/:*?"<>|]+/gu, '_').trim() || 'export';
}

/** The export menu: netlists, bill of materials, SVG and PDF. */
export function ExportMenu({
  controller,
  docName,
  onClose,
}: {
  readonly controller: ViewerController;
  readonly docName: string;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine, appInfo } = useAppState();
  const { project, notify } = useProject();
  const [busy, setBusy] = useState(false);
  if (project === null) {
    return null;
  }
  const save = async (
    kind: ExportKind,
    name: string,
    content: () => Promise<string>,
  ): Promise<void> => {
    setBusy(true);
    try {
      const text = await content();
      const path = await window.ew.dialogs.saveExport(
        kind,
        joinPath(project.path, name, appInfo.platform),
        text,
      );
      if (path !== null) {
        notify(t('schematic.export.saved', { path }));
      }
      onClose();
    } catch (error) {
      notify(errorMessage(t, error), 'error');
    } finally {
      setBusy(false);
    }
  };
  const base = safeName(project.name);
  const labels = {
    file: t('viewer.titleFile'),
    size: t('viewer.titleSize'),
    date: t('viewer.titleDate'),
    revision: t('viewer.titleRevision'),
  };
  const items: readonly { key: MessageKey; testId: string; run: () => Promise<void> }[] = [
    {
      key: 'schematic.export.netlistKicad',
      testId: 'export-netlist-kicad',
      run: () =>
        save(
          'netlist',
          `${base}.net`,
          async () =>
            (await engine.call<{ text: string }>('schematic.netlist', { format: 'kicad' })).text,
        ),
    },
    {
      key: 'schematic.export.netlistCsv',
      testId: 'export-netlist-csv',
      run: () =>
        save(
          'csv',
          `${base}-netlist.csv`,
          async () =>
            (await engine.call<{ text: string }>('schematic.netlist', { format: 'csv' })).text,
        ),
    },
    {
      key: 'schematic.export.bom',
      testId: 'export-bom',
      run: () =>
        save(
          'bom',
          `${base}-bom.csv`,
          async () => (await engine.call<{ text: string }>('schematic.bom')).text,
        ),
    },
    {
      key: 'schematic.export.svg',
      testId: 'export-svg',
      run: () =>
        save('svg', `${safeName(docName)}.svg`, () => {
          const scene = controller.currentScene;
          return Promise.resolve(scene === null ? '' : sceneToSvg(scene, docName).svg);
        }),
    },
    {
      key: 'schematic.export.pdf',
      testId: 'export-pdf',
      run: () =>
        save('pdf', `${base}.pdf`, async () => {
          const sheets = [];
          for (const document of project.documents.filter((d) => d.kind === 'schematic')) {
            const reply = await engine.call<SceneReply>(
              'view.scene',
              { doc: document.id, labels },
              120_000,
            );
            sheets.push(sceneToSvg(new Scene(reply), document.name));
          }
          return printDocument(sheets, project.name);
        }),
    },
  ];
  return (
    <div
      className="popover export-menu"
      role="menu"
      aria-label={t('schematic.export.title')}
      data-testid="export-menu"
    >
      {items.map((item) => (
        <button
          key={item.testId}
          type="button"
          role="menuitem"
          className="popover__item"
          disabled={busy}
          data-testid={item.testId}
          onClick={() => {
            void item.run();
          }}
        >
          {t(item.key)}
        </button>
      ))}
    </div>
  );
}

/** Grid size, snapping and a check for wire ends off the grid. */
export function GridAssistant({
  doc,
  editor,
  units,
  onClose,
}: {
  readonly doc: string;
  readonly editor: SchematicEditor;
  readonly units: Units;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine, snapshot: settings } = useAppState();
  const { project, setProperties, notify } = useProject();
  const state = useSyncExternalStore(editor.subscribe, editor.snapshot);
  const [offGrid, setOffGrid] = useState<number | null>(null);
  const grid = project?.settings.schematicGrid ?? 1_270_000;

  useEffect(() => {
    let cancelled = false;
    const check = (): void => {
      void engine.call<ErcReport>('schematic.erc').then(
        (report) => {
          if (!cancelled) {
            setOffGrid(
              report.issues.filter((i) => i.code === 'off-grid' && i.sheet === doc).length,
            );
          }
        },
        () => undefined,
      );
    };
    check();
    const off = engine.onEvent((event) => {
      if (isEngineEvent(event)) {
        check();
      }
    });
    return () => {
      cancelled = true;
      off();
    };
  }, [engine, doc, grid]);

  return (
    <div className="popover grid-assistant" data-testid="grid-assistant">
      <header className="popover__header">
        <h2 className="popover__title">{t('schematic.grid.title')}</h2>
        <button
          type="button"
          className="button button--small button--quiet"
          aria-label={t('settings.close')}
          onClick={onClose}
        >
          <Icon name="close" size={14} />
        </button>
      </header>
      <label className="checkbox">
        <input
          type="checkbox"
          checked={state.snap}
          data-testid="grid-snap"
          onChange={(event) => {
            editor.setSnap(event.target.checked);
          }}
        />
        {t('schematic.grid.snap')}
      </label>
      <label className="field field--compact">
        <span className="field__label">{t('schematic.grid.size')}</span>
        <select
          className="field__control"
          value={grid}
          data-testid="grid-size"
          onChange={(event) => {
            if (project !== null) {
              void setProperties({
                settings: { ...project.settings, schematicGrid: Number(event.target.value) },
              });
            }
          }}
        >
          {(GRID_SIZES.includes(grid) ? GRID_SIZES : [grid, ...GRID_SIZES]).map((size) => (
            <option key={size} value={size}>
              {formatLength(size, units, settings.language)}
            </option>
          ))}
        </select>
      </label>
      <p className="field__hint">{t('schematic.grid.hint')}</p>
      {offGrid !== null && (
        <p
          className={offGrid === 0 ? 'grid-assistant__ok' : 'grid-assistant__warn'}
          data-testid="grid-status"
        >
          {offGrid === 0
            ? t('schematic.grid.onGrid')
            : t('schematic.grid.offGrid', { count: offGrid })}
        </p>
      )}
      {offGrid !== null && offGrid > 0 && (
        <button
          type="button"
          className="button button--small"
          data-testid="grid-fix"
          onClick={() => {
            engine.call('schematic.snapToGrid', { doc }).catch((error: unknown) => {
              notify(errorMessage(t, error), 'error');
            });
          }}
        >
          {t('schematic.grid.fix')}
        </button>
      )}
    </div>
  );
}

interface SheetContent {
  readonly symbols?: readonly { readonly designator?: string }[];
  readonly wires?: readonly unknown[];
  readonly powerPorts?: readonly unknown[];
}

const STEPS = ['place', 'wire', 'power', 'annotate', 'erc'] as const;

/** The guided first schematic: five steps that tick themselves off. */
export function Guide({
  doc,
  onClose,
}: {
  readonly doc: string;
  readonly onClose: () => void;
}): ReactNode {
  const { t, engine } = useAppState();
  const erc = useSyncExternalStore(ercState.subscribe, ercState.get);
  const [content, setContent] = useState<SheetContent | null>(null);

  useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const load = (): void => {
      void engine.call<SheetContent>('document.get', { doc }).then(
        (sheet) => {
          if (!cancelled) {
            setContent(sheet);
          }
        },
        () => undefined,
      );
    };
    load();
    const off = engine.onEvent((event) => {
      if (isEngineEvent(event)) {
        clearTimeout(timer);
        timer = setTimeout(load, 150);
      }
    });
    return () => {
      cancelled = true;
      clearTimeout(timer);
      off();
    };
  }, [engine, doc]);

  const symbols = content?.symbols ?? [];
  const parts = symbols.filter((s) => !(s.designator ?? '').startsWith('#'));
  const power =
    (content?.powerPorts?.length ?? 0) +
    symbols.filter((s) => (s.designator ?? '').startsWith('#PWR')).length;
  const done: Readonly<Record<(typeof STEPS)[number], boolean>> = {
    place: parts.length >= 2,
    wire: (content?.wires?.length ?? 0) > 0,
    power: power >= 2,
    annotate: parts.length > 0 && parts.every((s) => !(s.designator ?? '?').endsWith('?')),
    erc: erc.report !== null,
  };
  const all = STEPS.every((step) => done[step]);
  return (
    <aside className="guide" aria-label={t('schematic.guide.title')} data-testid="guide">
      <header className="popover__header">
        <h2 className="popover__title">{t('schematic.guide.title')}</h2>
        <button
          type="button"
          className="button button--small button--quiet"
          aria-label={t('schematic.guide.hide')}
          title={t('schematic.guide.hide')}
          data-testid="guide-close"
          onClick={onClose}
        >
          <Icon name="close" size={14} />
        </button>
      </header>
      <p className="guide__intro">{t('schematic.guide.intro')}</p>
      <ol className="guide__steps">
        {STEPS.map((step) => (
          <li
            key={step}
            className="guide__step"
            data-done={done[step]}
            data-testid={`guide-${step}`}
          >
            <span className="guide__check" aria-hidden="true">
              {done[step] ? <Icon name="check" size={14} /> : null}
            </span>
            <span>{t(`schematic.guide.steps.${step}`)}</span>
          </li>
        ))}
      </ol>
      {all && <p className="guide__done">{t('schematic.guide.done')}</p>}
    </aside>
  );
}
