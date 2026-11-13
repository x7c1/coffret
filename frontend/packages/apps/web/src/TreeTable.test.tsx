import { cleanup, createEvent, fireEvent, render, screen, within } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Fill, ListedFile, ListedFolder, Listing } from '@coffret/api';

import type { Described } from './details';
import { size, time } from './humanize';
import { NOTHING_TO_DELETE } from './actions';
import { FORGET_FOLDER, PROSPECTIVE } from './prospective';
import { TRY_AGAIN } from './Region';
import { NOTHING_SELECTED, type Selection } from './selection';
import { ancestry, fileKey, folderKey, treeRows, type TreeState } from './tree';
import { COLOR } from './theme';
import { EMPTY_MAPPED_ROOT, FOLDER_SIZE_TITLE, ON_THIS_DEVICE, ROWS_BEYOND, TreeTable } from './TreeTable';

afterEach(cleanup);

function file(name: string, over: Partial<ListedFile> = {}, folder = 'albums'): ListedFile {
  return {
    name,
    path: folder === '' ? name : `${folder}/${name}`,
    size: 12,
    mtime: null,
    deposited_at: null,
    state: 'remote',
    container: 'one-file',
    reserved: false,
    openable: true,
    content_type: 'image/jpeg',
    ...over,
  };
}

function folder(name: string, over: Partial<ListedFolder> = {}, parent = 'albums'): ListedFolder {
  return {
    name,
    path: parent === '' ? name : `${parent}/${name}`,
    mapped: true,
    present_throughout: false,
    files_total: 1,
    files_present: 0,
    size: 0,
    mtime: null,
    ...over,
  };
}

function listing(over: Partial<Listing> = {}): Listing {
  return {
    path: 'albums',
    mapped: true,
    held: true,
    folders: [],
    files: [],
    folders_on_disk: [],
    empty_mapped_prefixes: [],
    ...over,
  };
}

interface Options {
  stranded?: boolean;
  strandedKnown?: boolean;
  prospective?: string[];
  prospectiveHere?: boolean;
  prepared?: string[];
  fill?: Fill | null;
  /** More listings the tree holds, by folder. */
  more?: Listing[];
  /** Folders open besides the ones down to `shown`. */
  open?: string[];
  /** The focused row; the folder `shown` lists where not given. */
  focus?: string | null;
  /** Folders whose listing was refused, with the refusal's sentence. */
  refused?: Record<string, string>;
  /** The path the reader's sheet shows, where it is open. */
  readerOn?: string | null;
  /** Whether `↑`/`↓` read on, as they do while the reader's sheet is open. */
  readingOn?: boolean;
  /** Whether `Escape` is the tree's; not while the reader's sheet is open. */
  escapeLetsGo?: boolean;
}

/**
 * The tree standing on `shown`: every folder above it open, each listing just
 * the next folder down, and the focus on `shown`'s own row — so that `shown`
 * is the folder the screen's actions are about.
 */
function mount(shown: Listing, options: Options = {}) {
  const held = new Map<string, Listing>();
  const above = ['', ...ancestry(shown.path)].slice(0, -1);
  for (const at of above) {
    const down = ancestry(shown.path)[ancestry(at).length];
    held.set(
      at,
      listing({ path: at, folders: [folder(down.slice(down.lastIndexOf('/') + 1), { mapped: shown.mapped }, at)] }),
    );
  }
  held.set(shown.path, shown);
  for (const more of options.more ?? []) {
    held.set(more.path, more);
  }
  const calls = {
    onChange: vi.fn(),
    onSelect: vi.fn(),
    onOpenFolder: vi.fn(),
    onOpenFile: vi.fn(),
    onUnsupported: vi.fn(),
    onAdd: vi.fn(),
    onUnmapped: vi.fn(),
    onMap: vi.fn(),
    onPack: vi.fn(),
    onDelete: vi.fn(),
    onNewFolder: vi.fn(),
    onForget: vi.fn(),
    onRename: vi.fn(),
    onMoveTo: vi.fn(),
    onMove: vi.fn(),
    onAskAgain: vi.fn(),
    onCloseReader: vi.fn(),
    onReadOn: vi.fn(),
  };
  const start: TreeState = {
    focus:
      options.focus !== undefined ? options.focus : shown.path === '' ? null : folderKey(shown.path),
    expanded: new Set([...ancestry(shown.path), ...(options.open ?? [])]),
  };
  function Harness() {
    const [state, setState] = useState(start);
    const [selection, setSelection] = useState<Selection>(NOTHING_SELECTED);
    const [moving, setMoving] = useState<readonly Described[] | null>(null);
    const rows = treeRows({
      listing: (path) => {
        const refusal = options.refused?.[path];
        if (refusal !== undefined) {
          return { status: 'failed', message: refusal };
        }
        const one = held.get(path);
        return one === undefined ? undefined : { status: 'ready', value: one };
      },
      expanded: state.expanded,
      prospective: options.prospective ?? [],
      stranded: [],
    });
    return (
      <TreeTable
        rows={rows}
        state={state}
        onChange={(next) => {
          calls.onChange(next);
          setState(next);
        }}
        selection={selection}
        onSelect={(next) => {
          calls.onSelect(next);
          setSelection(next);
        }}
        listingOf={(at) => held.get(at) ?? null}
        context={shown}
        prospectiveHere={options.prospectiveHere ?? false}
        prepared={options.prepared ?? []}
        fillOf={(at) => (options.fill?.folder === at ? options.fill : null)}
        freeze={null}
        stranded={options.stranded ?? false}
        strandedKnown={options.strandedKnown ?? true}
        onOpenFolder={calls.onOpenFolder}
        onOpenFile={calls.onOpenFile}
        readerOn={options.readerOn ?? null}
        onCloseReader={(next) => {
          calls.onCloseReader(next);
          setState(next);
        }}
        onUnsupported={calls.onUnsupported}
        onAdd={calls.onAdd}
        onCollecting={() => undefined}
        onUnreadable={() => undefined}
        onUnmapped={calls.onUnmapped}
        onMap={calls.onMap}
        onPack={calls.onPack}
        onDelete={calls.onDelete}
        onNewFolder={calls.onNewFolder}
        onForget={calls.onForget}
        onRename={calls.onRename}
        onMoveTo={calls.onMoveTo}
        moving={moving}
        onMoving={setMoving}
        onMove={calls.onMove}
        onAskAgain={calls.onAskAgain}
        measure={(text, measured) => text.length * (measured === 'heading' ? 6 : 8)}
        escapeLetsGo={options.escapeLetsGo ?? true}
        onReadOn={options.readingOn === true ? calls.onReadOn : undefined}
      />
    );
  }
  render(<Harness />);
  return { tree: screen.getByRole('tree'), ...calls };
}

/** The text of the whole screen. */
function said(): string {
  return document.body.textContent ?? '';
}

/** The labels of the items a row's "⋮" offers, opening it to read them. */
function menuOf(name: string): string[] {
  fireEvent.click(screen.getByLabelText(`actions for ${name}`));
  const items = screen.getAllByRole('menuitem').map((item) => item.textContent ?? '');
  fireEvent.keyDown(document, { key: 'Escape' });
  return items;
}

/** The names of the rows drawn as selected — tinted or focused — top to bottom. */
function selectedNames(): string[] {
  return [...document.querySelectorAll('tbody tr[aria-selected="true"]')].map((row) => row.getAttribute('title') ?? '');
}

/** The names of the rows on show, top to bottom. */
function rowNames(): string[] {
  return [...document.querySelectorAll('tbody tr[title]')].map((row) => row.getAttribute('title') ?? '');
}

const carryingFiles = { dataTransfer: { types: ['Files'] } };
const emptyDrop = { dataTransfer: { files: [], items: [] } };

