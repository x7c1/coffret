// What the server's background work adds to what a listing says, kept free of
// DOM so it is unit testable.
//
// The listing is the one answer about what is in the folder: `present`,
// `remote` or `added`, and nothing else, because nothing on this device
// changes between asking for an Entry and being handed it. A fill is the other
// half — work the server took up unasked — and it only ever *adds* to a `remote`
// row: that a fetch of it is running now, that it was declined and why, that
// Storage stopped before it was reached. A row the listing calls `present` is
// present, whatever a fill left over from a moment ago still says, and a row it
// calls `added` is a file in the folder that the Library does not have.

import type {
  Catalog,
  DeclinedEntry,
  DisplacedFill,
  DisplacedFreeze,
  Fill,
  Finding,
  Freeze,
  LibraryState,
  ListedFile,
  Progress,
  Reconnect,
  Step,
  Sync,
} from '@coffret/api';

import { size } from './humanize';

/** How often the work answer is asked for while anything is happening. */
export const POLL_INTERVAL_MS = 700;

/** What one row of a listing shows for its state. */
export type RowState =
  /** This device has the file. */
  | 'present'
  /** The Library has it and this device does not. */
  | 'remote'
  /** It is in the folder and the Library does not have it. */
  | 'added'
  /** It is being brought over right now. */
  | 'fetching'
  /** Storage stopped the fill before it was reached. */
  | 'failed'
  /** The fill would not place it, and said why. */
  | 'declined';

/**
 * What each row state is called on the screen.
 *
 * The chip's word, the way `DOING` is the phase line's: a word a person looking
 * at the row could act on rather than the listing's own name for it. Most
 * states are their own word. `added` is not, because to a person the one fact
 * that sets the row apart is that the file is here and the Library does not
 * have it.
 */
