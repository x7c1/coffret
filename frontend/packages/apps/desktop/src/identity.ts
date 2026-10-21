// How the app looks, from its identifier: the installed app as it always has,
// and the development app told apart from it wherever the two would otherwise
// look the same — its windows' titles, its tray icon and tooltip, its window
// and Dock icons, and on Linux the app ID the desktop groups its windows by.

import { DEV_IDENTIFIER } from './settings.js';

/** What the app shows of itself, as files under the package's `assets/`. */
export interface Identity {
  /** Every window's title. */
  title: string;
  /** The tray icon's tooltip. */
  tooltip: string;
  /**
   * The tray icon: the black picture on macOS, which the menu bar draws as a
   * template image in its own colours, and the white one elsewhere, since
   * GNOME's top bar is dark in both its light and its dark style. The `@2x`
   * file beside each is picked up with it.
   */
  trayIcon: string;
  /** Whether the tray icon is a macOS template image. */
  trayTemplate: boolean;
  /** The windows' icon (Linux), where it is not the platform's default. */
  windowIcon: string | null;
  /** The Dock's icon (macOS), where it is not the app's own. */
  dockIcon: string | null;
  /**
   * The app ID its windows carry on Linux — the Wayland app ID and the X11
   * window class — where it is not the default, so that the desktop groups
   * and names them apart from the installed app's.
   */
  linuxAppId: string | null;
}

/** The badged icon the development app's windows and Dock tile carry. */
const DEV_ICON = 'icon-dev.png';

/** How the app with `identifier` looks on `platform`. */
export function identityFor(identifier: string, platform: NodeJS.Platform): Identity {
  const development = identifier === DEV_IDENTIFIER;
  const suffix = development ? '-dev' : '';
  const name = development ? 'Coffret (dev)' : 'Coffret';
  const mac = platform === 'darwin';
  return {
    title: name,
    tooltip: name,
    trayIcon: mac ? `tray${suffix}.png` : `tray${suffix}-light.png`,
    trayTemplate: mac,
    windowIcon: development && platform === 'linux' ? DEV_ICON : null,
    dockIcon: development && mac ? DEV_ICON : null,
    linuxAppId: development && platform === 'linux' ? identifier : null,
  };
}
