import { readFileSync } from 'node:fs';

import { describe, expect, it, vi } from 'vitest';

import type { DropAnswer } from './addon.js';
import { MOST_DROPPED_ITEMS, TAKE_DROP, takeDropHandler, type DropAsking } from './drops.js';

const ORIGIN = 'http://127.0.0.1:41234';
const ANSWER: DropAnswer = {
  handle: 'a'.repeat(64),
  files: [{ path: 'book/page-001.jpg', size: 6, mtime: 1_444_000_000_000 }],
  notTaken: [],
};

function harness(explorer: unknown = { id: 'explorer' }) {
  const take = vi.fn<(paths: string[]) => Promise<DropAnswer>>(async () => ANSWER);
  const warn = vi.fn();
  const handle = takeDropHandler(
    () => explorer,
    () => ORIGIN,
    take,
    warn,
  );
  const asking = (sender: unknown, url = `${ORIGIN}/`): DropAsking => ({
    sender,
    senderFrame: { url },
  });
  return { take, warn, handle, asking, explorer };
}

describe('the drop handler', () => {
  it('takes a list of paths from the explorer window and answers with what the addon held', async () => {
    const { take, handle, asking, explorer } = harness();

    await expect(handle(asking(explorer), ['/Users/someone/book'])).resolves.toEqual(ANSWER);
    expect(take).toHaveBeenCalledWith(['/Users/someone/book']);
  });

  it('refuses any other window', async () => {
    const { take, handle, asking, warn } = harness();

    await expect(handle(asking({ id: 'passphrase' }), ['/a'])).rejects.toThrow();
    expect(take).not.toHaveBeenCalled();
    expect(warn).toHaveBeenCalledOnce();
  });

  it('refuses while there is no explorer window', async () => {
    const { take, handle, asking } = harness(null);

    await expect(handle(asking(null), ['/a'])).rejects.toThrow();
    expect(take).not.toHaveBeenCalled();
  });

  it('refuses a frame of the explorer window that is not on its origin', async () => {
    const { take, handle, asking, explorer } = harness();

    await expect(handle(asking(explorer, 'https://elsewhere.example/'), ['/a'])).rejects.toThrow();
    await expect(
      handle({ sender: explorer, senderFrame: null }, ['/a']),
    ).rejects.toThrow();
    expect(take).not.toHaveBeenCalled();
  });

  it('refuses an empty or non-string path list', async () => {
    const { take, handle, asking, explorer, warn } = harness();

    for (const paths of [
      [],
      [1],
      ['/a', 2],
      [null],
      '/a',
      undefined,
      { 0: '/a', length: 1 },
      Array.from({ length: MOST_DROPPED_ITEMS + 1 }, () => '/a'),
    ]) {
      await expect(handle(asking(explorer), paths)).rejects.toThrow();
    }
    expect(take).not.toHaveBeenCalled();
    for (const [said] of warn.mock.calls) {
      expect(said).not.toContain('/a');
    }
  });
});

describe('the explorer preload', () => {
  const preload = readFileSync(new URL('./preload.cts', import.meta.url), 'utf8');

  it('asks over the channel the main process listens on', () => {
    expect(preload).toContain(`'${TAKE_DROP}'`);
  });

  it('maps each dropped file to its path and hands over nothing else', () => {
    expect(preload).toContain('webUtils.getPathForFile(file)');
    expect(preload).toContain("exposeInMainWorld('coffretDesktop'");
  });
});
