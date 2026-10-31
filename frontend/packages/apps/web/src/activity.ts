// What the Activity panel says, kept free of DOM so it is unit testable.
//
// Two groups. **In progress** is what is moving now: a drop's files being
// sent, and the runs the work answer reports as running, in the order of what
// is happening now — the drop, the run holding the turn at this device's
// pending work, the runs waiting for it, the fill, and the reader's own
// fetch. **Earlier** is what is
// over: the server's log of the work it finished (spec: LA-12), and the
// notices only this page knows of — a row asked to open that did not, a drop
// that came to nothing — merged into one list, newest first.
//
// Each finished entry says its outcome as a dot — success, failure, notice —
// and a failure that asking again could help carries the Retry that takes the
// work up again: at a fill, a sync or a freeze that stopped, at a folder a
// queue lost, and at a deletion another device's commit got in front of.
//
// A line in progress that can be stopped carries a Stop (spec: LA-34): an add
// — the drop being sent, or the sync or the freeze carrying one in — a freeze,
// a fill, a book or a folder still waiting in its queue, and the reader's own
// fetch. A stop is a request the run answers by
// ending (spec: LA-35), so a line whose Stop was pressed says it is stopping
// until the server's answer says the run has ended, and a run that has begun
// committing says it is finishing and offers none. Both come from the server's
// answer and not from this page's memory of the press, so a page reloaded
// mid-stop says the same.

import type {
  AddUnderWay,
  DeleteTarget,
  Fill,
  Freeze,
  Logged,
  Move,
  Refused,
  Step,
  StopOffer,
  StoppedAdd,
  StopTarget,
  TurnTaker,
} from '@coffret/api';

import { deletedLine, deletingLine } from './deleteEntries';
import { discardedBooksLine, discardedLine, fillLine, freezeLine, syncLine } from './fill';
import { ranOut } from './ranOut';
import { retryable, type Trouble } from './retry';
import { waitsFor, type Turns } from './turns';

/** What a finished entry came to, which is the colour of its dot. */
export type Outcome = 'success' | 'failure' | 'notice';

/** What one press of an entry's Retry asks for again. */
export type Retry =
  | { flow: 'fill'; folder: string }
  | { flow: 'freeze'; folder: string }
  | { flow: 'sync' }
  | { flow: 'delete'; target: DeleteTarget };

/** What one line's Stop asks for. */
export type StopAsk =
  /** A stop the server answers: an add, a freeze or a fill (spec: LA-35). */
  | { server: StopTarget }
  /**
   * The reader's own fetch of one file, which this page stops by withdrawing
   * its request: the file being placed is placed whole, and nothing more is
   * asked for it (spec: LA-37).
   */
  | { reader: true };

/** One line of **In progress**. */
export interface Running {
  /** Which work this is, stable across answers. */
  key: string;
  /** What it is doing, in the words the running line has always used. */
  text: string;
  /** How far it has got, from 0 to 1, and `null` where it cannot say. */
  progress: number | null;
  /** The Stop the line offers, and `null` where it offers none. */
  stop: StopAsk | null;
  /**
   * What the line says in place of a Stop: `stopping` once one was pressed
   * and the run has not answered, `finishing` once the run has begun
   * committing and so takes none — and `null` otherwise.
   */
  ending: 'stopping' | 'finishing' | null;
}

/** The words a line's `ending` is said in. */
export const ENDING_SAID: Record<NonNullable<Running['ending']>, string> = {
  stopping: 'Stopping…',
  finishing: 'Finishing…',
};

