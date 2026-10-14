// Which of the three turn-taking flows finished last, kept free of DOM so it is
// unit testable.
//
// The status bar has room for one line, and once nothing is running the line
// it owes is the result of whatever finished most recently — the Pack a drop
// made, then the file added one by one after it, then the deletion after that.
// A fixed order of flows cannot say that: it left a finished freeze's `packed …`
// standing over every later add and delete.
//
// The work answer carries no clock, and the run numbers are counted per flow,
// so they cannot be compared across flows either. What this page can see is
// when a run becomes finished: a run of a flow that is over, numbered higher
// than the last one of that flow it saw over, finished since the previous
// answer. So the order is the order the answers reported the endings in, kept
// here. Two endings in one answer cannot be told apart, and take the order the
// bar used to rank the flows in.

import type { Delete, Freeze, Sync, TurnTaker } from '@coffret/api';

/** The order finished runs were reported in, and what it was measured from. */
export interface Finished {
  /**
   * The process the run numbers below are counted by, and `null` before any
   * answer has said. A new process counts from 1 again, so a name that changes
   * forgets everything here, as it forgets what a tab put away.
   */
  server: string | null;
  /** The flows whose run on record is over, the most recently finished first. */
  order: readonly TurnTaker[];
  /** The highest run of each flow seen over, and `0` where none has been. */
  seen: Record<TurnTaker, number>;
}

/** Nothing seen finish, which is where every tab starts. */
export const NOTHING_FINISHED: Finished = {
  server: null,
  order: [],
  seen: { sync: 0, freeze: 0, delete: 0 },
};

/**
 * The order the bar ranks two flows ending in one answer by: the freeze
 * first, then the sync, then the deletion — the order it ranked every line in
 * before it ranked by recency, and the larger thing a person asked for first.
 */
export const TIED: readonly TurnTaker[] = ['freeze', 'sync', 'delete'];

/** The three runs an answer carries, and the process that sent it. */
export interface Ended {
  server: string;
  sync: Sync | null;
  freeze: Freeze | null;
  delete: Delete | null;
}

/** Whether a run is over: done, stopped, or anything else that is not running. */
function isOver(run: Sync | Freeze | Delete | null): boolean {
  return (
    run !== null &&
    run.status !== 'syncing' &&
    run.status !== 'freezing' &&
    run.status !== 'deleting'
  );
}

/**
 * The order after one more answer.
 *
 * A flow whose run is over and numbered above the last one of it seen over has
 * finished since, and goes to the front. Where several did in one answer, the
 * one {@link TIED} ranks first ends up in front.
 */
export function finishedAfter(before: Finished, ended: Ended): Finished {
  const from =
    before.server === ended.server ? before : { ...NOTHING_FINISHED, server: ended.server };
  let order = from.order;
  const seen = { ...from.seen };
  for (const flow of [...TIED].reverse()) {
    const run = ended[flow];
    if (run === null || !isOver(run) || run.run <= seen[flow]) {
      continue;
    }
    seen[flow] = run.run;
    order = [flow, ...order.filter((other) => other !== flow)];
  }
  // The same value where nothing finished, so a state holding it does not
  // change on every answer.
  return order === from.order ? from : { server: ended.server, order, seen };
}

/**
 * Every flow, the most recently finished first: the ones seen finish in the
 * order they did, then the rest in the {@link TIED} order.
 *
 * What the bar walks to find the latest result it has a line for, so a line put
 * away gives its place to the one that finished before it.
 */
export function latestFirst(order: readonly TurnTaker[]): readonly TurnTaker[] {
  return [...order, ...TIED.filter((flow) => !order.includes(flow))];
}
