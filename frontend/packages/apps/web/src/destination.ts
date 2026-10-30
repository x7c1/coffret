// Where a drop lands, as the confirmation says it before anything is sent.
//
// A drop lands in the folder it was let go over — the one the list is showing,
// or the one a tree row names — and a dropped folder lands under its own name
// inside it. The confirmation names that place as the Library will hold it: the
// full Entry Path of the folder the files go under, so that `Vol 3` dropped
// onto `comics/Foo` reads `comics/Foo/Vol 3/` rather than leaving somebody to
// work out which `Vol 3` they are about to make.
//
// A Library has no empty folders (spec: EP-2): a folder is there because
// something is under it. So a drop is also how a folder comes to be, and the
// levels of the path the Library does not name yet are marked as new — except
// one already standing on this device's disk under a mapped folder, which the
// Library does not hold yet either, and which the drop does not make: what is
// in it goes in with the drop.
//
// Kept free of DOM and of React so it is unit testable, as `packChoice.ts`
// beside it is.

import type { Listing } from '@coffret/api';

import type { DropSummary } from './packChoice';
import { parentOf } from './prospective';
import { keyed } from './tree';

/**
 * The folder files are added to, read off one row of the tree: a folder row's
 * own folder, the folder a file row stands in, and the Library root where no
 * row is named.
 *
 * One rule for both ways in. The header's **+** reads it off the focused row,
 * and a drop off the row under the pointer — so files picked with the focus on
 * a page land beside that page, as files dropped onto it do, and a folder
 * focused or dropped onto takes them inside it whether it is open or not.
 */
export function landingOf(key: string | null): string {
  if (key === null) {
    return '';
  }
  const row = keyed(key);
  return row.kind === 'folder' ? row.path : parentOf(row.path);
}

/** How a landing folder is named in a sentence: its path, or the Library for the root. */
export function placeOf(landing: string): string {
  return landing === '' ? 'the Library' : landing;
}

/**
 * Where one level of a drop's destination stands.
 *
 * - `named`: the Library already names the folder.
 * - `on_disk`: the Library does not, but the folder stands on this device's
 *   disk under a mapped folder, so a freeze of it takes the files already
 *   standing there along with the drop.
 * - `new`: neither; the drop is what makes it.
 */
export type LevelState = 'named' | 'on_disk' | 'new';

/** One folder on the way down to where a drop lands. */
export interface Level {
  /** Its own name, the last component of `path`. */
  name: string;
  /** Its Entry Path. */
  path: string;
  /** Whether the Library names it, or the disk holds it, or neither. */
  state: LevelState;
}

/** One place a drop lands, and what lands there. */
export interface Destination {
  /**
   * The dropped folder whose files land here, or `null` for the files dropped
   * loose, which land in the folder the drop was let go over.
   */
  folder: string | null;
  /** The folders from the top of the Library down to this place; none for the root. */
  levels: Level[];
}

/**
 * The levels from the top of the Library down to `path`, each with whether the
 * Library names it and, where it does not, whether it stands on this device's
 * disk. The root is the Library itself and is no level: it is there before
 * anything is in it.
 */
export function levelsOf(
  path: string,
  exists: (path: string) => boolean,
  onDisk: (path: string) => boolean = () => false,
): Level[] {
  if (path === '') {
    return [];
  }
  const names = path.split('/');
  return names.map((name, at) => {
    const prefix = names.slice(0, at + 1).join('/');
    const state: LevelState = exists(prefix) ? 'named' : onDisk(prefix) ? 'on_disk' : 'new';
    return { name, path: prefix, state };
  });
}

/**
 * Whether the Library names a folder, as a drop onto the folder `where` lists
 * reads it: `known` is the tree's answer, every folder the Library names, or
 * `null` while the tree has not answered or could not.
 *
 * The listing of the folder let go over answers on its own for that folder and
 * every one above it where the Library holds it, and for the folders in it.
 * The last is what the confirmation's "already exists here" is read from, so
 * the destination cannot call new a folder the same confirmation says is there.
 */
