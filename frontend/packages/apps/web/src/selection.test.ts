import { describe, expect, it } from 'vitest';

import type { ListedFile, Listing } from '@coffret/api';

import {
  clicked,
  clickOf,
  NOTHING_SELECTED,
  rightClicked,
  selectedRows,
  stepped,
  type Picked,
} from './selection';
import type { FileRow, TreeRow } from './tree';

const ALBUM: Listing = {
  path: 'album',
  mapped: true,
  held: true,
  folders: [],
  files: [],
  folders_on_disk: [],
  empty_mapped_prefixes: [],
};

function row(name: string): FileRow {
  const file: ListedFile = {
    name,
    path: `album/${name}`,
    size: 10,
    mtime: null,
    deposited_at: null,
    state: 'remote',
    container: 'one-file',
    reserved: false,
    openable: true,
    content_type: 'image/jpeg',
  };
  return { kind: 'file', key: `file:${file.path}`, path: file.path, name, depth: 1, file, in: ALBUM };
}

/** Five pages, and a note between the second and the third that nothing stands on. */
const ROWS: TreeRow[] = [
  row('1.jpg'),
  row('2.jpg'),
  { kind: 'note', key: 'note:album/x', path: 'album/x', depth: 1, refused: null },
  row('3.jpg'),
  row('4.jpg'),
  row('5.jpg'),
];

const key = (name: string) => `file:album/${name}`;

/** The names of the rows selected, top to bottom. */
function names(picked: Picked): string[] {
  return selectedRows(ROWS, picked).map((one) => one.name);
}

/** The focus on `name`, with nothing else selected, anchored there. */
function on(name: string): Picked {
  return { focus: key(name), selection: { keys: new Set(), anchor: key(name) } };
}

describe('clickOf', () => {
  it('reads Shift as a range, ⌘ or Ctrl as one row in or out, and otherwise a plain click', () => {
    expect(clickOf({ shiftKey: true, metaKey: false, ctrlKey: false })).toBe('range');
    expect(clickOf({ shiftKey: false, metaKey: true, ctrlKey: false })).toBe('toggle');
    expect(clickOf({ shiftKey: false, metaKey: false, ctrlKey: true })).toBe('toggle');
    expect(clickOf({ shiftKey: false, metaKey: false, ctrlKey: false })).toBe('plain');
  });
});

describe('selecting with the pointer', () => {
  it('selects the range from the anchor with Shift+click, either way, skipping notes', () => {
    const down = clicked(ROWS, on('2.jpg'), key('4.jpg'), 'range');
    expect(down.focus).toBe(key('4.jpg'));
    expect(names(down)).toEqual(['2.jpg', '3.jpg', '4.jpg']);

    // The anchor stays where the range started: a second Shift+click above it
    // turns the range round rather than growing it from the last click.
    const up = clicked(ROWS, down, key('1.jpg'), 'range');
    expect(names(up)).toEqual(['1.jpg', '2.jpg']);
  });

  it('adds one row with ⌘/Ctrl+click and takes it out again, keeping the rest', () => {
    const two = clicked(ROWS, on('1.jpg'), key('3.jpg'), 'toggle');
    expect(names(two)).toEqual(['1.jpg', '3.jpg']);
    const three = clicked(ROWS, two, key('5.jpg'), 'toggle');
    expect(names(three)).toEqual(['1.jpg', '3.jpg', '5.jpg']);
    const back = clicked(ROWS, three, key('3.jpg'), 'toggle');
    expect(names(back)).toEqual(['1.jpg', '5.jpg']);

    // A ⌘/Ctrl+click anchors a range there.
    expect(names(clicked(ROWS, three, key('4.jpg'), 'toggle'))).toEqual(['1.jpg', '3.jpg', '4.jpg', '5.jpg']);
    expect(names(clicked(ROWS, clicked(ROWS, on('1.jpg'), key('4.jpg'), 'toggle'), key('5.jpg'), 'range'))).toEqual([
      '4.jpg',
      '5.jpg',
    ]);
  });

  it('drops the selection on a plain click, which focuses the row clicked alone', () => {
    const several = clicked(ROWS, on('1.jpg'), key('4.jpg'), 'range');
    const plain = clicked(ROWS, several, key('2.jpg'), 'plain');
    expect(plain.focus).toBe(key('2.jpg'));
    expect(plain.selection.keys.size).toBe(0);
    expect(names(plain)).toEqual(['2.jpg']);
  });
});

