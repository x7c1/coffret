import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from 'react';

import { COLOR } from './theme';

/** One thing a row's menu offers. */
export interface MenuItem {
  label: string;
  onPress: () => void;
}

/**
 * What a row offers besides opening, behind one "⋮".
 *
 * A button per action on every row is a list of twenty rows showing twenty
 * buttons, which is restless to look at for something pressed now and then;
 * one quiet mark per row says the same and keeps the names the thing being
 * read. The items are a list so that what a row offers can grow without the
 * row changing shape. A row with nothing to offer draws no mark at all.
 *
 * The same menu stands on the file list's rows and on the folder tree's, so
 * that a folder deleted from either is asked about the same way.
 *
 * The menu closes on a click anywhere outside it, on `Escape`, and on choosing
 * an item. Whatever the row under it does with a click is the row's to keep
 * away from the menu: the file list stops the click at the menu's cell.
 *
 * It hangs below the mark, and above it where the scroll area it is in has no
 * room below for it and more above — the last rows of a list scrolled to its
 * end, where a menu hung below would be cut off by the bottom of the area.
 * Decided once it is drawn and before it is painted, since its height is what
 * is measured.
 */
export function RowMenu({ name, items }: { name: string; items: readonly MenuItem[] }) {
  const [open, setOpen] = useState(false);
  const [upward, setUpward] = useState(false);
  const here = useRef<HTMLDivElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (!open || here.current === null || menu.current === null) {
      setUpward(false);
      return;
    }
    setUpward(opensUpward(here.current, menu.current));
  }, [open]);
  useEffect(() => {
    if (!open) {
      return;
    }
    const away = (event: MouseEvent) => {
      if (!(event.target instanceof Node) || !here.current?.contains(event.target)) {
        setOpen(false);
      }
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', away);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('mousedown', away);
      document.removeEventListener('keydown', escape);
    };
  }, [open]);

  if (items.length === 0) {
    return null;
  }
  return (
    <div ref={here} style={{ position: 'relative', display: 'inline-block' }}>
      <button
        type="button"
        aria-label={`actions for ${name}`}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((was) => !was)}
        style={MENU_BUTTON}
      >
        ⋮
      </button>
      {open && (
        <div ref={menu} role="menu" style={upward ? MENU_UPWARD : MENU}>
          {items.map((item) => (
            <button
              key={item.label}
              type="button"
              role="menuitem"
              onClick={() => {
                setOpen(false);
                item.onPress();
              }}
              style={MENU_ITEM}
            >
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * Whether a menu hung below `mark` would run past the bottom of the scroll area
 * it is in, where there is more room above the mark than below it.
 */
function opensUpward(mark: HTMLElement, menu: HTMLElement): boolean {
  const area = mark.closest('[data-scroll-area]') ?? document.documentElement;
  const bounds = area.getBoundingClientRect();
  const at = mark.getBoundingClientRect();
  const height = menu.getBoundingClientRect().height;
  const below = bounds.bottom - at.bottom;
  const above = at.top - bounds.top;
  return height > below && above > below;
}

const MENU_BUTTON: CSSProperties = {
  border: 'none',
  background: 'transparent',
  color: COLOR.dim,
  font: 'inherit',
  fontSize: 14,
  lineHeight: 1,
  padding: '0 6px',
  borderRadius: 4,
  cursor: 'pointer',
};

const MENU: CSSProperties = {
  position: 'absolute',
  top: '100%',
  right: 0,
  zIndex: 2,
  minWidth: 120,
  padding: '4px 0',
  background: COLOR.panel,
  border: `1px solid ${COLOR.border}`,
  borderRadius: 4,
  boxShadow: '0 2px 8px rgba(0, 0, 0, 0.35)',
  textAlign: 'left',
};

/** The same menu, hung above the mark where there is no room below it. */
const MENU_UPWARD: CSSProperties = { ...MENU, top: undefined, bottom: '100%' };

const MENU_ITEM: CSSProperties = {
  display: 'block',
  width: '100%',
  border: 'none',
  background: 'transparent',
  color: COLOR.text,
  font: 'inherit',
  fontSize: 12,
  padding: '4px 12px',
  textAlign: 'left',
  cursor: 'pointer',
};
