// Which icon a row of the tree draws in front of its name, kept free of DOM so
// it is unit testable.

import type { FileRow, FolderRow } from './tree';

/**
 * The icon in front of a row's name: a folder; a picture, for a file the
 * reader can show as a page (`openable` — what a browser draws); and a plain
 * sheet for any other file, which the reader can only say it cannot show.
 */
export type RowIcon = 'folder' | 'image' | 'file';

/** The icon `row` draws (see [`RowIcon`](#rowicon)). */
export function rowIconOf(row: FolderRow | FileRow): RowIcon {
  if (row.kind === 'folder') {
    return 'folder';
  }
  return row.file.openable ? 'image' : 'file';
}

/** The folder icon's outline, in a 24px viewBox: drawn by the tree's rows and by a folder named in a dialog. */
export const FOLDER_ICON_PATH = 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z';
