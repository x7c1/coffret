import { expect, it } from 'vitest';

import { unmappedLine } from './unmapped';

// Both gestures come to nothing for one reason, and both say so — a click that
// answers with silence is indistinguishable from a screen that has stopped
// working. What differs is the half naming what was tried, so that the line is
// read as an answer to the gesture that was actually made.
it('says what was tried, and says the reason the same way both times', () => {
  expect(unmappedLine('open', true, [])).toBe(
    'nothing was opened — no folder on this device holds this part of the Library, so ' +
      'there is nowhere here to put the file that row stands for',
  );
  expect(unmappedLine('add', true, [])).toBe(
    'nothing was added — no folder on this device holds this part of the Library',
  );
});

// A drop onto an unmapped Library root is refused whole although a top-level
// folder below it is on this device and could have taken its share. The refusal
// names those folders: without them a person is left to find out by trial which
// part of their Library this device has.
it('names the folders below that would have taken the drop', () => {
  expect(unmappedLine('add', true, ['albums'])).toBe(
    'nothing was added — no folder on this device holds this part of the Library; the ' +
      'folders below it that do are albums, and a drop onto one of those is taken',
  );
  expect(unmappedLine('add', true, ['albums', 'books'])).toBe(
    'nothing was added — no folder on this device holds this part of the Library; the ' +
      'folders below it that do are albums and books, and a drop onto one of those is taken',
  );
});

// Two names and a count of the rest: the notice area is one line, and a root of
// thirty top-level folders would otherwise put a paragraph in it.
it('names two of them and counts the rest', () => {
  expect(unmappedLine('add', true, ['albums', 'books', 'notes', 'scans'])).toContain(
    'the folders below it that do are albums, books and 2 more',
  );
});

// A file row stands in this folder, and no folder beside it can hold that file:
// there is no other place to offer, so the click's answer names none.
it('offers no other folder to a click', () => {
  expect(unmappedLine('open', true, ['albums', 'books'])).toBe(
    unmappedLine('open', true, []),
  );
});

// A path the Library holds nothing at is not a mapping failure. The banner over
// those rows deliberately says nothing about mapping such a path — being told to
// map a part of the Library there is none of sends somebody to a terminal for
// nothing — and a notice giving the mapping reason anyway would have one screen
// explaining one path two different ways.
it('says the Library holds nothing there rather than blaming the mapping', () => {
  expect(unmappedLine('add', false, [])).toBe(
    'nothing was added — the Library holds nothing at this path',
  );
  expect(unmappedLine('open', false, [])).toBe(
    'nothing was opened — the Library holds nothing at this path',
  );
  for (const tried of ['open', 'add'] as const) {
    expect(unmappedLine(tried, false, [])).not.toContain('this part of the Library');
  }
});
