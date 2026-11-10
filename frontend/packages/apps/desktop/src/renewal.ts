// Keeping the explorer's session from ending under its page.
//
// A browser token lives 12 hours from when it was issued, and is never
// extended (spec: LA-18). Ahead of that, the main process issues a fresh
// launch ticket on its own timer and hands it to the explorer's page over
// `FRESH_TICKET`, a one-way message the preload relays to a listener the page
// registers; the page exchanges it for a new token and holds that in place of
// the old one, without loading again. The page has no call that makes the main
// process issue a ticket: what it can send is `SESSION_ENDED`, which answers
// nothing, and tells the app its page holds no session, so that the app loads
// the window afresh where it can (`desktop.ts`). Kept apart from Electron so
// that it is tested without it.

import { fromOrigin } from './drops.js';

/**
 * The channel a fresh ticket is handed to the explorer's page over.
 * `preload.cts` writes the same name out, since a sandboxed preload imports
 * nothing of the app's.
 */
export const FRESH_TICKET = 'coffret:fresh-ticket';

/**
 * The channel the explorer's page says over that it holds no session. Sent,
 * never invoked: it has no answer. `preload.cts` writes the same name out.
 */
export const SESSION_ENDED = 'coffret:session-ended';

/** How long a browser token lives from when it was issued (spec: LA-18). */
export const TOKEN_LIFETIME_MS = 12 * 60 * 60 * 1000;

/**
 * How long ahead of a token's expiry a fresh ticket is handed over: long
 * enough that a page slowed down in a hidden window, or a first exchange
 * refused for a moment, still has its token while it takes the next one up.
 */
export const RENEW_AHEAD_MS = 60 * 60 * 1000;

/**
 * How often the timer looks at the clock. It compares against the wall clock
 * rather than counting down, so that a device that slept through the time a
 * ticket was due hands it over as soon as it wakes.
 */
export const RENEWAL_CHECK_MS = 60 * 1000;

/** How long after a ticket that could not be issued another is tried. */
export const RENEWAL_RETRY_MS = 5 * 60 * 1000;

/** What a renewal reaches outside itself. */
export interface RenewalTarget {
  /** A fresh launch ticket, from the call within the process the host runs in (spec: LA-17). */
  issueTicket(): string;
  /**
   * Hands `ticket` to the explorer's page, saying whether there was one to
   * hand it to: none once the window was destroyed, nor while it shows
   * anything but the explorer.
   */
  deliver(ticket: string): boolean;
  /** Says something went other than expected. */
  warn(message: string): void;
}

/**
 * The timer that hands the explorer's page a fresh ticket ahead of its
 * token's expiry.
 *
 * Every load of the window from a ticket starts the count ({@link loaded}):
 * the token that ticket is exchanged for is issued after it, so it lives at
 * least until the ticket's issue plus {@link TOKEN_LIFETIME_MS}, and a fresh
 * one is handed over {@link RENEW_AHEAD_MS} before then. Every ticket handed
 * over starts it again. Where there is no page to hand one to, nothing is
 * handed over, and the count waits for the next load.
 */
export class Renewal {
  /** When the next ticket is due, or `null` where no page waits on one. */
  private due: number | null = null;
  private timer: ReturnType<typeof setInterval> | null = null;

  constructor(
    private readonly target: RenewalTarget,
    private readonly now: () => number = Date.now,
  ) {}

  /** Starts looking at the clock. */
  start(): void {
    if (this.timer === null) {
      this.timer = setInterval(() => this.tick(), RENEWAL_CHECK_MS);
    }
  }

  /** Stops for good: what the app quitting does. */
  stop(): void {
    if (this.timer !== null) {
      clearInterval(this.timer);
      this.timer = null;
    }
    this.due = null;
  }

  /** The window was loaded from a ticket issued now. */
  loaded(): void {
    this.due = this.now() + TOKEN_LIFETIME_MS - RENEW_AHEAD_MS;
  }

  /** When the next ticket is due, or `null`. */
  dueAt(): number | null {
    return this.due;
  }

  /** Hands a fresh ticket over where one is due. */
  tick(): void {
    if (this.due === null || this.now() < this.due) {
      return;
    }
    let ticket: string;
    try {
      ticket = this.target.issueTicket();
    } catch (error) {
      this.target.warn(
        `no ticket could be issued to renew the explorer's session; it is tried again: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      this.due = this.now() + RENEWAL_RETRY_MS;
      return;
    }
    // The ticket itself is never said: it would make a session.
    if (this.target.deliver(ticket)) {
      this.due = this.now() + TOKEN_LIFETIME_MS - RENEW_AHEAD_MS;
    } else {
      this.due = null;
    }
  }
}

/** The part of an `ipcMain.on` event a page's message is judged by. */
export interface PageSaying {
  /** The contents of the window that sent it. */
  sender: unknown;
  /** The frame that sent it, where Electron still has it. */
  senderFrame: { url: string } | null;
}

/**
 * A listener for a one-way message from the explorer's page, named `what` in
 * what is said about it.
 *
 * Only the explorer's window is heard, and only from the explorer's own
 * origin, as a drop is (`drops.ts`): `explorer()` is its contents while it is
 * open and `origin()` the origin it is served from. Anything else is said and
 * let be. What is heard is handed to `heard`, with nothing of the message's
 * own: the page says only that something happened.
 */
export function explorerPageListener(
  what: string,
  explorer: () => unknown,
  origin: () => string | null,
  heard: () => void,
  warn: (message: string) => void,
): (event: PageSaying) => void {
  return (event) => {
    const window = explorer();
    if (window === null || window === undefined || event.sender !== window) {
      warn(`${what} was ignored: it did not come from the explorer window`);
      return;
    }
    if (!fromOrigin(event.senderFrame, origin())) {
      warn(`${what} was ignored: it did not come from the explorer page`);
      return;
    }
    heard();
  };
}

/**
 * The listener `SESSION_ENDED` is heard by: from the explorer's page alone
 * ({@link explorerPageListener}). What is heard is handed to `ended`, which
 * decides whether the window is loaded afresh. Nothing is answered: the page
 * learns of a fresh ticket only by the window being loaded from one.
 */
export function sessionEndedListener(
  explorer: () => unknown,
  origin: () => string | null,
  ended: () => void,
  warn: (message: string) => void,
): (event: PageSaying) => void {
  return explorerPageListener('a session said to have ended', explorer, origin, ended, warn);
}
