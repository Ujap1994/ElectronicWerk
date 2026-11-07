import { Menu, type MenuItemConstructorOptions } from 'electron';

import type { MenuAction } from '@shared/api';
import { DOCS_URL, PROJECT_URL } from '@shared/links';
import type { Translate } from '@shared/i18n';

import { openExternalSafely } from './security';

export interface MenuOptions {
  readonly t: Translate;
  readonly isMac: boolean;
  readonly isPackaged: boolean;
  readonly onAction: (action: MenuAction) => void;
  readonly onAbout: () => void;
}

/** Builds the translated application menu. */
export function buildMenu({ t, isMac, isPackaged, onAction, onAbout }: MenuOptions): Menu {
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

  const fileMenu: MenuItemConstructorOptions = {
    label: t('menu.file'),
    submenu: [
      { label: t('menu.newProject'), accelerator: 'CmdOrCtrl+N', enabled: false },
      { label: t('menu.openProject'), accelerator: 'CmdOrCtrl+O', enabled: false },
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
      { label: t('menu.undo'), role: 'undo' },
      { label: t('menu.redo'), role: 'redo' },
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
