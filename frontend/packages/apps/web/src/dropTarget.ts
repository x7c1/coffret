// What a drop onto the folder on the screen will do, said while the files are
// still in the air.
//
// A drop is one of two different things, and from the outside they look the
// same. Onto a folder made here that the Library does not have yet, it is a
// book being brought in: the pages are frozen, packed together into one batch
// of Packs (spec: PK-7). Onto any other folder this device maps, it is files
// being added: the sync carries each one in as a Container of its own. A person
// is owed which of the two they are about to start before they let go rather
// than after, so the list says it while the drag is over it — and says the
// refusal there too, where a folder no mapping of this device reaches has
// nowhere to put any of them (spec: EP-9).
//
// Kept free of DOM and of React so it is unit testable, as `unmapped.ts` beside
// it is.

import { NO_FOLDER_HERE } from '@coffret/api';

import { NOTHING_AT_THIS_PATH } from './unmapped';

/**
 * What a drop onto a folder comes to.
 *
 * - `book`: the pages are packed together as one book.
 * - `book_after`: the same, behind a book that is being packed already.
 * - `files`: each file is added on its own, as the sync always does.
 * - `refused`: nothing is taken, because there is nowhere to put it.
 */
export type DropOutcome = 'book' | 'book_after' | 'files' | 'refused';

/** What the screen knows about the folder a drag is over. */
export interface DropTarget {
  /** Whether a mapping of this device reaches the folder. */
  mapped: boolean;
  /**
   * Whether the folder was made in this browser and the Library does not have
   * it yet — which is what the drop itself reads to ask for a freeze.
   */
  bookDrop: boolean;
  /** Whether a book is being packed right now, wherever it is. */
  freezing: boolean;
}

/**
 * The outcome a drop onto the folder would come to.
 *
 * Read off the same facts the drop is made from, and nothing else: `mapped`
 * is what turns a drop away, and `bookDrop` is what asks the server for a
 * freeze, so the line cannot promise one thing while the drop does another.
 *
 * A book with another in front of it is still taken — the server queues the
 * second behind the first, and they are packed one at a time — so what changes
 * is the order, and that is what is said.
 */
export function dropOutcome(target: DropTarget): DropOutcome {
  if (!target.mapped) {
    return 'refused';
  }
  if (!target.bookDrop) {
    return 'files';
  }
  return target.freezing ? 'book_after' : 'book';
}

/**
 * What a drop onto a folder made here comes to while another book is being
 * packed: books are packed one at a time (spec: PK-7), so this one waits.
 *
 * Shared by the banner such a folder stands under and the line a drag brings
 * up over it, which say the same thing.
 */
export const PACKED_AFTER =
  'a book is being packed already, and they are packed one at a time, so a book dropped ' +
  'here is packed after that one';

/**
 * The line the list shows while files are dragged over it.
 *
 * `held` is whether the Library holds the folder at all, and decides only the
 * refusal's reason — the same reason the answer to letting go gives, so the
 * line before the drop and the notice after it are one fact said twice.
 */
export function dropLine(outcome: DropOutcome, held: boolean): string {
  switch (outcome) {
    case 'book':
      return 'drop to pack these pages together as one book';
    case 'book_after':
      return `drop to pack these pages together as one book — ${PACKED_AFTER}`;
    case 'files':
      return 'drop to add these files one at a time';
    case 'refused':
      return `a drop here is not taken — ${held ? NO_FOLDER_HERE : NOTHING_AT_THIS_PATH}`;
  }
}
