import { apiUrl, askedForJson } from './request';

/**
 * Every folder in the Library — `GET /api/folders`.
 *
 * Flat and sorted, each folder named by its whole path, because a Library has
 * no folders to nest: what comes back is every path a separator implies, and
 * the tree the explorer draws is this client's arrangement of them.
 */
export interface Folders {
  folders: string[];
}

/** Asks for every folder in the Library. */
export function getFolders(signal?: AbortSignal): Promise<Folders> {
  return askedForJson<Folders>(apiUrl('folders'), signal);
}
