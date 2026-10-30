import { expect, it } from 'vitest';

import { LIBRARY_ROOT, locationOf } from './location';
import { fileKey, folderKey } from './tree';

// The focused row's containing folders and its own name, one name per level.
it('names a top-level folder by its name alone', () => {
  expect(locationOf(folderKey('Comics'))).toBe('Comics');
});

it('names a nested file by the folders it stands in and its name', () => {
  expect(locationOf(fileKey('Comics/Series A/Vol. 03/001.jpg'))).toBe('Comics / Series A / Vol. 03 / 001.jpg');
});

// Nothing focused is the Library root, which has no row of its own. The word
// is pinned here because the details pane and the reader say it too.
it('names the Library root where nothing is focused', () => {
  expect(LIBRARY_ROOT).toBe('Library');
  expect(locationOf(null)).toBe(LIBRARY_ROOT);
});
