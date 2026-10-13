// Which files of the listing are picked out for a deletion together.
//
// A Pack holding several of the files is rebuilt once per deletion that takes
// something out of it, so deleting its pages one at a time rebuilds it once per
// page. Picking the pages first and deleting them in one go rebuilds it once.
//
// Only files are picked: the server takes one folder per deletion, and a folder
// is deleted from its own row. And only files the Library holds, for the reason
// a row offers "Delete…" only on those (see [`deleteOffered`](./deleteEntries)).
//
// A selection belongs to the folder it was made in, so a listing of another
// folder starts with nothing picked whatever paths the two share; and it holds
// only what the listing still offers, so a file that left the folder leaves it.
//
// Kept free of DOM and of React so it is unit testable.

import type { Listing } from '@coffret/api';

import { deleteOffered } from './deleteEntries';

/** The files picked out, and the folder they were picked in. */
export interface Selection {
  folder: string;
  paths: ReadonlySet<string>;
}

/** The files of `listing` that can be picked, in the listing's order. */
export function selectable(listing: Listing): string[] {
  return listing.files.filter(deleteOffered).map((file) => file.path);
}

/**
 * What is picked out in `listing`, in the listing's order — which is the
 * order a deletion names them in, rather than the order they were clicked.
 */
export function chosen(selection: Selection | null, listing: Listing): string[] {
  if (selection === null || selection.folder !== listing.path) {
    return [];
  }
  return selectable(listing).filter((path) => selection.paths.has(path));
}

/**
 * `selection` as it stands against `listing`: emptied where the listing is of
 * another folder, and without the files it no longer offers. The same object
 * where nothing changed, so that holding it in state does not draw again.
 */
export function kept(selection: Selection | null, listing: Listing): Selection | null {
  if (selection === null) {
    return null;
  }
  const still = chosen(selection, listing);
  if (still.length === 0) {
    return null;
  }
  return still.length === selection.paths.size
    ? selection
    : { folder: listing.path, paths: new Set(still) };
}

/** `selection` with the file at `path` picked, or put back where it was. */
export function toggled(selection: Selection | null, listing: Listing, path: string): Selection {
  const paths = new Set(chosen(selection, listing));
  if (paths.has(path)) {
    paths.delete(path);
  } else {
    paths.add(path);
  }
  return { folder: listing.path, paths };
}

/**
 * What the header's box does: picks every file that can be picked, or clears
 * them all where every one already is.
 */
export function everyToggled(selection: Selection | null, listing: Listing): Selection | null {
  const all = selectable(listing);
  if (chosen(selection, listing).length === all.length) {
    return null;
  }
  return { folder: listing.path, paths: new Set(all) };
}

/** The line over the rows while files are picked out. */
export function selectedLine(count: number): string {
  return `${count} selected`;
}
