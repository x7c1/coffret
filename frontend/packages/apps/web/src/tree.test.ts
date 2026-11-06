import { describe, expect, it } from 'vitest';

import type { Filtered, ListedFile, ListedFolder, Listing } from '@coffret/api';

import { filteredOpen, shapeOf } from './filter';

import {
  ancestry,
  arrowed,
  contextOf,
  escaped,
  fileKey,
  folderKey,
  landedOn,
  readOn,
  refocused,
  shownFolders,
  standing,
  toggled,
  treeRows,
  type Arrow,
  type TreeRow,
  type TreeState,
} from './tree';
import type { Asked } from './useAsked';

function folder(path: string, over: Partial<ListedFolder> = {}): ListedFolder {
  return {
    name: path.slice(path.lastIndexOf('/') + 1),
    path,
    mapped: true,
    present_throughout: false,
    files_total: 1,
    files_present: 0,
    size: 0,
    mtime: null,
    ...over,
  };
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

  it('closes an open folder on Escape and keeps the focus on it, nested or top-level', () => {
    const nested = escaped(rowsOf(['albums', 'albums/2026']), state(folderKey('albums/2026'), ['albums', 'albums/2026']));
    expect(nested.focus).toBe(folderKey('albums/2026'));
    expect([...nested.expanded]).toEqual(['albums']);
    const top = escaped(rowsOf(['albums', 'books']), state(folderKey('albums'), ['albums', 'books']));
    expect(top.focus).toBe(folderKey('albums'));
    expect([...top.expanded]).toEqual(['books']);
  });

  it('closes the folder a file or a closed folder stands in on Escape, and moves the focus to it', () => {
    const open = ['albums', 'albums/2026'];
    const fromFile = escaped(rowsOf(open), state(fileKey('albums/2026/a.jpg'), open));
    expect(fromFile.focus).toBe(folderKey('albums/2026'));
    expect([...fromFile.expanded]).toEqual(['albums']);
    const fromClosed = escaped(rowsOf(['albums']), state(folderKey('albums/2026'), ['albums']));
    expect(fromClosed.focus).toBe(folderKey('albums'));
    expect(fromClosed.expanded.size).toBe(0);
  });

  it('lets go of the focus on Escape from a top-level file or closed folder', () => {
    for (const focus of [fileKey('readme.txt'), folderKey('books')]) {
      const next = escaped(rowsOf(['albums']), state(focus, ['albums']));
      expect(next.focus).toBeNull();
      expect([...next.expanded]).toEqual(['albums']);
    }
    const none = state(null, ['albums']);
    expect(escaped(rowsOf(['albums']), none)).toBe(none);
  });

  it('folds one level on each Escape from a file three levels deep, and lets go on the last', () => {
    const seen: (string | null)[] = [];
    let held = state(fileKey('albums/2026/a.jpg'), ['albums', 'albums/2026', 'books']);
    while (held.focus !== null) {
      held = escaped(rowsOf(held.expanded), held);
      seen.push(held.focus);
    }
    expect(seen).toEqual([folderKey('albums/2026'), folderKey('albums'), null]);
    expect([...held.expanded]).toEqual(['books']);
  });

  it('forgets the subfolders open under a folder closed by Escape', () => {
    const open = ['albums', 'albums/2026'];
    const closed = escaped(rowsOf(open), state(folderKey('albums'), open));
    const reopened = toggled(closed, 'albums');
    expect([...reopened.expanded]).toEqual(['albums']);
    expect(drawn(reopened.expanded)).toEqual(['albums', '  2026', '  cover.png', 'books', 'readme.txt']);
  });

  it('keeps the subfolders open under a folder closed by ← or by its button', () => {
    const open = ['albums', 'albums/2026'];
    const byArrow = press(state(folderKey('albums'), open), 'ArrowLeft');
    expect([...byArrow.expanded]).toEqual(['albums/2026']);
    expect([...press(byArrow, 'ArrowRight').expanded].sort()).toEqual(open);
    const byButton = toggled(state(folderKey('albums'), open), 'albums');
    expect([...toggled(byButton, 'albums').expanded].sort()).toEqual(open);
    expect(drawn(toggled(byButton, 'albums').expanded)).toContain('    a.jpg');
  });

  it('says the folder the focus is in', () => {
    expect(contextOf(state(null))).toBe('');
    expect(contextOf(state(fileKey('albums/2026/a.jpg'), ['albums', 'albums/2026']))).toBe('albums/2026');
    expect(contextOf(state(folderKey('albums'), ['albums']))).toBe('albums');
    // A closed folder's listing is not on hand, so the folder it stands in is.
    expect(contextOf(state(folderKey('albums/2026'), ['albums']))).toBe('albums');
  });
});

