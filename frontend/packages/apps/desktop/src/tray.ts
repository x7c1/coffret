// The tray icon's menu: what is left of the app once the explorer's window is
// closed, which keeps the app — and the server in it — running.

import type { MenuItemConstructorOptions } from 'electron';

export const OPEN_THE_EXPLORER = 'Open the explorer';
export const UNLOCK = 'Unlock…';
export const QUIT_COFFRET = 'Quit Coffret';

/** What the tray's items do. */
export interface TrayActions {
  /** Brings the explorer's window back, as a second launch does. */
  openExplorer(): void;
  /** Brings the Passphrase window forward to unlock the Library. */
  unlock(): void;
  /** Ends the app. */
  quit(): void;
}

/**
 * The tray's menu while the Library is `locked` or not: *Open the explorer*,
 * *Unlock…* — offered only while there is something to unlock — and, apart,
 * *Quit Coffret*.
 *
 * Quitting ends the server and so every browser session with it, which is
 * why there is no item of its own for ending the sessions: the explorer has
 * one window, which a reload gives a fresh session.
 */
export function trayMenuTemplate(locked: boolean, actions: TrayActions): MenuItemConstructorOptions[] {
  return [
    { label: OPEN_THE_EXPLORER, click: () => actions.openExplorer() },
    { label: UNLOCK, enabled: locked, click: () => actions.unlock() },
    { type: 'separator' },
    { label: QUIT_COFFRET, click: () => actions.quit() },
  ];
}
