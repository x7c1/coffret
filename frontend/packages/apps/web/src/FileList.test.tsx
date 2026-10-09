import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, expect, it, vi } from 'vitest';

import type { ListedFile, ListedFolder, Listing } from '@coffret/api';

import { TAKEN } from './dropTarget';
import { FileList } from './FileList';

function file(name: string, over: Partial<ListedFile> = {}): ListedFile {
  return {
    name,
    path: `albums/${name}`,
    size: 12,
    mtime: null,
    state: 'remote',
    container: 'one-file',
    openable: true,
    content_type: 'image/jpeg',
    ...over,
  };
}

function folder(name: string, mapped = false): ListedFolder {
  return { name, path: `albums/${name}`, mapped };
}

function listing(over: Partial<Listing> = {}): Listing {
  return {
    path: 'albums',
    mapped: true,
    held: true,
    folders: [],
    files: [],
    folders_on_disk: [],
    ...over,
  };
}

function draw(shown: Listing, madeHere = false, madeHereKnown = true): string {
  return renderToStaticMarkup(
    <FileList
      listing={shown}
      fill={null}
      freeze={null}
      madeHere={madeHere}
      madeHereKnown={madeHereKnown}
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
      onDelete={() => undefined}
    />,
  );
}

// The banner names what mapping this folder would be for, and the folder is what
// decides which: "to fetch its files" over a subtree of nothing but subfolders
// tells a person about something that is not on the screen.
it('tells a folder of files from a folder of folders', () => {
  const withFiles = draw(listing({ mapped: false, files: [file('cover.png')] }));
  expect(withFiles).toContain('this folder is not on this device — map ');
  expect(withFiles).toContain('to fetch its files');

  const onlyFolders = draw(listing({ mapped: false, folders: [folder('2026')] }));
  expect(onlyFolders).toContain('this folder is not on this device — map ');
  expect(onlyFolders).toContain('to fetch what is in the folders below it');
  expect(onlyFolders).not.toContain('to fetch its files');
});

// And a folder holding neither, which is a folder made in this browser inside an
// unmapped subtree: the drop it was made for is what the banner is there to warn
// about, and either fetching clause would point at rows that are not on the
// screen while the line under it says the folder is empty.
it('tells an empty one what to do before anything is put in it', () => {
  const made = draw(listing({ path: 'books/vol-1', mapped: false, held: false }), true);
  expect(made).toContain('this folder is not on this device — map ');
  expect(made).toContain('before putting anything in it');
  expect(made).not.toContain('to fetch');
});

// It is the top-level component that a mapping is keyed by, so that is what the
// banner tells a reader to map rather than the folder they are standing in.
it('names the top-level folder to map, whichever sentence it says', () => {
  for (const held of [
    listing({ path: 'books/vol-1', mapped: false, files: [file('page-001.png')] }),
    listing({ path: 'books/vol-1', mapped: false, folders: [folder('scans')] }),
  ]) {
    expect(draw(held)).toContain('<code>books</code>');
  }
});

// A folder of the Library holds something by definition, so an empty listing of
// one is a path naming nothing — a component mistyped into the address bar, or a
// link written before the Entries went. "this folder is empty" would have a
// person looking for files they never had.
it('tells a folder that is not there from one that is empty', () => {
  expect(draw(listing({ path: '', held: true }))).toContain('this Library is empty');
  expect(draw(listing({ path: 'album', held: false }))).toContain(
    'the Library holds nothing at this path',
  );

  // A folder made in this browser is empty and not yet the Library's, and it is
  // waiting for what it was made for rather than missing.
  const made = draw(listing({ path: 'books/vol-2', held: false }), true);
  expect(made).not.toContain('the Library holds nothing at this path');
  expect(made).toContain('the Library does not have it yet');
});

// A folder stranded by a book that never committed rejoins the folders made
// here only once the folder tree answers, which is a request of its own beside
// the listing's. Until it does, a `madeHere` of false is not an answer — and
// somebody who closed the tab while their book was packing and came back to
// that folder would be told the Library has nothing of theirs there. The
// sentence waits; what stands in the meantime is what stood before it. A tree
// whose request failed never answers, so it never says it — the screen is
// already showing that trouble, and this page cannot confirm what it asserts.
it('waits for the folders made here before saying the Library has nothing', () => {
  const waiting = draw(listing({ path: 'books/vol-2', held: false }), false, false);
  expect(waiting).not.toContain('the Library holds nothing at this path');
  expect(waiting).toContain('this folder is empty');

  // And says it once they are known and this folder is not among them.
  const answered = draw(listing({ path: 'books/vol-2', held: false }));
  expect(answered).toContain('the Library holds nothing at this path');
});

