// The options every window of the app is created with.
//
// No window has a preload, and the main process exposes nothing to any page:
// the Passphrase page talks to the door it was served from and the explorer to
// its host, each over its own origin, and neither reaches Node or the main
// process. Every window is sandboxed and isolated, and keeps its storage in a
// partition of its own, so that nothing one page stores is seen by the other.

import type { BrowserWindowConstructorOptions, WebPreferences } from 'electron';

/**
 * The Passphrase window's partition: in memory alone, since the page keeps
 * nothing between one opening and the next.
 */
export const PASSPHRASE_PARTITION = 'coffret-passphrase';

/**
 * The explorer's partition, kept on disk under the app's own data directory,
 * as a browser profile keeps a site's storage.
 */
export const EXPLORER_PARTITION = 'persist:coffret-explorer';

/** Every partition a window of the app is in. */
export const PARTITIONS = [PASSPHRASE_PARTITION, EXPLORER_PARTITION] as const;

const TITLE = 'Coffret';

/** What every page runs under: a renderer that reaches nothing but the web. */
function confined(partition: string): WebPreferences {
  return {
    contextIsolation: true,
    sandbox: true,
    nodeIntegration: false,
    nodeIntegrationInWorker: false,
    nodeIntegrationInSubFrames: false,
    webSecurity: true,
    allowRunningInsecureContent: false,
    webviewTag: false,
    navigateOnDragDrop: false,
    spellcheck: false,
    partition,
  };
}

/** The small window the Passphrase is typed into, served by the door. */
export function passphraseWindowOptions(): BrowserWindowConstructorOptions {
  return {
    title: TITLE,
    width: 380,
    height: 260,
    resizable: false,
    maximizable: false,
    fullscreenable: false,
    autoHideMenuBar: true,
    show: false,
    webPreferences: confined(PASSPHRASE_PARTITION),
  };
}

/** The window the explorer runs in, served by the explorer host. */
export function explorerWindowOptions(): BrowserWindowConstructorOptions {
  return {
    title: TITLE,
    width: 1280,
    height: 820,
    minWidth: 640,
    minHeight: 420,
    show: false,
    webPreferences: confined(EXPLORER_PARTITION),
  };
}
