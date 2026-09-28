// A folder made in the browser, before the Library has one. Kept free of DOM so
// it is unit testable.
//
// A Library has no folders to make. What a folder is, is the separators in the
// Entry Paths under it (spec: EP-2), so a folder with nothing in it is not
// something the Library can be asked to hold — there is no object to create, no
// row to commit, and nothing for the server to be told. Which means a folder
// somebody makes in the explorer is a place on this screen and nothing more,
// until the first Entry under it commits and `GET /api/folders` starts naming it
// for itself.
//
// That is what this is: the few rules for keeping such a place on the screen —
// what a name may be, where it stands among the folders the server answered
// with, and when it stops being this screen's business because the Library has
// taken it over.
//
// It is deliberately not persisted. A folder nobody ever dropped anything into
// is a gesture that came to nothing, and a reload is where it goes: writing it
// down would leave the tree carrying places that will never exist, with nothing
// to ever clear them.
//
// One kind comes back all the same, and not from anything written down here:
// `strandedFolders` reads the folders of the freezes that have not committed
// back out of the server's own answer. That is the whole of the rule — a place
// the server still has something to say about comes back, and a place nothing
// ever happened in does not.

import type { Freeze, Listing } from '@coffret/api';

import { said } from './useAsked';

/**
 * What is wrong with a name for a new folder, and `null` where nothing is.
 *
 * One component of an Entry Path and nothing else (spec: EP-2). A name carrying
 * a separator is a person asking for two folders at once, and the two relative
 * references are not names at all — the server refuses every one of these, and
 * saying so here means it is said before anything is on the screen rather than
 * after a drop onto a place no path could name.
 */
export function nameDefect(name: string): string | null {
  if (name === '') {
    return 'a folder needs a name';
  }
  if (name.includes('/')) {
    return 'a folder name cannot hold a “/” — make one folder at a time';
  }
  if (name.includes('\0')) {
    return 'a folder name cannot hold a NUL';
  }
  if (name === '.' || name === '..') {
    return '“.” and “..” are not names';
  }
  return null;
}

/** Everything making one folder reaches out to. */
export interface Making {
  /** The folder the new one goes under; the empty string is the Library root. */
  parent: string;
  /** What was typed for its name, as typed. */
  typed: string;
  /** The folders the Library names, and `null` before the tree has answered. */
  known: readonly string[] | null;
  /** The folders made here that the Library does not have yet. */
  pending: readonly string[];
  /** Asks what a path holds, which is [`getListing`](@coffret/api). */
  list: (path: string) => Promise<Listing>;
  /** Says why no folder was made. */
  notice: (line: string) => void;
  /** Makes the folder on this screen and walks into it. */
  make: (path: string) => void;
}

/**
 * Makes a folder, unless the name is no name or the place is already taken.
 *
 * Taken three ways. The Library names a folder there; this screen made one
 * there already; or a mapped folder holds one there on disk that no run has
 * carried in. The first two are on the screen, but the third is not: the tree
 * is the catalog's answer, and the catalog has never heard of files nothing has
 * committed. A folder made over one of those would be a pending folder, the
 * first drop into it would be a book being brought in, and the freeze behind
 * that drop takes every file under the folder — the ones that were already
 * standing there going into the book's Packs with nothing on the screen having
 * said they were there.
 *
 * So the path is listed before anything is made. The listing answers for a
 * path the Library does not hold, with the files standing in the mapped folder
 * there as `added` rows and the folders standing there as `folders_on_disk`,
 * and a listing with anything in it is a place somebody already has something
 * in. The folders matter as much as the files: a volume kept as chapter
 * folders has no file of its own one level down, and the freeze behind a drop
 * into it would take every chapter. What is asked is only whether there is:
 * the window between this answer and the drop stays open, and closing it is
 * not the point — the point is not surprising the one person at this screen.
 *
 * A listing that could not be had is a refusal too. Nothing is known about the
 * place then, and a folder made over what might be somebody's files is the one
 * outcome this is here to prevent; the name can be asked for again.
 *
 * It never rejects. What stops a folder being made is a sentence in the notice
 * area.
 */
