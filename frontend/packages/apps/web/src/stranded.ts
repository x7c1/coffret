// The folders of books whose freeze has not committed, which the tree draws
// although the Library does not name them yet. Kept free of DOM so it is unit
// testable.
//
// A Library has no empty folders: what a folder is, is the separators in the
// Entry Paths under it (spec: EP-2). So a book dropped as a Pack is a folder
// full of pages on the disk and out of the Library until its freeze commits,
// and `GET /api/folders` does not name it until then. The tree draws it all
// the same, dimmed, because the pages are there and a person has to be able to
// walk into them — to watch them pack, or to pack them again where Storage
// stopped the first attempt.
//
// Nothing is written down to get them back after a reload: the server is still
// holding the freezes, and every folder is named in its answer to
// `GET /api/work`. `strandedFolders` reads them back out of that answer, and
// `heldAfter` keeps one on the screen until the Library names it, so that a
// folder does not leave the tree between its freeze committing and the tree's
// next answer arriving.

import type { Freeze } from '@coffret/api';

/**
 * The folders to draw: the ones the Library has, and the stranded ones it has
 * not.
 *
 * Merged in the Library's own order rather than appended, because the order is
 * the byte order of the canonical paths and it is the one order every device
 * agrees on: a folder that sat at the end of the tree would be in a different
 * place from the one it takes the moment its first Entry commits, and a row
 * that jumps when the freeze lands is a row a person loses.
 *
 * A stranded folder the Library already names contributes nothing: the same
 * path twice would be two rows for one place.
 */
export function foldersWith(
  folders: readonly string[],
  stranded: readonly string[],
): string[] {
  const held = new Set(folders);
  const added = stranded.filter((path) => !held.has(path));
  if (added.length === 0) {
    return [...folders];
  }
  return [...folders, ...added].sort(inLibraryOrder);
}

/**
 * The stranded folders the screen goes on drawing: the ones it drew already
 * and the ones the latest answer names, less every one the Library names now.
 *
 * One leaves the moment the Library names it, which is what a committed Entry
 * under it does: from then on it is an ordinary folder answered for by the
 * server. Until then it stays, even where the freeze that named it has
 * committed — the tree is asked again after a commit, and a folder that
 * vanished in the moment between would be a row gone from under the pointer.
 *
 * One leaves as well where its freeze is one a person stopped (`stopped`):
 * what a drop wrote there was taken away again (spec: LA-36), so there is
 * nothing left on the disk for the row to stand for.
 *
 * `held` comes back as it was where nothing changed, so a caller keeping it in
 * state renders nothing for an answer that moved nothing.
 */
export function heldAfter(
  held: readonly string[],
  stranded: readonly string[],
  folders: readonly string[],
  stopped: readonly string[] = [],
): readonly string[] {
  const named = new Set([...folders, ...stopped]);
  const kept = held.filter((path) => !named.has(path));
  const back = stranded.filter((path) => !named.has(path) && !kept.includes(path));
  if (back.length === 0 && kept.length === held.length) {
    return held;
  }
  return [...kept, ...back];
}

/**
 * The folders the uncommitted freezes hold, oldest first.
 *
 * A book whose freeze has not committed — stopped by Storage, waiting its
 * turn, thrown away by a worker that died, still packing when the tab went
 * away, or finished with every file it read a finding and nothing committed —
 * is a folder full of pages sitting on the disk and out of the Library, which
 * the Library does not name. Without these a tab would draw no row for it,
 * offer no way to walk into it, and make no second attempt at it. The pages
 * would be there and nothing on the screen would say so.
 *
 * Every list the answer carries and not the running freeze alone, because the
 * server queues what it is asked for rather than refusing it: a folder added as
 * a Pack while another is packing sits in `waiting` with nothing
 * else on the screen naming it, a worker that died moves it to `discarded`, and a
 * book Storage stopped moves to `displaced` the moment the next one is taken off
 * the queue — which two books in one session is enough to reach. Each is a state
 * a reload can land in, and a folder missing from the tree in any of them is a
 * folder nobody can walk into: the Activity panel offers a discarded or a stopped
 * one again by name, and a waiting one only becomes visible when its turn comes.
 *
 * A folder the Library names is not one of these. Its first Entry committed, so
 * it is an ordinary folder the server answers for, and taking it back would draw
 * it twice. Neither is the Library root, which is not a folder at all
 * (spec: EP-2).
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
  // the Activity panel's "pack again" needs a place to walk into for. What is
  // queued behind either of them is another matter and does not end with the
  // run on record, which is why the queues are read whatever it says.
  //
  // A run the next one took the record from is one that stopped, so its folder
  // is held on the same terms the one on record is: the pages are sitting on the
  // disk and out of the Library, and the folder stands here until somebody packs
  // it again.
  //
  // One a person stopped holds nothing either: a drop's pages were taken away
  // again (spec: LA-36), and a folder somebody asked to have packed is the
  // Library's own already.
  const held =
    freeze.status === 'cancelled' || (freeze.status === 'done' && freeze.packs > 0)
      ? []
      : [freeze.folder];
  const stopped = freeze.displaced.map((run) => run.folder);
  const named = [...held, ...stopped, ...freeze.waiting, ...freeze.discarded];
  return named.filter(
    (folder, at) =>
      folder !== '' && !folders.includes(folder) && named.indexOf(folder) === at,
  );
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
export function inLibraryOrder(left: string, right: string): number {
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
