import { expect, it, vi } from 'vitest';

import { Refusal, type Refreshed } from '@coffret/api';

import { askWhatIsNew, refreshedLine } from './refresh';

/** What the server answered, over the shape a refresh always has. */
function refreshed(over: Partial<Refreshed> = {}): Refreshed {
  return { advanced: true, gained: 1, entries: 12, ...over };
}

/** One refresh, with everything it reaches out to recorded. */
function refreshing(ask: () => Promise<Refreshed>) {
  const said: (string | null)[] = [];
  const trouble: (string | null)[] = [];
  const reload = vi.fn();
  return {
    said,
    trouble,
    reload,
    run: () =>
      askWhatIsNew({
        ask,
        line: (line) => said.push(line),
        trouble: (line) => trouble.push(line),
        reload,
      }),
  };
}

// The whole of what the control is for: the server is asked, and the tree and
// the open folder are asked again — which is what puts the rows another device
// committed on the screen.
it('asks the server and then asks the folder and the tree for themselves again', async () => {
  const ask = vi.fn(() => Promise.resolve(refreshed({ gained: 3 })));
  const run = refreshing(ask);

  await run.run();

  expect(ask).toHaveBeenCalledTimes(1);
  expect(run.reload).toHaveBeenCalledTimes(1);
  expect(run.said).toEqual([null, '3 new files']);
  expect(run.trouble).toEqual([null]);
});

// A refresh that found nothing still reloads: the count says what the catalog
// gained, and a folder can have changed in ways no count states.
it('says so when there was nothing new, and still asks the folder', async () => {
  const run = refreshing(() => Promise.resolve(refreshed({ advanced: false, gained: 0 })));

  await run.run();

  expect(run.said.at(-1)).toBe('the Library is up to date');
  expect(run.reload).toHaveBeenCalledTimes(1);
});

// A refusal is the sentence beside the control that was pressed, and nothing
// else moves: a catch-up that stopped may have carried the catalog part of the
// way, but it stopped short of the head, and half an answer drawn under a
// refusal is not one.
it('shows what refused it and leaves the screen alone', async () => {
  const run = refreshing(() =>
    Promise.reject(
      new Refusal('storage', 502, "the Library's Storage did not answer"),
    ),
  );

  await run.run();

  expect(run.trouble).toEqual([null, "the Library's Storage did not answer"]);
  expect(run.reload).not.toHaveBeenCalled();
  expect(run.said).toEqual([null]);
});

// Advancing and gaining are two questions. A commit that only removed Entries
// moved the Library, and calling that "up to date" would tell somebody their
// screen is current at the moment a row leaves it.
it('tells a Library that gained nothing from one that did not change', () => {
  expect(refreshedLine(refreshed({ advanced: true, gained: 0 }))).toBe('the Library changed');
  expect(refreshedLine(refreshed({ advanced: false, gained: 0 }))).toBe(
    'the Library is up to date',
  );
  expect(refreshedLine(refreshed({ gained: 1 }))).toBe('1 new file');
  expect(refreshedLine(refreshed({ gained: -1 }))).toBe('1 file has left the Library');
  expect(refreshedLine(refreshed({ gained: -2 }))).toBe('2 files have left the Library');
});
