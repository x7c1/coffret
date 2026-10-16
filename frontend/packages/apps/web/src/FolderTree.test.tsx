import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { NO_FOLDER_HERE } from '@coffret/api';

import { TAKEN } from './dropTarget';
import { FileList } from './FileList';
import { FolderTree, type TreeDrops } from './FolderTree';

/** What a drop onto a row reaches out to, every part of it recorded. */
function dropsOf(mapped: (folder: string) => boolean | null = () => true) {
  return {
    mapped,
    freezing: false,
    onArrive: vi.fn(),
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
      current={current}
      onOpen={() => undefined}
      onDelete={() => undefined}
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
function mount(folders: string[], stranded: string[] = [], drops = dropsOf()) {
  Element.prototype.scrollIntoView = () => undefined;
  const onDelete = vi.fn();
  const onOpen = vi.fn();
  const { container } = render(
    <FolderTree
      folders={folders}
      stranded={stranded}
      current="albums"
      onOpen={onOpen}
      onDelete={onDelete}
      drops={drops}
    />,
  );
  return { container, onDelete, onOpen, drops };
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
      }}
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
    />,
  );
  fireEvent.click(screen.getByLabelText('actions for 2026'));
  fireEvent.click(screen.getByRole('menuitem', { name: 'Delete…' }));
  expect(fromList.mock.calls).toEqual(onDelete.mock.calls);
  cleanup();
  mount(['albums', 'albums/2026', 'books', 'made'], ['made']);

  expect(screen.getByLabelText('actions for books')).toBeTruthy();
  expect(screen.queryByLabelText('actions for albums')).toBeNull();
  expect(screen.queryByLabelText('actions for Library')).toBeNull();
  expect(screen.getByTitle('made')).toBeTruthy();
  expect(screen.queryByLabelText('actions for made')).toBeNull();
});

// A folder comes to be by a drop, which says where it lands: there is no
// control for making one on the screen alone.
it('offers no way to make a folder on the screen alone', () => {
  const { container } = mount(['albums', 'books']);
  expect(container.textContent).not.toContain('new folder');
  expect(screen.queryAllByRole('button').map((button) => button.textContent)).not.toContainEqual(
    expect.stringContaining('new folder'),
  );
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
    const { drops } = mount(['albums', 'albums/2026', 'books']);
    const row = dropTargetOf('2026');
    expect(row.textContent).not.toContain('drop to');

    fireEvent.dragEnter(row, carryingFiles);
    expect(row.textContent).toContain(TAKEN);
    expect((row as HTMLElement).style.outline).toContain('dashed');
    // The moment a drag arrives is the moment to learn which folders are mapped.
    expect(drops.onArrive).toHaveBeenCalled();
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
    const [folder, files] = drops.onAdd.mock.calls[0] as [string, { path: string }[]];
    expect(folder).toBe('albums/2026');
    expect(files.map((added) => added.path)).toEqual(['cover.png']);

    fireEvent.drop(dropTargetOf('Library'), carrying('notes.txt'));
    await waitFor(() => expect(drops.onAdd).toHaveBeenCalledTimes(2));
    expect(drops.onAdd.mock.calls[1][0]).toBe('');
  });

  // Not knowing yet is not refusing: the drop asks the folder's own listing.
  it('takes a drop where whether a mapping reaches the folder is not known yet', async () => {
    const { drops } = mount(['albums'], [], dropsOf(() => null));
    const row = dropTargetOf('albums');
    fireEvent.dragEnter(row, carryingFiles);
    expect(row.textContent).toContain(TAKEN);
    fireEvent.drop(row, carrying('cover.png'));
    await waitFor(() => expect(drops.onAdd).toHaveBeenCalledTimes(1));
    expect(drops.onUnmapped).not.toHaveBeenCalled();
  });
});
