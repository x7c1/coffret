import { cleanup, createEvent, fireEvent, render, screen, within } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { Fill, ListedFile, ListedFolder, Listing } from '@coffret/api';

import { TAKEN } from './dropTarget';
import { FORGET_FOLDER, PROSPECTIVE } from './prospective';
import { TRY_AGAIN } from './Region';
import { ancestry, fileKey, folderKey, treeRows, type TreeState } from './tree';
import { EMPTY_MAPPED_ROOT, END_SELECT, ON_THIS_DEVICE, SELECT, TreeTable } from './TreeTable';

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
    onOpenFolder: vi.fn(),
    onOpenFile: vi.fn(),
    onUnsupported: vi.fn(),
    onAdd: vi.fn(),
    onUnmapped: vi.fn(),
    onUnmappedDrop: vi.fn(),
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
        context={shown}
        prospectiveHere={options.prospectiveHere ?? false}
        prepared={options.prepared ?? []}
        fillOf={(at) => (options.fill?.folder === at ? options.fill : null)}
        freeze={null}
        stranded={options.stranded ?? false}
        strandedKnown={options.strandedKnown ?? true}
        mappedAt={(at) => held.get(at)?.mapped ?? (at.startsWith(`${shown.path}/`) ? shown.mapped : null)}
        onOpenFolder={calls.onOpenFolder}
        onOpenFile={calls.onOpenFile}
        onUnsupported={calls.onUnsupported}
        onAdd={calls.onAdd}
        onCollecting={() => undefined}
        onUnreadable={() => undefined}
        onUnmapped={calls.onUnmapped}
        onUnmappedDrop={calls.onUnmappedDrop}
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
  fireEvent.click(mark);
  return items;
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

  it('indents a row by its depth, a file keeping the room of a disclosure button', () => {
    mount(listing({ folders: [folder('2026')], files: [file('cover.png')] }));
    expect(screen.getByLabelText('collapse albums')).toBeTruthy();
    expect(screen.getByLabelText('expand 2026')).toBeTruthy();
    const nameCell = (title: string) =>
      screen.getByTitle(title).querySelector('td') as HTMLTableCellElement;
    expect(nameCell('albums').style.paddingLeft).toBe('6px');
    expect(nameCell('2026').style.paddingLeft).toBe('22px');
    expect(nameCell('cover.png').style.paddingLeft).toBe('22px');
  });

  // Each value column is as wide as the widest of its heading and its values.
  it('draws each value column as wide as its widest string on show', () => {
    mount(listing({ files: [file('a.png', { size: 999_900_000 }), file('b.png', { size: 12 })] }));
    const widths = [...document.querySelectorAll('tbody')[0].closest('table')!.querySelectorAll('col')].map(
      (col) => (col as HTMLElement).style.width,
    );
    // `999.9 MB` is 8 characters at 8 a character, and 20 of padding; `—` is
    // narrower than `Modified`, which is 8 at 6.
    expect(widths).toEqual(['', '84px', '28px', '68px', '36px']);
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
    // A double-click on a folder opens it in the reader, and does not expand it.
    fireEvent.doubleClick(screen.getByTitle('2026'));
    expect(onOpenFolder).toHaveBeenCalledWith('albums/2026');
    expect(screen.getByLabelText('expand 2026')).toBeTruthy();
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
  // The banner names what mapping this folder would be for.
  it('tells a folder of files from a folder of folders, and names the top-level folder to map', () => {
    mount(listing({ mapped: false, files: [file('cover.png')] }));
    expect(said()).toContain('this folder is not on this device — map ');
    expect(said()).toContain('to fetch its files');
    cleanup();

    mount(listing({ path: 'books/vol-1', mapped: false, folders: [folder('scans', { mapped: false }, 'books/vol-1')] }));
    expect(said()).toContain('to fetch what is in the folders below it');
    expect(document.querySelector('code')?.textContent).toBe('books');
  });

  it('offers to map the folder from the banner, and the root over an unmapped root', () => {
    const { onMap } = mount(listing({ path: 'books/vol-1', mapped: false, files: [file('p.png', {}, 'books/vol-1')] }));
    fireEvent.click(screen.getByText('map books…'));
    expect(onMap).toHaveBeenCalledWith('books');
    cleanup();

    const root = mount(listing({ path: '', mapped: false, files: [file('loose.png', {}, '')] }));
    fireEvent.click(screen.getByText('map the Library root…'));
    expect(root.onMap).toHaveBeenCalledWith(null);
  });

  it('keeps the banner back at a root holding only folders', () => {
    mount(listing({ path: '', mapped: false, folders: [folder('albums', { mapped: true }, '')] }));
    expect(screen.queryByText('map the Library root…')).toBeNull();
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

  it('offers to pack a mapped folder that is not the root, and hands over which', () => {
    const { onPack } = mount(listing({ path: 'books/vol-1', files: [file('page.jpg', {}, 'books/vol-1')] }));
    fireEvent.click(screen.getByText('Pack this folder…'));
    expect(onPack).toHaveBeenCalledWith('books/vol-1');
    cleanup();
    mount(listing({ mapped: false, files: [file('cover.png')] }));
    expect(screen.queryByText('Pack this folder…')).toBeNull();
    cleanup();
    mount(listing({ path: '', files: [file('cover.png', {}, '')] }));
    expect(screen.queryByText('Pack this folder…')).toBeNull();
  });

  // "New folder…" over the rows is how a path is prepared under the Library
  // root, which has no row of its own.
  it('offers "New folder…" for the focused folder where a mapping reaches it', () => {
    const { onNewFolder } = mount(listing({ path: '' }));
    fireEvent.click(screen.getByText('New folder…'));
    expect(onNewFolder).toHaveBeenCalledWith('');
    cleanup();
    mount(listing({ path: '', mapped: false }));
    expect(screen.queryByText('New folder…')).toBeNull();
  });
});

describe('row menus', () => {
  it('offers to delete a file and a folder, and hands over which', () => {
    const { onDelete, onOpenFile } = mount(listing({ folders: [folder('2026')], files: [file('cover.png')] }));
    for (const name of ['cover.png', '2026']) {
      fireEvent.click(screen.getByLabelText(`actions for ${name}`));
      fireEvent.click(screen.getByRole('menuitem', { name: 'Delete…' }));
    }
    expect(onDelete.mock.calls).toEqual([
      [{ folder: null, paths: ['albums/cover.png'] }],
      [{ folder: 'albums/2026', paths: [] }],
    ]);
    expect(onOpenFile).not.toHaveBeenCalled();
  });

  it('offers no deletion of a file only added here', () => {
    mount(listing({ files: [file('new.png', { state: 'added', container: null })] }));
    expect(menuOf('new.png')).toEqual([]);
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

describe('picking files out', () => {
  it('does not draw the row focused before "Select" as picked', () => {
    mount(listing({ files: [file('a.png'), file('b.png')] }), { focus: fileKey('albums/a.png') });
    expect(screen.getByTitle('a.png').getAttribute('aria-selected')).toBe('true');
    fireEvent.click(screen.getByText(SELECT));
    expect(screen.getByTitle('a.png').getAttribute('aria-selected')).toBe('false');
    fireEvent.click(screen.getByLabelText('select a.png'));
    expect(screen.getByTitle('a.png').getAttribute('aria-selected')).toBe('true');
  });

  it('shows no box until "Select" is pressed, and deletes the files picked together', () => {
    const { onDelete } = mount(listing({ folders: [folder('2026')], files: [file('a.png'), file('b.png')] }));
    expect(screen.queryByLabelText('select a.png')).toBeNull();
    fireEvent.click(screen.getByText(SELECT));
    // No box on a folder row: a folder is deleted from its own menu.
    expect(screen.queryByLabelText('select 2026')).toBeNull();
    fireEvent.click(screen.getByLabelText('select b.png'));
    fireEvent.click(screen.getByTitle('a.png'));
    expect(said()).toContain('2 selected');
    fireEvent.click(within(document.querySelector('thead') as HTMLElement).getByText('Delete…'));
    expect(onDelete).toHaveBeenCalledWith({ folder: null, paths: ['albums/a.png', 'albums/b.png'] });
    expect(screen.queryByText(END_SELECT)).toBeNull();
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
  it('says what a drop will do while files are dragged over it, and nothing for text', () => {
    const { tree } = mount(listing({ files: [file('cover.png')] }));
    fireEvent.dragEnter(tree, carryingFiles);
    expect(tree.textContent).toContain(TAKEN);
    fireEvent.dragLeave(tree);
    expect(tree.textContent).not.toContain('drop to');
    fireEvent.dragEnter(tree, { dataTransfer: { types: ['text/plain'] } });
    expect(tree.textContent).not.toContain('drop to');
  });

  it('refuses a drop on a folder no mapping reaches, and says which gesture it was', () => {
    const { tree, onUnmapped } = mount(listing({ path: '', mapped: false, folders: [folder('albums', {}, '')] }));
    fireEvent.drop(tree, emptyDrop);
    expect(onUnmapped).toHaveBeenCalledWith('add', true, expect.objectContaining({ path: '' }));
    cleanup();

    const nowhere = mount(listing({ path: 'nowhere', mapped: false, held: false }));
    fireEvent.drop(nowhere.tree, emptyDrop);
    expect(nowhere.onUnmapped).toHaveBeenCalledWith('add', false, expect.anything());
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
    expect(onUnmapped).toHaveBeenCalledWith('open', true, expect.objectContaining({ path: 'albums' }));
    expect(onOpenFile).not.toHaveBeenCalled();
  });
});
