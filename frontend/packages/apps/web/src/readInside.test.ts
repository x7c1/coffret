import { describe, expect, it } from 'vitest';

import type { Beside, Filtered, ListedFile, ListedFolder, Listing } from '@coffret/api';

import { filteredOpen, shapeOf, type FilterShape } from './filter';
import { readInside, type AskBeside } from './readInside';
import { inLibraryOrder } from './stranded';
import { fileKey, folderKey, landedOn, readOn, readTo, treeRows, type TreeState } from './tree';

/**
 * The fixture Library, every file of it:
 *
 * ```
 * a-match/     1.jpg, 2.jpg
 * b-other/     1.jpg, 2.jpg
 * c-match/     1.jpg, 2.jpg
 * loose/       deep/1.jpg, a.jpg, match.jpg, z.jpg
 * z-match/     1.jpg
 * readme.txt, top.jpg
 * ```
 *
 * `b-other` stands between the two first matches in reading order, and
 * `loose`'s own files beside the one of them that matches.
 */
const FILES = [
  'a-match/1.jpg',
  'a-match/2.jpg',
  'b-other/1.jpg',
  'b-other/2.jpg',
  'c-match/1.jpg',
  'c-match/2.jpg',
  'loose/deep/1.jpg',
  'loose/a.jpg',
  'loose/match.jpg',
  'loose/z.jpg',
  'z-match/1.jpg',
  'readme.txt',
  'top.jpg',
];

function folder(path: string): ListedFolder {
  return {
    name: path.slice(path.lastIndexOf('/') + 1),
    path,
    mapped: true,
    present_throughout: false,
    files_total: 1,
    files_present: 0,
    size: 0,
    mtime: null,
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
    content_type: 'image/jpeg',
  };
}

/** The listing of one folder of `files`, as the server answers it. */
function listingOf(files: readonly string[], path: string): Listing {
  const prefix = path === '' ? '' : `${path}/`;
  const folders = new Set<string>();
  const direct: string[] = [];
  for (const one of files.filter((each) => each.startsWith(prefix))) {
    const rest = one.slice(prefix.length);
    const cut = rest.indexOf('/');
    if (cut === -1) {
      direct.push(one);
    } else {
      folders.add(`${prefix}${rest.slice(0, cut)}`);
    }
  }
  return {
    path,
    mapped: true,
    held: true,
    folders: [...folders].sort(inLibraryOrder).map(folder),
    files: direct.sort(inLibraryOrder).map(file),
    folders_on_disk: [],
    empty_mapped_prefixes: [],
  };
}

/** Reading order, as the server reads it (spec: LA-32): a folder's folders before its files. */
function before(left: string, leftFolder: boolean, right: string, rightFolder: boolean): number {
  const one = left.split('/');
  const other = right.split('/');
  for (let at = 0; at < Math.min(one.length, other.length); at += 1) {
    const leftFile = at === one.length - 1 && !leftFolder;
    const rightFile = at === other.length - 1 && !rightFolder;
    if (leftFile !== rightFile) {
      return leftFile ? 1 : -1;
    }
    const order = inLibraryOrder(one[at], other[at]);
    if (order !== 0) {
      return order;
    }
  }
  return one.length - other.length;
}

/** The server over `files`, answering beside a row as `GET /api/beside` does, and counting what it was asked. */
function serverOf(files: readonly string[]) {
  const asked: [string, boolean, string | undefined][] = [];
  const ask: AskBeside = (path, isFolder, under) => {
    asked.push([path, isFolder, under]);
    const among = files.filter((one) => under === undefined || one.startsWith(`${under}/`));
    const earlier = among.filter((one) => before(one, false, path, isFolder) < 0).sort((l, r) => before(l, false, r, false));
    const later = among.filter((one) => before(one, false, path, isFolder) > 0).sort((l, r) => before(l, false, r, false));
    const answer: Beside = { previous: earlier[earlier.length - 1] ?? null, next: later[0] ?? null };
    return Promise.resolve(answer);
  };
  return { ask, asked };
}

