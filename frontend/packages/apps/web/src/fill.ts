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
  Catalog,
  DeclinedEntry,
  Fill,
  Freeze,
  ListedFile,
  Step,
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

/**
 * The run of the fill the rows of `folder` are to read, and `null` where none
 * of them is about this folder.
 *
 * The run on record where that is the one about this folder, and otherwise one
 * that stopped and had the record taken from it. A folder a fill stopped half
 * way through is half here, and the `failed` and `declined` chips over its rows
 * are the other half of the sentence its line carries — so they are owed for as
 * long as the line is, which is past the moment the next folder starts.
 *
 * The stopped runs are handed in rather than read off the run on record, because
 * what the rows may show is what the bar is showing: one whose notice somebody
 * has put away has taken its chips with it, exactly as putting the fill's own
 * line away takes its.
 */
export function fillOfFolder(
  onRecord: Fill | null,
  stopped: readonly Fill[],
  folder: string,
): Fill | null {
  if (onRecord !== null && onRecord.folder === folder) {
    return onRecord;
  }
  return stopped.find((run) => run.folder === folder) ?? null;
}

/** What one fill said about one Entry, where it said anything. */
function declinedEntry(fill: Fill | null, path: string): DeclinedEntry | null {
  return fill?.declined.find((entry) => entry.path === path) ?? null;
}

/**
 * The line the status bar shows for a fill, or `null` for one worth no line.
 *
 * A fill that finished having placed everything says nothing: it is the rows
 * that changed, and they say so themselves. A fill that stopped keeps its line,
 * because that line is what the retry hangs off.
 *
 * And a fill that finished having *declined* something keeps one too, which is
 * the state the two above leave between them. Such a run ends at "28/30" and
 * then takes its line off the screen, which is the same picture a fill that
 * stopped at 28 of 30 leaves — and the two are opposite answers: one is over
 * and the other is not. So the finished one says it finished, says how many it
 * left, and says why it left the first of them; the rows carry the rest, each
 * marked with what opening it would have said.
 *
 * The running line also names the folders waiting behind it, as the freeze's
 * does. A folder asked for by name queues rather than displacing the run, so
 * between the press and its turn the queue is the only thing about it there is
 * to say — the button that named it goes away as the server takes it up, and
 * the line names the folder being brought over. Only the running line: by the
 * time a run is over the queue has been taken up or thrown away, and a folder
 * thrown away has a notice of its own.
 */
export function fillLine(fill: Fill | null): string | null {
  if (fill === null) {
    return null;
  }
  switch (fill.status) {
    case 'filling': {
      // The total is unknown until the folder's listing has been read, and a
      // count of `0/0` would read as nothing to do rather than as not yet known.
      const over =
        fill.total === 0
          ? `bringing over ${named(fill.folder)}`
          : `bringing over ${fill.done}/${fill.total} in ${named(fill.folder)}`;
      return `${over}${queued(fill.waiting)}…`;
    }
    case 'stopped':
      return `could not bring over ${named(fill.folder)} — ${
        fill.stopped?.message ?? 'Storage did not answer'
      }`;
    case 'done':
      return fill.declined.length === 0 ? null : declinedLine(fill);
    case 'superseded':
      return null;
  }
}

/** What a fill that finished and left something behind says, as one line. */
function declinedLine(fill: Fill): string {
  const left = fill.declined.length;
  const many = left === 1 ? '1 file was not placed' : `${left} files were not placed`;
  return `brought over ${fill.done}/${fill.total} in ${named(fill.folder)} — ${many}: ${noted(
    fill.declined,
  )}`;
}

/**
 * The line the status bar shows for the folders a fill's queue lost, or `null`
 * where it lost none.
 *
 * Its own line rather than a clause on the fill's, because it is about neither
 * the run that died nor the one running now: it is the folders that were armed
 * behind a worker that ended without an answer and were thrown away with it.
 * The fill's own line and its retry both name the folder that died, so without
 * this a person takes that one up again and never learns the rest were dropped.
 */
export function droppedLine(folders: readonly string[]): string | null {
  return dropped(folders, 'brought over');
}

/**
 * The same for the books a freeze's queue lost.
 *
 * Its own line rather than the one above with a different word in it, because
 * the two queues are different work and can lose folders at the same time: a
 * fill worker and a freeze worker are separate tasks, and a person owed both
 * sentences must not be given one of them twice.
 */
export function droppedBooksLine(folders: readonly string[]): string | null {
  return dropped(folders, 'packed');
}

