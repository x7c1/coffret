import type { CSSProperties } from 'react';

import { PRIMARY } from './Dialog';
import { COLOR } from './theme';

/** What the main area says while the Library is locked, as its heading. */
export const LOCKED_HEADING = 'The Library is locked';

/** The two lines under it: what the lock does, then what to do about it. */
export const LOCKED_LINES = ['Its files stay hidden until it is unlocked.', 'Unlock it to browse and read again.'] as const;

/** What the Activity row says while the Library is locked, in place of the summary. */
export const LOCKED = 'Locked';

/** The button that asks for the unlock, wherever it is offered. */
export const UNLOCK = 'Unlock';

/**
 * The unlock the screen offers while the Library is locked: the button,
 * whether a press is still waiting for its answer, and what the last one came
 * to.
 */
export interface UnlockOffer {
  /** Whether a press is waiting for the server's answer. */
  asking: boolean;
  /** What the server answered the last press with, and `null` before any. */
  said: string | null;
  /** What refused the last press, and `null` where none was. */
  refused: string | null;
  /** Presses the button. */
  ask: () => void;
}

/**
 * The main area while the Library is locked, in place of the tree and the
 * details pane: a lock in a tile of the accent's tint, what the lock means,
 * and the one thing to do about it.
 *
 * Drawn in the screen's quiet colours and not in a refusal's: a lock is the
 * Library keeping its files safe while nobody uses it, and nothing went wrong.
 * What the last press came to stands under the button — the server's word
 * that the app is asking for the Passphrase in its own window, or, from a
 * server with no window to bring forward, what refused the press, which says
 * to start the server again — so that pressing it is never met with nothing.
 * That refusal is drawn quietly too: it is how this server is unlocked, not a
 * failure of anything.
 */
export function LockedScreen({ unlock }: { unlock: UnlockOffer }) {
  return (
    <section aria-labelledby="locked-heading" style={SCREEN}>
      <span style={TILE}>
        <LockIcon size={24} />
      </span>
      <h2 id="locked-heading" style={HEADING}>
        {LOCKED_HEADING}
      </h2>
      <p style={LINES}>
        {LOCKED_LINES.map((line) => (
          <span key={line} style={{ display: 'block' }}>
            {line}
          </span>
        ))}
      </p>
      <button
        type="button"
        onClick={unlock.ask}
        disabled={unlock.asking}
        style={{ ...PRIMARY, marginTop: 20, cursor: unlock.asking ? 'default' : 'pointer' }}
      >
        {UNLOCK}
      </button>
      {unlock.refused !== null ? (
        <p style={SAID}>{unlock.refused}</p>
      ) : (
        unlock.said !== null && <p style={SAID}>{unlock.said}</p>
      )}
    </section>
  );
}

/** A padlock, drawn in `currentColor`. */
export function LockIcon({ size }: { size: number }) {
  return (
    <svg
      aria-hidden="true"
      focusable="false"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ display: 'block' }}
    >
      <rect x={4} y={11} width={16} height={10} rx={2} />
      <path d="M8 11V7a4 4 0 0 1 8 0v4" />
    </svg>
  );
}

const SCREEN: CSSProperties = {
  flex: 1,
  minHeight: 0,
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'center',
  padding: 24,
  textAlign: 'center',
};

/** The round tile the lock stands in. */
const TILE: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: 56,
  height: 56,
  borderRadius: '50%',
  background: COLOR.accentTint,
  color: COLOR.accent,
};

const HEADING: CSSProperties = {
  margin: '16px 0 0',
  fontSize: 18,
  fontWeight: 600,
  color: COLOR.text,
};

const LINES: CSSProperties = {
  margin: '8px 0 0',
  fontSize: 14,
  lineHeight: 1.5,
  color: COLOR.dim,
};

/** What the last press came to, under the button. */
const SAID: CSSProperties = {
  margin: '12px 0 0',
  maxWidth: 480,
  fontSize: 13,
  color: COLOR.dim,
};
