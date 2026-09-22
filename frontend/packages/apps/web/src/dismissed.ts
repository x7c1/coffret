// Putting away what a run that is over had to say, kept free of DOM so it is
// unit testable.
//
// The status bar has room for one line and an order of precedence among the
// three flows, and the lines a finished run leaves are the ones with nothing to
// end them: a sync that carried files in and found something keeps its sentence
// for as long as the tab is open, and every fill's progress afterwards is drawn
// underneath it. The findings are worth saying — they are the only place a
// person is told a dropped file is not backed up — and they are worth saying
// once. A person who has read one has no way to say so, which is what this is.
//
// What is remembered is the run, not the sentence. Two runs of one flow can
// come to exactly the same words, and a screen that remembered the words would
// silently swallow the second one's — which for a finding about a file that is
// still not backed up is the whole loss. Every activity carries the run it
// belongs to, counted by the server from the moment it started, so a line put
// away stays away until the next run of that flow replaces it.
//
// Only a run that is over can be put away. A line about work happening right
// now is not something a person is finished with, and hiding it would leave the
// next poll to bring it straight back.
//
// The folders a worker that died threw away are the other thing on the bar with
// nothing to end it, and they are not a run: nothing counted them, so there is
// no number to remember one by. What they have is their name, and that is what
// is remembered. The runs a later one took the record from are put away by name
// too, although they are runs: the number a dismissal spends is the flow's
// latest, and one of those is never that — so putting the running flow's line
// away would silently take every stopped run before it off the screen. A name would stay put away for the life of the tab, where a
// run number is spent by the next run — so a name is forgotten the moment the
// server stops offering that folder, which is what arming it does. A folder
// thrown away a second time is an offer made again, and comes back with it.
//
// Both of those hold within one server. A run number is this process's count
// from 1 and a dropped folder is this process's offer, so neither means
// anything about the next process — and a locked Library is unlocked by typing
// the Passphrase and starting the server again, which makes a tab that outlives
// a restart the ordinary case rather than the strange one. So every answer says
// which process gave it, and a name that has changed empties all of this: see
// `servedBy`.

import type { Fill, Freeze, Sync } from '@coffret/api';

/** Which of the three flows a line belongs to. */
export type Flow = 'fill' | 'sync' | 'freeze';

/**
 * Which of the two flows that keep a queue a lost folder belongs to.
 *
 * The sync has none: it walks the device's mappings rather than a folder
 * somebody chose, so there is nothing for it to lose and nothing to offer.
 */
export type Queue = 'fill' | 'freeze';

/** What one press of the dismiss button puts away. */
export type Dismissable =
  /** The line of the run of that flow now on record. */
  | { kind: 'line'; flow: Flow }
  /** The notice about the folders that queue lost, and the offers in it. */
  | { kind: 'dropped'; queue: Queue; folders: readonly string[] }
  /**
   * The notice about the runs that stopped and that a later one took the record
   * from, and the offers in it.
   *
   * Put away by name like the one above it rather than by run number like a
   * line, although these are runs and carry one. The number a dismissal spends
   * is the flow's latest, and a displaced run is never that: putting the running
   * flow's line away would take every stopped run before it off the screen
   * unasked, which is the loss this whole notice exists against.
   */
  | { kind: 'stopped'; queue: Queue; folders: readonly string[] };

/** What a tab has been told it need not show again. */
export interface Dismissed {
  /**
   * The process all of this is about, and `null` before any answer has said.
   *
   * Everything below it is one server's: a run number counts that process's
   * runs from 1, and a dropped folder is that process's offer. Carried here
   * rather than remembered beside it, so that what has been put away and whose
   * runs they were cannot come apart.
   */
  server: string | null;
  /**
   * The last run of each flow whose line has been put away.
   *
   * A number and not a flag, because what is put away is one run: `0` is the run
   * no server ever has, so it reads as nothing put away without a second state
   * meaning the same thing.
   */
  runs: Record<Flow, number>;
  /**
   * The folders each queue lost whose offer has been put away.
   *
   * Names, because a lost folder is not a run and has no number. They are kept
   * only for as long as the server goes on offering them — see
   * [`stillOffered`].
   */
  folders: Record<Queue, readonly string[]>;
}

/** Nothing put away, which is where every tab starts. */
export const NOTHING_DISMISSED: Dismissed = {
  server: null,
  runs: { fill: 0, sync: 0, freeze: 0 },
  folders: { fill: [], freeze: [] },
};

