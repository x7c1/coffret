import { apiUrl, askedForJson } from './request';

/** One folder inside the folder a browse listed. */
export interface BrowsedFolder {
  name: string;
  /** Its whole path on this device. */
  path: string;
}

/** The folders directly inside one folder on this device. */
export interface Browsed {
  /**
   * The folder listed, as a whole path with no symbolic link left in it —
   * which is what a mapping of it would record, so it is the path to offer
   * rather than whatever text was asked with.
   */
  path: string;
  /** The folder above it, and `null` at the root of the filesystem. */
  parent: string | null;
  /**
   * The folders inside it, sorted by name. Never a file, never a name starting
   * with a dot, and never a symbolic link.
   */
  folders: BrowsedFolder[];
}

/**
 * The folders on this device inside `path` — `GET /api/browse` — or inside the
 * home directory where no path is given.
 *
 * Not about the Library: these are folders on the device's own disk, which is
 * what a mapping names. A path that is not a folder is a `bad_request` refusal
 * naming it.
 */
export function browseFolders(path: string | null, signal?: AbortSignal): Promise<Browsed> {
  return askedForJson<Browsed>(
    apiUrl('browse', path === null ? undefined : { path }),
    signal,
  );
}
