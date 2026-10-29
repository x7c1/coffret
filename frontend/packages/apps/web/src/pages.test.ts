import { expect, it } from 'vitest';

import type { ListedFile } from '@coffret/api';

import { pageAt, pagesOf } from './pages';

/** One row of a listing, as much of one as these cases read. */
function row(name: string, openable: boolean, state: 'present' | 'remote' = 'present'): ListedFile {
  return {
    name,
    path: `albums/${name}`,
    size: 1,
    mtime: null,
    deposited_at: null,
    state,
    container: 'one-file',
    reserved: false,
    openable,
    content_type: openable ? 'image/jpeg' : 'application/octet-stream',
  };
}

const LISTED = [
  row('a.jpg', true),
  row('notes.txt', false),
  row('b.jpg', true, 'remote'),
  row('archive.zip', false),
  row('c.jpg', true),
];

// Every stored file is a row whatever its format, and only the ones a browser
// draws are pages: what the reader keeps ready around the focus is the
// subsequence.
it('takes the openable files as the pages, in the listing order', () => {
  expect(pagesOf(LISTED).map((page) => page.name)).toEqual(['a.jpg', 'b.jpg', 'c.jpg']);
  expect(pagesOf(LISTED).map((page) => page.remote)).toEqual([false, true, false]);
  expect(pagesOf(LISTED)[0].path).toBe('albums/a.jpg');
});

it('finds where one file stands among the pages, and says when it is not one', () => {
  const pages = pagesOf(LISTED);

  expect(pageAt(pages, 'albums/b.jpg')).toBe(1);
  expect(pageAt(pages, 'albums/notes.txt')).toBeNull();
  expect(pageAt(pages, 'albums/gone.jpg')).toBeNull();
});

it('makes no pages of a folder holding nothing a browser draws', () => {
  expect(pagesOf([row('notes.txt', false)])).toEqual([]);
  expect(pagesOf([])).toEqual([]);
});
