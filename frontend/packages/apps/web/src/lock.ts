// The idle lock ending this server's hold on the Master Key, and what the
// screen does with that.
//
// The keys were derived once, when the server was started, and they live until
// the interval the server goes unasked for ends them, or until the server stops
// (spec: DK-1, DK-4). What the lock does here is two things and they are
// separate: the pages this device decrypted under those keys are given up, and
// then the questions the screen is drawn from are asked again.
//
// Nothing in this sequence invents a locked state. The screen has no opinion
// about whether the Library is open: the server is the one that knows, it is
// asked, and what it answers with — its own sentence about the Passphrase
// (spec: DK-2) — is what the regions show, each in the place every other
// refusal is shown.
//
// The lock arrives by one road. It happens on the server's own clock and nobody
// is told, so it arrives as a change in the answer to the question the screen
// is already asking — what this server is doing — and [`lockLanded`] is where
// that answer becomes the giving up. A tab left open over a page it decrypted
// lets go of it at the next poll rather than at the next page turn.
//
// Kept free of DOM and of React so it is unit testable, the way the refresh
// beside it is.

import type { LibraryState } from '@coffret/api';

/**
 * Whether this answer about the Library's state is news of a lock.
 *
 * News, and not the state itself: what a page has to act on is the moment the
 * Library stopped being open under it, because that is the moment the plaintext
 * it is holding stopped being protected by anything. So it is the change that
 * counts — `before` is the last state this page was told, and `null` where it
 * has not been told one yet.
 *
 * A first answer of `locked` is not news. The page came up to a shut Library,
 * every question it asked was refused, and it is holding nothing to give up —
 * discarding there would be this screen reacting to a state it was never in.
 */
export function lockLanded(before: LibraryState | null, now: LibraryState): boolean {
  return before === 'unlocked' && now === 'locked';
}
