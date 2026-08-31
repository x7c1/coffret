import { expect, it } from 'vitest';

import type { ListedFile } from '@coffret/api';

import { pageAt, pagesOf, stepped } from './pages';

/** One row of a listing, as much of one as these cases read. */
function row(name: string, openable: boolean, state: 'present' | 'remote' = 'present'): ListedFile {
  return {
    name,
    path: `albums/${name}`,
    size: 1,
    mtime: null,
    state,
    container: 'one-file',
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
// draws are pages: what the reader turns through is the subsequence.
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

// A step is a step through the pages, so the rows between two of them are
// stepped over rather than stopped on.
it('steps over the files a browser draws nothing from', () => {
  const pages = pagesOf(LISTED);

  expect(pages[stepped(pages, 0, 1)].name).toBe('b.jpg');
  expect(pages[stepped(pages, 2, -1)].name).toBe('b.jpg');
});

// The end of a folder is where a reader stops: a turn that wrapped round would
// look like the folder had been reordered under them.
it('clamps at both ends rather than wrapping', () => {
  const pages = pagesOf(LISTED);

  expect(stepped(pages, 0, -1)).toBe(0);
  expect(stepped(pages, 2, 1)).toBe(2);
  expect(stepped(pages, 0, -5)).toBe(0);
  expect(stepped(pages, 2, 5)).toBe(2);
});

it('makes no pages of a folder holding nothing a browser draws', () => {
  expect(pagesOf([row('notes.txt', false)])).toEqual([]);
  expect(pagesOf([])).toEqual([]);
});
