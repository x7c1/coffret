// Ending this server's hold on the Master Key, and what the screen does with
// that.
//
// The keys were derived once, when the server was started, and they live until
// this — or the interval the server goes unasked for — ends them
// (spec: DK-1, DK-3, DK-4). What the gesture does here is two things and they
// are separate: the pages this device decrypted under those keys are given up,
// and then the questions the screen is drawn from are asked again.
//
// Nothing in this sequence invents a locked state. The screen has no opinion
// about whether the Library is open: the server is the one that knows, it is
// asked, and what it answers with — its own sentence about the Passphrase
// (spec: DK-2) — is what the regions show, each in the place every other
// refusal is shown.
//
// Only the lock somebody asks for reaches this. The idle interval is the
// server's own clock (spec: DK-4) and says nothing to the browser, so a tab left
// open goes on holding what it drew until its next request is refused.
//
// Kept free of DOM and of React so it is unit testable, the way the refresh
// beside it is.

import type { Locked } from '@coffret/api';

import { said } from './useRemote';

/** Everything one lock reaches out to. */
export interface Locking {
  /** Asks the server, which is [`lockServer`](@coffret/api). */
  ask: () => Promise<Locked>;
  /** Gives up what this device decrypted under the key the lock has ended. */
  discard: () => void;
  /** Asks the Library, the tree and the open folder what they answer now. */
  reload: () => void;
  /** Says what refused the lock, and `null` clears what refused the last one. */
  trouble: (line: string | null) => void;
}

/**
 * Shuts the Library behind whoever asked for it.
 *
 * The discard comes before the reload and not after, because the two are not
 * the same kind of thing: the reload is a question, and its answer arrives when
 * the server gets round to it, while the plaintext this tab is holding is gone
 * the moment the key is. Waiting for an answer to drop it would leave a page on
 * the screen for as long as the network took.
 *
 * Neither happens where the lock did not. The pages are still protected by a key
 * the server still holds, and throwing them away would take the reading somebody
 * is in the middle of for a gesture that came to nothing.
 *
 * The sentence the last press left is taken down before this one is asked, the
 * way the refresh beside it clears its own line. It matters more here than
 * there, because of what that sentence says: a press that was refused leaves
 * "the Library is still open on this device" standing, and a second press that
 * takes would otherwise leave it standing over a screen refusing everything
 * because the Library is shut — the one contradiction on this screen that a
 * person could walk away from a machine on.
 *
 * It never rejects. What a refusal becomes is a sentence on the screen.
 */
export async function askToLock(locking: Locking): Promise<void> {
  locking.trouble(null);
  try {
    await locking.ask();
  } catch (refused: unknown) {
    locking.trouble(stillOpen(refused));
    return;
  }
  locking.discard();
  locking.reload();
}

/**
 * What a lock that did not happen says.
 *
 * The one refusal on this screen where the reason is the smaller half.
 * Everywhere else the sentence is the whole of it — a file was not opened, a
 * drop was not taken — and the screen goes on saying what it said before. Here
 * somebody asked to have the Library shut behind them, and one who read only the
 * reason could walk away from a machine they believe is closed.
 */
export function stillOpen(refused: unknown): string {
  return `the Library is still open on this device — ${said(refused)}`;
}
