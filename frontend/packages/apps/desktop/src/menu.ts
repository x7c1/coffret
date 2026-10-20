// The app's menu.

import type { MenuItemConstructorOptions } from 'electron';

/** The label of the item that gives the explorer a fresh launch ticket. */
export const RELOAD_THE_EXPLORER = 'Reload the explorer';

/**
 * The application menu: the platform's usual app, edit and window menus, and
 * *Reload the explorer* (Cmd/Ctrl+R) in place of a page reload.
 *
 * A plain reload would load the explorer's page again without a ticket, and a
 * page that lost its session would still have none. The item loads the
 * explorer from a fresh ticket instead, which is what such a page needs. No
 * item reloads a page as it is or opens the developer tools.
 */
export function menuTemplate(platform: NodeJS.Platform, reloadExplorer: () => void): MenuItemConstructorOptions[] {
  const explorer: MenuItemConstructorOptions = {
    label: 'Explorer',
    submenu: [{ label: RELOAD_THE_EXPLORER, accelerator: 'CmdOrCtrl+R', click: () => reloadExplorer() }],
  };
  if (platform === 'darwin') {
    return [{ role: 'appMenu' }, { role: 'editMenu' }, explorer, { role: 'windowMenu' }];
  }
  return [{ role: 'fileMenu' }, { role: 'editMenu' }, explorer];
}
