import { expect, it } from 'vitest';

import { NO_FOLDER_HERE } from '@coffret/api';

import { dropLine, dropOutcome, PACKED_AFTER, TAKEN } from './dropTarget';
import { NOTHING_AT_THIS_PATH } from './unmapped';

// What a drop is added as follows what was dropped: the line says a folder is
// asked about and files on their own go one by one, wherever the folder on
// the screen came from.
it('says a folder is asked about and files are added one by one', () => {
  const outcome = dropOutcome({ mapped: true, freezing: false });
  expect(outcome).toBe('taken');
  expect(dropLine(outcome, true)).toBe(TAKEN);
  expect(TAKEN).toContain('a folder is asked about first');
  expect(TAKEN).toContain('files on their own are added one by one');
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
