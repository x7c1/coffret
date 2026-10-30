// The explorer's one tree: the Library's folders and files as rows of one
// table, indented by depth, and the keys that walk it. Kept free of DOM and of
// React so it is unit testable.
//
// A Library has no folders to nest — a folder is the separators in the Entry
// Paths under it (spec: EP-2) — so the tree is this screen's arrangement of the
// listings it asked for, one folder at a time: the Library root's always, and
// each expanded folder's as it is expanded. A folder row that has not been
// expanded costs nothing beyond the listing it came from.

import type { ListedFile, ListedFolder, Listing } from '@coffret/api';

import { parentOf, prospectiveIn } from './prospective';
import { inLibraryOrder } from './stranded';
import type { Asked } from './useAsked';

/**
 * Every folder on the way down to one, the outermost first, the folder itself
 * last.
 *
 * What the tree opens to when the screen is restored from the URL: a folder
 * several components deep is only reachable with each of its ancestors
 * expanded.
 */
export function ancestry(path: string): string[] {
  if (path === '') {
    return [];
  }
  const folders: string[] = [];
  let cut = path.indexOf('/');
  while (cut !== -1) {
    folders.push(path.slice(0, cut));
    cut = path.indexOf('/', cut + 1);
  }
  folders.push(path);
  return folders;
}

/**
 * What a row is called by, a folder's told from a file's: the Library admits a
 * file and a folder at one path, since neither is a thing it has.
 */
export function folderKey(path: string): string {
  return `folder:${path}`;
}

export function fileKey(path: string): string {
  return `file:${path}`;
}

/** Which row a key names: its kind, and its Entry Path. */
export function keyed(key: string): { kind: 'folder' | 'file'; path: string } {
  return { kind: isFolderKey(key) ? 'folder' : 'file', path: pathOf(key) };
}

/** The Entry Path a row's key names. */
function pathOf(key: string): string {
  return key.slice(key.indexOf(':') + 1);
}

/** Whether a row's key names a folder. */
function isFolderKey(key: string): boolean {
  return key.startsWith('folder:');
}

/** A folder as a row of the tree. */
export interface FolderRow {
  kind: 'folder';
  key: string;
  path: string;
  name: string;
  /** How many folders it stands below the Library root; the root's own rows are 0. */
  depth: number;
  folder: ListedFolder;
  /** The listing this row came from: the folder it stands in. */
  in: Listing;
  /** Whether it is drawn open, its rows below it. */
  expanded: boolean;
  /**
   * Whether it has a disclosure button: anything to open. A folder of the
   * Library always does (spec: EP-2); a folder it does not hold yet only where
   * something is known to stand under it.
   */
  expandable: boolean;
  /** A folder the Library does not hold yet, to drop into (see [`prospective`](./prospective)). */
  prospective: boolean;
  /** The folder of a book whose freeze has not committed (see [`stranded`](./stranded)). */
  stranded: boolean;
  /**
   * A folder files are being added under from this page, which no listing
   * names yet (see {@link IncomingFile}).
   */
  incoming?: boolean;
}

/**
 * A file this page is adding — picked with **+** or dropped, confirmed, and
 * on its way to the server — drawn as a row at once, before any listing names
 * it, so that Add is answered on the tree where the files will be. The row is
 * one the folder's listing takes over once it names the path.
 */
export interface IncomingFile {
  /** Its Entry Path, as it is sent. */
  path: string;
  size: number;
  /** Whether it goes up as part of a Pack, which its row says while it waits. */
  packed: boolean;
}

/** A file as a row of the tree. */
export interface FileRow {
  kind: 'file';
  key: string;
  path: string;
  name: string;
  depth: number;
  file: ListedFile;
  /** The listing this row came from: the folder it stands in. */
  in: Listing;
}

/**
 * Where an expanded folder's listing is still out or was refused: a line in
 * place of its rows, which the keys walk past.
 */
export interface NoteRow {
  kind: 'note';
  key: string;
  /** The folder whose rows this stands for. */
  path: string;
  depth: number;
  /** `null` while the listing is out, and the refusal's sentence once it was refused. */
  refused: string | null;
}

export type TreeRow = FolderRow | FileRow | NoteRow;

/** A row the keys can stand on: a folder or a file, never a note. */
type Walkable = FolderRow | FileRow;

/** What the tree is drawn from. */
export interface TreeSource {
  /** The listing of one folder as far as it has been asked; `undefined` where it has not. */
  listing: (folder: string) => Asked<Listing> | undefined;
  expanded: ReadonlySet<string>;
  /** The folders drawn that the Library does not hold yet (see [`prospectiveFolders`](./prospective)). */
  prospective: readonly string[];
  /** The folders of books whose freeze has not committed. */
  stranded: readonly string[];
  /** The files this page is adding, which no listing may name yet. */
  incoming?: readonly IncomingFile[];
}