// Being told to map a folder that does not exist would send somebody to a
// terminal to give a place on this device to a part of the Library there is
// none of. Held back on what is on hand rather than on the final answer: the
// objection stands just as well while the folders made here are still out.
it('does not tell a reader to map a path the Library names nothing at', () => {
  for (const known of [true, false]) {
    expect(
      draw(listing({ path: 'album', mapped: false, held: false }), false, known),
    ).not.toContain('this folder is not on this device');
  }
});

// A row in a folder no mapping reaches answers a click and still does not
// invite one: the pointer says what the row will do, and this one does not open.
it('does not offer a row it will not open', () => {
  expect(draw(listing({ mapped: false, files: [file('cover.png')] }))).not.toContain(
    'row activatable',
  );
  expect(draw(listing({ files: [file('cover.png')] }))).toContain('row activatable');
});

/** The list drawn into the DOM, with what it hands its `onUnmapped` recorded. */
function mount(shown: Listing, madeHereKnown = true, madeHere = false) {
  const onUnmapped = vi.fn();
  const onOpenFile = vi.fn();
  const onMap = vi.fn();
  const onPack = vi.fn();
  const onDelete = vi.fn();
  const { container } = render(
    <FileList
      listing={shown}
      fill={null}
      freeze={null}
      madeHere={madeHere}
      madeHereKnown={madeHereKnown}
      selected={null}
      onOpenFolder={() => undefined}
      onOpenFile={onOpenFile}
      onUnsupported={() => undefined}
      onAdd={() => undefined}
      onCollecting={() => undefined}
      onUnreadable={() => undefined}
      onUnmapped={onUnmapped}
      onMap={onMap}
      onPack={onPack}
      onDelete={onDelete}
    />,
  );
  // The element the drag handlers are on, which is the list as a whole.
  const list = container.firstElementChild;
  if (list === null) {
    throw new Error('the list draws an element');
  }
  return { list, onUnmapped, onOpenFile, onMap, onPack, onDelete };
}

afterEach(cleanup);

// A row in a folder no mapping reaches is not offered, and a click on it is
// still answered: with which gesture it was and whether there is a folder here
// to talk about — which there is, since it has a row in it. What it does not do
// is open, since the fetch behind it would be declined.
it('answers a click on a row of an unmapped folder as an open, about a folder that is there', () => {
  const { onUnmapped, onOpenFile } = mount(
    listing({ mapped: false, files: [file('cover.png')] }),
  );

  fireEvent.click(screen.getByTitle('cover.png'));

  expect(onUnmapped).toHaveBeenCalledTimes(1);
  expect(onUnmapped).toHaveBeenCalledWith('open', true);
  expect(onOpenFile).not.toHaveBeenCalled();
});

// A folder dropped on the Library root of a device that maps one top-level
// folder and not the root: nowhere to put a single file, so it is refused and
// said to be — and the root is held, so what is said is about the mapping
// rather than about a path the Library holds nothing at.
it('answers a drop on an unmapped root as an add, about a folder that is there', () => {
  const { list, onUnmapped } = mount(
    listing({ path: '', mapped: false, held: true, folders: [folder('albums', true)] }),
  );

  fireEvent.drop(list, { dataTransfer: { files: [], items: [] } });

  expect(onUnmapped).toHaveBeenCalledTimes(1);
  expect(onUnmapped).toHaveBeenCalledWith('add', true);
});

// And the same drop on a path the Library holds nothing at is said to be about
// no folder at all — once the folders made in this browser are known, since
// until then a folder whose book is still being packed would look the same.
it('answers a drop on a path the Library holds nothing at as about no folder, once that is known', () => {
  const nowhere = listing({ path: 'nowhere', mapped: false, held: false });

  const known = mount(nowhere);
  fireEvent.drop(known.list, { dataTransfer: { files: [], items: [] } });
  expect(known.onUnmapped).toHaveBeenCalledWith('add', false);
  cleanup();

  const waiting = mount(nowhere, false);
  fireEvent.drop(waiting.list, { dataTransfer: { files: [], items: [] } });
  expect(waiting.onUnmapped).toHaveBeenCalledWith('add', true);
});

// The banner offers the mapping where it says the folder is not here, rather
// than sending a person to a terminal: one button, carrying the top-level
// folder a mapping of this one is for.
it('offers to map the folder from the banner rather than naming the command', () => {
  for (const shown of [
    listing({ mapped: false, files: [file('cover.png')] }),
    listing({ mapped: false, folders: [folder('2026')] }),
    listing({ path: 'books/vol-1', mapped: false, held: false }),
  ]) {
    const drawn = draw(shown, true);
    expect(drawn).not.toContain('coffret map');
    expect(drawn).toContain('<button');
  }
  expect(draw(listing({ mapped: false, files: [file('cover.png')] }))).toContain(
    'map this folder…',
  );
  expect(draw(listing({ path: 'books/vol-1', mapped: false, files: [file('p.png')] }))).toContain(
    'map books…',
  );

  const { onMap } = mount(listing({ path: 'books/vol-1', mapped: false, files: [file('p.png')] }));
  fireEvent.click(screen.getByText('map books…'));
  expect(onMap).toHaveBeenCalledWith('books');
});

