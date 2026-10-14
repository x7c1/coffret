import { authUrl, requested, requestedJson } from './request';
import type { LibraryState } from './work';

/**
 * What an exchange of a launch ticket answers with — `POST /auth/launch`
 * (spec: LA-16).
 */
export interface Launched {
  /** The browser token, for every request this page makes from now on. */
  token: string;
  /** How many seconds it is good for; nothing renews it (spec: LA-18). */
  expires_in: number;
  /**
   * The host's generation it belongs to: public, admitting nothing, and what a
   * page tells one generation's token from the next one's by.
   */
  instance_id: string;
}

/** What the host says of a session — `GET /auth/session` (spec: LA-16). */
export interface SessionStanding {
  /** How many seconds are left of it. */
  expires_in: number;
  /** Whether the Library behind the host is locked. */
  library: LibraryState;
}

/**
 * Exchanges a launch ticket for a browser token, spending the ticket
 * (spec: LA-16, LA-17).
 *
 * The ticket travels in `Authorization` and nowhere else, and the body is the
 * empty JSON object the route takes. A refusal is the caller's to read: there
 * was no session for it to end.
 */
export function launch(ticket: string, signal?: AbortSignal): Promise<Launched> {
  return requestedJson<Launched>(authUrl('launch'), {
    signal,
    method: 'POST',
    body: '{}',
    contentType: 'application/json',
    authorization: `Launch ${ticket}`,
  });
}

/**
 * Asks whether `token` still stands, and whether the Library is locked.
 *
 * Answered by the host and never by the server, so asking it keeps nothing
 * unlocked (spec: DK-4): it is what a page whose Library locked asks until it
 * is unlocked again, rather than anything under `/api`. A refusal is the
 * caller's to read, since the token asked about may not be the one held.
 */
export function checkSession(token: string, signal?: AbortSignal): Promise<SessionStanding> {
  return requestedJson<SessionStanding>(authUrl('session'), {
    signal,
    authorization: `Bearer ${token}`,
  });
}

/**
 * Ends the session of `token` at the host, and no other (spec: LA-16, LA-18).
 *
 * The token is named rather than read from what is held, because a page
 * letting go of its session lets go of the token first, so that nothing still
 * in flight is handed to it, and asks the host to end it after. Letting the
 * token go is the caller's to do, whatever this answers.
 */
export async function revokeSession(token: string, signal?: AbortSignal): Promise<void> {
  await requested(authUrl('session/revoke'), {
    signal,
    method: 'POST',
    authorization: `Bearer ${token}`,
  });
}
