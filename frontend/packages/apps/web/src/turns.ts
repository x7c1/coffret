// What a run waiting its turn at this device's pending work is waiting for,
// kept free of DOM so it is unit testable.
//
// A sync, a freeze and a deletion each own this device's pending work for the
// whole of a run, so the server runs them one at a time, in the order they were
// armed: one armed while another is running is on record as running, with no
// step yet, until its turn comes. Said on its line, so a line that stays put
// while a book is packed reads as waiting rather than stuck — and said by one
// function for all three lines, so no two of them can call the same work by
// different words.

import type { Delete, Freeze, Sync } from '@coffret/api';

/** One of the three flows that take turns at this device's pending work. */
export type TurnTaker = 'sync' | 'freeze' | 'deletion';

/**
 * What a run is waiting for, in the word its line says it in, or `null` where
 * it waits for nothing.
 */
export type WaitsFor = 'packing' | 'backup' | 'deletion' | null;

/** What each of the three flows is doing, as the work answer says it. */
export interface Turns {
  freeze: Freeze | null;
  sync: Sync | null;
  deletion: Delete | null;
}

/** One flow running, with the word for it and whether it has said a step. */
interface Running {
  taker: TurnTaker;
  word: Exclude<WaitsFor, null>;
  /** Whether it is under way rather than waiting for its own turn. */
  moving: boolean;
}

/**
 * What `own` waits for while it is running with no step: the other flow that
 * is running, or `null` where none is.
 *
 * Where two others are running, one of them is itself waiting, and the one
 * holding the turn is the one that has said how far it has got. Before either
 * has said, the order is packing, then backup, then deletion — the order the
 * status bar already ranks their lines in.
 */
export function waitsFor(own: TurnTaker, turns: Turns): WaitsFor {
  const running: Running[] = [];
  if (turns.freeze?.status === 'freezing') {
    running.push({ taker: 'freeze', word: 'packing', moving: turns.freeze.step !== null });
  }
  if (turns.sync?.status === 'syncing') {
    running.push({ taker: 'sync', word: 'backup', moving: turns.sync.step !== null });
  }
  if (turns.deletion?.status === 'deleting') {
    running.push({ taker: 'deletion', word: 'deletion', moving: turns.deletion.step !== null });
  }
  const others = running.filter((other) => other.taker !== own);
  return (others.find((other) => other.moving) ?? others[0])?.word ?? null;
}

/**
 * The clause a waiting run's line ends on, or nothing where it waits for
 * nothing.
 */
export function waitingClause(waits: WaitsFor): string {
  return waits === null ? '' : ` — waiting for the ${waits} under way to finish`;
}
