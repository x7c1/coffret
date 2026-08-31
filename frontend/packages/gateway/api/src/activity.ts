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

/** What the server is doing on its own — `GET /api/activity`. */
export interface Activity {
  /** The latest fill, running or finished, and `null` where none has run. */
  fill: Fill | null;
}

/** Asks what the server is doing on its own. */
export function getActivity(signal?: AbortSignal): Promise<Activity> {
  return askedForJson<Activity>(apiUrl('activity'), signal);
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
