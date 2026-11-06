import { describe, expect, it } from 'vitest';

import { nextToOpen, reportLine, summarized } from './measured';

describe('summarized', () => {
  it('takes the middle of an odd count and the mean of the middle two of an even one', () => {
    expect(summarized([5, 1, 9])).toEqual({ median: 5, worst: 9 });
    expect(summarized([4, 1, 2, 8])).toEqual({ median: 3, worst: 8 });
  });

  it('refuses nothing timed', () => {
    expect(() => summarized([])).toThrow();
  });
});

describe('reportLine', () => {
  it('writes the median and the worst to a tenth of a millisecond', () => {
    expect(reportLine('listing at the root', { median: 12.345, worst: 20 }, '5 rows')).toBe(
      '   listing at the root                      median      12.3 ms   worst      20.0 ms  (5 rows)',
    );
  });
});

describe('nextToOpen', () => {
  it('opens the closed folder with the most files below it, the first of a tie', () => {
    const files = new Map([
      ['Comics', 30_000],
      ['Novels', 20_000],
      ['Loose scans', 20_000],
    ]);
    expect(nextToOpen(['Novels', 'Comics', 'Loose scans'], files)).toBe('Comics');
    expect(nextToOpen(['Loose scans', 'Novels'], files)).toBe('Loose scans');
    expect(nextToOpen(['Unknown', 'Novels'], files)).toBe('Novels');
    expect(nextToOpen(['Unknown'], files)).toBe('Unknown');
    expect(nextToOpen([], files)).toBeUndefined();
  });
});
