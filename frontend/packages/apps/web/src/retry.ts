// Whether a second attempt at a stopped run is worth offering, and what one
// such press was refused with, kept free of DOM so it is unit testable.
//
// The offers themselves are made by the Activity panel from the server's log
// of finished work (see [`activity`](./activity)): a fill, a sync or a freeze
// that stopped, and a folder a queue lost. A refusal one of them met is kept as
// what it answered rather than as a sentence on its own, so it lives exactly
// as long as the offer it answered: tied to nothing, it would have to be ended
// by something, and the only thing left to end it by is the next answer —
// which would take the reason off the screen at the next poll with the button
// still standing and nothing saying why pressing it did nothing.

import type { DisplacedFill, DisplacedFreeze, Fill, Freeze, RefusalKind, Sync } from '@coffret/api';

/**
 * Which second attempt a press was.
 *
 * The sync's names no folder, unlike the other two: which folders a sync walks
 * is the device's mappings and never something a screen chooses. The other two
 * name one, and it is not always the folder of the latest run — a folder a
 * queue lost is offered by name.
 */
export type Pressed =
  | { flow: 'sync' }
  | { flow: 'fill'; folder: string }
  | { flow: 'freeze'; folder: string };

/** One press that was refused, and what refused it. */
export interface Trouble {
  /** Which offer this answered. */
  pressed: Pressed;
  /** The refusal's own sentence, as it goes on the screen. */
  said: string;
  /**
   * Whether what refused it is Storage no longer taking this device's grant,
   * which is the refusal the panel offers a reconnect beside rather than
   * leaving the press to be made again.
   */
  ranOut?: boolean;
}

/**
 * Whether a stopped background run can be helped by making the same request.
 *
 * A refused mapped root needs the recovery named in its visible explanation;
 * repeating the run cannot change the mapping. Older servers did not send a
 * structured reason, so their stopped runs retain the ordinary retry.
 *
 * Nor can a run the server stopped for a reason about this device's standing,
 * or about the Library's format, rather than about the run. `epoch` is a device
 * that has to be enrolled in the Library again, with the new Recovery Code, at
 * a terminal; `library_predates_format` is a Library written before this app's
 * format, which is made again rather than read; `locked` is a server that needs
 * the Passphrase, given in the desktop app's own window or by starting a
 * command-line server again, and while the Library is locked the *Unlock* of
 * the screen's locked state — in the main area and in the Activity panel's row
 * — stands in for every one of these buttons. In each case the same
 * request meets the same refusal, and the sentence already says the one thing
 * that remedies it — so a button beside it would be offering a press that
 * cannot change the answer, and a refusal of that press would repeat the line
 * above it.
 */
export function retryable(
  run: Fill | Sync | Freeze | DisplacedFill | DisplacedFreeze | null,
): boolean {
  if (run?.status !== 'stopped') {
    return false;
  }
  const refused = run.stopped;
  return !(
    (refused.kind === 'refused_placement' && refused.reason === 'refused_root') ||
    UNHELPED_BY_ASKING_AGAIN.includes(refused.kind)
  );
}

/**
 * The kinds of refusal no second attempt of the same run can meet differently,
 * because what they are about is this device or the Library, and not the run.
 */
const UNHELPED_BY_ASKING_AGAIN: readonly RefusalKind[] = [
  'epoch',
  'library_predates_format',
  'locked',
];
