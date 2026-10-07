import { expect, it } from 'vitest';

import { NO_FOLDER_HERE } from '@coffret/api';

import { dropLine, dropOutcome, PACKED_AFTER } from './dropTarget';
import { NOTHING_AT_THIS_PATH } from './unmapped';

// PK-7: a folder made here that the Library does not have yet takes a book,
// and its pages are packed together.
it('packs a drop onto a made folder as one book', () => {
  const outcome = dropOutcome({ mapped: true, bookDrop: true, freezing: false });
  expect(outcome).toBe('book');
  expect(dropLine(outcome, true)).toBe('drop to pack these pages together as one book');
});

// Any other folder this device maps is synced, one Container per file.
it('adds a drop onto an existing mapped folder one file at a time', () => {
  const outcome = dropOutcome({ mapped: true, bookDrop: false, freezing: false });
  expect(outcome).toBe('files');
  expect(dropLine(outcome, true)).toBe('drop to add these files one at a time');

  // A book being packed elsewhere changes nothing about a sync.
  expect(dropOutcome({ mapped: true, bookDrop: false, freezing: true })).toBe('files');
});

// EP-9: a folder no mapping of this device reaches takes nothing, made here or
// not, and the reason is the one the answer to letting go gives.
it('refuses a drop onto a folder that is not on this device', () => {
  for (const bookDrop of [false, true]) {
    for (const freezing of [false, true]) {
      expect(dropOutcome({ mapped: false, bookDrop, freezing })).toBe('refused');
    }
  }
  expect(dropLine('refused', true)).toBe(`a drop here is not taken — ${NO_FOLDER_HERE}`);
  expect(dropLine('refused', false)).toBe(`a drop here is not taken — ${NOTHING_AT_THIS_PATH}`);
});

// PK-7: books are packed one at a time, so a made folder behind another book
// still takes the drop and says it is packed after that one.
it('packs a drop onto a made folder after the book already being packed', () => {
  const outcome = dropOutcome({ mapped: true, bookDrop: true, freezing: true });
  expect(outcome).toBe('book_after');
  expect(dropLine(outcome, true)).toBe(
    `drop to pack these pages together as one book — ${PACKED_AFTER}`,
  );
  expect(PACKED_AFTER).toContain('packed after that one');
});