/** One line of **Earlier**. */
export interface Entry {
  /** Which entry this is, stable across answers and unique in the list. */
  key: string;
  /** When it ended, as ISO 8601. */
  at: string;
  outcome: Outcome;
  text: string;
  /** The second attempt it offers, and `null` where it offers none. */
  retry: Retry | null;
  /**
   * The words the Retry is named by — what it would do again — and `null`
   * where it offers none. Words and not "retry" alone, because one Storage
   * outage stops the fill, the sync and the freeze together, and identical
   * buttons side by side would leave a person guessing which is which.
   */
  retryAs: string | null;
  /** Whether this is a failure nothing has settled: what the badge counts. */
  standing: boolean;
  /** Whether a later run of the same work has taken its place. */
  settled: boolean;
  /** Whether what refused it is the permission Storage is reached through running out. */
  ranOut: boolean;
  /**
   * Whether it is this page's answer to a gesture made on it — a row that
   * would not open, a drop that came to nothing — rather than the server's
   * word on work it ran: the one kind of entry that takes the collapsed row
   * for a moment (see [`answering`](#answering)).
   */
  answers: boolean;
}

/**
 * A notice only this page knows of: the answer to a gesture made on this page
 * that no request carried to the server — or that reached no server at all.
 * Kept by the page, so a reload forgets it.
 */
export interface PageNotice {
  id: number;
  at: string;
  text: string;
  outcome: Exclude<Outcome, 'success'>;
  /** The deletion a *try again* beside it would ask about again. */
  retry: DeleteTarget | null;
  /** Whether its *try again* has been taken. */
  settled: boolean;
}

/** What the drop in flight and the runs reported running are, for the group. */
export interface InFlight extends Turns {
  /** The drop whose files are still being sent, if one is, as its line says it. */
  adding: string | null;
  /**
   * The name that drop was sent under, so its Stop can name it, and whether
   * that Stop has been pressed — `null` where no drop is being sent.
   */
  addingAs: { name: string; stopping: boolean } | null;
  /** The adds the server says it is carrying in (spec: LA-34). */
  adds: readonly AddUnderWay[];
  /** How far that drop has been sent, from 0 to 1, where that is known. */
  sent: number | null;
  fill: Fill | null;
  /** The file the reader is fetching, if it is. */
  fetching: string | null;
}

/** The three flows that take turns, in the order a tie between them is broken. */
const TIED: readonly TurnTaker[] = ['freeze', 'sync', 'delete'];

/**
 * **In progress**, top to bottom: the drop's own line, then the run holding
 * the turn, then the runs waiting for it, then the fill, then the reader's own
 * fetch. The fetch keeps its own line even beside a fill bringing its folder
 * over, since its Stop withdraws only the reader's request and the fill's Stop
 * stops the fill: one line for both would leave one of them out of reach.
 */
export function inProgress(flight: InFlight): Running[] {
  const running: Running[] = [];
  const as = flight.addingAs;
  if (flight.adding !== null) {
    const stopping =
      as !== null &&
      (as.stopping || flight.adds.some((add) => add.id === as.name && add.stopping));
    running.push({
      key: 'adding',
      text: flight.adding,
      progress: flight.sent,
      stop: as === null || stopping ? null : { server: { add: as.name } },
      ending: stopping ? 'stopping' : null,
    });
  }
  // An add another page sent, or this one before it was reloaded, still being
  // received while it is stopped: nothing else on this page says so.
  for (const add of flight.adds) {
    if (add.stage === 'receiving' && add.stopping && add.id !== as?.name) {
      running.push({
        key: `add:${add.id}`,
        text: `adding to ${shownAs(add.folder)}`,
        progress: null,
        stop: null,
        ending: 'stopping',
      });
    }
  }
  const order =
    flight.turn === null ? TIED : [flight.turn, ...TIED.filter((flow) => flow !== flight.turn)];
  for (const flow of order) {
    const line = runningLine(flow, flight);
    if (line !== null) {
      running.push(line);
    }
    if (flow === 'freeze') {
      running.push(...queuedBooks(flight.freeze));
    }
  }
  if (flight.fill?.status === 'filling') {
    const text = fillLine(flight.fill);
    if (text !== null) {
      const { done, total } = flight.fill;
      running.push({
        key: 'fill',
        text,
        progress: total === 0 ? null : done / total,
        ...stopOf(flight.fill.stop, { fill: flight.fill.run }),
      });
    }
  }
  running.push(...queuedFolders(flight.fill));
  if (flight.fetching !== null) {
    running.push({
      key: 'fetching',
      text: `fetching ${flight.fetching}…`,
      progress: null,
      stop: { reader: true },
      ending: null,
    });
  }
  return running;
}

