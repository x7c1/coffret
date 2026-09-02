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
// `strandedFolder` reads the folder of a freeze that has not committed back out
// of the server's own answer. That is the whole of the rule — a place the server
// still has something to say about comes back, and a place nothing ever happened
// in does not.

import type { Freeze } from '@coffret/api';

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
 * The folder an uncommitted freeze holds, and `null` where there is none.
 *
 * The other way into this lifecycle, and the only one a reload survives. A book
 * whose freeze has not committed — stopped by Storage, or still packing when
 * the tab went away — is a folder full of pages sitting on the disk and out of
 * the Library, and the folder itself was never anything but this screen's — so
 * a tab that came back would draw no row for it, offer no way to walk into it,
 * and make no second attempt at it. The pages would be there and nothing on the
 * screen would say so — and forgotten pages dropped into a re-made folder would
 * be synced one Container apiece instead of refused while the pack runs.
 *
 * Nothing was remembered to get it back. The server is still holding the
 * freeze, so the folder is named in the answer to `GET /api/activity`, and this
 * is that name read back out. In the tab that never went away this is a no-op:
 * the folder is pending there already.
 *
 * A folder the Library names is not one of these. Its first Entry committed, so
 * it is an ordinary folder the server answers for; taking it back would draw it
 * twice and would make the next drop into it a book being imported rather than
 * the files being added that it is. Neither is the Library root, which is not a
 * folder anybody made (spec: EP-2).
 */
export function strandedFolder(
  freeze: Freeze | null,
  folders: readonly string[],
): string | null {
  if (freeze === null || freeze.status === 'done' || freeze.folder === '') {
    return null;
  }
  return folders.includes(freeze.folder) ? null : freeze.folder;
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
