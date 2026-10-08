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
// The page also asks on its own, once for each lock, and only while somebody is
// looking at it ([`UnlockPrompting`]): the ask is the press of *unlock* made
// for the person who came back, and nothing more.
//
// Kept free of DOM and of React so it is unit testable, the way the refresh
// beside it is.

import { isRefusal, type LibraryState, type Unlocking } from '@coffret/api';

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

/**
 * Whether a question refused with the locked refusal should send the page to
 * read the work answer again.
 *
 * A page with the reader closed and nothing in flight asks nothing at the
 * interval (spec: DK-4), so a lock that lands while it sits in front is first
 * heard as the listing or the tree refused with the locked refusal. Where the
 * page still believes the Library open, that refusal is news the work answer
 * has not brought yet: reading it once turns the page's state to locked, and
 * what follows a lock — the discard, the *unlock* button, the ask
 * {@link UnlockPrompting} makes on its own — follows from there. Where the page
 * already knows it is locked, or has not been told anything yet, the work
 * answer is already on its way or already said so.
 */
export function lockUnheard(believed: LibraryState | null, refusedLocked: boolean): boolean {
  return refusedLocked && believed === 'unlocked';
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

/**
 * When the page asks for the unlock without anybody pressing *unlock*.
 *
 * Coming back to an explorer that locked while the person was away should bring
 * the desktop app's Passphrase window forward by itself. The ask is the very
 * one the button sends (spec: DK-1): it decides only when the window is shown,
 * and the Passphrase still never touches the page (spec: LA-3, LA-6). Three
 * rules bound it:
 *
 * - **Only while somebody is looking.** An idle lock happens while the person
 *   is away, so asking the moment it is seen would push the window in front of
 *   whatever they are doing elsewhere. A lock seen while the page is hidden or
 *   unfocused is owed an ask, paid the next time the page is in front.
 * - **Once for each lock.** A window closed without the Passphrase is an
 *   answer, and asking again would be nagging; the *unlock* button and the
 *   tray's *Unlock…* stay where they were. A lock is owed again only after an
 *   unlock, and a press of the button pays what is owed as well as an
 *   automatic ask would.
 * - **Only where it can work.** A server started without the desktop app has
 *   no window to bring forward and refuses with the locked refusal, whose
 *   sentence says to start it again. Nothing the server says beforehand tells
 *   the two apart, so that refusal is the sign, and the page asks nothing on
 *   its own for the rest of its life.
 *
 * Each of {@link UnlockPrompting.heard} and {@link UnlockPrompting.front}
 * answers whether to ask now; the caller sends the ask, and hands what refused
 * it to {@link UnlockPrompting.refused}.
 */
export class UnlockPrompting {
  /** The last state this page was told, and `null` before the first. */
  private library: LibraryState | null = null;
  /** Whether the lock the page is under has not been asked about yet. */
  private owed = false;
  /** Whether this server has said it has no window to bring forward. */
  private windowless = false;

  /**
   * Hears one answer about the Library's state, while the page is or is not
   * in front.
   *
   * A lock is news here on wider terms than {@link lockLanded}'s: a page that
   * came up to a shut Library is owed an ask as much as one that watched it
   * shut, since what it is for is the person who came back to it.
   */
  heard(now: LibraryState, inFront: boolean): boolean {
    const before = this.library;
    this.library = now;
    if (now === 'unlocked') {
      this.owed = false;
    } else if (before !== 'locked') {
      this.owed = true;
    }
    return this.due(inFront);
  }

  /** Hears that the page came in front, or went out of it. */
  front(inFront: boolean): boolean {
    return this.due(inFront);
  }

  /** Hears a press of *unlock*, which is the ask this lock was owed. */
  pressed(): void {
    this.owed = false;
  }

  /**
   * Hears what refused an ask, automatic or pressed.
   *
   * Only the locked refusal is the sign of a server with no window: the route
   * answers that refusal only where nothing can be asked. A request that did
   * not reach the server says nothing about it, and changes nothing here.
   */
  refused(thrown: unknown): void {
    if (isRefusal(thrown) && thrown.kind === 'locked') {
      this.windowless = true;
    }
  }

  private due(inFront: boolean): boolean {
    if (!this.owed || !inFront || this.windowless) {
      return false;
    }
    this.owed = false;
    return true;
  }
}
