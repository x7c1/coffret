// The files of one folder the reader can draw, kept free of DOM so it is unit
// testable.

import type { ListedFile } from '@coffret/api';

/** One file of a folder that the reader can display. */
export interface Page {
  path: string;
  name: string;
  /** Whether this device has the file, or a fetch has to place it first. */
  remote: boolean;
}

/**
 * The pages of one folder, in the listing's own order.
 *
 * Every stored file appears in the listing whatever its format; only the ones a
 * browser draws are pages. The reader's sheet stands on every row the focus
 * lands on, a page or not, but what it keeps ready around the focus is pages
 * alone (see [`sheetWanted`](./sheet)): there is nothing to fetch ahead for a
 * row it would have nothing to draw for.
 */
export function pagesOf(files: readonly ListedFile[]): Page[] {
  return files
    .filter((file) => file.openable)
    .map((file) => ({ path: file.path, name: file.name, remote: file.state === 'remote' }));
}

/** Where one file stands among the pages, or `null` where it is not one. */
export function pageAt(pages: readonly Page[], path: string): number | null {
  const at = pages.findIndex((page) => page.path === path);
  return at === -1 ? null : at;
}
