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
// levels of the path the Library does not name yet are marked as new.
//
// Kept free of DOM and of React so it is unit testable, as `packChoice.ts`
// beside it is.

import type { Listing } from '@coffret/api';

import type { DropSummary } from './packChoice';

/** One folder on the way down to where a drop lands. */
export interface Level {
  /** Its own name, the last component of `path`. */
  name: string;
  /** Its Entry Path. */
  path: string;
  /** Whether the Library already names this folder. */
  exists: boolean;
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
 * Library names it. The root is the Library itself and is no level: it is
 * there before anything is in it.
 */
export function levelsOf(path: string, exists: (path: string) => boolean): Level[] {
  if (path === '') {
    return [];
  }
  const names = path.split('/');
  return names.map((name, at) => {
    const prefix = names.slice(0, at + 1).join('/');
    return { name, path: prefix, exists: exists(prefix) };
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
 * Where each part of a drop onto `landing` lands: one place per dropped folder,
 * under its own name, in the order the drop named them — and the landing folder
 * itself where files were dropped loose, or where no folder was dropped at all.
 *
 * `exists` says whether the Library names a folder, the root aside.
 */
export function destinationsOf(
  landing: string,
  summary: DropSummary,
  exists: (path: string) => boolean,
): Destination[] {
  const into: Destination[] = summary.folders.map((folder) => ({
    folder: folder.name,
    levels: levelsOf(landing === '' ? folder.name : `${landing}/${folder.name}`, exists),
  }));
  if (summary.loose > 0 || summary.folders.length === 0) {
    into.push({ folder: null, levels: levelsOf(landing, exists) });
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
  const made = levels.filter((level) => !level.exists).map((level) => level.name);
  if (made.length === 0) {
    return null;
  }
  const named = made.length === 1 ? made[0] : `${made.slice(0, -1).join(', ')} and ${made.at(-1)}`;
  return made.length === 1
    ? `${named} is new — the Library has no such folder yet, and this drop makes it`
    : `${named} are new — the Library has no such folders yet, and this drop makes them`;
}