function shape(rows: Filtered['rows']): FilterShape {
  return shapeOf({ rows, total: rows.length, truncated: false });
}

/** What a name `match` is answered with: two folders, a file between them, and a last folder. */
const BY_NAME = shape([
  { path: 'a-match', kind: 'folder', deposited_at: null },
  { path: 'c-match', kind: 'folder', deposited_at: null },
  { path: 'loose/match.jpg', kind: 'file' },
  { path: 'z-match', kind: 'folder', deposited_at: null },
]);

/** What Recent is answered with: the root as a book, and two folders, one under a folder that is no book. */
const RECENT = shape([
  { path: '', kind: 'folder', deposited_at: null },
  { path: 'c-match', kind: 'folder', deposited_at: null },
  { path: 'loose/deep', kind: 'folder', deposited_at: null },
]);

function rowsOf(filter: FilterShape, expanded: ReadonlySet<string>) {
  return treeRows({
    listing: (path) => ({ status: 'ready', value: listingOf(FILES, path) }),
    expanded,
    prospective: [],
    stranded: [],
    filter,
  });
}

/** The tree just filtered by `filter`, the folders named opened too, and the focus on `focus`. */
function on(filter: FilterShape, focus: string, open: string[] = []): TreeState {
  return { focus, expanded: new Set([...filteredOpen(filter, new Set()), ...open]) };
}

/**
 * Presses `arrow` with the sheet open, the rows answering where they can and
 * the server asked where they cannot, as the explorer does. The server holds
 * `served`, which is the Library the rows were drawn from unless it changed
 * since.
 */
async function press(
  filter: FilterShape,
  start: TreeState,
  arrow: 'ArrowUp' | 'ArrowDown',
  served: readonly string[] = FILES,
) {
  const rows = rowsOf(filter, start.expanded);
  const server = serverOf(served);
  const step = readOn(rows, start, arrow);
  if (step.kind === 'focus') {
    return { state: step.state, asked: server.asked };
  }
  if (step.kind === 'stay') {
    return { state: start, asked: server.asked };
  }
  const went = await readInside(rows, start, arrow, filter, server.ask);
  const state =
    went.kind === 'land'
      ? landedOn(start, went.path)
      : went.kind === 'focus'
        ? readTo(start, went.focus)
        : start;
  return { state, asked: server.asked };
}

