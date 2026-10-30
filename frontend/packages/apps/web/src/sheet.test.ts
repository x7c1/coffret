import { describe, expect, it } from 'vitest';

import type { ListedFile, ListedFolder, Listing } from '@coffret/api';

import { size } from './humanize';
import {
  counterOf,
  NO_IMAGES,
  sheetKey,
  sheetOn,
  sheetShows,
  sheetWanted,
  type Sheeted,
} from './sheet';
import { fileKey, folderKey, treeRows } from './tree';
import type { Asked } from './useAsked';

function folder(path: string, over: Partial<ListedFolder> = {}): ListedFolder {
  return {
    name: path.slice(path.lastIndexOf('/') + 1),
    path,
    mapped: true,
    present_throughout: false,
    files_total: 1,
    files_present: 0,
    size: 2_000_000,
    mtime: null,
    ...over,
  };
}

function file(path: string): ListedFile {
  const image = /\.(jpg|png)$/.test(path);
  return {
    name: path.slice(path.lastIndexOf('/') + 1),
    path,
    size: 10,
    mtime: null,
    deposited_at: null,
    state: 'remote',
    container: 'one-file',
    reserved: false,
    openable: image,
    content_type: image ? 'image/jpeg' : 'text/plain',
  };
}

function listing(path: string, folders: string[], files: string[]): Listing {
  return {
    path,
    mapped: true,
    held: true,
    folders: folders.map((one) => folder(one)),
    files: files.map(file),
    folders_on_disk: [],
    empty_mapped_prefixes: [],
  };
}

/**
 * The fixture Library:
 *
 * ```
 * shelf/             vol-1/, vol-2/, empty/, notes.txt
 * shelf/vol-1/       p1.jpg, p2.jpg, notes.txt, p3.jpg
 * shelf/vol-2/       (not listed yet)
 * shelf/empty/       readme.txt
 * ```
 */
const LIBRARY: Record<string, Listing> = {
  '': listing('', ['shelf'], []),
  shelf: listing('shelf', ['shelf/vol-1', 'shelf/vol-2', 'shelf/empty'], ['shelf/notes.txt']),
  'shelf/vol-1': listing(
    'shelf/vol-1',
    [],
    ['shelf/vol-1/p1.jpg', 'shelf/vol-1/p2.jpg', 'shelf/vol-1/notes.txt', 'shelf/vol-1/p3.jpg'],
  ),
  'shelf/empty': listing('shelf/empty', [], ['shelf/empty/readme.txt']),
};

function asked(path: string): Asked<Listing> | undefined {
  const held = LIBRARY[path];
  return held === undefined ? undefined : { status: 'ready', value: held };
}

const ROWS = treeRows({
  listing: asked,
  expanded: new Set(['shelf', 'shelf/vol-1']),
  prospective: [],
  stranded: [],
});

function rowAt(key: string): Sheeted {
  const row = ROWS.find((one) => one.key === key);
  if (row === undefined || row.kind === 'note') {
    throw new Error(`no row ${key}`);
  }
  return row;
}

describe('counterOf', () => {
  // Folders and files the reader cannot draw count alike, so the total is the
  // folder's Items and the counter moves on every row the arrows land on.
  it('counts a folder among the folders and files beside it', () => {
    expect(counterOf(ROWS, folderKey('shelf/vol-1'))).toEqual({ at: 1, of: 4 });
    expect(counterOf(ROWS, folderKey('shelf/empty'))).toEqual({ at: 3, of: 4 });
  });

  it('counts an image among the images and the file beside them that is not one', () => {
    expect(counterOf(ROWS, fileKey('shelf/vol-1/p2.jpg'))).toEqual({ at: 2, of: 4 });
    expect(counterOf(ROWS, fileKey('shelf/vol-1/notes.txt'))).toEqual({ at: 3, of: 4 });
  });

  it('counts the last sibling as the total', () => {
    expect(counterOf(ROWS, fileKey('shelf/vol-1/p3.jpg'))).toEqual({ at: 4, of: 4 });
    expect(counterOf(ROWS, fileKey('shelf/notes.txt'))).toEqual({ at: 4, of: 4 });
    expect(counterOf(ROWS, folderKey('shelf'))).toEqual({ at: 1, of: 1 });
  });

  it('says nothing for a row not on show', () => {
    expect(counterOf(ROWS, fileKey('shelf/vol-2/p1.jpg'))).toBeNull();
  });
});

