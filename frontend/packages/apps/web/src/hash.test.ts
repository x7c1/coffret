import { expect, it } from 'vitest';

import { AT_ROOT, parseHash, toHash, type ViewState } from './hash';

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
