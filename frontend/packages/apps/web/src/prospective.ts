// The prospective folders: the folders the explorer draws that the Library does
// not hold yet, and offers as places to drop the first files into. Kept free of
// DOM so it is unit testable.
//
// A Library has no empty folders: a folder is the separators in the Entry Paths
// under it (spec: EP-2). So there is no folder to make before something is put
// in it, and two places a person can stand in and drop into are not folders of
// the Library at all:
//
// - A **prepared** path: one a person named with "New folder…", held by this
//   page alone. Nothing is sent and nothing is made on disk; it is where the
//   next drop is let go over, so that a series of hundreds of volumes can be
//   brought in a few at a time under one parent. A reload forgets it, and
//   preparing it again is the remedy: a place nothing was ever dropped into is
//   a gesture that came to nothing, and writing it down would leave the tree
//   carrying places that will never exist.
// - An **empty mapped prefix**: a top-level folder this device maps that holds
//   no Entry yet. The server names those in the root's listing
//   (`empty_mapped_prefixes`), because a mapping is this device's state and
//   not the page's.
//
// Both are drawn with the same mark, and both stop being this page's business
// the moment the Library names them: a drop's files land, a run carries them
// in, and from then on the folder is an ordinary one the server answers for.
//
// Every drop into one is confirmed like any other (see `DropConfirm`), and the
// confirmation marks as new the levels the Library does not have — which these
// are, since nothing here tells the destination they exist.

import {
  componentDefect,
  reservedName,
  type ComponentDefect,
  type ListedFolder,
  type Listing,
  type ReservedName,
} from '@coffret/api';

import { inLibraryOrder } from './stranded';
import { said } from './useAsked';

/** What a folder the Library does not hold yet is marked with, wherever it is drawn. */
export const PROSPECTIVE = 'not in the Library yet — drop files here to add them';

/** The menu item that prepares a path inside a folder. */
export const NEW_FOLDER = 'New folder…';

/** The menu item that lets go of a prepared path, and of every one prepared under it. */
export const FORGET_FOLDER = 'Forget this folder';

/**
 * Where a folder called `name` under `parent` stands in the Library.
 *
 * Put into NFC on the way in, as the server puts every path from outside the
 * Library (spec: EP-1): the composed path is what the page holds the prepared
 * path under and what it later compares with the paths the server answers
 * with, and equality there is byte-exact over the canonical form (spec: EP-3).
 * A name in another normalization would never be let go of, and would stand
 * beside the folder the Library names.
 *
 * The empty parent is the Library root, which contributes no separator.
 */
export function folderUnder(parent: string, name: string): string {
  const composed = parent === '' ? name : `${parent}/${name}`;
  return composed.normalize('NFC');
}

/** The folder `path` stands in; the empty string is the Library root. */
export function parentOf(path: string): string {
  const cut = path.lastIndexOf('/');
  return cut === -1 ? '' : path.slice(0, cut);
}

/** What a name the server would refuse as a component is said as. */
const SHAPE: Record<ComponentDefect, string> = {
  empty: 'a folder needs a name',
  separator: 'a folder name cannot hold a “/” — prepare one folder at a time',
  nul: 'a folder name cannot hold a NUL',
  relative: '“.” and “..” are not names',
};

/** What a name coffret keeps for itself is said as. */
const RESERVED: Record<ReservedName, string> = {
  scratch: 'names starting “.coffret-fetch-” are coffret’s own, for files half written',
  management_area: '“.coffret” is the folder coffret keeps its own records in',
  folds_to_management_area:
    'that name differs from “.coffret”, the folder coffret keeps its own records in, only by case',
};

/**
 * What is wrong with a typed name for a folder, or `null` where nothing is.
 *
 * The rules are the server's (see [`componentDefect`](@coffret/api) and
 * [`reservedName`](@coffret/api)): one component of an Entry Path and not a name
 * coffret keeps for itself. Said here so that a name no drop could land under
 * is refused before a row is drawn for it rather than after the drop.
 *
 * What was typed is trimmed first: a name somebody typed with a space at either
 * end is a name they did not mean to type that way.
 */
export function nameDefect(typed: string): string | null {
  const name = typed.trim();
  const shape = componentDefect(name);
  if (shape !== null) {
    return SHAPE[shape];
  }
  const reserved = reservedName(name);
  return reserved === null ? null : RESERVED[reserved];
}

/**
 * Whether the folder `parent` lists already holds something called `name`: a
 * folder of the Library, a file, a folder standing on disk that the Library
 * does not hold, or a folder this page already draws as not held yet.
 *
 * A folder prepared over a name already taken would be drawn as new and empty
 * over a place that holds somebody's files, or beside the folder that is there.
 */
