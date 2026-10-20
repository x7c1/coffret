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
// There is no IPC. No window has a preload and nothing here is exposed to a
// page: this process learns what happened from the addon's events alone.

import { join } from 'node:path';

import { app, BrowserWindow, dialog, Menu, session, shell } from 'electron';

import { loadAddon, type Addon } from './addon.js';
import { Desktop, message, type PassphraseWindow, type Shell } from './desktop.js';
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
import { IDLE_MINUTES, identifierFor, idleMinutes } from './settings.js';
import { explorerWindowOptions, PARTITIONS, passphraseWindowOptions } from './windows.js';

const STARTUP_FAILED = 'Coffret could not start';

function warn(said: string): void {
  console.warn(`coffret: ${said}`);
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
function start(identifier: string): void {
  let addon: Addon | null = null;
  try {
    const minutes = idleMinutes(process.env[IDLE_MINUTES]);
    addon = loadAddon();
    addon.start(identifier, minutes.toString());
    desktop = assemble(addon);
    addon.listen((event) => desktop?.handle(event));
    desktop.open();
  } catch (error) {
    desktop = null;
    failToStart(error, addon);
  }
}

/** The windows, the menu and the app's own events, around the addon. */
function assemble(addon: Addon): Desktop {
  for (const partition of PARTITIONS) {
    denyEveryPermission(session.fromPartition(partition));
  }

  const passphraseWindow = new BrowserWindow(passphraseWindowOptions());
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

  const shellActions: Shell = {
    openExplorer() {
      const address = addon.explorerAddress();
      if (address === null) {
        warn('a Library was opened, but the explorer is not served');
        return;
      }
      const origin = explorerOrigin(address);
      const window = new BrowserWindow(explorerWindowOptions());
      guardNavigation(window.webContents, 'explorer', () => origin, warn);
      window.webContents.setWindowOpenHandler(explorerWindowOpener((url) => shell.openExternal(url), warn));
      // Until the app stays resident with a tray of its own, the explorer's
      // window is what is left of it once a Library is open.
      window.on('closed', () => {
        explorerWindow = null;
        app.quit();
      });
      explorerWindow = window;
      launchExplorer(window, address);
      window.show();
    },
    focusExplorer() {
      if (explorerWindow === null) {
        return false;
      }
      bringForward(explorerWindow);
      return true;
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

  const started = new Desktop(addon, passphrase, shellActions);
  passphraseWindow.on('close', (event) => started.passphraseClosing(event));

  Menu.setApplicationMenu(
    Menu.buildFromTemplate(
      menuTemplate(process.platform, () => {
        const address = addon.explorerAddress();
        if (explorerWindow !== null && address !== null) {
          launchExplorer(explorerWindow, address);
        }
      }),
    ),
  );

  app.on('activate', () => started.bringForward());
  app.on('before-quit', () => started.quit());
  // Every browser session, ticket and door token goes as the app ends, since
  // the server and the host that issued them end with it.
  app.on('will-quit', () => retire(addon));
  return started;
}

function bringForward(window: BrowserWindow): void {
  if (window.isMinimized()) {
    window.restore();
  }
  window.show();
  window.focus();
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
  // is apart from the installed app's and the two run side by side.
  app.setPath('userData', join(app.getPath('appData'), identifier));

  // Taken first, so that a second launch ends here, before it starts anything,
  // and brings the running copy's window forward instead.
  if (!app.requestSingleInstanceLock()) {
    app.quit();
    return;
  }
  app.on('second-instance', () => desktop?.bringForward());
  // Every way the app ends is asked for explicitly — the Passphrase window
  // closed before a Library is open, the explorer's window closed, the server
  // stopping, or a quit — so the last window closing ends nothing by itself:
  // the hidden Passphrase window is not closed.
  app.on('window-all-closed', () => {});

  void app.whenReady().then(() => start(identifier));
}

main();