export function namedBy(
  known: readonly string[] | null,
  where: Listing,
): (path: string) => boolean {
  return (path) =>
    (known?.includes(path) ?? false) ||
    (where.held && (path === where.path || where.path.startsWith(`${path}/`))) ||
    where.folders.some((folder) => folder.path === path);
}

/**
 * Whether a folder stands on this device's disk without the Library holding
 * it, as the listing `where` of the folder let go over reads it: its
 * `folders_on_disk` names the folders directly in it, so it answers for those
 * and for nothing deeper or above.
 *
 * Compared in NFC, the form the page holds every path in (spec: EP-1): a name
 * read off a disk may come in another normalization.
 */
export function onDiskIn(where: Listing): (path: string) => boolean {
  return (path) => {
    const name = path.slice(path.lastIndexOf('/') + 1);
    return (
      parentOf(path) === where.path &&
      where.folders_on_disk.some((folder) => folder.normalize('NFC') === name)
    );
  };
}

/**
 * Where each part of a drop onto `landing` lands: one place per dropped folder,
 * under its own name, in the order the drop named them — and the landing folder
 * itself where files were dropped loose, or where no folder was dropped at all.
 *
 * `exists` says whether the Library names a folder, the root aside, and
 * `onDisk` whether one it does not name stands on this device's disk.
 */
export function destinationsOf(
  landing: string,
  summary: DropSummary,
  exists: (path: string) => boolean,
  onDisk: (path: string) => boolean = () => false,
): Destination[] {
  const into: Destination[] = summary.folders.map((folder) => ({
    folder: folder.name,
    levels: levelsOf(landing === '' ? folder.name : `${landing}/${folder.name}`, exists, onDisk),
  }));
  if (summary.loose > 0 || summary.folders.length === 0) {
    into.push({ folder: null, levels: levelsOf(landing, exists, onDisk) });
  }
  return into;
}

/** What the Library root is called where it is the place a drop lands. */
export const LIBRARY_ROOT = 'the top of the Library';

/**
 * The place as the Library will hold it: the folder's Entry Path with a
 * trailing separator, since what lands is under it — `comics/Foo/Vol 3/` — and
 * the root, which has no path, by name.
 */
export function pathLine(levels: readonly Level[]): string {
  if (levels.length === 0) {
    return LIBRARY_ROOT;
  }
  return `${levels.map((level) => level.name).join('/')}/`;
}

/**
 * What the confirmation says of the levels the Library does not name yet, or
 * `null` where every one of them is there already.
 */
export function newLine(levels: readonly Level[]): string | null {
  const made = namesIn(levels, 'new');
  if (made.length === 0) {
    return null;
  }
  const named = listed(made);
  return made.length === 1
    ? `${named} is new — the Library has no such folder yet, and adding these files makes it`
    : `${named} are new — the Library has no such folders yet, and adding these files makes them`;
}

/**
 * What the confirmation says of the levels standing on this device's disk that
 * the Library does not hold yet, or `null` where there are none: the drop does
 * not make them, and what is already in them goes into the Library with it.
 */
export function onDiskLine(levels: readonly Level[]): string | null {
  const standing = namesIn(levels, 'on_disk');
  if (standing.length === 0) {
    return null;
  }
  const named = listed(standing);
  return standing.length === 1
    ? `${named} is on this device’s disk and not in the Library yet — it is added ` +
        'together with what is already in it'
    : `${named} are on this device’s disk and not in the Library yet — they are added ` +
        'together with what is already in them';
}

/** The names of the levels in `state`, top down. */
function namesIn(levels: readonly Level[], state: LevelState): string[] {
  return levels.filter((level) => level.state === state).map((level) => level.name);
}

/** Names said as one list: `a`, `a and b`, `a, b and c`. */
function listed(names: readonly string[]): string {
  return names.length === 1 ? names[0] : `${names.slice(0, -1).join(', ')} and ${names.at(-1)}`;
}
