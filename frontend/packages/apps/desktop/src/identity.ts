// How the app looks, from its identifier: the installed app as it always has,
// and the development app told apart from it wherever the two would otherwise
// look the same — its windows' titles, its tray icon and tooltip, its window
// and Dock icons, and on Linux the app ID the desktop groups its windows by,
// which is each app's own identifier.

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
   * GNOME's top bar is dark in both its light and its dark style.
   */
  trayIcon: string;
  /**
   * The scale the tray icon's picture is drawn at. On macOS it is 2: the
   * 32-pixel picture is a 16-point icon at twice the resolution, the size the
   * menu bar draws its icons at, where read at 1 it would stand 32 points tall.
   * Elsewhere 1, and the panel fits the picture to its own size.
   */
  trayScale: number;
  /** Whether the tray icon is a macOS template image. */
  trayTemplate: boolean;
  /** The windows' icon (Linux), where it is not the platform's default. */
  windowIcon: string | null;
  /**
   * The Dock's icon (macOS), where it is not the app's own: a picture inset on
   * a transparent canvas, as the .icns is, since the Dock and the app switcher
   * draw it across the whole tile and a full-bleed one stands larger than its
   * neighbours.
   */
  dockIcon: string | null;
  /**
   * The app ID its windows carry on Linux — the Wayland app ID and the X11
   * window class: the identifier, so that the desktop groups and names the
   * development app's windows apart from the installed app's, and matches the
   * installed app's with the desktop entry its .deb installs, whose
   * `StartupWMClass` is the installed identifier (`electron-builder.config.js`).
   */
  linuxAppId: string | null;
}

/** The badged icon the development app's windows carry. */
const DEV_ICON = 'icon-dev.png';

/** The same badged icon, inset for the development app's Dock tile. */
const DEV_DOCK_ICON = 'dock-dev.png';

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
    trayScale: mac ? 2 : 1,
    trayTemplate: mac,
    windowIcon: development && platform === 'linux' ? DEV_ICON : null,
    dockIcon: development && mac ? DEV_DOCK_ICON : null,
    linuxAppId: platform === 'linux' ? identifier : null,
  };
}
