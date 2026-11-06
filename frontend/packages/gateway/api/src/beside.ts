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
 * `GET /api/beside?path=[&folder=true][&under=]`; `folder` where the row is
 * the folder at that path rather than the file.
 *
 * `under`, where given, limits the answer to the files under that folder, at
 * any depth: the nearest of them either side of the row, wherever it stands.
 * The file at the folder's own path stands after everything under it, so
 * asked about with the folder as `under`, it answers with the folder's last
 * file as `previous`.
 *
 * Out of the catalog alone: it answers for folders this page has never
 * listed, which is what a reader at the end of one folder asks to read on
 * into the next, and sets nothing going.
 */
export function getBeside(
  path: string,
  folder: boolean,
  { under, signal }: { under?: string; signal?: AbortSignal } = {},
): Promise<Beside> {
  const params: Record<string, string> = { path };
  if (folder) {
    params.folder = 'true';
  }
  if (under !== undefined) {
    params.under = under;
  }
  return askedForJson<Beside>(apiUrl('beside', params), signal);
}
