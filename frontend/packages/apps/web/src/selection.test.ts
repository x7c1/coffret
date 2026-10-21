import { expect, it } from 'vitest';

import type { ListedFile, Listing } from '@coffret/api';

import { chosen, everyToggled, kept, selectable, selectedLine, toggled } from './selection';

function file(name: string, over: Partial<ListedFile> = {}): ListedFile {
  return {
    name,
    path: `books/${name}`,
    size: 12,
    mtime: null,
    deposited_at: null,
    state: 'remote',
    container: 'pack-1',
    reserved: false,
    openable: true,
    content_type: 'image/jpeg',
    ...over,
  };
}

function listing(files: ListedFile[], path = 'books'): Listing {
  return { path, mapped: true, held: true, folders: [], files, folders_on_disk: [], empty_mapped_prefixes: [] };
}

const book = listing([
  file('p1.jpg'),
  file('new.jpg', { state: 'added', container: null }),
  file('p2.jpg'),
  file('p3.jpg'),
]);

// A file only added here is not in the Library, and has nothing to delete.
it('offers only the files the Library holds', () => {
  expect(selectable(book)).toEqual(['books/p1.jpg', 'books/p2.jpg', 'books/p3.jpg']);
});

// The order a deletion names its files in is the listing's, whatever order
// they were picked in.
it('names what is picked in the listing order', () => {
  let picked = toggled(null, book, 'books/p3.jpg');
  picked = toggled(picked, book, 'books/p1.jpg');
  expect(chosen(picked, book)).toEqual(['books/p1.jpg', 'books/p3.jpg']);

  picked = toggled(picked, book, 'books/p3.jpg');
  expect(chosen(picked, book)).toEqual(['books/p1.jpg']);
});

it('picks every file and clears them again from the header', () => {
  const all = everyToggled(toggled(null, book, 'books/p2.jpg'), book);
  expect(chosen(all, book)).toEqual(['books/p1.jpg', 'books/p2.jpg', 'books/p3.jpg']);
  expect(everyToggled(all, book)).toBeNull();
});

// A selection belongs to its folder, and holds only what the listing offers.
it('keeps nothing across folders and nothing the listing dropped', () => {
  const picked = toggled(toggled(null, book, 'books/p1.jpg'), book, 'books/p2.jpg');
  expect(kept(picked, book)).toBe(picked);

  const elsewhere = listing([file('p1.jpg')], 'albums');
  expect(chosen(picked, { ...elsewhere, files: book.files })).toEqual([]);
  expect(kept(picked, elsewhere)).toBeNull();

  const reloaded = listing([file('p2.jpg'), file('p3.jpg')]);
  expect(chosen(kept(picked, reloaded), reloaded)).toEqual(['books/p2.jpg']);
  expect(kept(picked, listing([file('p3.jpg')]))).toBeNull();
});

it('says how many are picked', () => {
  expect(selectedLine(2)).toBe('2 selected');
});
