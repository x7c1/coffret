import { describe, expect, it } from 'vitest';

import type { ListedFile, ListedFolder, Listing } from '@coffret/api';

import {
  ancestry,
  arrowed,
  contextOf,
  fileKey,
  folderKey,
  shownFolders,
  toggled,
  treeRows,
  type Arrow,
  type TreeState,
} from './tree';
import type { Asked } from './useAsked';

function folder(path: string, over: Partial<ListedFolder> = {}): ListedFolder {
  return { name: path.slice(path.lastIndexOf('/') + 1), path, mapped: true, present_throughout: false, ...over };
}

function file(path: string): ListedFile {
  return {
    name: path.slice(path.lastIndexOf('/') + 1),
    path,
    size: 10,
    mtime: null,
    deposited_at: null,
    state: 'remote',
    container: 'one-file',
    reserved: false,
    openable: true,
    content_type: 'image/png',
  };
}

function listing(path: string, folders: string[], files: string[], over: Partial<Listing> = {}): Listing {
  return {
    path,
    mapped: true,
    held: true,
    folders: folders.map((one) => folder(one)),
    files: files.map(file),
    folders_on_disk: [],
    empty_mapped_prefixes: [],
    ...over,
  };
}

/**
 * The fixture Library:
 *
 * ```
 * albums/            2026/, cover.png
 * albums/2026/       a.jpg, b.jpg
 * books/             vol-1/
 * books/vol-1/       p1.png, p2.png
 * readme.txt
 * ```
 */
const LIBRARY: Record<string, Listing> = {
  '': listing('', ['albums', 'books'], ['readme.txt']),
  albums: listing('albums', ['albums/2026'], ['albums/cover.png']),
  'albums/2026': listing('albums/2026', [], ['albums/2026/a.jpg', 'albums/2026/b.jpg']),
  books: listing('books', ['books/vol-1'], []),
  'books/vol-1': listing('books/vol-1', [], ['books/vol-1/p1.png', 'books/vol-1/p2.png']),
};

function asked(path: string): Asked<Listing> | undefined {
  const held = LIBRARY[path];
  return held === undefined ? undefined : { status: 'ready', value: held };
}

function rowsOf(expanded: Iterable<string>) {
  return treeRows({ listing: asked, expanded: new Set(expanded), prospective: [], stranded: [] });
}

/** The rows on show, each as its name indented two spaces per level. */
function drawn(expanded: Iterable<string>): string[] {
  return rowsOf(expanded).map((row) =>
    row.kind === 'note' ? `${'  '.repeat(row.depth)}(note)` : `${'  '.repeat(row.depth)}${row.name}`,
  );
}

function state(focus: string | null, expanded: Iterable<string> = []): TreeState {
  return { focus, expanded: new Set(expanded) };
}

/** Presses `keys` in turn on the fixture tree, which is drawn again after each. */
function press(start: TreeState, ...keys: Arrow[]): TreeState {
  return keys.reduce((held, key) => arrowed(rowsOf(held.expanded), held, key), start);
}

it('lists every folder on the way down to one, outermost first', () => {
  expect(ancestry('albums/2026/08')).toEqual(['albums', 'albums/2026', 'albums/2026/08']);
  expect(ancestry('albums')).toEqual(['albums']);
  expect(ancestry('')).toEqual([]);
});

describe('the rows of the tree', () => {
  it('draws the root’s folders then its files, each open folder’s rows under it', () => {
    expect(drawn([])).toEqual(['albums', 'books', 'readme.txt']);
    expect(drawn(['albums', 'albums/2026'])).toEqual([
      'albums',
      '  2026',
      '    a.jpg',
      '    b.jpg',
      '  cover.png',
      'books',
      'readme.txt',
    ]);
  });

  // A folder open under a closed one is not on show, and its listing is not
  // one the tree needs.
  it('draws nothing under a closed folder, whatever is open below it', () => {
    expect(drawn(['albums/2026'])).toEqual(['albums', 'books', 'readme.txt']);
    expect(shownFolders(new Set(['albums/2026', 'books']))).toEqual(['', 'books']);
  });

  it('stands a line in for a folder whose listing is out', () => {
    const rows = treeRows({
      listing: (path) => (path === 'albums' ? { status: 'loading' } : asked(path)),
      expanded: new Set(['albums']),
      prospective: [],
      stranded: [],
    });
    expect(rows.map((row) => row.kind)).toEqual(['folder', 'note', 'folder', 'file']);
  });

  // EP-3 order is the one every device agrees on, so a folder the Library does
  // not hold yet is merged into it rather than appended.
  it('merges a prepared folder into the Library’s order, with no disclosure', () => {
    const rows = treeRows({ listing: asked, expanded: new Set(), prospective: ['b'], stranded: [] });
    expect(rows.map((row) => row.kind !== 'note' && row.name)).toEqual(['albums', 'b', 'books', 'readme.txt']);
    const prepared = rows[1];
    expect(prepared.kind === 'folder' && prepared.prospective).toBe(true);
    expect(prepared.kind === 'folder' && prepared.expandable).toBe(false);
  });
});

