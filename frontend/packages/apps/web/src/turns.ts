// What a run waiting its turn at this device's pending work is waiting for,
// kept free of DOM so it is unit testable.
//
// A sync, a freeze and a deletion each own this device's pending work for the
// whole of a run, so the server runs them one at a time, in the order the runs
// ask for the turn — a run asks as its flow takes it up. The work answer says
// which flow's run holds the turn (`turn`) and whether each run is still
// waiting for it (`waits`), and this reads those two and nothing else.
//
// Said on its line, so a line that stays put while a book is packed reads as
// waiting rather than stuck — and said by one function for all three lines, so
// no two of them can call the same work by different words.

import type { Delete, Freeze, Sync, TurnTaker } from '@coffret/api';

/**
 * What a run is waiting for, in the word its line says it in, or `null` where
 * it waits for nothing.
 */
export type WaitsFor = 'packing' | 'backup' | 'deletion' | 'move' | null;

/** What each of the three flows is doing, as the work answer says it. */
export interface Turns {
  /** Which flow's run holds the turn, and `null` where nobody does. */
  turn: TurnTaker | null;
  freeze: Freeze | null;
  sync: Sync | null;
  deletion: Delete | null;
}

/** The word each flow is waited for in. */
const WORD: Record<TurnTaker, Exclude<WaitsFor, null>> = {
  freeze: 'packing',
  sync: 'backup',
  delete: 'deletion',
  move: 'move',
};

/** The run of `flow` on record, whichever of the three it is. */
export function runOf(flow: TurnTaker, turns: Turns): Freeze | Sync | Delete | null {
  switch (flow) {
    case 'freeze':
      return turns.freeze;
    case 'sync':
      return turns.sync;
    case 'delete':
      return turns.deletion;
    // A move answers the request that asked for it, so no run of it is ever
    // on record as running.
    case 'move':
      return null;
  }
}

/**
 * What the run of `own` waits for: the flow holding the turn, where the server
 * says the run waits and somebody else holds it, and `null` otherwise.
 *
 * Never read off the step. A run the server says is not waiting is not
 * waiting, whatever it has or has not said about how far it has got; and a run
 * that waits while nobody holds the turn — the moment between one run giving
 * it back and the next taking it — is waiting for nobody in particular, and
 * says nothing.
 */
export function waitsFor(own: TurnTaker, turns: Turns): WaitsFor {
  const run = runOf(own, turns);
  if (run === null || !run.waits || turns.turn === null || turns.turn === own) {
    return null;
  }
  return WORD[turns.turn];
}

/**
 * What a run armed now would wait for: whoever holds the turn, in the word
 * its line would say it in, and `null` where nobody does.
 *
 * What a confirmation says before anything is armed, so there is no run of
 * its own to ask about.
 */
export function heldBy(turn: TurnTaker | null): WaitsFor {
  return turn === null ? null : WORD[turn];
}

/**
 * The clause a waiting run's line ends on, or nothing where it waits for
 * nothing.
 */
export function waitingClause(waits: WaitsFor): string {
  return waits === null ? '' : ` — waiting for the ${waits} under way to finish`;
}