/**
 * What a run's line offers about stopping it, as the server says it: a Stop
 * naming `target` where one is offered, and in its place what the run is doing
 * about one asked, or about its commit.
 */
function stopOf(offer: StopOffer | null, target: StopTarget): Pick<Running, 'stop' | 'ending'> {
  switch (offer) {
    case 'offered':
      return { stop: { server: target }, ending: null };
    case 'asked':
      return { stop: null, ending: 'stopping' };
    case 'committing':
      return { stop: null, ending: 'finishing' };
    case null:
      return { stop: null, ending: null };
  }
}

/**
 * A line for each book waiting in the freeze's queue, each with the Stop that
 * takes it off before its run begins (spec: LA-34) — or, for one holding adds
 * that was asked already, saying it is stopping until its run has undone them.
 */
function queuedBooks(freeze: Freeze | null): Running[] {
  return (freeze?.waiting ?? []).map((folder) => {
    const stopping = freeze?.stopping.includes(folder) ?? false;
    return {
      key: `queued-freeze:${folder}`,
      text: `waiting to pack ${shownAs(folder)}`,
      progress: null,
      stop: stopping ? null : { server: { queued_freeze: folder } },
      ending: stopping ? 'stopping' : null,
    };
  });
}

/**
 * A line for each folder waiting in the fill's queue, each with the Stop that
 * takes it off before its run begins (spec: LA-34).
 */
function queuedFolders(fill: Fill | null): Running[] {
  return (fill?.waiting ?? []).map((folder) => ({
    key: `queued-fill:${folder}`,
    text: `waiting to bring over ${shownAs(folder)}`,
    progress: null,
    stop: { server: { queued_fill: folder } },
    ending: null,
  }));
}

/** The line of one turn-taking flow's run while it is running, and `null` otherwise. */
function runningLine(flow: TurnTaker, turns: Turns): Running | null {
  const waits = waitsFor(flow, turns);
  switch (flow) {
    case 'freeze': {
      const run = turns.freeze;
      if (run?.status !== 'freezing') {
        return null;
      }
      const text = freezeLine(run, waits);
      return text === null
        ? null
        : {
            key: 'freeze',
            text,
            progress: stepped(run.step),
            ...stopOf(run.stop, { freeze: run.run }),
          };
    }
    case 'sync': {
      const run = turns.sync;
      if (run?.status !== 'syncing') {
        return null;
      }
      const text = syncLine(run, waits);
      return text === null
        ? null
        : {
            key: 'sync',
            text,
            progress: stepped(run.step),
            ...stopOf(run.stop, { sync: run.run }),
          };
    }
    case 'delete': {
      const run = turns.deletion;
      if (run?.status !== 'deleting') {
        return null;
      }
      // A deletion cannot be stopped (spec: LA-34).
      return {
        key: 'delete',
        text: deletingLine(run, waits),
        progress: stepped(run.step),
        stop: null,
        ending: null,
      };
    }
    // A move is never on record as running: its request answers it.
    case 'move':
      return null;
  }
}

/** How far a step says its phase has got: by bytes where it counts them, then by items. */
function stepped(step: Step | null): number | null {
  if (step === null) {
    return null;
  }
  if (step.bytes !== null && step.bytes.total > 0) {
    return step.bytes.done / step.bytes.total;
  }
  return step.total === null || step.total === 0 ? null : step.done / step.total;
}

/**
 * **Earlier**: the server's log and this page's own notices, newest first.
 *
 * Nothing is offered a second attempt while the Library is locked: each would
 * meet the same locked refusal until the Passphrase is given, and the panel
 * offers the unlock in their place.
 */
