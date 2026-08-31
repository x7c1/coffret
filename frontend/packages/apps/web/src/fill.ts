// What the server's background work adds to what a listing says, kept free of
// DOM so it is unit testable.
//
// The listing is the one answer about what is in the folder: `present`,
// `remote` or `uploading`, and nothing else, because nothing on this device
// changes between asking for an Entry and being handed it. A fill is the other
// half — work the server took up unasked — and it only ever *adds* to a `remote`
// row: that a fetch of it is running now, that it was declined and why, that
// Storage stopped before it was reached. A row the listing calls `present` is
// present, whatever a fill left over from a moment ago still says, and a row it
// calls `uploading` is a file in the folder that the Library does not have.

import type { DeclinedEntry, Fill, ListedFile, Sync } from '@coffret/api';

/** How often the activity is asked for while anything is happening. */
export const ACTIVITY_INTERVAL_MS = 700;

/** What one row of a listing shows for its state. */
export type RowState =
  /** This device has the file. */
  | 'present'
  /** The Library has it and this device does not. */
  | 'remote'
  /** It is in the folder and the Library does not have it yet. */
  | 'uploading'
  /** It is being brought over right now. */
  | 'fetching'
  /** Storage stopped the fill before it was reached. */
  | 'failed'
  /** The fill would not place it, and said why. */
  | 'declined';

/** What a row shows, and the sentence behind it where there is one. */
export interface RowFill {
  state: RowState;
  /** What to say about the row, and `null` where its state says it all. */
  message: string | null;
}

/**
 * What one row of `folder` shows, given the listing and the fill.
 *
 * Only a fill of the folder being looked at says anything about these rows. A
 * fill of somewhere else is somebody else's folder being brought over, and the
 * rows here are what the listing says they are.
 */
export function rowFill(file: ListedFile, folder: string, fill: Fill | null): RowFill {
  if (file.state === 'present') {
    return { state: 'present', message: null };
  }
  // A file the Library does not hold. There is no Entry to fetch, so a fill has
  // nothing to say about it — and the sync that will carry it in says what it
  // did in the status bar rather than row by row.
  if (file.state === 'uploading') {
    return {
      state: 'uploading',
      message: 'this file is in the folder and not in the Library yet',
    };
  }
  if (fill === null || fill.folder !== folder) {
    return { state: 'remote', message: null };
  }
  const declined = declinedEntry(fill, file.path);
  if (declined !== null) {
    // Not a failure: the fill found something about this one Entry — a file
    // this device did not place, a Container it has no key for — and said what
    // the file route would have said had it been clicked.
    return { state: 'declined', message: declined.message };
  }
  switch (fill.status) {
    case 'filling':
      return { state: 'fetching', message: null };
    case 'stopped':
      return {
        state: 'failed',
        message: fill.stopped?.message ?? 'the fill stopped before reaching this file',
      };
    // A fill that finished or was left for another folder says nothing about a
    // row it never reached: the row is what the listing calls it.
    case 'done':
    case 'superseded':
      return { state: 'remote', message: null };
  }
}

/** What one fill said about one Entry, where it said anything. */
function declinedEntry(fill: Fill | null, path: string): DeclinedEntry | null {
  return fill?.declined.find((entry) => entry.path === path) ?? null;
}

/**
 * The line the status bar shows for a fill, or `null` for one worth no line.
 *
 * A fill that finished says nothing: it is the rows that changed, and they say
 * so themselves. A fill that stopped keeps its line, because that line is what
 * the retry hangs off.
 */
export function fillLine(fill: Fill | null): string | null {
  if (fill === null) {
    return null;
  }
  switch (fill.status) {
    case 'filling':
      // The total is unknown until the folder's listing has been read, and a
      // count of `0/0` would read as nothing to do rather than as not yet known.
      return fill.total === 0
        ? `bringing over ${named(fill.folder)}…`
        : `bringing over ${fill.done}/${fill.total} in ${named(fill.folder)}…`;
    case 'stopped':
      return `could not bring over ${named(fill.folder)} — ${
        fill.stopped?.message ?? 'Storage did not answer'
      }`;
    case 'done':
    case 'superseded':
      return null;
  }
}

/**
 * The line the status bar shows for a sync, or `null` for one worth no line.
 *
 * A sync that finished with nothing to report says nothing: it is the rows that
 * changed, and they say so themselves. One that found something keeps its line,
 * because that finding is the only place the person who dropped a file is told
 * their file was not backed up — and one that stopped keeps its line for the
 * reason a stopped fill does: the retry hangs off it.
 */
export function syncLine(sync: Sync | null): string | null {
  if (sync === null) {
    return null;
  }
  switch (sync.status) {
    case 'syncing':
      return 'backing up what was added…';
    case 'stopped':
      return `could not back up what was added — ${
        sync.stopped?.message ?? 'Storage did not answer'
      }`;
    case 'done':
      return sync.noted.length === 0 ? null : noted(sync);
  }
}

/** What a run that succeeded still had to say, as one line. */
function noted(sync: Sync): string {
  const [first] = sync.noted;
  const rest = sync.noted.length - 1;
  const named = first.path === null ? first.message : `${first.path} — ${first.message}`;
  return rest === 0 ? named : `${named} (and ${rest} more)`;
}

/** The line shown while a drop's own files are still going up. */
export function addingLine(files: number, folder: string): string {
  return `adding ${files} ${files === 1 ? 'file' : 'files'} to ${named(folder)}…`;
}

/** Whether a fill is under way, rather than finished, stopped or left. */
function isFilling(fill: Fill | null): boolean {
  return fill?.status === 'filling';
}

/** Whether a sync is under way, rather than finished or stopped. */
function isSyncing(sync: Sync | null): boolean {
  return sync?.status === 'syncing';
}

/**
 * Whether to be polling the activity route at all.
 *
 * An explorer with nothing in flight asks for nothing: the whole point of the
 * interval is the minutes a fill or a sync takes, and an idle tab that kept
 * asking would be a page making a request a second forever.
 *
 * Three reasons to ask, and they overlap rather than nest. The reader being open
 * means a fetch may be about to arm a fill this screen has not heard of yet; a
 * fill already running means there is something to follow, whether or not the
 * reader is still open over it; and a sync running is the rows of the folder
 * somebody just dropped into being about to change.
 */
export function shouldPoll(
  readerOpen: boolean,
  fill: Fill | null,
  sync: Sync | null,
): boolean {
  return readerOpen || isFilling(fill) || isSyncing(sync);
}

/** The Library root has no name of its own, and is not called the empty string. */
function named(folder: string): string {
  return folder === '' ? 'the Library root' : folder;
}
