import { apiUrl, askedForJson } from './request';

/** What a reconnect that started answers with. */
export interface Reconnecting {
  /**
   * Google's consent page, for the page to open in a tab of its own.
   *
   * It carries nothing secret — it is what a browser is sent anyway — and it is
   * the only thing the flow hands a page: the grant it yields goes into the
   * server's cache and nowhere else.
   */
  url: string;
  /** The server's own sentence, written to be read by a person. */
  message: string;
}

/**
 * Asks the server to renew the grant Storage stopped taking —
 * `POST /api/reconnect`.
 *
 * Not a sign-in button and not offered as one. What this exists for is the one
 * Storage refusal no retry clears: a grant that ran out (`storage`, reason
 * `unauthenticated`). The server runs the consent flow itself, with the keys
 * the open Library already holds, so no Passphrase is asked for and none
 * crosses the page; the page opens the URL it is answered with, and learns how
 * the flow ended from the work answer's `reconnect`.
 *
 * A second press while a flow waits is answered with the same page.
 */
export function startReconnect(signal?: AbortSignal): Promise<Reconnecting> {
  return askedForJson<Reconnecting>(apiUrl('reconnect'), signal, 'POST');
}