/**
 * Every row on show, top to bottom: the root's folders then its files, each
 * expanded folder's rows straight under it, one level further in.
 *
 * Folders first and then files within each folder, both in the order the
 * server answered in — the byte order of the canonical paths, which is the
 * one order every device agrees on. The folders the Library does not hold yet
 * and the stranded ones are merged into that order rather than appended, so a
 * folder stays where it is drawn when its first Entry commits.
 *
 * Nothing for the root itself while its listing is out or refused: that is the
 * whole tree, and the screen says so in its place.
 */
export function treeRows(source: TreeSource): TreeRow[] {
  const rows: TreeRow[] = [];
  const prospective = new Set(source.prospective);
  // Every folder a stranded one implies, which no listing names: a book whose
  // freeze has not committed may stand in a folder the Library does not hold
  // either.
  const strandedAbove = new Set(source.stranded.flatMap(ancestry));
  const incoming = source.incoming ?? [];
  const walk = (folder: string, depth: number) => {
    const asked = source.listing(folder);
    if (asked === undefined || asked.status !== 'ready') {
      if (folder !== '') {
        rows.push({
          kind: 'note',
          key: `note:${folder}`,
          path: folder,
          depth,
          refused: asked?.status === 'failed' ? asked.message : null,
        });
      }
      return;
    }
    const listing = asked.value;
    const arriving = incomingIn(listing, incoming);
    for (const child of foldersIn(listing, source.prospective, strandedAbove, arriving.folders)) {
      const named = listing.folders.some((one) => one.path === child.path);
      // Not where the Library names it already: its first Entry committed, and
      // it is an ordinary folder from here on.
      const notHeld = prospective.has(child.path) && !named;
      const waiting = strandedAbove.has(child.path) && !named;
      const adding = arriving.folders.some((one) => one.path === child.path);
      const expandable = !notHeld || knownUnder(child.path, source);
      const expanded = expandable && source.expanded.has(child.path);
      rows.push({
        kind: 'folder',
        key: folderKey(child.path),
        path: child.path,
        name: child.name,
        depth,
        folder: child,
        in: listing,
        expanded,
        expandable,
        prospective: notHeld,
        stranded: waiting,
        ...(adding ? { incoming: true } : {}),
      });
      if (expanded) {
        walk(child.path, depth + 1);
      }
    }
    for (const file of arriving.files) {
      rows.push({
        kind: 'file',
        key: fileKey(file.path),
        path: file.path,
        name: file.name,
        depth,
        file,
        in: listing,
      });
    }
  };
  walk('', 0);
  return rows;
}

/**
 * The folders standing directly in the folder `listing` lists: the Library's,
 * the ones it does not hold yet, and the stranded ones, in the Library's order
 * and each once.
 */
function foldersIn(
  listing: Listing,
  prospective: readonly string[],
  stranded: ReadonlySet<string>,
  arriving: readonly ListedFolder[] = [],
): ListedFolder[] {
  const named = new Set(listing.folders.map((folder) => folder.path));
  const notHeld = prospectiveIn(listing, prospective);
  for (const folder of notHeld) {
    named.add(folder.path);
  }
  const waiting: ListedFolder[] = [...stranded]
    .filter((path) => parentOf(path) === listing.path && !named.has(path))
    .map((path) => ({
      name: path.slice(path.lastIndexOf('/') + 1),
      path,
      mapped: listing.mapped,
      present_throughout: false,
      // Its Entries are not committed, so the Library counts none under it.
      files_total: 0,
      files_present: 0,
      size: 0,
      mtime: null,
    }));
  for (const folder of waiting) {
    named.add(folder.path);
  }
  const extra = [...notHeld, ...waiting, ...arriving.filter((folder) => !named.has(folder.path))];
  if (extra.length === 0) {
    return listing.folders;
  }
  return [...listing.folders, ...extra].sort((left, right) => inLibraryOrder(left.path, right.path));
}

/**
 * What of `incoming` stands directly in the folder `listing` lists and the
 * listing does not name yet: the files, merged with its own in the Library's
 * order, and the folders some of them are under.
 */
