import { apiUrl, askedForJson } from './request';

/** Whether a row a filter kept is a folder or a file. */
export type FilterRowKind = 'folder' | 'file';

/**
 * One row a filtered tree keeps: a file, or a folder, by its path.
 *
 * Only the row itself. The folders above it are not rows of the answer — the
 * explorer derives them from the path — and nor is what a matched folder
 * holds, which is listed folder by folder as it is opened.
 */
export interface FilterRow {
  /** A file's Entry Path, or a folder's path; the Library root, a row only as a book, is the empty string. */
  path: string;
  kind: FilterRowKind;
  /**
   * On a folder, the newest deposit time among the files directly in it, ISO
   * 8601 in UTC: when the book it is was added. `null` where it holds no file
   * directly, or for a count of seconds no calendar reaches. Absent on a file.
   */
  deposited_at?: string | null;
}

/**
 * Which rows a tree filtered by a name, by the most recently added books, or
 * by both keeps — `GET /api/filter?name=&recent=`.
 *
 * The rows are in path order, at most a fixed number of them; `total` is how
 * many matched over the whole Library, and `truncated` whether `rows` stops
 * short of it.
 */
export interface Filtered {
  rows: FilterRow[];
  total: number;
  truncated: boolean;
}

/**
 * What a filter is asked with: a name matched against each segment of every
 * path, the count of most recently added books, or both. A filter naming
 * neither, or an empty name, is refused by the server.
 */
export interface FilterAsk {
  name?: string;
  recent?: number;
}

/** Asks which rows a filtered tree keeps, out of the catalog alone. */
export function getFilter(ask: FilterAsk, signal?: AbortSignal): Promise<Filtered> {
  const params: [string, string][] = [];
  if (ask.name !== undefined) {
    params.push(['name', ask.name]);
  }
  if (ask.recent !== undefined) {
    params.push(['recent', String(ask.recent)]);
  }
  return askedForJson<Filtered>(apiUrl('filter', params), signal);
}
