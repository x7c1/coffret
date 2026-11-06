import { useCallback, useEffect, useRef, useState, type ComponentType } from 'react';

import {
  checkSession,
  dropToken,
  heldToken,
  isRefusal,
  onSessionEnding,
  withdrawAnswers,
  type Refusal,
} from '@coffret/api';

import { App } from './App';
import { renew, type DesktopSession, type Opened, type Shelf } from './session';
import { COLOR } from './theme';

/** How often a page whose Library locked asks the host whether it is unlocked. */
export const LOCKED_POLL_MS = 2_000;

/** What a page holding no token says, in place of everything else. */
export const NO_SESSION = 'The explorer holds no session, so it shows nothing of the Library.';

/**
 * How the explorer is opened again, which is the one thing such a page can
 * offer: only the app can give the page a fresh ticket.
 */
export const HOW_TO_OPEN =
  'Reload the explorer from the coffret app to open it again — or, in development, open the ' +
  'launch file the development host printed.';

/** What a page whose session ended under it says, where the host gave no reason of its own. */
export const ENDED = 'The explorer’s session ended: it ran out, was revoked, or the explorer was started again.';

/** Where the page stands with the explorer host. */
type Standing =
  | { kind: 'opening' }
  | { kind: 'none'; said: string | null }
  | {
      kind: 'held';
      /**
       * Which drawing of the screen this is. A new one is drawn — everything
       * the last one held dropped, its object URLs revoked with the reader —
       * when the Library locks and when it is unlocked again.
       */
      round: number;
      locked: boolean;
    };

/**
 * The explorer, behind the session that admits it.
 *
 * Until the page holds a browser token there is no screen at all, only how the
 * explorer is opened, and nothing under `/api` is asked (spec: LA-21). Once it
 * holds one, the screen is drawn, and what ends the session, or locks the
 * Library under it, ends the screen:
 *
 * - **ended** — the token admits nothing any more, the server behind the
 *   host stopped, or the session ran out. The token is let go of and
 *   forgotten, and the screen goes with everything it held.
 * - **locked** — the Library locked under a session that still stands. The
 *   screen is drawn afresh, which drops the listing, what the server was doing
 *   and every page the reader held, and what it asks is refused as locked,
 *   which the screen draws as its locked state (see
 *   [`LockedScreen`](./Locked)). Meanwhile the host's
 *   session route is asked, and never the server, until the Library is
 *   unlocked (spec: DK-4); then the screen is drawn afresh again.
 *
 * Either way, an answer to a request of the screen before is let go of rather
 * than drawn, and nothing that was refused is sent again.
 *
 * In the desktop app's window (`desktop`), the session is kept from ending
 * under the page (spec: LA-18). The app hands the page a fresh ticket ahead
 * of its token's expiry, and the page exchanges it and holds the new token in
 * place of the old one: the screen is not drawn again, and keeps what it
 * shows. Where the session ends anyway, or never came, or the exchange of a
 * delivered ticket is refused, the page tells the app, which loads the window
 * afresh from a ticket of its own where it can (spec: LA-21).
 *
 * The page offers no control that ends its own session: disconnecting is the
 * desktop app's, by quitting, for every session at once.
 */