function incomingIn(
  listing: Listing,
  incoming: readonly IncomingFile[],
): { files: ListedFile[]; folders: ListedFolder[] } {
  if (incoming.length === 0) {
    return { files: listing.files, folders: [] };
  }
  const prefix = listing.path === '' ? '' : `${listing.path}/`;
  const listed = new Set(listing.files.map((file) => file.path));
  const files: ListedFile[] = [];
  const folders = new Map<string, ListedFolder>();
  for (const one of incoming) {
    if (!one.path.startsWith(prefix)) {
      continue;
    }
    const rest = one.path.slice(prefix.length);
    const cut = rest.indexOf('/');
    if (cut === -1) {
      if (!listed.has(one.path)) {
        files.push(arrivingFile(one, rest));
      }
      continue;
    }
    const name = rest.slice(0, cut);
    const path = `${prefix}${name}`;
    if (!folders.has(path)) {
      folders.set(path, {
        name,
        path,
        mapped: listing.mapped,
        present_throughout: false,
        // Nothing under it is the Library's yet, so the Library counts none.
        files_total: 0,
        files_present: 0,
        size: 0,
        mtime: null,
      });
    }
  }
  return {
    files:
      files.length === 0
        ? listing.files
        : [...listing.files, ...files].sort((left, right) => inLibraryOrder(left.path, right.path)),
    folders: [...folders.values()],
  };
}

/**
 * An incoming file as a row: standing in the folder and not an Entry yet, as
 * a file a drop wrote is once it lands (`added`), and waiting for its Pack
 * where it goes up as one.
 */
function arrivingFile(one: IncomingFile, name: string): ListedFile {
  return {
    name,
    path: one.path,
    size: one.size,
    mtime: null,
    deposited_at: null,
    state: 'added',
    container: null,
    reserved: one.packed,
    openable: false,
    content_type: 'application/octet-stream',
  };
}

/**
 * Whether anything is known to stand under a folder the Library does not hold
 * yet: its own listing has rows, or another such folder is prepared inside it.
 * Where neither, it is an empty folder and has no disclosure button.
 */
function knownUnder(path: string, source: TreeSource): boolean {
  const asked = source.listing(path);
  if (asked?.status === 'ready') {
    const listing = asked.value;
    if (listing.files.length > 0 || listing.folders.length > 0) {
      return true;
    }
  }
  const prefix = `${path}/`;
  return (
    source.prospective.some((other) => other.startsWith(prefix)) ||
    source.stranded.some((other) => other.startsWith(prefix))
  );
}

/** Which row has the focus, and which folders are open. */
export interface TreeState {
  /** The key of the focused row, or `null` where none is. */
  focus: string | null;
  expanded: ReadonlySet<string>;
}

/** The arrow keys the tree answers. */
export type Arrow = 'ArrowUp' | 'ArrowDown' | 'ArrowLeft' | 'ArrowRight';

/**
 * What an arrow key does to the tree.
 *
 * - `↑`/`↓` move the focus to the previous or next row on show, and no
 *   further than the first or the last.
 * - `→` on a closed folder opens it; on an open one, moves to its first row.
 * - `←` on an open folder closes it; anywhere else, moves to the folder the
 *   row stands in.
 *
 * Neither `←` nor `→` turns a page, and nothing else does either: the
 * reader's sheet follows the focus, so `↑`/`↓` down the page rows of an open
 * book is paging (see [`ReaderView`](./ReaderView)).
 *
 * With no row focused, any of them focuses the first row — or, for `↑`, the
 * last — so that the keys have somewhere to start.
 *
 * The state comes back as it was where the key does nothing, so a caller can
 * tell a key that moved nothing.
 */
export function arrowed(rows: readonly TreeRow[], state: TreeState, arrow: Arrow): TreeState {
  const walkable = rows.filter((row): row is Walkable => row.kind !== 'note');
  if (walkable.length === 0) {
    return state;
  }
  const at = walkable.findIndex((row) => row.key === state.focus);
  if (at === -1) {
    const start = arrow === 'ArrowUp' ? walkable[walkable.length - 1] : walkable[0];
    return { ...state, focus: start.key };
  }
  const row = walkable[at];
  const open = row.kind === 'folder' && row.expandable && state.expanded.has(row.path);
  switch (arrow) {
    case 'ArrowUp':
    case 'ArrowDown': {
      const next = at + (arrow === 'ArrowDown' ? 1 : -1);
      if (next < 0 || next >= walkable.length) {
        return state;
      }
      return { ...state, focus: walkable[next].key };
    }
    case 'ArrowRight': {
      if (row.kind !== 'folder' || !row.expandable) {
        return state;
      }
      if (!open) {
        return { ...state, expanded: new Set([...state.expanded, row.path]) };
      }
      const first = walkable[at + 1];
      return first !== undefined && first.depth === row.depth + 1 && parentOf(first.path) === row.path
        ? { ...state, focus: first.key }
        : state;
    }
    case 'ArrowLeft': {
      if (open) {
        return collapsed(state, row.path);
      }
      const parent = parentOf(row.path);
      if (parent === '') {
        return state;
      }
      const holder = walkable.find((one) => one.key === folderKey(parent));
      return holder === undefined ? state : { ...state, focus: holder.key };
    }
  }
}

