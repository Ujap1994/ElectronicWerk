import { useCallback, useEffect, useState, type ReactNode } from 'react';

import type { DocumentSummary } from '@shared/project';

import { LibraryEditor } from '../libedit/LibraryEditor';
import { GerberViewer } from '../outputs/GerberViewer';
import { forgetOutputRuns, OpenGerberContext, OutputJobEditor } from '../outputs/OutputJobEditor';
import { outputCommands } from '../outputs/stores';
import { historyLabel } from '../project/labels';
import { useProject } from '../project/ProjectProvider';
import { selectionKey, type TreeTarget } from '../project/tree';
import { resetBoardStores } from '../board/stores';
import { resetSchematicStores } from '../schematic/stores';
import { useAppState } from '../state';
import { OpenDocumentContext } from '../viewer/open-context';
import { DocumentDetails } from './DocumentDetails';
import { DocumentViewer, forgetViewMemories } from './DocumentViewer';
import { Icon, type IconName } from './Icon';
import { ProjectTree } from './ProjectTree';

function ToolButton({
  icon,
  label,
  title,
  disabled = false,
  iconOnly = false,
  testId,
  onClick,
}: {
  readonly icon: IconName;
  readonly label: string;
  readonly title?: string;
  readonly disabled?: boolean;
  /** Shows the icon only (the label stays for screen readers). */
  readonly iconOnly?: boolean;
  readonly testId: string;
  readonly onClick: () => void;
}): ReactNode {
  return (
    <button
      type="button"
      className={`toolbar__button${iconOnly ? ' toolbar__button--icon' : ''}`}
      title={title ?? label}
      disabled={disabled}
      data-testid={testId}
      onClick={onClick}
    >
      <Icon name={icon} />
      <span className="toolbar__label">{label}</span>
    </button>
  );
}

/** An open tab: a document or a Gerber view. */
interface OpenTab {
  readonly id: string;
  readonly name: string;
  readonly kind: string;
  readonly document: DocumentSummary | null;
  readonly gerber: { readonly name: string; readonly paths: readonly string[] } | null;
}

const TAB_ICONS: Readonly<Record<string, IconName>> = {
  pcb: 'board',
  library: 'library',
  schematic: 'schematic',
  outputJob: 'output',
  gerber: 'layers',
};

/**
 * The open project: toolbar, project tree and details. Increasing
 * `projectView` selects the project properties (menu command).
 */
