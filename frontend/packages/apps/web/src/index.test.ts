import { expect, it } from 'vitest';

import { appName } from './index';

it('names the app', () => {
  expect(appName).toBe('coffret');
});