describe('the keys on the tree', () => {
  it('moves down and up across open and closed folders', () => {
    const open = ['albums', 'albums/2026'];
    expect(press(state(null, open), 'ArrowDown').focus).toBe(folderKey('albums'));
    expect(press(state(folderKey('albums'), open), 'ArrowDown', 'ArrowDown').focus).toBe(
      fileKey('albums/2026/a.jpg'),
    );
    // `books` is closed, so its rows are walked past.
    expect(press(state(fileKey('albums/cover.png'), open), 'ArrowDown', 'ArrowDown').focus).toBe(
      fileKey('readme.txt'),
    );
    expect(press(state(fileKey('readme.txt'), open), 'ArrowUp', 'ArrowUp', 'ArrowUp').focus).toBe(
      fileKey('albums/2026/b.jpg'),
    );
  });

  it('moves no further than the first or the last row on show', () => {
    const first = state(folderKey('albums'));
    expect(press(first, 'ArrowUp')).toBe(first);
    const last = state(fileKey('readme.txt'));
    expect(press(last, 'ArrowDown')).toBe(last);
  });

  it('opens a closed folder with →, and then moves into it', () => {
    const opened = press(state(folderKey('books')), 'ArrowRight');
    expect(opened.focus).toBe(folderKey('books'));
    expect([...opened.expanded]).toEqual(['books']);
    const entered = press(opened, 'ArrowRight');
    expect(entered.focus).toBe(folderKey('books/vol-1'));
    expect(press(entered, 'ArrowRight', 'ArrowRight').focus).toBe(fileKey('books/vol-1/p1.png'));
    // A file has nothing to open.
    const onFile = state(fileKey('readme.txt'));
    expect(press(onFile, 'ArrowRight')).toBe(onFile);
  });

  it('closes an open folder with ←, and then climbs to the folder above', () => {
    const inside = state(fileKey('books/vol-1/p2.png'), ['books', 'books/vol-1']);
    const climbed = press(inside, 'ArrowLeft');
    expect(climbed.focus).toBe(folderKey('books/vol-1'));
    const closed = press(climbed, 'ArrowLeft');
    expect(closed.focus).toBe(folderKey('books/vol-1'));
    expect([...closed.expanded]).toEqual(['books']);
    expect(press(closed, 'ArrowLeft').focus).toBe(folderKey('books'));
    const top = press(closed, 'ArrowLeft', 'ArrowLeft');
    expect(top.expanded.size).toBe(0);
    // At the root's own rows there is nowhere further up to go.
    expect(press(top, 'ArrowLeft')).toBe(top);
  });

  // The focus is never on a row that is not on show.
  it('brings the focus out of a folder closed over it, and leaves it anywhere else', () => {
    const inside = state(fileKey('albums/2026/a.jpg'), ['albums', 'albums/2026', 'books']);
    expect(toggled(inside, 'albums').focus).toBe(folderKey('albums'));
    expect(toggled(inside, 'albums/2026').focus).toBe(folderKey('albums/2026'));
    expect(toggled(inside, 'books').focus).toBe(fileKey('albums/2026/a.jpg'));
    // A folder whose name starts with the closed one's is not inside it.
    const beside = state(folderKey('albums-old'), ['albums']);
    expect(toggled(beside, 'albums').focus).toBe(folderKey('albums-old'));
    // Opening moves nothing.
    expect(toggled(state(fileKey('readme.txt')), 'books').focus).toBe(fileKey('readme.txt'));
  });

  it('says the folder the focus is in', () => {
    expect(contextOf(state(null))).toBe('');
    expect(contextOf(state(fileKey('albums/2026/a.jpg'), ['albums', 'albums/2026']))).toBe('albums/2026');
    expect(contextOf(state(folderKey('albums'), ['albums']))).toBe('albums');
    // A closed folder's listing is not on hand, so the folder it stands in is.
    expect(contextOf(state(folderKey('albums/2026'), ['albums']))).toBe('albums');
  });
});
