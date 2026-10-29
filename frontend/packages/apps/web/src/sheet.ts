// What the reader's sheet stands on and shows: the row of the tree the focus
// is on, whatever it is. Kept free of DOM and of React so it is unit testable.
//
// The sheet follows the tree's focus rather than turning pages of its own, so
// everything here is read off the tree's rows and the listings the tree holds:
// walking the page rows of an open book with `↑`/`↓` is paging, and stepping
// onto the book's folder or the next book is what the sheet then shows.

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

/**
 * What the sheet says on the stack where a folder's cover is not on this
 * device: the sheet only peeks at a cover (see [`sheetWanted`](#sheetWanted)),
 * opening the folder included, so the cover comes once a page of the book is
 * read — the focus on one of its page rows — or the book is filled.
 */
export const COVER_NOT_HERE = 'Cover not on this device yet';

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
   * Whether `shown` is peeked at (`peekFile`) rather than read (`getFile`):
   * a folder's cover, which looking at must not set the folder coming.
   */
  peek: boolean;
  /**
   * The pages it keeps ready besides, nearest first and biased forward (see
   * [`prefetchTargets`](./prefetch)) — `null` where nothing anchors them, and
   * whatever is held stays held.
   */
  ahead: string[] | null;
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
 * it wants.
 *
 * On a folder, its cover alone, and only peeked at (`peekFile`): the next row
 * may be its first page or the next folder, and walking a shelf of books with
 * the sheet open is looking at them, not reading them. A read of each cover
 * would set every book walked past coming; a peek sets nothing coming and is
 * answered only from a file already on this device, so a cover not here yet
 * is said to be not here ({@link COVER_NOT_HERE}) rather than fetched. No
 * pages are guessed at around it either, since a guess either way would fetch
 * a book's pages for every folder walked past.
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
        peek: false,
        ahead: at === null ? [] : prefetchTargets(at, pages.length, radius).map((one) => pages[one].path),
      };
    }
    case 'folder':
      return { shown: shows.cover, peek: true, ahead: [] };
    case 'unsupported':
      return { shown: null, peek: false, ahead: null };
  }
}
