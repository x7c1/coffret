import { expect, it } from 'vitest';

import { addName } from './addName';

// LA-35: what the server takes as an add's name, and a new one every time.
it('draws a name the server takes, and a different one for each add', () => {
  const first = addName();
  expect(first).toMatch(/^[0-9a-f]{32}$/);
  expect(addName()).not.toBe(first);
});
