import { useEffect, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react';

import { RECENT_EXPLAINED, recentCount, type FilterSettings } from './filter';
import { COLOR } from './theme';

/**
 * The search field in the header, which filters the tree (see
 * [`askOf`](./filter)): inside it, a magnifier, a chip `Recent n ×` while the
 * most recent books are asked for, the name input, a × that empties the name
 * while there is one, a divider, and ⏷ (More filters), which opens a small
 * panel under the field holding the conditions besides the name — for now
 * Recent and its count — and a line saying what it does. `Escape` or a press
 * outside closes the panel, and what was set in it stays set.
 *
 * The keys, beside what is typed:
 *
 * - `Ctrl+F` (`Cmd+F` on macOS) and `/` put the focus in the name input from
 *   anywhere on the page, except while a modal dialog is open or another text
 *   input holds the focus — `/` typed into one is a character. The field is
 *   drawn only while the explorer is shown, so the browser's own find is
 *   taken from it only then.
 * - In the input, `↓` and `Enter` hand the focus to the first row of the tree.
 * - In the input, `Escape` empties the name, and the next — or the first,
 *   where it is empty already — gives the focus back to the tree. Recent is
 *   left as it is.
 */
export function FilterField({
  settings,
  onChange,
  onToTree,
  onKeysBack,
}: {
  settings: FilterSettings;
  onChange: (next: FilterSettings) => void;
  /** `↓` or `Enter` in the input: the focus to the tree's first row. */
  onToTree: () => void;
  /** The input let go of by `Escape`: the keys go back to the tree. */
  onKeysBack: () => void;
}) {
  const field = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const more = useRef<HTMLButtonElement>(null);
  const [panel, setPanel] = useState(false);
  // What the count's input holds as typed, which may for a moment be no count
  // at all — emptied to type another — while the settings keep the last one.
  const [count, setCount] = useState(String(settings.recent));
  useEffect(() => {
    setCount((typed) => (recentCount(typed) === settings.recent ? typed : String(settings.recent)));
  }, [settings.recent]);

  // `Ctrl/Cmd+F` and `/`, from anywhere on the page.
  useEffect(() => {
    const onKey = (event: globalThis.KeyboardEvent) => {
      const find =
        (event.key === 'f' || event.key === 'F') &&
        !event.altKey &&
        !event.shiftKey &&
        (MAC ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey);
      const slash = event.key === '/' && !event.ctrlKey && !event.metaKey && !event.altKey;
      if ((!find && !slash) || event.defaultPrevented) {
        return;
      }
      if (document.querySelector('[role="dialog"][aria-modal="true"]') !== null) {
        return;
      }
      const own = input.current;
      if (own === null) {
        return;
      }
      const holding = document.activeElement;
      if (holding === own) {
        // Already here: `/` is a character of the name, and `Ctrl/Cmd+F`
        // selects what is typed rather than opening the browser's find.
        if (find) {
          event.preventDefault();
          own.select();
        }
        return;
      }
      if (typesText(holding)) {
        return;
      }
      event.preventDefault();
      own.focus();
      own.select();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  // The panel closes on a press anywhere outside the field.
  useEffect(() => {
    if (!panel) {
      return;
    }
    const away = (event: MouseEvent) => {
      if (event.target instanceof Node && field.current?.contains(event.target) !== true) {
        setPanel(false);
      }
    };
    document.addEventListener('mousedown', away);
    return () => document.removeEventListener('mousedown', away);
  }, [panel]);

  const onInputKey = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown' || event.key === 'Enter') {
      event.preventDefault();
      onToTree();
      return;
    }
    if (event.key === 'Escape') {
      // Not the reader's `Escape`, nor anything else's.
      event.preventDefault();
      event.stopPropagation();
      if (settings.name !== '') {
        onChange({ ...settings, name: '' });
      } else {
        onKeysBack();
      }
    }
  };

  // `Escape` in the panel, or on ⏷ while it is open: the panel closed, the
  // focus on ⏷, and the key nobody else's — not the reader's `Escape`.
  const closeByEscape = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      setPanel(false);
      more.current?.focus();
    }
  };

  return (
    <div ref={field} role="search" style={FIELD}>
      <svg
        aria-hidden
        width={14}
        height={14}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={2.2}
        strokeLinecap="round"
        style={{ flex: '0 0 auto', color: COLOR.faint }}
      >
        <circle cx={11} cy={11} r={6.5} />
        <path d="m16 16 4.5 4.5" />
      </svg>
      {settings.recentOn && (
        <span data-testid="recent-chip" style={CHIP}>
          Recent {settings.recent}
          <button
            type="button"
            aria-label="Turn off Recent"
            title="Turn off Recent"
            onClick={() => onChange({ ...settings, recentOn: false })}
            style={CHIP_CLOSE}
          >
            ×
          </button>
        </span>
      )}
      <input
        ref={input}
        type="text"
        aria-label="Filter by name"
        placeholder="Filter by name"
        autoComplete="off"
        spellCheck={false}
        value={settings.name}
        onChange={(event) => onChange({ ...settings, name: event.currentTarget.value })}
        onKeyDown={onInputKey}
        style={INPUT}
      />
      {settings.name !== '' && (
        <button
          type="button"
          aria-label="Clear the name"
          title="Clear the name"
          onClick={() => {
            onChange({ ...settings, name: '' });
            input.current?.focus();
          }}
          style={CLEAR}
        >
          ×
        </button>
      )}
      <span aria-hidden style={DIVIDER} />
      <button
        ref={more}
        type="button"
        aria-label="More filters"
        title="More filters"
        aria-expanded={panel}
        onClick={() => setPanel((open) => !open)}
        // The focus stays here after the press that opened the panel, so
        // `Escape` closes it from here too, not only from inside it.
        onKeyDown={(event) => {
          if (panel) {
            closeByEscape(event);
          }
        }}
        style={MORE}
      >
        ⏷
      </button>
      {panel && (
        <div
          role="group"
          aria-label="More filters"
          onKeyDown={closeByEscape}
          style={PANEL}
        >
          {/* One condition a line, so that more can be added under it. */}
          <label style={CONDITION}>
            <input
              type="checkbox"
              checked={settings.recentOn}
              onChange={(event) => onChange({ ...settings, recentOn: event.currentTarget.checked })}
            />
            Recent
            <input
              type="number"
              aria-label="How many recent folders"
              min={1}
              step={1}
              inputMode="numeric"
              value={count}
              onChange={(event) => {
                const typed = event.currentTarget.value;
                setCount(typed);
                const read = recentCount(typed);
                if (read !== null && read !== settings.recent) {
                  onChange({ ...settings, recent: read });
                }
              }}
              onBlur={() => setCount(String(settings.recent))}
              style={COUNT}
            />
            folders
          </label>
          <p style={EXPLAINED}>{RECENT_EXPLAINED}</p>
        </div>
      )}
    </div>
  );
}

