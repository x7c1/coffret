import type { ReactNode } from 'react';

import { ReconnectOffer, type Offer } from './ReconnectOffer';
import { COLOR } from './theme';
import type { Asked } from './useAsked';

/** What a region refused offers: asking again. */
export const TRY_AGAIN = 'try again';

/**
 * One region of the screen, in whichever of its three states it is in.
 *
 * Loading and failed are both states the screen can be left from, which is the
 * point of stating them: a request that failed says what the server said and
 * offers to ask again, rather than leaving the region saying "loading" for as
 * long as the tab is open.
 *
 * A refusal by the lock is not one of these: asking again only earns the same
 * refusal while the Library stays locked, so the screen draws its locked state
 * in place of the regions instead (see [`LockedScreen`](./Locked)).
 */
export function Region<T>({
  state,
  onRetry,
  reconnect,
  children,
}: {
  state: Asked<T>;
  onRetry: () => void;
  /** The reconnect offered where the refusal is a permission that ran out. */
  reconnect: Offer | null;
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
          <button onClick={onRetry} style={BUTTON}>
            {TRY_AGAIN}
          </button>
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
