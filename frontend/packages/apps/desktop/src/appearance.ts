// The explorer's appearance, followed by the app's own windows.
//
// The explorer's colour scheme is chosen in its header and kept by the server
// as this device's state (spec: LA-38); the addon reads it back. The app draws
// the native chrome of its windows — the title bars, and on macOS the menus —
// in the same scheme by setting Electron's `nativeTheme.themeSource` from it:
// `system` follows the operating system, `light` and `dark` force that scheme.
// It is set as the explorer's window is filled before its page draws, and
// again each time the page says, over `APPEARANCE_CHANGED`, that the server
// took a new appearance: the app then reads the appearance afresh from the
// addon rather than taking the page's word for it, so the message carries
// nothing. Kept apart from Electron so that it is tested without it.

import { message } from './desktop.js';
import { explorerPageListener, type PageSaying } from './renewal.js';
import type { Appearance } from './windows.js';

/**
 * The channel the explorer's page says over that the server took a new
 * appearance. Sent, never invoked: it has no answer. `preload.cts` writes the
 * same name out, since a sandboxed preload imports nothing of the app's.
 */
export const APPEARANCE_CHANGED = 'coffret:appearance-changed';

/** What `nativeTheme.themeSource` is set to. */
export type ThemeSource = 'system' | 'light' | 'dark';

/** The part of Electron's `nativeTheme` the app's chrome is drawn by. */
export interface NativeTheme {
  themeSource: ThemeSource;
}

/** The theme source the windows' chrome is drawn in for `appearance`. */
export function themeSourceOf(appearance: Appearance): ThemeSource {
  switch (appearance) {
    case 'system':
      return 'system';
    case 'light':
      return 'light';
    case 'dark':
      return 'dark';
  }
}

/**
 * The explorer's appearance, read afresh by `read` (the addon's), and the
 * windows' chrome set to follow it on `theme`. A failed read is said and taken
 * as `system`, so that the chrome follows the operating system as it did
 * before anything was chosen: how the explorer looks is never a reason for it
 * not to open.
 *
 * Answers with the appearance read. It is set before anything reads
 * `nativeTheme.shouldUseDarkColors` for it, since a theme source forces that
 * too: under `system` it is the operating system's again, which is what the
 * explorer's ground for `system` is picked by (`groundOf`).
 */
export function followAppearance(
  read: () => Appearance,
  theme: NativeTheme,
  warn: (said: string) => void,
): Appearance {
  let appearance: Appearance = 'system';
  try {
    appearance = read();
  } catch (error) {
    warn(`the explorer's appearance could not be read; it follows the system: ${message(error)}`);
  }
  theme.themeSource = themeSourceOf(appearance);
  return appearance;
}

/**
 * The listener `APPEARANCE_CHANGED` is heard by: from the explorer's page
 * alone, as a session said to have ended is (`renewal.ts`). What is heard is
 * handed to `changed`, which reads the appearance afresh.
 */
export function appearanceChangedListener(
  explorer: () => unknown,
  origin: () => string | null,
  changed: () => void,
  warn: (said: string) => void,
): (event: PageSaying) => void {
  return explorerPageListener('an appearance said to have changed', explorer, origin, changed, warn);
}
