// The options every window of the app is created with.
//
// The Passphrase page talks to the door it was served from and the explorer to
// its host, each over its own origin, and neither reaches Node. The explorer's
// window has the one preload of the app, which exposes a single function to
// its page — taking a drop by the paths of what was let go of — over the one
// channel the main process answers, and only for that window (`drops.ts`). The
// Passphrase window has none. Every window is sandboxed and isolated, the
// preload included, and keeps its storage in a partition of its own, so that
// nothing one page stores is seen by the other.

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

/** What every window shows of the app: its title, and its icon where it has one of its own. */
export interface Appearance {
  /** The window's title, kept whatever the page's own says. */
  title: string;
  /** The icon file's path (Linux), or `undefined` for the platform's default. */
  icon?: string;
}

/** The installed app's windows: titled Coffret, with no icon of their own. */
export const INSTALLED: Appearance = { title: 'Coffret' };

/** `appearance`, as window options: the icon left out where there is none. */
function shown(appearance: Appearance): BrowserWindowConstructorOptions {
  return {
    title: appearance.title,
    ...(appearance.icon === undefined ? {} : { icon: appearance.icon }),
  };
}

/** What every page runs under: a renderer that reaches nothing but the web. */
function confined(partition: string, preload?: string): WebPreferences {
  return {
    ...(preload === undefined ? {} : { preload }),
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
export function passphraseWindowOptions(appearance: Appearance = INSTALLED): BrowserWindowConstructorOptions {
  return {
    ...shown(appearance),
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

/**
 * The window the explorer runs in, served by the explorer host, with the
 * preload at `preload` that hands a drop over by path.
 */
export function explorerWindowOptions(
  preload: string,
  appearance: Appearance = INSTALLED,
): BrowserWindowConstructorOptions {
  return {
    ...shown(appearance),
    width: 1280,
    height: 820,
    minWidth: 640,
    minHeight: 420,
    show: false,
    webPreferences: confined(EXPLORER_PARTITION, preload),
  };
}