describe('sheetShows', () => {
  it('shows an image file as itself', () => {
    const shows = sheetShows(rowAt(fileKey('shelf/vol-1/p1.jpg')), undefined);
    expect(shows).toEqual({ kind: 'image', file: LIBRARY['shelf/vol-1'].files[0] });
  });

  // A cover, with what the folder holds said under it.
  it('shows a folder holding images by its first one, and what it holds', () => {
    expect(sheetShows(rowAt(folderKey('shelf/vol-1')), asked('shelf/vol-1'))).toEqual({
      kind: 'folder',
      cover: LIBRARY['shelf/vol-1'].files[0],
      caption: `Folder · 4 items · ${size(2_000_000)}`,
      note: null,
      refused: null,
    });
  });

  it('shows a folder holding no image as the stack saying so', () => {
    expect(sheetShows(rowAt(folderKey('shelf/empty')), asked('shelf/empty'))).toEqual({
      kind: 'folder',
      cover: null,
      caption: `Folder · 1 item · ${size(2_000_000)}`,
      note: NO_IMAGES,
      refused: null,
    });
  });

  // Not "No images" before the listing has said so.
  it('says nothing of a folder whose listing is still out, and the refusal where it was refused', () => {
    const vol2 = rowAt(folderKey('shelf/vol-2'));
    expect(sheetShows(vol2, { status: 'loading' })).toEqual({
      kind: 'folder',
      cover: null,
      caption: `Folder · ${size(2_000_000)}`,
      note: null,
      refused: null,
    });
    expect(sheetShows(vol2, { status: 'failed', message: 'the Library is locked' })).toMatchObject({
      note: null,
      refused: 'the Library is locked',
    });
  });

  it('shows a file a browser draws nothing from by its name alone', () => {
    expect(sheetShows(rowAt(fileKey('shelf/vol-1/notes.txt')), undefined)).toEqual({
      kind: 'unsupported',
      name: 'notes.txt',
    });
  });
});

describe('sheetWanted', () => {
  // The pages beside the one shown, the file between them stepped over. A
  // page is read, which is the book being read.
  it('reads an image and keeps the pages around it ready', () => {
    const row = rowAt(fileKey('shelf/vol-1/p2.jpg'));
    expect(sheetWanted(row, sheetShows(row, undefined), 3)).toEqual({
      shown: LIBRARY['shelf/vol-1'].files[1],
      peek: false,
      ahead: ['shelf/vol-1/p3.jpg', 'shelf/vol-1/p1.jpg'],
    });
  });

  // Walking past a folder is looking at it: its cover is peeked, which sets
  // nothing coming.
  it('peeks at a folder its cover and fetches nothing ahead', () => {
    const row = rowAt(folderKey('shelf/vol-1'));
    expect(sheetWanted(row, sheetShows(row, asked('shelf/vol-1')), 3)).toEqual({
      shown: LIBRARY['shelf/vol-1'].files[0],
      peek: true,
      ahead: [],
    });
  });

  it('fetches nothing for a file it cannot draw, and lets go of nothing held', () => {
    const row = rowAt(fileKey('shelf/vol-1/notes.txt'));
    expect(sheetWanted(row, sheetShows(row, undefined), 3)).toEqual({ shown: null, peek: false, ahead: null });
  });
});

describe('the sheet in the URL', () => {
  it('names the row it is open on by its key', () => {
    expect(sheetKey(sheetOn('shelf', { kind: 'folder', path: 'shelf/vol-1' }))).toBe(folderKey('shelf/vol-1'));
    expect(sheetKey(sheetOn('shelf/vol-1', { kind: 'file', path: 'shelf/vol-1/p1.jpg' }))).toBe(
      fileKey('shelf/vol-1/p1.jpg'),
    );
    expect(sheetKey({ folder: 'shelf', open: null })).toBeNull();
  });
});
