// What a background fill adds to what a listing says, kept free of DOM so it is
// unit testable.
//
// The listing is the one answer about what is on this device: `present` or
// `remote`, and nothing else, because nothing on this device changes between
// asking for an Entry and being handed it. A fill is the other half — work the
// server took up unasked — and it only ever *adds* to a `remote` row: that a
// fetch of it is running now, that it was declined and why, that Storage stopped
// before it was reached. A row the listing calls `present` is present, whatever
// a fill left over from a moment ago still says.

import type { DeclinedEntry, Fill, ListedFile } from '@coffret/api';

/** How often the activity is asked for while anything is happening. */
export const ACTIVITY_INTERVAL_MS = 700;

/** What one row of a listing shows for its state. */
export type RowState =
  /** This device has the file. */
  | 'present'
  /** The Library has it and this device does not. */
  | 'remote'
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

/** Whether a fill is under way, rather than finished, stopped or left. */
function isFilling(fill: Fill | null): boolean {
  return fill?.status === 'filling';
}

/**
 * Whether to be polling the activity route at all.
 *
 * An explorer with nothing in flight asks for nothing: the whole point of the
 * interval is the minutes a fill takes, and an idle tab that kept asking would
 * be a page making a request a second forever.
 *
 * Two reasons to ask, and they overlap rather than nest. The reader being open
 * means a fetch may be about to arm a fill this screen has not heard of yet; a
 * fill already running means there is something to follow, whether or not the
 * reader is still open over it.
 */
export function shouldPoll(readerOpen: boolean, fill: Fill | null): boolean {
  return readerOpen || isFilling(fill);
}

/** The Library root has no name of its own, and is not called the empty string. */
function named(folder: string): string {
  return folder === '' ? 'the Library root' : folder;
}
