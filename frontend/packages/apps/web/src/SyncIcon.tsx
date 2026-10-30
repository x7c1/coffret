import { CHANGES_WAITING, CHECKING, LOOKING } from './refresh';
import { COLOR } from './theme';

/**
 * The control that asks the Library what is new, drawn as a sync icon.
 *
 * Three ways to be drawn, and its name says which, because the drawing alone
 * says it only to somebody who can see the colour:
 *
 * - **plain** — the bar's own colour, called "Check for changes";
 * - **changes waiting** — the last look at the remote head said another
 *   device has committed since this one caught up: drawn in the accent colour
 *   with a dot in the corner, and called so;
 * - **looking** — the catch-up a press started is running: the arrows turn,
 *   and the icon cannot be pressed again until it lands.
 *
 * Drawn in `currentColor`, so the plain icon is whatever colour the bar's text
 * is and nothing here has to know which.
 */
export function SyncIcon({
  running,
  waiting,
  onPress,
}: {
  /** Whether the catch-up a press started is still running. */
  running: boolean;
  /** Whether the last look said there are changes this device has not taken in. */
  waiting: boolean;
  onPress: () => void;
}) {
  const label = running ? LOOKING : waiting ? CHANGES_WAITING : CHECKING;
  return (
    <button
      type="button"
      onClick={onPress}
      disabled={running}
      title={label}
      aria-label={label}
      style={{
        ...BUTTON,
        color: waiting && !running ? COLOR.accent : 'inherit',
        cursor: running ? 'default' : 'pointer',
      }}
    >
      <svg
        width={16}
        height={16}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={2}
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
        focusable="false"
      >
        <g>
          {/* SMIL rather than a stylesheet's keyframes: the explorer draws in
              inline styles, and a turn is the one thing an inline style cannot
              say. */}
          {running && (
            <animateTransform
              attributeName="transform"
              type="rotate"
              from="0 12 12"
              to="360 12 12"
              dur="1s"
              repeatCount="indefinite"
            />
          )}
          <path d="M3 12a9 9 0 0 1 15.4-6.4L21 8" />
          <path d="M21 3v5h-5" />
          <path d="M21 12a9 9 0 0 1-15.4 6.4L3 16" />
          <path d="M3 21v-5h5" />
        </g>
        {waiting && !running && <circle cx={20} cy={20} r={3.5} fill="currentColor" stroke="none" />}
      </svg>
    </button>
  );
}

/** The icon's button: no frame, so the icon is the control. */
const BUTTON = {
  display: 'inline-flex',
  alignItems: 'center',
  border: 'none',
  background: 'none',
  padding: 2,
  borderRadius: 4,
  font: 'inherit',
} as const;
