import { expect, it } from 'vitest';

import { size, time } from './humanize';

it('states a size in the largest unit that leaves a readable number', () => {
  expect(size(0)).toBe('0 B');
  expect(size(999)).toBe('999 B');
  expect(size(1000)).toBe('1.0 kB');
  expect(size(1_500_000)).toBe('1.5 MB');
  expect(size(2_000_000_000)).toBe('2.0 GB');
});

// A count of seconds no calendar reaches is what the server says `null` for,
// rather than naming a moment that is not the file's.
it('shows no time where the Entry carries none', () => {
  expect(time(null)).toBe('—');
  expect(time('not a date')).toBe('—');
});

it('shows a time it was given', () => {
  expect(time('2023-11-14T22:13:20Z')).not.toBe('—');
});