// An unmapped Library root holding files of its own offers the root, which is
// mapped with no prefix at all.
it('offers to map the Library root over an unmapped root', () => {
  const { onMap } = mount(listing({ path: '', mapped: false, files: [file('loose.png')] }));
  fireEvent.click(screen.getByText('map the Library root…'));
  expect(onMap).toHaveBeenCalledWith(null);
});

// While files are dragged over the list it says what letting go will do — the
// same line over a folder made here as over one the Library has, since how a
// drop is added follows what was dropped. Letting go takes it away, and a drag
// that carries no files gets no such line.
const carryingFiles = { dataTransfer: { types: ['Files'] } };

it('says what a drop will do while it is dragged over the list', () => {
  const existing = mount(listing({ files: [file('cover.png')] }));
  expect(existing.list.textContent).not.toContain('drop to');
  fireEvent.dragEnter(existing.list, carryingFiles);
  expect(existing.list.textContent).toContain(TAKEN);
  fireEvent.dragLeave(existing.list);
  expect(existing.list.textContent).not.toContain('drop to');
  cleanup();

  // A folder made here says the same: making it decided nothing about how a
  // drop into it is added.
  const made = mount(listing({ path: 'books/vol-1', held: false }), true, true);
  fireEvent.dragEnter(made.list, carryingFiles);
  expect(made.list.textContent).toContain(TAKEN);
  fireEvent.drop(made.list, { dataTransfer: { files: [], items: [] } });
  expect(made.list.textContent).not.toContain('drop to');
  cleanup();

  const away = mount(listing({ mapped: false, files: [file('cover.png')] }));
  fireEvent.dragEnter(away.list, carryingFiles);
  expect(away.list.textContent).toContain('a drop here is not taken');
});

it('says nothing about a drop while text rather than files is dragged over the list', () => {
  const { list } = mount(listing({ files: [file('cover.png')] }));
  fireEvent.dragEnter(list, { dataTransfer: { types: ['text/plain'] } });
  expect(list.textContent).not.toContain('drop to');
});

// "Pack this folder…" is the folder's own action, and only where a freeze of it
// could be armed: a folder this device maps (spec: EP-9), and never the Library
// root, whose freeze would be the whole Library.
it('offers to pack a mapped folder, and hands over which one', () => {
  const { onPack } = mount(listing({ path: 'books/vol-1', files: [file('page.jpg')] }));
  fireEvent.click(screen.getByText('Pack this folder…'));
  expect(onPack).toHaveBeenCalledWith('books/vol-1');
});

it('does not offer to pack an unmapped folder or the Library root', () => {
  expect(draw(listing({ mapped: false, files: [file('cover.png')] }))).not.toContain(
    'Pack this folder…',
  );
  expect(draw(listing({ path: '', files: [file('cover.png')] }))).not.toContain(
    'Pack this folder…',
  );
  expect(draw(listing({ files: [file('cover.png')] }))).toContain('Pack this folder…');
});

// A path the Library holds nothing at is not a folder, and has nothing to pack.
it('does not offer to pack a path the Library holds nothing at', () => {
  expect(draw(listing({ path: 'albums/typo', held: false }))).not.toContain('Pack this folder…');
});

// "Delete…" on a file row and on a folder row hands over what to delete — the
// file by its Entry Path, the folder with everything under it — and opens
// neither: the press is the button's and not the row's.
it('offers to delete a file and a folder, and hands over which', () => {
  const { onDelete, onOpenFile } = mount(
    listing({ folders: [folder('2026', true)], files: [file('cover.png')] }),
  );

  fireEvent.click(screen.getByLabelText('delete cover.png'));
  fireEvent.click(screen.getByLabelText('delete 2026'));

  expect(onDelete.mock.calls).toEqual([
    [{ folder: null, paths: ['albums/cover.png'] }],
    [{ folder: 'albums/2026', paths: [] }],
  ]);
  expect(onOpenFile).not.toHaveBeenCalled();
});

// A deletion is of the Library and touches no file here, so it is offered in a
// folder this device does not map too. A file only added here is not in the
// Library at all, so it has nothing to delete.
it('offers to delete what the Library holds, mapped here or not, and nothing else', () => {
  const unmapped = draw(listing({ mapped: false, files: [file('cover.png')] }));
  expect(unmapped).toContain('aria-label="delete cover.png"');

  const added = draw(
    listing({ files: [file('new.jpg', { state: 'added', container: null })] }),
  );
  expect(added).not.toContain('aria-label="delete new.jpg"');
});