export const SAYS: Record<RowState, string> = {
  present: 'present',
  remote: 'remote',
  added: 'not in Library',
  fetching: 'fetching',
  failed: 'failed',
  declined: 'declined',
};

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
export function rowFill(
  file: ListedFile,
  folder: string,
  fill: Fill | DisplacedFill | null,
): RowFill {
  if (file.state === 'present') {
    return { state: 'present', message: null };
  }
  // A file the Library does not hold yet. There is no Entry to fetch, so a fill
  // has nothing to say about it — and the run that will carry it in says what it
  // did in the status bar rather than row by row. Every file in this state is
  // one the next sync or freeze of its folder sees as new, whether it was never
  // in the Library or another device removed its Container; the sentence stops
  // short of promising the Entry, because a name the run refuses is not taken
  // in, and the run is what says so.
  if (file.state === 'added') {
    return {
      state: 'added',
      message:
        'this file is not in the Library yet — the next sync or freeze of this folder takes it in, or says why not',
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
      return { state: 'failed', message: fill.stopped.message };
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
  stopped: readonly DisplacedFill[],
  folder: string,
): Fill | DisplacedFill | null {
  if (onRecord !== null && onRecord.folder === folder) {
    return onRecord;
  }
  return stopped.find((run) => run.folder === folder) ?? null;
}

/** What one fill said about one Entry, where it said anything. */
function declinedEntry(fill: Fill | DisplacedFill | null, path: string): DeclinedEntry | null {
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
 * does. A folder asked for by name queues rather than superseding the run, so
 * between the press and its turn the queue is the only thing about it there is
 * to say — the button that named it goes away as the server takes it up, and
 * the line names the folder being brought over. Only the running line: by the
 * time a run is over the queue has been taken up or thrown away, and a folder
 * thrown away has a notice of its own.
 */
export function fillLine(fill: Fill | DisplacedFill | null): string | null {
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
      return `could not bring over ${named(fill.folder)} — ${fill.stopped.message}`;
    case 'done':
      return doneLine(fill);
    case 'superseded':
      return null;
  }
}

/**
 * What a fill that finished says, and `null` where it has nothing to say.
 *
 * The Entries it left behind first, because that is the news about the folder
 * somebody is reading. What it found beside them — what its reads found of the
 * Library's Keyring — is said the way a sync's and a freeze's findings are, and
 * after them rather than instead of them: the rows carry the declined Entries,
 * and nothing but this line carries the finding, which is the only place a
 * person who only opens files hears that the Keyring is short (spec: KL-15).
 */
function doneLine(fill: Fill): string | null {
  if (fill.findings.length === 0) {
    return fill.declined.length === 0 ? null : declinedLine(fill);
  }
  const found = oneLine(fill.findings);
  return fill.declined.length === 0 ? found : `${declinedLine(fill)}; ${found}`;
}

/** What a fill that finished and left something behind says, as one line. */
function declinedLine(fill: Fill): string {
  const left = fill.declined.length;
  const many = left === 1 ? '1 file was not placed' : `${left} files were not placed`;
  return `brought over ${fill.done}/${fill.total} in ${named(fill.folder)} — ${many}: ${oneLine(
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
 * this a person takes that one up again and never learns the rest were discarded.
 */
export function discardedLine(folders: readonly string[]): string | null {
  return discarded(folders, 'brought over');
}

/**
 * The same for the books a freeze's queue lost.
 *
 * Its own line rather than the one above with a different word in it, because
 * the two queues are different work and can lose folders at the same time: a
 * fill worker and a freeze worker are separate tasks, and a person owed both
 * sentences must not be given one of them twice.
 */
export function discardedBooksLine(folders: readonly string[]): string | null {
  return discarded(folders, 'packed');
}

/** What a queue that lost folders says, whichever queue it was. */
function discarded(folders: readonly string[], ended: string): string | null {
  if (folders.length === 0) {
    return null;
  }
  const rest = folders.length - 1;
  const first = named(folders[0]);
  return rest === 0
    ? `${first} was discarded before it was ${ended}`
    : `${first} and ${rest} more were discarded before they were ${ended}`;
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
 * discarded folders have one: it is about neither the run that is going nor the
 * one before it in particular. And distinct from the discarded line, because the
 * two say different things — one folder was never started on and this one is
 * half here — and a person owed both is owed both.
 */
export function stoppedLine(runs: readonly DisplacedFill[]): string | null {
  return andTheRest(runs.length, fillLine(runs[0] ?? null));
}

/** The same for the books a freeze stopped on. */
export function stoppedBooksLine(runs: readonly DisplacedFreeze[]): string | null {
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
 * reason a stopped fill does: the retry hangs off it. A stopped one says what
 * stopped it first and then what it found on the way, which is how a Keyring
 * repair its commit made before failing reaches the person who dropped the
 * file (spec: KL-15).
 */
export function syncLine(sync: Sync | null): string | null {
  if (sync === null) {
    return null;
  }
  switch (sync.status) {
    case 'syncing':
      return `backing up what was added${phaseOf(sync.step)}…`;
    case 'stopped':
      return besides(`could not back up what was added — ${sync.stopped.message}`, sync.findings);
    case 'done':
      return sync.findings.length === 0 ? null : oneLine(sync.findings);
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
 * that stopped keeps its line because the retry hangs off it, and says what it
 * found on the way after what stopped it, as a sync's does.
 */
export function freezeLine(freeze: Freeze | DisplacedFreeze | null): string | null {
  if (freeze === null) {
    return null;
  }
  switch (freeze.status) {
    case 'freezing':
      return `${PACKING} ${named(freeze.folder)}${phaseOf(freeze.step, PACKING)}${queued(
        freeze.waiting,
      )}…`;
    case 'stopped':
      return besides(
        `could not pack ${named(freeze.folder)} — ${freeze.stopped.message}`,
        freeze.findings,
      );
    case 'done':
      return freeze.findings.length === 0 ? packed(freeze) : oneLine(freeze.findings);
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
  const counted = doing === null ? ` — ${count}` : ` — ${doing} ${count}`;
  return `${counted}${bytesOf(step)}`;
}

/**
 * How many bytes of a phase have gone, as a clause after its count, for the
 * phase that counts them.
 *
 * Written the way the adding line writes its own, so the two halves of a drop
 * read alike. A phase with nothing to send says nothing,
 * as `0/0` would have said nothing worth reading.
 */
function bytesOf(step: Step): string {
  if (step.bytes === null || step.bytes.total === 0) {
    return '';
  }
  return ` — ${size(step.bytes.done)} of ${size(step.bytes.total)}`;
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
  settling: 'settling what an interrupted run left',
  scanning: 'reading the folders',
  packing: PACKING,
  uploading: 'sending',
  committing: 'committing to the Library',
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
function packed(freeze: Freeze | DisplacedFreeze): string {
  const packs = `${freeze.packs} ${freeze.packs === 1 ? 'Pack' : 'Packs'}`;
  const entries = `${freeze.entries} ${freeze.entries === 1 ? 'file' : 'files'}`;
  return freeze.entries === 0
    ? `${named(freeze.folder)} was already packed`
    : `packed ${entries} of ${named(freeze.folder)} into ${packs}`;
}

/**
 * What a run that succeeded still had to say, as one line.
 *
 * Shared by the sync, the freeze and the fill, because the findings are: a page
 * whose Entry is inside a Pack and a photograph whose Entry is are the same
 * sentence about the same state (spec: PK-14).
 */
function oneLine(findings: readonly Pick<Finding, 'path' | 'message'>[]): string {
  const [first] = findings;
  const rest = findings.length - 1;
  const named = first.path === null ? first.message : `${first.path} — ${first.message}`;
  return rest === 0 ? named : `${named} (and ${rest} more)`;
}

/**
 * A stopped run's line, with what the run found before it stopped after it.
 *
 * After, because what stopped the run is what the retry beside the line is
 * about; the findings are what the run did or met on the way — a Keyring
 * repair its commit made before it failed is one — and none of them is why it
 * stopped.
 */
function besides(line: string, findings: readonly Pick<Finding, 'path' | 'message'>[]): string {
  return findings.length === 0 ? line : `${line}; ${oneLine(findings)}`;
}

/**
 * The line shown while a drop's own files are still being written into the
 * folder this device maps.
 *
 * With how much of the request has gone, once the browser has said: a book of
 * several hundred pages is tens of megabytes, and a line with nothing moving in
 * it for as long as they take to send cannot be told from one that is stuck.
 *
 * Not the line for them going into the Library: what carries them there is the
 * sync or the freeze the drop armed, and that has its own, which takes over
 * from this one once the server answers.
 */
export function addingLine(files: number, folder: string, sent: Sent | null = null): string {
  const adding = `adding ${files} ${files === 1 ? 'file' : 'files'} to ${named(folder)}`;
  return sent === null ? `${adding}…` : `${adding} — ${size(sent.sent)} of ${size(sent.total)}…`;
}

/**
 * How much of a drop's request has gone, as the browser last said it.
 *
 * Bytes of the whole body, framing included, so the total is a little more than
 * the files come to; written the way the size column writes a file's length,
 * so the two read alike.
 */
export interface Sent {
  sent: number;
  total: number;
}

/**
 * The least time between two progress lines a drop puts on the screen.
 *
 * A browser says how far an upload has got many times a second, and a status
 * line redrawn at that rate is a blur nobody reads. A few times a second is
 * as often as the number can be taken in.
 */
export const PROGRESS_INTERVAL_MS = 250;

/**
 * `say`, told no more often than every `interval` milliseconds.
 *
 * The first report is passed on, so the line starts moving as soon as there is
 * a number, and so is the last — the whole body sent — so the line never stops
 * short of the total it names. Everything between is dropped when it arrives
 * sooner than `interval` after the last one passed on.
 */
export function paced(
  say: Progress,
  interval: number = PROGRESS_INTERVAL_MS,
  now: () => number = Date.now,
): Progress {
  let last: number | null = null;
  return (sent, total) => {
    const at = now();
    if (last === null || at - last >= interval || sent >= total) {
      last = at;
      say(sent, total);
    }
  };
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
 * What the screen reads to say that a folder added as a Pack now is packed
 * after the one already going up: they are packed one at a time (spec: PK-7),
 * and the server queues the second behind the first.
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
 * Whether to be polling the work route at all.
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
 *
 * A reconnect waiting on its consent page is one more: it ends in a browser
 * tab, on nobody's clock, and the page that opened the consent page learns how
 * it ended only by asking.
 *
 * A Library that has locked is the last, for the same reason. It is unlocked
 * in the desktop app's own window — asked for from this page or from the app's
 * tray, which tells this page nothing — and the page learns it is open again
 * only by asking, which is what brings its listing back without a reload. The
 * asking takes no key, so it neither counts as somebody being here nor keeps
 * anything unlocked; what it costs is one loopback request at the interval for
 * as long as a tab stays open over a locked Library.
 */
export function shouldPoll(
  readerOpen: boolean,
  fill: Fill | null,
  sync: Sync | null,
  freeze: Freeze | null = null,
  catalog: Catalog | null = null,
  reconnect: Reconnect | null = null,
  library: LibraryState | null = null,
): boolean {
  return (
    library === 'locked' ||
    reconnect?.state === 'waiting' ||
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
 * Whether to ask the work route now, given whether this page has been told
 * anything yet and whether there is anything to follow.
 *
 * Two reasons. The second is the interval's, which is [`shouldPoll`]: something
 * is in flight, so ask again in a moment. The first is the page coming up —
 * because "nothing in flight" is a statement about this page and not about the
 * server. A freeze Storage stopped is still stopped after a reload, with a
 * book's pages sitting in the folder and out of the Library, and a page that
 * came up without asking would show nothing about them and offer nothing to do
 * about them. So the work answer is asked for once at the start, alongside the
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
 * server is doing — and, since nothing about a failed work request is shown
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