export function ProjectWorkspace({
  projectView,
  onOpenLibrary,
}: {
  readonly projectView: number;
  readonly onOpenLibrary: () => void;
}): ReactNode {
  const { t, appInfo } = useAppState();
  const { project, save, undo, redo, validate, archive, closeProject, addDocument } = useProject();
  const [target, setTarget] = useState<TreeTarget>({ type: 'project' });
  const [shownView, setShownView] = useState(projectView);
  const [tabs, setTabs] = useState<readonly string[]>([]);
  const [gerberTabs, setGerberTabs] = useState<
    Readonly<Record<string, { readonly name: string; readonly paths: readonly string[] }>>
  >({});
  const [activeTab, setActiveTab] = useState<string | null>(null);
  if (shownView !== projectView) {
    setShownView(projectView);
    setTarget({ type: 'project' });
    setActiveTab(null);
  }
  const projectId = project?.id ?? null;
  useEffect(
    () => () => {
      forgetViewMemories();
      forgetOutputRuns();
      resetSchematicStores();
      resetBoardStores();
    },
    [projectId],
  );
  const openDocument = useCallback((document: DocumentSummary) => {
    setTabs((current) => (current.includes(document.id) ? current : [...current, document.id]));
    setActiveTab(document.id);
  }, []);
  const openGerber = useCallback((name: string, paths: readonly string[]) => {
    const id = `gerber:${name}`;
    setGerberTabs((current) => ({ ...current, [id]: { name, paths } }));
    setTabs((current) => (current.includes(id) ? current : [...current, id]));
    setActiveTab(id);
  }, []);
  const firstJob = project?.documents.find((document) => document.kind === 'outputJob') ?? null;
  const jobName = t('defaults.outputJob');
  useEffect(() => {
    outputCommands.set({
      openJob: () => {
        if (firstJob !== null) {
          openDocument(firstJob);
          return;
        }
        void addDocument('outputJob', jobName).then((id) => {
          if (id !== null) {
            setTabs((current) => (current.includes(id) ? current : [...current, id]));
            setActiveTab(id);
          }
        });
      },
      openGerbers: () => {
        void window.ew.dialogs.chooseGerbers().then((paths) => {
          if (paths !== null && paths.length > 0) {
            const first = paths[0] ?? '';
            const name = first.split(/[\\/]/u).pop() ?? first;
            openGerber(name, paths);
          }
        });
      },
    });
    return () => {
      outputCommands.set(null);
    };
  }, [addDocument, firstJob, jobName, openDocument, openGerber]);
  if (project === null) {
    return null;
  }
  // Tabs of removed documents disappear.
  const openTabs = tabs
    .map((id): OpenTab | undefined => {
      const gerber = gerberTabs[id];
      if (gerber !== undefined) {
        return {
          id,
          name: t('outputs.viewer.tab', { name: gerber.name }),
          kind: 'gerber',
          document: null,
          gerber,
        };
      }
      const document = project.documents.find((entry) => entry.id === id);
      return document === undefined
        ? undefined
        : { id, name: document.name, kind: document.kind, document, gerber: null };
    })
    .filter((tab) => tab !== undefined);
  const active = openTabs.find((tab) => tab.id === activeTab) ?? null;
  const closeTab = (id: string): void => {
    const index = openTabs.findIndex((document) => document.id === id);
    setTabs((current) => current.filter((entry) => entry !== id));
    if (activeTab === id) {
      const next = openTabs[index + 1] ?? openTabs[index - 1];
      setActiveTab(next === undefined || next.id === id ? null : next.id);
    }
  };
  const { history } = project;
  const mod = appInfo.platform === 'darwin' ? '⌘' : 'Ctrl+';
  const undoTitle =
    history.undoLabel === null
      ? t('project.undo')
      : t('project.undoAction', { action: historyLabel(t, history.undoLabel) });
  const redoTitle =
    history.redoLabel === null
      ? t('project.redo')
      : t('project.redoAction', { action: historyLabel(t, history.redoLabel) });

  return (
    <div className="workspace" data-testid="workspace">
      <header className="toolbar" aria-label={t('project.toolbar')}>
        <div className="toolbar__title">
          <Icon name="project" size={20} className="toolbar__logo" />
          <span className="toolbar__name" data-testid="project-title">
            {project.name}
          </span>
          {project.revision !== '' && <span className="toolbar__revision">{project.revision}</span>}
          <span className="toolbar__state" data-dirty={history.dirty} data-testid="dirty-indicator">
            {history.dirty ? t('project.unsaved') : t('project.allSaved')}
          </span>
        </div>
        <div className="toolbar__group">
          <ToolButton
            icon="save"
            label={t('project.save')}
            title={`${t('project.save')} (${mod}S)`}
            disabled={!history.dirty && !project.needsUpgrade}
            testId="toolbar-save"
            onClick={() => {
              void save();
            }}
          />
          <ToolButton
            icon="undo"
            label={t('project.undo')}
            title={undoTitle}
            disabled={!history.canUndo}
            testId="toolbar-undo"
            onClick={undo}
          />
          <ToolButton
            icon="redo"
            label={t('project.redo')}
            title={redoTitle}
            disabled={!history.canRedo}
            testId="toolbar-redo"
            onClick={redo}
          />
        </div>
        <div className="toolbar__group">
          <ToolButton
            icon="check"
            label={t('project.validate')}
            testId="toolbar-validate"
            onClick={validate}
          />
          <ToolButton
            icon="archive"
            label={t('project.archive')}
            testId="toolbar-archive"
            onClick={() => {
              void archive();
            }}
          />
          <ToolButton
            icon="output"
            label={t('project.outputs')}
            title={t('commands.generateOutputs')}
            testId="toolbar-outputs"
            onClick={() => {
              outputCommands.get()?.openJob();
            }}
          />
        </div>
        <div className="toolbar__group">
          <ToolButton
            icon="library"
            label={t('library.open')}
            title={`${t('library.open')} (${mod}${appInfo.platform === 'darwin' ? '⇧' : 'Shift+'}L)`}
            testId="toolbar-library"
            iconOnly
            onClick={onOpenLibrary}
          />
        </div>
        <div className="toolbar__spacer" />
        <ToolButton
          icon="close"
          label={t('project.close')}
          testId="toolbar-close"
          onClick={() => {
            void closeProject();
          }}
        />
      </header>
      <OpenDocumentContext value={openDocument}>
        <OpenGerberContext value={openGerber}>
          <div className="workspace__body">
            <aside className="workspace__sidebar">
              <ProjectTree
                selected={selectionKey(target)}
                onSelect={(next) => {
                  setTarget(next);
                  if (active !== null) {
                    setActiveTab(null);
                  }
                }}
              />
            </aside>
            <div className="workspace__main">
              {openTabs.length > 0 && (
                <div className="tabs" role="tablist" aria-label={t('viewer.tabs')}>
                  <button
                    type="button"
                    role="tab"
                    className="tabs__tab"
                    aria-selected={active === null}
                    data-testid="tab-details"
                    onClick={() => {
                      setActiveTab(null);
                    }}
                  >
                    <Icon name="info" size={16} />
                    <span>{t('viewer.details')}</span>
                  </button>
                  {openTabs.map((document) => (
                    <div
                      key={document.id}
                      className="tabs__item"
                      data-selected={document.id === active?.id}
                    >
                      <button
                        type="button"
                        role="tab"
                        className="tabs__tab"
                        aria-selected={document.id === active?.id}
                        data-testid={`tab-${document.id}`}
                        onClick={() => {
                          setActiveTab(document.id);
                        }}
                      >
                        <Icon name={TAB_ICONS[document.kind] ?? 'schematic'} size={16} />
                        <span>{document.name}</span>
                      </button>
                      <button
                        type="button"
                        className="tabs__close"
                        aria-label={t('viewer.closeTab', { name: document.name })}
                        title={t('viewer.closeTab', { name: document.name })}
                        data-testid={`tab-close-${document.id}`}
                        onClick={() => {
                          closeTab(document.id);
                        }}
                      >
                        <Icon name="close" size={14} />
                      </button>
                    </div>
                  ))}
                </div>
              )}
              {active === null ? (
                <main className="workspace__details" data-testid="details">
                  <DocumentDetails target={target} />
                </main>
              ) : (
                <main className="workspace__viewer">
                  {active.gerber !== null ? (
                    <GerberViewer
                      key={`${active.id}:${active.gerber.paths.join('|')}`}
                      name={active.gerber.name}
                      paths={active.gerber.paths}
                      units={project.settings.units}
                    />
                  ) : active.document === null ? null : active.document.kind === 'library' ? (
                    <LibraryEditor
                      key={active.id}
                      document={active.document}
                      units={project.settings.units}
                    />
                  ) : active.document.kind === 'outputJob' ? (
                    <OutputJobEditor key={active.id} document={active.document} />
                  ) : (
                    <DocumentViewer
                      key={active.id}
                      document={active.document}
                      units={project.settings.units}
                    />
                  )}
                </main>
              )}
            </div>
          </div>
        </OpenGerberContext>
      </OpenDocumentContext>
    </div>
  );
}
