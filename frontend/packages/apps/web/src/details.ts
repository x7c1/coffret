// What the details pane says about the rows selected in the tree. Kept free of
// DOM and of React so it is unit testable.
//
// The words are chosen to fit any kind of file and not only the pages of a
// book: a Library holds whatever somebody put in it, so the pane speaks the way
// a desktop file manager's inspector does — a kind, a size, a time — rather
// than in pages and volumes.

import type { Freeze, ListedFile, ListedFolder, Listing, Sync } from '@coffret/api';

import type { RowFill } from './fill';
import type { FileRow, FolderRow } from './tree';

/** A row the pane can describe: a folder or a file, never a note. */
export type Described = FolderRow | FileRow;

/**
 * The kinds of file the pane names, by the extension a name ends in, in the
 * words the macOS Finder uses for them.
 *
 * The Finder's words because they are the ones a person has already read about
 * their own files, on the desk the files came from. A table of the common
 * ones, and not a registry: anything else is a `Document`, which is true of
 * every file.
 */
const KINDS: Readonly<Record<string, string>> = {
  avif: 'AVIF image',
  bmp: 'Windows BMP image',
  gif: 'GIF image',
  heic: 'HEIF image',
  jpeg: 'JPEG image',
  jpg: 'JPEG image',
  png: 'PNG image',
  svg: 'SVG image',
  tif: 'TIFF image',
  tiff: 'TIFF image',
  webp: 'WebP image',
  pdf: 'PDF document',
  epub: 'EPUB book',
  txt: 'Plain text',
  md: 'Markdown document',
  csv: 'Comma-separated values',
  json: 'JSON document',
  html: 'HTML text',
  mov: 'QuickTime movie',
  mp4: 'MPEG-4 movie',
  m4v: 'MPEG-4 movie',
  mp3: 'MP3 audio',
  m4a: 'MPEG-4 audio',
  wav: 'Waveform audio',
  zip: 'ZIP archive',
};

/** What a file nothing in [`KINDS`] names is called. */
export const DOCUMENT = 'Document';

/** What a folder is called. */
export const FOLDER = 'Folder';

/**
 * What a name ends in after its last `.`, lower-cased, or `null` where it has
 * none.
 *
 * The server's reading of an extension (its `classify`): a name that is all
 * extension — `.gitignore` — has none, since the dot there begins the name, and
 * `.JPG` and `.jpg` are one person's two ways of naming one kind of file.
 */
export function extensionOf(name: string): string | null {
  const dot = name.lastIndexOf('.');
  if (dot <= 0 || dot === name.length - 1) {
    return null;
  }
  return name.slice(dot + 1).toLowerCase();
}

/** A file's kind, the way the Finder words it, and `Document` where unknown. */
export function kindOf(name: string): string {
  const extension = extensionOf(name);
  return (extension !== null && KINDS[extension]) || DOCUMENT;
}

/**
 * The first image directly in a folder — for the folder of a scanned book, the
 * cover — or `null` where it holds none.
 *
 * The first in the listing's order, the byte order of the paths, which is the
 * order the folder's rows stand in, so the picture the pane shows is the cover
 * the reader's sheet shows for the folder (see [`sheetShows`](./sheet)). An
 * image is a file the reader can show (`openable`).
 */
export function firstImage(listing: Listing): ListedFile | null {
  return listing.files.find((file) => file.openable) ?? null;
}

/**
 * Whether the pane offers Open for one row: an image, or a folder holding
 * images directly — the two things `Enter` and a double-click open the
 * reader's sheet on (see [`opened`](./TreeTable)). Elsewhere opening says why
 * nothing opened instead; the sheet reaches such a row only by following the
 * focus once it is open.
 *
 * A folder's answer needs its own listing, `null` while it is not on hand, and
 * a folder not known to hold an image offers nothing yet rather than a button
 * that might open nothing.
 */
export function opensInReader(row: Described, listing: Listing | null): boolean {
  if (row.kind === 'file') {
    return row.file.openable;
  }
  return listing !== null && firstImage(listing) !== null;
}

