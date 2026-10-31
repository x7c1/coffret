import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react';

import type { Action, ActionItem } from './actions';
import { COLOR, SHADOW } from './theme';

/** Where the menu opens: a point on the screen, and which side of it the menu hangs from. */
export interface MenuAt {
  x: number;
  y: number;
  /**
   * `start`: the menu's top left corner at the point — the pointer of a
   * right-click. `end`: its top right corner there — under a "⋮" at a row's
   * right end, so that it opens over the row rather than past the window.
   */
  side: 'start' | 'end';
  /**
   * Opened from the keyboard, by `Enter` or Space on a "⋮": the first item is
   * shaded from the start, so that `Enter` again chooses it rather than
   * nothing.
   */
  byKeys?: boolean;
}

/**
 * The actions menu: a small popover at the pointer, or under a row's "⋮",
 * listing what the rows it was opened on offer (see [`actionsFor`](./actions)).
 *
 * One menu for the whole tree rather than one per row, since one is open at a
 * time and a right-click opens it wherever the pointer is. Drawn fixed to the
 * window and above the reader's sheet: while the sheet covers the right part
 * of the tree, a right-click on the strip left uncovered is the way in, and the
 * menu may well hang over the sheet.
 *
 * It closes on a click anywhere outside it, on `Escape`, and on choosing an
 * item. Its title — what it acts on — heads it, and is not one of its items.
 * The item under the pointer or the keyboard's focus is shaded; `↑`/`↓`
 * move that focus and `Enter` chooses, so the menu works from the keyboard
 * as a menu does. A destructive item is drawn in red, and shaded in a tint of
 * it. An item of a choice — the header's appearance switch — is a radio item,
 * the chosen one checked. An item with no action is drawn faint and disabled:
 * the pointer and the keys pass over it, and nothing chooses it.
 *
 * Kept on the screen: where it would run past the bottom of the window it opens
 * upward from the point instead, and where past the right edge, leftward.
 * Decided once it is drawn and before it is painted, since its size is what is
 * measured.
 */
export function ActionsMenu({
  title,
  label,
  at,
  items,
  onChoose,
  onClose,
}: {
  /**
   * The line heading the menu: what it acts on — `cover.png`, `3 items`, `Add
   * to albums`. Drawn and not read out: `label` says it, with what the menu is.
   */
  title: string;
  /** What the menu is read out as: `actions for cover.png`, `actions for 3 items`. */
  label: string;
  at: MenuAt;
  items: readonly ActionItem[];
  /** An item that can be chosen was: what it does. */
  onChoose: (action: Action) => void;
  /**
   * The menu is let go of: by `Escape`, after which the keys belong where they
   * were before it opened, or by a press `away` from it, which puts the focus
   * where it pressed.
   */
  onClose: (how: 'escape' | 'away') => void;
}) {
  const menu = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState<{ left: number; top: number } | null>(null);
  // The first item that can be chosen, which the keys start on.
  const first = items.findIndex((item) => item.action !== null);
  const [active, setActive] = useState(at.byKeys === true ? first : -1);

  useLayoutEffect(() => {
    const element = menu.current;
    if (element === null) {
      return;
    }
    // Opened again elsewhere without closing first — from another row's "⋮" —
    // the shade starts over, since the items under it may be others.
    setActive(at.byKeys === true ? first : -1);
    const box = element.getBoundingClientRect();
    setPlace(placed(at, { width: box.width, height: box.height }, {
      width: window.innerWidth,
      height: window.innerHeight,
    }));
  }, [at, first]);
  // The menu takes the keyboard, so that `↑`/`↓` and `Enter` reach it — once
  // it is placed, since until then it is hidden, and a hidden element takes
  // no focus; and again each time it is placed anew, since the "⋮" pressed to
  // open it elsewhere took the focus on the way.
  useEffect(() => {
    if (place !== null) {
      menu.current?.focus();
    }
  }, [place]);

  useEffect(() => {
    const away = (event: MouseEvent) => {
      if (!(event.target instanceof Node) || !menu.current?.contains(event.target)) {
        onClose('away');
      }
    };
    const escape = (event: globalThis.KeyboardEvent) => {
      if (event.key === 'Escape') {
        // Taken: the reader's sheet, open behind the menu, closes on an
        // `Escape` nothing else took, and this one was the menu's.
        event.preventDefault();
        onClose('escape');
      }
    };
    document.addEventListener('mousedown', away);
    document.addEventListener('keydown', escape);
    return () => {
      document.removeEventListener('mousedown', away);
      document.removeEventListener('keydown', escape);
    };
  }, [onClose]);

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      setActive((was) => stepped(items, was, event.key === 'ArrowDown' ? 1 : -1));
    } else if (event.key === 'Enter' && active !== -1) {
      event.preventDefault();
      const action = items[active]?.action ?? null;
      if (action !== null) {
        onChoose(action);
      }
    }
  };

  return (
    <div
      ref={menu}
      role="menu"
      aria-label={label}
      tabIndex={-1}
      onKeyDown={onKeyDown}
      // Nothing under the menu answers its right-click: the browser's own menu
      // over this one would be a menu over a menu.
      onContextMenu={(event) => event.preventDefault()}
      style={{
        ...MENU,
        left: place?.left ?? at.x,
        top: place?.top ?? at.y,
        visibility: place === null ? 'hidden' : 'visible',
      }}
    >
      <div aria-hidden style={TITLE}>
        {title}
      </div>
      {items.map((item, index) => {
        const shaded = index === active;
        const action = item.action;
        if (action === null) {
          return (
            <div key={item.label}>
              {item.separated && <div role="separator" style={SEPARATOR} />}
              <button
                type="button"
                role="menuitem"
                tabIndex={-1}
                disabled
                aria-disabled
                style={{ ...ITEM, color: COLOR.faint, background: 'transparent', cursor: 'default' }}
              >
                {item.label}
              </button>
            </div>
          );
        }
        return (
          <div key={item.label}>
            {item.separated && <div role="separator" style={SEPARATOR} />}
            <button
              type="button"
              role={item.checked === undefined ? 'menuitem' : 'menuitemradio'}
              aria-checked={item.checked}
              tabIndex={-1}
              onMouseEnter={() => setActive(index)}
              onMouseLeave={() => setActive((was) => (was === index ? -1 : was))}
              onClick={() => onChoose(action)}
              style={{
                ...ITEM,
                color: item.destructive ? COLOR.refused : COLOR.text,
                background: shaded ? (item.destructive ? COLOR.refusedGround : COLOR.hover) : 'transparent',
              }}
            >
              {item.checked !== undefined && <Check shown={item.checked} />}
              {item.label}
            </button>
          </div>
        );
      })}
    </div>
  );
}