describe('one tree', () => {
  it('draws folders and files as rows of one table, in the order Name, Size, on this device, Modified', () => {
    mount(listing({ folders: [folder('2026')], files: [file('cover.png')] }));
    expect(rowNames()).toEqual(['albums', '2026', 'cover.png']);
    const headings = [...document.querySelectorAll('thead th')].map((cell) => cell.textContent);
    expect(headings).toEqual(['Name', 'Size', 'On this device', 'Modified', '']);
    // No Items column.
    expect(said()).not.toContain('Items');
  });

  // The icon in front of each name says what the row is: a folder, a page
  // the reader can show, or a file it cannot (see `rowIconOf`); on the
  // focused row's accent it is drawn in white, as the name is.
  it('draws an icon in front of each name, white on the focused row', () => {
    mount(
      listing({
        folders: [folder('2026')],
        files: [file('cover.png'), file('notes.pdf', { openable: false, content_type: 'application/pdf' })],
      }),
      { focus: fileKey('albums/cover.png') },
    );
    const icon = (title: string) => screen.getByTitle(title).querySelector('svg[data-icon]') as SVGElement;
    expect(icon('2026').dataset.icon).toBe('folder');
    expect(icon('cover.png').dataset.icon).toBe('image');
    expect(icon('notes.pdf').dataset.icon).toBe('file');
    const ink = (title: string) => (icon(title).parentElement as HTMLElement).style.color;
    expect(ink('cover.png')).toBe(resolved(COLOR.onAccent));
    expect(ink('notes.pdf')).toBe(resolved(COLOR.accent));
    expect(ink('2026')).toBe(resolved(COLOR.folderStroke));
  });

  // A folder's icon is filled pale blue inside a deeper blue outline — selected
  // or not — and on the focused row's accent it is a white outline alone.
  it('fills a folder’s icon pale blue, and leaves it a white outline on the focused row', () => {
    mount(listing({ folders: [folder('2026'), folder('2027'), folder('2028')] }), {
      focus: folderKey('albums/2028'),
    });
    // 2028 stays selected beside the focus, which the click moves to 2026.
    fireEvent.click(screen.getByTitle('2026'), { metaKey: true });
    const icon = (title: string) => screen.getByTitle(title).querySelector('svg[data-icon]') as SVGElement;
    const ink = (title: string) => (icon(title).parentElement as HTMLElement).style.color;
    expect(icon('2026').getAttribute('fill')).toBe('none');
    expect(icon('2026').getAttribute('stroke')).toBe('currentColor');
    expect(ink('2026')).toBe(resolved(COLOR.onAccent));
    for (const unfocused of ['2027', '2028']) {
      expect(icon(unfocused).getAttribute('fill')).toBe(COLOR.folderFill);
      expect(icon(unfocused).getAttribute('stroke')).toBe('currentColor');
      expect(ink(unfocused)).toBe(resolved(COLOR.folderStroke));
    }
    expect(selectedNames()).toContain('2028');
  });

  it('indents a row by its depth, a file keeping the room of a disclosure button', () => {
    mount(listing({ folders: [folder('2026')], files: [file('cover.png')] }));
    expect(screen.getByLabelText('collapse albums')).toBeTruthy();
    expect(screen.getByLabelText('expand 2026')).toBeTruthy();
    const nameCell = (title: string) =>
      screen.getByTitle(title).querySelector('td') as HTMLTableCellElement;
    expect(nameCell('albums').style.paddingLeft).toBe('6px');
    expect(nameCell('2026').style.paddingLeft).toBe('20px');
    expect(nameCell('cover.png').style.paddingLeft).toBe('20px');
  });

  // Each value column is as wide as the widest of its heading and its values.
  it('draws each value column as wide as its widest string among the rows', () => {
    mount(listing({ files: [file('a.png', { size: 999_900_000 }), file('b.png', { size: 12 })] }));
    const widths = [...document.querySelectorAll('tbody')[0].closest('table')!.querySelectorAll('col')].map(
      (col) => (col as HTMLElement).style.width,
    );
    // `999.9 MB` is 8 characters at 8 a character, and 20 of padding; `—` is
    // narrower than `Modified`, which is 8 at 6.
    expect(widths).toEqual(['', '84px', '28px', '60px', '32px']);
  });

  // A folder's Size and Modified are the Library's about everything below it,
  // formatted as a file's are; a folder the Library does not hold yet has
  // nothing to measure, and its row already says so.
  it('fills a folder row with the size and newest time below it, and leaves a prospective one empty', () => {
    const modified = '2026-03-04T05:06:00Z';
    mount(listing({ folders: [folder('2026', { size: 999_900_000, mtime: modified })] }), {
      prospective: ['albums/new'],
    });
    const cells = (title: string) =>
      [...screen.getByTitle(title).querySelectorAll('td')].map((cell) => cell.textContent);
    expect(cells('2026').slice(1, 4)).toEqual([size(999_900_000), '', time(modified)]);
    expect(cells('new').slice(1, 4)).toEqual(['', '', '']);
    // The folder's values count toward the widths: `999.9 MB` is 8 characters
    // at 8 a character, and 20 of padding, and so is the time's 16.
    const widths = [...document.querySelector('table')!.querySelectorAll('col')].map(
      (col) => (col as HTMLElement).style.width,
    );
    expect(widths.slice(1, 4)).toEqual(['84px', '28px', '140px']);
  });

  // A folder's size is a total, drawn a step fainter than a file's own and said
  // so where the pointer rests on it.
  it("draws a folder's size fainter than a file's, with what it totals in its title", () => {
    mount(listing({ folders: [folder('2026', { size: 2_000 })], files: [file('cover.png', { size: 1_000 })] }));
    const sizeCell = (title: string) => screen.getByTitle(title).querySelectorAll('td')[1] as HTMLTableCellElement;
    expect(sizeCell('2026').style.color).toBe(resolved(COLOR.faint));
    expect(sizeCell('2026').title).toBe(FOLDER_SIZE_TITLE);
    expect(sizeCell('cover.png').style.color).toBe(resolved(COLOR.dim));
    expect(sizeCell('cover.png').title).toBe('');
  });
});

describe('whether a row is on this device', () => {
  it('draws nothing for a remote file, and a laptop for one on this device', () => {
    mount(listing({ files: [file('here.png', { state: 'present' }), file('away.png')] }));
    expect(within(screen.getByTitle('here.png')).getByLabelText(ON_THIS_DEVICE)).toBeTruthy();
    expect(within(screen.getByTitle('away.png')).queryByRole('img', { name: ON_THIS_DEVICE })).toBeNull();
  });

  it('draws a ring while a file is being fetched or added, and a warning where the fill stopped', () => {
    const fill = {
      folder: 'albums',
      status: 'filling',
      done: 0,
      total: 2,
      declined: [],
    } as unknown as Fill;
    mount(listing({ files: [file('a.png'), file('b.png', { state: 'added', container: null })] }), { fill });
    expect(within(screen.getByTitle('a.png')).getByLabelText('Being fetched to this device')).toBeTruthy();
    expect(within(screen.getByTitle('b.png')).getByRole('img').getAttribute('aria-label')).toContain(
      'being added to the Library',
    );
    cleanup();

    const stopped = {
      ...fill,
      status: 'stopped',
      stopped: { kind: 'storage_unavailable', message: 'Storage did not answer' },
    } as unknown as Fill;
    mount(listing({ files: [file('a.png')] }), { fill: stopped });
    expect(within(screen.getByTitle('a.png')).getByRole('img').getAttribute('aria-label')).toBe(
      'Fetching failed — Storage did not answer',
    );
  });

  // The promise that the whole book reads offline, which the server answers.
  it('draws a laptop on a folder only where every file below it is here', () => {
    mount(
      listing({
        folders: [folder('whole', { present_throughout: true }), folder('part', { present_throughout: false })],
      }),
    );
    expect(within(screen.getByTitle('whole')).getByLabelText(ON_THIS_DEVICE)).toBeTruthy();
    expect(within(screen.getByTitle('part')).queryByLabelText(ON_THIS_DEVICE)).toBeNull();
  });
});