// With the reader's sheet open, `↑`/`↓` from a file read on: to the file
// before or after it in reading order, at any depth, folder rows passed over;
// from a folder they move to the row drawn beside it. Where a closed folder
// stands in the way only the server can say what is in it, so the case
// answers for it out of the fixture, in reading order, as the server does.
describe('reading on with the reader open', () => {
  /** Every file of the fixture, in reading order: each folder's folders, then its files. */
  function readingOrder(folder = ''): string[] {
    const held = LIBRARY[folder];
    return [...held.folders.flatMap((one) => readingOrder(one.path)), ...held.files.map((one) => one.path)];
  }

  /** What the server answers for a row: the file after or before it. */
  function server(path: string, isFolder: boolean, arrow: 'ArrowUp' | 'ArrowDown'): string | null {
    const files = readingOrder();
    if (isFolder) {
      const inside = files.findIndex((one) => one.startsWith(`${path}/`));
      return arrow === 'ArrowDown' ? (files[inside] ?? null) : (files[inside - 1] ?? null);
    }
    const at = files.indexOf(path);
    return files[at + (arrow === 'ArrowDown' ? 1 : -1)] ?? null;
  }

  /** Presses `arrow` with the sheet open, asking the server where the rows cannot say. */
  function read(start: TreeState, arrow: 'ArrowUp' | 'ArrowDown'): { state: TreeState; asked: boolean } {
    const step = readOn(rowsOf(start.expanded), start, arrow);
    switch (step.kind) {
      case 'focus':
        return { state: step.state, asked: false };
      case 'stay':
        return { state: start, asked: false };
      case 'ask': {
        const landed = server(step.path, step.folder, arrow);
        return { state: landed === null ? start : landedOn(start, landed), asked: true };
      }
    }
  }

  it('reads off the end of one folder into the first file of the next, opening it', () => {
    const open = ['albums', 'albums/2026'];
    // `albums/2026` holds what comes before `albums`'s own files.
    const off = read(state(fileKey('albums/2026/b.jpg'), open), 'ArrowDown');
    expect(off).toEqual({ state: state(fileKey('albums/cover.png'), open), asked: false });
    // `books` is closed, so the server says its first file, two levels down,
    // and every folder on the way is opened.
    const into = read(off.state, 'ArrowDown');
    expect(into.asked).toBe(true);
    expect(into.state.focus).toBe(fileKey('books/vol-1/p1.png'));
    expect([...into.state.expanded]).toEqual(['albums', 'albums/2026', 'books', 'books/vol-1']);
    // Now on show, the rows answer the rest, and the folders read through stay open.
    const next = read(into.state, 'ArrowDown');
    expect(next).toEqual({ state: { ...into.state, focus: fileKey('books/vol-1/p2.png') }, asked: false });
    // Out of the last folder, onto a file of the Library root: a file the
    // reader cannot show is landed on like any other, and the sheet says so.
    expect(read(next.state, 'ArrowDown').state.focus).toBe(fileKey('readme.txt'));
  });

  it('steps back from the first file of a folder onto the last of the one before', () => {
    const open = ['books', 'books/vol-1'];
    const back = read(state(fileKey('books/vol-1/p1.png'), open), 'ArrowUp');
    // `books` and `books/vol-1` are passed over, and `albums` is closed.
    expect(back.asked).toBe(true);
    expect(back.state.focus).toBe(fileKey('albums/cover.png'));
    expect(read(back.state, 'ArrowUp').state.focus).toBe(fileKey('albums/2026/b.jpg'));
    expect(read(state(fileKey('readme.txt'), open), 'ArrowUp').state.focus).toBe(fileKey('books/vol-1/p2.png'));
  });

  // From a closed folder the sheet shows its cover, and the arrows page
  // through the rows drawn beside it rather than into it.
  it('moves from a closed folder to the row drawn beside it, never into it', () => {
    // `albums` and `books` side by side, both closed: `↓` pages from one cover
    // to the next, and on to the file drawn after the folders.
    const toSibling = read(state(folderKey('albums')), 'ArrowDown');
    expect(toSibling).toEqual({ state: state(folderKey('books')), asked: false });
    expect(read(toSibling.state, 'ArrowDown')).toEqual({ state: state(fileKey('readme.txt')), asked: false });
    expect(read(toSibling.state, 'ArrowUp')).toEqual({ state: state(folderKey('albums')), asked: false });
    // With an open folder above, `↑` lands on the last row it draws.
    const open = ['albums', 'albums/2026'];
    expect(read(state(folderKey('books'), open), 'ArrowUp')).toEqual({
      state: state(fileKey('albums/cover.png'), open),
      asked: false,
    });
    // A closed folder inside an open one: `↓` to the row after it, a file of
    // the folder above, and `↑` to the open folder's own row.
    expect(read(state(folderKey('albums/2026'), ['albums']), 'ArrowDown')).toEqual({
      state: state(fileKey('albums/cover.png'), ['albums']),
      asked: false,
    });
    expect(read(state(folderKey('albums/2026'), ['albums']), 'ArrowUp')).toEqual({
      state: state(folderKey('albums'), ['albums']),
      asked: false,
    });
  });

  it('pages through a run of closed folders, and stops at either end', () => {
    const series: Record<string, Listing> = {
      '': listing('', ['vol-1', 'vol-2', 'vol-3'], []),
      'vol-1': listing('vol-1', [], ['vol-1/001.png']),
      'vol-2': listing('vol-2', [], ['vol-2/001.png']),
      'vol-3': listing('vol-3', [], ['vol-3/001.png']),
    };
    const rows = treeRows({
      listing: (path) => (series[path] === undefined ? undefined : { status: 'ready', value: series[path] }),
      expanded: new Set(),
      prospective: [],
      stranded: [],
    });
    const on = (path: string) => state(folderKey(path));
    expect(readOn(rows, on('vol-1'), 'ArrowDown')).toEqual({ kind: 'focus', state: on('vol-2') });
    expect(readOn(rows, on('vol-2'), 'ArrowDown')).toEqual({ kind: 'focus', state: on('vol-3') });
    expect(readOn(rows, on('vol-3'), 'ArrowDown')).toEqual({ kind: 'stay' });
    expect(readOn(rows, on('vol-1'), 'ArrowUp')).toEqual({ kind: 'stay' });
    expect(readOn(rows, on('vol-3'), 'ArrowUp')).toEqual({ kind: 'focus', state: on('vol-2') });
  });

  it('keeps reading on from a file past a closed folder, asking the server', () => {
    const fromFile = read(state(fileKey('albums/cover.png'), ['albums']), 'ArrowDown');
    expect(fromFile.asked).toBe(true);
    expect(fromFile.state.focus).toBe(fileKey('books/vol-1/p1.png'));
  });

  // An open folder is a folder row like a closed one: the arrows go to the
  // row drawn beside it, and never ask the server.
  it('moves from an open folder to the row drawn beside it, whichever kind it is', () => {
    // Onto a closed child folder, not into it.
    expect(read(state(folderKey('books'), ['books']), 'ArrowDown')).toEqual({
      state: state(folderKey('books/vol-1'), ['books']),
      asked: false,
    });
    // Onto a child folder that is open, and from there onto its first file.
    const open = ['albums', 'albums/2026'];
    expect(read(state(folderKey('albums'), open), 'ArrowDown')).toEqual({
      state: state(folderKey('albums/2026'), open),
      asked: false,
    });
    expect(read(state(folderKey('albums/2026'), open), 'ArrowDown')).toEqual({
      state: state(fileKey('albums/2026/a.jpg'), open),
      asked: false,
    });
    // `↑` onto the folder above, and from an open folder onto the last row
    // drawn above it.
    expect(read(state(folderKey('albums/2026'), open), 'ArrowUp')).toEqual({
      state: state(folderKey('albums'), open),
      asked: false,
    });
    expect(read(state(folderKey('books'), [...open, 'books']), 'ArrowUp')).toEqual({
      state: state(fileKey('albums/cover.png'), [...open, 'books']),
      asked: false,
    });
  });

  it('moves from an empty open folder to the rows beside it', () => {
    const held: Record<string, Listing> = {
      '': listing('', ['a', 'empty', 'z'], []),
      a: listing('a', [], ['a/1.png']),
      empty: listing('empty', [], []),
      z: listing('z', [], ['z/1.png']),
    };
    const rows = treeRows({
      listing: (path) => (held[path] === undefined ? undefined : { status: 'ready', value: held[path] }),
      expanded: new Set(['a', 'empty']),
      prospective: [],
      stranded: [],
    });
    const open = ['a', 'empty'];
    expect(readOn(rows, state(folderKey('empty'), open), 'ArrowDown')).toEqual({
      kind: 'focus',
      state: state(folderKey('z'), open),
    });
    expect(readOn(rows, state(folderKey('empty'), open), 'ArrowUp')).toEqual({
      kind: 'focus',
      state: state(fileKey('a/1.png'), open),
    });
  });

  // Where the rows say there is no file further that way, the focus goes to
  // the row drawn beside the file instead.
  it('moves off the first file onto the folder rows above it', () => {
    const everything = ['albums', 'albums/2026', 'books', 'books/vol-1'];
    const first = state(fileKey('albums/2026/a.jpg'), everything);
    const up = read(first, 'ArrowUp');
    expect(up).toEqual({ state: state(folderKey('albums/2026'), everything), asked: false });
    // From there the folder rule takes over, up to the first row drawn.
    const top = read(up.state, 'ArrowUp');
    expect(top).toEqual({ state: state(folderKey('albums'), everything), asked: false });
    expect(readOn(rowsOf(everything), top.state, 'ArrowUp')).toEqual({ kind: 'stay' });
  });

  it('moves off the last file onto the rows drawn below it that hold no file', () => {
    const held: Record<string, Listing> = {
      '': listing('', ['a', 'empty'], []),
      a: listing('a', [], ['a/1.png', 'a/2.png']),
      empty: listing('empty', [], []),
    };
    const rows = treeRows({
      listing: (path) => (held[path] === undefined ? undefined : { status: 'ready', value: held[path] }),
      expanded: new Set(['a', 'empty']),
      prospective: [],
      stranded: [],
    });
    const open = ['a', 'empty'];
    expect(readOn(rows, state(fileKey('a/2.png'), open), 'ArrowDown')).toEqual({
      kind: 'focus',
      state: state(folderKey('empty'), open),
    });
    // The last row drawn: nothing further.
    expect(readOn(rows, state(folderKey('empty'), open), 'ArrowDown')).toEqual({ kind: 'stay' });
  });

  it('asks the server where an open folder’s rows are not on hand', () => {
    const rows = treeRows({
      listing: (path) => (path === 'books' ? { status: 'loading' } : asked(path)),
      expanded: new Set(['books']),
      prospective: [],
      stranded: [],
    });
    expect(readOn(rows, state(fileKey('readme.txt'), ['books']), 'ArrowUp')).toEqual({
      kind: 'ask',
      path: 'readme.txt',
      folder: false,
    });
  });

  // Reading on landed in a folder whose listing was then refused: the focused
  // row is not drawn, so the rows say nothing either side of it, and the
  // server is asked about the focus itself — the keys keep working.
  it('asks the server about a focus whose row is not drawn', () => {
    const open = ['albums', 'books', 'books/vol-1'];
    const rows = treeRows({
      listing: (path) => (path === 'books/vol-1' ? { status: 'failed', message: 'refused' } : asked(path)),
      expanded: new Set(open),
      prospective: [],
      stranded: [],
    });
    const landed = state(fileKey('books/vol-1/p1.png'), open);
    expect(rows.some((row) => row.key === landed.focus)).toBe(false);
    const back = readOn(rows, landed, 'ArrowUp');
    expect(back).toEqual({ kind: 'ask', path: 'books/vol-1/p1.png', folder: false });
    const on = readOn(rows, landed, 'ArrowDown');
    expect(on).toEqual({ kind: 'ask', path: 'books/vol-1/p1.png', folder: false });
    // The server answers out of reading order: back to the file before, and on to the next.
    expect(server('books/vol-1/p1.png', false, 'ArrowUp')).toBe('albums/cover.png');
    expect(server('books/vol-1/p1.png', false, 'ArrowDown')).toBe('books/vol-1/p2.png');
    // A folder focus not drawn is asked about as a folder.
    expect(readOn(rows, state(folderKey('books/vol-1/inner'), open), 'ArrowDown')).toEqual({
      kind: 'ask',
      path: 'books/vol-1/inner',
      folder: true,
    });
    // With no row focused there is nothing to read on from.
    expect(readOn(rows, state(null, open), 'ArrowDown')).toEqual({ kind: 'stay' });
  });

  it('does nothing at the first and the last row drawn, and does not wrap', () => {
    const everything = ['albums', 'albums/2026', 'books', 'books/vol-1'];
    const last = state(fileKey('readme.txt'), everything);
    expect(readOn(rowsOf(everything), last, 'ArrowDown')).toEqual({ kind: 'stay' });
    const first = state(folderKey('albums'), everything);
    expect(readOn(rowsOf(everything), first, 'ArrowUp')).toEqual({ kind: 'stay' });
    // With the folders closed, the server is asked and answers the same.
    expect(read(state(fileKey('albums/cover.png'), ['albums']), 'ArrowUp').state).toEqual(
      state(fileKey('albums/2026/b.jpg'), ['albums', 'albums/2026']),
    );
    const closed = state(fileKey('readme.txt'));
    expect(read(closed, 'ArrowDown')).toEqual({ state: closed, asked: false });
  });

  it('leaves the keys with the reader closed as they were: row by row', () => {
    const open = ['albums', 'albums/2026'];
    expect(press(state(fileKey('albums/2026/b.jpg'), open), 'ArrowDown').focus).toBe(fileKey('albums/cover.png'));
    expect(press(state(fileKey('albums/cover.png'), open), 'ArrowDown').focus).toBe(folderKey('books'));
    expect(press(state(fileKey('albums/2026/a.jpg'), open), 'ArrowUp').focus).toBe(folderKey('albums/2026'));
  });
});