export function takenIn(name: string, parent: Listing, prospective: readonly string[]): boolean {
  const path = folderUnder(parent.path, name.trim());
  const named = path.slice(path.lastIndexOf('/') + 1);
  return (
    parent.folders.some((folder) => folder.path === path) ||
    parent.files.some((file) => file.path === path) ||
    parent.folders_on_disk.some((folder) => folder.normalize('NFC') === named) ||
    parent.empty_mapped_prefixes.includes(named) ||
    prospective.includes(path)
  );
}

/** What preparing a folder came to: the path to draw, or why nothing was prepared. */
export type Prepared = { path: string } | { refused: string };

/**
 * Prepares `typed` under `parent`, unless the name is no name or the place is
 * taken.
 *
 * The name is judged before anything is asked; then the parent is listed,
 * since whether it holds the name is the listing's answer — the tree knows the
 * Library's folders and not the files or the folders on disk. A listing that
 * could not be had is a refusal too: nothing is known about the place then.
 *
 * It never rejects.
 */
export async function prepare({
  parent,
  typed,
  prospective,
  list,
}: {
  parent: string;
  typed: string;
  /** The folders this page already draws as not held yet. */
  prospective: readonly string[];
  /** Asks what a folder holds, which is [`getListing`](@coffret/api). */
  list: (path: string) => Promise<Listing>;
}): Promise<Prepared> {
  const defect = nameDefect(typed);
  if (defect !== null) {
    return { refused: defect };
  }
  const name = typed.trim();
  let listing: Listing;
  try {
    listing = await list(parent);
  } catch (refused: unknown) {
    return { refused: `whether the folder already holds ${name} could not be asked: ${said(refused)}` };
  }
  if (takenIn(name, listing, prospective)) {
    return { refused: `there is already something called ${name} here` };
  }
  return { path: folderUnder(parent, name) };
}

/**
 * The prepared paths still this page's business: every one but those the
 * Library now names, which `named` says.
 *
 * `prepared` comes back as it was where nothing left it, so a caller keeping it
 * in state renders nothing for an answer that moved nothing.
 */
export function preparedAfter(
  prepared: readonly string[],
  named: (path: string) => boolean,
): readonly string[] {
  const kept = prepared.filter((path) => !named(path));
  return kept.length === prepared.length ? prepared : kept;
}

/**
 * The prepared paths left once `path` is forgotten: every one but `path` and
 * those prepared under it.
 *
 * A path mistyped into "New folder…" would otherwise stand in the tree and the
 * list until a reload, and a reload forgets every other prepared path with it.
 * Forgetting is the page's alone, as preparing is: nothing is sent and nothing
 * on disk changes. The paths under it go too, since a row drawn under a parent
 * that is no longer drawn would be somewhere nobody can walk to.
 *
 * `prepared` comes back as it was where nothing left it, as in
 * {@link preparedAfter}.
 */
export function forgotten(prepared: readonly string[], path: string): readonly string[] {
  const kept = prepared.filter((one) => one !== path && !one.startsWith(`${path}/`));
  return kept.length === prepared.length ? prepared : kept;
}

/**
 * Every folder drawn as not held yet: the prepared paths and the empty mapped
 * prefixes the root's listing names, in the Library's order, each once — less
 * any the tree's answer `known` names, which the Library holds now.
 *
 * The root's listing is asked for once wherever the screen is not at the root,
 * and a drop into an empty mapped prefix commits without its being asked
 * again; the tree's answer is asked again after every run, so it is the one
 * that says when such a folder stopped being empty.
 */
export function prospectiveFolders(
  prepared: readonly string[],
  root: Listing | null,
  known: readonly string[] | null,
): readonly string[] {
  const mapped = root === null || root.path !== '' ? [] : root.empty_mapped_prefixes;
  const all = [...new Set([...mapped, ...prepared])].filter(
    (path) => known === null || !known.includes(path),
  );
  return all.sort(inLibraryOrder);
}

/**
 * The folders not held yet that stand directly in the folder `listing` lists,
 * as rows of it — less any the listing names already, which is the Library
 * holding it now.
 *
 * Each is reached where the folder is, and at the root where a mapping of its
 * own stands for it: a prepared path is only offered where a mapping reaches.
 */
export function prospectiveIn(listing: Listing, prospective: readonly string[]): ListedFolder[] {
  return prospective
    .filter(
      (path) =>
        parentOf(path) === listing.path &&
        !listing.folders.some((folder) => folder.path === path),
    )
    .map((path) => {
      const name = path.slice(path.lastIndexOf('/') + 1);
      return {
        name,
        path,
        mapped: listing.mapped || (listing.path === '' && listing.empty_mapped_prefixes.includes(name)),
        // The Library holds nothing under it, so nothing of it is here to read.
        present_throughout: false,
        files_total: 0,
        files_present: 0,
        size: 0,
        mtime: null,
      };
    });
}