/**
 * The mark before an item of a choice: a check where it is the one chosen, and
 * the same room left empty where it is not, so the labels stand in one column.
 */
function Check({ shown }: { shown: boolean }) {
  return (
    <svg
      aria-hidden
      width={14}
      height={14}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      style={{ flex: '0 0 auto', marginRight: 8, visibility: shown ? 'visible' : 'hidden' }}
    >
      <path d="M5 12.5l4.5 4.5L19 7.5" />
    </svg>
  );
}

/**
 * The item `↑` (`step` -1) or `↓` (1) shades after `was`, `-1` being none:
 * the next one round that can be chosen, and none where no item can be.
 */
function stepped(items: readonly ActionItem[], was: number, step: 1 | -1): number {
  const count = items.length;
  let at = was === -1 && step === -1 ? count : was;
  for (let tried = 0; tried < count; tried += 1) {
    at = (at + step + count) % count;
    if (items[at].action !== null) {
      return at;
    }
  }
  return -1;
}

/**
 * Where a menu of `size` goes for `at` in a window of `room`: hung from the
 * point on the side asked, and turned upward or leftward where it would run
 * past the window's bottom or right edge — never above its top or past its
 * left edge.
 */
export function placed(
  at: MenuAt,
  size: { width: number; height: number },
  room: { width: number; height: number },
): { left: number; top: number } {
  let left = at.side === 'end' ? at.x - size.width : at.x;
  if (left + size.width > room.width - EDGE) {
    left = at.x - size.width;
  }
  let top = at.y;
  if (top + size.height > room.height - EDGE) {
    top = at.y - size.height;
  }
  return { left: Math.max(EDGE, left), top: Math.max(EDGE, top) };
}

/** The room kept between the menu and the window's edge. */
const EDGE = 4;

const MENU: CSSProperties = {
  position: 'fixed',
  zIndex: 20,
  boxSizing: 'border-box',
  width: 232,
  padding: 6,
  background: COLOR.panel,
  border: `1px solid ${COLOR.border}`,
  borderRadius: 8,
  boxShadow: SHADOW.menu,
  outline: 'none',
  textAlign: 'left',
};

/** What the menu acts on, over its items: cut short where it runs past the menu's width. */
const TITLE: CSSProperties = {
  padding: '4px 10px 6px',
  fontSize: 12,
  color: COLOR.dim,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
};

const ITEM: CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  width: '100%',
  minHeight: 34,
  border: 'none',
  borderRadius: 6,
  font: 'inherit',
  fontSize: 13,
  padding: '0 10px',
  textAlign: 'left',
  whiteSpace: 'nowrap',
  cursor: 'pointer',
};

const SEPARATOR: CSSProperties = {
  height: 1,
  margin: '6px 4px',
  background: COLOR.rowRule,
};