export async function askToMake(making: Making): Promise<void> {
  const name = making.typed.trim();
  const defect = nameDefect(name);
  if (defect !== null) {
    making.notice(`no folder was made — ${defect}`);
    return;
  }
  const path = folderUnder(making.parent, name);
  if (making.known?.includes(path) === true || isPending(making.pending, path)) {
    making.notice(`there is already a folder called ${name} here`);
    return;
  }
  let listing: Listing;
  try {
    listing = await making.list(path);
  } catch (refused: unknown) {
    making.notice(`no folder was made — whether ${name} is already on disk could not be asked: ${said(refused)}`);
    return;
  }
  if (
    listing.files.length > 0 ||
    listing.folders.length > 0 ||
    listing.folders_on_disk.length > 0
  ) {
    making.notice(
      `no folder was made — a folder called ${name} is already in the mapped folder, with files the Library does not hold yet`,
    );
    return;
  }
  making.make(path);
}

/**
 * Where a folder called `name` under `parent` stands in the Library.
 *
 * The parent is an Entry Path already — it came from the tree, which came from
 * the server — but the name has just been typed, and text arriving from outside
 * the Library is put into NFC on the way in (spec: EP-1). The server does that
 * to it too, which is exactly why it is done here: the composed path is what
 * this screen keeps its pending folder under and what it later compares against
 * the paths the server answers with, and equality there is byte-exact over the
 * canonical form (spec: EP-3). A name left in some other normalization would
 * name the same place and match none of them — the folder would never be let go
 * of, and would stand in the tree a second time beside the one the Library
 * names.
 *
 * The empty parent is the Library root, which is not a path and contributes no
 * separator.
 */
export function folderUnder(parent: string, name: string): string {
  const composed = parent === '' ? name : `${parent}/${name}`;
  return composed.normalize('NFC');
}

/**
 * The folders to draw: the ones the Library has, and the ones made here that it
 * has not.
 *
 * Merged in the Library's own order rather than appended, because the order is
 * the byte order of the canonical paths and it is the one order every device
 * agrees on: a new folder that sat at the end of the tree would be in a
 * different place from the one it takes the moment its first Entry commits, and
 * a row that jumps when the freeze lands is a row a person loses.
 *
 * A pending folder the Library already names contributes nothing: the same path
 * twice would be two rows for one place.
 */
export function foldersWith(
  folders: readonly string[],
  pending: readonly string[],
): string[] {
  const held = new Set(folders);
  const added = pending.filter((path) => !held.has(path));
  if (added.length === 0) {
    return [...folders];
  }
  return [...folders, ...added].sort(inLibraryOrder);
}

/**
 * The pending folders that are still this screen's business.
 *
 * One leaves the moment the Library names it, which is what a committed Entry
 * under it does: from then on it is an ordinary folder answered for by the
 * server, and keeping it here as well would be this screen holding a second
 * opinion about a place that now exists.
 *
 * One that was abandoned — made, and never dropped into — stays until the tab
 * does. There is nothing to clear it against: the Library will never name it,
 * and a folder that vanished from under somebody who was about to drop a book
 * into it would be worse than one that outstays its usefulness.
 */
export function pendingAfter(
  pending: readonly string[],
  folders: readonly string[],
): string[] {
  const held = new Set(folders);
  return pending.filter((path) => !held.has(path));
}

