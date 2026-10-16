import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, expect, it, vi } from 'vitest';

import { FileList } from './FileList';
import { FolderTree } from './FolderTree';

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
      pending={[]}
      current={current}
      onOpen={() => undefined}
      onNewFolder={() => undefined}
      onDelete={() => undefined}
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

/** The tree over `folders`, with what its book menus hand over recorded. */
function mount(folders: string[], pending: string[] = []) {
  Element.prototype.scrollIntoView = () => undefined;
  const onDelete = vi.fn();
  const onOpen = vi.fn();
  const { container } = render(
    <FolderTree
      folders={folders}
      pending={pending}
      current="albums"
      onOpen={onOpen}
      onNewFolder={() => undefined}
      onDelete={onDelete}
    />,
  );
  return { container, onDelete, onOpen };
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
// under it is not a book, and a folder made here has nothing in the Library.
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
      madeHere={false}
      madeHereKnown
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