describe('the mouse on the tree', () => {
  it('focuses a row on a click and opens it on a double-click', () => {
    const { onChange, onOpenFile, onOpenFolder } = mount(
      listing({ folders: [folder('2026')], files: [file('cover.png')] }),
    );
    fireEvent.click(screen.getByTitle('cover.png'));
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ focus: fileKey('albums/cover.png') }));
    expect(screen.getByTitle('cover.png').getAttribute('aria-selected')).toBe('true');
    expect(onOpenFile).not.toHaveBeenCalled();

    fireEvent.doubleClick(screen.getByTitle('cover.png'));
    expect(onOpenFile).toHaveBeenCalledWith('albums/cover.png');
    // A double-click on a folder opens it in the reader and in the tree, with
    // the focus on it, so the arrows go on into its files.
    fireEvent.doubleClick(screen.getByTitle('2026'));
    expect(onOpenFolder).toHaveBeenCalledWith('albums/2026');
    const last = onChange.mock.calls.at(-1)?.[0];
    expect(last.focus).toBe(folderKey('albums/2026'));
    expect([...last.expanded]).toContain('albums/2026');
  });

  // Opening is a toggle: an open folder closes, and the reader with it where
  // it shows the folder or anything under it; a reader elsewhere is left
  // alone. The file the reader shows closes it.
  it('closes an open folder on a double-click, and the reader only where it shows something in it', () => {
    const shown = listing({ folders: [folder('2026')], files: [file('cover.png')] });
    const more = [listing({ path: 'albums/2026', files: [file('a.jpg', {}, 'albums/2026')] })];
    for (const [readerOn, closes] of [
      ['albums/2026/a.jpg', true],
      ['albums/2026', true],
      ['albums/cover.png', false],
      ['albums/2026-other', false],
      [null, false],
    ] as const) {
      const { onChange, onCloseReader, onOpenFolder } = mount(shown, { more, open: ['albums/2026'], readerOn });
      const row = screen.getByTitle('2026');
      fireEvent.click(row);
      fireEvent.doubleClick(row);
      expect(onOpenFolder).not.toHaveBeenCalled();
      expect(rowNames()).toEqual(['albums', '2026', 'cover.png']);
      const next = (closes ? onCloseReader : onChange).mock.calls.at(-1)?.[0];
      expect(next.focus).toBe(folderKey('albums/2026'));
      expect([...next.expanded]).not.toContain('albums/2026');
      expect(onCloseReader).toHaveBeenCalledTimes(closes ? 1 : 0);
      cleanup();
    }
  });

  it('closes the reader on a double-click of the file it shows, and opens any other', () => {
    const shown = listing({ files: [file('a.png'), file('b.png')] });
    const { onOpenFile, onCloseReader } = mount(shown, { readerOn: 'albums/a.png' });
    fireEvent.click(screen.getByTitle('b.png'));
    fireEvent.doubleClick(screen.getByTitle('b.png'));
    expect(onOpenFile).toHaveBeenCalledWith('albums/b.png');
    expect(onCloseReader).not.toHaveBeenCalled();

    fireEvent.click(screen.getByTitle('a.png'));
    fireEvent.doubleClick(screen.getByTitle('a.png'));
    expect(onCloseReader).toHaveBeenCalledWith(expect.objectContaining({ focus: fileKey('albums/a.png') }));
    expect(onOpenFile).toHaveBeenCalledTimes(1);
  });

  it('opens and closes a folder from its disclosure button without moving the focus', () => {
    const { onChange } = mount(listing({ folders: [folder('2026')], files: [file('cover.png')] }), {
      more: [listing({ path: 'albums/2026', files: [file('a.jpg', {}, 'albums/2026')] })],
      focus: fileKey('albums/cover.png'),
    });
    fireEvent.click(screen.getByLabelText('expand 2026'));
    expect(rowNames()).toEqual(['albums', '2026', 'a.jpg', 'cover.png']);
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ focus: fileKey('albums/cover.png') }));
  });

  // The focus is never on a row that is not on show.
  it('moves the focus to a folder closed over it', () => {
    const { onChange } = mount(listing({ files: [file('cover.png')] }), { focus: fileKey('albums/cover.png') });
    fireEvent.click(screen.getByLabelText('collapse albums'));
    expect(rowNames()).toEqual(['albums']);
    expect(onChange).toHaveBeenLastCalledWith(expect.objectContaining({ focus: folderKey('albums') }));
    expect(screen.getByTitle('albums').getAttribute('aria-selected')).toBe('true');
  });

  it('opens a row on a touch tap', () => {
    const { onOpenFile } = mount(listing({ files: [file('cover.png')] }));
    const row = screen.getByTitle('cover.png');
    const down = createEvent.pointerDown(row);
    Object.defineProperty(down, 'pointerType', { value: 'touch' });
    fireEvent(row, down);
    fireEvent.click(row);
    expect(onOpenFile).toHaveBeenCalledWith('albums/cover.png');
    // The double-click a browser may make of two taps opens nothing more.
    fireEvent.doubleClick(row);
    expect(onOpenFile).toHaveBeenCalledTimes(1);
  });
});

describe('the keys on the tree', () => {
  it('walks the rows with the arrows and opens the focused one on Enter', () => {
    const { tree, onOpenFile, onOpenFolder } = mount(listing({ folders: [folder('2026')], files: [file('cover.png')] }), {
      more: [listing({ path: 'albums/2026', files: [file('a.jpg', {}, 'albums/2026')] })],
    });
    fireEvent.keyDown(tree, { key: 'ArrowDown' });
    expect(screen.getByTitle('2026').getAttribute('aria-selected')).toBe('true');
    fireEvent.keyDown(tree, { key: 'ArrowRight' });
    expect(rowNames()).toEqual(['albums', '2026', 'a.jpg', 'cover.png']);
    fireEvent.keyDown(tree, { key: 'ArrowRight' });
    expect(screen.getByTitle('a.jpg').getAttribute('aria-selected')).toBe('true');
    fireEvent.keyDown(tree, { key: 'Enter' });
    expect(onOpenFile).toHaveBeenCalledWith('albums/2026/a.jpg');
    fireEvent.keyDown(tree, { key: 'ArrowLeft' });
    fireEvent.keyDown(tree, { key: 'ArrowLeft' });
    expect(rowNames()).toEqual(['albums', '2026', 'cover.png']);
    fireEvent.keyDown(tree, { key: 'Enter' });
    expect(onOpenFolder).toHaveBeenCalledWith('albums/2026');
  });

  // `Enter` is the double-click's toggle.
  it('closes the focused open folder on Enter, and the reader on the file it shows', () => {
    const more = [listing({ path: 'albums/2026', files: [file('a.jpg', {}, 'albums/2026')] })];
    const shown = listing({ folders: [folder('2026')], files: [file('cover.png')] });
    const folderMount = mount(shown, { more, open: ['albums/2026'], focus: folderKey('albums/2026'), readerOn: 'albums/2026' });
    fireEvent.keyDown(folderMount.tree, { key: 'Enter' });
    expect(rowNames()).toEqual(['albums', '2026', 'cover.png']);
    expect(folderMount.onCloseReader).toHaveBeenCalledTimes(1);
    expect(folderMount.onOpenFolder).not.toHaveBeenCalled();
    // And opens it again, the reader on it.
    fireEvent.keyDown(folderMount.tree, { key: 'Enter' });
    expect(rowNames()).toEqual(['albums', '2026', 'a.jpg', 'cover.png']);
    expect(folderMount.onOpenFolder).toHaveBeenCalledWith('albums/2026');
    cleanup();

    const fileMount = mount(shown, { focus: fileKey('albums/cover.png'), readerOn: 'albums/cover.png' });
    fireEvent.keyDown(fileMount.tree, { key: 'Enter' });
    expect(fileMount.onCloseReader).toHaveBeenCalledTimes(1);
    expect(fileMount.onOpenFile).not.toHaveBeenCalled();
  });

  it('focuses the first row when the keyboard gives the tree the focus', () => {
    const { tree } = mount(listing({ path: '', files: [file('a.png', {}, ''), file('b.png', {}, '')] }));
    fireEvent.focus(tree);
    expect(screen.getByTitle('a.png').getAttribute('aria-selected')).toBe('true');
  });
});

