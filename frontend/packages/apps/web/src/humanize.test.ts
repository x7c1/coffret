import { afterEach, expect, it } from 'vitest';

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

const zone = process.env.TZ;

afterEach(() => {
  if (zone === undefined) {
    delete process.env.TZ;
  } else {
    process.env.TZ = zone;
  }
});

/** Reads `iso` as a device in the zone `tz` shows it. */
function shownIn(tz: string, iso: string): string {
  process.env.TZ = tz;
  return time(iso);
}

// RFC 3339 to the second, the wall clock the reader's own, and the offset
// beside it saying which zone that is — the offset in force at that moment,
// daylight saving and half-hour zones included.
it('shows a time as RFC 3339 in the reader’s own zone', () => {
  expect(shownIn('Asia/Tokyo', '2023-11-14T22:13:20Z')).toBe('2023-11-15T07:13:20+09:00');
  expect(shownIn('America/New_York', '2015-10-04T23:06:40Z')).toBe('2015-10-04T19:06:40-04:00');
  expect(shownIn('America/New_York', '2015-12-04T23:06:40Z')).toBe('2015-12-04T18:06:40-05:00');
  expect(shownIn('Asia/Kolkata', '2023-11-14T22:13:20Z')).toBe('2023-11-15T03:43:20+05:30');
  expect(shownIn('UTC', '2023-11-14T22:13:20Z')).toBe('2023-11-14T22:13:20+00:00');
});

// FM-9: a time before 1970 is a time like any other.
it('shows a time before 1970', () => {
  expect(shownIn('UTC', '1969-12-31T23:59:58Z')).toBe('1969-12-31T23:59:58+00:00');
  expect(shownIn('America/New_York', '1969-07-20T20:17:40Z')).toBe('1969-07-20T16:17:40-04:00');
});
