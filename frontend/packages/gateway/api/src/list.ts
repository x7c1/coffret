import { apiUrl, askedForJson } from './request';

/** Whether this device has the file for one Entry right now. */
export type EntryState = 'present' | 'remote';

/** Whether an Entry lives in a Container of its own or inside a Pack. */
export type ContainerKind = 'one-file' | 'pack';

/** One folder inside another. */
export interface ListedFolder {
  /** Its last path component, which is what it is called. */
  name: string;
  /** Where in the Library it stands. */
  path: string;
  /**
   * Whether a folder on this device stands for it.
   *
   * Mappings are made at the top level, so the children of the Library root are
   * the one place two siblings can differ; deeper down every child repeats its
   * parent's answer.
   */
  mapped: boolean;
}

/** One Entry as a row in a listing. */
export interface ListedFile {
  name: string;
  path: string;
  /** The Entry's plaintext length in bytes. */
  size: number;
  /** ISO 8601 in UTC, and `null` for a count of seconds no calendar reaches. */
  mtime: string | null;
  state: EntryState;
  container: ContainerKind;
  /** Whether the explorer can display the Entry itself. */
  openable: boolean;
  /** What the bytes are served as. */
  content_type: string;
}

/**
 * What one folder holds, one level down — `GET /api/list?path=`.
 *
 * Both lists are in the order the server answered in, which is the byte order
 * of the canonical paths with no case folding and no locale. It is the order
 * because it is the only one every device agrees on: nothing here re-sorts them.
 */
export interface Listing {
  /** The folder this is a listing of; the Library root is the empty string. */
  path: string;
  /**
   * Whether a folder on this device stands for this part of the Library.
   *
   * It says nothing about what is on disk — that is each row's `state` — and
   * everything about whether anything here *can* be. A listing that says `false`
   * is one whose files no fetch could place, which is why the screen says so
   * over the rows rather than letting a reader find out by being declined.
   */
  mapped: boolean;
  folders: ListedFolder[];
  files: ListedFile[];
}

/** Asks what one folder holds; the empty string is the Library root. */
export function getListing(folder: string, signal?: AbortSignal): Promise<Listing> {
  return askedForJson<Listing>(
    apiUrl('list', folder === '' ? undefined : { path: folder }),
    signal,
  );
}
