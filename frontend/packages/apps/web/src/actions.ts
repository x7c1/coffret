// What the actions menu offers for the rows it was opened on. Kept free of DOM
// and of React so it is unit testable.
//
// One menu for every row, opened by a right-click on the row or by the "⋮" at
// its right end, so that whatever a row offers is found in one place, worded
// one way. What it holds depends on the rows:
//
// - **Open**, where the reader can show the row ([`opensInReader`](./details)):
//   what the details pane's Open does, and what `Enter` and a double-click do
//   on a row not open already — those close a row opened again (see `openRow`
//   in [`TreeTable`](./TreeTable)), and Open does not.
// - **New folder…**, on a folder a mapping of this device reaches: where a
//   drop into it would be taken (see [`prospective`](./prospective)).
// - **Pack this folder…**, on a folder under a top-level folder this device
//   has a folder for (see [`packOffered`](./packFolder)), and not on one the
//   Library does not hold yet, which has nothing to pack.
// - **Rename…** and **Move to…**, on a folder or a file the Library holds
//   (see [`moving`](./moving)) — not on a folder prepared with "New folder…",
//   which has nothing in the Library to give a new path.
// - then, set apart, **Delete…** — or, on a folder prepared with "New folder…"
//   and not in the Library yet, **Forget this folder**, since there is nothing
//   of it in the Library to delete.
//
// Several rows selected together offer two things: **Move to…**, which moves
// every one of them the Library holds in one move, and `Delete <n> items…`,
// which names every row the Library holds in one deletion: a Pack holding
// several of them is rebuilt once rather than once per row (see
// [`deleteEntries`](./deleteEntries)).
//
// Rows that offer nothing at all — a selection of folders prepared and not in
// the Library yet, a file only added here — still open the menu, holding one
// item that cannot be chosen, `Nothing here to delete`: the right-click takes
// the place of the browser's own menu, so a click that opened nothing would go
// unanswered.
//
// No key deletes. Which key means delete differs between systems, and a key
// that takes things out of a Library is one pressed by mistake.

import type { Appearance, DeleteTarget, Listing } from '@coffret/api';

import { deleteItems, deleteOffered, DELETE } from './deleteEntries';
import { opensInReader, type Described } from './details';
import { movedOf, MOVE_TO, RENAME } from './moving';
import { packOffered, PACK_THIS_FOLDER } from './packFolder';
import { FORGET_FOLDER, NEW_FOLDER } from './prospective';

/** What an item of the menu does, for the screen to carry out. */
export type Action =
  | { kind: 'open'; row: Described }
  | { kind: 'newFolder'; folder: string }
  | { kind: 'pack'; folder: string }
  | { kind: 'delete'; target: DeleteTarget }
  | { kind: 'forget'; folder: string }
  | { kind: 'rename'; row: Described }
  /** The rows a move gives a new path (see [`movedOf`](./moving)). */
  | { kind: 'moveTo'; rows: Described[] }
  // The header's **+**, whose menu is the same popover: the operating
  // system's picker for files, or for one folder.
  | { kind: 'addFiles' }
  | { kind: 'addFolder' }
  // The header's appearance switch, whose menu is the same popover too: the
  // set the explorer is drawn in.
  | { kind: 'appearance'; appearance: Appearance };

/** What the one item of a menu over rows that offer nothing says. */
export const NOTHING_TO_DELETE = 'Nothing here to delete';

/** One item of the menu. */
export interface ActionItem {
  label: string;
  /** What choosing it does, and `null` for an item drawn disabled, which cannot be chosen. */
  action: Action | null;
  /** Drawn in red, and shaded in a tint of it: it takes something away. */
  destructive: boolean;
  /** Set apart from the items above it by a line. */
  separated: boolean;
  /**
   * One of a set of which one is chosen, and whether it is that one: drawn
   * with a check mark where it is, and read out as checked. Left out of an
   * item that is an action rather than a choice.
   */
  checked?: boolean;
}

