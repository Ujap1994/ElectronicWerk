import { Menu, type MenuItemConstructorOptions } from 'electron';

import type { MenuAction, WindowState } from '@shared/api';
import { DOCS_URL, PROJECT_URL } from '@shared/links';
import type { Translate } from '@shared/i18n';
import type { RecentProject } from '@shared/recent';

import { openExternalSafely } from './security';

export interface MenuOptions {
  readonly t: Translate;
  readonly isMac: boolean;
  readonly isPackaged: boolean;
  readonly onAction: (action: MenuAction) => void;
  readonly onAbout: () => void;
  readonly state: WindowState;
  readonly recent: readonly RecentProject[];
  readonly onOpenRecent: (manifestPath: string) => void;
  readonly onClearRecent: () => void;
}

/** Builds the translated application menu. */
export function buildMenu({
  t,
  isMac,
  isPackaged,
  onAction,
  onAbout,
  state,
  recent,
  onOpenRecent,
  onClearRecent,
}: MenuOptions): Menu {
  const action = (
    label: string,
    command: MenuAction,
    extra: Partial<MenuItemConstructorOptions> = {},
  ): MenuItemConstructorOptions => ({
    label,
    click: () => {
      onAction(command);
    },
    ...extra,
  });
  const open = state.projectOpen;
  const updatesItem: MenuItemConstructorOptions = {
    label: t('menu.checkForUpdates'),
    click: () => {
      onAction('check-updates');
    },
  };

  const settingsItem: MenuItemConstructorOptions = {
    label: t('menu.settings'),
    accelerator: 'CmdOrCtrl+,',
    click: () => {
      onAction('settings');
    },
  };

  const appMenu: MenuItemConstructorOptions[] = isMac
    ? [
        {
          label: t('app.name'),
          submenu: [
            { label: t('menu.about'), role: 'about' },
            updatesItem,
            { type: 'separator' },
            settingsItem,
            { type: 'separator' },
            { label: t('menu.hide'), role: 'hide' },
            { label: t('menu.hideOthers'), role: 'hideOthers' },
            { label: t('menu.showAll'), role: 'unhide' },
            { type: 'separator' },
            { label: t('menu.quit'), role: 'quit' },
          ],
        },
      ]
    : [];

  const recentItems: MenuItemConstructorOptions[] =
    recent.length === 0
      ? [{ label: t('menu.noRecent'), enabled: false }]
      : [
          ...recent.map((project): MenuItemConstructorOptions => ({
            label: project.name === '' ? project.manifestPath : project.name,
            sublabel: project.manifestPath,
            toolTip: project.manifestPath,
            click: () => {
              onOpenRecent(project.manifestPath);
            },
          })),
          { type: 'separator' },
          { label: t('menu.clearRecent'), click: onClearRecent },
        ];

  const fileMenu: MenuItemConstructorOptions = {
    label: t('menu.file'),
    submenu: [
      action(t('menu.newProject'), 'new-project', { accelerator: 'CmdOrCtrl+N' }),
      action(t('menu.openProject'), 'open-project', { accelerator: 'CmdOrCtrl+O' }),
      { label: t('menu.openRecent'), submenu: recentItems },
      { type: 'separator' },
      action(t('menu.save'), 'save', { accelerator: 'CmdOrCtrl+S', enabled: open }),
      action(t('menu.saveAs'), 'save-as', { accelerator: 'CmdOrCtrl+Shift+S', enabled: open }),
      action(t('menu.saveCopy'), 'save-copy', { enabled: open }),
      action(t('menu.archiveProject'), 'archive', { enabled: open }),
      { type: 'separator' },
      action(t('menu.projectProperties'), 'project-properties', { enabled: open }),
      action(t('menu.validateProject'), 'validate', { enabled: open }),
      { type: 'separator' },
      action(t('menu.closeProject'), 'close-project', { enabled: open }),
      ...(isMac
        ? []
        : ([
            { type: 'separator' },
            settingsItem,
            { type: 'separator' },
            { label: t('menu.quit'), role: 'quit' },
          ] satisfies MenuItemConstructorOptions[])),
    ],
  };

  const editMenu: MenuItemConstructorOptions = {
    label: t('menu.edit'),
    submenu: [
      // Undo/redo go to the renderer: text fields use their own undo, the
      // project uses the engine history.
      action(t('menu.undo'), 'undo', { accelerator: 'CmdOrCtrl+Z' }),
      action(t('menu.redo'), 'redo', {
        accelerator: isMac ? 'Shift+CmdOrCtrl+Z' : 'CmdOrCtrl+Y',
      }),
      { type: 'separator' },
      { label: t('menu.cut'), role: 'cut' },
      { label: t('menu.copy'), role: 'copy' },
      { label: t('menu.paste'), role: 'paste' },
      { label: t('menu.selectAll'), role: 'selectAll' },
    ],
  };

  const viewMenu: MenuItemConstructorOptions = {
    label: t('menu.view'),
    submenu: [
      ...(isPackaged
        ? []
        : ([
            { label: t('menu.reload'), role: 'reload' },
            { label: t('menu.toggleDevTools'), role: 'toggleDevTools' },
            { type: 'separator' },
          ] satisfies MenuItemConstructorOptions[])),
      {
        label: t('menu.commandPalette'),
        accelerator: 'CmdOrCtrl+K',
        click: () => {
          onAction('command-palette');
        },
      },
      { type: 'separator' },
      { label: t('menu.actualSize'), role: 'resetZoom' },
      { label: t('menu.zoomIn'), role: 'zoomIn' },
      { label: t('menu.zoomOut'), role: 'zoomOut' },
      { type: 'separator' },
      { label: t('menu.toggleFullScreen'), role: 'togglefullscreen' },
    ],
  };

  const windowMenu: MenuItemConstructorOptions[] = isMac
    ? [
        {
          label: t('menu.window'),
          role: 'windowMenu',
          submenu: [
            { label: t('menu.minimize'), role: 'minimize' },
            { label: t('menu.zoom'), role: 'zoom' },
          ],
        },
      ]
    : [];

  const helpMenu: MenuItemConstructorOptions = {
    label: t('menu.help'),
    role: 'help',
    submenu: [
      {
        label: t('menu.documentation'),
        click: () => {
          openExternalSafely(DOCS_URL);
        },
      },
      {
        label: t('menu.reportIssue'),
        click: () => {
          openExternalSafely(`${PROJECT_URL}/issues`);
        },
      },
      ...(isMac
        ? []
        : ([
            { type: 'separator' },
            updatesItem,
            { label: t('menu.about'), click: onAbout },
          ] satisfies MenuItemConstructorOptions[])),
    ],
  };

  return Menu.buildFromTemplate([
    ...appMenu,
    fileMenu,
    editMenu,
    viewMenu,
    ...windowMenu,
    helpMenu,
  ]);
}
