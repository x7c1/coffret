import { expect, it } from 'vitest';

import { prefetchTargets } from './prefetch';

it('prefetches nearest-first with a forward bias', () => {
  expect(prefetchTargets(10, 3000, 2)).toEqual([11, 9, 12, 8]);
});

it('clamps prefetch targets at library bounds', () => {
  expect(prefetchTargets(0, 3000, 2)).toEqual([1, 2]);
  expect(prefetchTargets(2999, 3000, 2)).toEqual([2998, 2997]);
});

// One page in a folder is one page: there is nothing on either side of it.
it('prefetches nothing around a folder holding one page', () => {
  expect(prefetchTargets(0, 1, 3)).toEqual([]);
});
