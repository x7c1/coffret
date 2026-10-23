import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { NO_FOLDER_HERE } from '@coffret/api';

import { COLOR } from './theme';
import type { Drop } from './drop';
import { NOT_KNOWN_YET, TAKEN } from './dropTarget';
import { FORGET_FOLDER, PROSPECTIVE } from './prospective';
import { FileList } from './FileList';
import { FolderTree, type TreeDrops } from './FolderTree';

/** What a drop onto a row reaches out to, every part of it recorded. */
function dropsOf(mapped: (folder: string) => boolean | null = () => true) {
  return {
    mapped,
    freezing: false,
    onAdd: vi.fn(),
    onCollecting: vi.fn(),
    onUnreadable: vi.fn(),
    onUnmapped: vi.fn(),
  } satisfies TreeDrops;
}

// jsdom lays nothing out and has no scrollIntoView of its own, so each case
// that needs one puts a recorder in its place and this takes it away again.
const unscrolled = Element.prototype.scrollIntoView;
afterEach(() => {
  cleanup();
  Element.prototype.scrollIntoView = unscrolled;
});

function drawAt(current: string) {
  return (
    <FolderTree
      folders={['albums', 'albums/2026', 'albums/2026/08', 'books']}
      stranded={[]}
      prospective={[]}
      prepared={[]}
      current={current}
      onOpen={() => undefined}
      onDelete={() => undefined}
      onNewFolder={() => undefined}
      onForget={() => undefined}
      drops={dropsOf()}
    />
  );
}

/** The row a tree-name button stands in, which is what gets scrolled. */
function rowOf(name: string): Element | null {
  return screen.getByTitle(name).parentElement;
}

// A folder restored from a URL several components deep opens every branch above
// it, and the row that marks it has to be brought into view as well — otherwise
// it is below the fold with nothing on the screen to say where it is.
it('scrolls the current folder into view, and moves with it', () => {
  const scrolled: Element[] = [];
  const scroll = vi.fn(function (this: Element) {
    scrolled.push(this);
  });
  Element.prototype.scrollIntoView = scroll;

  const { rerender } = render(drawAt('albums/2026/08'));
  expect(scroll).toHaveBeenCalledWith({ block: 'nearest' });
  expect(scrolled).toEqual([rowOf('08')]);

  rerender(drawAt('books'));
  expect(scrolled).toEqual([rowOf('08'), rowOf('books')]);
});