/** What a queue that lost folders says, whichever queue it was. */
function dropped(folders: readonly string[], ended: string): string | null {
  if (folders.length === 0) {
    return null;
  }
  const rest = folders.length - 1;
  const first = named(folders[0]);
  return rest === 0
    ? `${first} was dropped before it was ${ended}`
    : `${first} and ${rest} more were dropped before they were ${ended}`;
}

/**
 * The line the status bar shows for the fills that stopped and had the record
 * taken from them by a later run, or `null` where there are none.
 *
 * The run's own sentence, said by the same function that says it while that run
 * is the one on record: it stopped, and nothing about it changed when the next
 * folder started. Written out again here it would drift from the one the bar
 * showed a tick earlier, which is the same failure reported twice in two
 * wordings.
 *
 * Its own line rather than a clause on the running run's, for the reason the
 * dropped folders have one: it is about neither the run that is going nor the
 * one before it in particular. And distinct from the dropped line, because the
 * two say different things — one folder was never started on and this one is
 * half here — and a person owed both is owed both.
 */
export function stoppedLine(runs: readonly Fill[]): string | null {
  return andTheRest(runs.length, fillLine(runs[0] ?? null));
}

/** The same for the books a freeze stopped on. */
export function stoppedBooksLine(runs: readonly Freeze[]): string | null {
  return andTheRest(runs.length, freezeLine(runs[0] ?? null));
}

/**
 * One run's sentence, with a count of the ones standing behind it.
 *
 * The bar has room for one line and one Storage outage stops every folder queued
 * behind the first, so the oldest speaks for them: a bare count says how many
 * there are, and each of them has a button of its own beside it saying which.
 */
function andTheRest(count: number, line: string | null): string | null {
  const rest = count - 1;
  return line === null || rest < 1 ? line : `${line} (and ${rest} more stopped)`;
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
      return `backing up what was added${phaseOf(sync.step)}…`;
    case 'stopped':
      return `could not back up what was added — ${
        sync.stopped?.message ?? 'Storage did not answer'
      }`;
    case 'done':
      return sync.noted.length === 0 ? null : noted(sync.noted);
  }
}

/**
 * The one word for what a freeze does, said once.
 *
 * Shared by the freeze's own line and by the name its `packing` phase goes
 * under, because the two are the same word about the same work and a line that
 * has them apart is a line that can say it twice.
 */
const PACKING = 'packing';

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
      return `${PACKING} ${named(freeze.folder)}${phaseOf(freeze.step, PACKING)}${queued(
        freeze.waiting,
      )}…`;
    case 'stopped':
      return `could not pack ${named(freeze.folder)} — ${
        freeze.stopped?.message ?? 'Storage did not answer'
      }`;
    case 'done':
      return freeze.noted.length === 0 ? packed(freeze) : noted(freeze.noted);
  }
}

/**
 * What a run says about where it has got to, as a clause on its own line.
 *
 * The flow's own answer and not this screen's reading of one: the same
 * [`Step`](@coffret/api) the command line renders, reported by the use case
 * through the same port, so a browser and a terminal watching one run say the
 * same thing about it. That is the whole of why a freeze of several hundred
 * pages no longer shows one fixed sentence for minutes.
 *
 * A phase that cannot count its work says its name and no numbers, which is not
 * the same as a phase with nothing in it — the first is exactly the phase that
 * goes quiet, and a `0/0` beside it would read as work already done.
 *
 * `said` is the word the line has already used for itself, where it has used
 * one. A freeze whose phase is `packing` says "packing" in its own opening
 * clause, and repeating it reads as `packing books/vol-1 — packing 12/300`: the
 * same word twice with the only new thing in the clause, the count, hidden
 * behind it. Named rather than guessed at from the status, because what the two
 * must agree on is the word itself.
 */
function phaseOf(step: Step | null, said: string | null = null): string {
  if (step === null) {
    return '';
  }
  const doing = DOING[step.phase] === said ? null : DOING[step.phase];
  if (step.total === null) {
    return doing === null ? '' : ` — ${doing}`;
  }
  const count = `${step.done}/${step.total}`;
  return doing === null ? ` — ${count}` : ` — ${doing} ${count}`;
}

/**
 * What each phase is called on the screen.
 *
 * A word a person watching could act on rather than the flow's own name for the
 * step. Every phase has one, because a phase with none would be a line that
 * went blank in the middle of a run.
 */
