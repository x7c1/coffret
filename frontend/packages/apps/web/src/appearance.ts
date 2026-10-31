// Which of the explorer's two sets of colours the page is drawn in, and the
// painting of it, kept free of React so it is unit testable.
//
// What a person chooses is an `Appearance` — the system's, or light or dark
// over it — and what the page is drawn in is a `Scheme`, which the system's
// choice is read into. The colours themselves are custom properties on the
// page's root (see [`COLOR`](./theme)), so drawing the page in the other set
// is writing those properties again, and nothing re-renders for it.

import type { Appearance } from '@coffret/api';

import { colorProperty, DARK, LIGHT } from './theme';

/** Which set the page is drawn in. */
export type Scheme = 'light' | 'dark';

/** What the system's own choice is read from, and heard changing through. */
export const SYSTEM_DARK = '(prefers-color-scheme: dark)';

/**
 * The set the page is drawn in for `choice`: the system's where it is
 * `system` — dark where `systemDark` — and the one chosen otherwise, whatever
 * the system says.
 */
export function schemeOf(choice: Appearance, systemDark: boolean): Scheme {
  if (choice === 'system') {
    return systemDark ? 'dark' : 'light';
  }
  return choice;
}

/**
 * Draws the page under `root` in `scheme`: every colour's custom property set
 * to that set's value, and the browser told which it is, so that what it
 * draws for itself — scroll bars, a field's caret — matches. The scheme is
 * also on the root as `data-scheme`, which is what an end-to-end journey reads
 * to tell which set is showing.
 */
export function paintScheme(scheme: Scheme, root: HTMLElement): void {
  const set = scheme === 'dark' ? DARK : LIGHT;
  for (const name of Object.keys(LIGHT) as (keyof typeof LIGHT)[]) {
    root.style.setProperty(colorProperty(name), set[name]);
  }
  root.style.colorScheme = scheme;
  root.dataset.scheme = scheme;
}
