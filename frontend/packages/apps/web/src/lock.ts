// The idle lock ending this server's hold on the Master Key, the unlock giving
// it back, and what the screen does with each.
//
// The keys were derived when the server was started or last unlocked, and they
// live until the interval the server goes unasked for ends them, or until the
// server stops (spec: DK-1, DK-4). What the lock does here is two things and
// they are separate: the pages this device decrypted under those keys are given
// up, and then the questions the screen is drawn from are asked again.
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
// The unlock arrives by the same road, the other way. The Passphrase is never
// typed into this page: the page asks the server to have the desktop app put
// its own window in front ([`askForUnlock`]), the Passphrase is typed there —
// or the app's tray was used, and this page asked nothing — and the server
// holds the Library again. What this page sees is the same answer turning back
// to `unlocked`, and [`unlockLanded`] is where that becomes asking the
// questions again, so a page that was refused comes back by itself.
//
// Kept free of DOM and of React so it is unit testable, the way the refresh
// beside it is.

import type { LibraryState, Unlocking } from '@coffret/api';

import { said } from './useAsked';

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

/**
 * Whether this answer about the Library's state is news of an unlock.
 *
 * The mirror of {@link lockLanded}, and news on the same terms: the change and
 * not the state. A page that was told `locked` and is now told `unlocked` has
 * been refused everything since, and asks it all again — the listing, the tree,
 * and the page the reader was on — without anybody pressing *try again*.
 *
 * A first answer of `unlocked` is not news. The page came up to an open
 * Library and asked its questions already; asking them all again would be this
 * screen reacting to a state it was never out of.
 */
export function unlockLanded(before: LibraryState | null, now: LibraryState): boolean {
  return before === 'locked' && now === 'unlocked';
}

/** What a press of *unlock* is wired to. */
export interface UnlockPressing {
  /** Asks the server to have the Library unlocked. */
  ask: () => Promise<Unlocking>;
  /** Says what the server answered, or nothing. */
  line: (said: string | null) => void;
  /** Says what refused the press, or nothing. */
  trouble: (said: string | null) => void;
  /** Asks what the server is doing, where the answer says it is open already. */
  recheck: () => void;
}

/**
 * Asks for the unlock, and says what came of it.
 *
 * What the server answers with is a sentence for the person — the app is
 * asking for the Passphrase in its own window — and nothing for the screen to
 * do: the unlock itself lands later, from that window, and is heard by
 * {@link unlockLanded}. A server that says the Library is already open is asked
 * what it is doing at once, so the screen catches up with it without waiting.
 *
 * A server started from the command line has no window to ask, and refuses with
 * the locked refusal, whose sentence says to start it again; that sentence is
 * shown as what refused the press.
 *
 * It never rejects.
 */
export async function askForUnlock(pressing: UnlockPressing): Promise<void> {
  pressing.line(null);
  pressing.trouble(null);
  try {
    const answer = await pressing.ask();
    pressing.line(answer.message);
    if (answer.library === 'unlocked') {
      pressing.recheck();
    }
  } catch (refused: unknown) {
    pressing.trouble(said(refused));
  }
}
