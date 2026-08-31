// What the reader turns through, kept free of DOM so it is unit testable.

import type { ListedFile } from '@coffret/api';

/** One file of the current folder that the reader can display. */
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
 * browser draws are pages. That is what makes `←` and `→` skip the rest rather
 * than stopping on a row the reader would have nothing to show for — the reader
 * moves through this subsequence, so a folder of photographs with a stray
 * `notes.txt` in it turns as though the note were not there.
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

/**
 * The page `step` away from the one at `at`, clamped at both ends.
 *
 * Clamped and not wrapped: the end of a folder is where a reader stops, and a
 * `→` that jumped back to the first page would look like the folder had been
 * reordered under them.
 */
export function stepped(pages: readonly Page[], at: number, step: number): number {
  return Math.max(0, Math.min(pages.length - 1, at + step));
}