describe('selecting with the keys', () => {
  it('grows the range with Shift+↑/↓ from where the focus started, and shrinks it back', () => {
    const start = on('3.jpg');
    const once = { focus: key('4.jpg'), selection: stepped(ROWS, start, key('4.jpg'), true) };
    expect(names(once)).toEqual(['3.jpg', '4.jpg']);
    const twice = { focus: key('5.jpg'), selection: stepped(ROWS, once, key('5.jpg'), true) };
    expect(names(twice)).toEqual(['3.jpg', '4.jpg', '5.jpg']);
    const back = { focus: key('4.jpg'), selection: stepped(ROWS, twice, key('4.jpg'), true) };
    expect(names(back)).toEqual(['3.jpg', '4.jpg']);
    // Past the anchor, the range turns round it.
    const above = { focus: key('2.jpg'), selection: stepped(ROWS, on('3.jpg'), key('2.jpg'), true) };
    expect(names(above)).toEqual(['2.jpg', '3.jpg']);
  });

  it('starts a range from the focus where nothing anchored one', () => {
    const unanchored: Picked = { focus: key('1.jpg'), selection: NOTHING_SELECTED };
    expect(names({ focus: key('2.jpg'), selection: stepped(ROWS, unanchored, key('2.jpg'), true) })).toEqual([
      '1.jpg',
      '2.jpg',
    ]);
  });

  it('drops the selection on a plain arrow, and anchors at the row arrived at', () => {
    const several = clicked(ROWS, on('1.jpg'), key('4.jpg'), 'range');
    const plain = stepped(ROWS, several, key('5.jpg'), false);
    expect(plain).toEqual({ keys: new Set(), anchor: key('5.jpg') });
    expect(names({ focus: key('5.jpg'), selection: plain })).toEqual(['5.jpg']);
  });
});

describe('what the rows selected are', () => {
  it('is the focused row alone where nothing else is selected, and nothing with no focus', () => {
    expect(names(on('2.jpg'))).toEqual(['2.jpg']);
    expect(names({ focus: null, selection: NOTHING_SELECTED })).toEqual([]);
  });

  // A row deleted, or under a folder closed, is no longer on show and leaves
  // the selection with it.
  it('holds only the rows on show', () => {
    const several = clicked(ROWS, on('1.jpg'), key('3.jpg'), 'range');
    const shown = ROWS.filter((one) => one.key !== key('2.jpg'));
    expect(selectedRows(shown, several).map((one) => one.name)).toEqual(['1.jpg', '3.jpg']);
  });
});

describe('a right-click', () => {
  it('acts on the whole selection where the row is inside one of several, and leaves it as it is', () => {
    const several = clicked(ROWS, on('1.jpg'), key('3.jpg'), 'range');
    const inside = rightClicked(ROWS, several, key('2.jpg'));
    expect(inside.picked).toBe(several);
    expect(inside.target.map((one) => one.name)).toEqual(['1.jpg', '2.jpg', '3.jpg']);
  });

  it('makes a row outside the selection the only focus, and acts on it alone', () => {
    const several = clicked(ROWS, on('1.jpg'), key('3.jpg'), 'range');
    const outside = rightClicked(ROWS, several, key('5.jpg'));
    expect(outside.picked.focus).toBe(key('5.jpg'));
    expect(outside.picked.selection.keys.size).toBe(0);
    expect(outside.target.map((one) => one.name)).toEqual(['5.jpg']);
  });

  it('acts on the row alone where only the focus is selected', () => {
    const alone = rightClicked(ROWS, on('1.jpg'), key('4.jpg'));
    expect(alone.picked.focus).toBe(key('4.jpg'));
    expect(alone.target.map((one) => one.name)).toEqual(['4.jpg']);
  });
});