/** Whether the page reads `Cmd` rather than `Ctrl` as the key find goes with. */
const MAC = typeof navigator !== 'undefined' && /Mac|iPhone|iPad/.test(navigator.platform);

/** Whether `element` takes typing: a text input, a text area, or something editable. */
function typesText(element: Element | null): boolean {
  if (element instanceof HTMLTextAreaElement || element instanceof HTMLSelectElement) {
    return true;
  }
  if (element instanceof HTMLInputElement) {
    return !NOT_TYPED.has(element.type);
  }
  return element instanceof HTMLElement && element.isContentEditable;
}

/** The inputs that take no typing. */
const NOT_TYPED = new Set(['button', 'checkbox', 'color', 'file', 'hidden', 'image', 'radio', 'range', 'reset', 'submit']);

/**
 * The field: as high as the Library's box, shrinking with the header but no
 * narrower than leaves the input room to type in.
 */
const FIELD: CSSProperties = {
  position: 'relative',
  flex: '0 1 320px',
  minWidth: 150,
  boxSizing: 'border-box',
  height: 32,
  display: 'flex',
  alignItems: 'center',
  gap: 6,
  padding: '0 4px 0 10px',
  border: `1px solid ${COLOR.border}`,
  borderRadius: 8,
  background: COLOR.ground,
};

const INPUT: CSSProperties = {
  flex: '1 1 auto',
  minWidth: 64,
  height: '100%',
  border: 'none',
  outline: 'none',
  background: 'transparent',
  color: COLOR.text,
  font: 'inherit',
  fontSize: 13,
  padding: 0,
};

const CHIP: CSSProperties = {
  flex: '0 0 auto',
  display: 'flex',
  alignItems: 'center',
  gap: 2,
  padding: '1px 2px 1px 8px',
  borderRadius: 10,
  background: COLOR.accentTint,
  color: COLOR.accentDeep,
  fontSize: 12,
  whiteSpace: 'nowrap',
};

const SMALL_BUTTON: CSSProperties = {
  flex: '0 0 auto',
  border: 'none',
  background: 'transparent',
  font: 'inherit',
  lineHeight: 1,
  padding: '2px 4px',
  borderRadius: 4,
  cursor: 'pointer',
};

const CHIP_CLOSE: CSSProperties = { ...SMALL_BUTTON, color: COLOR.accentDeep, fontSize: 13 };

const CLEAR: CSSProperties = { ...SMALL_BUTTON, color: COLOR.dim, fontSize: 15 };

const DIVIDER: CSSProperties = {
  flex: '0 0 auto',
  width: 1,
  height: 18,
  background: COLOR.border,
};

const MORE: CSSProperties = { ...SMALL_BUTTON, color: COLOR.dim, fontSize: 12, padding: '4px 6px' };

/** Hung under the field from its right edge, over the tree. */
const PANEL: CSSProperties = {
  position: 'absolute',
  top: 'calc(100% + 4px)',
  right: 0,
  zIndex: 10,
  width: 280,
  maxWidth: 'calc(100vw - 32px)',
  boxSizing: 'border-box',
  padding: '10px 12px',
  border: `1px solid ${COLOR.border}`,
  borderRadius: 8,
  background: COLOR.panel,
  color: COLOR.text,
  fontSize: 13,
};

const CONDITION: CSSProperties = { display: 'flex', alignItems: 'center', gap: 6 };

const COUNT: CSSProperties = {
  width: 56,
  font: 'inherit',
  color: COLOR.text,
  background: COLOR.ground,
  border: `1px solid ${COLOR.border}`,
  borderRadius: 4,
  padding: '2px 4px',
};

const EXPLAINED: CSSProperties = { margin: '8px 0 0', color: COLOR.dim, fontSize: 12 };
