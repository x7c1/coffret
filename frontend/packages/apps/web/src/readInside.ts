// Reading on while the tree is filtered: `↑`/`↓` with the reader's sheet open
// stay inside what the filter kept, and stop at its ends. Kept free of DOM and
// of React so it is unit testable.
//
// What the rows on show answer is [`readOn`](./tree)'s, filtered or not: a
// filtered tree draws only the rows inside it and the folders above them, so
// the first file drawn that way, with nothing hiding rows between, is the next
// file inside. What this module answers is the rest — where a closed folder,
// or a listing not on hand, stands in the way, and the server is asked.
//
// How the server is asked. The server's reading order is the whole Library's
// (spec: LA-32), and a filter can keep folders far apart in it: a file after
// the last of one kept folder may be the first of thousands that were not
// kept. Asking again from each answer that fell outside would cost a request
// per file between two matches. But the page already knows every row the
// filter kept, and so which part of the Library comes next in reading order:
// a kept file is landed on with no request at all, and a kept folder is asked
// about with `under` naming it, which answers its first or its last file
// directly. Reading on inside a kept folder asks with `under` too, so the
// answer never steps out of it. So a key press asks at most twice — once
// inside the folder the focus is in, once for the next kept folder — however
// far apart the matches stand, and usually once or not at all. (A kept folder
// the Library has emptied since the filter was answered is passed over for
// the next, at one more request each, until the filter is answered again.)

import type { Beside } from '@coffret/api';

import type { FilterShape } from './filter';
import { parentOf } from './prospective';
import { inLibraryOrder } from './stranded';
import { drawnBeside, fileKey, hides, keyed, type TreeRow, type TreeState } from './tree';

/** Asks the server for the files beside a row (see [`getBeside`](@coffret/api)). */
export type AskBeside = (path: string, folder: boolean, under?: string) => Promise<Beside>;

/** Where reading on inside a filtered tree goes. */
export type ReadInside =
  /** To the file at `path`, the folders on the way down to it opened. */
  | { kind: 'land'; path: string }
  /** To the row drawn beside the focus: there is no file inside further that way. */
  | { kind: 'focus'; focus: string }
  /** Nowhere: no file inside further that way, and no row drawn beside. */
  | { kind: 'stay' };

/**
 * A part of a filtered tree every file of which is inside it: a file the
 * filter kept, a folder it kept — everything under it — or the files directly
 * in the Library root, where the root is answered as a book.
 */
type Part = { kind: 'file'; path: string } | { kind: 'folder'; path: string } | { kind: 'root' };

/**
 * One step of a place in reading order: whether it is a file, and its name.
 * Folders come before files, each in the Library's order.
 */
type Step = readonly [file: boolean, name: string];

/** A place in reading order, step by step down from the Library root. */
type Place = readonly Step[];

function placeOf(path: string, folder: boolean): Place {
  const names = path.split('/');
  return names.map((name, at) => [at === names.length - 1 && !folder, name] as const);
}

/**
 * Where a part starts. The root's own files come after every folder in it, so
 * the root as a book starts after them all, and before its first file.
 */
function startOf(part: Part): Place {
  return part.kind === 'root' ? [[true, '']] : placeOf(part.path, part.kind === 'folder');
}

/** Reading order (spec: LA-32): a place before everything under it. */
function compared(left: Place, right: Place): number {
  for (let at = 0; at < Math.min(left.length, right.length); at += 1) {
    const [leftFile, leftName] = left[at];
    const [rightFile, rightName] = right[at];
    if (leftFile !== rightFile) {
      return leftFile ? 1 : -1;
    }
    const order = inLibraryOrder(leftName, rightName);
    if (order !== 0) {
      return order;
    }
  }
  return left.length - right.length;
}

function samePart(one: Part, other: Part): boolean {
  if (one.kind === 'root' || other.kind === 'root') {
    return one.kind === other.kind;
  }
  return one.kind === other.kind && one.path === other.path;
}

function under(path: string, folder: string): boolean {
  return path.startsWith(`${folder}/`);
}

const parted = new WeakMap<FilterShape, Part[]>();

/**
 * The parts of a filtered tree, in reading order: each row the filter kept
 * that no folder it kept stands above, and the Library root's files where it
 * is answered as a book.
 */
function partsOf(shape: FilterShape): Part[] {
  const held = parted.get(shape);
  if (held !== undefined) {
    return held;
  }
  const kept = [...shape.answered.keys()].map(keyed);
  const folders = kept.filter((one) => one.kind === 'folder').map((one) => one.path);
  const covered = (path: string) =>
    folders.some((folder) => under(path, folder)) || (shape.rootBook && parentOf(path) === '');
  const parts: Part[] = kept
    .filter((one) => !covered(one.path))
    .map((one) => ({ kind: one.kind, path: one.path }));
  if (shape.rootBook) {
    parts.push({ kind: 'root' });
  }
  parts.sort((left, right) => compared(startOf(left), startOf(right)));
  parted.set(shape, parts);
  return parts;
}

