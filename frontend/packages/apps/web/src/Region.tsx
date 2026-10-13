import type { ReactNode } from 'react';

import { ReconnectOffer, type Offer } from './ReconnectOffer';
import { UNLOCK, type UnlockOffer } from './StatusBar';
import { COLOR } from './theme';
import type { Asked } from './useAsked';

/** What the region offers in place of "unlock" where the refusal is anything else. */
export const TRY_AGAIN = 'try again';

/**
 * One region of the screen, in whichever of its three states it is in.
 *
 * Loading and failed are both states the screen can be left from, which is the
 * point of stating them: a request that failed says what the server said and
 * offers to ask again, rather than leaving the region saying "loading" for as
 * long as the tab is open.
 *
 * Except where what refused it is the lock. Asking again only earns the same
 * refusal while the Library stays locked, so a *try again* there is a button
 * that changes nothing however often it is pressed. The region offers the
 * unlock instead — the same ask the status bar's button makes — and says what
 * that ask came to beside it, so that a server with no window to bring forward
 * (one started from the command line) answers with where to unlock rather than
 * with the refusal the region is already showing.
 */
export function Region<T>({
  state,
  onRetry,
  reconnect,
  unlock,
  children,
}: {
  state: Asked<T>;
  onRetry: () => void;
  /** The reconnect offered where the refusal is a permission that ran out. */
  reconnect: Offer | null;
  /** The unlock offered where the refusal is the locked one. */
  unlock: UnlockOffer;
  children: (value: T) => ReactNode;
}) {
  switch (state.status) {
    case 'loading':
      return <p style={{ padding: 16, color: COLOR.dim }}>loading…</p>;
    case 'failed':
      return (
        <div style={{ padding: 16 }}>
          <p style={{ color: COLOR.refused }}>{state.message}</p>
          {state.ranOut === true && reconnect !== null && (
            <p style={{ color: COLOR.text, fontSize: 13 }}>
              <ReconnectOffer offer={reconnect} />
            </p>
          )}
          {state.locked === true ? (
            <>
              <button
                onClick={unlock.ask}
                disabled={unlock.asking}
                style={{ ...BUTTON, cursor: unlock.asking ? 'default' : 'pointer' }}
              >
                {UNLOCK}
              </button>
              {unlock.refused !== null ? (
                <p style={{ color: COLOR.refused, fontSize: 13 }}>{unlock.refused}</p>
              ) : (
                unlock.said !== null && (
                  <p style={{ color: COLOR.text, fontSize: 13 }}>{unlock.said}</p>
                )
              )}
            </>
          ) : (
            <button onClick={onRetry} style={BUTTON}>
              {TRY_AGAIN}
            </button>
          )}
        </div>
      );
    case 'ready':
      return <>{children(state.value)}</>;
  }
}

const BUTTON = {
  border: `1px solid ${COLOR.border}`,
  background: COLOR.panel,
  color: COLOR.text,
  font: 'inherit',
  padding: '4px 12px',
  borderRadius: 4,
  cursor: 'pointer',
};
