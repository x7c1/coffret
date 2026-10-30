// What the reader's sheet stands on and shows: the row of the tree the focus
// is on, whatever it is. Kept free of DOM and of React so it is unit testable.
//
// The sheet follows the tree's focus rather than turning pages of its own, so
// everything here is read off the tree's rows and the listings the tree holds:
// walking the page rows of an open book with `↑`/`↓` is paging, and reading
// off its end steps into the first file of the next folder (see
// [`readOn`](./tree)).

import type { ListedFile, Listing } from '@coffret/api';

import { counted, firstImage, FOLDER, itemsIn } from './details';
import type { ViewState } from './hash';
import { size } from './humanize';
import { pageAt, pagesOf } from './pages';
import { prefetchTargets } from './prefetch';
import { parentOf } from './prospective';
import { fileKey, folderKey, type FileRow, type FolderRow, type TreeRow } from './tree';
import type { Asked } from './useAsked';

/** A row the sheet can stand on: a folder or a file, never a note. */
export type Sheeted = FolderRow | FileRow;

/** What the sheet says over a folder holding no image to draw as its cover. */
export const NO_IMAGES = 'No images';

/** What the sheet says over a file a browser draws nothing from. */
export const UNSUPPORTED = 'preview of this format is not supported yet';

/**
 * The key of the row the URL has the sheet open on (see [`folderKey`](./tree)),
 * or `null` where the sheet is closed.
 */
export function sheetKey(view: ViewState): string | null {
  if (view.open !== null) {
    return fileKey(view.open);
  }
  return view.cover === undefined ? null : folderKey(view.cover);
}

/**
 * The URL's place with the sheet open on `row`: the row's own path in the
 * field for its kind, and the folder the screen's actions are about kept as
 * `folder` says it.
 */
export function sheetOn(folder: string, row: { kind: 'folder' | 'file'; path: string }): ViewState {
  return row.kind === 'file' ? { folder, open: row.path } : { folder, open: null, cover: row.path };
}

/** Where one row stands among the rows beside it: `at` of `of`, from 1. */
export interface Counter {
  at: number;
  of: number;
}

/**
 * Where the row keyed `key` stands among its siblings — every folder and file
 * drawn in the folder it stands in, in the tree's order — or `null` where no
 * such row is on show.
 *
 * Folders count, and so do files the reader cannot draw: the counter is a
 * position among the rows the arrow keys land on, so it moves on every one of
 * them and its total is the folder's Items in the details pane (see
 * [`itemsIn`](./details)).
 */
export function counterOf(rows: readonly TreeRow[], key: string): Counter | null {
  const walkable = rows.filter((row): row is Sheeted => row.kind !== 'note');
  const row = walkable.find((one) => one.key === key);
  if (row === undefined) {
    return null;
  }
  const parent = parentOf(row.path);
  const siblings = walkable.filter((one) => parentOf(one.path) === parent);
  return { at: siblings.indexOf(row) + 1, of: siblings.length };
}

/** What the sheet shows for the row it stands on. */
export type SheetShows =
  /** A file a browser draws: the picture itself, filling the sheet. */
  | { kind: 'image'; file: ListedFile }
  /**
   * A folder: its first image as a cover on a small stack of sheets, smaller
   * than a page is drawn, and what it holds under it — so that a folder and the
   * first page in it never look alike.
   */
  | {
      kind: 'folder';
      /** The first image directly in it, `null` where none is or its listing is not on hand. */
      cover: ListedFile | null;
      /** `Folder · 12 items · 3.4 MB`: as much of it as is known. */
      caption: string;
      /** What stands on the stack in place of a cover: nothing yet, or {@link NO_IMAGES}. */
      note: string | null;
      /** The refusal's sentence where its listing was refused. */
      refused: string | null;
    }
  /** A file a browser draws nothing from: its name and {@link UNSUPPORTED}. */
  | { kind: 'unsupported'; name: string };

/**
 * What the sheet shows for `row`, a folder's out of `asked`, its own listing as
 * far as it has been asked.
 *
 * A folder's count of items is its listing's, and is left out of the caption
 * until that answer lands; its size is the server's count of everything below
 * it, and is left out for a folder the Library does not hold yet, which has
 * none to measure — as the details pane leaves it out.
 */