// `Escape` folds one level of what is shown, and from a top-level row lets go
// of the focus.
describe('Escape on the tree', () => {
  /**
   * `albums/2026/08/a.jpg`, a file three levels deep, beside `albums/cover.png`
   * and `albums/2026/b.jpg`; `albums/2026` and `albums/2026/08` open.
   */
  function deep(options: Options = {}) {
    return mount(listing({ folders: [folder('2026')], files: [file('cover.png')] }), {
      more: [
        listing({ path: 'albums/2026', folders: [folder('08', {}, 'albums/2026')], files: [file('b.jpg', {}, 'albums/2026')] }),
        listing({ path: 'albums/2026/08', files: [file('a.jpg', {}, 'albums/2026/08')] }),
      ],
      open: ['albums/2026', 'albums/2026/08'],
      ...options,
    });
  }
  const focusedName = () => document.querySelector('tbody tr[aria-selected="true"]')?.getAttribute('title') ?? null;

  it('closes an open folder and keeps the focus on it, nested and top-level', () => {
    const { tree } = deep({ focus: folderKey('albums/2026') });
    fireEvent.keyDown(tree, { key: 'Escape' });
    expect(rowNames()).toEqual(['albums', '2026', 'cover.png']);
    expect(focusedName()).toBe('2026');
    cleanup();

    const top = deep({ focus: folderKey('albums') });
    fireEvent.keyDown(top.tree, { key: 'Escape' });
    expect(rowNames()).toEqual(['albums']);
    expect(focusedName()).toBe('albums');
  });

  it('closes the folder a file stands in, moves the focus to it, and drops the selection', () => {
    const { tree, onSelect } = deep({ focus: fileKey('albums/2026/08/a.jpg') });
    fireEvent.keyDown(tree, { key: 'ArrowDown', shiftKey: true });
    expect(selectedNames()).toEqual(['a.jpg', 'b.jpg']);
    // The focus is on `b.jpg`, the row the range grew to, in `2026`.
    fireEvent.keyDown(tree, { key: 'Escape' });
    expect(rowNames()).toEqual(['albums', '2026', 'cover.png']);
    expect(selectedNames()).toEqual(['2026']);
    expect(onSelect).toHaveBeenLastCalledWith(NOTHING_SELECTED);
  });

  it('closes the folder a closed folder stands in, moves the focus to it, and drops the selection', () => {
    const { tree, onSelect } = deep({ focus: folderKey('albums/2026/08'), open: ['albums/2026'] });
    fireEvent.keyDown(tree, { key: 'ArrowDown', shiftKey: true });
    expect(selectedNames()).toEqual(['08', 'b.jpg']);
    fireEvent.keyDown(tree, { key: 'Escape' });
    expect(rowNames()).toEqual(['albums', '2026', 'cover.png']);
    expect(selectedNames()).toEqual(['2026']);
    expect(onSelect).toHaveBeenLastCalledWith(NOTHING_SELECTED);
  });

  it('lets go of the focus from a top-level file or closed folder', () => {
    const root = listing({ path: '', folders: [folder('books', {}, '')], files: [file('a.png', {}, '')] });
    for (const focus of [fileKey('a.png'), folderKey('books')]) {
      const { tree } = mount(root, { focus });
      fireEvent.keyDown(tree, { key: 'Escape' });
      expect(focusedName()).toBeNull();
      expect(rowNames()).toEqual(['books', 'a.png']);
      cleanup();
    }
  });

  it('folds one level on each press from a file three levels deep, and lets go on the last', () => {
    const { tree } = deep({ focus: fileKey('albums/2026/08/a.jpg') });
    expect(rowNames()).toEqual(['albums', '2026', '08', 'a.jpg', 'b.jpg', 'cover.png']);
    fireEvent.keyDown(tree, { key: 'Escape' });
    expect(rowNames()).toEqual(['albums', '2026', '08', 'b.jpg', 'cover.png']);
    expect(focusedName()).toBe('08');
    fireEvent.keyDown(tree, { key: 'Escape' });
    expect(rowNames()).toEqual(['albums', '2026', 'cover.png']);
    expect(focusedName()).toBe('2026');
    fireEvent.keyDown(tree, { key: 'Escape' });
    expect(rowNames()).toEqual(['albums']);
    expect(focusedName()).toBe('albums');
    fireEvent.keyDown(tree, { key: 'Escape' });
    expect(rowNames()).toEqual(['albums']);
    expect(focusedName()).toBeNull();
  });

  it('opens a folder it closed with every subfolder closed', () => {
    const { tree } = deep({ focus: folderKey('albums/2026') });
    fireEvent.keyDown(tree, { key: 'Escape' });
    fireEvent.keyDown(tree, { key: 'ArrowRight' });
    expect(rowNames()).toEqual(['albums', '2026', '08', 'b.jpg', 'cover.png']);
  });

  it('opens a folder closed by ← or by a click with its subfolders open again', () => {
    const byArrow = deep({ focus: folderKey('albums/2026') });
    fireEvent.keyDown(byArrow.tree, { key: 'ArrowLeft' });
    expect(rowNames()).toEqual(['albums', '2026', 'cover.png']);
    fireEvent.keyDown(byArrow.tree, { key: 'ArrowRight' });
    expect(rowNames()).toEqual(['albums', '2026', '08', 'a.jpg', 'b.jpg', 'cover.png']);
    cleanup();

    deep({ focus: folderKey('albums/2026') });
    fireEvent.click(screen.getByLabelText('collapse 2026'));
    expect(rowNames()).toEqual(['albums', '2026', 'cover.png']);
    fireEvent.click(screen.getByLabelText('expand 2026'));
    expect(rowNames()).toEqual(['albums', '2026', '08', 'a.jpg', 'b.jpg', 'cover.png']);
  });

  it('leaves the open folders and the focus alone while the reader is open', () => {
    const { tree, onChange, onSelect } = deep({ focus: fileKey('albums/2026/08/a.jpg'), readingOn: true, escapeLetsGo: false });
    fireEvent.keyDown(tree, { key: 'Escape' });
    expect(rowNames()).toEqual(['albums', '2026', '08', 'a.jpg', 'b.jpg', 'cover.png']);
    expect(focusedName()).toBe('a.jpg');
    expect(onChange).not.toHaveBeenCalled();
    expect(onSelect).not.toHaveBeenCalled();
  });
});

// A click on something that cannot hold the focus leaves it on the page's
// body; the keys pressed then are the tree's, and give it the focus back.
describe('the keys pressed with nothing holding the focus', () => {
  const three = () => listing({ files: [file('a.png'), file('b.png'), file('c.png')] });
  /** An element of the page outside the tree, removed again after the test. */
  const outside = (tag: string, attributes: Record<string, string> = {}): HTMLElement => {
    const element = document.createElement(tag);
    element.setAttribute('data-outside', '');
    for (const [name, value] of Object.entries(attributes)) {
      element.setAttribute(name, value);
    }
    document.body.appendChild(element);
    return element;
  };
  afterEach(() => {
    for (const element of document.body.querySelectorAll(':scope > [data-outside]')) {
      element.remove();
    }
  });

  it('walks the rows from the body, and hands the tree the focus', () => {
    const { tree } = mount(three(), { focus: fileKey('albums/a.png') });
    expect(document.activeElement).toBe(document.body);
    fireEvent.keyDown(document.body, { key: 'ArrowDown' });
    expect(selectedNames()).toEqual(['b.png']);
    expect(document.activeElement).toBe(tree);
    // Handed back, not started over at the first row.
    fireEvent.keyDown(tree, { key: 'ArrowDown' });
    expect(selectedNames()).toEqual(['c.png']);
  });

  it('starts from the first row from the body when nothing is focused', () => {
    const { tree } = mount(listing({ path: '', files: [file('a.png', {}, ''), file('b.png', {}, '')] }));
    fireEvent.keyDown(document.body, { key: 'ArrowDown' });
    expect(selectedNames()).toEqual(['a.png']);
    expect(document.activeElement).toBe(tree);
  });

  it('opens the focused row on Enter, and grows the range with Shift+↓, from the body', () => {
    const { onOpenFile } = mount(three(), { focus: fileKey('albums/a.png') });
    fireEvent.keyDown(document.body, { key: 'ArrowDown', shiftKey: true });
    expect(selectedNames()).toEqual(['a.png', 'b.png']);
    (document.activeElement as HTMLElement).blur();
    fireEvent.keyDown(document.body, { key: 'Enter' });
    expect(onOpenFile).toHaveBeenCalledWith('albums/b.png');
  });

  it('climbs on Escape from the body', () => {
    const { tree } = mount(three(), { focus: fileKey('albums/b.png') });
    fireEvent.keyDown(document.body, { key: 'Escape' });
    expect(screen.getByTitle('albums').getAttribute('aria-selected')).toBe('true');
    expect(document.activeElement).toBe(tree);
  });

  it('reads on from the body while the reader is open', () => {
    const { tree, onReadOn, onChange } = mount(three(), {
      focus: fileKey('albums/a.png'),
      readerOn: 'albums/a.png',
      readingOn: true,
      escapeLetsGo: false,
    });
    fireEvent.keyDown(document.body, { key: 'ArrowDown' });
    expect(onReadOn).toHaveBeenCalledWith('ArrowDown');
    expect(onChange).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(tree);
  });

  it('leaves Escape from the body to the reader while it is open', () => {
    const { tree, onChange, onSelect } = mount(three(), {
      focus: fileKey('albums/b.png'),
      readerOn: 'albums/b.png',
      readingOn: true,
      escapeLetsGo: false,
    });
    const escape = createEvent.keyDown(document.body, { key: 'Escape' });
    fireEvent(document.body, escape);
    expect(escape.defaultPrevented).toBe(false);
    expect(onChange).not.toHaveBeenCalled();
    expect(onSelect).not.toHaveBeenCalled();
    expect(document.activeElement).not.toBe(tree);
  });

  it('takes no key something else took first', () => {
    const { tree, onChange } = mount(three(), { focus: fileKey('albums/a.png') });
    const taking = (event: KeyboardEvent) => event.preventDefault();
    document.body.addEventListener('keydown', taking);
    try {
      fireEvent.keyDown(document.body, { key: 'ArrowDown' });
      fireEvent.keyDown(document.body, { key: 'Escape' });
    } finally {
      document.body.removeEventListener('keydown', taking);
    }
    expect(onChange).not.toHaveBeenCalled();
    expect(document.activeElement).not.toBe(tree);
  });

  it('leaves the arrows to a text field', () => {
    const { onChange } = mount(three(), { focus: fileKey('albums/a.png') });
    const field = outside('input', { type: 'text' });
    field.focus();
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    fireEvent.keyDown(field, { key: 'ArrowUp' });
    expect(onChange).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(field);
  });

  it('leaves Enter to a button outside the tree', () => {
    const { onOpenFile } = mount(three(), { focus: fileKey('albums/a.png') });
    const button = outside('button');
    button.focus();
    fireEvent.keyDown(button, { key: 'Enter' });
    expect(onOpenFile).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(button);
  });

  it('takes no key from the body while a dialog stands over the screen', () => {
    const { tree, onChange, onOpenFile } = mount(three(), { focus: fileKey('albums/a.png') });
    const dialog = outside('div', { role: 'dialog', 'aria-modal': 'true' });
    fireEvent.keyDown(document.body, { key: 'ArrowDown' });
    fireEvent.keyDown(document.body, { key: 'Enter' });
    fireEvent.keyDown(document.body, { key: 'Escape' });
    expect(onChange).not.toHaveBeenCalled();
    expect(onOpenFile).not.toHaveBeenCalled();
    expect(document.activeElement).not.toBe(tree);
    // And takes them again once it is gone.
    dialog.remove();
    fireEvent.keyDown(document.body, { key: 'ArrowDown' });
    expect(selectedNames()).toEqual(['b.png']);
  });

  // A press on a disabled item leaves the focus on the body with the menu
  // still open: its keys are the menu's, and `Escape` closes it alone.
  it('takes no key from the body while the actions menu is open', () => {
    const { onChange, onOpenFile } = mount(three(), { focus: fileKey('albums/a.png') });
    fireEvent.contextMenu(screen.getByTitle('b.png'));
    expect(screen.getByRole('menu')).toBeTruthy();
    // The right-click focuses its row; only what the keys do after it counts.
    onChange.mockClear();
    (document.activeElement as HTMLElement).blur();
    expect(document.activeElement).toBe(document.body);
    fireEvent.keyDown(document.body, { key: 'ArrowDown' });
    fireEvent.keyDown(document.body, { key: 'Enter' });
    expect(onChange).not.toHaveBeenCalled();
    expect(onOpenFile).not.toHaveBeenCalled();
    fireEvent.keyDown(document.body, { key: 'Escape' });
    expect(screen.queryByRole('menu')).toBeNull();
    // Closed, not climbed: the focus stays on the row the menu was for.
    expect(onChange).not.toHaveBeenCalled();
  });

  // The header's **+** menu, opened after the tree, hears `Escape` after it:
  // the tree must leave the key to it rather than climb first.
  it('takes no key from the body while a menu outside the tree is open', () => {
    const { tree, onChange, onOpenFile } = mount(three(), { focus: fileKey('albums/b.png') });
    const menu = outside('div', { role: 'menu' });
    fireEvent.keyDown(document.body, { key: 'ArrowDown' });
    fireEvent.keyDown(document.body, { key: 'Enter' });
    fireEvent.keyDown(document.body, { key: 'Escape' });
    expect(onChange).not.toHaveBeenCalled();
    expect(onOpenFile).not.toHaveBeenCalled();
    expect(document.activeElement).not.toBe(tree);
    menu.remove();
    fireEvent.keyDown(document.body, { key: 'ArrowDown' });
    expect(selectedNames()).toEqual(['c.png']);
  });

  it('leaves the browser its own keys with Alt, Ctrl or ⌘', () => {
    const { onChange } = mount(three(), { focus: fileKey('albums/a.png') });
    fireEvent.keyDown(document.body, { key: 'ArrowLeft', altKey: true });
    fireEvent.keyDown(document.body, { key: 'ArrowDown', ctrlKey: true });
    fireEvent.keyDown(document.body, { key: 'ArrowDown', metaKey: true });
    expect(onChange).not.toHaveBeenCalled();
  });
});

