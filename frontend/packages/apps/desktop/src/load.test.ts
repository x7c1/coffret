import { expect, it, vi } from 'vitest';

import { BLANK, loadAfresh, type Loadable } from './load.js';

it('passes through an empty page, then clears the history', async () => {
  const steps: string[] = [];
  const contents: Loadable = {
    loadURL: vi.fn(async (url: string) => {
      steps.push(`load ${url}`);
    }),
    navigationHistory: { clear: () => steps.push('clear') },
  };
  await loadAfresh(contents, 'http://127.0.0.1:1/#door=T');
  expect(steps).toEqual([`load ${BLANK}`, 'load http://127.0.0.1:1/#door=T', 'clear']);
});

it('stops where a load fails', async () => {
  const clear = vi.fn();
  const contents: Loadable = {
    loadURL: vi.fn(() => Promise.reject(new Error('ERR_CONNECTION_REFUSED'))),
    navigationHistory: { clear },
  };
  await expect(loadAfresh(contents, 'http://127.0.0.1:1/')).rejects.toThrow();
  expect(clear).not.toHaveBeenCalled();
});
