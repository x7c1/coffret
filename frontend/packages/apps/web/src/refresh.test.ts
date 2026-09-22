import { expect, it, vi } from 'vitest';

import { Refusal, type Catalog, type Refreshed } from '@coffret/api';

import { askWhatIsNew, ASKING, catalogLine, catchUpLanded, refreshedLine } from './refresh';

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

/** How the catalog stands, over the shape the answer always has. */
function catalog(over: Partial<Catalog> = {}): Catalog {
  return { state: 'caught_up', trouble: null, ...over };
}

// The one sentence an empty explorer cannot say for itself. A device fresh from
// `join` whose catch-up did not land shows nothing, and shows it in exactly the
// way a Library with nothing in it does — so a person reading the rows cannot
// tell "there is nothing here" from "this is not all of it".
it('says why an empty Library may not be an empty Library', () => {
  expect(catalogLine(catalog())).toBeNull();
  expect(catalogLine(null)).toBeNull();

  const catchingUp = catalogLine(catalog({ state: 'catching_up' }));
  expect(catchingUp).not.toBeNull();
  expect(catchingUp).toContain('catching up');

  const behind = catalogLine(
    catalog({
      state: 'behind',
      trouble: { error: 'storage', message: 'Storage did not answer' },
    }),
  );
  expect(behind).toContain('Storage did not answer');
  // And ends on the move that gets out of it, named by the words written on the
  // control: the sentence stands at the top of the screen and the control is in
  // the bar at the bottom, among three other buttons offering a second attempt.
  expect(behind).toContain(`"${ASKING}"`);
});

// The two are different states and must read as different sentences: one is
// something to wait through, and the other is something to press a button
// about.
it('tells a catch-up that is running from one that did not finish', () => {
  expect(catalogLine(catalog({ state: 'catching_up' }))).not.toBe(
    catalogLine(catalog({ state: 'behind' })),
  );
});

// A server that said it was behind without saying what stopped it still leaves
// a person with the difference that matters, rather than with nothing.
it('says the catalog is behind even where nothing named what stopped it', () => {
  const behind = catalogLine(catalog({ state: 'behind' }));
  expect(behind).toContain('has not caught up');
});

// What the banner promises while a catch-up runs: the rest arrives when it
// lands. Somebody who waited rather than pressing anything is the reading that
// would otherwise end worst — the banner goes away by itself, and the rows the
// catalog held before it are left with nothing saying they are not all of it.
it('asks the folder and the tree again when a catch-up lands', () => {
  expect(catchUpLanded('catching_up', 'caught_up')).toBe(true);
  // The same news in fewer words: a window polling every so often can be told
  // `behind` and then `caught_up`, the run that fixed it having begun and ended
  // between two answers.
  expect(catchUpLanded('behind', 'caught_up')).toBe(true);
});

// And nothing else is that news. A page that came up to a caught-up server has
// just asked for both, and a catch-up that stopped short of the head is the one
// state a refused refresh already declines to draw.
it('asks again for the landing and for no other answer', () => {
  expect(catchUpLanded(null, 'caught_up')).toBe(false);
  expect(catchUpLanded('caught_up', 'caught_up')).toBe(false);
  expect(catchUpLanded('catching_up', 'behind')).toBe(false);
  expect(catchUpLanded('catching_up', 'catching_up')).toBe(false);
});
