import { cleanup, createEvent, fireEvent, render, screen, within } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Fill, ListedFile, ListedFolder, Listing } from '@coffret/api';

import { size, time } from './humanize';
import { FORGET_FOLDER, PROSPECTIVE } from './prospective';
import { TRY_AGAIN } from './Region';
import { NOTHING_SELECTED, type Selection } from './selection';
import { ancestry, fileKey, folderKey, treeRows, type TreeState } from './tree';
import { COLOR } from './theme';
import { EMPTY_MAPPED_ROOT, FOLDER_SIZE_TITLE, ON_THIS_DEVICE, TreeTable } from './TreeTable';

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
    onAskAgain: vi.fn(),
  };
  const start: TreeState = {
    focus:
      options.focus !== undefined ? options.focus : shown.path === '' ? null : folderKey(shown.path),
    expanded: new Set([...ancestry(shown.path), ...(options.open ?? [])]),
  };
  function Harness() {
    const [state, setState] = useState(start);
    const [selection, setSelection] = useState<Selection>(NOTHING_SELECTED);
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
        onAskAgain={calls.onAskAgain}
        measure={(text, measured) => text.length * (measured === 'heading' ? 6 : 8)}
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
  const mark = screen.queryByLabelText(`actions for ${name}`);
  if (mark === null) {
    return [];
  }
  fireEvent.click(mark);
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
    expect(ink('2026')).toBe(resolved(COLOR.dim));
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
  it('draws each value column as wide as its widest string on show', () => {
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

  it('focuses the first row when the keyboard gives the tree the focus', () => {
    const { tree } = mount(listing({ path: '', files: [file('a.png', {}, ''), file('b.png', {}, '')] }));
    fireEvent.focus(tree);
    expect(screen.getByTitle('a.png').getAttribute('aria-selected')).toBe('true');
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

  it('draws no "⋮" on a row with nothing to offer', () => {
    mount(listing({ files: [file('new.pdf', { state: 'added', container: null, openable: false })] }));
    expect(menuOf('new.pdf')).toEqual([]);
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
    expect(screen.getAllByRole('menuitem').map((item) => item.textContent)).toEqual(['Delete 3 items…']);
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
    expect(tree.style.outline).toBe('none');
    fireEvent.dragOver(screen.getByTitle('loose.png'), carryingFiles);
    expect(screen.getByRole('status').textContent).toBe('Drop to add to the Library');
    expect(screen.getByTitle('albums').style.outline).toBe('none');
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

/** The colour the browser resolves `hex` to, as an inline style reads back. */
function resolved(hex: string): string {
  const probe = document.createElement('span');
  probe.style.color = hex;
  return probe.style.color;
}
