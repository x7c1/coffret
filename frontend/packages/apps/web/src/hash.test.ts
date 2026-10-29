import { expect, it } from 'vitest';

import { AT_ROOT, parseHash, sheetOpen, toHash, type ViewState } from './hash';

const ROUND_TRIPPED: ViewState[] = [
  AT_ROOT,
  { folder: 'albums', open: null },
  { folder: 'albums/2026/08', open: null },
  { folder: 'albums/2026', open: 'albums/2026/spring.jpg' },
  { folder: '', open: 'notes.txt' },
  // EP-1: a name is whatever the user called their file, accents and all.
  { folder: 'albums', open: 'albums/café.jpg' },
  // And `&` and `=` are characters a filename may hold like any other.
  { folder: 'a&b', open: 'a&b/x=1.jpg' },
  // A space is the commonest of them, and `%` is the one that would be read
  // back as an escape if it were not escaped itself — `x%20y` is a five-
  // character name and must not come back as `x y`.
  { folder: 'my albums', open: 'my albums/a b.jpg' },
  { folder: '100% done', open: '100% done/x%20y.jpg' },
  // A `+` means a plus in a path and a space in a form encoding, so it goes
  // through as `%2B` rather than being left for the parser to read either way.
  { folder: 'a+b', open: 'a+b/c+d.jpg' },
  // The sheet open on a folder, showing its cover: a closed one, listed in
  // the folder it stands in, and an open one, which is its own context.
  { folder: 'books', open: null, cover: 'books/vol-1' },
  { folder: 'books/vol-1', open: null, cover: 'books/vol-1' },
];

it('carries a state through the hash and back', () => {
  for (const state of ROUND_TRIPPED) {
    expect(parseHash(toHash(state))).toEqual(state);
  }
});

// The separator is the one character that can never be part of a name, so
// leaving it as itself is unambiguous — and it is what keeps the address bar
// readable.
it('spells the separator as itself', () => {
  expect(toHash({ folder: 'albums/2026', open: 'albums/2026/spring.jpg' })).toBe(
    '#path=albums/2026&open=albums/2026/spring.jpg',
  );
  expect(toHash(AT_ROOT)).toBe('#');
});

// A hash is something a person can type. One that names nothing is the screen
// the explorer opens at, not an error about a URL.
it('reads anything it cannot make sense of as the Library root', () => {
  for (const hash of ['', '#', '#nonsense', '#path=', '#open=']) {
    expect(parseHash(hash)).toEqual(AT_ROOT);
  }
});

it('reads a hash written without its leading marker', () => {
  expect(parseHash('path=albums')).toEqual({ folder: 'albums', open: null });
});

// EP-1: an Entry Path is NFC, and so is every path the parsed state is compared
// with. A hash typed in NFD names the same place and comes back as the NFC path.
it('reads a hash spelled in NFD as the NFC paths the Library holds', () => {
  const folder = 'caf\u0065\u0301';
  const open = `${folder}/r\u0065\u0301sum\u0065\u0301.jpg`;
  expect(folder).not.toBe('café');

  const parsed = parseHash(`#path=${encodeURIComponent(folder)}&open=${encodeURIComponent(open)}`);
  expect(parsed).toEqual({ folder: 'café', open: 'café/résumé.jpg' });

  expect(parseHash(`#path=${folder}`)).toEqual({ folder: 'café', open: null });
});

// The Library admits a file and a folder at one path, so the URL says which
// the sheet is open on — and one row at a time, the file where both are named.
it('tells a sheet open on a folder from one open on a file', () => {
  expect(toHash({ folder: 'books', open: null, cover: 'books/vol-1' })).toBe(
    '#path=books&cover=books/vol-1',
  );
  expect(parseHash('#path=books&open=books/a.jpg&cover=books/vol-1')).toEqual({
    folder: 'books',
    open: 'books/a.jpg',
  });
  expect(parseHash('#cover=')).toEqual(AT_ROOT);
  expect(sheetOpen(parseHash('#path=books&cover=books/vol-1'))).toBe(true);
  expect(sheetOpen(parseHash('#path=books&open=books/a.jpg'))).toBe(true);
  expect(sheetOpen(parseHash('#path=books'))).toBe(false);
});
