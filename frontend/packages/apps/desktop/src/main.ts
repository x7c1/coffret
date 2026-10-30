// Coffret's desktop app, on Electron.
//
// The server runs inside this process, through the Node-API addon
// (coffret-napi). The app has two windows. The Passphrase window is served
// by the addon's door, and is where a Library is chosen and its Passphrase
// typed: the page submits to the door it came from, so the Passphrase never
// passes through this process's JavaScript. The explorer's window is served
// by the explorer host the addon runs in front of the server, and is admitted
// by a one-time launch ticket in the fragment of the URL it is loaded from.
//
// The app stays resident once a Library is open: closing the explorer's window
// hides it, and the tray icon, a second launch or the Dock brings it back. It
// ends by the tray's *Quit Coffret*, the menu's *Quit*, or the session ending —
// or by closing the explorer's window on a Linux desktop with no tray host,
// where nothing else would be left to reach it by (`tray-host.ts`).
//
// This process learns what happened from the addon's events. The one thing a
// page may ask of it is to take a drop onto the explorer's window by path: the
// explorer's preload asks over one channel, which is answered for that window
// alone and hands the paths to the addon (`drops.ts`). Nothing else is exposed
// to any page, and the Passphrase window has no preload.

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { app, BrowserWindow, dialog, ipcMain, Menu, nativeImage, powerMonitor, session, shell, Tray } from 'electron';

import { loadAddon, type Addon } from './addon.js';
import { Desktop, explorerCloses, message, type PassphraseWindow, type Shell } from './desktop.js';
import { TAKE_DROP, takeDropHandler } from './drops.js';
import { identityFor, type Identity } from './identity.js';
import { loadAfresh } from './load.js';
import { menuTemplate } from './menu.js';
import {
  denyEveryWindow,
  explorerLaunchUrl,
  explorerOrigin,
  explorerWindowOpener,
  guardNavigation,
} from './navigation.js';
import { denyEveryPermission } from './permissions.js';
import { resourcesFor } from './resources.js';
import { IDLE_MINUTES, identifierFor, idleMinutes } from './settings.js';
import { trayMenuTemplate } from './tray.js';
import { trayHostLine, trayHostOn } from './tray-host.js';
import { type Appearance, explorerWindowOptions, PARTITIONS, passphraseWindowOptions } from './windows.js';

const STARTUP_FAILED = 'Coffret could not start';

/** Where this file was built, `dist/` of the package. */
const BUILT = dirname(fileURLToPath(import.meta.url));

/** The explorer window's preload, built beside this file. */
const EXPLORER_PRELOAD = join(BUILT, 'preload.cjs');

/** The addon and the icons, from the package or from the checkout. */
const RESOURCES = resourcesFor(app.isPackaged, process.resourcesPath, BUILT);

/** The app's icons. */
const ASSETS = RESOURCES.assets;

/**
 * How often the tray's menu is brought in step with whether the Library is
 * locked, which it does by itself on the server's own clock and is not told
 * of: often enough that *Unlock…* is offered soon after the idle lock.
 */
const TRAY_REFRESH_MS = 5000;

function warn(said: string): void {
  console.warn(`coffret: ${said}`);
}

function inform(said: string): void {
  console.info(`coffret: ${said}`);
}

/**
 * Says why the app could not start, in a message box since an installed app
 * has no terminal to say it on, and exits 1 once it is dismissed.
 */
function failToStart(error: unknown, addon: Addon | null): void {
  const sentence = message(error);
  console.error(sentence);
  if (addon !== null) {
    retire(addon);
  }
  void app.whenReady().then(() => {
    dialog.showErrorBox(STARTUP_FAILED, sentence);
    app.exit(1);
  });
}

function retire(addon: Addon): void {
  try {
    addon.retire();
  } catch (error) {
    warn(`the credentials could not be retired: ${message(error)}`);
  }
}

/** Where the app is between the windows, once it has started. */
let desktop: Desktop | null = null;

/**
 * Starts the addon, then opens the Passphrase window on the door.
 *
 * Every failure here is before anything is served, and ends the app through
 * {@link failToStart}.
 */
function start(identifier: string, identity: Identity, tray: boolean): void {
  let addon: Addon | null = null;
  try {
    const minutes = idleMinutes(process.env[IDLE_MINUTES]);
    addon = loadAddon(RESOURCES);
    addon.start(identifier, minutes.toString());
    const assembled = assemble(addon, identity, tray);
    desktop = assembled.desktop;
    addon.listen((event) => {
      desktop?.handle(event);
      assembled.refreshTray();
    });
    desktop.open();
  } catch (error) {
    desktop = null;
    failToStart(error, addon);
  }
}