describe('the block over the rows', () => {
  // The banner names the top-level folder a folder on this device would be
  // chosen for — and never says the folder is "not on this device", which the
  // column says of a file that is not here.
  it('tells a folder of files from a folder of folders, and names the top-level folder to choose for', () => {
    mount(listing({ mapped: false, files: [file('cover.png')] }));
    expect(said()).toContain('No folder chosen for “albums” yet: choose one to fetch its files');
    expect(said()).not.toContain('not on this device');
    expect(said()).not.toMatch(/\bmap/);
    cleanup();

    mount(listing({ path: 'books/vol-1', mapped: false, folders: [folder('scans', { mapped: false }, 'books/vol-1')] }));
    expect(said()).toContain('No folder chosen for “books” yet');
    expect(said()).toContain('to fetch what is in the folders below it');
  });

  it('offers to choose a folder from the banner, and one for the Library over an unmapped root', () => {
    const { onMap } = mount(listing({ path: 'books/vol-1', mapped: false, files: [file('p.png', {}, 'books/vol-1')] }));
    fireEvent.click(screen.getByText('Choose a folder…'));
    expect(onMap).toHaveBeenCalledWith('books');
    cleanup();

    const root = mount(listing({ path: '', mapped: false, files: [file('loose.png', {}, '')] }));
    expect(said()).toContain('No folder chosen for the Library yet');
    fireEvent.click(screen.getByText('Choose a folder…'));
    expect(root.onMap).toHaveBeenCalledWith(null);
  });

  it('keeps the banner back at a root holding only folders', () => {
    mount(listing({ path: '', mapped: false, folders: [folder('albums', { mapped: true }, '')] }));
    expect(screen.queryByText('Choose a folder…')).toBeNull();
  });

  // A folder row with no folder chosen says so in those words.
  it('marks a folder row with no folder chosen in those words', () => {
    mount(listing({ path: '', folders: [folder('books', { mapped: false }, '')] }));
    expect(within(screen.getByTitle('books')).getByText('no folder chosen')).toBeTruthy();
  });

  // A folder of the Library holds something by definition, so an empty listing
  // of one is a path naming nothing.
  it('tells a folder that is not there from one that is empty', () => {
    mount(listing({ path: '', held: true }));
    expect(said()).toContain(EMPTY_MAPPED_ROOT);
    cleanup();
    mount(listing({ path: '', held: true, mapped: false }));
    expect(said()).not.toContain(EMPTY_MAPPED_ROOT);
    cleanup();
    mount(listing({ path: 'album', held: false }));
    expect(said()).toContain('the Library holds nothing at this path');
    cleanup();
    // Not before the stranded folders are known, nor over a stranded folder.
    mount(listing({ path: 'books/vol-2', held: false }), { strandedKnown: false });
    expect(said()).not.toContain('the Library holds nothing at this path');
    cleanup();
    mount(listing({ path: 'books/vol-2', held: false }), { stranded: true });
    expect(said()).toContain('this folder is empty');
  });

  // The Library root's "New folder…" is the header's **+** menu's; nothing over
  // the rows offers it, and a folder's own actions are in its row's menu.
  it('offers no "New folder…" or "Pack this folder…" over the rows', () => {
    mount(listing({ path: '' }));
    expect(screen.queryByRole('button', { name: 'New folder…' })).toBeNull();
    cleanup();
    mount(listing({ path: 'books/vol-1', files: [file('page.jpg', {}, 'books/vol-1')] }));
    expect(screen.queryByRole('button', { name: 'New folder…' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Pack this folder…' })).toBeNull();
  });
});

describe('the actions menu', () => {
  it('opens from a row’s "⋮" and from a right-click, and hands over what was chosen', () => {
    const { onDelete, onOpenFile } = mount(listing({ folders: [folder('2026')], files: [file('cover.png')] }));
    fireEvent.click(screen.getByLabelText('actions for cover.png'));
    expect(screen.getByRole('menu', { name: 'actions for cover.png' })).toBeTruthy();
    fireEvent.click(screen.getByRole('menuitem', { name: 'Delete…' }));
    expect(screen.queryByRole('menu')).toBeNull();

    fireEvent.contextMenu(screen.getByTitle('2026'));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Delete…' }));
    expect(onDelete.mock.calls).toEqual([
      [{ folders: [], paths: ['albums/cover.png'] }],
      [{ folders: ['albums/2026'], paths: [] }],
    ]);
    expect(onOpenFile).not.toHaveBeenCalled();
  });

  it('opens a row in the reader, and packs and prepares in a folder, from the menu', () => {
    const { onOpenFile, onPack, onNewFolder } = mount(listing({ folders: [folder('2026')], files: [file('cover.png')] }));
    fireEvent.contextMenu(screen.getByTitle('cover.png'));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Open' }));
    expect(onOpenFile).toHaveBeenCalledWith('albums/cover.png');
    fireEvent.contextMenu(screen.getByTitle('2026'));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Pack this folder…' }));
    expect(onPack).toHaveBeenCalledWith('albums/2026');
    fireEvent.contextMenu(screen.getByTitle('2026'));
    fireEvent.click(screen.getByRole('menuitem', { name: 'New folder…' }));
    expect(onNewFolder).toHaveBeenCalledWith('albums/2026');
  });

  it('closes on Escape and on a press outside it', () => {
    mount(listing({ files: [file('cover.png')] }));
    fireEvent.contextMenu(screen.getByTitle('cover.png'));
    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('menu')).toBeNull();
    fireEvent.contextMenu(screen.getByTitle('cover.png'));
    fireEvent.mouseDown(document.body);
    expect(screen.queryByRole('menu')).toBeNull();
  });

  it('walks its items with the arrows and chooses on Enter', () => {
    const { onOpenFile } = mount(listing({ files: [file('cover.png')] }));
    fireEvent.contextMenu(screen.getByTitle('cover.png'));
    const menu = screen.getByRole('menu');
    fireEvent.keyDown(menu, { key: 'ArrowDown' });
    fireEvent.keyDown(menu, { key: 'Enter' });
    expect(onOpenFile).toHaveBeenCalledWith('albums/cover.png');
  });

  it('closes from the "⋮" it was opened by, and shades the first item when opened from the keys', () => {
    const { onOpenFile } = mount(listing({ files: [file('cover.png')] }));
    const mark = screen.getByLabelText('actions for cover.png');
    fireEvent.click(mark, { detail: 1 });
    fireEvent.click(mark, { detail: 1 });
    expect(screen.queryByRole('menu')).toBeNull();
    // `Enter` on the "⋮" is a click with no pointer behind it.
    fireEvent.click(mark, { detail: 0 });
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'Enter' });
    expect(onOpenFile).toHaveBeenCalledWith('albums/cover.png');
  });

  // The right-click takes the place of the browser's own menu, so a row with
  // nothing to offer still answers it: one item, disabled, saying so.
  it('says there is nothing to delete on a row with nothing to offer', () => {
    const { onDelete } = mount(
      listing({ files: [file('new.pdf', { state: 'added', container: null, openable: false })] }),
    );
    expect(menuOf('new.pdf')).toEqual([NOTHING_TO_DELETE]);
    fireEvent.contextMenu(screen.getByTitle('new.pdf'));
    const item = screen.getByRole('menuitem', { name: NOTHING_TO_DELETE });
    expect((item as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(item);
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'ArrowDown' });
    fireEvent.keyDown(screen.getByRole('menu'), { key: 'Enter' });
    expect(screen.getByRole('menu')).toBeTruthy();
    expect(onDelete).not.toHaveBeenCalled();
  });

  it('marks a folder the Library does not hold yet, and offers to forget only a prepared one', () => {
    const { onForget } = mount(listing({ path: '' }), {
      prospective: ['comics', 'mapped'],
      prepared: ['comics'],
    });
    expect(screen.getByTitle('comics').textContent).toContain('not in Library');
    expect(screen.getAllByTitle(PROSPECTIVE)).toHaveLength(2);
    expect(menuOf('comics')).toEqual(['New folder…', FORGET_FOLDER]);
    expect(menuOf('mapped')).toEqual(['New folder…']);
    fireEvent.click(screen.getByLabelText('actions for comics'));
    fireEvent.click(screen.getByRole('menuitem', { name: FORGET_FOLDER }));
    expect(onForget).toHaveBeenCalledWith('comics');
  });
});

