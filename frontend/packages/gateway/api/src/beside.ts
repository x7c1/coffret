import { apiUrl, askedForJson } from './request';

/**
 * The files either side of one row of the Library in reading order — the
 * order the tree draws the whole Library in with every folder open, a
 * folder's folders before its files — each `null` at that end of the
 * Library.
 *
 * Entry Paths, at any depth: the file after the last of one folder is the
 * first of the next. Only the Library's own Entries are in the order; a file
 * no run has carried in yet is not.
 */
export interface Beside {
  previous: string | null;
  next: string | null;
}

/**
 * Asks which files stand either side of the row at `path` —
 * `GET /api/beside?path=[&folder=true]`; `folder` where the row is the
 * folder at that path rather than the file.
 *
 * Out of the catalog alone: it answers for folders this page has never
 * listed, which is what a reader at the end of one folder asks to read on
 * into the next, and sets nothing going.
 */
export function getBeside(path: string, folder: boolean, signal?: AbortSignal): Promise<Beside> {
  return askedForJson<Beside>(
    apiUrl('beside', folder ? { path, folder: 'true' } : { path }),
    signal,
  );
}