/** How many items stand directly in a folder: its folders and its files. */
export function itemsIn(listing: Listing): number {
  return listing.folders.length + listing.files.length;
}

/**
 * Whether one file is on this device, in the pane's words.
 *
 * Read off what the row shows (see [`rowFill`](./fill)), with the one thing
 * the row has no state for: whether the run that would carry a file of this
 * device's into the Library stopped. A file this device has and the Library
 * does not is `uploading` until then — the next sync or freeze takes it in —
 * and `upload failed` once the run that was to has stopped. A fetch that
 * failed or was declined left nothing here, so it is `No`; why is the row's
 * own sentence.
 */
export function fileOnDevice(shown: RowFill, uploadStopped: boolean): string {
  switch (shown.state) {
    case 'present':
      return 'Yes';
    case 'fetching':
      return 'Fetching…';
    case 'added':
    case 'reserved':
      return uploadStopped ? 'Yes · upload failed' : 'Yes · uploading';
    case 'remote':
    case 'failed':
    case 'declined':
      return 'No';
  }
}

/**
 * Whether the run that would carry one file into the Library has stopped: the
 * freeze of the folder it was dropped into for a file waiting to be packed,
 * and the sync for any other file the Library does not hold yet.
 *
 * The run on record and not any before it: a sync or freeze started since is
 * the one that carries the file now, and it has not failed.
 */
export function uploadStopped(file: ListedFile, sync: Sync | null, freeze: Freeze | null): boolean {
  if (file.reserved) {
    return (
      freeze !== null &&
      freeze.status === 'stopped' &&
      (freeze.folder === '' || file.path.startsWith(`${freeze.folder}/`))
    );
  }
  return file.state === 'added' && sync !== null && sync.status === 'stopped';
}

/**
 * Whether a folder is on this device, counting every file at every depth below
 * it: `All files`, `18 of 196 files`, or `No`.
 *
 * The server's counts (`files_present`, `files_total`), because the page holds
 * the listings it asked for and not every descendant.
 */
export function folderOnDevice(folder: ListedFolder): string {
  const { files_present: present, files_total: total } = folder;
  if (total > 0 && present >= total) {
    return 'All files';
  }
  if (present === 0) {
    return 'No';
  }
  return `${present} of ${total} files`;
}

/**
 * What a prepared folder says in place of whether it is on this device: made
 * with "New folder…" and empty, so the Library has no folder there yet
 * (spec: EP-2), and the page alone remembers it.
 */
export const NOT_IN_LIBRARY_YET = 'Not in the Library until a file is added';

/**
 * What the folder of a book whose packing has not committed says in its place:
 * its pages are on this device, and the Library holds none of them yet.
 */
export const NOT_PACKED_YET = 'Not in the Library until it is packed';

/** What the pane says for several rows selected together. */
export interface SelectionSummary {
  /** How many rows are selected. */
  count: number;
  folders: number;
  files: number;
  /**
   * The bytes the selection comes to: each file's size, and each folder's —
   * everything at every depth below it, as the server counts it.
   *
   * A row standing below a selected folder is in that folder's size already,
   * so it is not added a second time.
   */
  bytes: number;
}

/** The totals of a selection of folders and files. */
export function summaryOf(rows: readonly Described[]): SelectionSummary {
  const folderPaths = rows.filter((row) => row.kind === 'folder').map((row) => row.path);
  const inSelectedFolder = (path: string) => folderPaths.some((folder) => path.startsWith(`${folder}/`));
  let folders = 0;
  let files = 0;
  let bytes = 0;
  for (const row of rows) {
    if (row.kind === 'folder') {
      folders += 1;
    } else {
      files += 1;
    }
    if (!inSelectedFolder(row.path)) {
      bytes += row.kind === 'folder' ? row.folder.size : row.file.size;
    }
  }
  return { count: rows.length, folders, files, bytes };
}

/** `3 items selected`. */
export function selectedHeading(count: number): string {
  return `${counted(count, 'item', 'items')} selected`;
}

/** `2 folders`, `1 file`: a count and the noun that goes with it. */
export function counted(count: number, one: string, many: string): string {
  return `${count} ${count === 1 ? one : many}`;
}