/**
 * The same, emptied where `server` is not the process it was put away under.
 *
 * What ends every dismissal at once, and the only thing that can: run numbers
 * start again at 1 with each process, so the new server's runs 1..N would be
 * hidden by what somebody put away before the restart — a fill's line and its
 * declined and failed chips, and the one sentence that says a sync did not back
 * a file up. Comparing the numbers cannot tell that case from the one the `<=`
 * in [`isPutAway`] is for: a run arriving lower than a dismissed one is equally
 * an answer that was already in flight when the button was pressed.
 *
 * Applied to every answer, because every answer carries the name. The first one
 * a tab hears is a change too — from nothing to this server — and nothing is
 * lost to it, since a run can only have been put away after an answer named it.
 */
export function servedBy(dismissed: Dismissed, server: string): Dismissed {
  return dismissed.server === server ? dismissed : { ...NOTHING_DISMISSED, server };
}

/** Records that the line of `run` of `flow` has been read and put away. */
export function putAway(dismissed: Dismissed, flow: Flow, run: number): Dismissed {
  return run <= dismissed.runs[flow]
    ? dismissed
    : { ...dismissed, runs: { ...dismissed.runs, [flow]: run } };
}

/**
 * Records that the offer of `folders` from `queue` has been read and put away.
 *
 * All of them at once, because they are one notice: the line names them
 * together and the buttons beside it are what it names. A person who wants one
 * of them presses its button, which takes that folder off the server's list —
 * and puts the rest away with one press afterwards.
 */
export function putAwayFolders(
  dismissed: Dismissed,
  queue: Queue,
  folders: readonly string[],
): Dismissed {
  const away = dismissed.folders[queue];
  const added = folders.filter((folder) => !away.includes(folder));
  return added.length === 0
    ? dismissed
    : { ...dismissed, folders: { ...dismissed.folders, [queue]: [...away, ...added] } };
}

/**
 * The same, minus the folders `queue` is no longer offering.
 *
 * What ends a dismissal, and the counterpart of a run number being spent by the
 * next run. A folder leaves the server's list when somebody takes it up, so
 * forgetting it there is forgetting it here — and a folder thrown away a second
 * time arrives as an offer nobody has answered rather than as one put away
 * before it was made.
 *
 * Applied to every answer that carries the list rather than to the arming
 * alone, because arming is not always this tab's doing: opening a file in a
 * folder takes it up too, and the list is where either of them shows.
 */
export function stillOffered(
  dismissed: Dismissed,
  queue: Queue,
  offered: readonly string[],
): Dismissed {
  const away = dismissed.folders[queue];
  const kept = away.filter((folder) => offered.includes(folder));
  return kept.length === away.length
    ? dismissed
    : { ...dismissed, folders: { ...dismissed.folders, [queue]: kept } };
}

/** The folders `queue` lost that are still worth showing. */
export function shownFolders(
  dismissed: Dismissed,
  queue: Queue,
  dropped: readonly string[],
): readonly string[] {
  return dropped.filter((folder) => !dismissed.folders[queue].includes(folder));
}

/**
 * The same for the runs `queue` stopped on, which are put away by the folder
 * they name.
 *
 * One set of names for both notices, because one folder is never in both at
 * once: a folder the queue lost is one nothing started on, a displaced run is
 * one that ran, and taking a folder up is what moves it off either list.
 */
export function shownRuns<R extends { folder: string }>(
  dismissed: Dismissed,
  queue: Queue,
  runs: readonly R[],
): readonly R[] {
  return runs.filter((run) => !dismissed.folders[queue].includes(run.folder));
}

/**
 * Whether what this run has to say has been put away.
 *
 * `<=` rather than `===`: an answer that arrived from before the dismissal —
 * a request already in flight when the button was pressed — is about a run
 * somebody has finished with, and bringing its line back for one tick would be
 * a dismissal that flickers.
 */
export function isPutAway(
  dismissed: Dismissed,
  flow: Flow,
  run: Fill | Sync | Freeze | null,
): boolean {
  return run !== null && run.run <= dismissed.runs[flow];
}

/**
 * Whether a run's line can be put away at all.
 *
 * Only one that is over. A line about work happening now is not something a
 * person has finished with — the counts in it are still moving — and putting it
 * away would last exactly until the next answer arrived.
 *
 * A fill that was superseded is over in this sense too, although it placed only
 * part of its folder: nothing is going to take it up again on its own, so its
 * line is as final as a finished one's.
 */
export function canPutAway(run: Fill | Sync | Freeze | null): run is Fill | Sync | Freeze {
  return (
    run !== null &&
    run.status !== 'filling' &&
    run.status !== 'syncing' &&
    run.status !== 'freezing'
  );
}
