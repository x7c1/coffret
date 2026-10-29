// Which rows of the tree are selected together, the way a desktop file
// manager's list selects them: with modifier keys, and no mode to enter first.
//
// - A plain click, or a plain arrow move, focuses one row and drops whatever
//   else was selected.
// - Shift+click selects every row on show from the anchor to the row clicked.
// - ⌘+click (Ctrl+click off macOS) adds one row to the selection, or takes it
//   out.
// - Shift+↑/↓ moves the focus and grows (or shrinks) the range from the anchor
//   to it.
//
// The anchor is where a range starts: the row last clicked or walked to
// without Shift, which is what a desktop file manager extends a range from.
//
// The focused row is not part of the selection's own record: with nothing
// selected besides it, the selection is the focused row alone. The tree's
// tint, the details pane and the actions menu all read the rows selected from
// here (see [`selectedRows`](#selectedRows)), so what the tree shows selected
// is what the pane describes and what the menu acts on.
//
// A selection holds rows by key and the rows on show are what it is read
// against, so a row that left the tree — deleted, or under a folder closed —
// leaves the selection with it, without anything having to take it out.
//
// Kept free of DOM and of React so it is unit testable.

import type { Described } from './details';
import type { TreeRow } from './tree';

/** The rows selected besides the focus, and where a Shift range starts. */
export interface Selection {
  /**
   * The keys of the rows selected. Empty where nothing is selected beyond the
   * focused row, which then stands for the selection on its own.
   */
  keys: ReadonlySet<string>;
  /** Where a Shift range starts; `null` for the focused row. */
  anchor: string | null;
}

/** Nothing selected beyond the focused row. */
export const NOTHING_SELECTED: Selection = { keys: new Set(), anchor: null };

/** What a click on a row asks for, read off the keys held with it. */
export type Click = 'plain' | 'toggle' | 'range';

/**
 * The click a pointer event is: Shift for a range, ⌘ or Ctrl for one row in or
 * out, and otherwise a plain click. Both ⌘ and Ctrl, because which of the two
 * a desktop uses for it differs between systems — and on macOS a Ctrl+click is
 * a right-click, which never reaches here as a click.
 */
export function clickOf(event: { shiftKey: boolean; metaKey: boolean; ctrlKey: boolean }): Click {
  if (event.shiftKey) {
    return 'range';
  }
  return event.metaKey || event.ctrlKey ? 'toggle' : 'plain';
}

/** Where the focus is and what is selected, together. */
export interface Picked {
  focus: string | null;
  selection: Selection;
}

/** The keys of the rows the keys and the pointer can stand on, top to bottom. */
function walkable(rows: readonly TreeRow[]): string[] {
  return rows.filter((row) => row.kind !== 'note').map((row) => row.key);
}

/**
 * Every row on show from `from` to `to`, both included, top to bottom. Just
 * `to` where `from` is not on show any longer: a range has nowhere to start.
 */
function range(rows: readonly TreeRow[], from: string, to: string): Set<string> {
  const keys = walkable(rows);
  const start = keys.indexOf(from);
  const end = keys.indexOf(to);
  if (start === -1 || end === -1) {
    return new Set([to]);
  }
  return new Set(keys.slice(Math.min(start, end), Math.max(start, end) + 1));
}

/**
 * The keys selected as they stand: the selection's own, or the focused row
 * alone where it has none.
 */
function current(picked: Picked): Set<string> {
  if (picked.selection.keys.size > 0) {
    return new Set(picked.selection.keys);
  }
  return picked.focus === null ? new Set() : new Set([picked.focus]);
}

/** What a click on the row `key` does to the focus and the selection. */
export function clicked(rows: readonly TreeRow[], picked: Picked, key: string, how: Click): Picked {
  switch (how) {
    case 'plain':
      return { focus: key, selection: { keys: new Set(), anchor: key } };
    case 'toggle': {
      const keys = current(picked);
      if (keys.has(key)) {
        keys.delete(key);
      } else {
        keys.add(key);
      }
      return { focus: key, selection: { keys, anchor: key } };
    }
    case 'range': {
      const anchor = picked.selection.anchor ?? picked.focus ?? key;
      return { focus: key, selection: { keys: range(rows, anchor, key), anchor } };
    }
  }
}

/**
 * What a move of the focus by the keys does to the selection: a plain move
 * drops it and anchors at the row arrived at, and a move with Shift held
 * selects the range from the anchor to it.
 */
export function stepped(
  rows: readonly TreeRow[],
  before: Picked,
  focus: string | null,
  extend: boolean,
): Selection {
  if (focus === null) {
    return NOTHING_SELECTED;
  }
  if (!extend) {
    return { keys: new Set(), anchor: focus };
  }
  const anchor = before.selection.anchor ?? before.focus ?? focus;
  return { keys: range(rows, anchor, focus), anchor };
}

/** Whether the row `key` is drawn as selected: tinted, beside the focused row's strong highlight. */
export function isSelected(selection: Selection, key: string): boolean {
  return selection.keys.has(key);
}

/**
 * The rows selected, top to bottom and only those on show: the selection's own,
 * or the focused row alone where it has none.
 */
export function selectedRows(rows: readonly TreeRow[], picked: Picked): Described[] {
  const keys = current(picked);
  return rows.filter((row): row is Described => row.kind !== 'note' && keys.has(row.key));
}

/**
 * What a right-click on the row `key` acts on, and the focus and selection it
 * leaves.
 *
 * A row inside a selection of several acts for the whole selection, which is
 * left as it is. Any other row first becomes the only focus — the way a click
 * would make it — so that what the menu acts on is what the screen shows
 * selected.
 */
export function rightClicked(
  rows: readonly TreeRow[],
  picked: Picked,
  key: string,
): { picked: Picked; target: Described[] } {
  const selected = selectedRows(rows, picked);
  if (selected.length > 1 && selected.some((row) => row.key === key)) {
    return { picked, target: selected };
  }
  const alone = clicked(rows, picked, key, 'plain');
  return { picked: alone, target: selectedRows(rows, alone) };
}
