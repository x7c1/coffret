// What a drop onto the folder on the screen will do, said while the files are
// still in the air.
//
// What a drop is added as follows what was dropped, not the folder it is
// dropped onto. Files on their own are added one at a time: the sync carries
// each one in as a Container of its own. A drop holding a folder is asked about
// once it has been read and before anything is sent — whether it goes in as a
// Pack, packed together in one batch (spec: PK-7), or as its files one by one.
// A browser does not say while a drag is in the air whether it carries a
// folder, so the line says both. It says the refusal there too, where a folder
// no mapping of this device reaches has nowhere to put any of them (spec: EP-9).
//
// Kept free of DOM and of React so it is unit testable, as `unmapped.ts` beside
// it is.

import { NO_FOLDER_HERE } from '@coffret/api';

import { NOTHING_AT_THIS_PATH } from './unmapped';

/**
 * What a drop onto a folder comes to.
 *
 * - `taken`: the drop is added — a folder in it asked about first.
 * - `taken_after`: the same, while a freeze is running already, so a folder
 *   added as a Pack now is packed after that one.
 * - `refused`: nothing is taken, because there is nowhere to put it.
 */
export type DropOutcome = 'taken' | 'taken_after' | 'refused';

/** What the screen knows about the folder a drag is over. */
export interface DropTarget {
  /** Whether a mapping of this device reaches the folder. */
  mapped: boolean;
  /** Whether a book is being packed right now, wherever it is. */
  freezing: boolean;
}

/**
 * The outcome a drop onto the folder would come to.
 *
 * Read off the same fact the drop is turned away by, and nothing else: `mapped`
 * is what refuses a drop, so the line cannot promise one thing while the drop
 * does another.
 *
 * A folder added as a Pack while another freeze runs is still taken — the
 * server queues the second freeze behind the first, and they run one at a
 * time — so what changes is the order, and that is what is said.
 */
export function dropOutcome(target: DropTarget): DropOutcome {
  if (!target.mapped) {
    return 'refused';
  }
  return target.freezing ? 'taken_after' : 'taken';
}

/**
 * What a folder added as a Pack while another freeze runs comes to: freezes
 * run one at a time (spec: PK-7), so this one waits.
 */
export const PACKED_AFTER =
  'a book is being packed already, and books are packed in turn, so a folder added as a ' +
  'Pack now is packed after it';

/** What letting go of files over a folder that takes them does. */
export const TAKEN =
  'drop to add — a folder is asked about first, as a Pack or its files one by one; files ' +
  'on their own are added one by one';

/**
 * The line the list shows while files are dragged over it.
 *
 * `held` is whether the Library holds the folder at all, and decides only the
 * refusal's reason — the same reason the answer to letting go gives, so the
 * line before the drop and the notice after it are one fact said twice.
 */
export function dropLine(outcome: DropOutcome, held: boolean): string {
  switch (outcome) {
    case 'taken':
      return TAKEN;
    case 'taken_after':
      return `${TAKEN} — ${PACKED_AFTER}`;
    case 'refused':
      return `a drop here is not taken — ${held ? NO_FOLDER_HERE : NOTHING_AT_THIS_PATH}`;
  }
}