export function earlier(
  log: readonly Logged[],
  notices: readonly PageNotice[],
  locked: boolean,
): Entry[] {
  const entries = [
    ...log.map((logged) => entryOf(logged, locked)),
    ...notices.map((notice) => noticeEntry(notice, locked)),
  ];
  // ISO 8601 in UTC compares as text; a page's notices are stamped in the
  // same form. A stable sort keeps the server's own order between two of its
  // entries logged in the same second.
  return entries.sort((a, b) => (a.at < b.at ? 1 : a.at > b.at ? -1 : 0));
}

/** One entry of the server's log, as the panel draws it. */
export function entryOf(logged: Logged, locked: boolean): Entry {
  const retry = retryOf(logged, locked);
  const refused = refusalOf(logged);
  return {
    key: `server:${logged.id}`,
    at: logged.at,
    outcome: outcomeOf(logged),
    text: textOf(logged),
    retry,
    retryAs: retry === null ? null : wordsOf(retry, logged.kind === 'discarded'),
    standing: !logged.settled && outcomeOf(logged) === 'failure',
    settled: logged.settled,
    ranOut: ranOut(refused),
    answers: false,
  };
}

/** One of this page's notices, as the panel draws it. */
function noticeEntry(notice: PageNotice, locked: boolean): Entry {
  const retry: Retry | null =
    notice.retry === null || notice.settled || locked
      ? null
      : { flow: 'delete', target: notice.retry };
  return {
    key: `page:${notice.id}`,
    at: notice.at,
    outcome: notice.outcome,
    text: notice.text,
    retry,
    retryAs: retry === null ? null : wordsOf(retry, false),
    standing: !notice.settled && notice.outcome === 'failure',
    settled: notice.settled,
    ranOut: false,
    answers: true,
  };
}

/**
 * The second attempt one entry of the log offers, and `null` where it offers
 * none.
 *
 * A failure a later run of the same work has settled offers none: that run's
 * own entry is the one standing now. A stopped run is offered one only where
 * asking again could help (see [`retryable`](./retry)) — a root that refused,
 * an epoch, a lock are each met again by asking again. A folder a queue lost
 * never started, so asking for it is always worth it. A deletion is offered one
 * only where another device's commit got in front of it, which is the one
 * refusal whose own sentence says to run it again. A refused fetch is the
 * reader's, and is asked again by opening the page.
 */
export function retryOf(logged: Logged, locked: boolean): Retry | null {
  if (locked || logged.settled) {
    return null;
  }
  switch (logged.kind) {
    case 'fill':
      return retryable(logged.run) ? { flow: 'fill', folder: logged.run.folder } : null;
    case 'freeze':
      return retryable(logged.run) ? { flow: 'freeze', folder: logged.run.folder } : null;
    case 'sync':
      return retryable(logged.run) ? { flow: 'sync' } : null;
    case 'delete': {
      const target = deletedLine(logged.run)?.retry ?? null;
      return target === null ? null : { flow: 'delete', target };
    }
    case 'discarded':
      return { flow: logged.queue, folder: logged.folder };
    // A move is asked for again where it was made, not from the panel; and
    // an add a person stopped is added again by dropping it again.
    case 'move':
    case 'fetch':
    case 'add':
      return null;
  }
}

/**
 * The entries as the panel offers them now: a Retry whose work is under way or
 * queued already is taken away until that work ends.
 *
 * The failure stands until the run its Retry started settles it, but the
 * button goes as the server takes the request up — the run's own line in
 * **In progress** is the answer to the press, and a button still standing
 * beside the failure would read as a press that did nothing and invite a
 * second one, queuing the same folder twice.
 */
export function offeredNow(
  entries: readonly Entry[],
  flight: Pick<InFlight, 'fill' | 'sync' | 'freeze'>,
): Entry[] {
  return entries.map((entry) =>
    entry.retry !== null && underWay(entry.retry, flight)
      ? { ...entry, retry: null, retryAs: null }
      : entry,
  );
}

