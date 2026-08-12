import { expect, it } from 'vitest';

import { columnCount, prefetchTargets, rowCount, rowItems } from './layout';

it('computes columns from container width, never below one', () => {
  expect(columnCount(1280, 200)).toBe(6);
  expect(columnCount(150, 200)).toBe(1);
});

it('computes row count including a partial last row', () => {
  expect(rowCount(3000, 6)).toBe(500);
  expect(rowCount(3001, 6)).toBe(501);
  expect(rowCount(0, 6)).toBe(0);
});

it('lists the items of a row, truncated at the end of the library', () => {
  expect(rowItems(0, 6, 3001)).toEqual([0, 1, 2, 3, 4, 5]);
  expect(rowItems(500, 6, 3001)).toEqual([3000]);
});

it('prefetches nearest-first with a forward bias', () => {
  expect(prefetchTargets(10, 3000, 2)).toEqual([11, 9, 12, 8]);
});

it('clamps prefetch targets at library bounds', () => {
  expect(prefetchTargets(0, 3000, 2)).toEqual([1, 2]);
  expect(prefetchTargets(2999, 3000, 2)).toEqual([2998, 2997]);
});