const DOING: Record<Step['phase'], string> = {
  catching_up: 'catching up with the Library',
  reconciling: 'settling what an interrupted run left',
  scanning: 'reading the folders',
  packing: PACKING,
  uploading: 'sending',
  fetching: 'bringing over',
};

/**
 * What a run says about the folders behind it, where any are waiting.
 *
 * One clause for both queues, because they are the same news in the same
 * words: a book waiting to be packed and a folder waiting to be brought over
 * are each a person's press with nothing else on the screen about it.
 */
function queued(waiting: readonly string[]): string {
  if (waiting.length === 0) {
    return '';
  }
  return waiting.length === 1
    ? `, with ${named(waiting[0])} after it`
    : `, with ${named(waiting[0])} and ${waiting.length - 1} more after it`;
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
function noted(findings: readonly Pick<SyncFinding, 'path' | 'message'>[]): string {
  const [first] = findings;
  const rest = findings.length - 1;
  const named = first.path === null ? first.message : `${first.path} — ${first.message}`;
  return rest === 0 ? named : `${named} (and ${rest} more)`;
}

/**
 * The line shown while a drop's own files are still being written into the
 * folder this device maps.
 *
 * Not the line for them going up: what carries them into the Library is the
 * sync or the freeze the drop armed, and that has its own.
 */
export function addingLine(files: number, folder: string): string {
  return `adding ${files} ${files === 1 ? 'file' : 'files'} to ${named(folder)}…`;
}

/**
 * The line shown between a drop being let go of and there being anything to
 * send.
 *
 * A browser hands a dropped folder over as something to walk rather than as
 * files, one batch of children at a time, and a nested folder of several
 * hundred pages is seconds of that before the first byte goes anywhere. No
 * count can be named yet — counting them is what the walk is — so what is said
 * is what is happening.
 *
 * It names no folder either, and deliberately: the files of a nested drop are
 * going into folders one level down that this screen has no rows for, so naming
 * the folder on the screen would be naming the one place most of them are not.
 */
export function collectingLine(): string {
  return 'reading what was dropped…';
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
 * What the screen reads to say that a book dropped now is packed after the one
 * already going up: they are packed one at a time (spec: PK-7), and the server
 * queues the second rather than displacing the first.
 */
export function isFreezing(freeze: Freeze | null): boolean {
  return freeze?.status === 'freezing';
}

/**
 * Whether this device's catalog is being caught up with the Library right now.
 *
 * A reason to keep asking: what a listing shows comes out of the catalog, and a
 * catch-up that lands changes every folder's answer.
 */
export function isCatchingUp(catalog: Catalog | null): boolean {
  return catalog?.state === 'catching_up';
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
 * The reasons overlap rather than nest. The reader being open means a fetch may
 * be about to arm a fill this screen has not heard of yet; a fill already
 * running means there is something to follow, whether or not the reader is
 * still open over it — which is what a reload in the middle of one comes back
 * to, since the run is the server's and only the page forgot; a sync running is
 * the rows of the folder somebody just dropped into being about to change; a
 * freeze running, or one waiting its turn, is a whole book's worth of them
 * being about to; and a catch-up running is every folder's answer being about
 * to change at once.
 *
 * A folder waiting behind a fill counts for the reason a book waiting behind a
 * freeze does: it is work the server is going to do that nothing has answered
 * for yet, and a page that stopped asking would leave the press it was made by
 * with no ending on the screen.
 */
export function shouldPoll(
  readerOpen: boolean,
  fill: Fill | null,
  sync: Sync | null,
  freeze: Freeze | null = null,
  catalog: Catalog | null = null,
): boolean {
  return (
    readerOpen ||
    isFilling(fill) ||
    (fill?.waiting.length ?? 0) > 0 ||
    isSyncing(sync) ||
    isFreezing(freeze) ||
    (freeze?.waiting.length ?? 0) > 0 ||
    isCatchingUp(catalog)
  );
}

/**
 * Whether to ask the activity route now, given whether this page has been told
 * anything yet and whether there is anything to follow.
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
 *
 * `told` and not "asked", and the difference is the whole of one failure: a
 * request that never answered taught this page nothing, so a page counting it
 * as having asked would go quiet for the life of the tab with no idea what the
 * server is doing — and, since nothing about a failed activity request is shown
 * on the screen, with nothing to press about it either. A question counts once
 * it has been answered.
 */
export function shouldAsk(told: boolean, polling: boolean): boolean {
  return polling || !told;
}

/** The Library root has no name of its own, and is not called the empty string. */
function named(folder: string): string {
  return folder === '' ? 'the Library root' : folder;
}
