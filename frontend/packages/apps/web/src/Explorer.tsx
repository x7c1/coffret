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
import type { Opened, Shelf } from './session';
import { COLOR } from './theme';

/** How often a page whose Library locked asks the host whether it is unlocked. */
export const LOCKED_POLL_MS = 2_000;

/** What a page holding no token says, in place of everything else. */
export const NO_SESSION = 'This tab holds no session with the explorer, so it shows nothing of the Library.';

/**
 * How the explorer is opened again, which is the one thing such a page can
 * offer: only the app can give the page a fresh ticket.
 */
export const HOW_TO_OPEN =
  'Reload the explorer from the coffret app to open it again — or, in development, open the ' +
  'launch file the development host printed.';

/** What a tab whose session ended under it says, where the host gave no reason of its own. */
const ENDED = 'This tab’s session ended: it ran out, was revoked, or the explorer was started again.';

/** Where the tab stands with the explorer host. */
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
 * Until the tab holds a browser token there is no screen at all, only how the
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
 *   which is the locked state each region already shows. Meanwhile the host's
 *   session route is asked, and never the server, until the Library is
 *   unlocked (spec: DK-4); then the screen is drawn afresh again.
 *
 * Either way, an answer to a request of the screen before is let go of rather
 * than drawn, and nothing that was refused is sent again.
 *
 * The page offers no control that ends its own session: disconnecting is the
 * desktop app's, from its tray, for every tab at once.
 */
export function Explorer({
  opened,
  shelved,
  screen: Screen = App,
}: {
  /** How the tab came up, asked once as the page loaded. */
  opened: Promise<Opened>;
  /** Where the tab keeps its token, which ending its session empties. */
  shelved: Shelf;
  /** What is drawn while a token is held: the explorer's own screen. */
  screen?: ComponentType;
}) {
  const [standing, setStanding] = useState<Standing>({ kind: 'opening' });
  const [expiresIn, setExpiresIn] = useState<number | null>(null);

  useEffect(() => {
    let live = true;
    void opened.then((came) => {
      if (!live) {
        return;
      }
      if (came.kind === 'held') {
        setExpiresIn(came.expiresIn);
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
      setExpiresIn(null);
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

  // A session runs out on the host's clock, and nothing renews it
  // (spec: LA-18): the screen goes when it does, whether or not anything was
  // being asked at the time.
  useEffect(() => {
    if (!held || expiresIn === null) {
      return;
    }
    const timer = window.setTimeout(() => endSession(ENDED), expiresIn * 1000);
    return () => window.clearTimeout(timer);
  }, [held, expiresIn, endSession]);

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
          if (isRefusal(refused) && refused.kind === 'authentication_required') {
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
  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      <div style={{ flex: 1, minHeight: 0 }}>
        <Screen key={standing.round} />
      </div>
      {standing.locked && <LockedBar />}
    </div>
  );
}

/** What a refusal that ended the session is said as. */
function endedBy(refusal: Refusal): string {
  return refusal.kind === 'server_unavailable' ? refusal.message : ENDED;
}

/** The page a tab without a session shows: what it is, and how to open the explorer. */
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

/** The line under the screen while the Library is locked under this tab's session. */
function LockedBar() {
  return (
    <div
      style={{
        padding: '2px 12px',
        fontSize: 12,
        textAlign: 'right',
        color: COLOR.dim,
        background: COLOR.panel,
        borderTop: `1px solid ${COLOR.border}`,
      }}
    >
      the Library is locked; this tab waits for it to be unlocked
    </div>
  );
}
