// How this page comes to hold a browser token, how it takes up a new one in
// place of the one it holds, and where it keeps one across a reload, kept free
// of DOM and of React so it is unit testable.
//
// The explorer host admits a page only by a browser token of its own
// (spec: LA-14). A page gets one by exchanging the one-time launch ticket the
// app delivered to it through a file only its owner can read: the file
// refreshes to `http://127.0.0.1:<port>/#launch=<ticket>`, and the page takes
// the ticket out of the fragment, removes it from the address at once, and
// exchanges it (spec: LA-21). A page reloaded later has no fragment, and uses
// the token it kept — once the host says it still stands.
//
// In the desktop app's window, the app also hands the page a fresh ticket
// ahead of its token's expiry, and the page exchanges it for a new token and
// holds that in place of the old one, without loading again (spec: LA-18).
//
// Where it is kept is this page's `sessionStorage`, under a key naming the
// host's generation, and nowhere else: never `localStorage`, a cookie,
// IndexedDB, a Service Worker or Cache Storage, which would each outlive the
// page or reach other pages. Where that storage refuses, the token is held in
// memory only, and a reload asks for the explorer to be opened again.

import {
  checkSession,
  heldToken,
  holdToken,
  isRefusal,
  launch,
  renewToken,
  revokeSession,
} from '@coffret/api';

/** What the key a token is kept under starts with; the generation follows. */
export const KEPT_PREFIX = 'coffret.explorer.session.';

/** What the fragment carrying a ticket starts with. */
const LAUNCH = '#launch=';

/** The part of `location` a ticket is read out of. */
export type Address = Pick<Location, 'hash' | 'pathname' | 'search'>;

/**
 * The launch ticket in the address's fragment, removed from the address before
 * anything else happens, or `null` where there is none.
 *
 * Removed with `replaceState`, so that the address the page is left at — the
 * one its history, a bookmark and a copied link would carry — never holds it.
 */
export function takeTicket(address: Address, history: Pick<History, 'replaceState'>): string | null {
  if (!address.hash.startsWith(LAUNCH)) {
    return null;
  }
  const ticket = address.hash.slice(LAUNCH.length);
  history.replaceState(null, '', `${address.pathname}${address.search}`);
  return ticket === '' ? null : ticket;
}

/** One token kept for one generation of the host. */
export interface Kept {
  instanceId: string;
  token: string;
}

/** Where this page keeps its token between one load and the next. */
export interface Shelf {
  keep: (instanceId: string, token: string) => void;
  /** Every token kept: one, for a page that has opened the explorer before. */
  kept: () => Kept[];
  forget: (instanceId: string) => void;
  forgetAll: () => void;
}

/** The storage asked for lazily, because asking can throw too. */
export type Storing = () => Storage;

/**
 * This page's shelf: `sessionStorage` while it answers, and memory from the
 * first time it does not.
 *
 * A private window, a browser that blocks site data, or a full quota each make
 * reading or writing it throw, and the explorer must still work for the life
 * of the page; only a reload forgets.
 */
export function shelf(storing: Storing = () => window.sessionStorage): Shelf {
  const memory = new Map<string, string>();
  let storage: Storage | null = null;
  try {
    storage = storing();
  } catch (refused) {
    console.warn('the explorer cannot keep its session in this page; it keeps it in memory', refused);
  }

  /** Runs `use` on the storage, or on nothing once it has refused. */
  const stored = <T>(use: (storage: Storage) => T, otherwise: () => T): T => {
    if (storage !== null) {
      try {
        return use(storage);
      } catch (refused) {
        console.warn('the explorer cannot keep its session in this page; it keeps it in memory', refused);
        storage = null;
      }
    }
    return otherwise();
  };

  const keys = (storage: Storage): string[] => {
    const found: string[] = [];
    for (let index = 0; index < storage.length; index += 1) {
      const key = storage.key(index);
      if (key !== null && key.startsWith(KEPT_PREFIX)) {
        found.push(key);
      }
    }
    return found;
  };

  return {
    keep(instanceId, token) {
      memory.set(instanceId, token);
      stored(
        (storage) => storage.setItem(`${KEPT_PREFIX}${instanceId}`, token),
        () => undefined,
      );
    },
    kept() {
      return stored(
        (storage) =>
          keys(storage).flatMap((key) => {
            const token = storage.getItem(key);
            return token === null ? [] : [{ instanceId: key.slice(KEPT_PREFIX.length), token }];
          }),
        () => [...memory].map(([instanceId, token]) => ({ instanceId, token })),
      );
    },
    forget(instanceId) {
      memory.delete(instanceId);
      stored(
        (storage) => storage.removeItem(`${KEPT_PREFIX}${instanceId}`),
        () => undefined,
      );
    },
    forgetAll() {
      memory.clear();
      stored(
        (storage) => {
          for (const key of keys(storage)) {
            storage.removeItem(key);
          }
        },
        () => undefined,
      );
    },
  };
}

/** How this page came up: holding a token, or not, and why not. */
export type Opened =
  | {
      kind: 'held';
      /** How many seconds the session has left. */
      expiresIn: number;
    }
  | {
      kind: 'none';
      /** What refused the exchange or the check, and `null` where nothing was asked. */
      said: string | null;
    };

/** The two questions a page coming up asks the host. */
export interface Asking {
  launch: typeof launch;
  checkSession: typeof checkSession;
}

/**
 * Takes a token up for this page: by exchanging `ticket` where there is one,
 * and otherwise by the token kept for this page, once the host says it still
 * stands. Every token kept that the host no longer admits is forgotten.
 *
 * The exchange is asked once, whoever waits on the answer: the ticket is
 * spent by it (spec: LA-17).
 */