export function sheetShows(row: Sheeted, asked: Asked<Listing> | undefined): SheetShows {
  if (row.kind === 'file') {
    return row.file.openable ? { kind: 'image', file: row.file } : { kind: 'unsupported', name: row.name };
  }
  const listing = asked?.status === 'ready' ? asked.value : null;
  const parts = [FOLDER];
  if (listing !== null) {
    parts.push(counted(itemsIn(listing), 'item', 'items'));
  }
  if (!row.prospective && !row.stranded) {
    parts.push(size(row.folder.size));
  }
  const cover = listing === null ? null : firstImage(listing);
  return {
    kind: 'folder',
    cover,
    caption: parts.join(' · '),
    note: listing !== null && cover === null ? NO_IMAGES : null,
    refused: asked?.status === 'failed' ? asked.message : null,
  };
}

/** What the sheet draws, and what it keeps ready around it. */
export interface Wanted {
  /** The file whose bytes the sheet draws, `null` where it draws none. */
  shown: ListedFile | null;
  /**
   * Whether `shown` is read ahead (`readAhead`) rather than read (`getFile`):
   * a folder's cover, which looking at must not set the folder coming.
   */
  ahead: boolean;
  /**
   * The pages it keeps ready besides, nearest first and biased forward (see
   * [`prefetchTargets`](./prefetch)) — `null` where nothing anchors them, and
   * whatever is held stays held.
   */
  around: string[] | null;
}

/**
 * What the sheet fetches for the row it stands on: the picture it draws, and
 * the pages around it the focus is likely to land on next.
 *
 * Around a page, the pages beside it in its folder, `radius` on either side —
 * the files the reader cannot draw stepped over, since there is nothing of
 * them to fetch. A page is read (`getFile`), and the server takes that as the
 * book being read: where its folder is not wholly on this device, the rest of
 * it is set coming in the background, which is what somebody paging through
 * it wants. Only `radius` either side, since that fill brings the rest of the
 * folder anyway; what lies past the end of the folder is
 * [`readsOnFrom`](#readsOnFrom)'s.
 *
 * On a folder, its cover alone, read ahead (`readAhead`): fetched where this
 * device does not have it, and setting none of the folder coming, since
 * walking past folders with the sheet open is looking at them, not reading
 * them — a read of each cover would set every folder walked past coming, and
 * stop the fill of the one being read. No pages are guessed at around it
 * either, since a guess either way would fetch a folder's pages for every
 * folder walked past.
 *
 * On a file the reader cannot draw, nothing, and the pages held around where
 * the focus was stay held, so that stepping back onto them does not fetch them
 * again.
 */
export function sheetWanted(row: Sheeted, shows: SheetShows, radius: number): Wanted {
  switch (shows.kind) {
    case 'image': {
      const pages = pagesOf(row.kind === 'file' ? row.in.files : []);
      const at = pageAt(pages, shows.file.path);
      return {
        shown: shows.file,
        ahead: false,
        around: at === null ? [] : prefetchTargets(at, pages.length, radius).map((one) => pages[one].path),
      };
    }
    case 'folder':
      return { shown: shows.cover, ahead: true, around: [] };
    case 'unsupported':
      return { shown: null, ahead: false, around: null };
  }
}

/**
 * How near the end of its folder the focus comes before the reader reads
 * ahead into the next one: within this many files.
 *
 * More than the pages kept ready around the focus. Listing the next folder,
 * which sets the server reading the fronts of its Containers, and reading its
 * first pages takes a few round trips to Storage, and three pages are read
 * before they finish.
 */
export const READ_ON_WITHIN = 10;

/**
 * The file the reader reads on from: the last file of the focused file's
 * folder, where the focus is on one of that folder's last
 * {@link READ_ON_WITHIN} files, and `null` anywhere else — a folder row
 * included, which is looked at rather than read.
 *
 * What comes after it is the first file of another folder, which the tree
 * holds no listing of while it is closed; the server says which file that is
 * (see [`getBeside`](@coffret/api)), and the reader reads that folder's first
 * pages ahead, before the focus reaches the end (see
 * [`aheadIn`](#aheadIn)).
 */
export function readsOnFrom(row: Sheeted): string | null {
  if (row.kind !== 'file') {
    return null;
  }
  const files = row.in.files;
  const at = files.findIndex((one) => one.path === row.path);
  if (at === -1 || files.length - 1 - at >= READ_ON_WITHIN) {
    return null;
  }
  return files[files.length - 1].path;
}

/**
 * The pages read ahead in the folder the reader reads on into: the first
 * `count` pages of `listing` from the file `next` on, the files the reader
 * cannot draw stepped over.
 */
export function aheadIn(listing: Listing, next: string, count: number): string[] {
  const from = listing.files.findIndex((file) => file.path === next);
  return from === -1
    ? []
    : pagesOf(listing.files.slice(from))
        .slice(0, count)
        .map((page) => page.path);
}
