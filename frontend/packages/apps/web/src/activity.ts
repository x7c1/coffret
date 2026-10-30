// What the Activity panel says, kept free of DOM so it is unit testable.
//
// Two groups. **In progress** is what is moving now: a drop's files being
// sent, and the runs the work answer reports as running, in the order of what
// is happening now — the drop, the run holding the turn at this device's
// pending work, the runs waiting for it, the fill, and the reader's own fetch
// where nothing else is moving. **Earlier** is what is
// over: the server's log of the work it finished (spec: LA-12), and the
// notices only this page knows of — a row asked to open that did not, a drop
// that came to nothing — merged into one list, newest first.
//
// Each finished entry says its outcome as a dot — success, failure, notice —
// and a failure that asking again could help carries the Retry that takes the
// work up again: at a fill, a sync or a freeze that stopped, at a folder a
// queue lost, and at a deletion another device's commit got in front of.

import type { DeleteTarget, Fill, Logged, Refused, Step, TurnTaker } from '@coffret/api';

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

/** One line of **In progress**. */
export interface Running {
  /** Which work this is, stable across answers. */
  key: string;
  /** What it is doing, in the words the running line has always used. */
  text: string;
  /** How far it has got, from 0 to 1, and `null` where it cannot say. */
  progress: number | null;
}

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
 * the turn, then the runs waiting for it, then the fill — and the reader's own
 * fetch only where nothing else is moving, since a fill under way is that same
 * fetch's folder being brought over, and both would report one fetch twice.
 */
export function inProgress(flight: InFlight): Running[] {
  const running: Running[] = [];
  if (flight.adding !== null) {
    running.push({ key: 'adding', text: flight.adding, progress: flight.sent });
  }
  const order =
    flight.turn === null ? TIED : [flight.turn, ...TIED.filter((flow) => flow !== flight.turn)];
  for (const flow of order) {
    const line = runningLine(flow, flight);
    if (line !== null) {
      running.push(line);
    }
  }
  if (flight.fill?.status === 'filling') {
    const text = fillLine(flight.fill);
    if (text !== null) {
      const { done, total } = flight.fill;
      running.push({ key: 'fill', text, progress: total === 0 ? null : done / total });
    }
  }
  if (running.length === 0 && flight.fetching !== null) {
    running.push({ key: 'fetching', text: `fetching ${flight.fetching}…`, progress: null });
  }
  return running;
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
      return text === null ? null : { key: 'freeze', text, progress: stepped(run.step) };
    }
    case 'sync': {
      const run = turns.sync;
      if (run?.status !== 'syncing') {
        return null;
      }
      const text = syncLine(run, waits);
      return text === null ? null : { key: 'sync', text, progress: stepped(run.step) };
    }
    case 'delete': {
      const run = turns.deletion;
      if (run?.status !== 'deleting') {
        return null;
      }
      return { key: 'delete', text: deletingLine(run, waits), progress: stepped(run.step) };
    }
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
    case 'fetch':
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
        : logged.run.findings.length > 0
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
    case 'discarded':
    case 'fetch':
      return 'failure';
  }
}

/** What refused an entry's work, where something did. */
function refusalOf(logged: Logged): Refused | null {
  switch (logged.kind) {
    case 'fill':
    case 'sync':
    case 'freeze':
    case 'delete':
      return logged.run.stopped;
    case 'fetch':
      return logged.refused;
    case 'discarded':
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
    case 'discarded':
      return (
        (logged.queue === 'fill'
          ? discardedLine([logged.folder])
          : discardedBooksLine([logged.folder])) ?? ''
      );
    case 'fetch':
      return `could not fetch ${logged.path} — ${logged.refused.message}`;
  }
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
 * never blank while there is something to have read.
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
  return { text: newest?.text ?? NOTHING_RUNNING, progress: null, failed, said: newest ?? null };
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

/** What the collapsed row says where nothing is in progress and nothing has ended. */
export const NOTHING_RUNNING = 'nothing running';

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
