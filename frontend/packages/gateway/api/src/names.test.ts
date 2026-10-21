import { expect, it } from 'vitest';

import { componentDefect, MANAGEMENT_AREA, reservedName } from './names';

// EP-2: one component, and nothing an Entry Path cannot hold as one.
it('says what is wrong with a name as one component of an Entry Path', () => {
  expect(componentDefect('')).toBe('empty');
  expect(componentDefect('comics/Series')).toBe('separator');
  expect(componentDefect('a\u0000b')).toBe('nul');
  expect(componentDefect('.')).toBe('relative');
  expect(componentDefect('..')).toBe('relative');
  expect(componentDefect('...')).toBeNull();
  expect(componentDefect('Vol 1')).toBeNull();
});

// EP-11, EP-14: the three reserved names, as the server answers them, and a name
// of the person's own.
it('names the reserved names the server refuses a drop for', () => {
  expect(reservedName('.coffret-fetch-incoming-x.part')).toBe('scratch');
  expect(reservedName(MANAGEMENT_AREA)).toBe('management_area');
  expect(reservedName('.COFFRET')).toBe('folds_to_management_area');
  expect(reservedName('.Coffret')).toBe('folds_to_management_area');
  expect(reservedName('.coffret-derived')).toBeNull();
  expect(reservedName('comics')).toBeNull();
});
