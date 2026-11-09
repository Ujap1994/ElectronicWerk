import { useCallback, useEffect, useState, type ReactNode } from 'react';

import type { DocumentSummary } from '@shared/project';

import { historyLabel } from '../project/labels';
import { useProject } from '../project/ProjectProvider';
import { selectionKey, type TreeTarget } from '../project/tree';
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
  const { project, save, undo, redo, validate, archive, closeProject } = useProject();
  const [target, setTarget] = useState<TreeTarget>({ type: 'project' });
  const [shownView, setShownView] = useState(projectView);
  const [tabs, setTabs] = useState<readonly string[]>([]);
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
      resetSchematicStores();
    },
    [projectId],
  );
  const openDocument = useCallback((document: DocumentSummary) => {
    setTabs((current) => (current.includes(document.id) ? current : [...current, document.id]));
    setActiveTab(document.id);
  }, []);
  if (project === null) {
    return null;
  }
  // Tabs of removed documents disappear.
  const openTabs = tabs
    .map((id) => project.documents.find((document) => document.id === id))
    .filter((document) => document !== undefined);
  const active = openTabs.find((document) => document.id === activeTab) ?? null;
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
                      <Icon name={document.kind === 'pcb' ? 'board' : 'schematic'} size={16} />
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
                <DocumentViewer key={active.id} document={active} units={project.settings.units} />
              </main>
            )}
          </div>
        </div>
      </OpenDocumentContext>
    </div>
  );
}
