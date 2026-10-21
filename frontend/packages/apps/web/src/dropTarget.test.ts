import { expect, it } from 'vitest';

import { NO_FOLDER_HERE, type Listing } from '@coffret/api';

import { dropLine, dropOutcome, mappedAt, PACKED_AFTER, TAKEN } from './dropTarget';
import { NOTHING_AT_THIS_PATH } from './unmapped';

// Every drop is confirmed before anything is sent — where it lands and what it
// is added as — and the line while the drag is in the air says so, wherever the
// folder it is over came from.
it('says the drop is confirmed, where and how, before anything is sent', () => {
  const outcome = dropOutcome({ mapped: true, freezing: false });
  expect(outcome).toBe('taken');
  expect(dropLine(outcome, true)).toBe(TAKEN);
  expect(TAKEN).toContain('where it goes, and how, is confirmed before anything is sent');
});

// EP-9: a folder no mapping of this device reaches takes nothing, and the
// reason is the one the answer to letting go gives.
it('refuses a drop onto a folder that is not on this device', () => {
  for (const freezing of [false, true]) {
    expect(dropOutcome({ mapped: false, freezing })).toBe('refused');
  }
  expect(dropLine('refused', true)).toBe(`a drop here is not taken — ${NO_FOLDER_HERE}`);
  expect(dropLine('refused', false)).toBe(`a drop here is not taken — ${NOTHING_AT_THIS_PATH}`);
});

// PK-7: freezes run one at a time, so a drop while one is running is still
// taken and says a folder added as a Pack now is packed after that one.
it('says a folder added as a Pack while a freeze runs is packed after it', () => {
  const outcome = dropOutcome({ mapped: true, freezing: true });
  expect(outcome).toBe('taken_after');
  expect(dropLine(outcome, true)).toBe(`${TAKEN} — ${PACKED_AFTER}`);
  expect(PACKED_AFTER).toContain('packed after it');
});

/** The listing of a Library root, mapped or not, over two top-level folders. */
function root(mapped: boolean): Listing {
  return {
    path: '',
    mapped,
    held: true,
    folders: [
      { name: 'albums', path: 'albums', mapped: true },
      { name: 'books', path: 'books', mapped },
    ],
    files: [],
    folders_on_disk: [],
    empty_mapped_prefixes: [],
  };
}

// EP-9: a mapping stands for a top-level folder and everything under it, or for
// the root — so the root's listing answers for every row of the tree.
it('reads whether a tree row is mapped off the listing of the root', () => {
  expect(mappedAt('', root(false))).toBe(false);
  expect(mappedAt('', root(true))).toBe(true);
  expect(mappedAt('albums/2026/08', root(false))).toBe(true);
  expect(mappedAt('books/vol-1', root(false))).toBe(false);
  // A top-level folder the root does not list is reached where the root is,
  // and otherwise is not known from here.
  expect(mappedAt('stranded/vol-1', root(true))).toBe(true);
  expect(mappedAt('stranded/vol-1', root(false))).toBeNull();
  // A top-level folder this device maps with nothing under it is reached,
  // whether the root is or not.
  const empty = { ...root(false), empty_mapped_prefixes: ['comics'] };
  expect(mappedAt('comics', empty)).toBe(true);
  expect(mappedAt('comics/Series', empty)).toBe(true);
  // Nothing is known before the root has been listed.
  expect(mappedAt('albums', null)).toBeNull();
});
