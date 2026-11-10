import { useEffect, useId, useRef, useState, type CSSProperties, type KeyboardEvent } from 'react';

import { RECENT_EXPLAINED, recentCount, type FilterSettings } from './filter';
import { COLOR, LAYER, SHADOW } from './theme';

/**
 * The search field in the header, which filters the tree (see
 * [`askOf`](./filter)): inside it, a magnifier, a chip `Recent n ×` while the
 * most recent books are asked for, the name input, a × that empties the name
 * while there is one, a divider, and ⏷ (More filters), which opens a small
 * panel under the field holding the conditions besides the name — for now
 * Recent and its count, on the line `Recent [n] folders`, with a question
 * mark in a circle ("About Recent") at its right end. `Escape` or a press
 * outside closes the panel, and what was set in it stays set. The panel is
 * drawn in front of the reader (see [`LAYER`](./theme)).
 *
 * The question mark keeps what Recent does out of the way: the explainer is
 * shown under the line while the question mark is hovered or holds the focus,
 * and from a press on it — a tap, where there is no hover — until a second
 * press, a press elsewhere, or `Escape`. `Escape` while the explainer is shown
 * hides it and leaves the panel open; the next closes the panel. The question
 * mark is described by the explainer (`aria-describedby`), so a screen reader
 * reads it whether or not it is shown.
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
  onNameLeft = () => {},
  onToTree,
  onKeysBack,
}: {
  settings: FilterSettings;
  onChange: (next: FilterSettings) => void;
  /** The name input lost the focus: what was typed is kept now rather than after the pause. */
  onNameLeft?: () => void;
  /** `↓` or `Enter` in the input: the focus to the tree's first row. */
  onToTree: () => void;
  /** The input let go of by `Escape`: the keys go back to the tree. */
  onKeysBack: () => void;
}) {
  const field = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const more = useRef<HTMLButtonElement>(null);
  const help = useRef<HTMLButtonElement>(null);
  const helpId = useId();
  const [panel, setPanel] = useState(false);
  // What shows the explainer under the question mark: the pointer over it, the focus
  // in it, a press on it that pinned it — unless it was hidden by a second
  // press or `Escape` since, which the next hover, focus or press undoes.
  const [helpHovered, setHelpHovered] = useState(false);
  const [helpFocused, setHelpFocused] = useState(false);
  const [helpPinned, setHelpPinned] = useState(false);
  const [helpDismissed, setHelpDismissed] = useState(false);
  const explained = !helpDismissed && (helpPinned || helpHovered || helpFocused);
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

  // The panel closes on a press anywhere outside the field, and whatever showed
  // the explainer goes with it: a panel opened again starts without it.
  useEffect(() => {
    if (!panel) {
      setHelpHovered(false);
      setHelpFocused(false);
      setHelpPinned(false);
      setHelpDismissed(false);
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

  // An explainer pinned by a press is unpinned by a press anywhere but the question mark.
  useEffect(() => {
    if (!helpPinned) {
      return;
    }
    const away = (event: MouseEvent) => {
      if (event.target instanceof Node && help.current?.contains(event.target) !== true) {
        setHelpPinned(false);
      }
    };
    document.addEventListener('mousedown', away);
    return () => document.removeEventListener('mousedown', away);
  }, [helpPinned]);

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

  // `Escape` in the panel, or on ⏷ while it is open: the explainer hidden
  // where it is shown, the panel closed otherwise, with the focus on ⏷; either
  // way the key is nobody else's — not the reader's `Escape`.
  const closeByEscape = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key !== 'Escape') {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (explained) {
      setHelpPinned(false);
      setHelpDismissed(true);
      return;
    }
    setPanel(false);
    more.current?.focus();
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
        onBlur={onNameLeft}
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
          <div style={LINE}>
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
            <button
              ref={help}
              type="button"
              aria-label="About Recent"
              aria-describedby={helpId}
              onMouseEnter={() => {
                setHelpHovered(true);
                setHelpDismissed(false);
              }}
              onMouseLeave={() => setHelpHovered(false)}
              onFocus={() => {
                setHelpFocused(true);
                setHelpDismissed(false);
              }}
              onBlur={() => setHelpFocused(false)}
              onClick={() => {
                setHelpDismissed(helpPinned);
                setHelpPinned(!helpPinned);
              }}
              style={HELP}
            >
              <HelpIcon />
            </button>
            {/* Kept in the page while hidden, so that the question mark is described by it either way. */}
            <span id={helpId} role="tooltip" hidden={!explained} style={EXPLAINED}>
              {RECENT_EXPLAINED}
            </span>
          </div>
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
  zIndex: LAYER.filterPanel,
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

/**
 * The Recent line: the condition, then its question mark at the right end,
 * which the explainer hangs under.
 */
const LINE: CSSProperties = {
  position: 'relative',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: 6,
};

/**
 * The question mark's button: no frame, so the icon is the control, on a 24px
 * square hit area. It keeps the browser's own focus ring.
 */
const HELP: CSSProperties = {
  ...SMALL_BUTTON,
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: 24,
  height: 24,
  padding: 0,
  color: COLOR.dim,
};

/** A question mark in a circle, drawn in `currentColor` so it takes the button's colour in either scheme. */
function HelpIcon() {
  return (
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
      <circle cx={12} cy={12} r={10} />
      <path d="M9.1 9a3 3 0 0 1 5.8 1c0 2-3 3-3 3" />
      <path d="M12 17h.01" />
    </svg>
  );
}

/** Hung under the Recent line, over whatever the panel holds below it. */
const EXPLAINED: CSSProperties = {
  position: 'absolute',
  top: 'calc(100% + 6px)',
  left: 0,
  right: 0,
  zIndex: 1,
  padding: '8px 10px',
  border: `1px solid ${COLOR.border}`,
  borderRadius: 8,
  background: COLOR.panel,
  boxShadow: SHADOW.menu,
  color: COLOR.dim,
  fontSize: 12,
};
