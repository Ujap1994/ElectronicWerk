import { useState, type ReactNode } from 'react';

import { historyLabel } from '../project/labels';
import { useProject } from '../project/ProjectProvider';
import { selectionKey, type TreeTarget } from '../project/tree';
import { useAppState } from '../state';
import { DocumentDetails } from './DocumentDetails';
import { Icon, type IconName } from './Icon';
import { ProjectTree } from './ProjectTree';

function ToolButton({
  icon,
  label,
  title,
  disabled = false,
  testId,
  onClick,
}: {
  readonly icon: IconName;
  readonly label: string;
  readonly title?: string;
  readonly disabled?: boolean;
  readonly testId: string;
  readonly onClick: () => void;
}): ReactNode {
  return (
    <button
      type="button"
      className="toolbar__button"
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
export function ProjectWorkspace({ projectView }: { readonly projectView: number }): ReactNode {
  const { t, appInfo } = useAppState();
  const { project, save, undo, redo, validate, archive, closeProject } = useProject();
  const [target, setTarget] = useState<TreeTarget>({ type: 'project' });
  const [shownView, setShownView] = useState(projectView);
  if (shownView !== projectView) {
    setShownView(projectView);
    setTarget({ type: 'project' });
  }
  if (project === null) {
    return null;
  }
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
      <div className="workspace__body">
        <aside className="workspace__sidebar">
          <ProjectTree selected={selectionKey(target)} onSelect={setTarget} />
        </aside>
        <main className="workspace__details" data-testid="details">
          <DocumentDetails target={target} />
        </main>
      </div>
    </div>
  );
}
