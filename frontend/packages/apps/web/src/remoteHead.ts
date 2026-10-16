// Looking, on its own, whether the Library has anything this device has not
// caught up with.
//
// The catalog moves only when somebody asks it to — the sync icon in the status
// bar is that ask — but whether there is anything to ask for is a question the
// explorer can put without moving anything: the server compares the names
// Storage lists with the head the catalog stands at, reads no object, and
// answers `newer` or `current`. This is when that question is put, and what is
// done with the answer.
//
// Kept free of React so it is unit testable: what it does is a schedule — once
// as the page comes up, again whenever the tab comes back to the front, and
// every few minutes while it is shown — with two rules over it: never while a
// catch-up is running, and never one answer that arrived after a catch-up began.

import type { HeadState, RemoteHead } from '@coffret/api';

/**
 * How long the explorer waits between two looks while its tab is shown.
 *
 * Three minutes. Each look costs Storage one walk of the listing — names only,
 * but a page of them per few hundred objects — so a Library whose provider
 * counts calls is not asked every few seconds by a tab nobody is reading. And
 * nothing about the Library's correctness rests on hearing promptly (a stale
 * catalog is what every writing run's own catch-up fixes first): what rests on
 * it is a person glancing at the icon after another device committed, and
 * coming back to the tab already looks at once. A few minutes is the wait
 * somebody who left the tab in front would not notice.
 */
export const LOOK_EVERY_MS = 3 * 60 * 1000;

/** Everything the looks reach out to. */
export interface Looking {
  /** Asks the server, which is [`getRemoteHead`](@coffret/api). */
  ask: () => Promise<RemoteHead>;
  /** Says what the last look found. */
  heard: (head: HeadState) => void;
  /**
   * Whether a look now would be out of place: a catch-up is running, so the
   * catalog the answer compares against is moving under it, or the Library
   * is locked and the look would only be refused.
   */
  held: () => boolean;
  /** Whether the tab is shown, which is what the interval waits for. */
  shown: () => boolean;
}

/**
 * When the explorer looks at the remote head, and which answers it keeps.
 *
 * A look that fails says nothing. The icon keeps whatever the last answer drew
 * and the next look tries again: the look is a courtesy beside the control, and
 * Storage being away is said — loudly enough — by the press of it, which is a
 * catch-up and reports its refusal. The server logs what refused the look on
 * its own side.
 */
export class RemoteHeadLooks {
  private readonly looking: Looking;
  /** Advanced by every [`forget`](RemoteHeadLooks.forget). */
  private generation = 0;
  /** Whether a look is waiting for its answer. */
  private asking = false;

  constructor(looking: Looking) {
    this.looking = looking;
  }

  /**
   * Looks once as the page comes up, then every [`LOOK_EVERY_MS`] while the
   * tab is shown, until the returned function is called.
   */
  start(): () => void {
    this.look();
    const timer = setInterval(() => {
      if (this.looking.shown()) {
        this.look();
      }
    }, LOOK_EVERY_MS);
    return () => clearInterval(timer);
  }

  /** The tab came back to the front, which is the moment it is looked at. */
  front(): void {
    this.look();
  }

  /**
   * Forgets the look in flight, if any: a catch-up has begun, and what that
   * look answers is about the catalog as it stood before the catch-up moved
   * it. Drawn after the catch-up landed, a `newer` would say changes are
   * waiting that have just been taken in.
   */
  forget(): void {
    this.generation += 1;
  }

  /** Looks now, unless a look is out of place or one is already on its way. */
  look(): void {
    if (this.asking || this.looking.held()) {
      return;
    }
    this.asking = true;
    const asked = this.generation;
    this.looking
      .ask()
      .then(
        ({ head }) => {
          if (asked === this.generation && !this.looking.held()) {
            this.looking.heard(head);
          }
        },
        () => {
          // Kept as it was: see the class's own account of a failed look.
        },
      )
      .finally(() => {
        this.asking = false;
      });
  }
}