/** Whether the work one Retry would ask for is running or waiting its turn. */
function underWay(retry: Retry, flight: Pick<InFlight, 'fill' | 'sync' | 'freeze'>): boolean {
  switch (retry.flow) {
    case 'sync':
      return flight.sync?.status === 'syncing';
    case 'fill':
      return (
        (flight.fill?.status === 'filling' && flight.fill.folder === retry.folder) ||
        (flight.fill?.waiting.includes(retry.folder) ?? false)
      );
    case 'freeze':
      return (
        (flight.freeze?.status === 'freezing' && flight.freeze.folder === retry.folder) ||
        (flight.freeze?.waiting.includes(retry.folder) ?? false)
      );
    case 'delete':
      // A deletion's Retry goes through the confirmation again, which says
      // itself that one is running already.
      return false;
  }
}

/**
 * What a Retry is named by: what it would do again. A folder a queue lost is
 * named, because it is not the folder any line about a run names.
 */
export function wordsOf(retry: Retry, lost: boolean): string {
  switch (retry.flow) {
    case 'sync':
      return 'back up again';
    case 'fill':
      return lost ? `bring over ${shownAs(retry.folder)}` : 'bring over again';
    case 'freeze':
      return lost ? `pack ${shownAs(retry.folder)}` : 'pack again';
    case 'delete':
      return 'try again';
  }
}

/**
 * One second attempt as a key: two Retries with the same key ask for the same
 * work again, whichever entry offered them.
 */
export function retryKey(retry: Retry): string {
  switch (retry.flow) {
    case 'sync':
      return 'sync';
    case 'fill':
    case 'freeze':
      return `${retry.flow}:${retry.folder}`;
    case 'delete':
      return `delete:${JSON.stringify([retry.target.folders, retry.target.paths])}`;
  }
}

/**
 * The refused press, for as long as an entry of the log still offers what it
 * pressed — and `null` once none does.
 *
 * What ends a refusal is the offer ending: the work ending again and settling
 * the entry, whichever way it went. A poll that brings the same entry back is
 * not that, and taking the reason away there would leave the button standing
 * with nothing saying why pressing it did nothing.
 */
export function stillStanding(trouble: Trouble | null, log: readonly Logged[]): Trouble | null {
  if (trouble === null) {
    return null;
  }
  const pressed = retryKey(trouble.pressed);
  return log.some((logged) => {
    const offered = retryOf(logged, false);
    return offered !== null && retryKey(offered) === pressed;
  })
    ? trouble
    : null;
}

/**
 * What one entry came to. A run that stopped, a folder that never started and
 * a fetch refused are failures. A run that finished and found something — a
 * file not backed up, an Entry not placed, a Keyring repaired — is a notice:
 * something to be told, and not the failure of anything anybody asked for. A
 * file a Pack drop reserved is on its way to its Pack, and a copy a deletion
 * discarded is what the person asked for, so neither alone is a notice.
 */
function outcomeOf(logged: Logged): Outcome {
  switch (logged.kind) {
    case 'fill': {
      const run = logged.run;
      return run.status === 'stopped'
        ? 'failure'
        : run.status === 'cancelled'
          ? 'notice'
        : run.declined.length + run.findings.length > 0
          ? 'notice'
          : 'success';
    }
    case 'sync': {
      const run = logged.run;
      const found = run.findings.filter((finding) => finding.surfaced !== 'AwaitsPack');
      return run.status === 'stopped' ? 'failure' : found.length > 0 ? 'notice' : 'success';
    }
    case 'freeze':
      return logged.run.status === 'stopped'
        ? 'failure'
        : logged.run.status === 'cancelled' || logged.run.findings.length > 0
          ? 'notice'
          : 'success';
    case 'delete': {
      const run = logged.run;
      const found = run.findings.filter((finding) => finding.surfaced !== 'Discarded');
      return run.status === 'stopped'
        ? 'failure'
        : found.length + run.refused.length + run.missing.length > 0
          ? 'notice'
          : 'success';
    }
    case 'move': {
      // A file that followed, or left this device, is what the person asked for.
      const run = logged.run;
      const found = run.findings.filter(
        (finding) => finding.surfaced !== 'Relocated' && finding.surfaced !== 'RelocationLeft',
      );
      return run.status === 'stopped'
        ? 'failure'
        : found.length > 0 || run.unrelocated !== null
          ? 'notice'
          : 'success';
    }
    case 'discarded':
    case 'fetch':
      return 'failure';
    // What a person asked for, and so no failure; worth being told of all the
    // same, since it is the answer to their Stop.
    case 'add':
      return 'notice';
  }
}