describe('the folders kept open', () => {
  // A deletion took `albums/2026` away: its parent's listing no longer names
  // it, so it is not asked for again — nor is anything under it.
  it('drops a folder a listing shows is gone, and what was open under it', () => {
    const gone: Record<string, Listing> = { ...LIBRARY, albums: listing('albums', [], ['albums/cover.png']) };
    const ask = (path: string): Asked<Listing> | undefined =>
      gone[path] === undefined ? undefined : { status: 'ready', value: gone[path] };
    const expanded = new Set(['albums', 'albums/2026', 'albums/2026/08', 'books']);
    const rows = treeRows({ listing: ask, expanded, prospective: [], stranded: [] });
    expect([...standing(expanded, rows, ask, rowsOf(expanded))]).toEqual(['albums', 'books']);
  });

  // A stranded book's folder, which the Library never named: once its freeze
  // commits and the Library's folders name it, it is no longer drawn as
  // stranded, and until the listing it stands in is answered again no row
  // stands for it. Nothing is gone there, so it stays open and its listing
  // goes on being asked for, which is what brings its rows up to date.
  it('keeps a folder the Library never named open while no row stands for it', () => {
    const book = 'albums/book';
    const pages = listing(book, [], [`${book}/page-0000.jpg`], { held: false });
    const ask = (path: string): Asked<Listing> | undefined =>
      path === book ? { status: 'ready', value: pages } : asked(path);
    const expanded = new Set(['albums', book]);
    const stranded = treeRows({ listing: ask, expanded, prospective: [], stranded: [book] });
    expect(stranded.some((row) => row.kind === 'folder' && row.path === book && row.stranded)).toBe(true);
    const committed = treeRows({ listing: ask, expanded, prospective: [], stranded: [] });
    expect(committed.some((row) => row.path === book)).toBe(false);
    const kept = standing(expanded, committed, ask, stranded);
    expect(kept).toBe(expanded);
    expect(shownFolders(kept)).toContain(book);
    // Nor is a prepared path, drawn from this page alone, taken for gone.
    const prepared = treeRows({ listing: ask, expanded, prospective: [book], stranded: [] });
    expect(standing(expanded, committed, ask, prepared)).toBe(expanded);
  });

  it('keeps the set as it is where nothing is gone', () => {
    const expanded = new Set(['albums', 'albums/2026']);
    expect(standing(expanded, rowsOf(expanded), asked, rowsOf(expanded))).toBe(expanded);
  });

  // A listing still out, or refused — as every one is while the Library is
  // locked — shows nothing gone.
  it('drops nothing where the listing that would show it is out or refused', () => {
    const expanded = new Set(['albums', 'albums/2026']);
    for (const held of [{ status: 'loading' }, { status: 'failed', message: 'locked' }] as Asked<Listing>[]) {
      const ask = (path: string) => (path === 'albums' ? held : asked(path));
      const rows = treeRows({ listing: ask, expanded, prospective: [], stranded: [] });
      expect(standing(expanded, rows, ask, rowsOf(expanded))).toBe(expanded);
    }
  });

  // A folder under one that is closed is not on show, so its parent's listing
  // being on hand says nothing about it.
  it('keeps a folder under a closed one', () => {
    const expanded = new Set(['albums/2026']);
    expect(standing(expanded, rowsOf(expanded), asked, rowsOf(expanded))).toBe(expanded);
  });
});

