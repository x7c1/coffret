import { useEffect, useState, useSyncExternalStore, type CSSProperties } from 'react';

import { getAppearance, setAppearance, type Appearance } from '@coffret/api';

import type { Action, ActionItem } from './actions';
import { ActionsMenu, type MenuAt } from './ActionsMenu';
import { SHOWN_HERE, type ShownAppearance } from './ShownAppearance';
import { COLOR } from './theme';

/** Where the choice is kept for this device: the server's two calls about it. */
export interface KeptAppearance {
  read: (signal: AbortSignal) => Promise<Appearance>;
  write: (appearance: Appearance) => Promise<Appearance>;
}

/** The server's, which is what everything but a test keeps the choice in. */
const ON_THE_SERVER: KeptAppearance = { read: getAppearance, write: setAppearance };

/** What each choice is called, in the menu and in the button's name. */
export const APPEARANCE_LABEL: Record<Appearance, string> = {
  system: 'System',
  light: 'Light',
  dark: 'Dark',
};

/** The menu's items, in the order they are offered, the one shown checked. */
export function appearanceItems(shown: Appearance): ActionItem[] {
  return (['system', 'light', 'dark'] as const).map((appearance) => ({
    label: APPEARANCE_LABEL[appearance],
    action: { kind: 'appearance', appearance },
    destructive: false,
    separated: false,
    checked: appearance === shown,
  }));
}

/**
 * The header's appearance switch: an icon button opening a small menu of the
 * three sets the explorer can be drawn in — `System`, the default, `Light` and
 * `Dark` — the one shown checked.
 *
 * Its icon is what is shown: a sun for Light, a moon for Dark, and a circle
 * half filled for System, which is either as the system says. Its name says the
 * same in words, so it is read out as what it is and where it stands.
 *
 * What is chosen is drawn at once and kept on the server as this device's own
 * state, which is read as the switch is drawn (spec: LA-38). While the
 * Library is locked the host refuses both (spec: LA-14), so a page opened then
 * is drawn in `System`. The unlock draws the screen afresh, the switch with
 * it, which then reads the kept choice — or, where one was made on this page
 * while locked, keeps that one instead. A choice that cannot be kept is said to
 * the console: the next launch draws what was kept before it, which is no
 * worse than not choosing. A kept choice that cannot be read leaves the page
 * at `System`.
 */
export function AppearanceSwitch({
  onKeysBack,
  shown = SHOWN_HERE,
  kept = ON_THE_SERVER,
}: {
  /** The menu was let go of by `Escape` or by a choice: the keys go back to the tree. */
  onKeysBack: () => void;
  /** The page's appearance, which the switch shows and changes. */
  shown?: ShownAppearance;
  /** Where the choice is kept for this device. */
  kept?: KeptAppearance;
}) {
  const appearance = useSyncExternalStore(shown.subscribe, () => shown.appearance);
  const [open, setOpen] = useState<MenuAt | null>(null);

  useEffect(() => {
    // A choice made on this page is kept again each time the switch is
    // drawn — the screen is drawn afresh when the Library locks and when it
    // is unlocked — since one made while it was locked was refused by the
    // host (spec: LA-14) and would otherwise be lost at the next launch.
    if (shown.chosenHere) {
      kept.write(shown.appearance).catch((refused: unknown) => {
        console.warn('the explorer could not keep the appearance for this device', refused);
      });
      return;
    }
    const aborter = new AbortController();
    kept.read(aborter.signal).then(
      (answer) => shown.adopt(answer),
      (refused: unknown) => {
        if (!aborter.signal.aborted) {
          console.warn('the explorer could not read the appearance kept for this device', refused);
        }
      },
    );
    return () => aborter.abort();
  }, [kept, shown]);

  const choose = (action: Action) => {
    setOpen(null);
    // The keys go back to the tree, as they do out of the tree's own menu:
    // the item that held them is gone with the menu.
    onKeysBack();
    if (action.kind !== 'appearance') {
      return;
    }
    shown.choose(action.appearance);
    kept.write(action.appearance).catch((refused: unknown) => {
      console.warn('the explorer could not keep the appearance for this device', refused);
    });
  };

  const name = `Appearance: ${APPEARANCE_LABEL[appearance]}`;
  return (
    <>
      <button
        type="button"
        aria-label={name}
        title={name}
        aria-haspopup="menu"
        aria-expanded={open !== null}
        // The menu closes on a press outside it, which this is; the click
        // below opens it, or closes it where it is open.
        onMouseDown={(event) => event.stopPropagation()}
        onClick={(event) => {
          if (open !== null) {
            setOpen(null);
            return;
          }
          const box = event.currentTarget.getBoundingClientRect();
          // Hung from its right edge, as the **+**'s is: the button stands at
          // the window's right end.
          setOpen({ x: box.right, y: box.bottom, side: 'end', byKeys: event.detail === 0 });
        }}
        style={BUTTON}
      >
        <AppearanceIcon appearance={appearance} />
      </button>
      {open !== null && (
        <ActionsMenu
          title="Appearance"
          label="appearance"
          at={open}
          items={appearanceItems(appearance)}
          onChoose={choose}
          onClose={(how) => {
            setOpen(null);
            if (how === 'escape') {
              onKeysBack();
            }
          }}
        />
      )}
    </>
  );
}

/** A sun, a moon, or a circle half filled: what `appearance` is drawn as. */
function AppearanceIcon({ appearance }: { appearance: Appearance }) {
  const shared = {
    'aria-hidden': true,
    'data-appearance': appearance,
    width: 18,
    height: 18,
    viewBox: '0 0 24 24',
    fill: 'none',
    stroke: 'currentColor',
    strokeWidth: 2,
    strokeLinecap: 'round',
    strokeLinejoin: 'round',
    style: { display: 'block' },
  } as const;
  switch (appearance) {
    case 'light':
      return (
        <svg {...shared}>
          <circle cx={12} cy={12} r={4} />
          <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
        </svg>
      );
    case 'dark':
      return (
        <svg {...shared}>
          <path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z" />
        </svg>
      );
    case 'system':
      return (
        <svg {...shared}>
          <circle cx={12} cy={12} r={8.5} />
          <path d="M12 3.5a8.5 8.5 0 0 1 0 17z" fill="currentColor" />
        </svg>
      );
  }
}

/** A square as high as the **+**, drawn as a quiet button beside it. */
const BUTTON: CSSProperties = {
  flex: '0 0 auto',
  width: 32,
  height: 32,
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  border: `1px solid ${COLOR.border}`,
  background: 'transparent',
  color: COLOR.dim,
  padding: 0,
  borderRadius: 8,
  cursor: 'pointer',
};
