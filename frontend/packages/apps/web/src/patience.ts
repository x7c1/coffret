// Asking again, on its own, for something whose request the network dropped.
//
// A load that got no answer at all — the `fetch` rejected, so there is no HTTP
// status — is most often not a server that went away but a network that moved
// under the request: a Wi-Fi or VPN switching, or the host's interfaces
// changing, which a browser reports by aborting every request in flight
// (Chromium's `net::ERR_NETWORK_CHANGED`). Asked again a moment later, the same
// request is answered. A screen that drew "the coffret server did not answer"
// at the first such drop would sit on it until somebody pressed "try again",
// with a server answering behind it the whole time.
//
// So the loads that feed the screen's regions ask again by themselves, a
// bounded number of times with a short backoff, and only a load whose attempts
// are all spent is drawn as failed. An answer that refuses — any HTTP status, a
// locked Library included — is the server's own word and is not asked again
// this way: the same request would only meet the same refusal.
//
// Kept free of React so it is unit testable.

import { isRefusal } from '@coffret/api';

/**
 * How long to wait before each attempt after the first, in milliseconds: one
 * attempt more than there are pauses, and about four seconds in all, which
 * outlasts the second or two a host's interfaces take to settle.
 */
export const DROPPED_PAUSES_MS: readonly number[] = [250, 500, 1000, 2000];

/**
 * Whether a request got no answer at all: a `fetch` that rejected without being
 * aborted, which the gateway mints as `unreachable` with no status. An
 * `unreachable` whose status arrived and whose body broke off is an answer, and
 * is not this.
 */
export function droppedByNetwork(refused: unknown): boolean {
  return isRefusal(refused) && refused.kind === 'unreachable' && refused.status === 0;
}

/**
 * `ask`, asked again after each pause in `pauses` for as long as it gets no
 * answer at all (see {@link droppedByNetwork}).
 *
 * Settles as the first attempt that got an answer settles, or as the last
 * attempt does. Once `signal` is aborted it asks nothing more, and rejects as
 * the attempt before it did, which the caller lets go of as it lets go of any
 * aborted load.
 */
export async function askedThroughDrops<T>(
  ask: (signal: AbortSignal) => Promise<T>,
  signal: AbortSignal,
  pauses: readonly number[] = DROPPED_PAUSES_MS,
): Promise<T> {
  for (let attempt = 0; ; attempt += 1) {
    try {
      return await ask(signal);
    } catch (refused: unknown) {
      if (signal.aborted || attempt >= pauses.length || !droppedByNetwork(refused)) {
        throw refused;
      }
      console.warn(
        `a load got no answer (attempt ${attempt + 1} of ${pauses.length + 1}); asking again`,
        refused,
      );
      await paused(pauses[attempt], signal);
      if (signal.aborted) {
        throw refused;
      }
    }
  }
}

/** Resolves after `ms`, or as soon as `signal` is aborted. */
function paused(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    const done = () => {
      clearTimeout(timer);
      signal.removeEventListener('abort', done);
      resolve();
    };
    const timer = setTimeout(done, ms);
    signal.addEventListener('abort', done, { once: true });
  });
}
