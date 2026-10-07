import { apiUrl, askedForJson } from './request';
import type { LibraryState } from './work';

/** What an unlock that was asked for answers with. */
export interface Unlocking {
  /**
   * `unlocked` where the Library already was, and `locked` where the app has
   * been asked for the Passphrase and has not been given it yet. The work
   * answer's `library` says when it has.
   */
  library: LibraryState;
  /** The server's own sentence, written to be read by a person. */
  message: string;
}

/**
 * Asks the server to have the Library unlocked — `POST /api/unlock`.
 *
 * It carries no Passphrase, and nothing on a page ever does: a Passphrase typed
 * into a page would be a Passphrase carried through one. What the server does
 * is ask the desktop app it runs in to put its own window in front, and the
 * Passphrase is typed there; the page learns the Library is open again from the
 * work answer's `library`.
 *
 * A server started from the command line has no such window, and refuses with
 * `locked`, whose sentence says to start it again — the one true thing there is
 * to tell somebody there.
 */
export function askToUnlock(signal?: AbortSignal): Promise<Unlocking> {
  return askedForJson<Unlocking>(apiUrl('unlock'), signal, 'POST');
}
