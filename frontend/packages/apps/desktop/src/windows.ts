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
export interface WindowLook {
  /** The window's title, kept whatever the page's own says. */
  title: string;
  /** The icon file's path (Linux), or `undefined` for the platform's default. */
  icon?: string;
}

/** Which set of colours the explorer is drawn in, as the addon reads what the server kept. */
export type Appearance = 'system' | 'light' | 'dark';

/**
 * The explorer's ground in each of its two sets: `LIGHT.ground` and
 * `DARK.ground` of the explorer's own colours (`apps/web/src/theme.ts`).
 */
export const GROUND = { light: '#f7f8fa', dark: '#15171c' } as const;

/**
 * What the explorer's window is filled with before its page has drawn
 * anything: the ground of the set the page will be drawn in — the one this
 * device chose, or for `system` the one the system asks for, which is
 * `systemDark` (`nativeTheme.shouldUseDarkColors`). Without it the window
 * opens white, and a dark explorer flashes white before it is drawn.
 */
export function groundOf(appearance: Appearance, systemDark: boolean): string {
  const scheme = appearance === 'system' ? (systemDark ? 'dark' : 'light') : appearance;
  return GROUND[scheme];
}

/** The installed app's windows: titled Coffret, with no icon of their own. */
export const INSTALLED: WindowLook = { title: 'Coffret' };

/** `look`, as window options: the icon left out where there is none. */
function shown(look: WindowLook): BrowserWindowConstructorOptions {
  return {
    title: look.title,
    ...(look.icon === undefined ? {} : { icon: look.icon }),
  };
}

/**
 * The menu bar, hidden on every platform but macOS: there the bar is the
 * system's, not the window's, and its Edit menu is what carries Cmd+C, Cmd+V
 * and Cmd+Z to the page's inputs. Elsewhere the window would draw a bar of
 * File, Edit and Explorer the app hardly needs; hidden, it comes back on Alt,
 * and the menu it belongs to stays the application's, so its accelerators —
 * Ctrl+R to reload the explorer, Ctrl+Q to quit (see `menu.ts`) — keep working.
 */
function menuBar(platform: NodeJS.Platform): BrowserWindowConstructorOptions {
  return platform === 'darwin' ? {} : { autoHideMenuBar: true };
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

/** The small window the Passphrase is typed into on `platform`, served by the door. */
export function passphraseWindowOptions(
  platform: NodeJS.Platform,
  look: WindowLook = INSTALLED,
): BrowserWindowConstructorOptions {
  return {
    ...shown(look),
    ...menuBar(platform),
    width: 380,
    height: 260,
    resizable: false,
    maximizable: false,
    fullscreenable: false,
    show: false,
    webPreferences: confined(PASSPHRASE_PARTITION),
  };
}

/**
 * The window the explorer runs in on `platform`, served by the explorer host,
 * with the preload at `preload` that hands a drop over by path, filled with
 * `ground` until the page draws (see [`groundOf`](#groundOf)).
 */
export function explorerWindowOptions(
  preload: string,
  platform: NodeJS.Platform,
  look: WindowLook = INSTALLED,
  ground: string = GROUND.light,
): BrowserWindowConstructorOptions {
  return {
    ...shown(look),
    ...menuBar(platform),
    backgroundColor: ground,
    width: 1280,
    height: 820,
    minWidth: 640,
    minHeight: 420,
    show: false,
    webPreferences: confined(EXPLORER_PARTITION, preload),
  };
}