/** What refused an entry's work, where something did. */
function refusalOf(logged: Logged): Refused | null {
  switch (logged.kind) {
    case 'fill':
    case 'sync':
    case 'freeze':
    case 'delete':
    case 'move':
      return logged.run.stopped;
    case 'fetch':
      return logged.refused;
    case 'discarded':
    case 'add':
      return null;
  }
}

/**
 * What one entry says: the sentence the running line says of a run once it is
 * over, so a run reads the same in the panel as it did as it ended.
 */
function textOf(logged: Logged): string {
  switch (logged.kind) {
    case 'fill':
      return fillLine(logged.run) ?? broughtOver(logged.run);
    case 'sync':
      return syncLine(logged.run) ?? '';
    case 'freeze':
      return freezeLine(logged.run) ?? '';
    case 'delete':
      return deletedLine(logged.run)?.text ?? '';
    case 'move':
      return movedLine(logged.run);
    case 'discarded':
      return (
        (logged.queue === 'fill'
          ? discardedLine([logged.folder])
          : discardedBooksLine([logged.folder])) ?? ''
      );
    case 'fetch':
      return `could not fetch ${logged.path} — ${logged.refused.message}`;
    case 'add':
      return stoppedLine(logged.add);
  }
}

/**
 * What an add a person stopped came to (spec: LA-36), named by the folder it
 * was made onto as every other line names its folder: that nothing of it is
 * in the Library — or, where a run under way beside it had already carried
 * some of it in, how much — and how many of its files could not be taken back
 * out of the folder, where any could not. Every other file it wrote is gone,
 * and a file it had written over is back as it was; one this device could not
 * take away is the folder's like any copied there, so the next sync carries
 * it in unless the person takes it away first. An add whose files could not
 * be put back at all is never stopped: its stop is refused, and says why.
 */
export function stoppedLine(add: StoppedAdd): string {
  const added =
    add.committed === 0
      ? 'nothing was added to the Library'
      : `${files(add.committed)} had been added to the Library already`;
  const left =
    add.left === 0
      ? ''
      : `; ${files(add.left)} could not be taken back out of the folder, and the next sync adds ${add.left === 1 ? 'it' : 'them'}`;
  return `stopped adding to ${shownAs(add.folder)} — ${added}${left}`;
}

/** A count of files, as a line says it. */
function files(count: number): string {
  return count === 1 ? '1 file' : `${count} files`;
}

/**
 * What a move that ended says: what it moved, and what kept this device's
 * files from following where something did; or what stopped it.
 */
function movedLine(run: Move): string {
  const named =
    run.pairs.length === 1
      ? `${run.pairs[0].from} → ${run.pairs[0].to}`
      : `${run.pairs.length} items`;
  if (run.status === 'stopped') {
    return `could not move ${named} — ${run.stopped.message}`;
  }
  const left = run.left === 0 ? '' : `; ${run.left} no longer on this device`;
  const unrelocated =
    run.unrelocated === null
      ? ''
      : `; this device's files did not follow yet — ${run.unrelocated.message}`;
  return `moved ${named}${left}${unrelocated}`;
}

/**
 * What a fill that brought everything over says. The running line has nothing
 * to say of one — while it was the run on record its news was the rows turning
 * present — but an entry in a list of what happened has to say what it was.
 */
function broughtOver(run: Fill): string {
  const files = run.done === 1 ? '1 file' : `${run.done} files`;
  return `brought over ${files} in ${shownAs(run.folder)}`;
}