/** The part of a filtered tree the row at `path` stands in, or `null` where it stands in none. */
function partOf(shape: FilterShape, path: string, folder: boolean): Part | null {
  const holder = partsOf(shape).find(
    (part) => part.kind === 'folder' && (under(path, part.path) || (folder && path === part.path)),
  );
  if (holder !== undefined) {
    return holder;
  }
  if (!folder && shape.answered.has(fileKey(path))) {
    return { kind: 'file', path };
  }
  if (!folder && shape.rootBook && parentOf(path) === '') {
    return { kind: 'root' };
  }
  return null;
}

/**
 * Whether the rows hide something inside `folder` between the focus and the
 * end of that folder's rows the way `down` says: where they do not, there is
 * nothing more inside the folder that way to ask about, since the rows drawn
 * there are no files either (or [`readOn`](./tree) would have landed on one).
 * A focus whose row is not drawn says nothing, and is asked about.
 */
function hiddenWithin(rows: readonly TreeRow[], focus: string, folder: string, down: boolean): boolean {
  const at = rows.findIndex((row) => row.kind !== 'note' && row.key === focus);
  if (at === -1) {
    return true;
  }
  for (let step = down ? at + 1 : at - 1; step >= 0 && step < rows.length; step += down ? 1 : -1) {
    const row = rows[step];
    if (!under(row.path, folder)) {
      return false;
    }
    if (hides(row)) {
      return true;
    }
  }
  return false;
}

/**
 * The first file of `part` reading on into it, or its last reading back into
 * it; `null` where it holds none.
 */
async function endOf(
  part: Part,
  down: boolean,
  rows: readonly TreeRow[],
  ask: AskBeside,
): Promise<string | null> {
  switch (part.kind) {
    case 'file':
      return part.path;
    case 'folder': {
      // The folder's own place is before everything under it, and the file at
      // its path after everything under it (spec: LA-32).
      const answer = await ask(part.path, down, part.path);
      return down ? answer.next : answer.previous;
    }
    case 'root': {
      // The root's listing draws the tree, so its files are rows on show.
      const files = rows.filter((row) => row.kind === 'file' && parentOf(row.path) === '');
      const end = down ? files[0] : files[files.length - 1];
      return end === undefined ? null : end.path;
    }
  }
}

/**
 * Where `↑`/`↓` read on to from the focus while the tree is filtered by
 * `shape`, where [`readOn`](./tree) said the server is to be asked: to the
 * file before or after it in reading order among the files inside the
 * filtered tree — a file the filter kept, one anywhere under a folder it kept,
 * or one directly in the Library root where the root is answered as a book.
 * A file outside is never landed on.
 *
 * At either end, with no file inside further that way, the focus goes to the
 * row drawn beside it, or stays where none is drawn; it never wraps.
 *
 * `going` says whether the press is still wanted — the focus not moved, the
 * sheet not closed, the key not pressed again, the filter still the one it was
 * pressed under — and once it is not, nothing more is asked, and the focus
 * stays.
 */
export async function readInside(
  rows: readonly TreeRow[],
  state: TreeState,
  arrow: 'ArrowUp' | 'ArrowDown',
  shape: FilterShape,
  ask: AskBeside,
  going: () => boolean = () => true,
): Promise<ReadInside> {
  if (state.focus === null) {
    return { kind: 'stay' };
  }
  const focus = keyed(state.focus);
  const folder = focus.kind === 'folder';
  const down = arrow === 'ArrowDown';
  const own = partOf(shape, focus.path, folder);
  // Inside the kept folder the focus stands in, the server answers for that
  // folder alone.
  if (own?.kind === 'folder' && hiddenWithin(rows, state.focus, own.path, down)) {
    const answer = await ask(focus.path, folder, own.path);
    const landed = down ? answer.next : answer.previous;
    if (landed !== null) {
      return { kind: 'land', path: landed };
    }
  }
  // Then the parts beyond it, nearest first. Each is only passed over where
  // the Library changed since the filter was answered and it holds no file
  // any more, so the first one normally answers.
  const at = placeOf(focus.path, folder);
  const beyond = partsOf(shape).filter(
    (part) => (own === null || !samePart(part, own)) && compared(startOf(part), at) * (down ? 1 : -1) > 0,
  );
  if (!down) {
    beyond.reverse();
  }
  for (const part of beyond) {
    if (!going()) {
      return { kind: 'stay' };
    }
    const landed = await endOf(part, down, rows, ask);
    if (landed !== null) {
      return { kind: 'land', path: landed };
    }
  }
  const beside = drawnBeside(rows, state.focus, arrow);
  return beside === null ? { kind: 'stay' } : { kind: 'focus', focus: beside };
}