describe('the focus after the tree is drawn again', () => {
  const expanded = ['albums', 'albums/2026'];

  it('goes back to the row it was on', () => {
    expect(refocused(rowsOf(expanded), fileKey('albums/2026/a.jpg'), asked)).toBe(fileKey('albums/2026/a.jpg'));
  });

  it('goes to the nearest folder above it still drawn where the row is gone', () => {
    expect(refocused(rowsOf(expanded), fileKey('albums/2026/gone.jpg'), asked)).toBe(folderKey('albums/2026'));
    expect(refocused(rowsOf(expanded), fileKey('albums/gone/x.jpg'), asked)).toBe(folderKey('albums'));
    expect(refocused(rowsOf(expanded), fileKey('gone/x.jpg'), asked)).toBeNull();
    expect(refocused(rowsOf(expanded), null, asked)).toBeNull();
  });

  // The root, or a folder on the way down, still being asked: the row may yet
  // be drawn, so nothing is settled.
  it('waits while the listings on the way down are out', () => {
    const loading = (out: string) => (path: string) =>
      path === out ? ({ status: 'loading' } as const) : asked(path);
    const row = fileKey('albums/2026/a.jpg');
    for (const out of ['', 'albums/2026']) {
      const ask = loading(out);
      const rows = treeRows({ listing: ask, expanded: new Set(expanded), prospective: [], stranded: [] });
      expect(refocused(rows, row, ask)).toBeUndefined();
    }
  });
});