/**
 * The collapsed panel's one row: a summary of what is in progress, how far the
 * first of it has got, and how many failures stand.
 *
 * The first thing in progress speaks for the rest, with a count of them after
 * it. Where nothing is, the newest finished entry stands there instead — the
 * answer to whatever just ended, or to the gesture just made — so the row is
 * never blank while there is something to have read. Where nothing is running
 * and nothing has ended it says nothing (an empty `text`): being idle is the
 * usual state, and words for it on show would only stand in the way of the
 * words that matter when they come.
 *
 * An `answer` to the gesture just made (see [`answering`](#answering)) goes
 * ahead of both: it is said in place of the summary, while the bar for what is
 * running and the count of failures stay as they were, since neither is what
 * the gesture asked about and both are still true.
 */
export function collapsed(
  running: readonly Running[],
  entries: readonly Entry[],
  answer: Entry | null = null,
): Row {
  const failed = entries.filter((entry) => entry.standing).length;
  const first = running[0];
  const progress = first?.progress ?? null;
  if (answer !== null) {
    return { text: answer.text, progress, failed, said: answer };
  }
  if (first !== undefined) {
    const more = running.length - 1;
    return {
      text: more === 0 ? first.text : `${first.text} · ${more} more`,
      progress,
      failed,
      said: null,
    };
  }
  const newest = entries[0];
  return { text: newest?.text ?? '', progress: null, failed, said: newest ?? null };
}

/** What the collapsed row shows. */
export interface Row {
  text: string;
  /** How far the first thing in progress has got, and `null` where it cannot say or nothing is. */
  progress: number | null;
  /** How many failures stand: the badge. */
  failed: number;
  /**
   * The finished entry the row says, whose outcome is the colour it is said
   * in — and `null` where the row says what is in progress, or that nothing is.
   */
  said: Entry | null;
}

/**
 * How long an answer to a gesture holds the collapsed row, in milliseconds,
 * where no later gesture takes it away first: long enough to read one line,
 * short enough that the work going on is soon said again.
 */
export const ANSWER_HOLDS = 6000;

/**
 * The answer to the gesture just made that the collapsed row says now, with
 * the moment it stops saying it — and `null` where there is none.
 *
 * The row says what is in progress while anything is, and an answer filed
 * only under **Earlier** is one nobody sees — an Enter on a file the reader
 * cannot show would seem to do nothing. So the newest of this page's own notices takes the
 * row for [`ANSWER_HOLDS`](#answer_holds), or until the next gesture, which
 * asks something new and is owed its own answer. `gestured` is when the last
 * gesture was made, in milliseconds since the epoch: a notice from before it
 * answered an earlier one. A notice settled already — its *try again* taken —
 * has had its answer.
 */
export function answering(
  entries: readonly Entry[],
  now: number,
  gestured: number,
): { entry: Entry; until: number } | null {
  const notice = entries.find((entry) => entry.answers);
  if (notice === undefined || notice.settled) {
    return null;
  }
  const at = Date.parse(notice.at);
  const until = at + ANSWER_HOLDS;
  return Number.isNaN(at) || at < gestured || now >= until ? null : { entry: notice, until };
}

/**
 * The failure the badge takes a person to: the newest one standing, which is
 * the first of them in **Earlier** — and `null` where none stands.
 */
export function firstStanding(entries: readonly Entry[]): Entry | null {
  return entries.find((entry) => entry.standing) ?? null;
}

/** The badge for the failures standing, and `null` where none does. */
export function failedBadge(failed: number): string | null {
  return failed === 0 ? null : `${failed} failed`;
}

/** The Library root has no name of its own, and is not called the empty string. */
function shownAs(folder: string): string {
  return folder === '' ? 'the Library root' : folder;
}

/** A moment as the panel's time column says it: hours and minutes, in the reader's zone. */
export function clock(at: string): string {
  const date = new Date(at);
  if (Number.isNaN(date.getTime())) {
    return '';
  }
  return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
}