/** The tree over `folders`, with what its book menus and its drops hand over recorded. */
function mount(
  folders: string[],
  stranded: string[] = [],
  drops = dropsOf(),
  prospective: string[] = [],
  prepared: string[] = [],
) {
  Element.prototype.scrollIntoView = () => undefined;
  const onDelete = vi.fn();
  const onOpen = vi.fn();
  const onNewFolder = vi.fn();
  const onForget = vi.fn();
  const { container } = render(
    <FolderTree
      folders={folders}
      stranded={stranded}
      prospective={prospective}
      prepared={prepared}
      current="albums"
      onOpen={onOpen}
      onDelete={onDelete}
      onNewFolder={onNewFolder}
      onForget={onForget}
      drops={drops}
    />,
  );
  return { container, onDelete, onOpen, onNewFolder, onForget, drops };
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

// The placeholder marks are gone: no mark on the root, and none on a folder
// without folders under it, whose name keeps the marker's room so that the
// names at one depth start at one place.
it('draws no mark on the root or on a folder without subfolders, and keeps their indent', () => {
  const { container } = mount(['albums', 'albums/2026', 'books']);
  expect(container.textContent).not.toMatch(/[◈·]/);
  expect(screen.getByLabelText('collapse albums').textContent).toBe('▾');

  const leading = (name: string) => screen.getByTitle(name).previousElementSibling as HTMLElement;
  // `albums` has a branch and `books` has none, both at the top level.
  expect(leading('albums').tagName).toBe('BUTTON');
  expect(leading('books').tagName).toBe('SPAN');
  expect(leading('books').style.width).toBe(leading('albums').style.width);
  expect(leading('books').style.marginLeft).toBe(leading('albums').style.marginLeft);
  expect(leading('Library').tagName).toBe('SPAN');
});

// A book — a folder of pages, with no folders under it — is deleted from its
// row's "⋮" by handing over the very target the file list's folder menu hands
// over, so the screen asks the same question of either. A folder with folders
// under it is not a book, and a stranded folder has nothing in the Library.
it('offers to delete a book from its row, as the file list does', () => {
  const { onDelete, onOpen } = mount(['albums', 'albums/2026', 'books', 'made'], ['made']);

  fireEvent.click(screen.getByLabelText('actions for 2026'));
  fireEvent.click(screen.getByRole('menuitem', { name: 'Delete…' }));
  expect(onDelete).toHaveBeenCalledWith({ folder: 'albums/2026', paths: [] });
  expect(onOpen).not.toHaveBeenCalled();
  cleanup();

  // The file list's folder row, over the same folder, hands over the same.
  const fromList = vi.fn();
  render(
    <FileList
      listing={{
        path: 'albums',
        mapped: true,
        held: true,
        folders: [{ name: '2026', path: 'albums/2026', mapped: true }],
        files: [],
        folders_on_disk: [],
        empty_mapped_prefixes: [],
      }}
      prospective={[]}
      prospectiveHere={false}
      prepared={[]}
      fill={null}
      freeze={null}
      stranded={false}
      strandedKnown
      selected={null}
      onOpenFolder={() => undefined}
      onOpenFile={() => undefined}
      onUnsupported={() => undefined}
      onAdd={() => undefined}
      onCollecting={() => undefined}
      onUnreadable={() => undefined}
      onUnmapped={() => undefined}
      onMap={() => undefined}
      onPack={() => undefined}
      onDelete={fromList}
      onNewFolder={() => undefined}
      onForget={() => undefined}
    />,
  );
  fireEvent.click(screen.getByLabelText('actions for 2026'));
  fireEvent.click(screen.getByRole('menuitem', { name: 'Delete…' }));
  expect(fromList.mock.calls).toEqual(onDelete.mock.calls);
  cleanup();
  // Where no mapping reaches, a row offers nothing but what the Library has.
  mount(['albums', 'albums/2026', 'books', 'made'], ['made'], dropsOf(() => false));

  expect(menuOf('books')).toEqual(['Delete…']);
  expect(screen.queryByLabelText('actions for albums')).toBeNull();
  expect(screen.queryByLabelText('actions for Library')).toBeNull();
  expect(screen.getByTitle('made')).toBeTruthy();
  expect(screen.queryByLabelText('actions for made')).toBeNull();
});

// "New folder…" prepares a path a drop is to land under, so it is offered where
// a mapping of this device reaches the folder — the root's row among them — and
// nowhere a drop would be refused or might be.
describe('"New folder…" on a row of the tree', () => {
  it('is offered on every row a mapping reaches, the root’s included', () => {
    const { onNewFolder } = mount(['albums', 'albums/2026', 'books']);
    expect(menuOf('Library')).toEqual(['New folder…']);
    expect(menuOf('albums')).toEqual(['New folder…']);
    expect(menuOf('books')).toEqual(['New folder…', 'Delete…']);

    fireEvent.click(screen.getByLabelText('actions for 2026'));
    fireEvent.click(screen.getByRole('menuitem', { name: 'New folder…' }));
    expect(onNewFolder).toHaveBeenCalledWith('albums/2026');
    fireEvent.click(screen.getByLabelText('actions for Library'));
    fireEvent.click(screen.getByRole('menuitem', { name: 'New folder…' }));
    expect(onNewFolder).toHaveBeenLastCalledWith('');
  });

  it('is not offered where no mapping reaches the folder, or where that is not known', () => {
    mount(['albums', 'books'], [], dropsOf((folder) => (folder === '' ? false : folder === 'albums')));
    expect(menuOf('Library')).toEqual([]);
    expect(menuOf('albums')).toEqual(['New folder…', 'Delete…']);
    expect(menuOf('books')).toEqual(['Delete…']);
    cleanup();

    mount(['albums'], [], dropsOf(() => null));
    expect(menuOf('Library')).toEqual([]);
    expect(menuOf('albums')).toEqual(['Delete…']);
  });
});

// A folder the Library does not hold yet is drawn with its mark, offers to
// prepare a path inside it, and has nothing to delete.
it('marks a folder the Library does not hold yet, and offers no deletion of it', () => {
  mount(['albums', 'comics', 'comics/Series'], [], dropsOf(), ['comics', 'comics/Series']);
  fireEvent.click(screen.getByLabelText('expand comics'));
  const marks = screen.getAllByTitle(PROSPECTIVE);
  expect(marks).toHaveLength(2);
  expect(marks[0].textContent).toBe('not in Library');
  expect(menuOf('comics')).toEqual(['New folder…']);
  expect(menuOf('Series')).toEqual(['New folder…']);
  // The Library's own folder is drawn as it was.
  expect(screen.getByTitle('albums').parentElement?.textContent).not.toContain('not in Library');
});

// A mistyped prepared path is let go of from its own row; an empty mapped
// prefix is the mapping's, and a folder the Library holds is the Library's.
it('offers to forget a prepared path, and nothing else', () => {
  const { onForget } = mount(
    ['albums', 'comics', 'comics/Seris'],
    [],
    dropsOf(),
    ['comics', 'comics/Seris'],
    ['comics/Seris'],
  );
  fireEvent.click(screen.getByLabelText('expand comics'));
  expect(menuOf('Seris')).toEqual(['New folder…', FORGET_FOLDER]);
  expect(menuOf('comics')).toEqual(['New folder…']);
  expect(menuOf('albums')).toEqual(['New folder…', 'Delete…']);

  fireEvent.click(screen.getByLabelText('actions for Seris'));
  fireEvent.click(screen.getByRole('menuitem', { name: FORGET_FOLDER }));
  expect(onForget).toHaveBeenCalledWith('comics/Seris');
});

// The folder of a book whose freeze has not committed is drawn, though the
// Library does not name it yet.
it('draws a stranded folder, dimmed', () => {
  mount(['albums', 'books', 'vol-1'], ['vol-1']);
  expect(screen.getByTitle('vol-1').style.color).not.toBe(screen.getByTitle('books').style.color);
});

/** The element a row's drag handlers are on, which holds its name. */
function dropTargetOf(name: string): Element {
  const target = screen.getByTitle(name).parentElement?.parentElement;
  if (target === null || target === undefined) {
    throw new Error(`no drop target holds ${name}`);
  }
  return target;
}

/** A drop carrying one file called `name`, as a browser hands it over. */
function carrying(name: string) {
  const file = { name, size: 10 } as File;
  return {
    dataTransfer: {
      types: ['Files'],
      files: [file],
      items: [
        {
          webkitGetAsEntry: () => ({
            isFile: true,
            isDirectory: false,
            name,
            file: (onFile: (file: File) => void) => onFile(file),
          }),
        },
      ],
    },
  };
}

const carryingFiles = { dataTransfer: { types: ['Files'] } };

describe('a drop onto a row of the tree', () => {
  // The same line and the same outline the list gives a drag over it.
  it('says what letting go will do while files are dragged over the row', () => {
    mount(['albums', 'albums/2026', 'books']);
    const row = dropTargetOf('2026');
    expect(row.textContent).not.toContain('drop to');

    fireEvent.dragEnter(row, carryingFiles);
    expect(row.textContent).toContain(TAKEN);
    expect((row as HTMLElement).style.outline).toContain('dashed');
    // Only the row under the pointer says it.
    expect(dropTargetOf('books').textContent).not.toContain('drop to');

    fireEvent.dragLeave(row);
    expect(row.textContent).not.toContain('drop to');
    expect((row as HTMLElement).style.outline).toBe('none');
  });

  it('says nothing while text rather than files is dragged over the row', () => {
    mount(['albums', 'books']);
    const row = dropTargetOf('books');
    fireEvent.dragEnter(row, { dataTransfer: { types: ['text/plain'] } });
    expect(row.textContent).not.toContain('drop to');
  });

  // EP-9: a folder no mapping reaches says so before the drop, and the drop is
  // answered as the list answers one — nothing read, nothing sent.
  it('refuses a drop onto a folder no mapping reaches, as the list does', () => {
    const { drops } = mount(
      ['albums', 'books'],
      [],
      dropsOf((folder) => folder.startsWith('albums')),
    );
    const row = dropTargetOf('books');
    fireEvent.dragEnter(row, carryingFiles);
    expect(row.textContent).toContain(`a drop here is not taken — ${NO_FOLDER_HERE}`);
    expect(dropTargetOf('albums').textContent).not.toContain('not taken');

    fireEvent.drop(row, carrying('cover.png'));
    expect(drops.onUnmapped).toHaveBeenCalledWith('books');
    expect(drops.onCollecting).not.toHaveBeenCalled();
    expect(drops.onAdd).not.toHaveBeenCalled();
    expect(row.textContent).not.toContain('not taken');
  });

  // The drop lands in the row's folder — the Library root's included — and the
  // screen confirms it before anything is sent.
  it('hands the files over for the row’s own folder', async () => {
    const { drops } = mount(['albums', 'albums/2026', 'books']);
    fireEvent.drop(dropTargetOf('2026'), carrying('cover.png'));
    expect(drops.onCollecting).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(drops.onAdd).toHaveBeenCalledTimes(1));
    const [folder, drop] = drops.onAdd.mock.calls[0] as [string, Drop];
    expect(folder).toBe('albums/2026');
    expect(drop.files.map((added) => added.path)).toEqual(['cover.png']);
    // A browser's walk leaves nothing out it could say.
    expect(drop.notTaken).toEqual([]);

    fireEvent.drop(dropTargetOf('Library'), carrying('notes.txt'));
    await waitFor(() => expect(drops.onAdd).toHaveBeenCalledTimes(2));
    expect(drops.onAdd.mock.calls[1][0]).toBe('');
  });

  // Not knowing yet is not refusing: the drop asks the folder's own listing.
  // Nor is it promising, so the row is drawn in neither colour and says the
  // answer is not in yet.
  it('takes a drop where whether a mapping reaches the folder is not known yet', async () => {
    const { drops } = mount(['albums'], [], dropsOf(() => null));
    const row = dropTargetOf('albums');
    fireEvent.dragEnter(row, carryingFiles);
    expect(row.textContent).toContain(NOT_KNOWN_YET);
    expect(row.textContent).not.toContain(TAKEN);
    expect((row as HTMLElement).style.outline).toBe(`2px dashed ${COLOR.dim}`);
    fireEvent.drop(row, carrying('cover.png'));
    await waitFor(() => expect(drops.onAdd).toHaveBeenCalledTimes(1));
    expect(drops.onUnmapped).not.toHaveBeenCalled();
  });
});
