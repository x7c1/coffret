// Where the focus stands, as the header's location says it.
//
// The focused row's Entry Path, one name per level, with the separators drawn
// as ` / ` so that a long path reads as a trail of names rather than as one
// string to be parsed: `Comics / Series A / Vol. 03`. The containing folders
// and the row's own name together, since the name alone says nothing about
// which `Vol. 03` of several the focus is on.
//
// Kept free of DOM and of React so it is unit testable, as `destination.ts`
// beside it is.

import { keyed } from './tree';

/**
 * What the location says with no row focused: the top of the Library, which has
 * no row. The same word the screen's sentences use for it — `the Library`, as
 * [`placeOf`](./destination) says it and the bar over the tree's "New folder…"
 * is titled — standing alone here, without the article. The details pane and
 * the reader name the root of a containing path by it too.
 */
export const LIBRARY_ROOT = 'Library';

/**
 * The location of the row `focus` names — a folder's or a file's — as the
 * header draws it, or [`LIBRARY_ROOT`](#LIBRARY_ROOT) where nothing is focused.
 */
export function locationOf(focus: string | null): string {
  if (focus === null) {
    return LIBRARY_ROOT;
  }
  const { path } = keyed(focus);
  return path === '' ? LIBRARY_ROOT : path.split('/').join(' / ');
}