describe('the tree a filter keeps', () => {
  const kept = (rows: Filtered['rows'], expanded: Iterable<string>, filtered = shapeOf({ rows, total: rows.length, truncated: false })) =>
    treeRows({ listing: asked, expanded: new Set(expanded), prospective: [], stranded: [], filter: filtered });
  const drawn = (rows: TreeRow[]) =>
    rows.map((row) =>
      row.kind === 'note'
        ? `note ${row.path}`
        : row.kind === 'folder'
          ? `${row.path}/${row.above === true ? ' above' : ''}${row.added !== undefined ? ` added ${row.added}` : ''}${row.expanded ? ' open' : ''}`
          : row.path,
    );

  it('draws a kept folder closed, under the folders above it drawn open and dimmed', () => {
    const answer: Filtered['rows'] = [{ path: 'albums/2026', kind: 'folder', deposited_at: '2026-01-02T00:00:00Z' }];
    const shape = shapeOf({ rows: answer, total: 1, truncated: false });
    expect(drawn(kept(answer, filteredOpen(shape, new Set()), shape))).toEqual([
      'albums/ above open',
      'albums/2026/ added 2026-01-02T00:00:00Z',
    ]);
    // Opened, it lists everything in it, not only what matched.
    expect(drawn(kept(answer, ['albums', 'albums/2026'], shape))).toEqual([
      'albums/ above open',
      'albums/2026/ added 2026-01-02T00:00:00Z open',
      'albums/2026/a.jpg',
      'albums/2026/b.jpg',
    ]);
    // A folder above a match closes like any other, and its match with it.
    expect(drawn(kept(answer, [], shape))).toEqual(['albums/ above']);
  });

  it('draws a kept file without the other files of its folder', () => {
    expect(drawn(kept([{ path: 'books/vol-1/p2.png', kind: 'file' }], ['books', 'books/vol-1']))).toEqual([
      'books/ above open',
      'books/vol-1/ above open',
      'books/vol-1/p2.png',
    ]);
    expect(drawn(kept([{ path: 'readme.txt', kind: 'file' }], []))).toEqual(['readme.txt']);
  });

  it('draws every file of the Library root answered as a book, and nothing where nothing matched', () => {
    expect(
      drawn(kept([{ path: '', kind: 'folder', deposited_at: null }, { path: 'albums/cover.png', kind: 'file' }], ['albums'])),
    ).toEqual(['albums/ above open', 'albums/cover.png', 'readme.txt']);
    expect(kept([], [])).toEqual([]);
  });

  it('says a listing above a match is still out', () => {
    // A match its folder's listing does not name yet: nothing drawn for it.
    expect(drawn(kept([{ path: 'books/vol-9/p1.png', kind: 'file' }], ['books', 'books/vol-9']))).toEqual(['books/ above open']);
    const out = treeRows({
      listing: (path) => (path === 'albums' ? { status: 'loading' } : asked(path)),
      expanded: new Set(['albums']),
      prospective: [],
      stranded: [],
      filter: shapeOf({ rows: [{ path: 'albums/cover.png', kind: 'file' }], total: 1, truncated: false }),
    });
    expect(drawn(out)).toEqual(['albums/ above open', 'note albums']);
  });
});