describe('selecting rows with modifier keys', () => {
  const four = () => listing({ files: [file('a.png'), file('b.png'), file('c.png'), file('d.png')] });

  it('selects a range with Shift+click and one row more with ⌘/Ctrl+click, and drops them on a plain click', () => {
    mount(four(), { focus: fileKey('albums/a.png') });
    fireEvent.click(screen.getByTitle('c.png'), { shiftKey: true });
    expect(selectedNames()).toEqual(['a.png', 'b.png', 'c.png']);
    // The focused row keeps its strong highlight; the others are tinted.
    expect(screen.getByTitle('c.png').style.background).not.toBe(screen.getByTitle('a.png').style.background);
    fireEvent.click(screen.getByTitle('b.png'), { metaKey: true });
    // Taken out, the focused row is drawn as not selected: the details pane
    // and the menu act on the rows selected, which it no longer is.
    expect(selectedNames()).toEqual(['a.png', 'c.png']);
    fireEvent.click(screen.getByTitle('d.png'), { ctrlKey: true });
    expect(selectedNames()).toEqual(['a.png', 'c.png', 'd.png']);
    fireEvent.click(screen.getByTitle('b.png'), { ctrlKey: true });
    expect(selectedNames()).toEqual(['a.png', 'b.png', 'c.png', 'd.png']);
    fireEvent.click(screen.getByTitle('a.png'));
    expect(selectedNames()).toEqual(['a.png']);
  });

  it('grows the range with Shift+↓ and drops it on a plain arrow', () => {
    const { tree } = mount(four(), { focus: fileKey('albums/a.png') });
    fireEvent.keyDown(tree, { key: 'ArrowDown', shiftKey: true });
    fireEvent.keyDown(tree, { key: 'ArrowDown', shiftKey: true });
    expect(selectedNames()).toEqual(['a.png', 'b.png', 'c.png']);
    fireEvent.keyDown(tree, { key: 'ArrowDown' });
    expect(selectedNames()).toEqual(['d.png']);
  });

  it('acts on the whole selection from a row inside it, and on a row outside it alone', () => {
    const { onDelete } = mount(four(), { focus: fileKey('albums/a.png') });
    fireEvent.click(screen.getByTitle('c.png'), { shiftKey: true });
    fireEvent.contextMenu(screen.getByTitle('b.png'));
    expect(screen.getAllByRole('menuitem').map((item) => item.textContent)).toEqual(['Move to…', 'Delete 3 items…']);
    fireEvent.click(screen.getByRole('menuitem', { name: 'Delete 3 items…' }));
    expect(onDelete).toHaveBeenLastCalledWith({
      folders: [],
      paths: ['albums/a.png', 'albums/b.png', 'albums/c.png'],
    });
    expect(selectedNames()).toEqual(['a.png', 'b.png', 'c.png']);

    fireEvent.contextMenu(screen.getByTitle('d.png'));
    expect(selectedNames()).toEqual(['d.png']);
    fireEvent.click(screen.getByRole('menuitem', { name: 'Delete…' }));
    expect(onDelete).toHaveBeenLastCalledWith({ folders: [], paths: ['albums/d.png'] });
  });

  // Folders prepared and not in the Library yet have nothing in it to delete:
  // a right-click on a selection of them opens the menu all the same, with the
  // one disabled item saying so, rather than swallowing the click.
  it('answers a right-click on a selection with nothing to delete', () => {
    const { onDelete, onForget } = mount(listing({ path: '' }), {
      prospective: ['comics', 'manga'],
      prepared: ['comics', 'manga'],
      focus: folderKey('comics'),
    });
    fireEvent.click(screen.getByTitle('manga'), { shiftKey: true });
    expect(selectedNames()).toEqual(['comics', 'manga']);
    fireEvent.contextMenu(screen.getByTitle('manga'));
    expect(screen.getByRole('menu', { name: 'actions for 2 items' })).toBeTruthy();
    const items = screen.getAllByRole('menuitem');
    expect(items.map((item) => item.textContent)).toEqual([NOTHING_TO_DELETE]);
    expect((items[0] as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(items[0]);
    expect(onDelete).not.toHaveBeenCalled();
    expect(onForget).not.toHaveBeenCalled();
    expect(selectedNames()).toEqual(['comics', 'manga']);
  });

  it('draws no "Select" mode and no boxes', () => {
    mount(four());
    expect(screen.queryByText('Select')).toBeNull();
    expect(screen.queryAllByRole('checkbox')).toEqual([]);
  });
});

describe('an open folder whose listing was refused', () => {
  it('says the refusal in place of its rows, and asks for that folder again', () => {
    const { onAskAgain } = mount(listing({ folders: [folder('2026')] }), {
      open: ['albums/2026'],
      refused: { 'albums/2026': 'the server could not read that folder' },
    });
    expect(said()).toContain('the server could not read that folder');
    fireEvent.click(screen.getByText(TRY_AGAIN));
    expect(onAskAgain).toHaveBeenCalledWith('albums/2026');
  });
});

describe('a drop on the tree', () => {
  it('says where a drop lands while files are dragged over it, and nothing for text', () => {
    const { tree } = mount(listing({ files: [file('cover.png')] }));
    fireEvent.dragEnter(tree, carryingFiles);
    expect(screen.getByRole('status').textContent).toBe('Drop to add to albums');
    fireEvent.dragLeave(tree);
    expect(said()).not.toContain('Drop to add');
    fireEvent.dragEnter(tree, { dataTransfer: { types: ['text/plain'] } });
    expect(said()).not.toContain('Drop to add');
  });

  // The pill and the outline follow the row under the pointer: a folder row's
  // own folder, and a file row's folder.
  it('outlines the row the drop lands in, and names it in the pill', () => {
    const { tree } = mount(listing({ path: '', folders: [folder('albums', {}, '')], files: [file('loose.png', {}, '')] }), {
      more: [listing({ files: [file('cover.png')] })],
      open: ['albums'],
    });
    fireEvent.dragEnter(tree, carryingFiles);
    fireEvent.dragOver(screen.getByTitle('cover.png'), carryingFiles);
    expect(screen.getByRole('status').textContent).toBe('Drop to add to albums');
    expect(screen.getByTitle('albums').style.outline).toContain('solid');
    expect(screen.getByTitle('albums').style.background).toBe(resolved(COLOR.accentTint));
    expect(tree.style.outlineStyle).toBe('none');
    fireEvent.dragOver(screen.getByTitle('loose.png'), carryingFiles);
    expect(screen.getByRole('status').textContent).toBe('Drop to add to the Library');
    expect(screen.getByTitle('albums').style.outlineStyle).toBe('none');
    expect(tree.style.outline).toContain('solid');
  });

  // A folder no folder of this device is chosen for takes the drop — the
  // screen asks for one (spec: EP-9) — and only a path naming nothing refuses it.
  it('hands over a drop on a folder with no folder chosen, and refuses one on a path naming nothing', async () => {
    const { tree, onAdd, onUnmapped } = mount(listing({ path: '', mapped: false, folders: [folder('albums', {}, '')] }));
    fireEvent.drop(tree, emptyDrop);
    await vi.waitFor(() => expect(onAdd).toHaveBeenCalledTimes(1));
    expect(onAdd.mock.calls[0][0]).toBe('');
    expect(onUnmapped).not.toHaveBeenCalled();
    cleanup();

    const nowhere = mount(listing({ path: 'nowhere', mapped: false, held: false }));
    fireEvent.drop(nowhere.tree, emptyDrop);
    expect(nowhere.onUnmapped).toHaveBeenCalledWith('add', false);
  });

  it('hands a drop on a folder row over for that folder', async () => {
    const { tree, onAdd } = mount(listing({ folders: [folder('2026')] }));
    const files = [{ name: 'a.jpg', size: 1, lastModified: 0 } as File];
    fireEvent.dragEnter(tree, carryingFiles);
    fireEvent.dragOver(screen.getByTitle('2026'), carryingFiles);
    fireEvent.drop(tree, {
      dataTransfer: {
        types: ['Files'],
        files,
        items: files.map((one) => ({
          webkitGetAsEntry: () => ({
            isFile: true,
            isDirectory: false,
            name: one.name,
            file: (onFile: (file: File) => void) => onFile(one),
          }),
        })),
      },
    });
    await vi.waitFor(() => expect(onAdd).toHaveBeenCalledTimes(1));
    expect(onAdd.mock.calls[0][0]).toBe('albums/2026');
  });

  it('answers opening a file of an unmapped folder as an open, and does not open it', () => {
    const { onUnmapped, onOpenFile } = mount(listing({ mapped: false, files: [file('cover.png')] }));
    fireEvent.doubleClick(screen.getByTitle('cover.png'));
    expect(onUnmapped).toHaveBeenCalledWith('open', true);
    expect(onOpenFile).not.toHaveBeenCalled();
  });
});

describe('a drag of rows onto a folder', () => {
  /** What a drag of rows of the tree carries: no files, and room for what is set on it. */
  function rowsInTheAir() {
    const data = new Map<string, string>();
    return {
      dataTransfer: {
        types: ['application/x-coffret-rows'],
        effectAllowed: 'none',
        dropEffect: 'none',
        setData: (type: string, value: string) => data.set(type, value),
        getData: (type: string) => data.get(type) ?? '',
      },
    };
  }

  // albums/ holds 2026/ and cover.png; the root holds albums/ and books/.
  function mountAlbums() {
    return mount(
      listing({ path: '', folders: [folder('albums', {}, ''), folder('books', {}, '')] }),
      {
        more: [listing({ folders: [folder('2026')], files: [file('cover.png')] })],
        open: ['albums'],
      },
    );
  }

  it('says where the rows move to, outlines the folder, and hands them over on release', () => {
    const { tree, onMove } = mountAlbums();
    const air = rowsInTheAir();
    fireEvent.dragStart(screen.getByTitle('cover.png'), air);
    fireEvent.dragEnter(tree, air);
    fireEvent.dragOver(screen.getByTitle('books'), air);
    expect(screen.getByRole('status').textContent).toBe('Move to books');
    expect(screen.getByTitle('books').style.outline).toContain('solid');
    expect(air.dataTransfer.dropEffect).toBe('move');
    fireEvent.dragOver(screen.getByTitle('2026'), air);
    expect(screen.getByRole('status').textContent).toBe('Move to albums/2026');
    fireEvent.drop(screen.getByTitle('2026'), air);
    expect(onMove).toHaveBeenCalledTimes(1);
    expect(onMove.mock.calls[0][0].map((row: Described) => row.path)).toEqual(['albums/cover.png']);
    expect(onMove.mock.calls[0][1]).toBe('albums/2026');
    expect(said()).not.toContain('Move to');
  });

  it('lands over a file row in the folder that file stands in', () => {
    const { tree, onMove } = mountAlbums();
    const air = rowsInTheAir();
    fireEvent.dragStart(screen.getByTitle('books'), air);
    fireEvent.dragEnter(tree, air);
    fireEvent.dragOver(screen.getByTitle('cover.png'), air);
    expect(screen.getByRole('status').textContent).toBe('Move to albums');
    expect(screen.getByTitle('albums').style.outline).toContain('solid');
    expect(screen.getByTitle('cover.png').style.outlineStyle).toBe('none');
    expect(air.dataTransfer.dropEffect).toBe('move');
    fireEvent.drop(screen.getByTitle('cover.png'), air);
    expect(onMove.mock.calls[0][0].map((row: Described) => row.path)).toEqual(['books']);
    expect(onMove.mock.calls[0][1]).toBe('albums');
  });

  it('says nothing over the folder the rows are in, over a dragged row, or under a dragged folder', () => {
    const { tree, onMove } = mountAlbums();
    const air = rowsInTheAir();
    fireEvent.dragStart(screen.getByTitle('albums'), air);
    fireEvent.dragEnter(tree, air);
    for (const over of ['albums', '2026', 'cover.png']) {
      fireEvent.dragOver(screen.getByTitle(over), air);
      expect(screen.queryByRole('status')).toBeNull();
      expect(screen.getByTitle(over).style.outlineStyle).toBe('none');
      expect(air.dataTransfer.dropEffect).toBe('none');
    }
    fireEvent.drop(screen.getByTitle('cover.png'), air);
    expect(onMove).not.toHaveBeenCalled();
  });

  it('takes rows into a folder prepared with New folder…', () => {
    const { tree, onMove } = mount(
      listing({ path: '', folders: [folder('albums', {}, ''), folder('books', {}, '')] }),
      {
        more: [listing({ folders: [folder('2026')], files: [file('cover.png')] })],
        open: ['albums'],
        prospective: ['albums/new'],
      },
    );
    const air = rowsInTheAir();
    fireEvent.dragStart(screen.getByTitle('cover.png'), air);
    fireEvent.dragEnter(tree, air);
    fireEvent.dragOver(screen.getByTitle('new'), air);
    expect(screen.getByRole('status').textContent).toBe('Move to albums/new');
    fireEvent.drop(screen.getByTitle('new'), air);
    expect(onMove.mock.calls[0][1]).toBe('albums/new');
  });

  it('drags the selection when a row inside it is grabbed', () => {
    const { tree, onMove } = mountAlbums();
    fireEvent.click(screen.getByTitle('2026'));
    fireEvent.click(screen.getByTitle('cover.png'), { metaKey: true });
    const air = rowsInTheAir();
    fireEvent.dragStart(screen.getByTitle('cover.png'), air);
    fireEvent.dragEnter(tree, air);
    fireEvent.dragOver(screen.getByTitle('books'), air);
    fireEvent.drop(screen.getByTitle('books'), air);
    expect(onMove.mock.calls[0][0].map((row: Described) => row.path)).toEqual(['albums/2026', 'albums/cover.png']);
  });

  it('offers Rename… and Move to… in the row’s menu', () => {
    const { onRename, onMoveTo } = mountAlbums();
    fireEvent.click(screen.getByLabelText('actions for cover.png'));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Rename…' }));
    expect(onRename.mock.calls[0][0].path).toBe('albums/cover.png');
    fireEvent.click(screen.getByLabelText('actions for 2026'));
    fireEvent.click(screen.getByRole('menuitem', { name: 'Move to…' }));
    expect(onMoveTo.mock.calls[0][0].map((row: Described) => row.path)).toEqual(['albums/2026']);
  });
});

/** The colour the browser resolves `hex` to, as an inline style reads back. */
function resolved(hex: string): string {
  const probe = document.createElement('span');
  probe.style.color = hex;
  return probe.style.color;
}

describe('only the rows on show are drawn', () => {
  // jsdom lays nothing out, so the tree is given the height of ten rows and a
  // scroll position that holds what is set on it. The block over the rows
  // measures 0 there, and a row the 32px asked for.
  const ROW = 32;
  const AREA = 10 * ROW;
  let scrolled: WeakMap<Element, number>;
  beforeEach(() => {
    scrolled = new WeakMap();
    Object.defineProperty(HTMLElement.prototype, 'scrollTop', {
      configurable: true,
      get(this: HTMLElement) {
        return scrolled.get(this) ?? 0;
      },
      set(this: HTMLElement, to: number) {
        scrolled.set(this, Math.max(0, to));
      },
    });
    Object.defineProperty(HTMLElement.prototype, 'clientHeight', {
      configurable: true,
      get(this: HTMLElement) {
        return this.getAttribute('role') === 'tree' ? AREA : 0;
      },
    });
  });
  afterEach(() => {
    delete (HTMLElement.prototype as Partial<HTMLElement>).scrollTop;
    delete (HTMLElement.prototype as Partial<HTMLElement>).clientHeight;
  });

  /** The name of the file at `index` among `count` pages: zero-padded, so they list in order. */
  const page = (index: number) => `p${String(index).padStart(4, '0')}.png`;
  const pages = (count: number, over: (index: number) => Partial<ListedFile> = () => ({}), at = 'albums') =>
    Array.from({ length: count }, (_, index) => file(page(index), over(index), at));

  /** albums/ open with `count` pages under it: the albums row at 0, page n at n + 1. */
  function mountPages(count: number, options: Options = {}) {
    return mount(listing({ files: pages(count) }), options);
  }

  /** The heights of the spacers above and below the rows drawn, 0 where there is none. */
  function spacers(): { above: number; below: number } {
    const all = [...document.querySelectorAll('tbody tr')];
    const height = (row: Element | undefined) =>
      row !== undefined && row.getAttribute('aria-hidden') === 'true'
        ? parseFloat((row.querySelector('td') as HTMLElement).style.height)
        : 0;
    return { above: height(all[0]), below: height(all[all.length - 1]) };
  }

  function scrollTo(tree: HTMLElement, top: number) {
    tree.scrollTop = top;
    fireEvent.scroll(tree);
  }

  function focusedName(): string | null {
    return document.querySelector('tbody tr[data-focused]')?.getAttribute('title') ?? null;
  }

  it('draws a bounded number of rows of thousands, and spacers keeping the whole height', () => {
    mountPages(5000);
    const names = rowNames();
    // The ten on show and the rows beyond them below; none above the first.
    expect(names).toHaveLength(10 + ROWS_BEYOND);
    expect(names[0]).toBe('albums');
    expect(names[names.length - 1]).toBe(page(10 + ROWS_BEYOND - 2));
    expect(spacers()).toEqual({ above: 0, below: (5001 - names.length) * ROW });
  });

  it('draws the rows the list is scrolled to, and moves the spacers with them', () => {
    const { tree } = mountPages(5000);
    scrollTo(tree, 2000 * ROW);
    const names = rowNames();
    expect(names).toHaveLength(10 + 2 * ROWS_BEYOND);
    // Row 2000 is page 1999; the drawn rows start ROWS_BEYOND above it.
    expect(names[0]).toBe(page(1999 - ROWS_BEYOND));
    expect(names).toContain(page(1999));
    expect(names).toContain(page(2008));
    const { above, below } = spacers();
    expect(above).toBe((2000 - ROWS_BEYOND) * ROW);
    expect(above + names.length * ROW + below).toBe(5001 * ROW);
    // To the end, the last row drawn and no spacer below it.
    scrollTo(tree, 5001 * ROW - AREA);
    expect(rowNames()[rowNames().length - 1]).toBe(page(4999));
    expect(spacers().below).toBe(0);
  });

  it('reaches a row not drawn with ↓ from the last drawn row, and keeps it three rows from the bottom', () => {
    // Focused on row 39, then scrolled back to the top: row 39 is the last drawn.
    const { tree } = mountPages(5000, { focus: fileKey(`albums/${page(38)}`) });
    scrollTo(tree, 0);
    expect(rowNames()[rowNames().length - 1]).toBe(page(38));
    expect(screen.queryByTitle(page(39))).toBeNull();
    fireEvent.keyDown(tree, { key: 'ArrowDown' });
    expect(focusedName()).toBe(page(39));
    // Row 40 ends at 41 rows, and three more below it fill the area to its bottom.
    expect(tree.scrollTop).toBe((41 + 3) * ROW - AREA);
  });

  it('reaches a row not drawn with ↑ from the first drawn row, and keeps it three rows from the top', () => {
    const { tree } = mountPages(5000, { focus: fileKey(`albums/${page(1999 - ROWS_BEYOND)}`) });
    scrollTo(tree, 2000 * ROW);
    expect(rowNames()[0]).toBe(page(1999 - ROWS_BEYOND));
    expect(screen.queryByTitle(page(1998 - ROWS_BEYOND))).toBeNull();
    fireEvent.keyDown(tree, { key: 'ArrowUp' });
    expect(focusedName()).toBe(page(1998 - ROWS_BEYOND));
    expect(tree.scrollTop).toBe((1999 - ROWS_BEYOND - 3) * ROW);
  });

  it('walks a long list with ↓ alone, every focused row drawn and in view', () => {
    // Twice past the rows drawn at the start (the area's ten and ROWS_BEYOND
    // below), and no further: each press renders the tree again.
    const presses = 2 * (10 + ROWS_BEYOND);
    const { tree } = mountPages(500);
    for (let press = 0; press < presses; press += 1) {
      fireEvent.keyDown(tree, { key: 'ArrowDown' });
    }
    expect(focusedName()).toBe(page(presses - 1));
    expect(tree.scrollTop).toBe((presses + 1 + 3) * ROW - AREA);
  });

  it('keeps the focus in view as a folder opens and closes above it', () => {
    // albums/ holds big/ (closed, 2000 pages) and then six pages; the focus on
    // the fourth of those, row 5, well inside the area.
    const { tree } = mount(listing({ folders: [folder('big')], files: pages(6) }), {
      more: [listing({ path: 'albums/big', files: pages(2000, () => ({}), 'albums/big') })],
      focus: fileKey(`albums/${page(3)}`),
    });
    expect(tree.scrollTop).toBe(0);
    fireEvent.click(screen.getByLabelText('expand big'));
    // Row 5 is row 2005 now: drawn, and three rows from the bottom.
    expect(focusedName()).toBe(page(3));
    expect(tree.scrollTop).toBe((2006 + 3) * ROW - AREA);
    // Scrolled up to big/'s row, a few rows past its top, and closed there:
    // the focus is row 5 again, and the list follows it back.
    scrollTo(tree, 4 * ROW);
    fireEvent.click(screen.getByLabelText('collapse big'));
    expect(focusedName()).toBe(page(3));
    expect(tree.scrollTop).toBe((5 - 3) * ROW);
  });

  it('says how many rows there are in all, and which of them each drawn row is and at what level', () => {
    const { tree } = mountPages(5000);
    expect(screen.getByTitle('albums').closest('table')!.getAttribute('aria-rowcount')).toBe('5001');
    expect(screen.getByTitle('albums').getAttribute('aria-rowindex')).toBe('1');
    expect(screen.getByTitle('albums').getAttribute('aria-level')).toBe('1');
    scrollTo(tree, 3000 * ROW);
    expect(screen.getByTitle(page(2999)).getAttribute('aria-rowindex')).toBe('3001');
    expect(screen.getByTitle(page(2999)).getAttribute('aria-level')).toBe('2');
  });

  it('measures the value columns over every row, drawn or not', () => {
    // The one large file is the last row, far below what is drawn.
    const { tree } = mount(listing({ files: pages(3000, (index) => ({ size: index === 2999 ? 999_900_000 : 12 })) }));
    expect(screen.queryByTitle(page(2999))).toBeNull();
    const widths = () =>
      [...document.querySelector('tbody')!.closest('table')!.querySelectorAll('col')].map(
        (col) => (col as HTMLElement).style.width,
      );
    // `999.9 MB` is 8 characters at 8 a character, and 20 of padding.
    expect(widths()[1]).toBe('84px');
    scrollTo(tree, 1500 * ROW);
    expect(widths()[1]).toBe('84px');
  });

  it('draws loading and refused rows as rows like the others', () => {
    // Two hundred open folders, the last two with no listing yet and refused.
    const folders = Array.from({ length: 200 }, (_, index) => folder(`f${String(index).padStart(3, '0')}`));
    const { tree, onAskAgain } = mount(listing({ folders }), {
      open: folders.map((one) => one.path),
      more: folders.slice(0, 198).map((one) => listing({ path: one.path })),
      refused: { 'albums/f199': 'the listing was refused' },
    });
    expect(screen.queryByText('loading…')).toBeNull();
    // The albums row, the 200 folders and the two notes: the last two rows
    // and a folder between them.
    scrollTo(tree, 203 * ROW - AREA);
    expect(screen.getByText('loading…').closest('tr')!.getAttribute('aria-rowindex')).toBe('201');
    expect(screen.getByText('the listing was refused').closest('tr')!.getAttribute('aria-rowindex')).toBe('203');
    fireEvent.click(screen.getByText(TRY_AGAIN));
    expect(onAskAgain).toHaveBeenCalledWith('albums/f199');
  });

  it('takes a drag over rows drawn only once the list scrolled during it', () => {
    // A drag of rows over pages: the folders below are not drawn until the
    // list is scrolled to them, as the browser does while a drag rests near
    // its edge.
    const { tree, onMove } = mount(listing({ path: '', folders: [folder('albums', {}, ''), folder('zz', {}, '')] }), {
      more: [listing({ files: pages(3000) })],
      open: ['albums'],
      focus: null,
    });
    const data = new Map<string, string>();
    const air = {
      dataTransfer: {
        types: ['application/x-coffret-rows'],
        effectAllowed: 'none',
        dropEffect: 'none',
        setData: (type: string, value: string) => data.set(type, value),
        getData: (type: string) => data.get(type) ?? '',
      },
    };
    fireEvent.dragStart(screen.getByTitle(page(0)), air);
    fireEvent.dragEnter(tree, air);
    expect(screen.queryByTitle('zz')).toBeNull();
    scrollTo(tree, 3002 * ROW - AREA);
    fireEvent.dragOver(screen.getByTitle('zz'), air);
    expect(screen.getByRole('status').textContent).toBe('Move to zz');
    fireEvent.drop(screen.getByTitle('zz'), air);
    expect(onMove.mock.calls[0][0].map((row: Described) => row.path)).toEqual([`albums/${page(0)}`]);
    expect(onMove.mock.calls[0][1]).toBe('zz');
  });
});