export async function establish(
  ticket: string | null,
  shelved: Shelf,
  asking: Asking = { launch, checkSession },
): Promise<Opened> {
  if (ticket !== null) {
    try {
      const launched = await asking.launch(ticket);
      shelved.forgetAll();
      shelved.keep(launched.instance_id, launched.token);
      holdToken(launched.token);
      return { kind: 'held', expiresIn: launched.expires_in };
    } catch (refused) {
      return { kind: 'none', said: refusedLaunch(refused) };
    }
  }
  for (const kept of shelved.kept()) {
    try {
      const standing = await asking.checkSession(kept.token);
      for (const other of shelved.kept()) {
        if (other.instanceId !== kept.instanceId) {
          shelved.forget(other.instanceId);
        }
      }
      holdToken(kept.token);
      return { kind: 'held', expiresIn: standing.expires_in };
    } catch (refused) {
      if (isRefusal(refused) && refused.kind === 'authentication_required') {
        shelved.forget(kept.instanceId);
        continue;
      }
      // The host did not say the token is gone — it did not answer, or
      // answered something else — so it is kept for the next load to ask
      // about, and nothing is held meanwhile.
      return { kind: 'none', said: sentence(refused) };
    }
  }
  return { kind: 'none', said: null };
}

/**
 * What refused the exchange of a ticket, as a sentence for the screen, and
 * `null` where the refusal is only that this page holds no session.
 *
 * A ticket already spent, or one the host never issued, is answered as a
 * request with no session — and the page says that itself, with how to open
 * the explorer, so the host's sentence under it would be the same thing said
 * twice.
 */
function refusedLaunch(refused: unknown): string | null {
  if (isRefusal(refused) && refused.kind === 'authentication_required') {
    return null;
  }
  return sentence(refused);
}

/** What a renewal came to: a new token held, or the exchange refused, and by what. */
export type Renewed =
  | {
      kind: 'renewed';
      /** How many seconds the new session has. */
      expiresIn: number;
    }
  | {
      kind: 'refused';
      /** What refused the exchange, and `null` where it is only that no session came of it. */
      said: string | null;
    };

/** The two questions a page renewing its token asks the host. */
export interface Renewing {
  launch: typeof launch;
  revokeSession: typeof revokeSession;
}

/**
 * Takes up a new token for this page, from the fresh `ticket` the desktop app
 * handed it, in place of the one it holds (spec: LA-18).
 *
 * The session is not extended: the ticket is exchanged for a token newly
 * issued, which is held for every request from now on — in the same round,
 * so that nothing in flight is let go of and nothing drawn is drawn again —
 * and kept on the shelf in place of the old one. The old token's session is
 * then revoked with that token itself, so that one window does not fill the
 * host's sessions with the ones it left behind; a refusal of that is said
 * and otherwise let be, since the old session ends at its expiry anyway.
 *
 * A token of another generation of the host than the one held is no
 * renewal: the host or the Library behind it changed, and what the page
 * shows is of the generation before. It is revoked, and the renewal is
 * refused, as the session ending.
 */
export async function renew(
  ticket: string,
  shelved: Shelf,
  asking: Renewing = { launch, revokeSession },
): Promise<Renewed> {
  const before = heldToken();
  const generation = shelved.kept().find((kept) => kept.token === before)?.instanceId ?? null;
  let launched;
  try {
    launched = await asking.launch(ticket);
  } catch (refused) {
    return { kind: 'refused', said: refusedLaunch(refused) };
  }
  if (generation !== null && launched.instance_id !== generation) {
    revokeLeft(asking, launched.token, 'the session of another generation');
    return { kind: 'refused', said: null };
  }
  shelved.forgetAll();
  shelved.keep(launched.instance_id, launched.token);
  renewToken(launched.token);
  if (before !== null) {
    revokeLeft(asking, before, 'the session the explorer renewed away from');
  }
  return { kind: 'renewed', expiresIn: launched.expires_in };
}

/** Revokes the session `token` stands for, saying so where that is refused. */
function revokeLeft(asking: Renewing, token: string, what: string): void {
  asking.revokeSession(token).catch((refused: unknown) => {
    console.warn(`the explorer could not revoke ${what}; it ends when it expires`, refused);
  });
}

/** What refused, as a sentence for the screen. */
function sentence(refused: unknown): string {
  if (isRefusal(refused)) {
    return refused.message;
  }
  console.error('the explorer failed to open at something that is not a refusal', refused);
  return 'the explorer could not open';
}

/**
 * What the desktop app's window offers the page about its session, through
 * the window's preload. Neither call asks the app for a ticket: the app
 * hands one over on its own, ahead of the token's expiry, and the page only
 * listens for it (spec: LA-17, LA-18). What the page says the other way, that
 * it holds no session, the app answers by loading the window afresh
 * (spec: LA-21).
 */
export interface DesktopSession {
  /**
   * Hands every fresh ticket the app delivers to `listener`, until the
   * function this returns is called.
   */
  onTicket(listener: (ticket: string) => void): () => void;
  /**
   * Tells the app that this page holds no session — it ended, or it never
   * came — so that the app loads its window afresh from a ticket of its own
   * where it can. It answers nothing.
   */
  sessionEnded(): void;
}

/** The desktop app's session bridge, where the page runs in its window, and `null` in a browser. */
export function desktopSession(): DesktopSession | null {
  const bridge = (globalThis as { coffretDesktop?: unknown }).coffretDesktop;
  if (typeof bridge !== 'object' || bridge === null) {
    return null;
  }
  const offered = bridge as Partial<DesktopSession>;
  return typeof offered.onTicket === 'function' && typeof offered.sessionEnded === 'function'
    ? (bridge as DesktopSession)
    : null;
}