/**
 * The folders the uncommitted freezes hold, oldest first.
 *
 * The other way into this lifecycle, and the only one a reload survives. A book
 * whose freeze has not committed — stopped by Storage, waiting its turn, thrown
 * away by a worker that died, still packing when the tab went away, or finished
 * with every file it read a finding and nothing committed — is a folder full
 * of pages sitting on the disk and out of the Library, and the folder itself
 * was never anything but this screen's — so a tab that came back would draw
 * no row for it, offer no way to walk into it, and make no second attempt at
 * it. The pages would be there and nothing on the screen would say
 * so — and forgotten pages dropped into a re-made folder would be synced one
 * Container apiece instead of refused while the pack runs.
 *
 * Nothing was remembered to get them back. The server is still holding the
 * freezes, so every folder is named in the answer to `GET /api/work`, and
 * these are those names read back out. In the tab that never went away this is
 * a no-op: the folders are pending there already.
 *
 * Every list the answer carries and not the running freeze alone, because the
 * server queues what it is asked for rather than refusing it: a book dropped
 * into a folder made while another is packing sits in `waiting` with nothing
 * else on the screen naming it, a worker that died moves it to `discarded`, and a
 * book Storage stopped moves to `displaced` the moment the next one is taken off
 * the queue — which two books in one session is enough to reach. Each is a state
 * a reload can land in, and a folder missing from the tree in any of them is a
 * folder nobody can walk into: the status bar reaches a discarded or a stopped one
 * by name, and a waiting one only becomes visible when its turn comes.
 *
 * A folder the Library names is not one of these. Its first Entry committed, so
 * it is an ordinary folder the server answers for; taking it back would draw it
 * twice and would make the next drop into it a book being imported rather than
 * the files being added that it is. Neither is the Library root, which is not a
 * folder anybody made (spec: EP-2).
 */
export function strandedFolders(
  freeze: Freeze | null,
  folders: readonly string[],
): string[] {
  if (freeze === null) {
    return [];
  }
  // The one on record first, then the books it took the record from, then the
  // queue behind it, then what a worker that died left — the order the server
  // names them in.
  //
  // A freeze that finished and committed a Pack contributes no folder of its
  // own: the Library names the folder and it is an ordinary folder from here
  // on. One that finished having committed nothing — every file it read ended
  // as a finding — is not that: its pages are on the disk and out of the
  // Library, the same as a stopped one's, so it is held on the same terms. The
  // filter below lets go of the folder the moment the Library names it, so a
  // finished freeze whose folder the Library already holds is dropped there
  // whatever it packed. One that stopped contributes its folder like one still
  // packing does — the pages are sitting on the disk and out of the Library,
  // and the folder stands here until somebody packs it again, which is what
  // the status bar's "pack again" needs a place to walk into for. What is
  // queued behind either of them is another matter and does not end with the
  // run on record, which is why the queues are read whatever it says.
  //
  // A run the next one took the record from is one that stopped, so its folder
  // is held on the same terms the one on record is: the pages are sitting on the
  // disk and out of the Library, and the folder stands here until somebody packs
  // it again.
  const held = freeze.status !== 'done' || freeze.packs === 0 ? [freeze.folder] : [];
  const stopped = freeze.displaced.map((run) => run.folder);
  const named = [...held, ...stopped, ...freeze.waiting, ...freeze.discarded];
  return named.filter(
    (folder, at) =>
      folder !== '' && !folders.includes(folder) && named.indexOf(folder) === at,
  );
}

/**
 * Whether this folder is one made here that the Library does not have yet.
 *
 * What the drop reads to know which gesture it is: files dropped onto such a
 * folder are a book being brought in, and are frozen rather than synced. Files
 * dropped onto any other folder are files being added to a folder that already
 * exists, and nothing about that changes.
 */
export function isPending(pending: readonly string[], folder: string): boolean {
  return pending.includes(folder);
}

/**
 * Two paths in the order the Library answers in.
 *
 * The byte order of the canonical paths, with no case folding and no locale.
 * UTF-8 sorts by code point, so comparing code points is comparing bytes —
 * which `<` on two JavaScript strings is not, because it compares UTF-16 code
 * units and puts every astral character before `U+E000`–`U+FFFF` rather than
 * after them. `localeCompare` would be a third order again.
 */
function inLibraryOrder(left: string, right: string): number {
  const one = [...left];
  const other = [...right];
  for (let at = 0; at < Math.min(one.length, other.length); at += 1) {
    const difference = (one[at].codePointAt(0) ?? 0) - (other[at].codePointAt(0) ?? 0);
    if (difference !== 0) {
      return difference;
    }
  }
  return one.length - other.length;
}