export function Explorer({
  opened,
  shelved,
  screen: Screen = App,
  desktop = null,
}: {
  /** How the page came up, asked once as it loaded. */
  opened: Promise<Opened>;
  /** Where the page keeps its token, which ending its session empties. */
  shelved: Shelf;
  /** What is drawn while a token is held: the explorer's own screen. */
  screen?: ComponentType;
  /** The desktop app's window, where the page runs in one, and `null` in a browser. */
  desktop?: DesktopSession | null;
}) {
  const [standing, setStanding] = useState<Standing>({ kind: 'opening' });
  // When the token held runs out: a new object for each token taken up, so
  // that a renewal to a session as long as the one before still starts the
  // clock again.
  const [expiry, setExpiry] = useState<{ seconds: number } | null>(null);

  useEffect(() => {
    let live = true;
    void opened.then((came) => {
      if (!live) {
        return;
      }
      if (came.kind === 'held') {
        setExpiry({ seconds: came.expiresIn });
        setStanding({ kind: 'held', round: 0, locked: false });
      } else {
        setStanding({ kind: 'none', said: came.said });
      }
    });
    return () => {
      live = false;
    };
  }, [opened]);

  // Whether the screen is drawn as locked, ahead of the render that draws it:
  // several requests refused as locked arrive together, and the screen is
  // drawn afresh once for all of them.
  const lockedNow = useRef(false);

  const endSession = useCallback(
    (said: string | null) => {
      lockedNow.current = false;
      dropToken();
      shelved.forgetAll();
      setExpiry(null);
      setStanding({ kind: 'none', said });
    },
    [shelved],
  );

  const held = standing.kind === 'held';
  const locked = standing.kind === 'held' && standing.locked;

  // What the answers say about the session, for as long as there is one.
  useEffect(() => {
    if (!held) {
      return;
    }
    return onSessionEnding((ending, refusal) => {
      if (ending === 'ended') {
        endSession(endedBy(refusal));
        return;
      }
      if (lockedNow.current) {
        return;
      }
      lockedNow.current = true;
      withdrawAnswers();
      setStanding((before) =>
        before.kind === 'held' ? { kind: 'held', round: before.round + 1, locked: true } : before,
      );
    });
  }, [held, endSession]);

  // A session runs out on the host's clock, and is never extended
  // (spec: LA-18): unless a new token was taken up first, the screen goes when
  // it does, whether or not anything was being asked at the time.
  useEffect(() => {
    if (!held || expiry === null) {
      return;
    }
    const timer = window.setTimeout(() => endSession(ENDED), expiry.seconds * 1000);
    return () => window.clearTimeout(timer);
  }, [held, expiry, endSession]);

  // A fresh ticket the desktop app hands over is exchanged for a new token,
  // held in place of the old one without drawing the screen again — locked or
  // not, since a locked Library keeps its sessions (spec: LA-18). An exchange
  // refused ends the session, which tells the app.
  useEffect(() => {
    if (!held || desktop === null) {
      return;
    }
    let live = true;
    const stop = desktop.onTicket((ticket) => {
      void renew(ticket, shelved).then((renewed) => {
        if (!live) {
          // The session ended, or the page went, while the exchange was on
          // its way: a token it brought is not held on.
          if (renewed.kind === 'renewed') {
            dropToken();
            shelved.forgetAll();
          }
          return;
        }
        if (renewed.kind === 'renewed') {
          setExpiry({ seconds: renewed.expiresIn });
          return;
        }
        console.warn('the explorer could not renew its session', renewed.said);
        endSession(renewed.said ?? ENDED);
      });
    });
    return () => {
      live = false;
      stop();
    };
  }, [held, desktop, shelved, endSession]);

  // A page in the desktop app's window holding no session tells the app,
  // which loads it afresh where it can, rather than leaving this page shown.
  const none = standing.kind === 'none';
  useEffect(() => {
    if (none && desktop !== null) {
      desktop.sessionEnded();
    }
  }, [none, desktop]);

  // While the Library is locked, the host is asked — not the server, and not
  // through anything that would keep the Library open — whether it is still.
  useEffect(() => {
    if (!locked) {
      return;
    }
    const aborter = new AbortController();
    const ask = () => {
      const token = heldToken();
      if (token === null) {
        return;
      }
      checkSession(token, aborter.signal).then(
        (session) => {
          if (aborter.signal.aborted || session.library !== 'unlocked') {
            return;
          }
          lockedNow.current = false;
          withdrawAnswers();
          setStanding((before) =>
            before.kind === 'held' ? { kind: 'held', round: before.round + 1, locked: false } : before,
          );
        },
        (refused: unknown) => {
          if (aborter.signal.aborted) {
            return;
          }
          // A token renewed away while it was being asked about was revoked on
          // purpose, and says nothing of the session its successor holds.
          if (isRefusal(refused) && refused.kind === 'authentication_required' && heldToken() === token) {
            endSession(ENDED);
          }
          // Anything else — the host not answering a moment — is asked again
          // at the next interval.
        },
      );
    };
    ask();
    const timer = window.setInterval(ask, LOCKED_POLL_MS);
    return () => {
      window.clearInterval(timer);
      aborter.abort();
    };
  }, [locked, endSession]);

  if (standing.kind === 'opening') {
    return <Notice lines={['Opening the explorer…']} />;
  }
  if (standing.kind === 'none') {
    return <Notice lines={[NO_SESSION, HOW_TO_OPEN]} said={standing.said} />;
  }
  return <Screen key={standing.round} />;
}

/** What a refusal that ended the session is said as. */
function endedBy(refusal: Refusal): string {
  return refusal.kind === 'server_unavailable' ? refusal.message : ENDED;
}

/** The page shown without a session: what it is, and how to open the explorer. */
function Notice({ lines, said = null }: { lines: string[]; said?: string | null }) {
  return (
    <main
      style={{
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        justifyContent: 'center',
        alignItems: 'center',
        gap: 8,
        padding: 24,
        boxSizing: 'border-box',
        textAlign: 'center',
      }}
    >
      {said !== null && <p style={{ color: COLOR.refused, margin: 0 }}>{said}</p>}
      {lines.map((line) => (
        <p key={line} style={{ margin: 0 }}>
          {line}
        </p>
      ))}
    </main>
  );
}
