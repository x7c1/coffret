import { expect, it } from 'vitest';

import { unmappedLine } from './unmapped';

// Both gestures come to nothing for one reason, and both say so — a click that
// answers with silence is indistinguishable from a screen that has stopped
// working. What differs is the half naming what was tried, so that the line is
// read as an answer to the gesture that was actually made.
it('says what was tried, and says the reason the same way both times', () => {
  expect(unmappedLine('open', true)).toBe(
    'nothing was opened — no folder on this device holds this part of the Library, so ' +
      'there is nowhere here to put the file that row stands for',
  );
  expect(unmappedLine('add', true)).toBe(
    'nothing was added — no folder on this device holds this part of the Library',
  );
});

// A path the Library holds nothing at is not a missing folder on this device.
// The banner over those rows deliberately offers no choice of folder for such a
// path — choosing one for a part of the Library there is none of is choosing
// for nothing — and a notice giving the EP-9 reason anyway would have one
// screen explaining one path two different ways.
it('says the Library holds nothing there rather than blaming the mapping', () => {
  expect(unmappedLine('add', false)).toBe(
    'nothing was added — the Library holds nothing at this path',
  );
  expect(unmappedLine('open', false)).toBe(
    'nothing was opened — the Library holds nothing at this path',
  );
  for (const tried of ['open', 'add'] as const) {
    expect(unmappedLine(tried, false)).not.toContain('this part of the Library');
  }
});