describe('reading on inside a filtered tree', () => {
  it('reads off the last file of one kept folder into the next, closed, past one not kept', async () => {
    const start = on(BY_NAME, fileKey('a-match/2.jpg'), ['a-match']);
    const { state, asked } = await press(BY_NAME, start, 'ArrowDown');
    expect(state.focus).toBe(fileKey('c-match/1.jpg'));
    expect(state.expanded.has('c-match')).toBe(true);
    // One request, for the kept folder itself: nothing of `b-other` is asked about.
    expect(asked).toEqual([['c-match', true, 'c-match']]);
  });

  it('reads back off the first file of a kept folder onto the last of the one before it, closed', async () => {
    const start = on(BY_NAME, fileKey('c-match/1.jpg'), ['c-match']);
    const { state, asked } = await press(BY_NAME, start, 'ArrowUp');
    expect(state.focus).toBe(fileKey('a-match/2.jpg'));
    expect(state.expanded.has('a-match')).toBe(true);
    expect(asked).toEqual([['a-match', false, 'a-match']]);
  });

  it('reads on inside a kept folder without stepping out of it', async () => {
    // `c-match/2.jpg` is the last file of `c-match`; the server is asked
    // about nothing inside, since nothing inside is hidden.
    const start = on(BY_NAME, fileKey('c-match/1.jpg'), ['c-match']);
    expect((await press(BY_NAME, start, 'ArrowDown')).state.focus).toBe(fileKey('c-match/2.jpg'));
    // With a closed folder inside, the server is asked for that folder's files alone.
    const deep = shape([{ path: 'loose', kind: 'folder', deposited_at: null }]);
    const inLoose = await press(deep, on(deep, folderKey('loose'), ['loose']), 'ArrowDown');
    expect(inLoose.state.focus).toBe(folderKey('loose/deep'));
    const fromA = await press(deep, on(deep, fileKey('loose/a.jpg'), ['loose']), 'ArrowUp');
    expect(fromA.state.focus).toBe(fileKey('loose/deep/1.jpg'));
    expect(fromA.asked).toEqual([['loose/a.jpg', false, 'loose']]);
  });

  it('passes over the unshown files beside a kept file alone in its folder', async () => {
    // `loose`'s own `a.jpg` and `z.jpg` stand either side of `match.jpg`.
    const start = on(BY_NAME, fileKey('loose/match.jpg'));
    const back = await press(BY_NAME, start, 'ArrowUp');
    expect(back.state.focus).toBe(fileKey('c-match/2.jpg'));
    const ahead = await press(BY_NAME, start, 'ArrowDown');
    expect(ahead.state.focus).toBe(fileKey('z-match/1.jpg'));
    expect(ahead.asked).toEqual([['z-match', true, 'z-match']]);
    // Into it, from a kept folder, with its folder closed: no request at all.
    const closed: TreeState = { focus: fileKey('c-match/2.jpg'), expanded: new Set(['c-match']) };
    const into = await press(BY_NAME, closed, 'ArrowDown');
    expect(into.state.focus).toBe(fileKey('loose/match.jpg'));
    expect(into.state.expanded.has('loose')).toBe(true);
    expect(into.asked).toEqual([]);
  });

  it('reads the root’s own files where the root is a book, and not those of a folder above a book', async () => {
    // From the last file of `c-match`, past `loose` closed, into `loose/deep`
    // — not `loose`'s own files, nor `z-match`.
    const fromC = await press(RECENT, { focus: fileKey('c-match/2.jpg'), expanded: new Set(['c-match']) }, 'ArrowDown');
    expect(fromC.state.focus).toBe(fileKey('loose/deep/1.jpg'));
    // `c-match` is read out of, and closes behind the focus.
    expect(fromC.state.expanded).toEqual(new Set(['loose', 'loose/deep']));
    // On from there, `loose`'s own files are not drawn: the root's are next.
    expect((await press(RECENT, fromC.state, 'ArrowDown')).state.focus).toBe(fileKey('readme.txt'));
    // And back from the root's first file, with `loose` closed, onto the last of `loose/deep`.
    const back = await press(RECENT, { focus: fileKey('readme.txt'), expanded: new Set() }, 'ArrowUp');
    expect(back.state.focus).toBe(fileKey('loose/deep/1.jpg'));
    expect(back.asked).toEqual([['loose/deep', false, 'loose/deep']]);
  });

  it('stops at the ends: on the row drawn beside where there is one, and nowhere where none is', async () => {
    // Before the first file inside, the folder it stands in.
    const first = await press(BY_NAME, on(BY_NAME, fileKey('a-match/1.jpg'), ['a-match']), 'ArrowUp');
    expect(first.state.focus).toBe(folderKey('a-match'));
    // After the last, nothing: `readme.txt` and `top.jpg` are not inside.
    const last = on(BY_NAME, fileKey('z-match/1.jpg'), ['z-match']);
    expect((await press(BY_NAME, last, 'ArrowDown')).state).toBe(last);
    // A kept folder the Library has emptied since it was drawn: passed over,
    // and with nothing inside after it, the focus goes to the row drawn
    // beside — the folder's own; the one read out of closes.
    const emptied = FILES.filter((one) => !one.startsWith('c-match/'));
    const twoFolders = shape([
      { path: 'a-match', kind: 'folder', deposited_at: null },
      { path: 'c-match', kind: 'folder', deposited_at: null },
    ]);
    const start = on(twoFolders, fileKey('a-match/2.jpg'), ['a-match']);
    const gone = await press(twoFolders, start, 'ArrowDown', emptied);
    expect(gone.state.focus).toBe(folderKey('c-match'));
    expect(gone.state.expanded).toEqual(new Set([...start.expanded].filter((path) => path !== 'a-match')));
  });

  it('never lands outside the filtered tree, nor opens a folder for a file there', async () => {
    // Every file it reads through, top to bottom, is inside.
    const inside = ['a-match/1.jpg', 'a-match/2.jpg', 'c-match/1.jpg', 'c-match/2.jpg', 'loose/match.jpg', 'z-match/1.jpg'];
    let state = on(BY_NAME, fileKey('a-match/1.jpg'), ['a-match']);
    const read = [state.focus];
    for (let times = 0; times < 8; times += 1) {
      state = (await press(BY_NAME, state, 'ArrowDown')).state;
      read.push(state.focus);
    }
    expect([...new Set(read)]).toEqual(inside.map(fileKey));
    expect(state.expanded.has('b-other')).toBe(false);
    expect([...state.expanded].every((path) => !path.startsWith('b-other'))).toBe(true);
  });

  it('closes the kept folder read out of, both ways, and keeps the rest open', async () => {
    // Down from the last file of `a-match` into `c-match`: `a-match` closes,
    // and `loose`, which is open above a kept file, stays open.
    const start = on(BY_NAME, fileKey('a-match/2.jpg'), ['a-match']);
    const down = await press(BY_NAME, start, 'ArrowDown');
    expect(down.state).toEqual({ focus: fileKey('c-match/1.jpg'), expanded: new Set(['loose', 'c-match']) });
    // And back up: `c-match` closes, `a-match` opens again.
    const up = await press(BY_NAME, down.state, 'ArrowUp');
    expect(up.state).toEqual({ focus: fileKey('a-match/2.jpg'), expanded: new Set(['loose', 'a-match']) });
    // Out of `loose` up into `c-match`, closed: `loose` closes behind.
    const fromLoose = await press(BY_NAME, on(BY_NAME, fileKey('loose/match.jpg')), 'ArrowUp');
    expect(fromLoose.state).toEqual({ focus: fileKey('c-match/2.jpg'), expanded: new Set(['c-match']) });
  });

  it('closes nothing on a move inside a kept folder, nor on one with nowhere to go', async () => {
    const start = on(BY_NAME, fileKey('c-match/1.jpg'), ['c-match']);
    expect((await press(BY_NAME, start, 'ArrowDown')).state).toEqual({ ...start, focus: fileKey('c-match/2.jpg') });
    // Asked of the server, inside the kept folder `loose`.
    const deep = shape([{ path: 'loose', kind: 'folder', deposited_at: null }]);
    const fromA = on(deep, fileKey('loose/a.jpg'), ['loose']);
    expect((await press(deep, fromA, 'ArrowUp')).state).toEqual({
      focus: fileKey('loose/deep/1.jpg'),
      expanded: new Set(['loose', 'loose/deep']),
    });
    const last = on(BY_NAME, fileKey('z-match/1.jpg'), ['z-match']);
    expect((await press(BY_NAME, last, 'ArrowDown')).state).toBe(last);
  });

  it('asks nothing more once the press is let go of', async () => {
    const start = on(BY_NAME, fileKey('a-match/2.jpg'), ['a-match']);
    const rows = rowsOf(BY_NAME, start.expanded);
    const server = serverOf(FILES);
    const went = await readInside(rows, start, 'ArrowDown', BY_NAME, server.ask, () => false);
    expect(went).toEqual({ kind: 'stay' });
    expect(server.asked).toEqual([]);
  });
});