/** The windows, the menus, the tray and the app's own events, around the addon. */
function assemble(
  addon: Addon,
  identity: Identity,
  trayWanted: boolean,
): { desktop: Desktop; refreshTray: () => void } {
  for (const partition of PARTITIONS) {
    denyEveryPermission(session.fromPartition(partition));
  }

  const appearance: Appearance = {
    title: identity.title,
    ...(identity.windowIcon === null ? {} : { icon: join(ASSETS, identity.windowIcon) }),
  };
  const passphraseWindow = new BrowserWindow(passphraseWindowOptions(process.platform, appearance));
  keepTitle(passphraseWindow);
  // The door's origin, fixed for the run, once its first URL is known.
  let doorOrigin: string | null = null;
  guardNavigation(passphraseWindow.webContents, 'Passphrase', () => doorOrigin, warn);
  passphraseWindow.webContents.setWindowOpenHandler(denyEveryWindow);
  const passphrase: PassphraseWindow = {
    load(url) {
      doorOrigin = new URL(url).origin;
      // Said without the error, which quotes the URL, and the URL carries the
      // window's door token.
      loadAfresh(passphraseWindow.webContents, url).catch(() => {
        warn('the Passphrase window could not load the door');
      });
    },
    bringForward() {
      bringForward(passphraseWindow);
    },
    hide() {
      passphraseWindow.hide();
    },
    isVisible() {
      return passphraseWindow.isVisible();
    },
  };

  let explorerWindow: BrowserWindow | null = null;
  // The origin the explorer is served from, fixed for the run once its window
  // is opened.
  let explorerServed: string | null = null;

  ipcMain.handle(
    TAKE_DROP,
    takeDropHandler(
      () => explorerWindow?.webContents ?? null,
      () => explorerServed,
      (paths) => addon.takeDrop(paths),
      warn,
    ),
  );

  /** Loads the explorer into its window from a fresh launch ticket. */
  const launchExplorer = (window: BrowserWindow, address: string) => {
    let ticket: string;
    try {
      ticket = addon.issueTicket();
    } catch (error) {
      warn(`no launch ticket could be issued for the explorer: ${message(error)}`);
      return;
    }
    // Said without the error, which quotes the URL and so the ticket.
    loadAfresh(window.webContents, explorerLaunchUrl(address, ticket)).catch(() => {
      warn('the explorer could not be loaded');
    });
  };

  // Set once the dispatcher is made, which the windows' handlers reach.
  let started: Desktop | null = null;

  const shellActions: Shell = {
    openExplorer() {
      const address = addon.explorerAddress();
      if (address === null) {
        warn('the explorer was asked for, but it is not served');
        return;
      }
      const origin = explorerOrigin(address);
      explorerServed = origin;
      const window = new BrowserWindow(explorerWindowOptions(EXPLORER_PRELOAD, process.platform, appearance));
      keepTitle(window);
      guardNavigation(window.webContents, 'explorer', () => origin, warn);
      window.webContents.setWindowOpenHandler(explorerWindowOpener((url) => shell.openExternal(url), warn));
      window.on('close', (event) => started?.explorerClosing(event));
      // Windows ends the session by closing every window, which is the app
      // ending rather than the explorer's window being put away.
      window.on('session-end', () => app.quit());
      window.on('closed', () => {
        if (explorerWindow === window) {
          explorerWindow = null;
        }
      });
      explorerWindow = window;
      launchExplorer(window, address);
      window.show();
    },
    focusExplorer() {
      if (explorerWindow === null || explorerWindow.isDestroyed()) {
        return false;
      }
      bringForward(explorerWindow);
      return true;
    },
    hideExplorer() {
      explorerWindow?.hide();
    },
    reportServerStopped() {
      dialog.showErrorBox(
        "The Library's server stopped",
        "The server serving the Library stopped, so Coffret quits. Start Coffret again to open the Library.",
      );
    },
    quit() {
      app.quit();
    },
    warn,
  };

  const tray = trayWanted ? showTray(identity) : null;
  if (trayWanted && tray === null) {
    // Start-up logged that a tray would be shown: say what closing the
    // explorer's window comes to without one.
    const closing = explorerCloses(process.platform, false, false) === 'quit' ? 'quits' : 'hides it';
    warn(`there is no tray icon after all; closing the explorer's window ${closing}`);
  }
  const dispatcher = new Desktop(addon, passphrase, shellActions, {
    platform: process.platform,
    tray: tray !== null,
  });
  started = dispatcher;
  passphraseWindow.on('close', (event) => dispatcher.passphraseClosing(event));

  Menu.setApplicationMenu(
    Menu.buildFromTemplate(
      menuTemplate(process.platform, () => {
        const address = addon.explorerAddress();
        if (explorerWindow !== null && !explorerWindow.isDestroyed() && address !== null) {
          launchExplorer(explorerWindow, address);
        }
      }),
    ),
  );

  // The tray's menu, rebuilt where whether the Library is locked has changed
  // since it was last built, so that *Unlock…* is offered only while there is
  // something to unlock.
  let trayLocked: boolean | null = null;
  const refreshTray = () => {
    if (tray === null || tray.isDestroyed()) {
      return;
    }
    let locked: boolean;
    try {
      locked = dispatcher.locked();
    } catch (error) {
      warn(`whether the Library is locked could not be read for the tray: ${message(error)}`);
      return;
    }
    if (locked === trayLocked) {
      return;
    }
    trayLocked = locked;
    tray.setContextMenu(
      Menu.buildFromTemplate(
        trayMenuTemplate(locked, {
          openExplorer: () => dispatcher.bringForward(),
          unlock: () => dispatcher.askForPassphrase(),
          quit: () => app.quit(),
        }),
      ),
    );
  };
  refreshTray();
  const refreshing = tray === null ? null : setInterval(refreshTray, TRAY_REFRESH_MS);
  if (tray !== null && process.platform !== 'darwin') {
    // macOS opens the menu on a click; elsewhere a click, where the platform
    // delivers one, brings the explorer back.
    tray.on('click', () => dispatcher.bringForward());
  }

  app.on('activate', () => dispatcher.bringForward());
  app.on('before-quit', () => {
    dispatcher.quit();
    if (refreshing !== null) {
      clearInterval(refreshing);
    }
  });
  // The session ending on Linux and macOS: the app quits, rather than holding
  // the shutdown up behind its hidden windows.
  powerMonitor.on('shutdown', () => app.quit());
  // Every browser session, ticket and door token goes as the app ends, since
  // the server and the host that issued them end with it.
  app.on('will-quit', () => {
    tray?.destroy();
    retire(addon);
  });
  return { desktop: dispatcher, refreshTray };
}