/**
 * What a press on a folder's disclosure button does: opens it, or closes it.
 *
 * The focus stays where it is — except that closing a folder the focused row
 * stands in moves the focus to that folder, so the focus is never on a row
 * that is not on show.
 */
export function toggled(state: TreeState, folder: string): TreeState {
  if (state.expanded.has(folder)) {
    return collapsed(state, folder);
  }
  return { ...state, expanded: new Set([...state.expanded, folder]) };
}

/** `folder` closed, and the focus brought out of it where it was inside. */
function collapsed(state: TreeState, folder: string): TreeState {
  const expanded = new Set(state.expanded);
  expanded.delete(folder);
  const inside = state.focus !== null && pathOf(state.focus).startsWith(`${folder}/`);
  return { focus: inside ? folderKey(folder) : state.focus, expanded };
}

/**
 * The folder the screen's own actions are about — what a drop on the list's
 * empty space lands in: the focused folder where it is open, and otherwise the
 * folder the focused row stands in; the Library root where nothing is
 * focused.
 *
 * An open folder or the folder a row stands in, never a closed one: what the
 * screen's actions act on is the folder whose rows are on show, and a closed
 * folder's are not. (The details pane asks for a focused closed folder's
 * listing all the same, to describe it; that is the pane's, not the
 * context's.)
 */
export function contextOf(state: TreeState): string {
  if (state.focus === null) {
    return '';
  }
  const path = pathOf(state.focus);
  return isFolderKey(state.focus) && state.expanded.has(path) ? path : parentOf(path);
}

/** The folders a tree with `expanded` open shows rows of, the root first. */
export function shownFolders(expanded: ReadonlySet<string>): string[] {
  return [
    '',
    ...[...expanded].filter((path) => ancestry(path).every((folder) => expanded.has(folder))),
  ];
}

/**
 * Where the focus goes back to once the tree is drawn again from scratch — as
 * it is after an unlock, the lock having refused every listing it was drawn
 * from: `focus` itself where its row is drawn again, and otherwise the nearest
 * folder above it whose row is, or `null` where none is.
 *
 * `undefined` while that cannot be told yet: while the root's listing is not
 * on hand, or a folder on the way down to the row is still being asked, the
 * row may yet be drawn, and settling for a folder above it would lose it.
 */
export function refocused(
  rows: readonly TreeRow[],
  focus: string | null,
  listing: (folder: string) => Asked<Listing> | undefined,
): string | null | undefined {
  if (focus === null) {
    return null;
  }
  if (listing('')?.status !== 'ready') {
    return undefined;
  }
  const drawn = (key: string) => rows.some((row) => row.kind !== 'note' && row.key === key);
  if (drawn(focus)) {
    return focus;
  }
  const above = ancestry(parentOf(pathOf(focus)));
  if (above.some((folder) => listing(folder)?.status === 'loading')) {
    return undefined;
  }
  const holder = above.reverse().find((folder) => drawn(folderKey(folder)));
  return holder === undefined ? null : folderKey(holder);
}

/**
 * `expanded` without the folders a listing shows are gone — a folder a
 * deletion took away, or one another device removed — and without anything
 * under them; `expanded` itself where none is.
 *
 * A folder is gone where the folder it stands in is on show — the root, or an
 * open folder whose row is drawn — and that folder's listing is on hand and
 * draws no row for it. Left open, it would be asked for once more and answer
 * that the Library holds nothing there, and a folder of the same name made
 * later would come back open. A listing still out or refused shows nothing
 * gone: a lock refuses every listing, and the tree comes back as it was.
 */
export function standing(
  expanded: ReadonlySet<string>,
  rows: readonly TreeRow[],
  listing: (folder: string) => Asked<Listing> | undefined,
): ReadonlySet<string> {
  const drawn = new Set(rows.filter((row) => row.kind === 'folder').map((row) => row.path));
  const gone = [...expanded].filter((path) => {
    const parent = parentOf(path);
    const onShow = parent === '' || (drawn.has(parent) && expanded.has(parent));
    return onShow && listing(parent)?.status === 'ready' && !drawn.has(path);
  });
  if (gone.length === 0) {
    return expanded;
  }
  return new Set(
    [...expanded].filter((path) => !gone.some((left) => path === left || path.startsWith(`${left}/`))),
  );
}
