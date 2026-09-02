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

import type {
  DeclinedEntry,
  Fill,
  Freeze,
  ListedFile,
  Sync,
  SyncFinding,
} from '@coffret/api';

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
      return sync.noted.length === 0 ? null : noted(sync.noted);
  }
}

/**
 * The line the status bar shows for a freeze, or `null` for one worth no line.
 *
 * The one that finished says what it came to rather than nothing, unlike a
 * finished fill or a quiet sync. That is the whole of what a person dropping a
 * book was after — their several hundred pages went up as a handful of objects
 * rather than as one per page — and the rows cannot say it: a row says whether
 * this device has the file, and every one of them would look exactly the same
 * had the pages been carried in one at a time.
 *
 * A run that left something alone says that instead, for the reason the sync
 * does: it is the only place the person is told a page was not packed. And one
 * that stopped keeps its line because the retry hangs off it.
 */
export function freezeLine(freeze: Freeze | null): string | null {
  if (freeze === null) {
    return null;
  }
  switch (freeze.status) {
    case 'freezing':
      return `packing ${named(freeze.folder)}…`;
    case 'stopped':
      return `could not pack ${named(freeze.folder)} — ${
        freeze.stopped?.message ?? 'Storage did not answer'
      }`;
    case 'done':
      return freeze.noted.length === 0 ? packed(freeze) : noted(freeze.noted);
  }
}

/** What a freeze that packed something came to, as one line. */
function packed(freeze: Freeze): string {
  const packs = `${freeze.packs} ${freeze.packs === 1 ? 'Pack' : 'Packs'}`;
  const entries = `${freeze.entries} ${freeze.entries === 1 ? 'file' : 'files'}`;
  return freeze.entries === 0
    ? `${named(freeze.folder)} was already packed`
    : `packed ${entries} of ${named(freeze.folder)} into ${packs}`;
}

/**
 * What a run that succeeded still had to say, as one line.
 *
 * Shared by the sync and the freeze, because the findings are: a page whose
 * Entry is inside a Pack and a photograph whose Entry is are the same sentence
 * about the same state (spec: PK-14).
 */
function noted(findings: readonly SyncFinding[]): string {
  const [first] = findings;
  const rest = findings.length - 1;
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
 * Whether a freeze is under way, rather than finished or stopped.
 *
 * What the screen reads to know not to offer a second book: one is packed at a
 * time, and a person told they may drop another would be queueing work behind
 * one whose own progress they are still watching.
 */
export function isFreezing(freeze: Freeze | null): boolean {
  return freeze?.status === 'freezing';
}

/**
 * Whether the folder on the screen is the one being packed right now.
 *
 * A freeze of somewhere else is somebody else's book being brought in, and this
 * folder is what the listing says it is — the same rule a fill's rows follow.
 */
export function freezingHere(freeze: Freeze | null, folder: string): boolean {
  return isFreezing(freeze) && freeze?.folder === folder;
}

/**
 * Whether to be polling the activity route at all.
 *
 * An explorer with nothing in flight asks for nothing: the whole point of the
 * interval is the minutes a fill, a sync or a freeze takes, and an idle tab that
 * kept asking would be a page making a request a second forever.
 *
 * Four reasons to ask, and they overlap rather than nest. The reader being open
 * means a fetch may be about to arm a fill this screen has not heard of yet; a
 * fill already running means there is something to follow, whether or not the
 * reader is still open over it; a sync running is the rows of the folder
 * somebody just dropped into being about to change; and a freeze running is a
 * whole book's worth of them being about to.
 */
export function shouldPoll(
  readerOpen: boolean,
  fill: Fill | null,
  sync: Sync | null,
  freeze: Freeze | null = null,
): boolean {
  return readerOpen || isFilling(fill) || isSyncing(sync) || isFreezing(freeze);
}

/**
 * Whether to ask the activity route now, given whether this page has asked at
 * all and whether there is anything to follow.
 *
 * Two reasons. The second is the interval's, which is [`shouldPoll`]: something
 * is in flight, so ask again in a moment. The first is the page coming up —
 * because "nothing in flight" is a statement about this page and not about the
 * server. A freeze Storage stopped is still stopped after a reload, with a
 * book's pages sitting in the folder and out of the Library, and a page that
 * came up without asking would show nothing about them and offer nothing to do
 * about them. So the activity is asked for once at the start, alongside the
 * Library, the folders and the listing the mount already asks for.
 *
 * Once, and then not again by itself: every finished and every stopped run
 * leaves `shouldPoll` false, so an explorer that comes up to a quiet server
 * makes that one request and no other. The discipline the interval keeps — an
 * idle explorer asks for nothing *while idle* — is untouched.
 */
export function shouldAsk(asked: boolean, polling: boolean): boolean {
  return polling || !asked;
}

/** The Library root has no name of its own, and is not called the empty string. */
function named(folder: string): string {
  return folder === '' ? 'the Library root' : folder;
}