/** What the menu needs to know beyond the rows themselves. */
export interface ActionContext {
  /** A folder's own listing, where it is on hand; it says whether the reader can show the folder. */
  listingOf: (folder: string) => Listing | null;
  /** The paths prepared with "New folder…" on this page, which are the page's to forget. */
  prepared: readonly string[];
}

/**
 * The items the menu offers for `rows`: one row's, or several rows' together —
 * and for rows that offer nothing, the one disabled `Nothing here to delete`.
 * Empty only where there are no rows.
 */
export function actionsFor(rows: readonly Described[], context: ActionContext): ActionItem[] {
  if (rows.length === 0) {
    return [];
  }
  const items = offeredFor(rows, context);
  return items.length > 0 ? items : [disabled(NOTHING_TO_DELETE)];
}

/** What `rows` offer, which may be nothing. */
function offeredFor(rows: readonly Described[], context: ActionContext): ActionItem[] {
  if (rows.length > 1) {
    const items: ActionItem[] = [];
    const moved = movedOf(rows);
    if (moved.length > 0) {
      items.push(plain(MOVE_TO, { kind: 'moveTo', rows: moved }));
    }
    const target = deleteTargetOf(rows);
    if (target !== null) {
      items.push(destructive(deleteItems(target), { kind: 'delete', target }, items.length > 0));
    }
    return items;
  }
  const row = rows[0];
  const items: ActionItem[] = [];
  if (opensInReader(row, row.kind === 'folder' ? context.listingOf(row.path) : null)) {
    items.push(plain('Open', { kind: 'open', row }));
  }
  if (row.kind === 'folder' && row.folder.mapped) {
    items.push(plain(NEW_FOLDER, { kind: 'newFolder', folder: row.path }));
  }
  if (row.kind === 'folder' && !row.prospective && packOffered({ path: row.path, mapped: row.folder.mapped })) {
    items.push(plain(PACK_THIS_FOLDER, { kind: 'pack', folder: row.path }));
  }
  const moved = movedOf([row]);
  if (moved.length > 0) {
    items.push(plain(RENAME, { kind: 'rename', row }));
    items.push(plain(MOVE_TO, { kind: 'moveTo', rows: moved }));
  }
  const apart = items.length > 0;
  if (row.kind === 'folder' && row.prospective) {
    // Only a path this page prepared, which is the page's to let go of; an
    // empty mapped top-level folder is the mapping's. Not drawn as
    // destructive: nothing is sent, and preparing it again undoes it.
    if (context.prepared.includes(row.path)) {
      items.push({ ...plain(FORGET_FOLDER, { kind: 'forget', folder: row.path }), separated: apart });
    }
    return items;
  }
  const target = deleteTargetOf(rows);
  if (target !== null) {
    items.push(destructive(DELETE, { kind: 'delete', target }, apart));
  }
  return items;
}

/**
 * The one deletion that takes `rows` out of the Library, or `null` where none
 * of them is in it.
 *
 * Every folder the Library holds and every file it holds, in the order the
 * rows stand. A folder the Library does not hold yet — prepared, or the folder
 * of a book whose packing has not committed — and a file only added here have
 * nothing in the Library to take out, and are left out of it. A row under a
 * selected folder is named all the same: the Library takes the union, and the
 * count the menu says is then the rows selected.
 */
export function deleteTargetOf(rows: readonly Described[]): DeleteTarget | null {
  const folders = rows
    .filter((row) => row.kind === 'folder' && !row.prospective && !row.stranded)
    .map((row) => row.path);
  const paths = rows
    .filter((row) => row.kind === 'file' && deleteOffered(row.file))
    .map((row) => row.path);
  return folders.length + paths.length === 0 ? null : { folders, paths };
}

function plain(label: string, action: Action): ActionItem {
  return { label, action, destructive: false, separated: false };
}

function destructive(label: string, action: Action, separated: boolean): ActionItem {
  return { label, action, destructive: true, separated };
}

function disabled(label: string): ActionItem {
  return { label, action: null, destructive: false, separated: false };
}
