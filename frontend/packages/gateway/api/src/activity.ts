import type { DeclinedReason, RefusalKind, SurfacedFinding } from './refusal';
import { apiUrl, askedForJson } from './request';

/** Where a fill of one folder stands. */
export type FillStatus =
  /** Armed, or walking the folder. */
  | 'filling'
  /** Every file it set out to bring over is here or accounted for. */
  | 'done'
  /** It stopped short, and `stopped` says what stopped it. */
  | 'stopped'
  /** A fetch landed in another folder and the fill followed it there. */
  | 'superseded';

/**
 * One refusal, in the shape every refusal from this server takes.
 *
 * The same fields a refused request carries, so a declined Entry is read with
 * the same three branches: which kind, which way, which finding.
 */
export interface Refused {
  error: RefusalKind;
  message: string;
  reason?: DeclinedReason;
  surfaced?: SurfacedFinding;
}

/** One Entry a fill did not bring over, and what it found instead. */
export interface DeclinedEntry extends Refused {
  path: string;
}

/**
 * What the server is bringing over on its own.
 *
 * Opening a file this device does not have fetches it and then goes on to fetch
 * the rest of its folder, unasked — nobody who opened page one stops there.
 * This is that work's account of itself, and it is the server's own state: it
 * says nothing about what the Library holds, it is gone when the server is, and
 * `present` and `remote` stay the listing's to say.
 */
export interface Fill {
  /** The folder being brought over; the Library root is the empty string. */
  folder: string;
  status: FillStatus;
  /**
   * How many of the folder's files the fill set out to bring over, and `0`
   * until it has read the folder's listing.
   */
  total: number;
  /** How many of them are on this device now. */
  done: number;
  /** The Entries it declined, each with what opening it would have said. */
  declined: DeclinedEntry[];
  /** What stopped the fill, and `null` where nothing did. */
  stopped: Refused | null;
}

/** Where a sync stands. */
export type SyncStatus =
  /** Armed, or walking the mapped folders. */
  | 'syncing'
  /** It finished, whatever it found. */
  | 'done'
  /** It stopped short, and `stopped` says what stopped it. */
  | 'stopped';

/**
 * One thing a sync that succeeded still has to say.
 *
 * Not a refusal: nothing was refused, the run succeeded, and this is what it
 * left alone — a file whose Entry lives in a Pack, a file this device no longer
 * has, a mapped root it could not vouch for. Reading only the counts would tell
 * somebody their file is backed up when it is not.
 */
export interface SyncFinding {
  /** The Entry this is about, and `null` where it is about no single one. */
  path: string | null;
  message: string;
}

/**
 * What the server is carrying into the Library on its own.
 *
 * Dropping a file means "add this", and adding is not finished when the bytes
 * reach the folder: the server runs the same sync the person would have typed,
 * and this is that run's account of itself. Like a fill it is the server's own
 * state — gone when the server is, and never uploaded.
 */
export interface Sync {
  status: SyncStatus;
  /** How many files the run carried in, and `0` until it is over. */
  added: number;
  /** What it found and did not act on. */
  noted: SyncFinding[];
  /** What stopped the sync, and `null` where nothing did. */
  stopped: Refused | null;
}

/** Where a freeze of one folder stands. */
export type FreezeStatus =
  /** Armed, or packing the folder. */
  | 'freezing'
  /** It finished, whatever it found. */
  | 'done'
  /** It stopped short, and `stopped` says what stopped it. */
  | 'stopped';

/**
 * What the server is packing into the Library on its own.
 *
 * Dropping a book into a folder made for it means "bring this in", and a book is
 * the one thing a sync is the wrong shape for: a folder of a few hundred page
 * images would become a few hundred Storage objects, a few hundred uploads, and
 * a few hundred calls to open it again. So the server packs them instead, and
 * this is that run's account of itself. Like a fill and a sync it is the
 * server's own state — gone when the server is, and never uploaded.
 *
 * There is no progress count, and there is nothing for one to count: a freeze
 * builds and commits one batch, so until it has committed there is no partial
 * answer that would be true. What is here is what it came to.
 */
export interface Freeze {
  /** The folder being packed; the Library root is the empty string. */
  folder: string;
  status: FreezeStatus;
  /** How many Packs the run built, and `0` until it is over. */
  packs: number;
  /** How many Entries those Packs hold, and `0` until it is over. */
  entries: number;
  /** What it found and did not act on. */
  noted: SyncFinding[];
  /** What stopped the freeze, and `null` where nothing did. */
  stopped: Refused | null;
}

/** What the server is doing on its own — `GET /api/activity`. */
export interface Activity {
  /** The latest fill, running or finished, and `null` where none has run. */
  fill: Fill | null;
  /** The latest sync, running or finished, and `null` where none has run. */
  sync: Sync | null;
  /** The latest freeze, running or finished, and `null` where none has run. */
  freeze: Freeze | null;
}

/** Asks what the server is doing on its own. */
export function getActivity(signal?: AbortSignal): Promise<Activity> {
  return askedForJson<Activity>(apiUrl('activity'), signal);
}

/**
 * Carries the mapped folders into the Library again — `POST /api/sync`.
 *
 * Not a "sync now" button and not offered as one. What syncs a dropped file is
 * dropping it; this exists for the state that leaves behind — a sync Storage
 * stopped, whose files are sitting in the folder with nothing left to drop —
 * where the alternative is telling somebody to add a file they have added.
 *
 * It takes no folder. Which folders a sync walks is the device's mappings and
 * never an argument, here as on the command line.
 */
export function startSync(signal?: AbortSignal): Promise<Activity> {
  return askedForJson<Activity>(apiUrl('sync'), signal, 'POST');
}

/**
 * Takes one folder up again — `POST /api/fill?path=`.
 *
 * Not a download button and not offered as one. What brings a folder over is
 * opening a file in it; this exists for the two states that leaves behind — a
 * fill Storage stopped, and one superseded when somebody clicked elsewhere —
 * where the alternative is telling a person to open a file they have opened.
 *
 * It answers with the activity as it stands the moment the fill is armed rather
 * than waiting for the work, which is why the caller goes on polling.
 */
export function startFill(folder: string, signal?: AbortSignal): Promise<Activity> {
  return askedForJson<Activity>(
    apiUrl('fill', folder === '' ? undefined : { path: folder }),
    signal,
    'POST',
  );
}

/**
 * Packs one folder into Packs again — `POST /api/freeze?path=`.
 *
 * Not a "pack this" button and not offered as one. What packs a book is bringing
 * it in — dropping its pages onto a folder made a moment ago, which arms this
 * itself — and this exists for the state that leaves behind: a freeze Storage
 * stopped, whose pages are sitting in the folder with nothing left to drop,
 * where the alternative is telling somebody to drop a book they have dropped.
 *
 * It takes a folder, unlike the sync: a freeze is of one folder, and one
 * narrowed to nothing would pack the whole Library.
 *
 * It answers with the activity as it stands the moment the freeze is armed
 * rather than waiting for the work, which is why the caller goes on polling.
 */
export function startFreeze(folder: string, signal?: AbortSignal): Promise<Activity> {
  return askedForJson<Activity>(
    apiUrl('freeze', folder === '' ? undefined : { path: folder }),
    signal,
    'POST',
  );
}
