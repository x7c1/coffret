import { afterEach, expect, it, vi } from 'vitest';

import { isRefusal } from './refusal';
import { addFiles, type Added } from './upload';
import uploadBudget from './upload-budget.json';

afterEach(() => {
  vi.unstubAllGlobals();
});

/**
 * A file that says it is `size` bytes long and holds none of them.
 *
 * Which is all a drop past the budget has to be to be refused before it is
 * sent: what is weighed is what the files say they come to, and a case that
 * had to hold 64 GiB to say so would be the case that could not be run.
 */
function claiming(path: string, size: number): Added {
  return { path, file: { size, name: path } as File };
}

// LA-9, LA-10: a drop whose files already come to more than the budget is
// refused before anything is sent (`addFiles` says why), in the server's own
// sentence, and nothing landed.
it('refuses a drop past the request budget before sending it, in the server’s words', async () => {
  const fetched = vi.fn();
  vi.stubGlobal('fetch', fetched);

  const half = uploadBudget.request_bytes / 2;
  const thrown: unknown = await addFiles('books', [
    claiming('page-001.jpg', half),
    claiming('page-002.jpg', half + 1),
  ]).catch((refusal: unknown) => refusal);

  expect(fetched).not.toHaveBeenCalled();
  expect(isRefusal(thrown)).toBe(true);
  if (isRefusal(thrown)) {
    expect(thrown.kind).toBe('bad_request');
    expect(thrown.status).toBe(413);
    expect(thrown.message).toBe(uploadBudget.request_too_large);
    expect(thrown.written).toEqual([]);
  }
});

// The budget is a boundary: files that come to exactly it are the server's to
// weigh, because the framing on top is what decides, and only the server sees
// the body the browser makes.
it('sends a drop whose files come to no more than the budget', async () => {
  const fetched = vi.fn(() => Promise.resolve(Response.json({ written: [], refused: [] })));
  vi.stubGlobal('fetch', fetched);
  vi.stubGlobal(
    'FormData',
    class {
      append() {}
    },
  );

  await expect(
    addFiles('books', [claiming('page-001.jpg', uploadBudget.request_bytes)]),
  ).resolves.toEqual({ written: [], refused: [] });
  expect(fetched).toHaveBeenCalledTimes(1);
});
