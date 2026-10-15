import type { Refusal } from './refusal';

/**
 * The browser token this tab holds, and the one place it is held.
 *
 * The explorer host admits a page only by a token of its own (spec: LA-14),
 * which the page got by exchanging a launch ticket. Every request this package
 * makes carries it, and nothing else in the page reaches it: it is never put
 * on a URL, in an `<img src>` or in a link (spec: LA-6). Where it is kept
 * across a reload is the app's business; this is where it is held while the
 * page runs.
 *
 * Holding it is counted in rounds. Every change of what is held — a token
 * taken up, a token let go, the Library locking under it — starts a new round,
 * and an answer to a request made in an older round is not handed to whoever
 * made it: it arrives after the session it was asked under ended, or after the
 * Library locked, and is nobody's to draw.
 */

/**
 * What an answer said became of the page's session: it ended, or the Library
 * locked under it, which keeps the session and refuses it data
 * (spec: LA-14, LA-18).
 */
export type Ending =
  /** The token admits nothing any more: revoked, expired, or retired. */
  | 'ended'
  /** The Library is locked; the token still stands, and is refused data. */
  | 'locked';

/** Told what became of the session, and by which refusal. */
export type EndingListener = (ending: Ending, refusal: Refusal) => void;

let token: string | null = null;
let round = 0;
const listeners = new Set<EndingListener>();

/** Holds `next` as this tab's token, from a new round. */
export function holdToken(next: string): void {
  token = next;
  round += 1;
}

/** Lets the token go, from a new round. */
export function dropToken(): void {
  token = null;
  round += 1;
}

/** The token held now, and `null` where none is. */
export function heldToken(): string | null {
  return token;
}

/**
 * Starts a new round with the same token, so that whatever is in flight now
 * is not handed to the page: what the Library locking does to the answers
 * that were on their way.
 */
export function withdrawAnswers(): void {
  round += 1;
}

/** The round requests made now belong to. */
export function currentRound(): number {
  return round;
}

/**
 * Tells `listener` whenever an answer says the session ended or the Library
 * locked under it, until the function this returns is called.
 */
export function onSessionEnding(listener: EndingListener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * Hears one refusal, and tells the listeners where it says the session ended —
 * the host no longer admits the token, or the server behind it stopped, which
 * revoked every session (spec: LA-15) — or where it says the Library locked.
 */
export function heardRefusal(refusal: Refusal): void {
  const ending = endingOf(refusal);
  if (ending === null) {
    return;
  }
  for (const listener of [...listeners]) {
    listener(ending, refusal);
  }
}

/** Which ending a refusal is, and `null` for one that says neither. */
function endingOf(refusal: Refusal): Ending | null {
  switch (refusal.kind) {
    case 'authentication_required':
    case 'server_unavailable':
      return 'ended';
    case 'library_locked':
      return 'locked';
    default:
      return null;
  }
}
