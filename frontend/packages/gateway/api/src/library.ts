import { apiUrl, askedForJson } from './request';

/**
 * Which Library the browser is looking at — `GET /api/library`.
 *
 * Three fields, and what is missing from them is the point: not the epoch, not
 * the checkpoint, not the folder id or the bucket name. The provider is here
 * because "on Google Drive" is something a person recognizes their own Library
 * by; it names the provider and nothing about the account.
 */
export interface Library {
  /**
   * The Library's name, which is what the Activity panel shows, and null where
   * nothing has named it yet: `library_id` is its ID and never stands in for
   * the name.
   */
  name: string | null;
  library_id: string;
  provider: string;
}

/** Asks which Library this is. */
export function getLibrary(signal?: AbortSignal): Promise<Library> {
  return askedForJson<Library>(apiUrl('library'), signal);
}

/**
 * The parcels this device holds for reading again, and what their files take
 * on its disk.
 *
 * A fetch holds every parcel it read until every Entry it covers that this
 * device maps is on the device or witnessed absent, or its Container leaves the
 * current set, and nothing else tells a person the directory they are held in
 * exists: a reader who opens the first page of many books leaves a parcel or
 * two behind for each, which over time is gigabytes.
 */
export interface HeldParcels {
  /** How many parcels are held. */
  count: number;
  /** How many bytes their files take. */
  bytes: number;
}

/**
 * What the whole Library comes to — `GET /api/library/totals`, out of the
 * catalog and a stat of each held parcel's file, never of Storage, and refused
 * while the Library is locked (spec: LA-39).
 *
 * A folder's counts in a listing, over every Entry, and the two no folder has:
 * how many objects the Library keeps on Storage, and the parcels this device
 * holds.
 */
export interface LibraryTotals {
  /**
   * How many objects the Library's current state rests on, as Storage counts
   * them: each current Container once, a Pack being one, the control head, and
   * each replica of the committed Keyring. The Journal behind the head and
   * older checkpoints are not counted, since only a listing of Storage could
   * say which of them are still there.
   */
  objects: number;
  /** How many files the Library holds at every depth. */
  files_total: number;
  /** How many of those this device has. */
  files_present: number;
  /** The plaintext bytes of every file together, here or not. */
  size: number;
  /**
   * When the newest file anywhere in the Library was modified, ISO 8601 in UTC;
   * `null` for a Library holding no file, and for a count of seconds no
   * calendar reaches.
   */
  mtime: string | null;
  /**
   * What the Library takes on this device's disk: the plaintext bytes of the
   * `files_present` files, and the bytes the held parcels' files take — the
   * present files' part alone where `held_parcels` could not be counted.
   */
  size_on_device: number;
  /** The parcels this device holds, and `null` where they could not be counted. */
  held_parcels: HeldParcels | null;
}

/** Asks what the whole Library comes to. */
export function getLibraryTotals(signal?: AbortSignal): Promise<LibraryTotals> {
  return askedForJson<LibraryTotals>(apiUrl('library/totals'), signal);
}