/**
 * Puts the app's icon in the tray, or says why it could not and leaves the app
 * without one.
 */
function showTray(identity: Identity): Tray | null {
  try {
    const image = nativeImage.createFromBuffer(readFileSync(join(ASSETS, identity.trayIcon)), {
      scaleFactor: identity.trayScale,
    });
    if (image.isEmpty()) {
      warn('the tray icon could not be read');
      return null;
    }
    image.setTemplateImage(identity.trayTemplate);
    const tray = new Tray(image);
    tray.setToolTip(identity.tooltip);
    return tray;
  } catch (error) {
    warn(`the tray icon could not be shown: ${message(error)}`);
    return null;
  }
}

/** Keeps `window`'s title the app's own, whatever its page is titled. */
function keepTitle(window: BrowserWindow): void {
  window.on('page-title-updated', (event) => event.preventDefault());
}

function bringForward(window: BrowserWindow): void {
  if (window.isMinimized()) {
    window.restore();
  }
  window.show();
  window.focus();
  // A window focused while another app is active stays behind it on macOS
  // until the app itself is: what brings a window forward here is somebody
  // asking for it — a second launch, the Dock, the tray, the explorer's own
  // unlock — so the app is made the active one.
  if (process.platform === 'darwin') {
    app.focus({ steal: true });
  }
}

function main(): void {
  let identifier: string;
  try {
    identifier = identifierFor(app.isPackaged, process.env);
  } catch (error) {
    failToStart(error, null);
    return;
  }
  // The app's own data — the explorer's partition, and the lock that keeps one
  // copy of it running — under its identifier, so that the development app's
  // is apart from the installed app's and the two run side by side. A
  // `--user-data-dir` given on the command line wins, so that a bundle can be
  // tried out beside the installed copy of it, which has the same identifier.
  if (!app.commandLine.hasSwitch('user-data-dir')) {
    app.setPath('userData', join(app.getPath('appData'), identifier));
  }
  const identity = identityFor(identifier, process.platform);
  if (identity.linuxAppId !== null) {
    // The Wayland app ID and the X11 window class, so that the desktop groups
    // and names the development app's windows apart from the installed app's,
    // and finds the installed app's desktop entry by its StartupWMClass.
    app.setDesktopName(identity.linuxAppId);
  }

  // Taken first, so that a second launch ends here, before it starts anything,
  // and brings the running copy's window forward instead.
  if (!app.requestSingleInstanceLock()) {
    app.quit();
    return;
  }
  app.on('second-instance', () => desktop?.bringForward());
  // Every way the app ends is asked for explicitly — the Passphrase window
  // closed before a Library is open, the explorer's window closed where there
  // is no tray, the server stopping, a quit, or the session ending — so the
  // last window closing ends nothing by itself: the hidden windows are not
  // closed.
  app.on('window-all-closed', () => {});
  // A plain `kill`, as a session manager may send, ends the app as a quit
  // does, retiring the credentials on the way out.
  process.on('SIGTERM', () => app.quit());

  void app
    .whenReady()
    .then(async () => {
      if (identity.dockIcon !== null) {
        app.dock?.setIcon(join(ASSETS, identity.dockIcon));
      }
      const host = await trayHostOn(process.platform);
      inform(trayHostLine(host));
      start(identifier, identity, host.present);
    })
    .catch((error: unknown) => failToStart(error, null));
}

main();
